//! Atomic runtime resource admission for bounded campaign waves.
//!
//! [`CampaignWavePlanner`](crate::CampaignWavePlanner) remains a stateless,
//! deterministic selector. This module is the distinct runtime authority: one
//! non-cloneable coordinator reserves a complete selected wave in one locked
//! transition, while move-only guards keep cores and estimated memory charged
//! until the corresponding owners have actually been dropped or transferred.
//! No algebra, reducer, worker dispatch/executor, or checkpoint is implemented
//! here; the controller merely owns the one pool a future executor must borrow.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use crate::{
    CampaignBaselineMemory, CampaignBytes, CampaignEstimatorRevision, CampaignJobKey,
    CampaignMemoryEstimate, CampaignResourceError, CampaignResourcePolicy,
    CampaignResourceWavePlan, CampaignTaskMemoryEnvelope, CampaignTaskResourceEstimate,
    CampaignWavePlanner, ParallelExecution,
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
    shared: Arc<CampaignAdmissionShared>,
}

impl fmt::Debug for CampaignAdmissionController {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let usage = self.try_usage();
        formatter
            .debug_struct("CampaignAdmissionController")
            .field("schema", &self.schema)
            .field("execution", &self.execution)
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
    fixed_and_shared: CampaignBytes,
    staged_results: CampaignBytes,
    resident_retained: CampaignBytes,
    in_flight_cores: usize,
    in_flight_peak_additional_memory: CampaignBytes,
    next_resident_token: u64,
}

impl CampaignAdmissionController {
    pub fn try_new(
        execution: ParallelExecution,
        estimator_revision: CampaignEstimatorRevision,
        max_memory: CampaignBytes,
        fixed_and_shared: CampaignBytes,
        staged_results: CampaignBytes,
    ) -> Result<Self, CampaignAdmissionError> {
        let core_capacity = execution.n_cores();
        let baseline =
            CampaignBaselineMemory::try_new(fixed_and_shared, CampaignBytes::ZERO, staged_results)?;
        CampaignResourcePolicy::try_new(estimator_revision, core_capacity, max_memory, baseline)?;
        Ok(Self {
            schema: CAMPAIGN_ADMISSION_V1_SCHEMA,
            execution,
            shared: Arc::new(CampaignAdmissionShared {
                state: Mutex::new(CampaignAdmissionState {
                    generation: NonZeroU64::MIN,
                    generation_exhausted: false,
                    invariant_broken: false,
                    estimator_revision,
                    core_capacity,
                    max_memory,
                    fixed_and_shared,
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

    /// The future campaign executor borrows this sole invocation-wide pool.
    /// It is intentionally not exposed as a public bypass around admission.
    #[allow(dead_code)]
    pub(crate) const fn execution(&self) -> &ParallelExecution {
        &self.execution
    }

    pub fn try_usage(&self) -> Result<CampaignAdmissionUsage, CampaignAdmissionError> {
        self.shared.lock().try_usage()
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

    pub fn try_set_fixed_and_shared(
        &mut self,
        fixed_and_shared: CampaignBytes,
    ) -> Result<(), CampaignAdmissionError> {
        let mut state = self.shared.lock();
        state.ensure_quiescent()?;
        let next_baseline = CampaignBaselineMemory::try_new(
            fixed_and_shared,
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
        let next_baseline = CampaignBaselineMemory::try_new(
            state.fixed_and_shared,
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

    pub fn try_reserve_wave(
        &mut self,
        snapshot: &CampaignAdmissionSnapshot,
        plan: &CampaignResourceWavePlan,
        requests: &BTreeMap<CampaignJobKey, CampaignTaskResourceEstimate>,
    ) -> Result<CampaignWaveReservation, CampaignAdmissionError> {
        self.try_reserve_wave_with_predecessors(snapshot, plan, requests, &BTreeMap::new())
    }

    /// Revalidate and atomically charge one complete statically selected wave.
    ///
    /// `predecessors` contains opaque, non-owning tokens for jobs whose output
    /// will replace an existing resident owner. The actual move-only resident
    /// is still required at commit. Omitting a token declares an initial
    /// resident output; uniqueness of that declaration belongs to the campaign
    /// workspace until it is integrated with this resource layer.
    pub fn try_reserve_wave_with_predecessors(
        &mut self,
        snapshot: &CampaignAdmissionSnapshot,
        plan: &CampaignResourceWavePlan,
        requests: &BTreeMap<CampaignJobKey, CampaignTaskResourceEstimate>,
        predecessors: &BTreeMap<CampaignJobKey, CampaignResidentToken>,
    ) -> Result<CampaignWaveReservation, CampaignAdmissionError> {
        if !snapshot.belongs_to(&self.shared) {
            return Err(CampaignAdmissionError::ForeignSnapshot);
        }
        let replayed = CampaignWavePlanner::try_plan(snapshot.policy, requests)?;
        if &replayed != plan {
            return Err(CampaignAdmissionError::WavePlanMismatch);
        }

        let mut tasks = Vec::new();
        tasks.try_reserve_exact(plan.jobs().len()).map_err(|_| {
            CampaignAdmissionError::AllocationFailure {
                resource: "campaign task reservations",
                requested: plan.jobs().len(),
            }
        })?;
        let mut sealed_cores = 0usize;
        let mut sealed_memory = CampaignBytes::ZERO;
        for job in plan.jobs() {
            let request = requests
                .get(job)
                .copied()
                .ok_or_else(|| CampaignAdmissionError::MissingTaskEstimate { job: job.clone() })?;
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
            let predecessor = predecessors.get(job).cloned();
            if let Some(token) = &predecessor {
                token.validate_for(&self.shared, job)?;
            }
            tasks.push(CampaignTaskReservation::inactive(
                Arc::clone(&self.shared),
                job.clone(),
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
        if let Some((job, _)) = predecessors
            .iter()
            .find(|(job, _)| plan.jobs().binary_search(job).is_err())
        {
            return Err(CampaignAdmissionError::UnexpectedPredecessorToken { job: job.clone() });
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
            tasks,
        })
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
        Ok(CampaignBaselineMemory::try_new(
            self.fixed_and_shared,
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
    job: CampaignJobKey,
    generation: NonZeroU64,
}

impl CampaignResidentToken {
    pub const fn job(&self) -> &CampaignJobKey {
        &self.job
    }

    pub const fn generation(&self) -> u64 {
        self.generation.get()
    }

    fn validate_for(
        &self,
        shared: &Arc<CampaignAdmissionShared>,
        job: &CampaignJobKey,
    ) -> Result<(), CampaignAdmissionError> {
        if !Weak::ptr_eq(&self.authority, &Arc::downgrade(shared)) {
            return Err(CampaignAdmissionError::ForeignResidentToken { job: job.clone() });
        }
        if &self.job != job {
            return Err(CampaignAdmissionError::ResidentJobMismatch {
                expected: job.clone(),
                actual: self.job.clone(),
            });
        }
        Ok(())
    }

    fn exact_match(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.authority, &other.authority)
            && self.job == other.job
            && self.generation == other.generation
    }
}

impl fmt::Debug for CampaignResidentToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignResidentToken")
            .field("job", &self.job)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

pub struct CampaignWaveReservation {
    schema: &'static str,
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

/// One move-only task charge split from an atomically reserved wave.
pub struct CampaignTaskReservation {
    shared: Arc<CampaignAdmissionShared>,
    job: CampaignJobKey,
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
        job: CampaignJobKey,
        request: CampaignTaskResourceEstimate,
        predecessor: Option<CampaignResidentToken>,
        successor_token: u64,
    ) -> Self {
        Self {
            shared,
            job,
            request,
            predecessor,
            successor_token,
            remaining_memory: request.memory().peak_additional(),
            retained_transferred: false,
            active: false,
            _not_sync: PhantomData,
        }
    }

    pub const fn job(&self) -> &CampaignJobKey {
        &self.job
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
            job: self.job.clone(),
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
            .field("job", &self.job)
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
        if let Some(actual) = predecessor {
            actual
                .token()
                .validate_for(&reservation.shared, &reservation.job)?;
        }
        let expected = reservation.predecessor.as_ref();
        let actual = predecessor.map(CampaignResident::token);
        match (expected, actual) {
            (None, None) => Ok(()),
            (Some(expected), Some(actual)) if expected.exact_match(actual) => Ok(()),
            (Some(expected), Some(actual)) => {
                Err(CampaignAdmissionError::ResidentGenerationMismatch {
                    job: expected.job.clone(),
                    expected: expected.generation.get(),
                    actual: Some(actual.generation.get()),
                })
            }
            (Some(expected), None) => Err(CampaignAdmissionError::ResidentGenerationMismatch {
                job: expected.job.clone(),
                expected: expected.generation.get(),
                actual: None,
            }),
            (None, Some(actual)) => Err(CampaignAdmissionError::UnexpectedResidentPredecessor {
                job: actual.job.clone(),
                generation: actual.generation.get(),
            }),
        }
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
        let next_baseline = CampaignBaselineMemory::try_new(
            state.fixed_and_shared,
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CampaignAdmissionError {
    ForeignSnapshot,
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
        job: CampaignJobKey,
    },
    UnexpectedPredecessorToken {
        job: CampaignJobKey,
    },
    ForeignResidentToken {
        job: CampaignJobKey,
    },
    ResidentJobMismatch {
        expected: CampaignJobKey,
        actual: CampaignJobKey,
    },
    ResidentGenerationMismatch {
        job: CampaignJobKey,
        expected: u64,
        actual: Option<u64>,
    },
    UnexpectedResidentPredecessor {
        job: CampaignJobKey,
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
            Self::ForeignSnapshot => {
                formatter.write_str("campaign admission snapshot belongs to another controller")
            }
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
            Self::MissingTaskEstimate { job } => write!(
                formatter,
                "campaign wave has no resource estimate for sector {}",
                job.sector()
            ),
            Self::UnexpectedPredecessorToken { job } => write!(
                formatter,
                "campaign predecessor token was supplied for unselected sector {}",
                job.sector()
            ),
            Self::ForeignResidentToken { job } => write!(
                formatter,
                "resident token for sector {} belongs to another admission controller",
                job.sector()
            ),
            Self::ResidentJobMismatch { expected, actual } => write!(
                formatter,
                "resident sector {} does not match expected sector {}",
                actual.sector(),
                expected.sector()
            ),
            Self::ResidentGenerationMismatch {
                job,
                expected,
                actual,
            } => write!(
                formatter,
                "resident sector {} generation {:?} does not match sealed predecessor generation {expected}",
                job.sector(),
                actual
            ),
            Self::UnexpectedResidentPredecessor { job, generation } => write!(
                formatter,
                "initial resident commit for sector {} unexpectedly received generation {generation}",
                job.sector()
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
