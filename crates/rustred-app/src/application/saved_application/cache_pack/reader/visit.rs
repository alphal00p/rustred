//! Explicit full-snapshot scan for compacting completed checkpoint records.
use std::{collections::BTreeSet, mem::size_of, sync::Arc};

use rustred::persistence::application_cache::OwnedApplicationCacheChunk;

use super::*;

impl SavedApplicationCachePack {
    pub(super) fn read_completed_chunk(
        directory: &Path,
        binding: &ApplicationCacheBinding,
        chunk: &index::Chunk,
        limits: SavedApplicationCachePackLimits,
        statistics: &mut SavedApplicationCachePackStatistics,
    ) -> Result<OwnedApplicationCacheChunk, AppError> {
        let encoded = index::read_bounded(
            &directory.join("chunks").join(chunk.file_name()),
            usize::try_from(chunk.bytes).map_err(fail)?,
        )?;
        statistics.chunks_read = statistics.chunks_read.saturating_add(1);
        statistics.chunk_bytes_read = statistics
            .chunk_bytes_read
            .saturating_add(encoded.len() as u64);
        statistics.largest_chunk_read = statistics.largest_chunk_read.max(encoded.len());
        if encoded.len() as u64 != chunk.bytes || blake3::hash(&encoded).as_bytes() != &chunk.digest
        {
            return Err(fail("cache chunk digest or length mismatch during scan"));
        }
        let payload =
            compression::decode(&encoded, chunk.decoded_bytes, &chunk.decoded_digest, limits)?;
        drop(encoded);
        statistics.decoded_chunk_bytes = statistics
            .decoded_chunk_bytes
            .saturating_add(payload.len() as u64);
        statistics.largest_decoded_chunk = statistics.largest_decoded_chunk.max(payload.len());
        resource_allocation(payload.len())?;
        let decoded =
            OwnedApplicationCacheChunk::inspect(Arc::from(payload), binding, limits.chunk)
                .map_err(binary_error)?;
        if decoded.len() != chunk.records
            || decoded.keys().next() != Some(&chunk.first)
            || decoded.keys().last() != Some(&chunk.last)
        {
            return Err(fail(
                "cache chunk index differs from native framing during scan",
            ));
        }
        if let Some(expected) = &chunk.key_fingerprints
            && index::key_filter::collect(decoded.keys())? != *expected
        {
            return Err(fail(
                "cache key fingerprints differ from native framing during scan",
            ));
        }
        Ok(decoded)
    }

    /// Visit the latest completed record for each exact key in this captured
    /// snapshot. Newer immutable runs win. Output order is deterministic scan
    /// order, not global key order; a single-chunk exporter must sort its output.
    ///
    /// Each chunk is authenticated/decompressed once, with one lazy native table
    /// shared by its records. The exact visited-key set and existing directory
    /// share `max_directory_owned_bytes`. Aggregate emitted coefficient terms
    /// and clone-owned bytes obey `chunk.max_decoded_coefficient_*`, so a caller
    /// may collect the output under those grants. Native scratch and allocator
    /// overhead retain their existing contracts. Callback retention is caller
    /// owned, and a callback error stops the scan immediately.
    ///
    /// This is explicit full-snapshot work under the pack/chunk bounds, not a
    /// lookup; per-lookup I/O budgets and hit/miss counters do not apply. Captured
    /// directory metadata is never refreshed, and uncommitted files are ignored.
    pub fn visit_completed_records(
        &mut self,
        context: &CoefficientContext,
        mut visit: impl FnMut(ApplicationCacheRecord) -> Result<(), AppError>,
    ) -> Result<usize, AppError> {
        let mut seen = BTreeSet::<ApplicationCacheKey>::new();
        let mut index_bytes = self.statistics.directory_owned_bytes;
        let mut coefficient_terms = 0usize;
        let mut coefficient_bytes = 0usize;
        resource_checkpoint()?;
        for run in self.manifest.runs.iter().rev() {
            for chunk in run {
                resource_checkpoint()?;
                let decoded = Self::read_completed_chunk(
                    &self.directory,
                    &self.manifest.binding,
                    chunk,
                    self.limits,
                    &mut self.statistics,
                )?;
                for key in decoded.keys() {
                    resource_checkpoint()?;
                    if seen.contains(key) {
                        continue;
                    }
                    // Conservative per-key tree metadata allowance, separate
                    // from the exact key vectors. Like the pack census this is
                    // an admission measure, not an allocator RSS guarantee.
                    let next_bytes = index_bytes
                        .checked_add(size_of::<ApplicationCacheKey>())
                        .and_then(|n| n.checked_add(key.owned_heap_bytes()))
                        .and_then(|n| n.checked_add(128))
                        .ok_or_else(|| AppError::limit("cache scan index census overflow"))?;
                    if next_bytes > self.limits.max_directory_owned_bytes {
                        return Err(AppError::limit(
                            "cache scan exact-key index exceeds allowance",
                        ));
                    }
                    let record = decoded
                        .decode_record(key, context)
                        .map_err(binary_error)?
                        .ok_or_else(|| fail("indexed cache record vanished during scan"))?;
                    let (terms, bytes) = record.coefficient_census().map_err(binary_error)?;
                    coefficient_terms = coefficient_terms.checked_add(terms).ok_or_else(|| {
                        AppError::limit("cache scan coefficient term census overflow")
                    })?;
                    coefficient_bytes = coefficient_bytes.checked_add(bytes).ok_or_else(|| {
                        AppError::limit("cache scan coefficient byte census overflow")
                    })?;
                    if (self.limits.chunk.diagnostics_enabled
                        && coefficient_terms > self.limits.chunk.max_decoded_coefficient_terms)
                        || coefficient_bytes > self.limits.chunk.max_decoded_coefficient_bytes
                    {
                        return Err(AppError::limit(
                            "cache scan emitted coefficient allowance exceeded",
                        ));
                    }
                    resource_allocation(next_bytes - index_bytes)?;
                    seen.insert(key.clone());
                    index_bytes = next_bytes;
                    visit(record)?;
                }
            }
        }
        resource_checkpoint()?;
        Ok(seen.len())
    }
}
