//! Explicit live-root export through the existing native bounded bundle writer.
use std::path::Path;

use rustred::{
    algebra::CoefficientContext,
    persistence::application_cache::{
        ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
    },
    reduction::{
        GuardedCacheBacking, GuardedCacheBackingError, GuardedCacheBackingErrorKind,
        GuardedRootExportStatistics,
    },
};
use serde_json::{Value, json};

use super::{
    CachePreparationCapabilities, CachePreparationOptions,
    batch::CompletedCacheBatch,
    closed::{ClosedApplicationCache, backing_error, guarded_error},
};
use crate::{AppError, SavedApplicationCacheBundleWriter, SavedApplicationCachePackLimits};

/// Explicit storage policy for a resident-root snapshot. Resolving this policy
/// does not modify a live cache's RAM allowance, eviction policy or work limits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CacheSaveOptions {
    pub disk_bytes: Option<u64>,
    /// Append bounded immutable runs to an existing exactly bound output.
    /// Repeated snapshots may retain shadowed records and consume more quota.
    pub resume: bool,
    storage_limits: SavedApplicationCachePackLimits,
}
impl CacheSaveOptions {
    pub fn with_storage_limits(mut self, limits: SavedApplicationCachePackLimits) -> Self {
        self.disk_bytes = Some(limits.max_disk_bytes);
        self.storage_limits = limits;
        self
    }

    pub fn resolve(
        &self,
        loop_count: usize,
        capabilities: CachePreparationCapabilities,
    ) -> Result<SavedApplicationCachePackLimits, AppError> {
        let mut limits = self.storage_limits;
        limits.max_disk_bytes =
            CachePreparationOptions::resolve_disk_bytes(loop_count, self.disk_bytes, capabilities)?;
        if limits.chunk.max_records == 0 {
            return Err(AppError::input(
                "cache export batch allowance must be positive",
            ));
        }
        Ok(limits)
    }
}

/// Native callback acceptance is distinct from successful atomic publication.
/// Incomplete saves retain their durable prefix and never clear the RAM cache.
#[derive(Clone, Debug)]
pub struct CacheSaveReport {
    pub complete: bool,
    pub accepted: GuardedRootExportStatistics,
    pub durable_records: usize,
    pub charged_disk_bytes: u64,
    /// An attached lazy backing is not traversed or copied by this operation.
    /// Only records already resident in the native RAM cache are exported.
    pub backing_attached: bool,
    pub storage_limits: SavedApplicationCachePackLimits,
    pub failure: Option<AppError>,
    pub checkpoint_failure: Option<AppError>,
}
impl CacheSaveReport {
    pub fn to_document(&self) -> Value {
        json!({"complete":self.complete, "accepted_records":self.accepted.exported_records,
            "accepted_coefficient_terms":self.accepted.exported_coefficient_terms,
            "accepted_coefficient_bytes":self.accepted.exported_coefficient_bytes,
            "durable_records":self.durable_records, "charged_disk_bytes":self.charged_disk_bytes,
            "backing_attached":self.backing_attached, "unloaded_backing_records_copied":false,
            "storage_limits":format!("{:?}", self.storage_limits),
            "failure":self.failure.as_ref().map(|e|json!({"kind":e.kind().as_str(),"detail":e.message()})),
            "checkpoint_failure":self.checkpoint_failure.as_ref().map(|e|json!({"kind":e.kind().as_str(),"detail":e.message()})),
            "scope":"completed guarded RAM roots only; no in-flight state or automatic copy of unloaded backing records"})
    }
}

struct ExportSink {
    binding: ApplicationCacheBinding,
    writer: SavedApplicationCacheBundleWriter,
    batch: CompletedCacheBatch,
    limits: SavedApplicationCachePackLimits,
}
impl ExportSink {
    fn flush(&mut self, context: &CoefficientContext) -> Result<(), AppError> {
        self.batch.flush(
            std::slice::from_ref(&self.binding),
            &mut self.writer,
            |_| Ok(context),
        )
    }
}
impl GuardedCacheBacking for ExportSink {
    fn binding(&self) -> &ApplicationCacheBinding {
        &self.binding
    }
    fn lookup(
        &mut self,
        _: &ApplicationCacheKey,
        _: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, GuardedCacheBackingError> {
        Err(GuardedCacheBackingError {
            kind: GuardedCacheBackingErrorKind::Invalid,
            message: "live-export sink is write-only".into(),
        })
    }
    fn store_completed(
        &mut self,
        record: &ApplicationCacheRecord,
        context: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        let weight =
            CompletedCacheBatch::weight(record, self.limits.chunk).map_err(backing_error)?;
        if self.batch.needs_flush(weight, self.limits.chunk) {
            self.flush(context).map_err(backing_error)?;
        }
        // At most the configured bounded batch is cloned, never the whole
        // live cache. Coefficient census and native codec limits remain active.
        self.batch
            .push(0, record.clone(), weight, self.limits.chunk)
            .map_err(backing_error)
    }
}

impl ClosedApplicationCache<'_> {
    /// Snapshot completed resident roots without changing RAM residency or
    /// eviction scores. This is not a portable copy of an attached base pack:
    /// records which have never been loaded, or were evicted, are excluded.
    pub fn save_completed(
        &self,
        output: &Path,
        limits: SavedApplicationCachePackLimits,
        resume: bool,
    ) -> Result<CacheSaveReport, AppError> {
        if limits.chunk.max_records == 0 || limits.max_disk_bytes == 0 {
            return Err(AppError::input(
                "live cache export needs positive batch and disk allowances",
            ));
        }
        let bindings = std::slice::from_ref(self.program().binding());
        let writer = if output
            .try_exists()
            .map_err(|e| AppError::input(e.to_string()))?
        {
            if !resume {
                return Err(AppError::input(
                    "cache export already exists and resume is disabled",
                ));
            }
            SavedApplicationCacheBundleWriter::resume(output, bindings, limits)?
        } else {
            SavedApplicationCacheBundleWriter::create(output, bindings, limits)?
        };
        let mut sink = ExportSink {
            binding: self.program().binding().clone(),
            writer,
            batch: CompletedCacheBatch::new(1),
            limits,
        };
        let (accepted, failure) = match self.export_to(&mut sink) {
            Ok(statistics) => (statistics, None),
            Err(error) => (error.accepted_prefix, Some(guarded_error(error.source))),
        };
        // A failed admission may leave earlier accepted records buffered.
        // Preserve them when the writer's publication state is still certain.
        let checkpoint_failure = if sink.writer.is_poisoned() {
            None
        } else {
            sink.flush(self.program().context()).err()
        };
        Ok(CacheSaveReport {
            complete: failure.is_none()
                && checkpoint_failure.is_none()
                && sink.batch.buffered_records() == 0,
            accepted,
            durable_records: sink.batch.durable_records(),
            charged_disk_bytes: sink.writer.charged_disk_bytes()?,
            backing_attached: self.io_statistics().is_some(),
            storage_limits: limits,
            failure,
            checkpoint_failure,
        })
    }
}
