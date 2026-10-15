use super::super::{
    CandidateRoutedError, CandidateRoutedFrontier, CandidateRoutedFrontierReason,
    CandidateRoutedTraceReport,
};
use super::*;
use crate::reduction::ReductionLimits;
use std::collections::{BTreeSet, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Instant;

pub(super) type Failure = CandidateRoutedCampaignFailure;
pub(super) type Work<const N: usize> = CandidateRoutedWork<N>;

#[path = "membership.rs"]
pub(super) mod membership;
use crate::persistence::finite_work::{
    GeneratedPageProvider, GeneratedWorkKey, TrustedGeneratedWorkDirectory,
};
use membership::Membership;

#[derive(Clone, Copy)]
pub(super) enum CompletedBaseline<'a, const N: usize> {
    Memory(&'a Membership<N>),
    Generated {
        directory: &'a TrustedGeneratedWorkDirectory,
        provider: &'a dyn GeneratedPageProvider,
    },
}

/// Bounds publication work and transient child staging, not native expansion.
/// Hash-table/queue growth can occasionally reallocate more than one batch.
pub(super) const PUBLICATION_BATCH_SIZE: usize = 256;

// Ordered markers, never a reordered/grouped-by-shard queue. Only preprobe_batch
// constructs these, after the same bounded-batch and intrinsic input admission.
enum ProbedWork<const N: usize> {
    KnownPresent,
    CompletedBaseline,
    NeedsCommit { node: Work<N>, shard: usize },
    Invalid(Failure),
}
struct ProbedBatch<const N: usize>(Vec<ProbedWork<N>>);

pub(super) fn limit(
    value: usize,
    addition: usize,
    maximum: usize,
    resource: &'static str,
) -> Result<usize, Failure> {
    let requested = value
        .checked_add(addition)
        .ok_or_else(|| resource_error(resource, usize::MAX, maximum))?;
    if requested > maximum {
        return Err(resource_error(resource, requested, maximum));
    }
    Ok(requested)
}
fn resource_error(resource: &'static str, requested: usize, limit: usize) -> Failure {
    CandidateRoutedError::ResourceLimit {
        resource,
        requested,
        limit,
    }
    .into()
}

struct State<const N: usize> {
    queue: VecDeque<Work<N>>,
    // Updated atomically with first-key admission, so snapshots never acquire
    // every membership shard. Operational phases remain counted in scheduled.
    physical_seen: usize,
    scheduled: usize,
    active: BTreeSet<Work<N>>,
    trace: CandidateRoutedTraceReport<N>,
    completed: usize,
    failed: usize,
    dedup: usize,
    baseline_hits: usize,
    rule_attempts: usize,
    coalescing: usize,
    reserved_coalescing: usize,
    missing_owners: usize,
    missing_rules: usize,
    failure: Option<Failure>,
    first_failure_work: Option<Work<N>>,
    census: Option<CandidateRoutedTraceCensus>,
}

pub(super) struct Shared<'a, const N: usize> {
    state: Mutex<State<N>>,
    membership: Membership<N>,
    completed_baseline: Option<CompletedBaseline<'a, N>>,
    wake: Condvar,
    progress: Condvar,
    start: Instant,
    workers: usize,
    cancellation: &'a AtomicBool,
    pub(super) limits: super::super::RoutedCandidateLimits,
    pub(super) reduction: ReductionLimits,
    census_limits: Option<CandidateRoutedTraceCensusLimits>,
}
impl<'a, const N: usize> Shared<'a, N> {
    pub(super) fn new(
        reducer: &RoutedCandidateReducer<N>,
        workers: usize,
        cancellation: &'a AtomicBool,
    ) -> Self {
        let mut trace = CandidateRoutedTraceReport::default();
        trace.family_fingerprint = reducer.programs.context.family.fingerprint_owner();
        Self {
            state: Mutex::new(State {
                queue: VecDeque::new(),
                physical_seen: 0,
                scheduled: 0,
                active: BTreeSet::new(),
                trace,
                completed: 0,
                failed: 0,
                dedup: 0,
                baseline_hits: 0,
                rule_attempts: 0,
                coalescing: 0,
                reserved_coalescing: 0,
                missing_owners: 0,
                missing_rules: 0,
                failure: None,
                first_failure_work: None,
                census: None,
            }),
            membership: Membership::new(),
            completed_baseline: None,
            wake: Condvar::new(),
            progress: Condvar::new(),
            start: Instant::now(),
            workers,
            cancellation,
            limits: reducer.limits,
            reduction: reducer.programs.context.limits,
            census_limits: None,
        }
    }
    pub(super) fn set_completed_baseline(&mut self, baseline: Option<CompletedBaseline<'a, N>>) {
        self.completed_baseline = baseline;
    }
    pub(super) fn apply_budget(&mut self, budget: CandidateRoutedWorkBudget) {
        self.limits.max_input_targets = self.limits.max_input_targets.min(budget.max_nodes);
        self.limits.max_unique_nodes = self.limits.max_unique_nodes.min(budget.max_nodes);
        self.limits.max_transport_calls = self
            .limits
            .max_transport_calls
            .min(budget.max_transport_calls);
        self.limits.max_transport_operations = self
            .limits
            .max_transport_operations
            .min(budget.max_transport_operations);
        self.limits.max_transport_endpoints = self
            .limits
            .max_transport_endpoints
            .min(budget.max_transport_endpoints);
        self.reduction.max_pending_frames = self.reduction.max_pending_frames.min(budget.max_nodes);
        self.reduction.max_rule_applications = self
            .reduction
            .max_rule_applications
            .min(budget.max_rule_applications);
        self.reduction.max_coalescing_additions = self
            .reduction
            .max_coalescing_additions
            .min(budget.max_coalescing_additions);
    }
    pub(super) fn enable_census(&mut self, limits: CandidateRoutedTraceCensusLimits) {
        self.census_limits = Some(limits);
        self.state
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .census = Some(CandidateRoutedTraceCensus::default());
    }
    pub(super) fn census_enabled(&self) -> bool {
        self.census_limits.is_some()
    }
    pub(super) fn selected_rule(
        &self,
        owner: &[bool],
        batch: usize,
        baseline: usize,
        selected: usize,
    ) -> Result<(), Failure> {
        let Some(limits) = self.census_limits else {
            return Ok(());
        };
        let mut state = self.lock();
        self.check_locked(&mut state)?;
        state
            .census
            .as_mut()
            .expect("enabled trace census")
            .record(owner, batch, baseline, selected, limits)
            .map_err(Failure::from)
    }
    fn lock(&self) -> MutexGuard<'_, State<N>> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn check_locked(&self, state: &mut State<N>) -> Result<(), Failure> {
        if self.cancellation.load(Ordering::Acquire) && state.failure.is_none() {
            state.failure = Some(Failure::Cancelled);
            self.wake.notify_all();
            self.progress.notify_one();
        }
        match &state.failure {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
    pub(super) fn check(&self) -> Result<(), Failure> {
        self.check_locked(&mut self.lock())
    }
    /// Cheap cancellation checks while staging a bounded batch outside the
    /// mutex. Peer failures are checked at each chunk and each locked admission.
    pub(super) fn check_cancellation(&self) -> Result<(), Failure> {
        if self.cancellation.load(Ordering::Acquire) {
            self.check()?;
        }
        Ok(())
    }
    pub(super) fn publication_capacity(&self) -> usize {
        PUBLICATION_BATCH_SIZE
            .min(self.limits.max_unique_nodes.max(1))
            .min(self.reduction.max_pending_frames.max(1))
    }
    pub(super) fn fail(&self, error: Failure) {
        self.lock().failure.get_or_insert(error);
        self.wake.notify_all();
        self.progress.notify_one();
    }
    #[cfg(test)]
    pub(super) fn prepare(
        &self,
        reducer: &RoutedCandidateReducer<N>,
        targets: impl IntoIterator<Item = IntegralKey>,
    ) -> Result<(), Failure> {
        self.prepare_with_entry_admission(
            reducer,
            targets,
            CandidateEntryAdmission::SavedGenerationScope,
        )
    }
    pub(super) fn prepare_with_entry_admission(
        &self,
        reducer: &RoutedCandidateReducer<N>,
        targets: impl IntoIterator<Item = IntegralKey>,
        admission: CandidateEntryAdmission<'_, N>,
    ) -> Result<(), Failure> {
        if !(1..=64).contains(&self.workers) {
            return Err(CandidateRoutedError::InvalidInput(
                "campaign workers must be in 1..=64".into(),
            )
            .into());
        }
        if cfg!(target_arch = "wasm32") && self.workers != 1 {
            return Err(CandidateRoutedError::InvalidInput(
                "WebAssembly campaigns require exactly one inline worker".into(),
            )
            .into());
        }
        let evaluator = worker::base(reducer);
        let mut entries = BTreeSet::new();
        for target in targets {
            self.check()?;
            {
                let mut state = self.lock();
                state.trace.input_targets = limit(
                    state.trace.input_targets,
                    1,
                    self.limits.max_input_targets,
                    "input targets",
                )?;
            }
            evaluator.validate_target(&target)?;
            admission.validate_entry(&target, reducer.programs.context.scope.max_numerator_rank)?;
            if !entries.contains(&target) {
                limit(
                    entries.len(),
                    1,
                    self.limits.max_unique_nodes,
                    "initial operational nodes",
                )?;
                limit(
                    entries.len(),
                    1,
                    self.reduction.max_pending_frames,
                    "initial pending nodes",
                )?;
                entries.insert(target);
            } else {
                let mut state = self.lock();
                state.dedup = limit(state.dedup, 1, usize::MAX, "deduplication counter")?;
            }
        }
        self.lock().trace.requested_targets = entries.len();
        for target in entries {
            self.schedule(Work::Route(target))?;
        }
        Ok(())
    }
    /// Caller validates the edge before this identity lookup. A shared in-flight
    /// join is not a cycle; support/phase/order descent proves acyclicity.
    pub(super) fn schedule(&self, node: Work<N>) -> Result<(), Failure> {
        // Generated pages can block on storage. Validate and probe outside the
        // scheduler lock, then recheck cancellation/live membership under it.
        // Preserve the existing memory-only ordering and cheap locked probe.
        let generated_hit = if matches!(
            self.completed_baseline,
            Some(CompletedBaseline::Generated { .. })
        ) {
            self.check()?;
            Self::validate_node(&node)?;
            Some(self.baseline_contains(&node)?)
        } else {
            None
        };
        let mut state = self.lock();
        self.check_locked(&mut state)?;
        Self::validate_node(&node)?;
        if match generated_hit {
            Some(hit) => hit,
            None => self.baseline_contains(&node)?,
        } {
            Self::baseline_hit(&mut state)?;
            return Ok(());
        }
        let shard = self.membership.shard_index(&node);
        let admitted = self.schedule_locked(&mut state, node, shard)?;
        drop(state);
        if admitted {
            self.wake.notify_all();
        }
        Ok(())
    }
    fn baseline_contains(&self, node: &Work<N>) -> Result<bool, Failure> {
        match self.completed_baseline {
            None => Ok(false),
            Some(CompletedBaseline::Memory(baseline)) => Ok(baseline.probe(node).1),
            Some(CompletedBaseline::Generated {
                directory,
                provider,
            }) => {
                let key = GeneratedWorkKey::from_native(node).map_err(Failure::Backing)?;
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    directory.lookup(&key, provider)
                }))
                .map_err(|_| Failure::WorkerPanicked)?
                .map(|result| result.present)
                .map_err(Failure::Backing)
            }
        }
    }
    fn baseline_hit(state: &mut State<N>) -> Result<(), Failure> {
        state.baseline_hits = limit(
            state.baseline_hits,
            1,
            usize::MAX,
            "completed baseline hit counter",
        )?;
        Ok(())
    }
    /// The Vec contains already-validated children, not a lazy iterator that
    /// could execute native validation while holding the scheduler mutex.
    pub(super) fn schedule_batch(&self, nodes: Vec<Work<N>>) -> Result<(), Failure> {
        let batch = self.preprobe_batch(nodes)?;
        self.commit_batch(batch)
    }
    fn preprobe_batch(&self, nodes: Vec<Work<N>>) -> Result<ProbedBatch<N>, Failure> {
        limit(
            0,
            nodes.len(),
            self.publication_capacity(),
            "publication batch",
        )?;
        let mut batch = Vec::with_capacity(nodes.len());
        for node in nodes {
            // Intrinsic validation is before membership, even for a duplicate.
            // Keep an error marker instead of returning early: earlier valid
            // nodes must still commit exactly their old prefix.
            if let Err(error) = Self::validate_node(&node) {
                batch.push(ProbedWork::Invalid(error));
                break;
            }
            match self.baseline_contains(&node) {
                Ok(true) => {
                    batch.push(ProbedWork::CompletedBaseline);
                    continue;
                }
                Ok(false) => {}
                Err(error) => {
                    batch.push(ProbedWork::Invalid(error));
                    break;
                }
            }
            let (shard, present) = self.membership.probe(&node);
            if present {
                batch.push(ProbedWork::KnownPresent);
                // Discard duplicate key storage outside the global state lock.
            } else {
                batch.push(ProbedWork::NeedsCommit { node, shard });
            }
        }
        Ok(ProbedBatch(batch))
    }
    fn commit_batch(&self, batch: ProbedBatch<N>) -> Result<(), Failure> {
        let mut state = self.lock();
        let mut published = false;
        let mut result = Ok(());
        for marker in batch.0 {
            // Do not aggregate duplicate counters ahead of a possible cap,
            // invalid-input or cancellation failure later in this same batch.
            let admitted = self.check_locked(&mut state).and_then(|()| match marker {
                ProbedWork::CompletedBaseline => {
                    Self::baseline_hit(&mut state)?;
                    Ok(false)
                }
                ProbedWork::KnownPresent => {
                    state.dedup = limit(state.dedup, 1, usize::MAX, "deduplication counter")?;
                    Ok(false)
                }
                ProbedWork::NeedsCommit { node, shard } => {
                    self.schedule_locked(&mut state, node, shard)
                }
                ProbedWork::Invalid(error) => Err(error),
            });
            match admitted {
                Ok(admitted) => published |= admitted,
                Err(error) => {
                    result = Err(error);
                    break;
                }
            }
        }
        drop(state);
        // A valid prefix must wake workers even when a later node hit a cap.
        if published {
            self.wake.notify_all();
        }
        result
    }
    fn validate_node(node: &Work<N>) -> Result<(), Failure> {
        let key = node.target();
        if !crate::fits_storage(key.powers().len(), N) {
            return Err(CandidateRoutedError::InvalidInput(
                "scheduled work has the wrong integral arity".into(),
            )
            .into());
        }
        if let Work::Apply { owner_sector, .. } = node {
            if key
                .powers()
                .iter()
                .zip(owner_sector)
                .any(|(&power, &active)| (power > 0) != active)
            {
                return Err(CandidateRoutedError::InvalidInput(
                    "scheduled apply owner differs from integral support".into(),
                )
                .into());
            }
        }
        Ok(())
    }
    fn schedule_locked(
        &self,
        state: &mut State<N>,
        node: Work<N>,
        shard_index: usize,
    ) -> Result<bool, Failure> {
        // The only nesting order is global state -> one membership shard.
        // A preprobe never retains its shard while waiting for global state.
        // Recheck absent hints: another batch or an earlier slot can now own it.
        let mut shard = self.membership.lock_shard(shard_index);
        if shard.contains(&node) {
            state.dedup = limit(state.dedup, 1, usize::MAX, "deduplication counter")?;
            return Ok(false);
        }
        let scheduled = limit(
            state.scheduled,
            1,
            self.limits.max_unique_nodes,
            "operational nodes",
        )?;
        // Pending includes executing nodes and queued work, not DFS leave frames.
        limit(
            state.queue.len() + state.active.len(),
            1,
            self.reduction.max_pending_frames,
            "pending nodes",
        )?;
        let new_physical = shard.insert(&node);
        // physical_seen <= old scheduled < admitted scheduled, so this cannot
        // overflow or introduce a separate unadmitted resource dimension.
        state.physical_seen += usize::from(new_physical);
        state.scheduled = scheduled;
        state.queue.push_back(node);
        Ok(true)
    }
    pub(super) fn take(&self) -> Option<Work<N>> {
        let mut state = self.lock();
        loop {
            if self.check_locked(&mut state).is_err() {
                return None;
            }
            if let Some(node) = state.queue.pop_front() {
                state.active.insert(node.clone());
                return Some(node);
            }
            if state.active.is_empty() {
                return None;
            }
            state = self
                .wake
                .wait_timeout(state, Duration::from_millis(100))
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }
    pub(super) fn finish(&self, node: Work<N>, result: Result<(), Failure>) {
        let mut state = self.lock();
        state.active.remove(&node);
        let already_failed = state.failure.is_some();
        match result {
            Ok(()) => state.completed += 1,
            Err(error) => {
                state.failed += 1;
                if state.failure.is_none() {
                    state.first_failure_work = Some(node);
                }
                state.failure.get_or_insert(error);
            }
        }
        self.check_locked(&mut state).ok();
        self.wake.notify_all();
        if Self::done(&state) || (!already_failed && state.failure.is_some()) {
            self.progress.notify_one();
        }
    }
    pub(super) fn degrees(&self, key: &IntegralKey) -> Result<(), Failure> {
        let mut rank = 0u128;
        let mut dots = 0u128;
        for &power in key.powers() {
            let (value, addition) = if power < 0 {
                (&mut rank, u128::from(power.unsigned_abs()))
            } else {
                (&mut dots, u128::from((power as u64).saturating_sub(1)))
            };
            *value = value.checked_add(addition).ok_or_else(|| {
                Failure::from(CandidateRoutedError::InvalidInput(
                    "routed degree census overflow".into(),
                ))
            })?;
        }
        let mut state = self.lock();
        state.trace.max_numerator_rank = state.trace.max_numerator_rank.max(rank);
        state.trace.max_dot_excess = state.trace.max_dot_excess.max(dots);
        Ok(())
    }
    pub(super) fn zero(&self, key: &IntegralKey) {
        self.lock().trace.visited_zeros.insert(key.clone());
    }
    pub(super) fn terminal(&self, key: &IntegralKey) {
        self.lock().trace.declared_terminals.insert(key.clone());
    }
    pub(super) fn frontier(&self, target: IntegralKey, reason: CandidateRoutedFrontierReason<N>) {
        let mut state = self.lock();
        let missing_owner = matches!(reason, CandidateRoutedFrontierReason::MissingOwner);
        if state
            .trace
            .frontier
            .insert(CandidateRoutedFrontier { target, reason })
        {
            if missing_owner {
                state.missing_owners += 1;
            } else {
                state.missing_rules += 1;
            }
        }
    }
    pub(super) fn transport_call(&self) -> Result<(), Failure> {
        let mut state = self.lock();
        self.check_locked(&mut state)?;
        state.trace.transport_calls = limit(
            state.trace.transport_calls,
            1,
            self.limits.max_transport_calls,
            "transport calls",
        )?;
        Ok(())
    }
    pub(super) fn transport_usage(
        &self,
        operations: usize,
        endpoints: usize,
    ) -> Result<(), crate::sector::symmetry::integral_transport::ExpansionError> {
        use crate::sector::symmetry::integral_transport::ExpansionError;
        let mut state = self.lock();
        // The callback is before native expansion. Atomic admission of both
        // work dimensions; one failed dimension consumes neither allowance.
        let add = |old: usize, amount: usize, maximum: usize, resource| {
            let requested = old
                .checked_add(amount)
                .ok_or(ExpansionError::ResourceCountOverflow { resource })?;
            if requested > maximum {
                return Err(ExpansionError::ResourceLimit {
                    resource,
                    requested,
                    limit: maximum,
                });
            }
            Ok(requested)
        };
        let ops = add(
            state.trace.transport_operations,
            operations,
            self.limits.max_transport_operations,
            "aggregate routed operations",
        )?;
        let ends = add(
            state.trace.transport_endpoints,
            endpoints,
            self.limits.max_transport_endpoints,
            "aggregate routed endpoints",
        )?;
        state.trace.transport_operations = ops;
        state.trace.transport_endpoints = ends;
        Ok(())
    }
    pub(super) fn begin_apply(&self) -> Result<(), Failure> {
        let mut state = self.lock();
        self.check_locked(&mut state)?;
        state.rule_attempts = limit(
            state.rule_attempts,
            1,
            self.reduction.max_rule_applications,
            "rule attempts",
        )?;
        Ok(())
    }
    pub(super) fn reserve_apply(&self, bound: usize) -> Result<(), Failure> {
        let mut state = self.lock();
        loop {
            self.check_locked(&mut state)?;
            let needed = limit(
                state.coalescing,
                bound,
                self.reduction.max_coalescing_additions,
                "conservative coalescing reservation",
            )?;
            if state.reserved_coalescing <= self.reduction.max_coalescing_additions - needed {
                state.reserved_coalescing += bound;
                return Ok(());
            }
            state = self
                .wake
                .wait_timeout(state, Duration::from_millis(100))
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }
    pub(super) fn settle_apply(
        &self,
        bound: usize,
        actual: usize,
        successful: bool,
    ) -> Result<(), Failure> {
        let mut state = self.lock();
        state.reserved_coalescing -= bound;
        if actual > bound {
            self.wake.notify_all();
            return Err(CandidateRoutedError::InvalidInput(
                "coalescing exceeded its duplicate-shift bound".into(),
            )
            .into());
        }
        state.coalescing = limit(
            state.coalescing,
            actual,
            self.reduction.max_coalescing_additions,
            "coalescing additions",
        )?;
        if successful {
            state.trace.rule_applications += 1;
        }
        self.wake.notify_all();
        Ok(())
    }
    fn done(state: &State<N>) -> bool {
        state.active.is_empty() && (state.queue.is_empty() || state.failure.is_some())
    }
    fn snapshot_locked(&self, state: &State<N>) -> CandidateRoutedCampaignSnapshot<N> {
        CandidateRoutedCampaignSnapshot {
            elapsed: self.start.elapsed(),
            workers: self.workers,
            input_targets: state.trace.input_targets,
            requested_targets: state.trace.requested_targets,
            scheduled_nodes: state.scheduled,
            queued_nodes: state.queue.len(),
            active_nodes: state.active.len(),
            completed_nodes: state.completed,
            failed_nodes: state.failed,
            deduplication_hits: state.dedup,
            completed_baseline_hits: state.baseline_hits,
            reachable_integrals: state.physical_seen,
            rule_attempts: state.rule_attempts,
            rule_applications: state.trace.rule_applications,
            transport_calls: state.trace.transport_calls,
            transport_operations: state.trace.transport_operations,
            transport_endpoints: state.trace.transport_endpoints,
            coalescing_additions: state.coalescing,
            reserved_coalescing_additions: state.reserved_coalescing,
            declared_terminals: state.trace.declared_terminals.len(),
            visited_zeros: state.trace.visited_zeros.len(),
            missing_owners: state.missing_owners,
            missing_rules: state.missing_rules,
            max_numerator_rank: state.trace.max_numerator_rank,
            max_dot_excess: state.trace.max_dot_excess,
            active: state.active.iter().cloned().collect(),
            first_failure: state.failure.clone(),
            first_failure_work: state.first_failure_work.clone(),
            finished: Self::done(state) && state.failure.is_none(),
        }
    }
    pub(super) fn snapshot(&self) -> CandidateRoutedCampaignSnapshot<N> {
        self.snapshot_locked(&self.lock())
    }
    pub(super) fn wait_snapshot(
        &self,
        interval: Duration,
    ) -> (CandidateRoutedCampaignSnapshot<N>, bool) {
        let deadline = Instant::now() + interval;
        let mut state = self.lock();
        let already_failed = state.failure.is_some();
        loop {
            self.check_locked(&mut state).ok();
            if Self::done(&state)
                || (!already_failed && state.failure.is_some())
                || Instant::now() >= deadline
            {
                return (self.snapshot_locked(&state), Self::done(&state));
            }
            state = self
                .progress
                .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }
    pub(super) fn into_result(
        self,
    ) -> Result<CandidateRoutedCampaignReport<N>, CandidateRoutedCampaignError<N>> {
        self.into_result_with_membership().map(|(report, _)| report)
    }
    pub(super) fn into_result_with_membership(
        self,
    ) -> Result<(CandidateRoutedCampaignReport<N>, Membership<N>), CandidateRoutedCampaignError<N>>
    {
        {
            let mut state = self.lock();
            self.check_locked(&mut state).ok();
            if state.failure.is_none()
                && (!Self::done(&state)
                    || state.completed != state.scheduled
                    || state.reserved_coalescing != 0)
            {
                state.failure = Some(
                    CandidateRoutedError::InvalidInput(
                        "campaign returned before its worklist drained".into(),
                    )
                    .into(),
                );
            }
        }
        let snapshot = self.snapshot();
        let mut state = self
            .state
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.trace.operational_nodes = state.scheduled;
        state.trace.reachable_integrals = state.physical_seen;
        let report = CandidateRoutedCampaignReport {
            trace: state.trace,
            snapshot,
            census: state.census,
        };
        match state.failure {
            Some(reason) => Err(CandidateRoutedCampaignError {
                reason,
                partial: report,
            }),
            None => Ok((report, self.membership)),
        }
    }
}

#[cfg(test)]
#[path = "membership_tests.rs"]
mod membership_tests;
