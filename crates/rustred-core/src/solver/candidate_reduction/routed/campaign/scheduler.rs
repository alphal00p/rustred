use super::super::{
    CandidateRoutedError, CandidateRoutedFrontier, CandidateRoutedFrontierReason,
    CandidateRoutedTraceReport,
};
use super::*;
use crate::reduction::ReductionLimits;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Instant;

pub(super) type Failure = CandidateRoutedCampaignFailure;
pub(super) type Work<const N: usize> = CandidateRoutedWork<N>;

/// Bounds publication work and transient child staging, not native expansion.
/// Hash-table/queue growth can occasionally reallocate more than one batch.
pub(super) const PUBLICATION_BATCH_SIZE: usize = 256;

#[derive(Default)]
struct SeenPhases<const N: usize> {
    route: bool,
    apply_owner: Option<[bool; N]>,
}
impl<const N: usize> SeenPhases<N> {
    fn contains(&self, node: &Work<N>) -> bool {
        match node {
            Work::Route(_) => self.route,
            Work::Apply { owner_sector, .. } => self.apply_owner.as_ref() == Some(owner_sector),
        }
    }
    fn insert(&mut self, node: &Work<N>) {
        match node {
            Work::Route(_) => self.route = true,
            Work::Apply { owner_sector, .. } => self.apply_owner = Some(*owner_sector),
        }
    }
}

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
    // Membership only: hash iteration never determines traversal or reports.
    // One physical key can have Route and its exact positive-support Apply.
    seen: HashMap<IntegralKey, SeenPhases<N>>,
    scheduled: usize,
    active: BTreeSet<Work<N>>,
    trace: CandidateRoutedTraceReport<N>,
    completed: usize,
    failed: usize,
    dedup: usize,
    rule_attempts: usize,
    coalescing: usize,
    reserved_coalescing: usize,
    missing_owners: usize,
    missing_rules: usize,
    failure: Option<Failure>,
    first_failure_work: Option<Work<N>>,
}

pub(super) struct Shared<'a, const N: usize> {
    state: Mutex<State<N>>,
    wake: Condvar,
    progress: Condvar,
    start: Instant,
    workers: usize,
    cancellation: &'a AtomicBool,
    pub(super) limits: super::super::RoutedCandidateLimits,
    pub(super) reduction: ReductionLimits,
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
                seen: HashMap::new(),
                scheduled: 0,
                active: BTreeSet::new(),
                trace,
                completed: 0,
                failed: 0,
                dedup: 0,
                rule_attempts: 0,
                coalescing: 0,
                reserved_coalescing: 0,
                missing_owners: 0,
                missing_rules: 0,
                failure: None,
                first_failure_work: None,
            }),
            wake: Condvar::new(),
            progress: Condvar::new(),
            start: Instant::now(),
            workers,
            cancellation,
            limits: reducer.limits,
            reduction: reducer.programs.context.limits,
        }
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
    pub(super) fn prepare(
        &self,
        reducer: &RoutedCandidateReducer<N>,
        targets: impl IntoIterator<Item = IntegralKey>,
    ) -> Result<(), Failure> {
        if !(1..=64).contains(&self.workers) {
            return Err(CandidateRoutedError::InvalidInput(
                "campaign workers must be in 1..=64".into(),
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
            crate::solver::candidate_reduction::evaluator::validate_entry_rank(
                &target,
                reducer.programs.context.scope.max_numerator_rank,
            )?;
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
        let mut state = self.lock();
        let admitted = self.schedule_locked(&mut state, node)?;
        drop(state);
        if admitted {
            self.wake.notify_all();
        }
        Ok(())
    }
    /// The Vec contains already-validated children, not a lazy iterator that
    /// could execute native validation while holding the scheduler mutex.
    pub(super) fn schedule_batch(&self, nodes: Vec<Work<N>>) -> Result<(), Failure> {
        limit(
            0,
            nodes.len(),
            self.publication_capacity(),
            "publication batch",
        )?;
        let mut state = self.lock();
        let mut published = false;
        let mut result = Ok(());
        for node in nodes {
            match self.schedule_locked(&mut state, node) {
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
    fn schedule_locked(&self, state: &mut State<N>, node: Work<N>) -> Result<bool, Failure> {
        self.check_locked(state)?;
        let key = node.target();
        if key.powers().len() != N {
            return Err(CandidateRoutedError::InvalidInput(
                "scheduled work has the wrong integral arity".into(),
            )
            .into());
        }
        if let Work::Apply { owner_sector, .. } = &node {
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
        let phases = state.seen.get_mut(key);
        if phases.as_ref().is_some_and(|phases| phases.contains(&node)) {
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
        if let Some(phases) = phases {
            phases.insert(&node);
        } else {
            let mut phases = SeenPhases::default();
            phases.insert(&node);
            state.seen.insert(key.clone(), phases);
        }
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
            reachable_integrals: state.seen.len(),
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
        state.trace.reachable_integrals = state.seen.len();
        let report = CandidateRoutedCampaignReport {
            trace: state.trace,
            snapshot,
        };
        match state.failure {
            Some(reason) => Err(CandidateRoutedCampaignError {
                reason,
                partial: report,
            }),
            None => Ok(report),
        }
    }
}
