//! Bounded preparation-only sink. Exact intermediate identities and their
//! guard provenance are supplied by the native program, never reconstructed
//! from the requested root's guards or from a guardless coefficient memo.
use std::sync::atomic::{AtomicBool, Ordering};
use symbolica::utils::resources::checkpoint;

use rustred::{
    algebra::CoefficientContext,
    persistence::application_cache::{
        ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
    },
    reduction::{GuardedCacheBackingError, GuardedCaptureStatistics, GuardedIntermediateCapture},
};

use super::{
    CachePreparationReport, CachePreparationResolvedOptions, batch::CompletedCacheBatch,
    binary_error, resource_error, session::backing_error,
};
use crate::{
    AppError, AppErrorKind, SavedApplicationCacheBundleWriter, SavedApplicationCacheLookupOutcome,
    SavedApplicationCachePack,
};

pub(super) struct PreparationCapture<'a> {
    pub namespace: usize,
    pub bindings: &'a [ApplicationCacheBinding],
    pub contexts: &'a [CoefficientContext],
    pub options: &'a CachePreparationResolvedOptions,
    pub cancellation: &'a AtomicBool,
    pub pending: &'a mut CompletedCacheBatch,
    pub readers: &'a mut [SavedApplicationCachePack],
    pub writer: &'a mut SavedApplicationCacheBundleWriter,
    pub report: &'a mut CachePreparationReport,
    pub failure: Option<AppError>,
    pub storage_failed: bool,
    pub failure_is_storage: bool,
    pub published: bool,
}

impl PreparationCapture<'_> {
    fn fail(&mut self, error: AppError) -> GuardedCacheBackingError {
        self.failure = Some(error.clone());
        backing_error(error)
    }
    fn flush(&mut self) -> Result<(), AppError> {
        let before = self.pending.durable_records();
        let selected_before = self.pending.durable_selected_roots();
        let result = self.pending.flush(self.bindings, self.writer, |namespace| {
            self.contexts
                .get(namespace)
                .ok_or_else(|| AppError::internal_invariant("capture namespace context is absent"))
        });
        // Each child publication is atomic, not the entire multi-namespace
        // flush. Account its successful prefix even if a later child fails.
        let published = self.pending.durable_records() - before;
        let selected = self.pending.durable_selected_roots() - selected_before;
        self.report.completed_roots += selected;
        self.report.newly_stored_roots += selected;
        self.report.newly_stored_intermediates += published - selected;
        self.published |= published != 0;
        result?;
        if published != 0 {
            for reader in self.readers.iter_mut() {
                reader.refresh()?;
            }
        }
        Ok(())
    }
}

impl GuardedIntermediateCapture for PreparationCapture<'_> {
    fn report_statistics(&mut self, statistics: GuardedCaptureStatistics) {
        self.report.capture_offered_records = self
            .report
            .capture_offered_records
            .saturating_add(statistics.captured_records);
        self.report.capture_native_skipped_records = self
            .report
            .capture_native_skipped_records
            .saturating_add(statistics.skipped_records);
        self.report.capture_metadata_exhausted_roots = self
            .report
            .capture_metadata_exhausted_roots
            .saturating_add(usize::from(statistics.metadata_exhausted));
        self.report.capture_metadata_peak_bytes = self
            .report
            .capture_metadata_peak_bytes
            .max(statistics.peak_metadata_bytes);
    }
    fn metadata_limit_bytes(&self) -> Option<usize> {
        Some(self.options.storage_limits.max_directory_owned_bytes)
    }
    fn wants(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<bool, GuardedCacheBackingError> {
        checkpoint().map_err(|error| self.fail(resource_error(error)))?;
        if self.cancellation.load(Ordering::Relaxed) {
            return Err(self.fail(AppError::new(
                AppErrorKind::Cancelled,
                "cache preparation cancelled during intermediate capture",
            )));
        }
        if self.pending.get(self.namespace, key).is_some() {
            return Ok(false);
        }
        match self.readers[self.namespace].lookup_outcome(key, context) {
            Ok(SavedApplicationCacheLookupOutcome::Hit(_)) => Ok(false),
            Ok(SavedApplicationCacheLookupOutcome::Absent) => Ok(true),
            // An optional write may be declined when presence cannot be
            // resolved within the configured lookup work allowance. This is
            // not authority to reuse any unknown record or to skip reduction.
            Ok(SavedApplicationCacheLookupOutcome::WorkBudgetExhausted) => {
                self.report.capture_lookup_budget_skips =
                    self.report.capture_lookup_budget_skips.saturating_add(1);
                Ok(false)
            }
            Err(error) => Err(self.fail(error)),
        }
    }
    fn store_completed(
        &mut self,
        record: &ApplicationCacheRecord,
        _: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        let limits = self.options.storage_limits.chunk;
        let result = (|| {
            let weight = match CompletedCacheBatch::weight(record, limits) {
                Ok(weight) => weight,
                Err(error)
                    if matches!(
                        error.kind(),
                        AppErrorKind::Limit | AppErrorKind::OutputLimit
                    ) && error.resource_limit_error().is_none() =>
                {
                    self.report.capture_oversized_records =
                        self.report.capture_oversized_records.saturating_add(1);
                    return Ok(());
                }
                Err(error) => return Err(error),
            };
            if self.pending.buffered_records() >= self.options.batch_records
                || self.pending.needs_flush(weight, limits)
            {
                if let Err(error) = self.flush() {
                    self.storage_failed = true;
                    return Err(error);
                }
            }
            let owned = record
                .try_clone_with_resources()
                .map_err(|error| binary_error(error, AppErrorKind::Limit))?;
            self.pending.push(self.namespace, owned, weight, limits)
        })();
        result.map_err(|error| {
            self.failure_is_storage = true;
            self.fail(error)
        })
    }
}
