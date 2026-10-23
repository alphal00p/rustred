use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use rustred::{
    algebra::CoefficientContext,
    persistence::application_cache::{
        ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
    },
    reduction::GuardedIntermediateCapture,
};
use serde_json::{Value, json};

use super::{
    CachePreparationResolvedOptions, CachePreparationTarget, CachePreparationWorkload,
    batch::CompletedCacheBatch, capture::PreparationCapture,
};
use crate::{
    AppError, AppErrorKind, SavedApplicationCacheBundle, SavedApplicationCacheBundleWriter,
    SavedApplicationCachePack,
};

/// Native execution boundary. Implementations own admission and reduction;
/// this runner owns no algebra, terminal inference or eviction algorithm.
pub trait CachePreparationBackend {
    fn bindings(&self) -> &[ApplicationCacheBinding];
    /// Native operational root stage. Single-solver programs use GuardedRoot;
    /// saved routed applications override this with Route.
    fn root_key(&self, target: &CachePreparationTarget) -> ApplicationCacheKey {
        ApplicationCacheKey::guarded_root(target.integral())
    }
    fn context(&self, namespace: usize) -> Result<&CoefficientContext, AppError>;
    /// Apply the explicit preparation RAM/policy without enlarging unrelated
    /// native arithmetic or work allowances.
    fn configure(&mut self, options: &CachePreparationResolvedOptions) -> Result<(), AppError>;
    /// Optional native descendant-cache attachment. The runner still owns
    /// root-level resume validation; native programs must also admit and
    /// validate every backing record they use during a reduction.
    fn attach_completed_cache(
        &mut self,
        _namespace: usize,
        _reader: SavedApplicationCachePack,
    ) -> Result<(), AppError> {
        Ok(())
    }
    /// Called only after durable batches, never per requested root. Preserve
    /// native RAM/work policy and cumulative I/O counters when refreshing.
    fn refresh_completed_caches(&mut self) -> Result<(), AppError> {
        Ok(())
    }
    /// Upper census of backing directories retained by the backend, distinct
    /// from runner readers, retained coefficient payload and process RSS.
    fn completed_cache_directory_bytes(&self) -> Result<usize, AppError> {
        Ok(0)
    }
    /// Must use the active native program's entry admission, including zeros.
    /// Called before every existing-record lookup, even when resuming.
    fn validate_entry(&self, target: &CachePreparationTarget) -> Result<(), AppError>;
    /// Validate active terminal ownership, context and complete guard payload.
    /// A binding/hash match alone does not grant native reduction authority.
    fn validate_completed_record(
        &self,
        namespace: usize,
        record: &ApplicationCacheRecord,
    ) -> Result<(), AppError>;
    fn reduce_guarded_root(
        &mut self,
        target: &CachePreparationTarget,
    ) -> Result<ApplicationCacheRecord, AppError>;
    /// Native implementations override this to emit completed intermediate
    /// reductions. The runner owns their bounded durable stream, not algebra.
    fn reduce_guarded_root_with_capture(
        &mut self,
        target: &CachePreparationTarget,
        _capture: &mut dyn GuardedIntermediateCapture,
    ) -> Result<ApplicationCacheRecord, AppError> {
        self.reduce_guarded_root(target)
    }
    /// Full current native operational profile, distinct from entry coverage.
    fn operational_profile(&self) -> Value;
    /// Native counters, not inferred from the number of stored root records.
    fn statistics(&self) -> Value;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CachePreparationStatus {
    Running,
    Complete,
    TimeBudget,
    Cancelled,
    StorageBudget,
    ResourceRefusal,
    Failed,
}
impl CachePreparationStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Complete => "complete",
            Self::TimeBudget => "time-budget",
            Self::Cancelled => "cancelled",
            Self::StorageBudget => "storage-budget",
            Self::ResourceRefusal => "resource-refusal",
            Self::Failed => "failed",
        }
    }
}

/// A partial report is not successful completion. Only selected workload roots
/// with checkpointed records or validated records already on disk contribute
/// to completed_roots. Complete does not assert that every possible internal
/// node was captured: optional-capture refusal counters remain explicit.
#[derive(Clone, Debug)]
pub struct CachePreparationReport {
    pub status: CachePreparationStatus,
    pub total_roots: usize,
    pub completed_roots: usize,
    pub newly_stored_roots: usize,
    /// Newly durable records which were not selected-root acknowledgements.
    pub newly_stored_intermediates: usize,
    pub capture_lookup_budget_skips: usize,
    pub capture_oversized_records: usize,
    pub capture_offered_records: usize,
    pub capture_native_skipped_records: usize,
    pub capture_metadata_exhausted_roots: usize,
    pub capture_metadata_peak_bytes: usize,
    pub reused_roots: usize,
    pub elapsed: Duration,
    pub charged_disk_bytes: u64,
    /// Sampled with native statistics at most once per second at selected-root
    /// boundaries; refreshed for the final report unless the census fails. A long individual
    /// native reduction does not invoke this runner's observer mid-reduction.
    pub directory_owned_bytes: usize,
    pub options: Value,
    pub native_profile: Value,
    /// Same sampling boundary as `directory_owned_bytes`, not a live view of
    /// an in-flight native reduction. Disk charges and durable counts are
    /// current at every observer callback independently of this sampling.
    pub native_statistics: Value,
    pub failure: Option<AppError>,
    pub checkpoint_failure: Option<AppError>,
}

/// Keep telemetry work off the per-root hot path while bounding its lag at
/// existing observer boundaries. This only reads counters and retained index
/// census; it never refreshes indices from disk or alters reduction state.
#[derive(Default)]
struct ProgressRefresh {
    sampled_at: Option<Duration>,
}
impl ProgressRefresh {
    fn refresh(
        &mut self,
        elapsed: Duration,
        force: bool,
        report: &mut CachePreparationReport,
        backend: &impl CachePreparationBackend,
        writer: &SavedApplicationCacheBundleWriter,
        readers: &[SavedApplicationCachePack],
    ) -> Result<(), AppError> {
        // The aggregate writer quota is already maintained by durable writes;
        // this is a cheap read, not a directory scan or payload census.
        report.charged_disk_bytes = writer.charged_disk_bytes()?;
        if !force
            && self
                .sampled_at
                .is_some_and(|sampled| elapsed.saturating_sub(sampled) < Duration::from_secs(1))
        {
            return Ok(());
        }
        let runner_directory_bytes = readers
            .iter()
            .map(|reader| reader.statistics().directory_owned_bytes)
            .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
            .ok_or_else(|| AppError::limit("cache reader directory byte count overflow"))?;
        let directory_owned_bytes = runner_directory_bytes
            .checked_add(backend.completed_cache_directory_bytes()?)
            .ok_or_else(|| AppError::limit("cache reader directory byte count overflow"))?;
        report.native_statistics = backend.statistics();
        report.directory_owned_bytes = directory_owned_bytes;
        self.sampled_at = Some(elapsed);
        Ok(())
    }
}
impl CachePreparationReport {
    pub fn to_document(&self) -> Value {
        json!({
            "status": self.status.label(), "total_roots": self.total_roots,
            "completed_roots": self.completed_roots, "newly_stored_roots": self.newly_stored_roots,
            "newly_stored_intermediates": self.newly_stored_intermediates,
            "capture_lookup_budget_skips": self.capture_lookup_budget_skips,
            "capture_oversized_records": self.capture_oversized_records,
            "capture_offered_records": self.capture_offered_records,
            "capture_native_skipped_records": self.capture_native_skipped_records,
            "capture_metadata_exhausted_roots": self.capture_metadata_exhausted_roots,
            "capture_metadata_peak_bytes": self.capture_metadata_peak_bytes,
            "reused_roots": self.reused_roots, "elapsed_seconds": self.elapsed.as_secs_f64(),
            "charged_disk_bytes": self.charged_disk_bytes,
            "reader_directory_owned_bytes": self.directory_owned_bytes,
            "options": self.options, "native_profile": self.native_profile,
            "native_statistics": self.native_statistics,
            "failure": self.failure.as_ref().map(|e| json!({"kind": e.kind().as_str(), "detail": e.message()})),
            "checkpoint_failure": self.checkpoint_failure.as_ref().map(|e| json!({"kind": e.kind().as_str(), "detail": e.message()})),
            "capture_profile": CAPTURE_PROFILE,
            "resume_semantics": "durable selected roots and guarded intermediates; no in-flight native operation state",
            "coverage_claim": false,
        })
    }
    fn stop(&mut self, error: AppError, storage: bool) {
        self.status = match error.kind() {
            AppErrorKind::Cancelled => CachePreparationStatus::Cancelled,
            AppErrorKind::Limit | AppErrorKind::OutputLimit if storage => {
                CachePreparationStatus::StorageBudget
            }
            AppErrorKind::Limit | AppErrorKind::OutputLimit => {
                CachePreparationStatus::ResourceRefusal
            }
            _ => CachePreparationStatus::Failed,
        };
        self.failure = Some(error);
    }
}

#[derive(bincode::Encode, bincode::Decode)]
struct Checkpoint {
    version: u32,
    workload: [u8; 32],
    total: u64,
    completed: u64,
    status: String,
    options: String,
    native_profile: String,
    capture_profile: String,
}
const CAPTURE_PROFILE: &str = "completed-intermediates-with-local-guard-provenance-v1";
impl Checkpoint {
    fn verify(bytes: &[u8], workload: &CachePreparationWorkload) -> Result<(), AppError> {
        let (version, _): (u32, usize) =
            bincode::decode_from_slice(bytes, bincode::config::standard())
                .map_err(|error| AppError::serialization(error.to_string()))?;
        if version != 2 {
            return Err(AppError::input(
                "cache preparation capture profile changed; use a new output directory to capture intermediate descendants",
            ));
        }
        let (checkpoint, used): (Self, usize) = bincode::decode_from_slice(
            bytes,
            bincode::config::standard().with_limit::<{ 64 * 1024 * 1024 }>(),
        )
        .map_err(|error| AppError::serialization(error.to_string()))?;
        if used != bytes.len()
            || checkpoint.version != 2
            || checkpoint.capture_profile != CAPTURE_PROFILE
            || checkpoint.workload != *workload.digest()
            || checkpoint.total != workload.targets().len() as u64
            || checkpoint.completed > checkpoint.total
        {
            return Err(AppError::input(
                "cache preparation checkpoint/workload mismatch",
            ));
        }
        Ok(())
    }
    fn publish(
        workload: &CachePreparationWorkload,
        report: &CachePreparationReport,
        writer: &mut SavedApplicationCacheBundleWriter,
    ) -> Result<(), AppError> {
        let checkpoint = Self {
            version: 2,
            workload: *workload.digest(),
            total: report.total_roots as u64,
            completed: report.completed_roots as u64,
            status: report.status.label().into(),
            options: report.options.to_string(),
            native_profile: report.native_profile.to_string(),
            capture_profile: CAPTURE_PROFILE.into(),
        };
        let bytes = bincode::encode_to_vec(checkpoint, bincode::config::standard())
            .map_err(|error| AppError::serialization(error.to_string()))?;
        writer.checkpoint_metadata(&bytes)
    }
}

impl CompletedCacheBatch {
    fn flush_preparation(
        &mut self,
        workload: &CachePreparationWorkload,
        backend: &mut impl CachePreparationBackend,
        writer: &mut SavedApplicationCacheBundleWriter,
        report: &mut CachePreparationReport,
        readers: &mut [SavedApplicationCachePack],
    ) -> Result<(), AppError> {
        let before = self.durable_records();
        let selected_before = self.durable_selected_roots();
        let result = self.flush(workload.bindings(), writer, |namespace| {
            backend.context(namespace)
        });
        // A later namespace may fail after an earlier child was published.
        // Keep that durable prefix in the report even when returning an error.
        let published = self.durable_records() - before;
        let selected = self.durable_selected_roots() - selected_before;
        report.completed_roots += selected;
        report.newly_stored_roots += selected;
        report.newly_stored_intermediates += published - selected;
        result?;
        if published != 0 {
            backend.refresh_completed_caches()?;
            for reader in readers {
                reader.refresh()?;
            }
        }
        Ok(())
    }
}

/// Explicit finite warmup. All filesystem and coefficient serialization is
/// delegated to the native cache bundle. No automatic evaluation download or
/// new root-domain authority is introduced. Setup errors return Err; errors
/// after setup produce an explicitly partial report retaining completed data.
pub fn prepare_application_cache(
    workload: &CachePreparationWorkload,
    options: &CachePreparationResolvedOptions,
    output: &Path,
    cancellation: &AtomicBool,
    backend: &mut impl CachePreparationBackend,
    observer: impl FnMut(&CachePreparationReport) -> Result<(), AppError>,
) -> Result<CachePreparationReport, AppError> {
    prepare_application_cache_with_progress_clock(
        workload,
        options,
        output,
        cancellation,
        backend,
        observer,
        |started| started.elapsed(),
    )
}

// Only telemetry sampling uses this private clock seam. Time budgets,
// checkpoints, cancellation and the public observer cadence are unchanged.
pub(super) fn prepare_application_cache_with_progress_clock(
    workload: &CachePreparationWorkload,
    options: &CachePreparationResolvedOptions,
    output: &Path,
    cancellation: &AtomicBool,
    backend: &mut impl CachePreparationBackend,
    mut observer: impl FnMut(&CachePreparationReport) -> Result<(), AppError>,
    mut progress_clock: impl FnMut(Instant) -> Duration,
) -> Result<CachePreparationReport, AppError> {
    options.validate()?;
    if workload.bindings() != backend.bindings() {
        return Err(AppError::input(
            "cache workload differs from active program bindings",
        ));
    }
    if workload.targets().len() > options.max_roots {
        return Err(AppError::limit(
            "cache workload exceeds resolved root allowance",
        ));
    }
    backend.configure(options)?;
    let started = Instant::now();
    let existed = output
        .try_exists()
        .map_err(|e| AppError::input(e.to_string()))?;
    let mut writer = if existed {
        if !options.resume {
            return Err(AppError::input(
                "cache output already exists and resume is disabled",
            ));
        }
        SavedApplicationCacheBundleWriter::resume(
            output,
            workload.bindings(),
            options.storage_limits,
        )?
    } else {
        SavedApplicationCacheBundleWriter::create(
            output,
            workload.bindings(),
            options.storage_limits,
        )?
    };
    let bundle = SavedApplicationCacheBundle::open_generated(
        output,
        workload.bindings(),
        options.storage_limits,
    )?;
    if existed {
        let metadata = bundle
            .metadata()?
            .ok_or_else(|| AppError::input("cache preparation metadata is absent"))?;
        Checkpoint::verify(&metadata, workload)?;
    }
    let mut readers: Vec<SavedApplicationCachePack> = workload
        .bindings()
        .iter()
        .map(|binding| bundle.open_reader(binding))
        .collect::<Result<_, _>>()?;
    for (namespace, reader) in readers.iter().enumerate() {
        backend.attach_completed_cache(namespace, reader.fork_reader())?;
    }
    // Contexts share Symbolica variable maps; no source expressions or program
    // state are cloned. A capture callback may flush while the native backend
    // is mutably reducing a root, so it cannot borrow that backend again.
    let capture_contexts = (0..workload.bindings().len())
        .map(|namespace| backend.context(namespace).cloned())
        .collect::<Result<Vec<_>, _>>()?;
    let mut report = CachePreparationReport {
        status: CachePreparationStatus::Running,
        total_roots: workload.targets().len(),
        completed_roots: 0,
        newly_stored_roots: 0,
        newly_stored_intermediates: 0,
        capture_lookup_budget_skips: 0,
        capture_oversized_records: 0,
        capture_offered_records: 0,
        capture_native_skipped_records: 0,
        capture_metadata_exhausted_roots: 0,
        capture_metadata_peak_bytes: 0,
        reused_roots: 0,
        elapsed: started.elapsed(),
        charged_disk_bytes: 0,
        directory_owned_bytes: 0,
        options: options.to_document(),
        native_profile: backend.operational_profile(),
        native_statistics: Value::Null,
        failure: None,
        checkpoint_failure: None,
    };
    let mut progress = ProgressRefresh::default();
    progress.refresh(
        progress_clock(started),
        true,
        &mut report,
        backend,
        &writer,
        &readers,
    )?;
    Checkpoint::publish(workload, &report, &mut writer)?;
    let mut pending = CompletedCacheBatch::new(workload.bindings().len());
    let mut last_checkpoint = started;
    let limits = options.storage_limits.chunk;
    let mut storage_failed = false;
    for target in workload.execution_order() {
        report.elapsed = started.elapsed();
        if cancellation.load(Ordering::Relaxed) {
            report.status = CachePreparationStatus::Cancelled;
            break;
        }
        if report.elapsed >= options.time_limit {
            report.status = CachePreparationStatus::TimeBudget;
            break;
        }
        if let Err(error) = progress.refresh(
            progress_clock(started),
            false,
            &mut report,
            backend,
            &writer,
            &readers,
        ) {
            report.stop(error, false);
            break;
        }
        if let Err(error) = observer(&report) {
            report.stop(error, false);
            break;
        }
        let mut capture_storage_failure = false;
        let result = (|| {
            backend.validate_entry(target)?;
            let key = backend.root_key(target);
            if let Some(record) = pending.get(target.namespace(), &key) {
                backend.validate_completed_record(target.namespace(), record)?;
                pending.mark_selected_root(target.namespace(), &key)?;
                return Ok(None);
            }
            if let Some(record) =
                readers[target.namespace()].lookup(&key, backend.context(target.namespace())?)?
            {
                backend.validate_completed_record(target.namespace(), &record)?;
                report.reused_roots += 1;
                report.completed_roots += 1;
                return Ok(None);
            }
            let mut capture = PreparationCapture {
                namespace: target.namespace(),
                bindings: workload.bindings(),
                contexts: &capture_contexts,
                options,
                cancellation,
                pending: &mut pending,
                readers: &mut readers,
                writer: &mut writer,
                report: &mut report,
                failure: None,
                storage_failed: false,
                failure_is_storage: false,
                published: false,
            };
            let reduced = backend.reduce_guarded_root_with_capture(target, &mut capture);
            let capture_failure = capture.failure.take();
            let published = capture.published;
            storage_failed |= capture.storage_failed;
            capture_storage_failure = capture.failure_is_storage;
            drop(capture);
            if published {
                backend.refresh_completed_caches()?;
            }
            if let Some(error) = capture_failure {
                return Err(error);
            }
            let record = reduced?;
            if record.key != key {
                return Err(AppError::internal_invariant(
                    "native prepared root has a different key or stage",
                ));
            }
            backend.validate_completed_record(target.namespace(), &record)?;
            Ok(Some(record))
        })();
        let record = match result {
            Ok(record) => record,
            Err(error) => {
                report.stop(error, capture_storage_failure);
                break;
            }
        };
        if let Some(record) = record {
            let weight = match CompletedCacheBatch::weight(&record, limits) {
                Ok(weight) => weight,
                Err(error) => {
                    report.stop(error, true);
                    break;
                }
            };
            if pending.buffered_records() >= options.batch_records
                || pending.needs_flush(weight, limits)
            {
                if let Err(error) = pending.flush_preparation(
                    workload,
                    backend,
                    &mut writer,
                    &mut report,
                    &mut readers,
                ) {
                    report.stop(error, true);
                    storage_failed = true;
                    break;
                }
            }
            let key = record.key.clone();
            if let Err(error) = pending
                .push(target.namespace(), record, weight, limits)
                .and_then(|()| pending.mark_selected_root(target.namespace(), &key))
            {
                report.stop(error, true);
                break;
            }
        }
        if last_checkpoint.elapsed() >= options.checkpoint_interval {
            let checkpoint = pending
                .flush_preparation(workload, backend, &mut writer, &mut report, &mut readers)
                .and_then(|()| Checkpoint::publish(workload, &report, &mut writer));
            if let Err(error) = checkpoint {
                report.stop(error, true);
                storage_failed = true;
                break;
            }
            last_checkpoint = Instant::now();
        }
    }
    if !storage_failed {
        if let Err(error) =
            pending.flush_preparation(workload, backend, &mut writer, &mut report, &mut readers)
        {
            if report.failure.is_none() {
                report.stop(error, true);
            } else {
                report.checkpoint_failure = Some(error);
            }
        }
    }
    if report.status == CachePreparationStatus::Running {
        report.status = CachePreparationStatus::Complete;
    }
    report.elapsed = started.elapsed();
    if let Err(error) = progress.refresh(
        progress_clock(started),
        true,
        &mut report,
        backend,
        &writer,
        &readers,
    ) {
        if report.failure.is_none() {
            report.stop(error, false);
        }
        // Preserve an earlier primary failure. A telemetry census failure is
        // not a failed checkpoint and must not overwrite that separate receipt.
    }
    if let Err(error) = Checkpoint::publish(workload, &report, &mut writer) {
        report.checkpoint_failure = Some(error);
        if report.status == CachePreparationStatus::Complete {
            report.status = CachePreparationStatus::Failed;
        }
    }
    report.charged_disk_bytes = writer.charged_disk_bytes()?;
    // Never announce completion until the final progress checkpoint is durable.
    if let Err(error) = observer(&report) {
        if report.failure.is_none() {
            report.stop(error, false);
            if let Err(error) = Checkpoint::publish(workload, &report, &mut writer) {
                report.checkpoint_failure = Some(error);
            }
            report.charged_disk_bytes = writer.charged_disk_bytes()?;
        }
    }
    Ok(report)
}
