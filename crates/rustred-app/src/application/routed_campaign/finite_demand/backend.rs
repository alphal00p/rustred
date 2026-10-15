//! Private exact-window -> native seal -> atomic generated cursor lifecycle.
//! No public campaign frontend, CP6 adoption or publication/export authority.
mod errors;
use super::{
    prepared::PreparedExecution,
    storage::{
        self, CursorRecord, GenerationState, RawCensus, ScopeBinding, ScopeRole, ScopeRow,
        StorageLimits, StoreWriter, ValidatedGeneration,
    },
    trace::ReductionAggregateOverrides,
    windows::{PrefixCursor, PrefixWindows, WindowLimits},
};
use crate::application::routed_campaign::{input::Selection, snapshot_json, walking::finite_scope};
use crate::{AppError, AppErrorKind, OwnerDomainWalkRequest, RoutedCampaignRequest};
use rustred::solver::{
    CandidateEntryAdmission, CandidateRoutedDeltaExportLimits, CandidateRoutedTraceCensusLimits,
    CandidateRoutedWorkBudget, FiniteRootAdmission,
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use symbolica::domains::integer::Integer;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Policy {
    pub workers: usize,
    pub windows: WindowLimits,
    pub per_batch_work: CandidateRoutedWorkBudget,
    pub prepared_reduction_aggregates: Option<ReductionAggregateOverrides>,
    pub census: CandidateRoutedTraceCensusLimits,
    pub export: CandidateRoutedDeltaExportLimits,
    pub storage: StorageLimits,
    pub max_total_rows: usize,
    pub max_source_bytes: usize,
}

/// Operational controls are reported, not authority to change saved scope.
#[derive(Clone, Debug)]
pub(crate) struct Progress {
    pub required_rows: usize,
    pub exhausted_required_rows: usize,
    pub auxiliary_rows: usize,
    pub exhausted_auxiliary_rows: usize,
    pub required_complete: bool,
    pub all_rows_exhausted: bool,
    /// Physical workers have joined; pending graph obligations may remain.
    pub no_active_native_work: bool,
    pub stopped: bool,
    pub input_occurrences: u64,
    pub completed_work: u64,
    pub current_row: Option<usize>,
    pub current_window: Option<u64>,
    pub generation: [u8; 32],
    pub generation_number: u64,
    pub checkpoint: PathBuf,
    pub policy: Policy,
    pub source_reduction: rustred::reduction::ReductionLimits,
    pub effective_reduction: rustred::reduction::ReductionLimits,
}
#[derive(Clone, Debug)]
pub(crate) struct CompletedWindow {
    pub row: usize,
    pub inputs: usize,
    pub new_work: usize,
    pub baseline_hits: usize,
    pub cleanup_warning: Option<String>,
}
#[derive(Clone, Debug)]
pub(crate) struct Step {
    pub window: Option<CompletedWindow>,
    pub progress: Progress,
}

/// No operation accepts caller-supplied starts, seals or successor bytes.
/// An error permanently stops this object; cold reopen determines CURRENT.
pub(crate) struct Session<const N: usize> {
    prepared: PreparedExecution<N>,
    scope: ScopeBinding,
    sources: Vec<PrefixWindows<N>>,
    cursors: Vec<PrefixCursor>,
    writer: StoreWriter,
    policy: Policy,
    path: PathBuf,
    stopped: bool,
    source_reduction: rustred::reduction::ReductionLimits,
    effective_reduction: rustred::reduction::ReductionLimits,
}

struct Admitted<const N: usize> {
    prepared: PreparedExecution<N>,
    scope: ScopeBinding,
    sources: Vec<PrefixWindows<N>>,
    cursors: Vec<PrefixCursor>,
    source_reduction: rustred::reduction::ReductionLimits,
    effective_reduction: rustred::reduction::ReductionLimits,
}

impl<const N: usize> Session<N> {
    pub(crate) fn create(
        source: &OwnerDomainWalkRequest,
        policy: Policy,
        path: &Path,
        cancel: &AtomicBool,
        observer: &impl Fn(Value),
    ) -> Result<Self, AppError> {
        let admitted = admit::<N>(source, policy, cancel, observer)?;
        let state = GenerationState {
            scope: admitted.scope.clone(),
            cursors: admitted
                .sources
                .iter()
                .zip(&admitted.cursors)
                .map(|(s, c)| stored_cursor(s, c, 0))
                .collect::<Result<_, _>>()?,
            completed_work: 0,
            input_occurrences: 0,
            census: RawCensus::default(),
        };
        check(cancel)?;
        let mut writer =
            StoreWriter::create(path, admitted.prepared.execution().clone(), policy.storage)
                .map_err(storage_error)?;
        // Always publish the pending generation before the first native window.
        writer
            .commit_generated(Vec::new(), state, cancel)
            .map_err(storage_error)?;
        Ok(Self::assemble(admitted, writer, policy, path))
    }

    pub(crate) fn resume(
        source: &OwnerDomainWalkRequest,
        policy: Policy,
        path: &Path,
        cancel: &AtomicBool,
        observer: &impl Fn(Value),
    ) -> Result<Self, AppError> {
        let mut admitted = admit::<N>(source, policy, cancel, observer)?;
        // Read/authenticate/restore BEFORE opening a writer or modifying scope.
        let previous = ValidatedGeneration::open_generated(
            path,
            admitted.prepared.execution(),
            policy.storage,
        )
        .map_err(storage_error)?;
        let old = &previous.state().scope;
        if old.original_source != admitted.scope.original_source
            || !admitted.scope.rows.starts_with(&old.rows)
        {
            return Err(AppError::input(
                "finite scope is not an exact append-only extension",
            ));
        }
        for (index, row) in old.rows.iter().enumerate() {
            admitted.cursors[index] = admitted.sources[index]
                .restore_trusted_generation_cursor(
                    &previous.cursor(index).map_err(storage_error)?,
                    previous.digest(),
                    row,
                    cancel,
                )
                .map_err(window_error)?;
        }
        let reader = previous.reader(policy.storage).map_err(storage_error)?;
        // Current-state census authentication only; no historical reseeding.
        admitted.prepared.bind_generation(
            &reader,
            old,
            policy.per_batch_work,
            policy.census,
            policy.export,
            cancel,
        )?;
        check(cancel)?;
        let mut writer = StoreWriter::resume_generated(
            path,
            admitted.prepared.execution().clone(),
            policy.storage,
        )
        .map_err(storage_error)?;
        if writer
            .current()
            .map_err(storage_error)?
            .map(ValidatedGeneration::digest)
            != Some(previous.digest())
        {
            return Err(AppError::input(
                "finite CURRENT changed while acquiring writer",
            ));
        }
        if admitted.scope.rows.len() > old.rows.len() {
            let mut state = previous.state().clone();
            state.scope = admitted.scope.clone();
            for index in old.rows.len()..admitted.scope.rows.len() {
                state.cursors.push(stored_cursor(
                    &admitted.sources[index],
                    &admitted.cursors[index],
                    0,
                )?);
            }
            // Metadata-only append: old cursor bytes, Work and raw census stay exact.
            writer
                .commit_generated(Vec::new(), state, cancel)
                .map_err(storage_error)?;
        }
        Ok(Self::assemble(admitted, writer, policy, path))
    }

    fn assemble(admitted: Admitted<N>, writer: StoreWriter, policy: Policy, path: &Path) -> Self {
        Self {
            prepared: admitted.prepared,
            scope: admitted.scope,
            sources: admitted.sources,
            cursors: admitted.cursors,
            writer,
            policy,
            path: path.to_path_buf(),
            stopped: false,
            source_reduction: admitted.source_reduction,
            effective_reduction: admitted.effective_reduction,
        }
    }

    pub(crate) fn generation(&self) -> Result<ValidatedGeneration, AppError> {
        self.writer
            .current()
            .map_err(storage_error)?
            .cloned()
            .ok_or_else(|| AppError::internal_invariant("pending finite generation absent"))
    }
    pub(crate) fn progress(&self) -> Result<Progress, AppError> {
        let generation = self.generation()?;
        let mut required = 0;
        let mut required_done = 0;
        let mut auxiliary = 0;
        let mut auxiliary_done = 0;
        for (row, cursor) in self.scope.rows.iter().zip(&self.cursors) {
            match row.role {
                ScopeRole::Required => {
                    required += 1;
                    required_done += usize::from(cursor.is_exhausted());
                }
                ScopeRole::Auxiliary => {
                    auxiliary += 1;
                    auxiliary_done += usize::from(cursor.is_exhausted());
                }
            }
        }
        let current = self.cursors.iter().position(|c| !c.is_exhausted());
        Ok(Progress {
            required_rows: required,
            exhausted_required_rows: required_done,
            auxiliary_rows: auxiliary,
            exhausted_auxiliary_rows: auxiliary_done,
            required_complete: required == required_done,
            all_rows_exhausted: current.is_none(),
            no_active_native_work: true,
            stopped: self.stopped,
            input_occurrences: generation.state().input_occurrences,
            completed_work: generation.state().completed_work,
            current_row: current,
            current_window: current.map(|i| self.cursors[i].windows()),
            generation: generation.digest(),
            generation_number: generation.generation(),
            checkpoint: self.path.clone(),
            policy: self.policy,
            source_reduction: self.source_reduction,
            effective_reduction: self.effective_reduction,
        })
    }

    pub(crate) fn step(
        &mut self,
        cancel: &AtomicBool,
        observer: &impl Fn(Value),
    ) -> Result<Step, AppError> {
        if self.stopped {
            return Err(AppError::execution(
                "finite session stopped; cold reopen required",
            ));
        }
        let result = self.step_inner(cancel, observer);
        if result.is_err() {
            self.stopped = true;
        }
        result
    }
    fn step_inner(
        &mut self,
        cancel: &AtomicBool,
        observer: &impl Fn(Value),
    ) -> Result<Step, AppError> {
        check(cancel)?;
        let Some(index) = self.cursors.iter().position(|c| !c.is_exhausted()) else {
            return Ok(Step {
                window: None,
                progress: self.progress()?,
            });
        };
        let window = self.sources[index]
            .next_window(&self.cursors[index], cancel)
            .map_err(window_error)?
            .ok_or_else(|| {
                AppError::internal_invariant("unfinished finite cursor has no window")
            })?;
        let previous = self.generation()?;
        let reader = previous
            .reader(self.policy.storage)
            .map_err(storage_error)?;
        // This conservative first backend revalidates current metadata per
        // batch. O(census) merge/revalidation and run fan-out remain scale limits.
        let baseline = self.prepared.bind_generation(
            &reader,
            &self.scope,
            self.policy.per_batch_work,
            self.policy.census,
            self.policy.export,
            cancel,
        )?;
        let admission = FiniteRootAdmission::try_new([window.region().clone()], 1)
            .map_err(|e| AppError::input(e.to_string()))?;
        let delta = baseline
            .trace_batch(
                window.targets().iter().cloned(),
                CandidateEntryAdmission::ExplicitFinite(&admission),
                self.policy.workers,
                cancel,
                |s| {
                    observer(json!({"event":"finite_backend_native","row":index,
                "window":self.cursors[index].windows(),"native":snapshot_json(s)}))
                },
            )
            .map_err(errors::completed)?;
        let snapshot = delta.report().snapshot();
        let multiplicity = delta
            .report()
            .complete_census()
            .ok_or_else(|| AppError::internal_invariant("native window census absent"))?
            .selections()
            .values()
            .try_fold(0u64, |sum, count| sum.checked_add(*count))
            .ok_or_else(|| AppError::limit("native window census multiplicity overflow"))?;
        if delta.predecessor() != previous.digest()
            || !delta.is_for_reducer(self.prepared.reducer())
            || snapshot.input_targets != window.targets().len()
            || snapshot.requested_targets != window.targets().len()
            || delta.report().complete_census().is_none()
            || u64::try_from(snapshot.rule_applications).ok() != Some(multiplicity)
        {
            return Err(AppError::internal_invariant(
                "exact finite window/native seal binding differs",
            ));
        }
        let mut completed = CompletedWindow {
            row: index,
            inputs: window.targets().len(),
            new_work: delta.work().len(),
            baseline_hits: snapshot.completed_baseline_hits,
            cleanup_warning: None,
        };
        let mut cursors = previous.state().cursors.clone();
        let occurrences = cursors[index]
            .input_occurrences
            .checked_add(
                u64::try_from(window.targets().len())
                    .map_err(|_| AppError::limit("window input count exceeds u64"))?,
            )
            .ok_or_else(|| AppError::limit("cursor occurrence overflow"))?;
        cursors[index] = stored_cursor(&self.sources[index], window.successor(), occurrences)?;
        let successor = window.successor().clone();
        observer(
            json!({"event":"finite_backend_before_commit","row":index,"window":self.cursors[index].windows(),
            "inputs":completed.inputs,"new_work":completed.new_work,"baseline_hits":completed.baseline_hits}),
        );
        check(cancel)?;
        let outcome = self.prepared.commit_delta_with_generated_metadata(
            &mut self.writer,
            delta,
            self.scope.clone(),
            cursors,
            cancel,
        )?;
        // No fallible work or observer lies between durable commit and adoption.
        self.cursors[index] = successor;
        completed.cleanup_warning = outcome.cleanup_warning;
        Ok(Step {
            window: Some(completed),
            progress: self.progress()?,
        })
    }
}

fn admit<const N: usize>(
    source: &OwnerDomainWalkRequest,
    policy: Policy,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<Admitted<N>, AppError> {
    check(cancel)?;
    if policy.workers == 0 {
        return Err(AppError::input("finite backend workers must be positive"));
    }
    rustred::campaign::ParallelExecution::preflight_requested_core_budget(policy.workers)
        .map_err(|e| AppError::input(e.to_string()))?;
    let (_, arity, _) = Selection::parse(&source.matching.selection_json)?;
    if !rustred::fits_storage(arity, N) {
        return Err(AppError::input("finite backend storage arity mismatch"));
    }
    let parsed = finite_scope::parse(
        source,
        arity,
        policy.max_total_rows,
        policy.max_source_bytes,
    )?;
    let mut load =
        RoutedCampaignRequest::new(source.matching.selection_json.clone(), String::new());
    load.owner_base = source.matching.owner_base.clone();
    load.workers = policy.workers;
    load.reduction_limits = source.matching.reduction_limits;
    if let Some(limits) = policy.prepared_reduction_aggregates {
        load.reduction_limits = limits.apply(load.reduction_limits);
    }
    load.trace_limits.max_input_targets = policy.windows.max_seed_points;
    load.trace_limits.max_unique_nodes = policy.per_batch_work.max_nodes;
    load.trace_limits.max_transport_calls = policy.per_batch_work.max_transport_calls;
    load.trace_limits.max_transport_operations = policy.per_batch_work.max_transport_operations;
    load.trace_limits.max_transport_endpoints = policy.per_batch_work.max_transport_endpoints;
    let prepared = PreparedExecution::<N>::prepare(&load, cancel, observer)?.ok_or_else(|| {
        AppError::new(
            AppErrorKind::Cancelled,
            "finite backend preparation cancelled",
        )
    })?;
    let mut scope = ScopeBinding {
        original_source: digest(&parsed.request_binding)?,
        rows: Vec::new(),
    };
    let mut sources = Vec::new();
    let mut cursors = Vec::new();
    for (ordinal, row) in parsed.rows.iter().enumerate() {
        check(cancel)?;
        let region = finite_scope::region(prepared.reducer(), source, row)?;
        let source_digest = parsed.sources[row.source]["blake3"]
            .as_str()
            .ok_or_else(|| AppError::internal_invariant("query source digest absent"))?;
        let origin = blake3::Hash::from(digest(&(
            1u32,
            row.source,
            source_digest,
            ordinal,
            &row.query.id,
            row.query.auxiliary,
            row.query.role_declared,
        ))?)
        .to_hex()
        .to_string();
        let exact_domain = digest(&(
            arity,
            region.support.to_vec(),
            &region.lower,
            &region.upper,
            region.rank,
            region.powers.max_positive_power,
            region.powers.min_power_difference,
            region.powers.max_power_difference,
        ))?;
        let windows = PrefixWindows::try_new(&origin, region, arity, policy.windows, cancel)
            .map_err(window_error)?;
        cursors.push(windows.initial_cursor());
        sources.push(windows);
        scope.rows.push(ScopeRow {
            origin,
            role: if row.query.auxiliary {
                ScopeRole::Auxiliary
            } else {
                ScopeRole::Required
            },
            exact_domain,
        });
    }
    check(cancel)?;
    Ok(Admitted {
        prepared,
        scope,
        sources,
        cursors,
        source_reduction: source.matching.reduction_limits,
        effective_reduction: load.reduction_limits,
    })
}
fn digest(value: &impl bincode::Encode) -> Result<[u8; 32], AppError> {
    bincode::encode_to_vec(value, bincode::config::standard())
        .map(|b| *blake3::hash(&b).as_bytes())
        .map_err(|e| AppError::serialization(e.to_string()))
}
fn stored_cursor<const N: usize>(
    source: &PrefixWindows<N>,
    cursor: &PrefixCursor,
    occurrences: u64,
) -> Result<CursorRecord, AppError> {
    if cursor.emitted() != &Integer::from(occurrences) {
        return Err(AppError::internal_invariant(
            "cursor/native occurrence count differs",
        ));
    }
    Ok(CursorRecord {
        source_policy: source.source_policy_binding(),
        completed_windows: cursor.windows(),
        input_occurrences: occurrences,
        encoded_cursor: cursor.encode().map_err(window_error)?,
    })
}
fn check(cancel: &AtomicBool) -> Result<(), AppError> {
    if cancel.load(Ordering::Acquire) {
        Err(AppError::new(
            AppErrorKind::Cancelled,
            "finite backend cancelled",
        ))
    } else {
        Ok(())
    }
}
fn window_error(error: super::windows::Error) -> AppError {
    use super::windows::ErrorKind as K;
    AppError::new(
        match error.kind {
            K::Resource => AppErrorKind::Limit,
            K::Cancelled => AppErrorKind::Cancelled,
            K::Cursor => AppErrorKind::Serialization,
            K::Native => AppErrorKind::Execution,
            _ => AppErrorKind::Input,
        },
        error.to_string(),
    )
}
fn storage_error(error: storage::StorageError) -> AppError {
    use storage::ErrorKind as K;
    AppError::new(
        match error.kind {
            K::Resource => AppErrorKind::Limit,
            K::Cancelled => AppErrorKind::Cancelled,
            K::Corrupt => AppErrorKind::Serialization,
            K::Input | K::Stale => AppErrorKind::Input,
            _ => AppErrorKind::Execution,
        },
        error.to_string(),
    )
}
