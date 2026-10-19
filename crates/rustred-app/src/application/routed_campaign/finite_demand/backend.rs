//! Private exact-window -> native seal -> atomic generated cursor lifecycle.
//! Private engine used by the public lifecycle and gated publication adapter;
//! this module alone grants neither CP6 adoption nor publication/export authority.
#[cfg(test)]
pub(crate) mod diagnostic;
mod errors;
mod group;
pub(crate) mod publication;
use super::{
    prepared::{PreparedExecution, PreparedGeneration},
    storage::{
        self, CursorRecord, GenerationState, RawCensus, ScopeBinding, ScopeRole, ScopeRow,
        StorageLimits, StoreWriter, ValidatedGeneration,
    },
    trace::ReductionAggregateOverrides,
    windows::{PrefixCursor, PrefixWindows, WindowLimits},
};
use crate::application::routed_campaign::{input::Selection, snapshot_json, walking::finite_scope};
use crate::{AppError, AppErrorKind, OwnerDomainWalkRequest, RoutedCampaignRequest};
use rustred::persistence::finite_work::{
    GeneratedWorkError, GeneratedWorkErrorKind, GeneratedWorkProgress, PreparedResidentWork,
    TrustedGeneratedResidentWork,
};
use rustred::solver::{
    CandidateEntryAdmission, CandidateRoutedDeltaExportLimits, CandidateRoutedTraceCensusLimits,
    CandidateRoutedWorkBudget, CandidateRoutedWorkerPool, FiniteRootAdmission,
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use symbolica::domains::integer::Integer;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Policy {
    pub workers: usize,
    /// Operational grouping of existing immutable windows, not a new partition.
    pub max_windows_per_batch: usize,
    pub reuse_worker_pool: bool,
    pub parallel_window_preparation: bool,
    pub windows: WindowLimits,
    pub per_batch_work: CandidateRoutedWorkBudget,
    pub prepared_reduction_aggregates: Option<ReductionAggregateOverrides>,
    pub operation_limits: Option<super::lifecycle::operation_limits::OperationLimits>,
    pub census: CandidateRoutedTraceCensusLimits,
    pub export: CandidateRoutedDeltaExportLimits,
    pub storage: StorageLimits,
    /// Optional conservative resident Work payload grant; zero keeps paging.
    /// Includes reserved decode, actual directory and delta coexistence. The
    /// paged cache is disabled while this complete resident index is active.
    /// This is not a process RSS limit or an allocator-exact byte guarantee.
    pub max_resident_work_bytes: usize,
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
    pub windows_count: usize,
    pub last_row: usize,
    pub last_window: u64,
    pub inputs: usize,
    pub new_work: usize,
    pub baseline_hits: usize,
    pub cleanup_warning: Option<String>,
    pub timings: GroupTimings,
    pub resident: Value,
}

/// Cheap wall-clock boundaries, not summed worker time or proof of progress.
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub(crate) struct GroupTimings {
    pub planning_seconds: f64,
    pub materialization_seconds: f64,
    pub binding_seconds: f64,
    pub trace_seconds: f64,
    pub staging_seconds: f64,
    pub commit_seconds: f64,
    pub cleanup_seconds: f64,
    pub total_seconds: f64,
}
#[derive(Clone, Debug)]
pub(crate) struct Step {
    pub window: Option<CompletedWindow>,
    pub progress: Progress,
}

/// No operation accepts caller-supplied starts, seals or successor bytes.
/// An error permanently stops this object; cold reopen determines CURRENT.
pub(crate) struct Session<const N: usize> {
    source: OwnerDomainWalkRequest,
    prepared: PreparedExecution<N>,
    authenticated: PreparedGeneration<N>,
    resident: Option<TrustedGeneratedResidentWork>,
    scope: ScopeBinding,
    sources: Vec<PrefixWindows<N>>,
    cursors: Vec<PrefixCursor>,
    writer: StoreWriter,
    policy: Policy,
    path: PathBuf,
    stopped: bool,
    source_reduction: rustred::reduction::ReductionLimits,
    effective_reduction: rustred::reduction::ReductionLimits,
    worker_pool: Option<CandidateRoutedWorkerPool>,
    work_progress: Option<GeneratedWorkProgress>,
}

struct Admitted<const N: usize> {
    source: OwnerDomainWalkRequest,
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
        let authenticated = admitted.prepared.authenticate_generation(
            writer.current().map_err(storage_error)?.unwrap().clone(),
            &admitted.scope,
            cancel,
        )?;
        let resident = Self::hydrate_resident(&authenticated, policy, cancel, observer)?;
        Ok(Self::assemble(
            admitted,
            writer,
            authenticated,
            policy,
            path,
            resident,
        ))
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
        // Authenticate current small metadata once; raw pages stay lazy until
        // exact census iteration/publication, not historical reseeding.
        let mut authenticated =
            admitted
                .prepared
                .authenticate_generation(previous.clone(), old, cancel)?;
        // Hydrate BEFORE any scope append: corrupt/missing/cancelled reads
        // cannot leave a newly installed metadata generation behind.
        let mut resident = Self::hydrate_resident(&authenticated, policy, cancel, observer)?;
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
        writer
            .share_captured_current(&previous)
            .map_err(storage_error)?;
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
            let had_resident = resident.is_some();
            let predecessor_reserved = resident
                .as_ref()
                .map(TrustedGeneratedResidentWork::statistics)
                .transpose()
                .map_err(|error| storage_error(error.into()))?
                .map_or(0, |statistics| statistics.reserved_coexisting_bytes);
            let mut staged = resident
                .take()
                .map(|index| {
                    index
                        .prepare_metadata_only(previous.digest())
                        .map_err(|error| storage_error(error.into()))
                })
                .transpose()?;
            writer
                .commit_generated_with_directory_accounting(
                    Vec::new(),
                    state,
                    cancel,
                    &mut |successor_directory| {
                        refresh_resident_directory(
                            &mut staged,
                            predecessor_reserved,
                            policy.storage.page_scratch_bytes()?,
                            successor_directory,
                        )
                    },
                )
                .map_err(storage_error)?;
            authenticated = admitted.prepared.authenticate_generation(
                writer.current().map_err(storage_error)?.unwrap().clone(),
                &admitted.scope,
                cancel,
            )?;
            resident =
                staged.map(|index| index.after_durable_commit(authenticated.generation().digest()));
            if had_resident && resident.is_none() {
                observer(
                    json!({"event":"finite_backend_resident_fallback","status":"declined",
                    "requested_bytes":policy.max_resident_work_bytes,
                    "retained_payload_estimate":0,"new_work":0,
                    "generation":previous.generation()}),
                );
            }
            #[cfg(test)]
            observer(json!({"event":"finite_backend_resident_metadata_committed",
                "resident":resident_statistics(resident.as_ref())}));
        }
        Ok(Self::assemble(
            admitted,
            writer,
            authenticated,
            policy,
            path,
            resident,
        ))
    }

    fn hydrate_resident(
        authenticated: &PreparedGeneration<N>,
        policy: Policy,
        cancel: &AtomicBool,
        observer: &impl Fn(Value),
    ) -> Result<Option<TrustedGeneratedResidentWork>, AppError> {
        // One complete hydration per session; no partial index is retained.
        // Avoid duplicating decoded pages while making the resident copy.
        let resident = if policy.max_resident_work_bytes == 0 {
            None
        } else {
            let reader = authenticated
                .generation()
                .reader(StorageLimits {
                    max_cached_pages: 0,
                    max_cached_page_bytes: 0,
                    ..policy.storage
                })
                .map_err(storage_error)?;
            let reserved = [
                policy.storage.page_scratch_bytes().map_err(storage_error)?,
                authenticated
                    .generation()
                    .metadata_payload_bytes()
                    .map_err(storage_error)?,
            ]
            .into_iter()
            .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
            .ok_or_else(|| AppError::limit("resident Work coexistence estimate overflow"))?;
            TrustedGeneratedResidentWork::hydrate(
                reader.directory(),
                &reader,
                policy.max_resident_work_bytes,
                reserved,
                cancel,
            )
            .map_err(|error| storage_error(error.into()))?
        };
        observer(json!({"event":"finite_backend_resident_hydrated",
            "requested_bytes":policy.max_resident_work_bytes,
            "status":if resident.is_some() {"active"} else if policy.max_resident_work_bytes == 0 {"disabled"} else {"declined"},
            "resident":resident_statistics(resident.as_ref())}));
        check(cancel)?;
        Ok(resident)
    }

    fn assemble(
        admitted: Admitted<N>,
        writer: StoreWriter,
        authenticated: PreparedGeneration<N>,
        policy: Policy,
        path: &Path,
        resident: Option<TrustedGeneratedResidentWork>,
    ) -> Self {
        Self {
            source: admitted.source,
            prepared: admitted.prepared,
            authenticated,
            resident,
            scope: admitted.scope,
            sources: admitted.sources,
            cursors: admitted.cursors,
            writer,
            policy,
            path: path.to_path_buf(),
            stopped: false,
            source_reduction: admitted.source_reduction,
            effective_reduction: admitted.effective_reduction,
            worker_pool: None,
            work_progress: None,
        }
    }

    pub(crate) fn set_work_progress(&mut self, progress: Option<GeneratedWorkProgress>) {
        self.writer.set_work_progress(progress.clone());
        self.work_progress = progress;
    }

    pub(crate) fn generation(&self) -> Result<ValidatedGeneration, AppError> {
        self.writer
            .current()
            .map_err(storage_error)?
            .cloned()
            .ok_or_else(|| AppError::internal_invariant("pending finite generation absent"))
    }
    pub(crate) fn total_input_occurrences(&self) -> Integer {
        self.sources.iter().fold(Integer::from(0), |total, source| {
            total + source.total_count()
        })
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
        // Capture-only publication and already-complete resumes never start
        // idle workers. The pool's scoped borrows end before delta adoption.
        if self.policy.reuse_worker_pool && self.worker_pool.is_none() {
            let started = Instant::now();
            self.worker_pool = Some(
                CandidateRoutedWorkerPool::try_new(self.policy.workers)
                    .map_err(|error| AppError::input(error.to_string()))?,
            );
            observer(
                json!({"event":"finite_worker_pool_ready","workers":self.policy.workers,
                "seconds":started.elapsed().as_secs_f64()}),
            );
        }
        let group_started = Instant::now();
        if let Some(progress) = &self.work_progress {
            progress.clear();
        }
        observer(json!({"event":"finite_backend_group_planning","row":index,
            "window":self.cursors[index].windows()}));
        let group = group::WindowGroup::plan(
            &self.sources,
            &self.cursors,
            index,
            self.policy,
            if self.policy.parallel_window_preparation {
                self.worker_pool.as_ref()
            } else {
                None
            },
            observer,
            cancel,
        )?;
        let group_metadata = group.metadata();
        let window_ordinal = group.first().ordinal;
        let planning_seconds = group.planning_seconds;
        let materialization_seconds = group.materialization_seconds;
        observer(json!({"event":"finite_backend_group_planned","row":index,
            "window":window_ordinal,"group":group_metadata}));
        let binding_started = Instant::now();
        let previous = self.generation()?;
        let reader_limits = if self.resident.is_some() {
            StorageLimits {
                max_cached_pages: 0,
                max_cached_page_bytes: 0,
                ..self.policy.storage
            }
        } else {
            self.policy.storage
        };
        let reader = previous.reader(reader_limits).map_err(storage_error)?;
        let baseline = self
            .authenticated
            .bind(
                &reader,
                self.policy.per_batch_work,
                self.policy.census,
                self.policy.export,
                cancel,
            )?
            .with_work_progress(self.work_progress.as_ref());
        let admission = FiniteRootAdmission::try_new(
            group.windows.iter().map(|w| w.window.region().clone()),
            self.policy.max_windows_per_batch,
        )
        .map_err(|e| AppError::input(e.to_string()))?;
        let binding_seconds = binding_started.elapsed().as_secs_f64();
        observer(json!({"event":"finite_backend_trace_started","row":index,
            "window":window_ordinal,"group":group_metadata}));
        let native_started = Instant::now();
        let targets = group
            .windows
            .iter()
            .flat_map(|w| w.window.targets().iter().cloned());
        let native_observer = |s: &rustred::solver::CandidateRoutedCampaignSnapshot<N>| {
            observer(json!({"event":"finite_backend_native","row":index,
                "window":window_ordinal,"group":group_metadata,"native":snapshot_json(s)}))
        };
        let delta_result = match self.worker_pool.as_ref() {
            Some(pool) => baseline.trace_batch_with_resident_pool(
                self.resident.as_ref(),
                targets,
                CandidateEntryAdmission::ExplicitFinite(&admission),
                pool,
                cancel,
                native_observer,
            ),
            None => baseline.trace_batch_with_resident(
                self.resident.as_ref(),
                targets,
                CandidateEntryAdmission::ExplicitFinite(&admission),
                self.policy.workers,
                cancel,
                native_observer,
            ),
        }
        .map_err(errors::completed);
        let trace_seconds = native_started.elapsed().as_secs_f64();
        let staging_started = Instant::now();
        #[cfg(test)]
        observer(
            json!({"event":"finite_backend_native_call_finished","row":index,
            "seconds":native_started.elapsed().as_secs_f64(),"succeeded":delta_result.is_ok(),
            "reader":diagnostic::reader_statistics(&reader),
            "resident_requested_bytes":self.policy.max_resident_work_bytes,
            "resident":resident_statistics(self.resident.as_ref())}),
        );
        let delta = delta_result?;
        drop(baseline);
        drop(admission);
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
            || snapshot.input_targets != group.inputs
            || snapshot.requested_targets != group.distinct_inputs
            || delta.report().complete_census().is_none()
            || u64::try_from(snapshot.rule_applications).ok() != Some(multiplicity)
        {
            return Err(AppError::internal_invariant(
                "exact finite window/native seal binding differs",
            ));
        }
        let mut completed = CompletedWindow {
            row: index,
            windows_count: group.windows.len(),
            last_row: group.last().row,
            last_window: group.last().ordinal,
            inputs: group.inputs,
            new_work: delta.work().len(),
            baseline_hits: snapshot.completed_baseline_hits,
            cleanup_warning: None,
            timings: GroupTimings {
                planning_seconds,
                materialization_seconds,
                binding_seconds,
                trace_seconds,
                ..GroupTimings::default()
            },
            resident: Value::Null,
        };
        let mut cursors = previous.state().cursors.clone();
        // All cursor allocation/encoding precedes publication. Adoption after
        // CURRENT is durable consists only of moves into existing slots.
        let mut successors = Vec::new();
        successors
            .try_reserve_exact(group.windows.len())
            .map_err(|_| AppError::limit("finite group successor allocation"))?;
        for proposed in &group.windows {
            let row = proposed.row;
            let occurrences = cursors[row]
                .input_occurrences
                .checked_add(
                    u64::try_from(proposed.window.targets().len())
                        .map_err(|_| AppError::limit("window input count exceeds u64"))?,
                )
                .ok_or_else(|| AppError::limit("cursor occurrence overflow"))?;
            cursors[row] =
                stored_cursor(&self.sources[row], proposed.window.successor(), occurrences)?;
            if let Some((last_row, successor)) = successors.last_mut()
                && *last_row == row
            {
                *successor = proposed.window.successor().clone();
            } else {
                successors.push((row, proposed.window.successor().clone()));
            }
        }
        observer(
            json!({"event":"finite_backend_before_commit","row":index,"window":window_ordinal,"group":group_metadata,
            "inputs":completed.inputs,"new_work":completed.new_work,"baseline_hits":completed.baseline_hits}),
        );
        check(cancel)?;
        // Take ownership before staging: any failure stops this Session and
        // drops the optimization. The current set is never cloned per window.
        let had_resident = self.resident.is_some();
        let predecessor_reserved = self
            .resident
            .as_ref()
            .map(TrustedGeneratedResidentWork::statistics)
            .transpose()
            .map_err(|error| storage_error(error.into()))?
            .map_or(0, |statistics| statistics.reserved_coexisting_bytes);
        let mut resident = self
            .resident
            .take()
            .map(|resident| {
                resident
                    .prepare_delta_with_progress(&delta, cancel, self.work_progress.as_ref())
                    .map_err(|error| storage_error(error.into()))
            })
            .transpose()?
            .flatten();
        #[cfg(test)]
        observer(
            json!({"event":"finite_backend_resident_staged","active":resident.is_some(),
            "records":resident.as_ref().map(|_| completed.new_work)}),
        );
        completed.timings.staging_seconds = staging_started.elapsed().as_secs_f64();
        observer(json!({"event":"finite_backend_commit_started","row":index,
            "window":window_ordinal,"group":group_metadata}));
        let commit_started = Instant::now();
        let outcome = self.prepared.commit_delta_with_directory_accounting(
            &self.authenticated,
            &mut self.writer,
            delta,
            self.scope.clone(),
            cursors,
            cancel,
            &mut |successor_directory| {
                refresh_resident_directory(
                    &mut resident,
                    predecessor_reserved,
                    self.policy.storage.page_scratch_bytes()?,
                    successor_directory,
                )
            },
        )?;
        // No fallible work or observer lies between durable commit and adoption.
        for (row, successor) in successors {
            self.cursors[row] = successor;
        }
        self.authenticated = outcome.generation;
        self.resident = resident.map(|resident| {
            resident.after_durable_commit_with_progress(
                self.authenticated.generation().digest(),
                self.work_progress.as_ref(),
            )
        });
        completed.cleanup_warning = outcome.cleanup_warning;
        completed.timings.commit_seconds = commit_started.elapsed().as_secs_f64();
        completed.resident = resident_statistics(self.resident.as_ref());
        if had_resident && self.resident.is_none() {
            observer(
                json!({"event":"finite_backend_resident_fallback","status":"declined",
                "requested_bytes":self.policy.max_resident_work_bytes,
                "retained_payload_estimate":0,"new_work":completed.new_work,
                "generation":previous.generation()}),
            );
        }
        #[cfg(test)]
        observer(json!({"event":"finite_backend_commit_finished","row":index,
            "seconds":commit_started.elapsed().as_secs_f64(),
            "generation":self.authenticated.generation().generation(),
            "resident":resident_statistics(self.resident.as_ref()),
            "resident_staged_records":self.resident.as_ref().map(|_| completed.new_work)}));
        observer(json!({"event":"finite_backend_group_cleanup","row":index,
            "window":window_ordinal,"group":group_metadata}));
        let cleanup_started = Instant::now();
        let mut cleanup_progress = GeneratedWorkProgress::begin(
            self.work_progress.as_ref(),
            "cleanup_group",
            None,
            "objects",
        );
        drop(reader);
        drop(previous);
        drop(group);
        cleanup_progress.finish(3);
        let progress = self.progress()?;
        completed.timings.cleanup_seconds = cleanup_started.elapsed().as_secs_f64();
        completed.timings.total_seconds = group_started.elapsed().as_secs_f64();
        Ok(Step {
            window: Some(completed),
            progress,
        })
    }
}

fn refresh_resident_directory(
    resident: &mut Option<PreparedResidentWork>,
    predecessor_reserved: usize,
    transient_reserved: usize,
    successor: storage::MetadataMemory,
) -> Result<(), storage::StorageError> {
    let Some(staged) = resident.take() else {
        return Ok(());
    };
    let add_metadata = |reserved: usize, transient: usize| {
        reserved
            .checked_add(successor.retained_bytes)
            .and_then(|n| n.checked_add(transient))
            .ok_or_else(|| {
                GeneratedWorkError::new(
                    GeneratedWorkErrorKind::Resource,
                    "resident Work coexistence estimate overflow",
                )
            })
    };
    // The reader and authenticated predecessor share one Arc allocation. The
    // writer builds a distinct successor while that predecessor remains live.
    *resident = staged.refresh_coexisting_bytes(
        add_metadata(predecessor_reserved, successor.transient_bytes)?,
        add_metadata(transient_reserved, 0)?,
    )?;
    Ok(())
}

fn resident_statistics(resident: Option<&TrustedGeneratedResidentWork>) -> Value {
    match resident.map(TrustedGeneratedResidentWork::statistics) {
        None => Value::Null,
        Some(Ok(s)) => json!({"records":s.records,"capacity":s.capacity,
            "retained_payload_estimate":s.retained_payload_estimate,
            "reserved_coexisting_bytes":s.reserved_coexisting_bytes,
            "max_live_payload_bytes":s.max_live_payload_bytes}),
        Some(Err(error)) => json!({"statistics_error":error.to_string()}),
    }
}

fn admit<const N: usize>(
    source: &OwnerDomainWalkRequest,
    policy: Policy,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<Admitted<N>, AppError> {
    check(cancel)?;
    if policy.workers == 0 || policy.max_windows_per_batch == 0 {
        return Err(AppError::input(
            "finite backend workers and windows per batch must be positive",
        ));
    }
    if policy.parallel_window_preparation && !policy.reuse_worker_pool {
        return Err(AppError::input(
            "parallel window preparation requires the persistent worker pool",
        ));
    }
    rustred::campaign::ParallelExecution::preflight_requested_core_budget(policy.workers)
        .map_err(|e| AppError::input(e.to_string()))?;
    let (_, arity, _) =
        Selection::parse_with_byte_limit(&source.matching.selection_json, policy.max_source_bytes)?;
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
    load.trace_limits.max_input_targets =
        group::checked_group_grant(policy.windows.max_seed_points, policy.max_windows_per_batch)?;
    group::checked_group_grant(policy.windows.max_seed_bytes, policy.max_windows_per_batch)?;
    load.trace_limits.max_unique_nodes = policy.per_batch_work.max_nodes;
    load.trace_limits.max_transport_calls = policy.per_batch_work.max_transport_calls;
    load.trace_limits.max_transport_operations = policy.per_batch_work.max_transport_operations;
    load.trace_limits.max_transport_endpoints = policy.per_batch_work.max_transport_endpoints;
    let prepared = PreparedExecution::<N>::prepare_with_operation_limits(
        &load,
        policy.operation_limits,
        policy.max_source_bytes,
        cancel,
        observer,
    )?
    .ok_or_else(|| {
        AppError::new(
            AppErrorKind::Cancelled,
            "finite backend preparation cancelled",
        )
    })?;
    let mut effective = load.clone();
    if let Some(limits) = policy.operation_limits {
        limits.apply(&mut effective);
    }
    observer(
        json!({"event":"finite_execution_limits","operation":"finite_demand",
        "operation_limits":policy.operation_limits,
        "effective_reduction":format!("{:?}",effective.reduction_limits),
        "effective_transport":format!("{:?}",effective.trace_limits.expansion),
        "identity_policy":"original source identity; resource-only execution overrides",
        "source_identity_unchanged":true}),
    );
    let mut scope = ScopeBinding {
        original_source: original_source_identity(&parsed.request_binding)?,
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
        let origin = origin_identity(
            row.source,
            source_digest,
            ordinal,
            &row.query.id,
            row.query.auxiliary,
            row.query.role_declared,
        )?;
        let exact_domain = domain_identity(arity, &region)?;
        let planning = policy
            .operation_limits
            .map_or_else(|| policy.windows.into(), |limits| limits.planning.into());
        let windows = PrefixWindows::try_new_with_planning_limits(
            &origin,
            region,
            arity,
            policy.windows,
            planning,
            cancel,
        )
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
        source: source.clone(),
        prepared,
        scope,
        sources,
        cursors,
        source_reduction: source.matching.reduction_limits,
        effective_reduction: effective.reduction_limits,
    })
}
pub(super) fn origin_identity(
    source: usize,
    source_digest: &str,
    ordinal: usize,
    id: &str,
    auxiliary: bool,
    role_declared: bool,
) -> Result<String, AppError> {
    Ok(blake3::Hash::from(digest(&(
        1u32,
        source,
        source_digest,
        ordinal,
        id,
        auxiliary,
        role_declared,
    ))?)
    .to_hex()
    .to_string())
}
fn domain_identity<const N: usize>(
    arity: usize,
    region: &rustred::solver::RootRegionInput<N>,
) -> Result<[u8; 32], AppError> {
    digest(&(
        arity,
        region.support.to_vec(),
        &region.lower,
        &region.upper,
        region.rank,
        region.powers.max_positive_power,
        region.powers.min_power_difference,
        region.powers.max_power_difference,
    ))
}
/// Preserve the persisted native encoding of the original request binding.
/// This is a digest of its encoded string, not the decoded hexadecimal digest.
pub(in crate::application::routed_campaign) fn original_source_identity(
    request_binding: &str,
) -> Result<[u8; 32], AppError> {
    digest(&request_binding)
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
pub(super) fn window_error(error: super::windows::Error) -> AppError {
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
pub(super) fn storage_error(error: storage::StorageError) -> AppError {
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
