//! Shared bounded staging for preparation and explicit live-cache export.
//! The bundle writer alone owns serialization, quota and atomic publication.
use rustred::{
    algebra::CoefficientContext,
    persistence::application_cache::{
        ApplicationCacheBinding, ApplicationCacheIoLimits, ApplicationCacheRecord,
    },
};

use crate::{AppError, SavedApplicationCacheBundleWriter};

#[derive(Clone, Copy)]
pub(super) struct RecordWeight {
    terms: usize,
    bytes: usize,
}

#[derive(Default)]
struct NamespaceBatch {
    records: Vec<ApplicationCacheRecord>,
    terms: usize,
    bytes: usize,
}

pub(super) struct CompletedCacheBatch {
    namespaces: Vec<NamespaceBatch>,
    count: usize,
    terms: usize,
    bytes: usize,
    durable_records: usize,
}
impl CompletedCacheBatch {
    pub fn new(namespaces: usize) -> Self {
        Self {
            namespaces: (0..namespaces).map(|_| NamespaceBatch::default()).collect(),
            count: 0,
            terms: 0,
            bytes: 0,
            durable_records: 0,
        }
    }
    pub fn durable_records(&self) -> usize {
        self.durable_records
    }
    pub fn buffered_records(&self) -> usize {
        self.count
    }
    pub fn weight(
        record: &ApplicationCacheRecord,
        limits: ApplicationCacheIoLimits,
    ) -> Result<RecordWeight, AppError> {
        let (terms, bytes) = record
            .coefficient_census()
            .map_err(|error| AppError::limit(error.to_string()))?;
        if terms > limits.max_decoded_coefficient_terms
            || bytes > limits.max_decoded_coefficient_bytes
        {
            return Err(AppError::limit(
                "completed root exceeds cache IO buffer allowance",
            ));
        }
        Ok(RecordWeight { terms, bytes })
    }
    pub fn needs_flush(&self, weight: RecordWeight, limits: ApplicationCacheIoLimits) -> bool {
        self.count >= limits.max_records
            || self
                .terms
                .checked_add(weight.terms)
                .is_none_or(|n| n > limits.max_decoded_coefficient_terms)
            || self
                .bytes
                .checked_add(weight.bytes)
                .is_none_or(|n| n > limits.max_decoded_coefficient_bytes)
    }
    pub fn push(
        &mut self,
        namespace: usize,
        record: ApplicationCacheRecord,
        weight: RecordWeight,
        limits: ApplicationCacheIoLimits,
    ) -> Result<(), AppError> {
        if self.needs_flush(weight, limits) {
            return Err(AppError::limit(
                "cache IO batch must be flushed before admission",
            ));
        }
        let batch = self
            .namespaces
            .get_mut(namespace)
            .ok_or_else(|| AppError::internal_invariant("unknown cache batch namespace"))?;
        batch
            .records
            .try_reserve(1)
            .map_err(|_| AppError::limit("cannot allocate cache batch records"))?;
        batch.records.push(record);
        batch.terms += weight.terms;
        batch.bytes += weight.bytes;
        self.count += 1;
        self.terms += weight.terms;
        self.bytes += weight.bytes;
        Ok(())
    }
    /// Counts advance only after a successful child checkpoint, not merely
    /// after accepting a callback record or staging an encoded chunk. On error,
    /// durable_records retains the exact successfully published child prefix.
    pub fn flush<'a>(
        &mut self,
        bindings: &[ApplicationCacheBinding],
        writer: &mut SavedApplicationCacheBundleWriter,
        mut context: impl FnMut(usize) -> Result<&'a CoefficientContext, AppError>,
    ) -> Result<(), AppError> {
        if bindings.len() != self.namespaces.len() {
            return Err(AppError::internal_invariant(
                "cache batch binding count differs",
            ));
        }
        for (namespace, batch) in self.namespaces.iter_mut().enumerate() {
            if batch.records.is_empty() {
                continue;
            }
            let durable = self
                .durable_records
                .checked_add(batch.records.len())
                .ok_or_else(|| AppError::limit("cache batch durable count overflow"))?;
            batch
                .records
                .sort_by(|left, right| left.key.cmp(&right.key));
            writer.append_completed(&bindings[namespace], &batch.records, context(namespace)?)?;
            self.durable_records = durable;
            self.count -= batch.records.len();
            self.terms -= batch.terms;
            self.bytes -= batch.bytes;
            *batch = NamespaceBatch::default();
        }
        Ok(())
    }
}
