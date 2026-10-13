use rustred::algebra::CoefficientContext;
use rustred::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheChunk, ApplicationCacheKey, ApplicationCacheRecord,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use super::*;
use crate::AppError;

/// Immutable on-disk snapshot plus local I/O statistics. Only range descriptors
/// are retained; every lookup drops its encoded chunk/native decode table.
pub struct SavedApplicationCachePack {
    directory: PathBuf,
    manifest: Arc<index::Manifest>,
    limits: SavedApplicationCachePackLimits,
    statistics: SavedApplicationCachePackStatistics,
}
impl SavedApplicationCachePack {
    /// Explicit trusted-native import. Binding equality and content hashes are
    /// necessary integrity checks, not a proof of supplied reduction identities.
    pub fn open_generated(
        directory: &Path,
        expected: &ApplicationCacheBinding,
        limits: SavedApplicationCachePackLimits,
    ) -> Result<Self, AppError> {
        if limits.max_manifest_bytes > index::MAX_FRAME {
            return Err(fail("manifest allowance exceeds bounded frame"));
        }
        let bytes =
            index::read_bounded(&directory.join(index::MANIFEST), limits.max_manifest_bytes)?;
        let (manifest, statistics) = index::decode(&bytes, expected, limits)?;
        Ok(Self {
            directory: directory.to_path_buf(),
            manifest: Arc::new(manifest),
            limits,
            statistics,
        })
    }
    pub fn binding(&self) -> &ApplicationCacheBinding {
        &self.manifest.binding
    }
    pub fn generation(&self) -> u64 {
        self.manifest.generation
    }
    pub fn statistics(&self) -> SavedApplicationCachePackStatistics {
        self.statistics
    }
    pub(super) fn captured_snapshot(&self) -> (&Path, &index::Manifest) {
        (&self.directory, &self.manifest)
    }
    /// Authenticate a newer committed directory without decoding coefficients.
    /// Intended for explicit durable-checkpoint boundaries, not each lookup.
    /// On failure the previous snapshot remains usable. Successful refreshes
    /// replace only directory metadata/census: cumulative payload-I/O, lookup
    /// charges and hit/miss counters are preserved, not reset. Index reads are
    /// separately bounded by `max_manifest_bytes`, as at initial open.
    pub fn refresh(&mut self) -> Result<(), AppError> {
        let bytes = index::read_bounded(
            &self.directory.join(index::MANIFEST),
            self.limits.max_manifest_bytes,
        )?;
        let (manifest, directory) = index::decode(&bytes, self.binding(), self.limits)?;
        if manifest.bundle_managed != self.manifest.bundle_managed
            || manifest.generation < self.manifest.generation
            || manifest.generation == self.manifest.generation && manifest != *self.manifest
        {
            return Err(fail(
                "cache refresh changed ownership or rewound/mutated its generation",
            ));
        }
        let mut statistics = self.statistics;
        statistics.runs = directory.runs;
        statistics.chunks = directory.chunks;
        statistics.records = directory.records;
        statistics.referenced_chunk_bytes = directory.referenced_chunk_bytes;
        statistics.manifest_bytes = directory.manifest_bytes;
        statistics.directory_owned_bytes = directory.directory_owned_bytes;
        self.manifest = Arc::new(manifest);
        self.statistics = statistics;
        Ok(())
    }
    /// Share only immutable directory metadata; no coefficient/route cloning.
    pub fn fork_reader(&self) -> Self {
        let mut statistics = self.statistics;
        statistics.chunks_read = 0;
        statistics.chunk_bytes_read = 0;
        statistics.largest_chunk_read = 0;
        statistics.decoded_chunk_bytes = 0;
        statistics.largest_decoded_chunk = 0;
        statistics.lookup_encoded_bytes_charged = 0;
        statistics.lookup_decoded_bytes_charged = 0;
        statistics.lookup_limit_refusals = 0;
        statistics.runs_probed = 0;
        statistics.hits = 0;
        statistics.misses = 0;
        Self {
            directory: self.directory.clone(),
            manifest: self.manifest.clone(),
            limits: self.limits,
            statistics,
        }
    }
    /// Exact key lookup, newest immutable run first. A matching range is only
    /// a candidate: complete key equality in the native chunk is authoritative.
    /// Exceeding cumulative lookup work is a typed refusal, never a cache miss.
    /// This low-level reader does not silently fall back to cold reduction;
    /// callers must choose such a policy explicitly and must not hide invalid
    /// bindings, corrupted payloads or native validation errors as misses.
    pub fn lookup(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, AppError> {
        key.validate(self.manifest.binding.physical_arity)
            .map_err(fail)?;
        let mut work = LookupWork::default();
        for run in self.manifest.runs.iter().rev() {
            self.statistics.runs_probed = self.statistics.runs_probed.saturating_add(1);
            let offset = run.partition_point(|chunk| chunk.last < *key);
            let Some(chunk) = run.get(offset).filter(|chunk| chunk.first <= *key) else {
                continue;
            };
            if let Err(error) = work.charge(chunk, self.limits) {
                self.statistics.lookup_limit_refusals =
                    self.statistics.lookup_limit_refusals.saturating_add(1);
                return Err(error);
            }
            self.statistics.lookup_encoded_bytes_charged = self
                .statistics
                .lookup_encoded_bytes_charged
                .saturating_add(chunk.bytes.saturating_add(1));
            self.statistics.lookup_decoded_bytes_charged = self
                .statistics
                .lookup_decoded_bytes_charged
                .saturating_add(chunk.decoded_bytes.saturating_add(1));
            let bytes = index::read_bounded(
                &self.directory.join("chunks").join(chunk.file_name()),
                usize::try_from(chunk.bytes).map_err(fail)?,
            )?;
            self.statistics.chunks_read = self.statistics.chunks_read.saturating_add(1);
            self.statistics.chunk_bytes_read = self
                .statistics
                .chunk_bytes_read
                .saturating_add(bytes.len() as u64);
            self.statistics.largest_chunk_read =
                self.statistics.largest_chunk_read.max(bytes.len());
            if bytes.len() as u64 != chunk.bytes || *blake3::hash(&bytes).as_bytes() != chunk.digest
            {
                return Err(fail("cache chunk digest or length mismatch"));
            }
            let payload = compression::decode(
                &bytes,
                chunk.decoded_bytes,
                &chunk.decoded_digest,
                self.limits,
            )?;
            self.statistics.decoded_chunk_bytes = self
                .statistics
                .decoded_chunk_bytes
                .saturating_add(payload.len() as u64);
            self.statistics.largest_decoded_chunk =
                self.statistics.largest_decoded_chunk.max(payload.len());
            drop(bytes);
            let decoded =
                ApplicationCacheChunk::inspect(&payload, &self.manifest.binding, self.limits.chunk)
                    .map_err(fail)?;
            if decoded.len() != chunk.records
                || decoded.first_key() != Some(&chunk.first)
                || decoded.last_key() != Some(&chunk.last)
            {
                return Err(fail("cache chunk index differs from native framing"));
            }
            if let Some(record) = decoded.decode_record(key, context).map_err(fail)? {
                self.statistics.hits = self.statistics.hits.saturating_add(1);
                return Ok(Some(record));
            }
        }
        self.statistics.misses = self.statistics.misses.saturating_add(1);
        Ok(None)
    }
}

#[derive(Default)]
struct LookupWork {
    chunks: usize,
    encoded: u64,
    decoded: u64,
}
impl LookupWork {
    fn charge(
        &mut self,
        chunk: &index::Chunk,
        limits: SavedApplicationCachePackLimits,
    ) -> Result<(), AppError> {
        let overflow = || AppError::limit("cache lookup work counter overflow");
        let chunks = self.chunks.checked_add(1).ok_or_else(overflow)?;
        let encoded = self
            .encoded
            .checked_add(chunk.bytes)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(overflow)?;
        let decoded = self
            .decoded
            .checked_add(chunk.decoded_bytes)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(overflow)?;
        if chunks > limits.max_lookup_chunks
            || encoded > limits.max_lookup_encoded_bytes
            || decoded > limits.max_lookup_decoded_bytes
        {
            return Err(AppError::limit(
                "cache cumulative lookup allowance exceeded",
            ));
        }
        self.chunks = chunks;
        self.encoded = encoded;
        self.decoded = decoded;
        Ok(())
    }
}
