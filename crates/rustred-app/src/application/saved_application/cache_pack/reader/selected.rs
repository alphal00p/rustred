//! Selected completed records use the same immutable chunks and exact precedence.
use std::{collections::BTreeMap, mem::size_of};

use super::*;

impl SavedApplicationCachePack {
    /// Visit only requested exact keys in this captured snapshot, once per key.
    /// Duplicates are ignored; newer runs win, including empty zero records.
    /// Output follows deterministic run/chunk order rather than global key order.
    /// Missing keys are an error, and earlier callbacks are not rolled back.
    ///
    /// Candidate chunks are selected by exact key ranges and negative filters.
    /// Each needed chunk is authenticated and imported once; only requested
    /// coefficients are decoded. Key identity includes its operational stage
    /// and Apply owner. Snapshot metadata is never refreshed, lookup counters
    /// are unchanged, and per-lookup diagnostic work grants do not apply.
    ///
    /// The requested-key index shares the directory-memory grant. Emitted
    /// coefficient grants and callback ownership match `visit_completed_records`.
    /// The caller must validate records against its authenticated application;
    /// transport and key validation alone do not establish reduction identities.
    pub fn visit_selected_completed_records(
        &mut self,
        keys: &[ApplicationCacheKey],
        context: &CoefficientContext,
        mut visit: impl FnMut(ApplicationCacheRecord) -> Result<(), AppError>,
    ) -> Result<usize, AppError> {
        let mut pending = BTreeMap::new();
        let mut index_bytes = self.statistics.directory_owned_bytes;
        for key in keys {
            resource_checkpoint()?;
            key.validate(self.manifest.binding.physical_arity)
                .map_err(fail)?;
            if pending.contains_key(key) {
                continue;
            }
            let bytes = size_of::<ApplicationCacheKey>()
                .checked_add(key.owned_heap_bytes())
                .and_then(|n| n.checked_add(128))
                .ok_or_else(|| AppError::limit("selected cache key census overflow"))?;
            index_bytes = index_bytes
                .checked_add(bytes)
                .ok_or_else(|| AppError::limit("selected cache index census overflow"))?;
            if index_bytes > self.limits.max_directory_owned_bytes {
                return Err(AppError::limit(
                    "selected cache exact-key index exceeds allowance",
                ));
            }
            resource_allocation(bytes)?;
            pending.insert(key.clone(), index::key_filter::fingerprint(key)?);
        }
        let requested = pending.len();
        let mut visited_chunks = BTreeMap::<[u8; 32], &index::Chunk>::new();
        let mut coefficient_terms = 0usize;
        let mut coefficient_bytes = 0usize;
        for run in self.manifest.runs.iter().rev() {
            for chunk in run {
                resource_checkpoint()?;
                if pending.is_empty() {
                    return Ok(requested);
                }
                let needed = pending
                    .range(&chunk.first..=&chunk.last)
                    .any(|(_, fingerprint)| {
                        chunk.key_fingerprints.as_ref().is_none_or(|fingerprints| {
                            index::key_filter::may_contain(fingerprints, *fingerprint)
                        })
                    });
                if !needed {
                    continue;
                }
                if let Some(previous) = visited_chunks.get(&chunk.digest) {
                    if !previous.compatible_payload(chunk) {
                        return Err(fail("conflicting selected cache chunk descriptors"));
                    }
                    continue;
                }
                let chunk_index_bytes = size_of::<([u8; 32], &index::Chunk)>()
                    .checked_add(128)
                    .ok_or_else(|| AppError::limit("selected cache chunk census overflow"))?;
                index_bytes = index_bytes
                    .checked_add(chunk_index_bytes)
                    .ok_or_else(|| AppError::limit("selected cache chunk index census overflow"))?;
                if index_bytes > self.limits.max_directory_owned_bytes {
                    return Err(AppError::limit(
                        "selected cache chunk index exceeds allowance",
                    ));
                }
                resource_allocation(chunk_index_bytes)?;
                let decoded = Self::read_completed_chunk(
                    &self.directory,
                    &self.manifest.binding,
                    chunk,
                    self.limits,
                    &mut self.statistics,
                )?;
                visited_chunks.insert(chunk.digest, chunk);
                for key in decoded.keys() {
                    resource_checkpoint()?;
                    if !pending.contains_key(key) {
                        continue;
                    }
                    let record = decoded
                        .decode_record(key, context)
                        .map_err(binary_error)?
                        .ok_or_else(|| fail("selected cache record vanished during scan"))?;
                    let (terms, bytes) = record.coefficient_census().map_err(binary_error)?;
                    coefficient_terms = coefficient_terms.checked_add(terms).ok_or_else(|| {
                        AppError::limit("selected cache coefficient term census overflow")
                    })?;
                    coefficient_bytes = coefficient_bytes.checked_add(bytes).ok_or_else(|| {
                        AppError::limit("selected cache coefficient byte census overflow")
                    })?;
                    if (self.limits.chunk.diagnostics_enabled
                        && coefficient_terms > self.limits.chunk.max_decoded_coefficient_terms)
                        || coefficient_bytes > self.limits.chunk.max_decoded_coefficient_bytes
                    {
                        return Err(AppError::limit(
                            "selected cache emitted coefficient allowance exceeded",
                        ));
                    }
                    pending.remove(key);
                    visit(record)?;
                }
            }
        }
        resource_checkpoint()?;
        if let Some((missing, _)) = pending.first_key_value() {
            return Err(AppError::input(format!(
                "{} selected cache records are absent; first missing key: {missing:?}",
                pending.len(),
            )));
        }
        Ok(requested)
    }
}
