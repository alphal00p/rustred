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
    binary_error,
    program::NativeCacheProgram,
    session::{
        NativeApplicationCache, NativeCachePreparationBackend, backing_error, guarded_error,
    },
};
use crate::{
    AppError, AppErrorKind, SavedApplicationCacheBundleWriter, SavedApplicationCacheCopyStatistics,
    SavedApplicationCachePackLimits,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheSaveMode {
    ResidentRoots,
    /// Copy all captured immutable bases without coefficient decoding, then
    /// overlay resident completed roots. Requires a new destination.
    FullSnapshot,
}

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
        if limits.chunk.diagnostics_enabled && limits.chunk.max_records == 0 {
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
    /// Incomplete native routed records omitted from a resident export.
    pub skipped_partial_records: usize,
    pub charged_disk_bytes: u64,
    /// Whether any session has a lazy backing. ResidentRoots excludes its
    /// unloaded records; FullSnapshot copies captured bases before the overlay.
    pub backing_attached: bool,
    pub mode: CacheSaveMode,
    /// Base publication is distinct from resident overlay completion.
    pub base_copy: Option<SavedApplicationCacheCopyStatistics>,
    pub storage_limits: SavedApplicationCachePackLimits,
    pub failure: Option<AppError>,
    pub checkpoint_failure: Option<AppError>,
}
impl CacheSaveReport {
    pub fn to_document(&self) -> Value {
        json!({"complete":self.complete, "accepted_records":self.accepted.exported_records,
            "accepted_coefficient_terms":self.accepted.exported_coefficient_terms,
            "accepted_coefficient_bytes":self.accepted.exported_coefficient_bytes,
            "durable_records":self.durable_records, "skipped_partial_records":self.skipped_partial_records, "charged_disk_bytes":self.charged_disk_bytes,
            "backing_attached":self.backing_attached, "base_copy_phase_completed":self.base_copy.is_some(),
            "mode":format!("{:?}",self.mode), "base_copy":self.base_copy.map(|s|json!({
                "namespaces":s.namespaces, "copied_bases":s.copied_bases, "copied_chunks":s.copied_chunks,
                "preserved_record_references":s.preserved_record_references, "compressed_bytes":s.compressed_bytes,
                "index_bytes":s.index_bytes, "transfer_buffer_bytes":s.transfer_buffer_bytes})),
            "storage_limits":format!("{:?}", self.storage_limits),
            "failure":self.failure.as_ref().map(|e|json!({"kind":e.kind().as_str(),"detail":e.message()})),
            "checkpoint_failure":self.checkpoint_failure.as_ref().map(|e|json!({"kind":e.kind().as_str(),"detail":e.message()})),
            "scope":"explicit resident roots or full captured bases plus resident overlay; no in-flight state"})
    }
}

struct ExportSink<'a> {
    bindings: Vec<ApplicationCacheBinding>,
    contexts: Vec<&'a CoefficientContext>,
    namespace: usize,
    writer: SavedApplicationCacheBundleWriter,
    batch: CompletedCacheBatch,
    limits: SavedApplicationCachePackLimits,
    failure: Option<AppError>,
}
impl ExportSink<'_> {
    fn fail(&mut self, error: AppError) -> GuardedCacheBackingError {
        self.failure = Some(error.clone());
        backing_error(error)
    }
    fn flush(&mut self) -> Result<(), AppError> {
        self.batch
            .flush(&self.bindings, &mut self.writer, |namespace| {
                Ok(self.contexts[namespace])
            })
    }
}
impl GuardedCacheBacking for ExportSink<'_> {
    fn binding(&self) -> &ApplicationCacheBinding {
        &self.bindings[self.namespace]
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
        _: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        let weight = CompletedCacheBatch::weight(record, self.limits.chunk)
            .map_err(|error| self.fail(error))?;
        if self.batch.buffered_records() >= 64 || self.batch.needs_flush(weight, self.limits.chunk)
        {
            self.flush().map_err(|error| self.fail(error))?;
        }
        let owned = record
            .try_clone_with_resources()
            .map_err(|error| binary_error(error, AppErrorKind::Limit))
            .map_err(|error| self.fail(error))?;
        // At most the configured bounded batch is cloned, never the whole
        // live cache. Coefficient census and native codec limits remain active.
        self.batch
            .push(self.namespace, owned, weight, self.limits.chunk)
            .map_err(|error| self.fail(error))
    }
}

impl<P: NativeCacheProgram> NativeApplicationCache<P> {
    /// Snapshot completed resident roots without changing RAM residency or
    /// eviction scores. This is not a portable copy of an attached base pack:
    /// records which have never been loaded, or were evicted, are excluded.
    pub fn save_completed(
        &self,
        output: &Path,
        limits: SavedApplicationCachePackLimits,
        resume: bool,
    ) -> Result<CacheSaveReport, AppError> {
        self.save_cache(output, CacheSaveMode::ResidentRoots, limits, resume)
    }
    pub fn save_cache(
        &self,
        output: &Path,
        mode: CacheSaveMode,
        limits: SavedApplicationCachePackLimits,
        resume: bool,
    ) -> Result<CacheSaveReport, AppError> {
        save_sessions(std::slice::from_ref(self), output, mode, limits, resume)
    }
}
impl<P: NativeCacheProgram> NativeCachePreparationBackend<P> {
    pub fn save_cache(
        &self,
        output: &Path,
        mode: CacheSaveMode,
        limits: SavedApplicationCachePackLimits,
        resume: bool,
    ) -> Result<CacheSaveReport, AppError> {
        save_sessions(self.sessions(), output, mode, limits, resume)
    }
}

fn save_sessions<P: NativeCacheProgram>(
    sessions: &[NativeApplicationCache<P>],
    output: &Path,
    mode: CacheSaveMode,
    limits: SavedApplicationCachePackLimits,
    resume: bool,
) -> Result<CacheSaveReport, AppError> {
    if (limits.chunk.diagnostics_enabled && limits.chunk.max_records == 0)
        || limits.max_disk_bytes == 0
    {
        return Err(AppError::input(
            "live cache export needs positive batch and disk allowances",
        ));
    }
    if sessions.is_empty() || (limits.diagnostics_enabled && sessions.len() > limits.max_namespaces)
    {
        return Err(AppError::limit("cache export namespace allowance exceeded"));
    }
    // Check aggregate report counters before any output is created.
    let mut total = GuardedRootExportStatistics::default();
    for session in sessions {
        let stats = session.cache_statistics();
        add_statistics(
            &mut total,
            GuardedRootExportStatistics {
                exported_records: stats.cached_integrals,
                exported_coefficient_terms: stats.cached_coefficient_terms,
                exported_coefficient_bytes: stats.cached_coefficient_bytes,
            },
        )?;
    }
    let bindings = sessions
        .iter()
        .map(|s| s.program().binding().clone())
        .collect::<Vec<_>>();
    let bases = sessions
        .iter()
        .filter_map(|s| s.backing_pack())
        .collect::<Vec<_>>();
    let (writer, base_copy) = if mode == CacheSaveMode::FullSnapshot {
        if resume {
            return Err(AppError::input(
                "full snapshot requires a new output; resume is only for resident-root saves",
            ));
        }
        let (writer, statistics) = SavedApplicationCacheBundleWriter::copy_bases_to_new(
            output, &bindings, &bases, limits,
        )?;
        (writer, Some(statistics))
    } else if output
        .try_exists()
        .map_err(|e| AppError::input(e.to_string()))?
    {
        if !resume {
            return Err(AppError::input(
                "cache export already exists and resume is disabled",
            ));
        }
        (
            SavedApplicationCacheBundleWriter::resume(output, &bindings, limits)?,
            None,
        )
    } else {
        (
            SavedApplicationCacheBundleWriter::create(output, &bindings, limits)?,
            None,
        )
    };
    let mut sink = ExportSink {
        bindings,
        contexts: sessions.iter().map(|s| s.program().context()).collect(),
        namespace: 0,
        writer,
        batch: CompletedCacheBatch::new(sessions.len()),
        limits,
        failure: None,
    };
    let mut accepted = GuardedRootExportStatistics::default();
    let mut failure = None;
    for (namespace, session) in sessions.iter().enumerate() {
        sink.namespace = namespace;
        let (part, error) = match session.export_to(&mut sink) {
            Ok(statistics) => (statistics, None),
            Err(error) => (
                error.accepted_prefix,
                Some(
                    sink.failure
                        .take()
                        .unwrap_or_else(|| guarded_error(error.source)),
                ),
            ),
        };
        if let Err(error) = add_statistics(&mut accepted, part) {
            failure = Some(error);
            break;
        }
        if error.is_some() {
            failure = error;
            break;
        }
    }
    // A failed admission may leave earlier accepted records buffered.
    // Preserve them when the writer's publication state is still certain.
    let checkpoint_failure = if sink.writer.is_poisoned() {
        None
    } else {
        sink.flush().err()
    };
    Ok(CacheSaveReport {
        skipped_partial_records: 0,
        complete: failure.is_none()
            && checkpoint_failure.is_none()
            && sink.batch.buffered_records() == 0,
        accepted,
        durable_records: sink.batch.durable_records(),
        charged_disk_bytes: sink.writer.charged_disk_bytes()?,
        backing_attached: !bases.is_empty(),
        mode,
        base_copy,
        storage_limits: limits,
        failure,
        checkpoint_failure,
    })
}

fn add_statistics(
    total: &mut GuardedRootExportStatistics,
    part: GuardedRootExportStatistics,
) -> Result<(), AppError> {
    let checked = |left: usize, right: usize| {
        left.checked_add(right)
            .ok_or_else(|| AppError::limit("cache export aggregate counter overflow"))
    };
    *total = GuardedRootExportStatistics {
        exported_records: checked(total.exported_records, part.exported_records)?,
        exported_coefficient_terms: checked(
            total.exported_coefficient_terms,
            part.exported_coefficient_terms,
        )?,
        exported_coefficient_bytes: checked(
            total.exported_coefficient_bytes,
            part.exported_coefficient_bytes,
        )?,
    };
    Ok(())
}
