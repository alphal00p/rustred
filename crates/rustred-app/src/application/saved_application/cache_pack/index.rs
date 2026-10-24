use std::{
    fs::{self, File},
    io::{self, Read, Write},
    mem::size_of,
    path::Path,
};

use rustred::persistence::application_cache::{ApplicationCacheBinding, ApplicationCacheKey};

use super::*;
use crate::AppError;

mod frame;
pub(super) mod key_filter;
mod legacy;

const MAGIC: &[u8; 16] = b"RR-CACHE-PACK-03";
pub(super) const MANIFEST: &str = "cache.rrindex";

#[cfg(test)]
pub(super) fn encode_legacy_for_test(manifest: &Manifest) -> Vec<u8> {
    legacy::encode_for_test(manifest)
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(super) struct Chunk {
    pub digest: [u8; 32],
    pub bytes: u64,
    pub decoded_digest: [u8; 32],
    pub decoded_bytes: u64,
    pub records: usize,
    pub first: ApplicationCacheKey,
    pub last: ApplicationCacheKey,
    /// Sorted fingerprints of every key, including repetitions on collision.
    /// None is retained only for legacy chunks. This trusted directory metadata
    /// can reject absence, but never establishes key equality or a cache hit.
    pub key_fingerprints: Option<Vec<u64>>,
}
impl Chunk {
    pub fn file_name(&self) -> String {
        format!(
            "{}.rrchunk.gz",
            blake3::Hash::from_bytes(self.digest).to_hex()
        )
    }
    /// A legacy descriptor may omit metadata for the very same native chunk.
    /// This only identifies duplicate files for copying, never lookup results.
    pub(super) fn compatible_payload(&self, other: &Self) -> bool {
        self.digest == other.digest
            && self.bytes == other.bytes
            && self.decoded_digest == other.decoded_digest
            && self.decoded_bytes == other.decoded_bytes
            && self.records == other.records
            && self.first == other.first
            && self.last == other.last
            && match (&self.key_fingerprints, &other.key_fingerprints) {
                (Some(left), Some(right)) => left == right,
                _ => true,
            }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
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
    expected.validate(limits.chunk).map_err(binary_error)?;
    if manifest.binding != *expected {
        return Err(fail("artifact, family, arity or cache layer mismatch"));
    }
    resource_checkpoint()?;
    if limits.diagnostics_enabled && manifest.runs.len() > limits.max_runs {
        return Err(AppError::limit("cache run directory exceeds allowance"));
    }
    let mut result = SavedApplicationCachePackStatistics::default();
    result.runs = manifest.runs.len();
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
        resource_checkpoint()?;
        if run.is_empty() {
            return Err(fail("empty immutable cache run"));
        }
        result.directory_owned_bytes = result
            .directory_owned_bytes
            .saturating_add(run.capacity().saturating_mul(size_of::<Chunk>()));
        let mut previous = None;
        for chunk in run {
            resource_checkpoint()?;
            chunk
                .first
                .validate(expected.physical_arity)
                .map_err(fail)?;
            chunk.last.validate(expected.physical_arity).map_err(fail)?;
            if chunk.first > chunk.last
                || previous.is_some_and(|key| key >= &chunk.first)
                || chunk.records == 0
                || (limits.chunk.diagnostics_enabled && chunk.records > limits.chunk.max_records)
            {
                return Err(fail("invalid, overlapping or unordered cache chunk range"));
            }
            previous = Some(&chunk.last);
            if let Some(fingerprints) = &chunk.key_fingerprints {
                if fingerprints.len() != chunk.records
                    || fingerprints.windows(2).any(|pair| pair[0] > pair[1])
                    || !key_filter::may_contain(
                        fingerprints,
                        key_filter::fingerprint(&chunk.first)?,
                    )
                    || !key_filter::may_contain(fingerprints, key_filter::fingerprint(&chunk.last)?)
                {
                    return Err(fail("invalid cache chunk key fingerprint directory"));
                }
                result.directory_owned_bytes = result
                    .directory_owned_bytes
                    .saturating_add(fingerprints.capacity().saturating_mul(size_of::<u64>()));
            }
            if chunk.bytes == 0
                || chunk.bytes > limits.max_encoded_chunk_bytes as u64
                || chunk.decoded_bytes == 0
                || chunk.decoded_bytes > limits.chunk.binary.max_program_bytes as u64
            {
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
    if (limits.diagnostics_enabled && result.chunks > limits.max_chunks)
        || result.referenced_chunk_bytes > limits.max_disk_bytes
        || result.directory_owned_bytes > limits.max_directory_owned_bytes
    {
        return Err(AppError::limit(
            "cache pack directory or referenced disk allowance exceeded",
        ));
    }
    resource_checkpoint()?;
    Ok(result)
}

pub(super) fn encode(
    manifest: &Manifest,
    limits: SavedApplicationCachePackLimits,
) -> Result<Vec<u8>, AppError> {
    validate(manifest, &manifest.binding, limits)?;
    let mut output = BoundedBytes::new(limits.max_manifest_bytes.saturating_sub(32));
    output.write_all(MAGIC).map_err(io_error)?;
    // Structural framing only; no algebra values are present in this index.
    bincode::encode_into_std_write(manifest, &mut output, bincode::config::standard())
        .map_err(encode_error)?;
    let mut bytes = output.finish();
    let digest = blake3::hash(&bytes);
    resource_allocation(32usize.saturating_sub(bytes.capacity() - bytes.len()))?;
    bytes.try_reserve_exact(32).map_err(fail)?;
    bytes.extend_from_slice(digest.as_bytes());
    if bytes.len() > limits.max_manifest_bytes {
        return Err(AppError::limit("cache manifest byte allowance exceeded"));
    }
    resource_checkpoint()?;
    Ok(bytes)
}

pub(super) fn decode(
    bytes: &[u8],
    expected: &ApplicationCacheBinding,
    limits: SavedApplicationCachePackLimits,
) -> Result<(Manifest, SavedApplicationCachePackStatistics), AppError> {
    resource_checkpoint()?;
    expected.validate(limits.chunk).map_err(binary_error)?;
    if bytes.len() > limits.max_manifest_bytes
        || bytes.len() < MAGIC.len() + 32
        || !(bytes.starts_with(MAGIC) || bytes.starts_with(legacy::MAGIC))
    {
        return Err(fail("invalid cache manifest framing or byte allowance"));
    }
    let end = bytes.len() - 32;
    if blake3::hash(&bytes[..end]).as_bytes() != &bytes[end..] {
        return Err(fail("cache manifest digest mismatch"));
    }
    let (manifest, consumed) =
        frame::decode(&bytes[MAGIC.len()..end], expected, bytes.starts_with(MAGIC))?;
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
    let mut file = File::open(path).map_err(fail)?;
    let length = usize::try_from(metadata.len()).map_err(fail)?;
    let mut bytes = Vec::new();
    resource_allocation(length)?;
    bytes.try_reserve_exact(length).map_err(fail)?;
    bytes.resize(length, 0);
    file.read_exact(&mut bytes).map_err(fail)?;
    if file.read(&mut [0u8; 1]).map_err(fail)? != 0 {
        return Err(fail("cache file changed or exceeded its read allowance"));
    }
    resource_checkpoint()?;
    Ok(bytes)
}

pub(super) struct BoundedBytes(Vec<u8>, usize, bool);
impl BoundedBytes {
    pub fn new(limit: usize) -> Self {
        Self(Vec::new(), limit, false)
    }
    pub fn finish(self) -> Vec<u8> {
        self.0
    }
    pub fn limit_exceeded(&self) -> bool {
        self.2
    }
    #[cfg(test)]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}
impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(requested) = self.0.len().checked_add(bytes.len()) else {
            self.2 = true;
            return Err(io::Error::other("cache frame byte overflow"));
        };
        if requested > self.1 {
            self.2 = true;
            return Err(io::Error::other("cache frame byte allowance exceeded"));
        }
        if requested > self.0.capacity() {
            // Amortize tiny bincode writes without asking Vec for capacity
            // beyond the admitted frame limit, even at a non-power-of-two cap.
            let capacity = requested
                .max(self.0.capacity().saturating_mul(2))
                .min(self.1);
            resource_allocation(capacity - self.0.capacity()).map_err(io::Error::other)?;
            self.0
                .try_reserve_exact(capacity - self.0.len())
                .map_err(io::Error::other)?;
        }
        resource_checkpoint().map_err(io::Error::other)?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
