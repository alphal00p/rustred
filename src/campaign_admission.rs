//! Atomic runtime resource admission for bounded campaign waves.
//!
//! [`CampaignWavePlanner`](crate::CampaignWavePlanner) remains a stateless,
//! deterministic selector. This module is the distinct runtime authority: one
//! non-cloneable coordinator reserves a complete selected wave in one locked
//! transition, while move-only guards keep cores and estimated memory charged
//! until the corresponding owners have actually been dropped or transferred.
//! No algebra or checkpoint coordinator is implemented here. The controller
//! owns the invocation-wide pool and provides move-owned wave/transform
//! execution primitives; production callbacks still belong to the higher
//! campaign coordinator.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use crate::{
    CampaignBaselineMemory, CampaignBytes, CampaignEstimatorRevision, CampaignExecutionWidthPlan,
    CampaignJobKey, CampaignMemoryEstimate, CampaignResourceError, CampaignResourcePolicy,
    CampaignResourceWavePlan, CampaignTaskMemoryEnvelope, CampaignTaskResourceEstimate,
    CampaignWavePlanner, CampaignWorkKey, ParallelExecution, ParallelExecutionError,
};

pub const CAMPAIGN_ADMISSION_V1_SCHEMA: &str = "rustred.campaign-admission.v1";

#[derive(Clone)]
pub struct CampaignAdmissionSnapshot {
    authority: Weak<CampaignAdmissionShared>,
    generation: NonZeroU64,
    policy: CampaignResourcePolicy,
}

impl CampaignAdmissionSnapshot {
    pub const fn generation(&self) -> u64 {
        self.generation.get()
    }

    pub const fn policy(&self) -> CampaignResourcePolicy {
        self.policy
    }

    fn belongs_to(&self, shared: &Arc<CampaignAdmissionShared>) -> bool {
        Weak::ptr_eq(&self.authority, &Arc::downgrade(shared))
    }
}

impl fmt::Debug for CampaignAdmissionSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignAdmissionSnapshot")
            .field("generation", &self.generation)
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignAdmissionUsage {
    generation: NonZeroU64,
    core_capacity: usize,
    in_flight_cores: usize,
    max_memory: CampaignBytes,
    baseline: CampaignBaselineMemory,
    in_flight_peak_additional_memory: CampaignBytes,
    total_charged_memory: CampaignBytes,
}

impl CampaignAdmissionUsage {
    pub const fn generation(self) -> u64 {
        self.generation.get()
    }

    pub const fn core_capacity(self) -> usize {
        self.core_capacity
    }

    pub const fn in_flight_cores(self) -> usize {
        self.in_flight_cores
    }

    pub const fn available_cores(self) -> usize {
        self.core_capacity - self.in_flight_cores
    }

    pub const fn max_memory(self) -> CampaignBytes {
        self.max_memory
    }

    pub const fn baseline(self) -> CampaignBaselineMemory {
        self.baseline
    }

    pub const fn in_flight_peak_additional_memory(self) -> CampaignBytes {
        self.in_flight_peak_additional_memory
    }

    pub const fn total_charged_memory(self) -> CampaignBytes {
        self.total_charged_memory
    }
}

/// The sole runtime admission authority for one campaign invocation.
///
/// This type is intentionally not `Clone`. Wave acquisition requires
/// `&mut self`; leases sent to workers retain only the private accounting
/// authority needed to release their own charges.
pub struct CampaignAdmissionController {
    schema: &'static str,
    execution: ParallelExecution,
    execution_width_plan: Option<CampaignExecutionWidthPlan>,
    shared: Arc<CampaignAdmissionShared>,
}

impl fmt::Debug for CampaignAdmissionController {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let usage = self.try_usage();
        formatter
            .debug_struct("CampaignAdmissionController")
            .field("schema", &self.schema)
            .field("execution", &self.execution)
            .field("execution_width_plan", &self.execution_width_plan)
            .field("usage", &usage)
            .finish_non_exhaustive()
    }
}

struct CampaignAdmissionShared {
    state: Mutex<CampaignAdmissionState>,
}

#[derive(Clone, Copy, Debug)]
struct CampaignAdmissionState {
    generation: NonZeroU64,
    generation_exhausted: bool,
    invariant_broken: bool,
    estimator_revision: CampaignEstimatorRevision,
    core_capacity: usize,
    max_memory: CampaignBytes,
    execution_fixed_memory: CampaignBytes,
    fixed_and_shared: CampaignBytes,
    reserved_fixed_components: CampaignBytes,
    staged_results: CampaignBytes,
    resident_retained: CampaignBytes,
    in_flight_cores: usize,
    in_flight_peak_additional_memory: CampaignBytes,
    next_resident_token: u64,
}

impl CampaignAdmissionController {
    /// Crate-internal constructor for focused admission tests and lower-layer
    /// composition. Public campaign bootstrap must consume a checked width
    /// plan so physical policy cannot diverge from pre-pool admission.
    pub(crate) fn try_new(
        execution: ParallelExecution,
        estimator_revision: CampaignEstimatorRevision,
        max_memory: CampaignBytes,
        fixed_and_shared: CampaignBytes,
        staged_results: CampaignBytes,
    ) -> Result<Self, CampaignAdmissionError> {
        Self::try_new_with_execution_policy(
            execution,
            None,
            estimator_revision,
            max_memory,
            CampaignBytes::ZERO,
            fixed_and_shared,
            staged_results,
        )
    }

    /// Consume a checked pre-pool width plan, construct its exact bounded
    /// executor, and derive every admission capacity from the retained plan.
    ///
    /// V1 starts before any heavyweight lane is hydrated, so a byte-only
    /// nonzero hydrated baseline is rejected: admitting it without the
    /// corresponding move-owned resident tokens would create unreleasable
    /// accounting. Staged bytes may already belong to the campaign workspace
    /// and remain a separately mutable baseline category.
    pub fn try_from_execution_width_plan(
        plan: CampaignExecutionWidthPlan,
    ) -> Result<Self, CampaignAdmissionError> {
        let baseline = plan.admission_baseline();
        if baseline.hydrated_retained() != CampaignBytes::ZERO {
            return Err(
                CampaignAdmissionError::ExecutionWidthPlanHasHydratedRetainedMemory {
                    bytes: baseline.hydrated_retained(),
                },
            );
        }
        let estimator_revision = plan.estimator_revision();
        let max_memory = plan.operational_memory_limit();
        let (plan, execution) = plan
            .try_into_plan_and_parallel_execution()
            .map_err(CampaignAdmissionError::ParallelExecution)?;
        debug_assert_eq!(execution.n_cores(), plan.effective_width());
        debug_assert_eq!(execution.worker_thread_count(), plan.worker_thread_count());
        Self::try_new_with_execution_policy(
            execution,
            Some(plan),
            estimator_revision,
            max_memory,
            baseline.fixed_and_shared(),
            CampaignBytes::ZERO,
            baseline.staged_results(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn try_new_with_execution_policy(
        execution: ParallelExecution,
        execution_width_plan: Option<CampaignExecutionWidthPlan>,
        estimator_revision: CampaignEstimatorRevision,
        max_memory: CampaignBytes,
        execution_fixed_memory: CampaignBytes,
        fixed_and_shared: CampaignBytes,
        staged_results: CampaignBytes,
    ) -> Result<Self, CampaignAdmissionError> {
        let core_capacity = execution.n_cores();
        let total_fixed = bytes_add(
            execution_fixed_memory,
            fixed_and_shared,
            "campaign execution plus configured fixed memory",
        )?;
        let baseline =
            CampaignBaselineMemory::try_new(total_fixed, CampaignBytes::ZERO, staged_results)?;
        CampaignResourcePolicy::try_new(estimator_revision, core_capacity, max_memory, baseline)?;
        Ok(Self {
            schema: CAMPAIGN_ADMISSION_V1_SCHEMA,
            execution,
            execution_width_plan,
            shared: Arc::new(CampaignAdmissionShared {
                state: Mutex::new(CampaignAdmissionState {
                    generation: NonZeroU64::MIN,
                    generation_exhausted: false,
                    invariant_broken: false,
                    estimator_revision,
                    core_capacity,
                    max_memory,
                    execution_fixed_memory,
                    fixed_and_shared,
                    reserved_fixed_components: CampaignBytes::ZERO,
                    staged_results,
                    resident_retained: CampaignBytes::ZERO,
                    in_flight_cores: 0,
                    in_flight_peak_additional_memory: CampaignBytes::ZERO,
                    next_resident_token: 1,
                }),
            }),
        })
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn execution_width_plan(&self) -> Option<&CampaignExecutionWidthPlan> {
        self.execution_width_plan.as_ref()
    }

    pub fn worker_thread_count(&self) -> usize {
        self.execution.worker_thread_count()
    }

    pub fn is_parallel(&self) -> bool {
        self.execution.is_parallel()
    }

    pub fn try_usage(&self) -> Result<CampaignAdmissionUsage, CampaignAdmissionError> {
        self.shared.lock().try_usage()
    }

    /// Settle one already admitted wave on the invocation-wide bounded pool.
    ///
    /// Every move-only reservation is transferred to exactly one worker. The
    /// callback borrows the reservation as a read-only task context, so only
    /// this executor can bind a successful retained output or keep a failure
    /// payload paired with its charge. Results preserve the wave's stable work
    /// order regardless of worker completion order. All work units settle
    /// before this method returns, which provides the deterministic wave
    /// barrier required by campaign planning.
    ///
    /// A typed build failure and a recovered panic both retain the complete task
    /// charge until their payload has been dropped. This matters because an
    /// error payload may itself own task-local Symbolica data. No callback runs
    /// while the admission accounting mutex is held.
    pub fn execute_reserved_wave<Output, BuildError, Build>(
        &mut self,
        wave: CampaignWaveReservation,
        build: Build,
    ) -> Result<Vec<CampaignTaskExecution<Output, BuildError>>, CampaignWaveExecutionAdmissionFailure>
    where
        Output: Send,
        BuildError: Send,
        Build:
            for<'task> Fn(CampaignTaskContext<'task>) -> Result<Output, BuildError> + Send + Sync,
    {
        if !wave.belongs_to(&self.shared) {
            return Err(CampaignWaveExecutionAdmissionFailure {
                error: CampaignAdmissionError::ForeignWaveReservation,
                wave,
            });
        }
        if let Some(task) = wave.tasks().iter().find(|task| task.cores() != 1) {
            return Err(CampaignWaveExecutionAdmissionFailure {
                error: CampaignAdmissionError::UnsupportedExecutorCoreWidth {
                    work: task.work().clone(),
                    requested: task.cores(),
                },
                wave,
            });
        }
        Ok(self
            .execution
            .map_owned_ordered(wave.into_tasks(), |reservation| {
                let context = CampaignTaskContext {
                    reservation: &reservation,
                };
                match catch_unwind(AssertUnwindSafe(|| build(context))) {
                    Ok(Ok(output)) => CampaignTaskExecution::Built(reservation.bind(output)),
                    Ok(Err(error)) => {
                        CampaignTaskExecution::Failed(CampaignTaskFailure::new(error, reservation))
                    }
                    Err(payload) => CampaignTaskExecution::Panicked(CampaignTaskPanic::new(
                        payload,
                        reservation,
                    )),
                }
            }))
    }

    /// Execute stable, already-paired resident transformations on the bounded
    /// invocation-wide pool.
    ///
    /// Unlike [`Self::execute_reserved_wave`], this path transfers the complete
    /// predecessor owner into the callback. It is the campaign seam for a live
    /// exact session: staging may clone a Symbolica reducer, but the resulting
    /// transaction still needs the predecessor session's private authorities in
    /// order to install the successor. On a recoverable failure the generic
    /// callback supplies the owner to keep charged in
    /// [`CampaignResidentTransformBuildFailure`]. This low-level public seam is
    /// cooperative: it cannot prove object identity for an arbitrary `T`.
    /// RustRed's crate-owned exact-session adapters are responsible for
    /// returning the same lineage; a panic destroys that worker's owner and
    /// leaves recovery to a durable campaign checkpoint.
    ///
    /// Batch preflight validates current accounting, but does not reserve the
    /// remaining `u64` accounting-generation namespace. If that namespace is
    /// exhausted while canonical commits settle, later ready items return
    /// charged [`CampaignResidentTransformExecution::CommitFailed`] values and
    /// the campaign is terminal until checkpoint recovery. This does not make
    /// resource capacity available early.
    pub fn execute_resident_transforms_ordered<Predecessor, Successor, BuildError, Transform>(
        &mut self,
        mut tasks: Vec<CampaignResidentTransformTask<Predecessor>>,
        transform: Transform,
    ) -> Result<
        Vec<CampaignResidentTransformExecution<Predecessor, Successor, BuildError>>,
        CampaignResidentTransformBatchAdmissionFailure<Predecessor>,
    >
    where
        Predecessor: Send,
        Successor: Send,
        BuildError: Send,
        Transform: for<'task> Fn(
                CampaignTaskContext<'task>,
                Predecessor,
            ) -> Result<
                Successor,
                CampaignResidentTransformBuildFailure<Predecessor, BuildError>,
            > + Send
            + Sync,
    {
        tasks.sort_unstable_by(|left, right| {
            left.reservation().work().cmp(right.reservation().work())
        });
        if let Some(task) = tasks.iter().find(|task| !task.belongs_to(&self.shared)) {
            return Err(CampaignResidentTransformBatchAdmissionFailure {
                error: CampaignAdmissionError::ForeignTaskReservation {
                    work: task.reservation().work().clone(),
                },
                tasks,
            });
        }
        if let Some(task) = tasks.iter().find(|task| task.reservation().cores() != 1) {
            return Err(CampaignResidentTransformBatchAdmissionFailure {
                error: CampaignAdmissionError::UnsupportedExecutorCoreWidth {
                    work: task.reservation().work().clone(),
                    requested: task.reservation().cores(),
                },
                tasks,
            });
        }
        if let Some(error) = tasks.iter().find_map(|task| task.preflight().err()) {
            return Err(CampaignResidentTransformBatchAdmissionFailure { error, tasks });
        }

        let prepared = self.execution.map_owned_ordered(tasks, |task| {
            let (reservation, predecessor) = task.into_parts();
            let (predecessor_owner, predecessor_charge) = predecessor.split_owner_charge();
            let context = CampaignTaskContext {
                reservation: &reservation,
            };
            match catch_unwind(AssertUnwindSafe(|| transform(context, predecessor_owner))) {
                Ok(Ok(successor)) => CampaignResidentTransformPrepared::Ready {
                    admitted: reservation.bind(successor),
                    predecessor_charge,
                },
                Ok(Err(failure)) => {
                    let (predecessor_owner, error) = failure.into_parts();
                    let predecessor = predecessor_charge.restore_owner(predecessor_owner);
                    CampaignResidentTransformPrepared::BuildFailed(
                        CampaignResidentTransformFailure {
                            error: Some(error),
                            task: Some(CampaignResidentTransformTask {
                                reservation: Some(reservation),
                                predecessor: Some(predecessor),
                            }),
                        },
                    )
                }
                Err(payload) => {
                    CampaignResidentTransformPrepared::Panicked(CampaignResidentTransformPanic {
                        payload: Some(payload),
                        predecessor_charge: Some(predecessor_charge),
                        reservation: Some(reservation),
                    })
                }
            }
        });

        // Workers finish before this loop. The executor-owned successor
        // transfers and direct predecessor replacements therefore occur in
        // canonical work-key order, never completion order. This low-level
        // generic API remains cooperative: an arbitrary callback payload could
        // itself contain some unrelated admission guard and drop it on a worker.
        // Crate-owned production adapters must be guard-free apart from the
        // predecessor explicitly split above.
        Ok(prepared
            .into_iter()
            .map(|prepared| match prepared {
                CampaignResidentTransformPrepared::Ready {
                    admitted,
                    predecessor_charge,
                } => match admitted.try_commit_successor(Some(predecessor_charge)) {
                    Ok(resident) => CampaignResidentTransformExecution::Committed(resident),
                    Err(failure) => CampaignResidentTransformExecution::CommitFailed(failure),
                },
                CampaignResidentTransformPrepared::BuildFailed(failure) => {
                    CampaignResidentTransformExecution::BuildFailed(failure)
                }
                CampaignResidentTransformPrepared::Panicked(panic) => {
                    CampaignResidentTransformExecution::Panicked(panic)
                }
            })
            .collect())
    }

    /// Freeze an advisory planning snapshot only at a quiescent wave barrier.
    pub fn try_snapshot(&self) -> Result<CampaignAdmissionSnapshot, CampaignAdmissionError> {
        let state = self.shared.lock();
        state.ensure_healthy()?;
        if state.in_flight_cores != 0
            || state.in_flight_peak_additional_memory != CampaignBytes::ZERO
        {
            return Err(CampaignAdmissionError::WaveStillInFlight {
                cores: state.in_flight_cores,
                peak_additional_memory: state.in_flight_peak_additional_memory,
            });
        }
        Ok(CampaignAdmissionSnapshot {
            authority: Arc::downgrade(&self.shared),
            generation: state.generation,
            policy: state.try_policy()?,
        })
    }

    /// Set additional configurable fixed/shared memory. For a controller
    /// bootstrapped from an execution-width plan, this can never replace or
    /// erase the immutable warmed execution reserve.
    pub fn try_set_fixed_and_shared(
        &mut self,
        fixed_and_shared: CampaignBytes,
    ) -> Result<(), CampaignAdmissionError> {
        let mut state = self.shared.lock();
        state.ensure_quiescent()?;
        let fixed_with_reserved = bytes_add(
            state.execution_fixed_memory,
            fixed_and_shared,
            "campaign execution plus configured fixed memory",
        )?;
        let fixed_with_reserved = bytes_add(
            fixed_with_reserved,
            state.reserved_fixed_components,
            "campaign configured plus reserved fixed components",
        )?;
        let next_baseline = CampaignBaselineMemory::try_new(
            fixed_with_reserved,
            state.resident_retained,
            state.staged_results,
        )?;
        state.check_baseline_capacity(next_baseline)?;
        if state.fixed_and_shared != fixed_and_shared {
            let next_generation = state.try_next_generation()?;
            state.fixed_and_shared = fixed_and_shared;
            state.generation = next_generation;
        }
        Ok(())
    }

    pub fn try_set_staged_results(
        &mut self,
        staged_results: CampaignBytes,
    ) -> Result<(), CampaignAdmissionError> {
        let mut state = self.shared.lock();
        state.ensure_quiescent()?;
        let fixed_with_reserved = bytes_add(
            state.execution_fixed_memory,
            state.fixed_and_shared,
            "campaign execution plus configured fixed memory",
        )?;
        let fixed_with_reserved = bytes_add(
            fixed_with_reserved,
            state.reserved_fixed_components,
            "campaign configured plus reserved fixed components",
        )?;
        let next_baseline = CampaignBaselineMemory::try_new(
            fixed_with_reserved,
            state.resident_retained,
            staged_results,
        )?;
        state.check_baseline_capacity(next_baseline)?;
        if state.staged_results != staged_results {
            let next_generation = state.try_next_generation()?;
            state.staged_results = staged_results;
            state.generation = next_generation;
        }
        Ok(())
    }

    /// Atomically reserve one move-owned fixed-component envelope.
    ///
    /// Construction-time consumers acquire this guard at a quiescent barrier
    /// before allocating their retained buffers. The separate counter cannot
    /// be overwritten by the absolute fixed/shared setter, and the guard's
    /// private shared authority can authenticate every task later admitted to
    /// the component.
    pub(crate) fn try_reserve_fixed_component(
        &mut self,
        upper_bound: CampaignBytes,
    ) -> Result<CampaignFixedComponentReservation, CampaignAdmissionError> {
        let mut state = self.shared.lock();
        state.ensure_quiescent()?;
        let usage = state.try_usage()?;
        let available = bytes_sub(
            state.max_memory,
            usage.total_charged_memory,
            "campaign fixed-component available memory",
        )?;
        if upper_bound > available {
            return Err(CampaignAdmissionError::MemoryCapacityUnavailable {
                requested: upper_bound,
                available,
            });
        }
        let next_reserved = bytes_add(
            state.reserved_fixed_components,
            upper_bound,
            "campaign fixed-component reservation",
        )?;
        let fixed_with_reserved = bytes_add(
            state.execution_fixed_memory,
            state.fixed_and_shared,
            "campaign execution plus configured fixed memory",
        )?;
        let fixed_with_reserved = bytes_add(
            fixed_with_reserved,
            next_reserved,
            "campaign configured plus reserved fixed components",
        )?;
        let next_baseline = CampaignBaselineMemory::try_new(
            fixed_with_reserved,
            state.resident_retained,
            state.staged_results,
        )?;
        state.check_baseline_capacity(next_baseline)?;
        let next_generation = if upper_bound == CampaignBytes::ZERO {
            state.generation
        } else {
            state.try_next_generation()?
        };

        // All arithmetic and capacity checks precede this infallible state
        // transition. Arc cloning below does not allocate.
        state.reserved_fixed_components = next_reserved;
        state.generation = next_generation;
        drop(state);
        Ok(CampaignFixedComponentReservation {
            shared: Arc::clone(&self.shared),
            bytes: upper_bound,
            active: true,
        })
    }

    pub fn try_reserve_wave(
        &mut self,
        snapshot: &CampaignAdmissionSnapshot,
        plan: &CampaignResourceWavePlan,
        requests: &BTreeMap<CampaignWorkKey, CampaignTaskResourceEstimate>,
    ) -> Result<CampaignWaveReservation, CampaignAdmissionError> {
        self.try_reserve_wave_with_predecessors(snapshot, plan, requests, &BTreeMap::new())
    }

    /// Revalidate and atomically charge one complete statically selected wave.
    ///
    /// `predecessors` contains opaque, non-owning tokens for work units whose
    /// output will replace an existing resident owner. The actual move-only
    /// resident is still required at commit. Omitting a token declares an
    /// initial resident output; uniqueness of that declaration belongs to the
    /// campaign workspace until it is integrated with this resource layer.
    pub fn try_reserve_wave_with_predecessors(
        &mut self,
        snapshot: &CampaignAdmissionSnapshot,
        plan: &CampaignResourceWavePlan,
        requests: &BTreeMap<CampaignWorkKey, CampaignTaskResourceEstimate>,
        predecessors: &BTreeMap<CampaignWorkKey, CampaignResidentToken>,
    ) -> Result<CampaignWaveReservation, CampaignAdmissionError> {
        if !snapshot.belongs_to(&self.shared) {
            return Err(CampaignAdmissionError::ForeignSnapshot);
        }
        let replayed = CampaignWavePlanner::try_plan(snapshot.policy, requests)?;
        if &replayed != plan {
            return Err(CampaignAdmissionError::WavePlanMismatch);
        }

        let mut tasks = Vec::new();
        tasks.try_reserve_exact(plan.work().len()).map_err(|_| {
            CampaignAdmissionError::AllocationFailure {
                resource: "campaign task reservations",
                requested: plan.work().len(),
            }
        })?;
        let mut sealed_cores = 0usize;
        let mut sealed_memory = CampaignBytes::ZERO;
        for work in plan.work() {
            let request = requests.get(work).copied().ok_or_else(|| {
                CampaignAdmissionError::MissingTaskEstimate { work: work.clone() }
            })?;
            sealed_cores = sealed_cores.checked_add(request.cores()).ok_or(
                CampaignAdmissionError::CoreCountOverflow {
                    operation: "campaign wave estimate sealing",
                },
            )?;
            sealed_memory = bytes_add(
                sealed_memory,
                request.memory().peak_additional(),
                "campaign wave estimate sealing",
            )?;
            let predecessor = predecessors.get(work).cloned();
            if let Some(token) = &predecessor {
                token.validate_for(&self.shared, work)?;
            }
            tasks.push(CampaignTaskReservation::inactive(
                Arc::clone(&self.shared),
                work.clone(),
                request,
                predecessor,
                0,
            ));
        }
        if sealed_cores != plan.selected_cores()
            || sealed_memory != plan.selected_peak_additional_memory()
        {
            return Err(CampaignAdmissionError::WavePlanMismatch);
        }
        if let Some((work, _)) = predecessors
            .iter()
            .find(|(work, _)| plan.work().binary_search(work).is_err())
        {
            return Err(CampaignAdmissionError::UnexpectedPredecessorToken { work: work.clone() });
        }

        let mut state = self.shared.lock();
        state.ensure_quiescent()?;
        if snapshot.generation != state.generation || snapshot.policy != state.try_policy()? {
            return Err(CampaignAdmissionError::StaleSnapshot {
                expected_generation: snapshot.generation.get(),
                actual_generation: state.generation.get(),
            });
        }
        if tasks.is_empty() {
            return Ok(CampaignWaveReservation {
                schema: CAMPAIGN_ADMISSION_V1_SCHEMA,
                authority: Arc::downgrade(&self.shared),
                tasks,
            });
        }
        let remaining_cores = state.core_capacity - state.in_flight_cores;
        if sealed_cores > remaining_cores {
            return Err(CampaignAdmissionError::CoreCapacityUnavailable {
                requested: sealed_cores,
                available: remaining_cores,
            });
        }
        let usage = state.try_usage()?;
        let available_memory = bytes_sub(
            state.max_memory,
            usage.total_charged_memory,
            "campaign admission available memory",
        )?;
        if sealed_memory > available_memory {
            return Err(CampaignAdmissionError::MemoryCapacityUnavailable {
                requested: sealed_memory,
                available: available_memory,
            });
        }
        let task_count = tasks.len();
        let token_count = u64::try_from(task_count).map_err(|_| {
            CampaignAdmissionError::ResidentTokenExhausted {
                requested: task_count,
            }
        })?;
        let next_token = state.next_resident_token.checked_add(token_count).ok_or(
            CampaignAdmissionError::ResidentTokenExhausted {
                requested: task_count,
            },
        )?;
        let next_generation = state.try_next_generation()?;
        let next_in_flight_cores = state.in_flight_cores.checked_add(sealed_cores).ok_or(
            CampaignAdmissionError::CoreCountOverflow {
                operation: "campaign wave reservation",
            },
        )?;
        let next_in_flight_memory = bytes_add(
            state.in_flight_peak_additional_memory,
            sealed_memory,
            "campaign wave reservation",
        )?;

        let first_token = state.next_resident_token;
        for (ordinal, task) in tasks.iter_mut().enumerate() {
            let ordinal = u64::try_from(ordinal).map_err(|_| {
                CampaignAdmissionError::ResidentTokenExhausted {
                    requested: task_count,
                }
            })?;
            task.successor_token = first_token.checked_add(ordinal).ok_or(
                CampaignAdmissionError::ResidentTokenExhausted {
                    requested: task_count,
                },
            )?;
        }

        // Every potentially failing calculation, allocation, and token write
        // has completed. The fixed accounting transition is now followed only
        // by arming already allocated guards with infallible boolean stores.
        state.next_resident_token = next_token;
        state.in_flight_cores = next_in_flight_cores;
        state.in_flight_peak_additional_memory = next_in_flight_memory;
        state.generation = next_generation;
        drop(state);

        for task in &mut tasks {
            task.active = true;
        }
        Ok(CampaignWaveReservation {
            schema: CAMPAIGN_ADMISSION_V1_SCHEMA,
            authority: Arc::downgrade(&self.shared),
            tasks,
        })
    }
}

/// Move-only charge for one dynamically retained fixed campaign component.
///
/// The authority and byte count are private: consumers can neither forge a
/// charge nor detach it from the controller that admitted their worker tasks.
pub(crate) struct CampaignFixedComponentReservation {
    shared: Arc<CampaignAdmissionShared>,
    bytes: CampaignBytes,
    active: bool,
}

impl CampaignFixedComponentReservation {
    pub(crate) const fn bytes(&self) -> CampaignBytes {
        self.bytes
    }

    /// Release unused construction headroom after the retained representation
    /// has been measured. Every check completes before accounting mutation.
    pub(crate) fn try_shrink(
        &mut self,
        retained: CampaignBytes,
    ) -> Result<(), CampaignAdmissionError> {
        if !self.active || retained > self.bytes {
            return Err(CampaignAdmissionError::AccountingInvariantBroken);
        }
        if retained == self.bytes {
            return Ok(());
        }
        let released = bytes_sub(
            self.bytes,
            retained,
            "campaign fixed-component reservation shrink",
        )?;
        let mut state = self.shared.lock();
        state.ensure_quiescent()?;
        let next_reserved = bytes_sub(
            state.reserved_fixed_components,
            released,
            "campaign fixed-component reservation shrink",
        )?;
        let next_generation = state.try_next_generation()?;

        state.reserved_fixed_components = next_reserved;
        state.generation = next_generation;
        self.bytes = retained;
        Ok(())
    }

    pub(crate) fn belongs_to_admitted<T>(&self, admitted: &CampaignAdmittedTask<T>) -> bool {
        self.active
            && admitted
                .reservation
                .as_ref()
                .is_some_and(|reservation| Arc::ptr_eq(&self.shared, &reservation.shared))
    }
}

impl fmt::Debug for CampaignFixedComponentReservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignFixedComponentReservation")
            .field("bytes", &self.bytes)
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

impl Drop for CampaignFixedComponentReservation {
    fn drop(&mut self) {
        if self.active {
            self.active = false;
            self.shared.release_fixed_component(self.bytes);
        }
    }
}

impl CampaignAdmissionShared {
    fn lock(&self) -> MutexGuard<'_, CampaignAdmissionState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                // No user callback or allocation runs under this lock, so a
                // poison event means RustRed itself panicked in an accounting
                // transition. Recover only to latch a fail-closed state; Drop
                // paths then remain non-panicking and never make the suspect
                // capacity available again.
                let mut state = poisoned.into_inner();
                state.invariant_broken = true;
                state
            }
        }
    }
}

impl CampaignAdmissionState {
    fn ensure_healthy(&self) -> Result<(), CampaignAdmissionError> {
        if self.invariant_broken {
            Err(CampaignAdmissionError::AccountingInvariantBroken)
        } else if self.generation_exhausted {
            Err(CampaignAdmissionError::GenerationExhausted)
        } else {
            Ok(())
        }
    }

    fn ensure_quiescent(&self) -> Result<(), CampaignAdmissionError> {
        self.ensure_healthy()?;
        if self.in_flight_cores != 0 || self.in_flight_peak_additional_memory != CampaignBytes::ZERO
        {
            Err(CampaignAdmissionError::WaveStillInFlight {
                cores: self.in_flight_cores,
                peak_additional_memory: self.in_flight_peak_additional_memory,
            })
        } else {
            Ok(())
        }
    }

    fn try_baseline(&self) -> Result<CampaignBaselineMemory, CampaignAdmissionError> {
        let fixed_with_reserved = bytes_add(
            self.execution_fixed_memory,
            self.fixed_and_shared,
            "campaign execution plus configured fixed memory",
        )?;
        let fixed_with_reserved = bytes_add(
            fixed_with_reserved,
            self.reserved_fixed_components,
            "campaign configured plus reserved fixed components",
        )?;
        Ok(CampaignBaselineMemory::try_new(
            fixed_with_reserved,
            self.resident_retained,
            self.staged_results,
        )?)
    }

    fn try_policy(&self) -> Result<CampaignResourcePolicy, CampaignAdmissionError> {
        Ok(CampaignResourcePolicy::try_new(
            self.estimator_revision,
            self.core_capacity,
            self.max_memory,
            self.try_baseline()?,
        )?)
    }

    fn try_usage(&self) -> Result<CampaignAdmissionUsage, CampaignAdmissionError> {
        self.ensure_healthy()?;
        let baseline = self.try_baseline()?;
        let total_charged_memory = bytes_add(
            baseline.total(),
            self.in_flight_peak_additional_memory,
            "campaign total charged memory",
        )?;
        if self.in_flight_cores > self.core_capacity || total_charged_memory > self.max_memory {
            return Err(CampaignAdmissionError::AccountingInvariantBroken);
        }
        Ok(CampaignAdmissionUsage {
            generation: self.generation,
            core_capacity: self.core_capacity,
            in_flight_cores: self.in_flight_cores,
            max_memory: self.max_memory,
            baseline,
            in_flight_peak_additional_memory: self.in_flight_peak_additional_memory,
            total_charged_memory,
        })
    }

    fn check_baseline_capacity(
        &self,
        baseline: CampaignBaselineMemory,
    ) -> Result<(), CampaignAdmissionError> {
        if baseline.total() > self.max_memory {
            Err(CampaignAdmissionError::MemoryCapacityUnavailable {
                requested: baseline.total(),
                available: self.max_memory,
            })
        } else {
            Ok(())
        }
    }

    fn try_next_generation(&self) -> Result<NonZeroU64, CampaignAdmissionError> {
        self.ensure_healthy()?;
        self.generation
            .get()
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .ok_or(CampaignAdmissionError::GenerationExhausted)
    }

    fn advance_generation_in_drop(&mut self) {
        if let Some(next) = self
            .generation
            .get()
            .checked_add(1)
            .and_then(NonZeroU64::new)
        {
            self.generation = next;
        } else {
            self.generation_exhausted = true;
        }
    }
}

fn bytes_add(
    left: CampaignBytes,
    right: CampaignBytes,
    operation: &'static str,
) -> Result<CampaignBytes, CampaignAdmissionError> {
    left.get()
        .checked_add(right.get())
        .map(CampaignBytes::new)
        .ok_or(CampaignAdmissionError::ByteCountOverflow { operation })
}

fn bytes_sub(
    left: CampaignBytes,
    right: CampaignBytes,
    operation: &'static str,
) -> Result<CampaignBytes, CampaignAdmissionError> {
    left.get()
        .checked_sub(right.get())
        .map(CampaignBytes::new)
        .ok_or(CampaignAdmissionError::ByteCountOverflow { operation })
}

/// Opaque, non-owning reference to one exact resident generation.
///
/// Cloning this token does not clone or keep alive the resident owner. A
/// successor reservation may seal it, but commit still requires consuming the
/// unique [`CampaignResident`] carrying the same generation.
#[derive(Clone)]
pub struct CampaignResidentToken {
    authority: Weak<CampaignAdmissionShared>,
    work: CampaignWorkKey,
    generation: NonZeroU64,
}

impl CampaignResidentToken {
    pub const fn work(&self) -> &CampaignWorkKey {
        &self.work
    }

    pub const fn job(&self) -> &CampaignJobKey {
        self.work.job()
    }

    pub const fn generation(&self) -> u64 {
        self.generation.get()
    }

    fn validate_for(
        &self,
        shared: &Arc<CampaignAdmissionShared>,
        work: &CampaignWorkKey,
    ) -> Result<(), CampaignAdmissionError> {
        if !Weak::ptr_eq(&self.authority, &Arc::downgrade(shared)) {
            return Err(CampaignAdmissionError::ForeignResidentToken { work: work.clone() });
        }
        if &self.work != work {
            return Err(CampaignAdmissionError::ResidentWorkMismatch {
                expected: work.clone(),
                actual: self.work.clone(),
            });
        }
        Ok(())
    }

    fn exact_match(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.authority, &other.authority)
            && self.work == other.work
            && self.generation == other.generation
    }
}

impl fmt::Debug for CampaignResidentToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignResidentToken")
            .field("work", &self.work)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

pub struct CampaignWaveReservation {
    schema: &'static str,
    authority: Weak<CampaignAdmissionShared>,
    tasks: Vec<CampaignTaskReservation>,
}

impl CampaignWaveReservation {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn tasks(&self) -> &[CampaignTaskReservation] {
        &self.tasks
    }

    pub fn into_tasks(self) -> Vec<CampaignTaskReservation> {
        self.tasks
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    fn belongs_to(&self, shared: &Arc<CampaignAdmissionShared>) -> bool {
        Weak::ptr_eq(&self.authority, &Arc::downgrade(shared))
            && self
                .tasks
                .iter()
                .all(|task| Arc::ptr_eq(&task.shared, shared))
    }
}

impl fmt::Debug for CampaignWaveReservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignWaveReservation")
            .field("schema", &self.schema)
            .field("task_count", &self.tasks.len())
            .finish_non_exhaustive()
    }
}

/// A whole admitted wave rejected before any worker callback ran.
pub struct CampaignWaveExecutionAdmissionFailure {
    error: CampaignAdmissionError,
    wave: CampaignWaveReservation,
}

impl CampaignWaveExecutionAdmissionFailure {
    pub const fn error(&self) -> &CampaignAdmissionError {
        &self.error
    }

    pub fn into_parts(self) -> (CampaignAdmissionError, CampaignWaveReservation) {
        (self.error, self.wave)
    }
}

impl fmt::Debug for CampaignWaveExecutionAdmissionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignWaveExecutionAdmissionFailure")
            .field("error", &self.error)
            .field("wave", &self.wave)
            .finish()
    }
}

/// Read-only resource context supplied to one admitted worker callback.
///
/// The executor deliberately retains ownership of the reservation. A callback
/// can size its Symbolica operation from this view, but cannot bind output,
/// transfer residency, release permits, or access the worker pool.
#[derive(Clone, Copy)]
pub struct CampaignTaskContext<'task> {
    reservation: &'task CampaignTaskReservation,
}

impl<'task> CampaignTaskContext<'task> {
    pub const fn work(self) -> &'task CampaignWorkKey {
        self.reservation.work()
    }

    pub const fn job(self) -> &'task CampaignJobKey {
        self.reservation.job()
    }

    pub const fn cores(self) -> usize {
        self.reservation.cores()
    }

    pub const fn memory(self) -> CampaignTaskMemoryEnvelope {
        self.reservation.memory()
    }

    pub const fn predecessor(self) -> Option<&'task CampaignResidentToken> {
        self.reservation.predecessor()
    }
}

impl fmt::Debug for CampaignTaskContext<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignTaskContext")
            .field("work", &self.reservation.work())
            .field("cores", &self.reservation.cores())
            .field("memory", &self.reservation.memory())
            .field("has_predecessor", &self.reservation.predecessor().is_some())
            .finish_non_exhaustive()
    }
}

/// Stable worker disposition for one admitted task.
///
/// Each variant continues to own the task reservation. Successful output can
/// be committed through its [`CampaignAdmittedTask`]; failure and panic owners
/// release their charges only after their payloads have been destroyed.
pub enum CampaignTaskExecution<Output, BuildError> {
    Built(CampaignAdmittedTask<Output>),
    Failed(CampaignTaskFailure<BuildError>),
    Panicked(CampaignTaskPanic),
}

impl<Output, BuildError> CampaignTaskExecution<Output, BuildError> {
    pub fn work(&self) -> &CampaignWorkKey {
        match self {
            Self::Built(admitted) => admitted.reservation().work(),
            Self::Failed(failure) => failure.reservation().work(),
            Self::Panicked(panic) => panic.reservation().work(),
        }
    }

    pub fn job(&self) -> &CampaignJobKey {
        self.work().job()
    }
}

impl<Output, BuildError> fmt::Debug for CampaignTaskExecution<Output, BuildError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Built(admitted) => formatter
                .debug_struct("Built")
                .field("work", admitted.reservation().work())
                .finish_non_exhaustive(),
            Self::Failed(failure) => fmt::Debug::fmt(failure, formatter),
            Self::Panicked(panic) => fmt::Debug::fmt(panic, formatter),
        }
    }
}

/// A typed task-local failure kept under its original resource charge.
pub struct CampaignTaskFailure<BuildError> {
    // Payload destruction must precede permit release, including on panic.
    error: Option<BuildError>,
    reservation: Option<CampaignTaskReservation>,
}

impl<BuildError> CampaignTaskFailure<BuildError> {
    fn new(error: BuildError, reservation: CampaignTaskReservation) -> Self {
        Self {
            error: Some(error),
            reservation: Some(reservation),
        }
    }

    pub fn error(&self) -> &BuildError {
        self.error.as_ref().expect("task failure retains its error")
    }

    pub fn reservation(&self) -> &CampaignTaskReservation {
        self.reservation
            .as_ref()
            .expect("task failure retains its reservation")
    }
}

impl<BuildError> fmt::Debug for CampaignTaskFailure<BuildError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Failed")
            .field("work", self.reservation().work())
            .finish_non_exhaustive()
    }
}

impl<BuildError> Drop for CampaignTaskFailure<BuildError> {
    fn drop(&mut self) {
        // Moving the reservation to a local makes it an unwind guard if the
        // arbitrary error payload's destructor panics.
        let reservation = self.reservation.take();
        let error = self.error.take();
        drop(error);
        drop(reservation);
    }
}

/// A recovered worker panic kept under its original resource charge.
pub struct CampaignTaskPanic {
    // Payload destruction must precede permit release, including on panic.
    payload: Option<Box<dyn std::any::Any + Send + 'static>>,
    reservation: Option<CampaignTaskReservation>,
}

impl CampaignTaskPanic {
    fn new(
        payload: Box<dyn std::any::Any + Send + 'static>,
        reservation: CampaignTaskReservation,
    ) -> Self {
        Self {
            payload: Some(payload),
            reservation: Some(reservation),
        }
    }

    pub fn message(&self) -> Option<&str> {
        let payload = self.payload.as_ref()?;
        payload
            .downcast_ref::<&'static str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
    }

    pub fn reservation(&self) -> &CampaignTaskReservation {
        self.reservation
            .as_ref()
            .expect("task panic retains its reservation")
    }
}

impl fmt::Debug for CampaignTaskPanic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Panicked")
            .field("work", self.reservation().work())
            .field("message", &self.message())
            .finish_non_exhaustive()
    }
}

impl Drop for CampaignTaskPanic {
    fn drop(&mut self) {
        // As for typed failures, the reservation remains an unwind guard around
        // arbitrary panic-payload destruction.
        let reservation = self.reservation.take();
        let payload = self.payload.take();
        drop(payload);
        drop(reservation);
    }
}

/// One move-only task charge split from an atomically reserved wave.
pub struct CampaignTaskReservation {
    shared: Arc<CampaignAdmissionShared>,
    work: CampaignWorkKey,
    request: CampaignTaskResourceEstimate,
    predecessor: Option<CampaignResidentToken>,
    successor_token: u64,
    remaining_memory: CampaignBytes,
    retained_transferred: bool,
    active: bool,
    _not_sync: PhantomData<Cell<()>>,
}

impl CampaignTaskReservation {
    fn inactive(
        shared: Arc<CampaignAdmissionShared>,
        work: CampaignWorkKey,
        request: CampaignTaskResourceEstimate,
        predecessor: Option<CampaignResidentToken>,
        successor_token: u64,
    ) -> Self {
        Self {
            shared,
            work,
            request,
            predecessor,
            successor_token,
            remaining_memory: request.memory().peak_additional(),
            retained_transferred: false,
            active: false,
            _not_sync: PhantomData,
        }
    }

    pub const fn work(&self) -> &CampaignWorkKey {
        &self.work
    }

    pub const fn job(&self) -> &CampaignJobKey {
        self.work.job()
    }

    pub const fn cores(&self) -> usize {
        self.request.cores()
    }

    pub const fn memory(&self) -> CampaignTaskMemoryEnvelope {
        self.request.memory()
    }

    pub const fn predecessor(&self) -> Option<&CampaignResidentToken> {
        self.predecessor.as_ref()
    }

    /// Pair this successor reservation with the exact move-only resident whose
    /// generation was sealed at wave admission.
    pub fn try_bind_resident_transform<T>(
        self,
        predecessor: CampaignResident<T>,
    ) -> Result<CampaignResidentTransformTask<T>, CampaignResidentTransformBindFailure<T>> {
        if let Err(error) = validate_predecessor(&self, Some(&predecessor)) {
            return Err(CampaignResidentTransformBindFailure {
                error,
                reservation: self,
                predecessor,
            });
        }
        Ok(CampaignResidentTransformTask {
            reservation: Some(self),
            predecessor: Some(predecessor),
        })
    }

    /// Bind the retained successor owner after task-local scratch has gone out
    /// of scope. Prefer [`Self::try_build`] when the construction itself may
    /// fail or panic.
    pub fn bind<T>(self, retained_output: T) -> CampaignAdmittedTask<T> {
        CampaignAdmittedTask {
            retained_output: Some(retained_output),
            reservation: Some(self),
        }
    }

    /// Run successor construction while this reservation remains live.
    /// Closure-local scratch is destroyed before an error is returned or a
    /// panic unwinds through the reservation.
    pub fn try_build<T, E, Build>(self, build: Build) -> Result<CampaignAdmittedTask<T>, E>
    where
        Build: FnOnce(&Self) -> Result<T, E>,
    {
        let retained_output = build(&self)?;
        Ok(self.bind(retained_output))
    }

    fn successor_token(&self) -> CampaignResidentToken {
        CampaignResidentToken {
            authority: Arc::downgrade(&self.shared),
            work: self.work.clone(),
            generation: NonZeroU64::new(self.successor_token)
                .expect("active task reservations have a nonzero successor token"),
        }
    }

    fn try_transfer_retained(&mut self) -> Result<(), CampaignAdmissionError> {
        if !self.active || self.retained_transferred {
            return Err(CampaignAdmissionError::AccountingInvariantBroken);
        }
        let retained = self.request.memory().retained_output().total();
        let transient = self.request.memory().transient_excluding_output().total();
        self.shared
            .try_transfer_task_retained(retained, self.remaining_memory, transient)?;
        self.remaining_memory = transient;
        self.retained_transferred = true;
        Ok(())
    }
}

impl fmt::Debug for CampaignTaskReservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignTaskReservation")
            .field("work", &self.work)
            .field("cores", &self.request.cores())
            .field("memory", &self.request.memory())
            .field("has_predecessor", &self.predecessor.is_some())
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

impl Drop for CampaignTaskReservation {
    fn drop(&mut self) {
        if self.active {
            self.shared
                .release_task(self.request.cores(), self.remaining_memory);
            self.active = false;
        }
    }
}

/// A successor reservation paired with the complete resident owner it will
/// transform. The pair is move-only and destroys the predecessor before
/// releasing the task reservation on every unwind path.
pub struct CampaignResidentTransformTask<T> {
    reservation: Option<CampaignTaskReservation>,
    predecessor: Option<CampaignResident<T>>,
}

impl<T> CampaignResidentTransformTask<T> {
    pub fn reservation(&self) -> &CampaignTaskReservation {
        self.reservation
            .as_ref()
            .expect("resident transform retains its reservation")
    }

    pub fn predecessor(&self) -> &CampaignResident<T> {
        self.predecessor
            .as_ref()
            .expect("resident transform retains its predecessor")
    }

    fn belongs_to(&self, shared: &Arc<CampaignAdmissionShared>) -> bool {
        self.reservation
            .as_ref()
            .is_some_and(|reservation| Arc::ptr_eq(&reservation.shared, shared))
            && self
                .predecessor
                .as_ref()
                .is_some_and(|predecessor| Arc::ptr_eq(&predecessor.shared, shared))
    }

    pub fn predecessor_mut(&mut self) -> &mut CampaignResident<T> {
        self.predecessor
            .as_mut()
            .expect("resident transform retains its predecessor")
    }

    fn preflight(&self) -> Result<(), CampaignAdmissionError> {
        let reservation = self.reservation();
        let predecessor = self.predecessor();
        validate_predecessor(reservation, Some(predecessor))?;
        reservation
            .shared
            .preflight_resident_transform(reservation, predecessor)
    }

    fn into_parts(mut self) -> (CampaignTaskReservation, CampaignResident<T>) {
        let reservation = self
            .reservation
            .take()
            .expect("resident transform retains its reservation");
        let predecessor = self
            .predecessor
            .take()
            .expect("resident transform retains its predecessor");
        (reservation, predecessor)
    }

    /// Discard the admitted successor attempt and recover the intact
    /// predecessor generation. This is useful after a typed build failure; the
    /// task reservation is released before the resident is returned.
    fn into_predecessor(mut self) -> CampaignResident<T> {
        let reservation = self.reservation.take();
        let predecessor = self
            .predecessor
            .take()
            .expect("resident transform retains its predecessor");
        drop(reservation);
        predecessor
    }
}

/// A resident-transform batch rejected before any predecessor owner was moved
/// into a callback. Dropping this value drops every still-charged task and
/// resident in owner-before-permit order; `into_parts` allows deterministic
/// retry or checkpoint recovery.
pub struct CampaignResidentTransformBatchAdmissionFailure<T> {
    error: CampaignAdmissionError,
    tasks: Vec<CampaignResidentTransformTask<T>>,
}

impl<T> CampaignResidentTransformBatchAdmissionFailure<T> {
    pub const fn error(&self) -> &CampaignAdmissionError {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        CampaignAdmissionError,
        Vec<CampaignResidentTransformTask<T>>,
    ) {
        (self.error, self.tasks)
    }
}

impl<T> fmt::Debug for CampaignResidentTransformBatchAdmissionFailure<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignResidentTransformBatchAdmissionFailure")
            .field("error", &self.error)
            .field("task_count", &self.tasks.len())
            .finish_non_exhaustive()
    }
}

impl<T> fmt::Debug for CampaignResidentTransformTask<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignResidentTransformTask")
            .field("work", self.reservation().work())
            .field("predecessor", self.predecessor().token())
            .finish_non_exhaustive()
    }
}

impl<T> Drop for CampaignResidentTransformTask<T> {
    fn drop(&mut self) {
        let reservation = self.reservation.take();
        let predecessor = self.predecessor.take();
        drop(predecessor);
        drop(reservation);
    }
}

pub struct CampaignResidentTransformBindFailure<T> {
    error: CampaignAdmissionError,
    reservation: CampaignTaskReservation,
    predecessor: CampaignResident<T>,
}

impl<T> CampaignResidentTransformBindFailure<T> {
    pub const fn error(&self) -> &CampaignAdmissionError {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        CampaignAdmissionError,
        CampaignTaskReservation,
        CampaignResident<T>,
    ) {
        (self.error, self.reservation, self.predecessor)
    }
}

impl<T> fmt::Debug for CampaignResidentTransformBindFailure<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignResidentTransformBindFailure")
            .field("error", &self.error)
            .field("work", self.reservation.work())
            .finish_non_exhaustive()
    }
}

/// Recoverable worker failure carrying the callback-supplied owner that remains
/// paired with the predecessor's resident charge.
///
/// For an arbitrary public `T`, this is a cooperative contract rather than an
/// identity proof. RustRed's internal exact-session adapters must return the
/// same allocation lineage and must not report a predecessor failure after a
/// committed mutation.
pub struct CampaignResidentTransformBuildFailure<T, BuildError> {
    callback_owner: T,
    error: BuildError,
}

impl<T, BuildError> CampaignResidentTransformBuildFailure<T, BuildError> {
    pub const fn new(callback_owner: T, error: BuildError) -> Self {
        Self {
            callback_owner,
            error,
        }
    }

    pub const fn callback_owner(&self) -> &T {
        &self.callback_owner
    }

    pub const fn error(&self) -> &BuildError {
        &self.error
    }

    fn into_parts(self) -> (T, BuildError) {
        (self.callback_owner, self.error)
    }
}

impl<T, BuildError> fmt::Debug for CampaignResidentTransformBuildFailure<T, BuildError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignResidentTransformBuildFailure")
            .finish_non_exhaustive()
    }
}

pub enum CampaignResidentTransformExecution<Predecessor, Successor, BuildError> {
    Committed(CampaignResident<Successor>),
    BuildFailed(CampaignResidentTransformFailure<Predecessor, BuildError>),
    Panicked(CampaignResidentTransformPanic),
    CommitFailed(CampaignCommitFailure<(), Successor>),
}

enum CampaignResidentTransformPrepared<Predecessor, Successor, BuildError> {
    Ready {
        admitted: CampaignAdmittedTask<Successor>,
        predecessor_charge: CampaignResident<()>,
    },
    BuildFailed(CampaignResidentTransformFailure<Predecessor, BuildError>),
    Panicked(CampaignResidentTransformPanic),
}

impl<Predecessor, Successor, BuildError>
    CampaignResidentTransformExecution<Predecessor, Successor, BuildError>
{
    pub fn work(&self) -> &CampaignWorkKey {
        match self {
            Self::Committed(resident) => resident.token().work(),
            Self::BuildFailed(failure) => failure.task().reservation().work(),
            Self::Panicked(panic) => panic.reservation().work(),
            Self::CommitFailed(failure) => failure.admitted.reservation().work(),
        }
    }

    pub fn job(&self) -> &CampaignJobKey {
        self.work().job()
    }
}

impl<Predecessor, Successor, BuildError> fmt::Debug
    for CampaignResidentTransformExecution<Predecessor, Successor, BuildError>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Committed(resident) => formatter
                .debug_struct("Committed")
                .field("work", resident.token().work())
                .finish_non_exhaustive(),
            Self::BuildFailed(failure) => fmt::Debug::fmt(failure, formatter),
            Self::Panicked(panic) => fmt::Debug::fmt(panic, formatter),
            Self::CommitFailed(failure) => fmt::Debug::fmt(failure, formatter),
        }
    }
}

pub struct CampaignResidentTransformFailure<T, BuildError> {
    // Diagnostic destruction precedes predecessor/task charge release.
    error: Option<BuildError>,
    task: Option<CampaignResidentTransformTask<T>>,
}

impl<T, BuildError> CampaignResidentTransformFailure<T, BuildError> {
    pub fn error(&self) -> &BuildError {
        self.error
            .as_ref()
            .expect("resident transform failure retains its error")
    }

    pub fn task(&self) -> &CampaignResidentTransformTask<T> {
        self.task
            .as_ref()
            .expect("resident transform failure retains its task")
    }

    /// Drop the diagnostic under the task charge, release the successor
    /// reservation, and return the callback-supplied resident owner for retry
    /// under the cooperative identity contract described above.
    pub fn recover_callback_owner(mut self) -> CampaignResident<T> {
        let task = self
            .task
            .take()
            .expect("resident transform failure retains its task");
        let error = self.error.take();
        drop(error);
        task.into_predecessor()
    }
}

impl<T, BuildError> fmt::Debug for CampaignResidentTransformFailure<T, BuildError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignResidentTransformBuildFailed")
            .field("work", self.task().reservation().work())
            .finish_non_exhaustive()
    }
}

impl<T, BuildError> Drop for CampaignResidentTransformFailure<T, BuildError> {
    fn drop(&mut self) {
        let task = self.task.take();
        let error = self.error.take();
        drop(error);
        drop(task);
    }
}

/// A resident-transform panic after the predecessor payload was transferred to
/// user code. The owner is considered lost and must be reconstructed from a
/// durable checkpoint; its old resident charge remains paired with this panic
/// until the panic payload is destroyed.
pub struct CampaignResidentTransformPanic {
    payload: Option<Box<dyn std::any::Any + Send + 'static>>,
    predecessor_charge: Option<CampaignResident<()>>,
    reservation: Option<CampaignTaskReservation>,
}

impl CampaignResidentTransformPanic {
    pub fn message(&self) -> Option<&str> {
        let payload = self.payload.as_ref()?;
        payload
            .downcast_ref::<&'static str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
    }

    pub fn reservation(&self) -> &CampaignTaskReservation {
        self.reservation
            .as_ref()
            .expect("resident transform panic retains its reservation")
    }
}

impl fmt::Debug for CampaignResidentTransformPanic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignResidentTransformPanicked")
            .field("work", self.reservation().work())
            .field("message", &self.message())
            .finish_non_exhaustive()
    }
}

impl Drop for CampaignResidentTransformPanic {
    fn drop(&mut self) {
        let reservation = self.reservation.take();
        let predecessor_charge = self.predecessor_charge.take();
        let payload = self.payload.take();
        drop(payload);
        drop(predecessor_charge);
        drop(reservation);
    }
}

/// A retained output whose complete task peak is still charged.
pub struct CampaignAdmittedTask<T> {
    // The owner is deliberately destroyed before its reservation.
    retained_output: Option<T>,
    reservation: Option<CampaignTaskReservation>,
}

impl<T> CampaignAdmittedTask<T> {
    pub fn retained_output(&self) -> &T {
        self.retained_output
            .as_ref()
            .expect("admitted task retains its output")
    }

    pub fn retained_output_mut(&mut self) -> &mut T {
        self.retained_output
            .as_mut()
            .expect("admitted task retains its output")
    }

    pub fn reservation(&self) -> &CampaignTaskReservation {
        self.reservation
            .as_ref()
            .expect("admitted task retains its reservation")
    }

    pub fn try_commit_initial(self) -> Result<CampaignResident<T>, CampaignCommitFailure<(), T>> {
        self.try_commit_successor(None::<CampaignResident<()>>)
    }

    /// Commit this output while consuming the exact predecessor generation
    /// sealed when the task wave was reserved.
    pub fn try_commit_successor<Previous>(
        mut self,
        predecessor: Option<CampaignResident<Previous>>,
    ) -> Result<CampaignResident<T>, CampaignCommitFailure<Previous, T>> {
        if let Err(error) = self.preflight_predecessor(predecessor.as_ref()) {
            return Err(CampaignCommitFailure {
                error,
                admitted: self,
                predecessor,
            });
        }
        if let Err(error) = self
            .reservation
            .as_ref()
            .expect("admitted task retains its reservation")
            .shared
            .preflight_transfer(
                self.reservation
                    .as_ref()
                    .expect("admitted task retains its reservation"),
            )
        {
            return Err(CampaignCommitFailure {
                error,
                admitted: self,
                predecessor,
            });
        }

        let token = self
            .reservation
            .as_ref()
            .expect("admitted task retains its reservation")
            .successor_token();
        let estimate = self
            .reservation
            .as_ref()
            .expect("admitted task retains its reservation")
            .request
            .memory()
            .retained_output();
        let shared = Arc::clone(
            &self
                .reservation
                .as_ref()
                .expect("admitted task retains its reservation")
                .shared,
        );
        let retained_output = self
            .retained_output
            .take()
            .expect("admitted task retains its output");
        let mut resident = CampaignResident {
            retained_output: Some(retained_output),
            shared,
            token,
            estimate,
            active: false,
        };

        // This exact reclassification keeps predecessor residency untouched.
        // It has no allocation or user callback after the fallible preflight.
        if let Err(error) = self
            .reservation
            .as_mut()
            .expect("admitted task retains its reservation")
            .try_transfer_retained()
        {
            self.retained_output = resident.retained_output.take();
            return Err(CampaignCommitFailure {
                error,
                admitted: self,
                predecessor,
            });
        }
        resident.active = true;

        // The task still holds cores and transient memory while arbitrary old
        // payload destruction runs. A panicking destructor unwinds both guards.
        drop(predecessor);
        drop(self);
        Ok(resident)
    }

    fn preflight_predecessor<Previous>(
        &self,
        predecessor: Option<&CampaignResident<Previous>>,
    ) -> Result<(), CampaignAdmissionError> {
        let reservation = self
            .reservation
            .as_ref()
            .expect("admitted task retains its reservation");
        validate_predecessor(reservation, predecessor)
    }
}

fn validate_predecessor<Previous>(
    reservation: &CampaignTaskReservation,
    predecessor: Option<&CampaignResident<Previous>>,
) -> Result<(), CampaignAdmissionError> {
    if let Some(actual) = predecessor {
        actual
            .token()
            .validate_for(&reservation.shared, &reservation.work)?;
    }
    let expected = reservation.predecessor.as_ref();
    let actual = predecessor.map(CampaignResident::token);
    match (expected, actual) {
        (None, None) => Ok(()),
        (Some(expected), Some(actual)) if expected.exact_match(actual) => Ok(()),
        (Some(expected), Some(actual)) => Err(CampaignAdmissionError::ResidentGenerationMismatch {
            work: expected.work.clone(),
            expected: expected.generation.get(),
            actual: Some(actual.generation.get()),
        }),
        (Some(expected), None) => Err(CampaignAdmissionError::ResidentGenerationMismatch {
            work: expected.work.clone(),
            expected: expected.generation.get(),
            actual: None,
        }),
        (None, Some(actual)) => Err(CampaignAdmissionError::UnexpectedResidentPredecessor {
            work: actual.work.clone(),
            generation: actual.generation.get(),
        }),
    }
}

impl<T> Drop for CampaignAdmittedTask<T> {
    fn drop(&mut self) {
        // Moving the reservation to a local makes its Drop an unwind guard if
        // the arbitrary retained-output destructor itself panics.
        let reservation = self.reservation.take();
        let retained_output = self.retained_output.take();
        drop(retained_output);
        drop(reservation);
    }
}

/// A heavyweight retained owner paired with its exact memory charge.
pub struct CampaignResident<T> {
    // Owner destruction always precedes releasing resident memory.
    retained_output: Option<T>,
    shared: Arc<CampaignAdmissionShared>,
    token: CampaignResidentToken,
    estimate: CampaignMemoryEstimate,
    active: bool,
}

impl<T> CampaignResident<T> {
    pub const fn token(&self) -> &CampaignResidentToken {
        &self.token
    }

    pub const fn estimate(&self) -> CampaignMemoryEstimate {
        self.estimate
    }

    pub fn retained_output(&self) -> &T {
        self.retained_output
            .as_ref()
            .expect("campaign resident retains its output")
    }

    pub fn retained_output_mut(&mut self) -> &mut T {
        self.retained_output
            .as_mut()
            .expect("campaign resident retains its output")
    }

    /// Separate the payload from its still-active accounting generation.
    ///
    /// The returned unit owner is an internal charge shell used only while a
    /// consuming resident transformation runs. Cloning the compact token here
    /// avoids unsafe field extraction from a type with a custom destructor.
    pub(crate) fn split_owner_charge(mut self) -> (T, CampaignResident<()>) {
        let retained_output = self
            .retained_output
            .take()
            .expect("campaign resident retains its output");
        let charge = CampaignResident {
            retained_output: Some(()),
            shared: Arc::clone(&self.shared),
            token: self.token.clone(),
            estimate: self.estimate,
            active: self.active,
        };
        self.active = false;
        drop(self);
        (retained_output, charge)
    }
}

impl CampaignResident<()> {
    pub(crate) fn restore_owner<T>(mut self, retained_output: T) -> CampaignResident<T> {
        let restored = CampaignResident {
            retained_output: Some(retained_output),
            shared: Arc::clone(&self.shared),
            token: self.token.clone(),
            estimate: self.estimate,
            active: self.active,
        };
        self.active = false;
        drop(self);
        restored
    }
}

impl<T> fmt::Debug for CampaignResident<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignResident")
            .field("token", &self.token)
            .field("estimate", &self.estimate)
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

struct CampaignResidentRelease {
    shared: Arc<CampaignAdmissionShared>,
    estimate: CampaignBytes,
    active: bool,
}

impl Drop for CampaignResidentRelease {
    fn drop(&mut self) {
        if self.active {
            self.shared.release_resident(self.estimate);
            self.active = false;
        }
    }
}

impl<T> Drop for CampaignResident<T> {
    fn drop(&mut self) {
        let release = CampaignResidentRelease {
            shared: Arc::clone(&self.shared),
            estimate: self.estimate.total(),
            active: self.active,
        };
        self.active = false;
        let retained_output = self.retained_output.take();
        drop(retained_output);
        drop(release);
    }
}

pub struct CampaignCommitFailure<Previous, Output> {
    error: CampaignAdmissionError,
    admitted: CampaignAdmittedTask<Output>,
    predecessor: Option<CampaignResident<Previous>>,
}

impl<Previous, Output> CampaignCommitFailure<Previous, Output> {
    pub const fn error(&self) -> &CampaignAdmissionError {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        CampaignAdmissionError,
        CampaignAdmittedTask<Output>,
        Option<CampaignResident<Previous>>,
    ) {
        (self.error, self.admitted, self.predecessor)
    }
}

impl<Previous, Output> fmt::Debug for CampaignCommitFailure<Previous, Output> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignCommitFailure")
            .field("error", &self.error)
            .field("has_predecessor", &self.predecessor.is_some())
            .finish_non_exhaustive()
    }
}

impl CampaignAdmissionShared {
    fn preflight_resident_transform<T>(
        &self,
        task: &CampaignTaskReservation,
        predecessor: &CampaignResident<T>,
    ) -> Result<(), CampaignAdmissionError> {
        if !predecessor.active {
            return Err(CampaignAdmissionError::AccountingInvariantBroken);
        }
        self.preflight_transfer(task)?;
        let state = self.lock();
        state.ensure_healthy()?;
        if state.resident_retained < predecessor.estimate.total() {
            return Err(CampaignAdmissionError::AccountingInvariantBroken);
        }
        Ok(())
    }

    fn preflight_transfer(
        &self,
        task: &CampaignTaskReservation,
    ) -> Result<(), CampaignAdmissionError> {
        let state = self.lock();
        state.ensure_healthy()?;
        if !task.active || task.retained_transferred {
            return Err(CampaignAdmissionError::AccountingInvariantBroken);
        }
        if state.in_flight_cores < task.request.cores()
            || state.in_flight_peak_additional_memory < task.remaining_memory
        {
            return Err(CampaignAdmissionError::AccountingInvariantBroken);
        }
        let retained = task.request.memory().retained_output().total();
        let transient = task.request.memory().transient_excluding_output().total();
        if bytes_sub(
            task.remaining_memory,
            retained,
            "campaign retained-transfer preflight",
        )? != transient
        {
            return Err(CampaignAdmissionError::AccountingInvariantBroken);
        }
        let next_in_flight = bytes_sub(
            state.in_flight_peak_additional_memory,
            retained,
            "campaign retained-transfer preflight",
        )?;
        let next_resident = bytes_add(
            state.resident_retained,
            retained,
            "campaign retained-transfer preflight",
        )?;
        let fixed_with_reserved = bytes_add(
            state.execution_fixed_memory,
            state.fixed_and_shared,
            "campaign execution plus configured fixed memory",
        )?;
        let fixed_with_reserved = bytes_add(
            fixed_with_reserved,
            state.reserved_fixed_components,
            "campaign configured plus reserved fixed components",
        )?;
        let next_baseline = CampaignBaselineMemory::try_new(
            fixed_with_reserved,
            next_resident,
            state.staged_results,
        )?;
        let next_total = bytes_add(
            next_baseline.total(),
            next_in_flight,
            "campaign retained-transfer preflight",
        )?;
        if next_total > state.max_memory {
            return Err(CampaignAdmissionError::AccountingInvariantBroken);
        }
        state.try_next_generation()?;
        Ok(())
    }

    fn try_transfer_task_retained(
        &self,
        retained: CampaignBytes,
        previous_task_memory: CampaignBytes,
        transient: CampaignBytes,
    ) -> Result<(), CampaignAdmissionError> {
        let mut state = self.lock();
        state.ensure_healthy()?;
        if bytes_sub(previous_task_memory, retained, "campaign retained transfer")? != transient {
            return Err(CampaignAdmissionError::AccountingInvariantBroken);
        }
        let next_in_flight = bytes_sub(
            state.in_flight_peak_additional_memory,
            retained,
            "campaign retained transfer",
        )?;
        let next_resident = bytes_add(
            state.resident_retained,
            retained,
            "campaign retained transfer",
        )?;
        let next_generation = state.try_next_generation()?;
        state.in_flight_peak_additional_memory = next_in_flight;
        state.resident_retained = next_resident;
        state.generation = next_generation;
        Ok(())
    }

    fn release_task(&self, cores: usize, memory: CampaignBytes) {
        let mut state = self.lock();
        if state.invariant_broken {
            return;
        }
        let Some(next_cores) = state.in_flight_cores.checked_sub(cores) else {
            state.invariant_broken = true;
            return;
        };
        let Some(next_memory) = state
            .in_flight_peak_additional_memory
            .get()
            .checked_sub(memory.get())
            .map(CampaignBytes::new)
        else {
            state.invariant_broken = true;
            return;
        };
        state.in_flight_cores = next_cores;
        state.in_flight_peak_additional_memory = next_memory;
        state.advance_generation_in_drop();
    }

    fn release_resident(&self, memory: CampaignBytes) {
        let mut state = self.lock();
        if state.invariant_broken {
            return;
        }
        let Some(next_resident) = state
            .resident_retained
            .get()
            .checked_sub(memory.get())
            .map(CampaignBytes::new)
        else {
            state.invariant_broken = true;
            return;
        };
        state.resident_retained = next_resident;
        state.advance_generation_in_drop();
    }

    fn release_fixed_component(&self, memory: CampaignBytes) {
        if memory == CampaignBytes::ZERO {
            return;
        }
        let mut state = self.lock();
        if state.invariant_broken {
            return;
        }
        let Some(next_reserved) = state
            .reserved_fixed_components
            .get()
            .checked_sub(memory.get())
            .map(CampaignBytes::new)
        else {
            state.invariant_broken = true;
            return;
        };
        state.reserved_fixed_components = next_reserved;
        state.advance_generation_in_drop();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CampaignAdmissionError {
    ExecutionWidthPlanHasHydratedRetainedMemory {
        bytes: CampaignBytes,
    },
    ParallelExecution(ParallelExecutionError),
    ForeignSnapshot,
    ForeignWaveReservation,
    ForeignTaskReservation {
        work: CampaignWorkKey,
    },
    UnsupportedExecutorCoreWidth {
        work: CampaignWorkKey,
        requested: usize,
    },
    StaleSnapshot {
        expected_generation: u64,
        actual_generation: u64,
    },
    WaveStillInFlight {
        cores: usize,
        peak_additional_memory: CampaignBytes,
    },
    WavePlanMismatch,
    MissingTaskEstimate {
        work: CampaignWorkKey,
    },
    UnexpectedPredecessorToken {
        work: CampaignWorkKey,
    },
    ForeignResidentToken {
        work: CampaignWorkKey,
    },
    ResidentWorkMismatch {
        expected: CampaignWorkKey,
        actual: CampaignWorkKey,
    },
    ResidentGenerationMismatch {
        work: CampaignWorkKey,
        expected: u64,
        actual: Option<u64>,
    },
    UnexpectedResidentPredecessor {
        work: CampaignWorkKey,
        generation: u64,
    },
    CoreCapacityUnavailable {
        requested: usize,
        available: usize,
    },
    MemoryCapacityUnavailable {
        requested: CampaignBytes,
        available: CampaignBytes,
    },
    ResidentTokenExhausted {
        requested: usize,
    },
    GenerationExhausted,
    AccountingInvariantBroken,
    CoreCountOverflow {
        operation: &'static str,
    },
    ByteCountOverflow {
        operation: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Resource(CampaignResourceError),
}

impl fmt::Display for CampaignAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExecutionWidthPlanHasHydratedRetainedMemory { bytes } => write!(
                formatter,
                "campaign execution-width bootstrap cannot seed {bytes} of hydrated retained memory without resident owners"
            ),
            Self::ParallelExecution(error) => {
                write!(
                    formatter,
                    "campaign execution-width bootstrap failed: {error}"
                )
            }
            Self::ForeignSnapshot => {
                formatter.write_str("campaign admission snapshot belongs to another controller")
            }
            Self::ForeignWaveReservation => formatter
                .write_str("campaign wave reservation belongs to another admission controller"),
            Self::ForeignTaskReservation { work } => write!(
                formatter,
                "campaign executor work unit for sector {} belongs to another admission controller",
                work.job().sector()
            ),
            Self::UnsupportedExecutorCoreWidth { work, requested } => write!(
                formatter,
                "campaign executor task for sector {} requests {requested} cores; the current controlled outer executor supports exactly one core per task",
                work.job().sector()
            ),
            Self::StaleSnapshot {
                expected_generation,
                actual_generation,
            } => write!(
                formatter,
                "campaign admission snapshot generation {expected_generation} is stale; current generation is {actual_generation}"
            ),
            Self::WaveStillInFlight {
                cores,
                peak_additional_memory,
            } => write!(
                formatter,
                "campaign wave still owns {cores} cores and {peak_additional_memory} of peak memory"
            ),
            Self::WavePlanMismatch => formatter.write_str(
                "campaign wave differs from deterministic selection replay for its snapshot",
            ),
            Self::MissingTaskEstimate { work } => write!(
                formatter,
                "campaign wave has no resource estimate for work unit in sector {}",
                work.job().sector()
            ),
            Self::UnexpectedPredecessorToken { work } => write!(
                formatter,
                "campaign predecessor token was supplied for an unselected work unit in sector {}",
                work.job().sector()
            ),
            Self::ForeignResidentToken { work } => write!(
                formatter,
                "resident token for a work unit in sector {} belongs to another admission controller",
                work.job().sector()
            ),
            Self::ResidentWorkMismatch { expected, actual } => write!(
                formatter,
                "resident work unit {:?} does not match expected work unit {:?}",
                actual, expected
            ),
            Self::ResidentGenerationMismatch {
                work,
                expected,
                actual,
            } => write!(
                formatter,
                "resident work unit in sector {} generation {:?} does not match sealed predecessor generation {expected}",
                work.job().sector(),
                actual
            ),
            Self::UnexpectedResidentPredecessor { work, generation } => write!(
                formatter,
                "initial resident commit for a work unit in sector {} unexpectedly received generation {generation}",
                work.job().sector()
            ),
            Self::CoreCapacityUnavailable {
                requested,
                available,
            } => write!(
                formatter,
                "campaign wave requests {requested} cores but only {available} are available"
            ),
            Self::MemoryCapacityUnavailable {
                requested,
                available,
            } => write!(
                formatter,
                "campaign wave requests {requested} but only {available} of estimated memory is available"
            ),
            Self::ResidentTokenExhausted { requested } => write!(
                formatter,
                "campaign cannot mint {requested} additional resident generations"
            ),
            Self::GenerationExhausted => {
                formatter.write_str("campaign admission generation is exhausted")
            }
            Self::AccountingInvariantBroken => {
                formatter.write_str("campaign admission accounting invariant is broken")
            }
            Self::CoreCountOverflow { operation } => {
                write!(formatter, "{operation} overflowed usize")
            }
            Self::ByteCountOverflow { operation } => {
                write!(formatter, "{operation} overflowed u64")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} units for {resource}"
            ),
            Self::Resource(error) => write!(formatter, "campaign resource policy failed: {error}"),
        }
    }
}

impl std::error::Error for CampaignAdmissionError {}

impl From<CampaignResourceError> for CampaignAdmissionError {
    fn from(value: CampaignResourceError) -> Self {
        Self::Resource(value)
    }
}
