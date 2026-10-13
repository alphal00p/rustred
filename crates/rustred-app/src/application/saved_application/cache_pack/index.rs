use std::{
    fs::{self, File},
    io::{self, Read, Write},
    mem::size_of,
    path::Path,
};

use rustred::persistence::application_cache::{ApplicationCacheBinding, ApplicationCacheKey};

use super::*;
use crate::AppError;

const MAGIC: &[u8; 16] = b"RR-CACHE-PACK-01";
pub(super) const MANIFEST: &str = "cache.rrindex";
pub(super) const MAX_FRAME: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, bincode::Encode, bincode::Decode)]
pub(super) struct Chunk {
    pub digest: [u8; 32],
    pub bytes: u64,
    pub records: usize,
    pub first: ApplicationCacheKey,
    pub last: ApplicationCacheKey,
}
impl Chunk {
    pub fn file_name(&self) -> String {
        format!("{}.rrchunk", blake3::Hash::from_bytes(self.digest).to_hex())
    }
}

#[derive(Clone, Debug, bincode::Encode, bincode::Decode)]
pub(super) struct Manifest {
    pub binding: ApplicationCacheBinding,
    pub bundle_managed: bool,
    pub generation: u64,
    /// Runs are newest-last; each run has sorted, nonoverlapping chunk ranges.
    pub runs: Vec<Vec<Chunk>>,
}

pub(super) fn validate(
    manifest: &Manifest,
    expected: &ApplicationCacheBinding,
    limits: SavedApplicationCachePackLimits,
) -> Result<SavedApplicationCachePackStatistics, AppError> {
    expected.validate(limits.chunk).map_err(fail)?;
    if manifest.binding != *expected {
        return Err(fail("artifact, family, arity or cache layer mismatch"));
    }
    if manifest.runs.len() > limits.max_runs {
        return Err(AppError::limit("cache run directory exceeds allowance"));
    }
    let mut result = SavedApplicationCachePackStatistics::default();
    result.directory_owned_bytes = size_of::<Manifest>()
        .saturating_add(manifest.binding.family_fingerprint.capacity())
        .saturating_add(manifest.binding.layer.capacity())
        .saturating_add(
            manifest
                .runs
                .capacity()
                .saturating_mul(size_of::<Vec<Chunk>>()),
        );
    for run in &manifest.runs {
        if run.is_empty() {
            return Err(fail("empty immutable cache run"));
        }
        result.directory_owned_bytes = result
            .directory_owned_bytes
            .saturating_add(run.capacity().saturating_mul(size_of::<Chunk>()));
        let mut previous = None;
        for chunk in run {
            chunk
                .first
                .validate(expected.physical_arity)
                .map_err(fail)?;
            chunk.last.validate(expected.physical_arity).map_err(fail)?;
            if chunk.first > chunk.last
                || previous.is_some_and(|key| key >= &chunk.first)
                || chunk.records == 0
                || chunk.records > limits.chunk.max_records
            {
                return Err(fail("invalid, overlapping or unordered cache chunk range"));
            }
            previous = Some(&chunk.last);
            if chunk.bytes == 0 || chunk.bytes > limits.chunk.binary.max_program_bytes as u64 {
                return Err(AppError::limit("cache chunk byte allowance exceeded"));
            }
            result.chunks = result
                .chunks
                .checked_add(1)
                .ok_or_else(|| fail("chunk count overflow"))?;
            result.records = result
                .records
                .checked_add(chunk.records)
                .ok_or_else(|| fail("cache record count overflow"))?;
            result.referenced_chunk_bytes = result
                .referenced_chunk_bytes
                .checked_add(chunk.bytes)
                .ok_or_else(|| fail("cache byte count overflow"))?;
            // Key constructors/decoders retain one vector per field. The
            // binary framing constrains both actual lengths before use.
            result.directory_owned_bytes = result
                .directory_owned_bytes
                .saturating_add(chunk.first.owned_heap_bytes())
                .saturating_add(chunk.last.owned_heap_bytes());
        }
    }
    if result.chunks > limits.max_chunks
        || result.referenced_chunk_bytes > limits.max_disk_bytes
        || result.directory_owned_bytes > limits.max_directory_owned_bytes
    {
        return Err(AppError::limit(
            "cache pack directory or referenced disk allowance exceeded",
        ));
    }
    Ok(result)
}

pub(super) fn encode(
    manifest: &Manifest,
    limits: SavedApplicationCachePackLimits,
) -> Result<Vec<u8>, AppError> {
    validate(manifest, &manifest.binding, limits)?;
    if limits.max_manifest_bytes > MAX_FRAME {
        return Err(fail("manifest allowance exceeds supported bounded frame"));
    }
    let mut output = BoundedBytes::new(limits.max_manifest_bytes.saturating_sub(32));
    output.write_all(MAGIC).map_err(fail)?;
    // Structural framing only; no algebra values are present in this index.
    bincode::encode_into_std_write(manifest, &mut output, bincode::config::standard())
        .map_err(fail)?;
    let mut bytes = output.0;
    let digest = blake3::hash(&bytes);
    bytes.extend_from_slice(digest.as_bytes());
    if bytes.len() > limits.max_manifest_bytes {
        return Err(AppError::limit("cache manifest byte allowance exceeded"));
    }
    Ok(bytes)
}

pub(super) fn decode(
    bytes: &[u8],
    expected: &ApplicationCacheBinding,
    limits: SavedApplicationCachePackLimits,
) -> Result<(Manifest, SavedApplicationCachePackStatistics), AppError> {
    if limits.max_manifest_bytes > MAX_FRAME
        || bytes.len() > limits.max_manifest_bytes
        || bytes.len() < MAGIC.len() + 32
        || !bytes.starts_with(MAGIC)
    {
        return Err(fail("invalid cache manifest framing or byte allowance"));
    }
    let end = bytes.len() - 32;
    if blake3::hash(&bytes[..end]).as_bytes() != &bytes[end..] {
        return Err(fail("cache manifest digest mismatch"));
    }
    let (manifest, consumed): (Manifest, usize) = bincode::decode_from_slice(
        &bytes[MAGIC.len()..end],
        bincode::config::standard().with_limit::<MAX_FRAME>(),
    )
    .map_err(fail)?;
    if consumed != end - MAGIC.len() {
        return Err(fail("trailing cache manifest bytes"));
    }
    let mut statistics = validate(&manifest, expected, limits)?;
    statistics.manifest_bytes = bytes.len();
    Ok((manifest, statistics))
}

pub(super) fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, AppError> {
    let metadata = fs::symlink_metadata(path).map_err(fail)?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err(fail("cache path is not a bounded regular file"));
    }
    let file = File::open(path).map_err(fail)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(metadata.len() as usize)
        .map_err(fail)?;
    file.take((limit as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(fail)?;
    if bytes.len() > limit || bytes.len() as u64 != metadata.len() {
        return Err(fail("cache file changed or exceeded its read allowance"));
    }
    Ok(bytes)
}

pub(super) struct BoundedBytes(Vec<u8>, usize);
impl BoundedBytes {
    pub fn new(limit: usize) -> Self {
        Self(Vec::new(), limit)
    }
    pub fn finish(self) -> Vec<u8> {
        self.0
    }
}
impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let requested = self
            .0
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("cache index byte overflow"))?;
        if requested > self.1 {
            return Err(io::Error::other("cache index byte allowance exceeded"));
        }
        self.0.try_reserve(bytes.len()).map_err(io::Error::other)?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
