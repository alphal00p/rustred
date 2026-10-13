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
    /// Share only immutable directory metadata; no coefficient/route cloning.
    pub fn fork_reader(&self) -> Self {
        let mut statistics = self.statistics;
        statistics.chunks_read = 0;
        statistics.chunk_bytes_read = 0;
        statistics.largest_chunk_read = 0;
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
    pub fn lookup(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, AppError> {
        key.validate(self.manifest.binding.physical_arity)
            .map_err(fail)?;
        for run in self.manifest.runs.iter().rev() {
            let offset = run.partition_point(|chunk| chunk.last < *key);
            let Some(chunk) = run.get(offset).filter(|chunk| chunk.first <= *key) else {
                continue;
            };
            let bytes = index::read_bounded(
                &self.directory.join("chunks").join(chunk.file_name()),
                self.limits.chunk.binary.max_program_bytes,
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
            let decoded =
                ApplicationCacheChunk::inspect(&bytes, &self.manifest.binding, self.limits.chunk)
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
