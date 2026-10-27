use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::cache_policy::DependencyPins;
use super::guard_union::GuardUnion;
use super::progress::{
    CandidateRoutedActivity, CandidateRoutedApplicationEnvelope, CandidateRoutedApplicationEvent,
    CandidateRoutedApplicationMetrics, CandidateRoutedApplicationObserver,
    CandidateRoutedCompletionSource, CandidateRoutedComplexity, CandidateRoutedFrontierSample,
    CandidateRoutedKeyDegrees, CandidateRoutedStoredState, CandidateRoutedWorkRef, Progress,
};
use super::{
    CandidateRoutedApplicationPhase, CandidateRoutedApplicationSnapshot,
    CandidateRoutedApplicationStatistics, CandidateRoutedApplicationStatus,
    CandidateRoutedCacheCensus, CandidateRoutedReduction, RoutedCandidateApplication,
};
use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::family::IntegralKey;
use crate::reduction::{ReductionError, ReductionRequest, accumulate_master_in_request};
use crate::sector::symmetry::integral_transport::ExpansionError;
use crate::solver::candidate_reduction::cache::{self, CachedTerms};
use crate::solver::candidate_reduction::evaluator::CandidateEvaluator;
use crate::solver::candidate_reduction::owners::OwnerStep;
use crate::solver::candidate_reduction::routed::trace::{Node, charge, successor, support};
use crate::solver::candidate_reduction::routed::{CandidateEntryAdmission, CandidateRoutedError};
use crate::solver::candidate_reduction::{CandidateReductionError, UnresolvedPolicy};

enum Frame<const N: usize> {
    Enter(Node<N>),
    Combine {
        node: Node<N>,
        terms: Arc<BTreeMap<Node<N>, Coefficient>>,
        conditions: Vec<Coefficient>,
        first_visit: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::RoutedCandidateReducer;
    use crate::solver::candidate_reduction::owner_test_support::*;

    #[test]
    fn active_operational_cycle_is_rejected_even_with_a_cached_result() {
        let family = Arc::new(crate::solver::tests::tadpole());
        let programs = programs(
            family.clone(),
            None,
            vec![input([true], None, vec![], &[[1]])],
            Default::default(),
        );
        let mut application = RoutedCandidateReducer::try_new(programs, [], Default::default())
            .unwrap()
            .application();
        application
            .reduce_linear_combination([(key([1]), family.coefficient_context().one())])
            .unwrap();
        let node = Node::Route(key([1]));
        let mut request = Request::default();
        request.active.insert(node.clone());
        request.push(Frame::Enter(node), None).unwrap();
        assert!(matches!(
            application.run_stack(&mut request),
            Err(CandidateRoutedError::Cycle { .. })
        ));

        let mut cycle = None;
        {
            let mut callback = |event: &CandidateRoutedApplicationEnvelope<'_>| {
                if event.event == CandidateRoutedApplicationEvent::CycleDetected {
                    cycle = Some((
                        event.active_path.len(),
                        event.path_total_nodes,
                        event.cycle_backedge,
                        event.active_path.first() == event.active_path.last(),
                    ));
                }
                Ok(super::super::progress::CandidateRoutedObserverControl::Continue)
            };
            let mut observer = CandidateRoutedApplicationObserver::new(
                super::super::progress::CandidateRoutedObservationOptions {
                    interval: Duration::ZERO,
                    ..Default::default()
                },
                &mut callback,
            );
            let node = Node::Route(key([1]));
            let mut request = Request {
                progress: Progress::from_observer(&mut observer),
                observation: Some(EngineObservation {
                    path_limit: 8,
                    ..Default::default()
                }),
                ..Default::default()
            };
            request.set_root(&node);
            request.active.insert(node.clone());
            request.retain_observed_path(&node);
            request
                .push(
                    Frame::Combine {
                        node: node.clone(),
                        terms: Arc::new(BTreeMap::new()),
                        conditions: vec![],
                        first_visit: true,
                    },
                    None,
                )
                .unwrap();
            request.push(Frame::Enter(node), None).unwrap();
            assert!(matches!(
                application.run_stack(&mut request),
                Err(CandidateRoutedError::Cycle { .. })
            ));
        }
        assert_eq!(cycle, Some((2, 2, Some(0), true)));
    }

    #[test]
    fn incomplete_child_cache_is_an_error_at_combine() {
        let family = Arc::new(crate::solver::tests::tadpole());
        let programs = programs(
            family.clone(),
            None,
            vec![input([true], None, vec![], &[[1]])],
            Default::default(),
        );
        let mut application = RoutedCandidateReducer::try_new(programs, [], Default::default())
            .unwrap()
            .application();
        let node = Node::Route(key([1]));
        let mut request = Request::default();
        request.active.insert(node.clone());
        request
            .push(
                Frame::Combine {
                    node,
                    terms: Arc::new(BTreeMap::from([(
                        Node::Apply([true], key([1])),
                        family.coefficient_context().one(),
                    )])),
                    conditions: vec![],
                    first_visit: true,
                },
                None,
            )
            .unwrap();
        assert!(matches!(
            application.run_stack(&mut request),
            Err(CandidateRoutedError::Candidate(
                CandidateReductionError::Application(ReductionError::ReducerInvariant { .. })
            ))
        ));
        assert_eq!(application.cache_statistics().cached_integrals(), 0);
    }

    #[test]
    fn failed_combine_drops_only_the_executing_observed_map() {
        let family = Arc::new(crate::solver::tests::tadpole());
        let programs = programs(
            family.clone(),
            None,
            vec![input([true], None, vec![], &[[1]])],
            Default::default(),
        );
        let mut application = RoutedCandidateReducer::try_new(programs, [], Default::default())
            .unwrap()
            .application();
        let mut request = Request {
            observation: Some(EngineObservation::default()),
            ..Default::default()
        };
        let ancestor = Node::Route(key([2]));
        let current = Node::Route(key([1]));
        for node in [ancestor, current] {
            request.active.insert(node.clone());
            request
                .push(
                    Frame::Combine {
                        node,
                        terms: Arc::new(BTreeMap::from([(
                            Node::Apply([true], key([1])),
                            family.coefficient_context().one(),
                        )])),
                        conditions: vec![],
                        first_visit: true,
                    },
                    None,
                )
                .unwrap();
        }
        assert_eq!(
            request
                .observation
                .as_ref()
                .unwrap()
                .stored_state()
                .deferred_rule_maps,
            Some(2)
        );
        assert!(matches!(
            application.run_stack(&mut request),
            Err(CandidateRoutedError::Candidate(
                CandidateReductionError::Application(ReductionError::ReducerInvariant { .. })
            ))
        ));
        let state = request.observation.as_ref().unwrap().stored_state();
        assert_eq!(state.deferred_rule_maps, Some(1));
        assert_eq!(state.deferred_child_integral_occurrences, Some(1));
        assert_eq!(request.stack.len(), 1);
        assert_eq!(application.cache_statistics().cached_integrals(), 0);
    }

    #[test]
    fn observed_counter_overflow_is_unknown_and_adds_no_reduction_error() {
        let family = Arc::new(crate::solver::tests::tadpole());
        let programs = programs(
            family.clone(),
            None,
            vec![input([true], None, vec![], &[[1]])],
            Default::default(),
        );
        let mut application = RoutedCandidateReducer::try_new(programs, [], Default::default())
            .unwrap()
            .application();
        let mut request = Request {
            observation: Some(EngineObservation {
                stored_state: CandidateRoutedStoredState {
                    deferred_rule_maps: Some(usize::MAX),
                    deferred_child_integral_occurrences: Some(usize::MAX),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };
        request
            .push(
                Frame::Combine {
                    node: Node::Route(key([1])),
                    terms: Arc::new(BTreeMap::from([(
                        Node::Apply([true], key([1])),
                        family.coefficient_context().one(),
                    )])),
                    conditions: vec![],
                    first_visit: true,
                },
                None,
            )
            .unwrap();
        let state = request.observation.as_ref().unwrap().stored_state();
        assert_eq!(state.deferred_rule_maps, None);
        assert_eq!(state.deferred_child_integral_occurrences, None);
        // The deliberately missing child still produces the original math error.
        assert!(matches!(
            application.run_stack(&mut request),
            Err(CandidateRoutedError::Candidate(
                CandidateReductionError::Application(ReductionError::ReducerInvariant { .. })
            ))
        ));
        let state = request.observation.as_ref().unwrap().stored_state();
        assert_eq!(state.deferred_rule_maps, None);
        assert_eq!(state.deferred_child_integral_occurrences, None);
    }
}

/// Optional request-local metadata. It never owns coefficients or participates
/// in numerical work accounting. Only a bounded prefix of active ancestry is
/// retained, and no metadata is allocated for an unobserved request.
struct EngineObservation<const N: usize> {
    root: Option<Node<N>>,
    current: Option<Node<N>>,
    metrics: CandidateRoutedApplicationMetrics,
    activity: CandidateRoutedActivity,
    path_prefix: Vec<Node<N>>,
    path_limit: usize,
    complexity: CandidateRoutedComplexity,
    has_seen_degrees: bool,
    /// Queued Combine maps only; the currently popped map is added on delivery.
    stored_state: CandidateRoutedStoredState,
    combining_child_occurrences: Option<usize>,
}

impl<const N: usize> Default for EngineObservation<N> {
    fn default() -> Self {
        Self {
            root: None,
            current: None,
            metrics: CandidateRoutedApplicationMetrics::default(),
            activity: CandidateRoutedActivity::default(),
            path_prefix: Vec::new(),
            path_limit: 0,
            complexity: CandidateRoutedComplexity::default(),
            has_seen_degrees: false,
            stored_state: CandidateRoutedStoredState {
                deferred_rule_maps: Some(0),
                deferred_child_integral_occurrences: Some(0),
                ..Default::default()
            },
            combining_child_occurrences: None,
        }
    }
}

impl<const N: usize> EngineObservation<N> {
    fn stored_state(&self) -> CandidateRoutedStoredState {
        let mut state = self.stored_state;
        if let Some(children) = self.combining_child_occurrences {
            state.deferred_rule_maps = state.deferred_rule_maps.and_then(|n| n.checked_add(1));
            state.deferred_child_integral_occurrences = state
                .deferred_child_integral_occurrences
                .and_then(|n| n.checked_add(children));
        }
        state
    }
}

fn try_clone_node<const N: usize>(node: &Node<N>) -> Option<Node<N>> {
    let key = IntegralKey::try_new(node.key().powers().iter().copied()).ok()?;
    Some(match node {
        Node::Route(_) => Node::Route(key),
        Node::Apply(owner, _) => Node::Apply(*owner, key),
    })
}

fn work_ref<const N: usize>(node: &Node<N>, arity: usize) -> CandidateRoutedWorkRef<'_> {
    match node {
        Node::Route(target) => CandidateRoutedWorkRef::Route(target),
        Node::Apply(owner_sector, target) => CandidateRoutedWorkRef::Apply {
            owner_sector: &owner_sector[..arity],
            target,
        },
    }
}

#[derive(Default)]
pub(super) struct Request<'a, 'storage, const N: usize> {
    algebra: ReductionRequest,
    pub(super) stats: CandidateRoutedApplicationStatistics,
    seen: BTreeSet<Node<N>>,
    active: BTreeSet<Node<N>>,
    stack: Vec<Frame<N>>,
    pub(super) pins: DependencyPins<N>,
    progress: Option<Progress<'a>>,
    observation: Option<EngineObservation<N>>,
    cancel: Option<&'a AtomicBool>,
    pub(super) persistent: super::persistent::PersistentRequest<'a, 'storage>,
}

impl<'a, 'storage, const N: usize> Request<'a, 'storage, N> {
    fn set_root(&mut self, node: &Node<N>) {
        if let Some(observation) = &mut self.observation {
            observation.root = try_clone_node(node);
        }
        self.set_current(node);
    }

    fn set_current(&mut self, node: &Node<N>) {
        if let Some(observation) = &mut self.observation {
            let degrees = CandidateRoutedKeyDegrees::from_key(node.key());
            if !observation.has_seen_degrees {
                observation.metrics.max_seen_degrees = degrees;
                observation.has_seen_degrees = true;
            } else {
                observation.metrics.max_seen_degrees.include(degrees);
            }
            observation.current = try_clone_node(node);
            observation.complexity = CandidateRoutedComplexity::default();
        }
    }

    fn set_entry_root(&mut self, key: &IntegralKey) {
        if let Some(observation) = &mut self.observation {
            let degrees = CandidateRoutedKeyDegrees::from_key(key);
            if !observation.has_seen_degrees {
                observation.metrics.max_seen_degrees = degrees;
                observation.has_seen_degrees = true;
            } else {
                observation.metrics.max_seen_degrees.include(degrees);
            }
            observation.root = IntegralKey::try_new(key.powers().iter().copied())
                .ok()
                .map(Node::Route);
            observation.current = observation.root.as_ref().and_then(try_clone_node);
            observation.complexity = CandidateRoutedComplexity::default();
        }
    }

    fn record_completion(&mut self, first_visit: bool) {
        if let Some(observation) = &mut self.observation {
            observation.metrics.completion_attempts =
                observation.metrics.completion_attempts.saturating_add(1);
            if first_visit {
                observation.metrics.completed_unique_operational_nodes = observation
                    .metrics
                    .completed_unique_operational_nodes
                    .saturating_add(1);
            }
        }
    }

    fn record_local_descent(&mut self) {
        if let Some(observation) = &mut self.observation {
            observation.metrics.observed_descent_checks = observation
                .metrics
                .observed_descent_checks
                .saturating_add(1);
        }
    }

    fn retain_observed_path(&mut self, node: &Node<N>) {
        if let Some(observation) = &mut self.observation
            && self.active.len() <= observation.path_limit
        {
            if observation.path_prefix.try_reserve(1).is_ok()
                && let Some(node) = try_clone_node(node)
            {
                observation.path_prefix.push(node);
            } else {
                // Telemetry allocation refusal only omits detail.
                observation.path_prefix.clear();
                observation.path_limit = 0;
            }
        }
    }

    fn release_observed_path(&mut self) {
        if let Some(observation) = &mut self.observation {
            observation.path_prefix.truncate(self.active.len());
        }
    }

    fn observe_activity(
        &mut self,
        application: &RoutedCandidateApplication<N>,
        activity: CandidateRoutedActivity,
        terms: Option<usize>,
    ) {
        if let Some(observation) = &mut self.observation {
            observation.activity = activity;
            observation.complexity = CandidateRoutedComplexity {
                terms,
                ..Default::default()
            };
            self.observe_event(
                application,
                None,
                CandidateRoutedApplicationStatus::Running,
                true,
                CandidateRoutedApplicationEvent::ActivityStarted,
                false,
            );
        }
    }

    fn observe_completion(
        &mut self,
        application: &RoutedCandidateApplication<N>,
        source: CandidateRoutedCompletionSource,
    ) {
        if self.observation.is_some() {
            self.observe_event(
                application,
                None,
                CandidateRoutedApplicationStatus::Running,
                false,
                CandidateRoutedApplicationEvent::NodeCompleted(source),
                true,
            );
        }
    }

    pub(super) fn with_persistent(
        persistent: super::persistent::PersistentRequest<'a, 'storage>,
    ) -> Self {
        Self {
            persistent,
            ..Default::default()
        }
    }
    fn check_cancel(&self) -> Result<(), CandidateRoutedError> {
        symbolica::utils::resources::checkpoint().map_err(ReductionError::from)?;
        if self
            .cancel
            .is_some_and(|cancel| cancel.load(Ordering::Relaxed))
        {
            Err(CandidateRoutedError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn observe(
        &mut self,
        application: &RoutedCandidateApplication<N>,
        phase: Option<CandidateRoutedApplicationPhase>,
        status: CandidateRoutedApplicationStatus,
        force: bool,
    ) {
        if let (Some(observation), Some(phase)) = (&mut self.observation, phase) {
            observation.activity = match phase {
                CandidateRoutedApplicationPhase::Admission => CandidateRoutedActivity::Admission,
                CandidateRoutedApplicationPhase::Expansion => CandidateRoutedActivity::Expansion,
                CandidateRoutedApplicationPhase::Combination => {
                    CandidateRoutedActivity::CoefficientCombination
                }
                CandidateRoutedApplicationPhase::RootCollection => {
                    CandidateRoutedActivity::RootCollection
                }
            };
        }
        self.observe_event(
            application,
            phase,
            status,
            force,
            if status == CandidateRoutedApplicationStatus::Running {
                CandidateRoutedApplicationEvent::Snapshot
            } else {
                CandidateRoutedApplicationEvent::CoreFinished
            },
            true,
        );
    }

    fn observe_event(
        &mut self,
        application: &RoutedCandidateApplication<N>,
        phase: Option<CandidateRoutedApplicationPhase>,
        status: CandidateRoutedApplicationStatus,
        force: bool,
        event: CandidateRoutedApplicationEvent,
        sample: bool,
    ) {
        let Some(progress) = &mut self.progress else {
            return;
        };
        if let Some(phase) = phase {
            progress.phase = phase;
        }
        let now = if event == CandidateRoutedApplicationEvent::ActivityStarted && !sample {
            progress.notice()
        } else {
            progress.due(force)
        };
        let Some(now) = now else {
            return;
        };
        let cached =
            CandidateRoutedCacheCensus::new(application.cache.len(), application.cache_weight);
        let mut statistics = self.stats;
        // The execution counter is finalized at return. Observers receive a
        // current copy without altering work accounting or the eventual result.
        statistics.operational_nodes = self.seen.len();
        let snapshot = CandidateRoutedApplicationSnapshot {
            sequence: progress.sequence,
            elapsed: now.duration_since(progress.started),
            phase: progress.phase,
            status,
            statistics,
            unique_nodes: self.seen.len(),
            active_nodes: self.active.len(),
            pending_frames: self.algebra.pending_frame_count(),
            cached,
            pinned: self.pins.census,
            eligible: self.pins.census.map(|pinned| cached.excluding(pinned)),
        };
        let family = application.family();
        let mut envelope = CandidateRoutedApplicationEnvelope::new(event, family.fingerprint());
        let Some(observation) = &self.observation else {
            progress.deliver(&snapshot, envelope);
            return;
        };
        let arity = family.denominator_count();
        envelope.activity = observation.activity;
        envelope.root = observation.root.as_ref().map(|node| work_ref(node, arity));
        envelope.current = observation
            .current
            .as_ref()
            .map(|node| work_ref(node, arity));
        envelope.degrees = observation
            .current
            .as_ref()
            .map(|node| CandidateRoutedKeyDegrees::from_key(node.key()));
        envelope.metrics = observation.metrics;
        envelope.stored_state = Some(observation.stored_state());
        // The activity already supplied this O(1) structural hint. Computing
        // cache censuses or sampling pending work remains sample-only below.
        envelope.complexity = observation.complexity;
        envelope.path_total_nodes = self.active.len();
        let mut path = Vec::new();
        if sample {
            if path.try_reserve(observation.path_prefix.len()).is_ok() {
                path.extend(
                    observation
                        .path_prefix
                        .iter()
                        .map(|node| work_ref(node, arity)),
                );
            }
            if event == CandidateRoutedApplicationEvent::CycleDetected
                && let Some(current) = &observation.current
            {
                // A cycle repeats an active ancestor. The full index is
                // recovered only on this actual error path, even when the
                // retained diagnostic prefix does not reach that ancestor.
                envelope.cycle_backedge = self
                    .stack
                    .iter()
                    .filter_map(|frame| match frame {
                        Frame::Combine { node, .. } => Some(node),
                        Frame::Enter(_) => None,
                    })
                    .position(|node| node == current)
                    .or_else(|| {
                        observation
                            .path_prefix
                            .iter()
                            .position(|node| node == current)
                    });
                envelope.path_total_nodes = envelope.path_total_nodes.saturating_add(1);
                if path.len() < progress.options().max_path_nodes && path.try_reserve(1).is_ok() {
                    path.push(work_ref(current, arity));
                }
            }
            envelope.active_path = &path;
            if let Some(entry) = observation
                .current
                .as_ref()
                .and_then(|node| application.cache.get(node))
            {
                envelope.complexity = CandidateRoutedComplexity {
                    terms: Some(match &entry.terms {
                        CachedTerms::Sparse(terms) => terms.len(),
                        CachedTerms::Factorized(terms) => terms.len(),
                    }),
                    coefficient_terms: Some(entry.weight.coefficient_terms),
                    coefficient_bytes: Some(entry.weight.coefficient_bytes),
                    factorized: matches!(&entry.terms, CachedTerms::Factorized(_)),
                    ..envelope.complexity
                };
            }
            let examined = self.stack.len().min(progress.options().max_frontier_frames);
            let mut nodes = Vec::new();
            let mut integrals = Vec::new();
            let available =
                nodes.try_reserve(examined).is_ok() && integrals.try_reserve(examined).is_ok();
            if available {
                for frame in self.stack.iter().rev().take(examined) {
                    if let Frame::Enter(node) = frame {
                        nodes.push(work_ref(node, arity));
                        integrals.push(node.key());
                    }
                }
                nodes.sort_unstable();
                nodes.dedup();
                integrals.sort_unstable();
                integrals.dedup();
            }
            envelope.frontier = Some(CandidateRoutedFrontierSample {
                frames_examined: if available { examined } else { 0 },
                total_pending_frames: self.stack.len(),
                unique_nodes: nodes.len(),
                unique_integrals: integrals.len(),
                exact: available && examined == self.stack.len(),
            });
        }
        progress.deliver(&snapshot, envelope);
        if !progress.is_advanced() {
            self.observation = None;
        }
    }

    fn push(&mut self, frame: Frame<N>, limit: Option<usize>) -> Result<(), CandidateRoutedError> {
        self.algebra.retain_pending_frame(limit)?;
        symbolica::utils::resources::check_allocation(std::mem::size_of::<Frame<N>>() as u64)
            .map_err(ReductionError::from)?;
        self.stack
            .try_reserve(1)
            .map_err(|_| ReductionError::AllocationFailure {
                resource: "routed application stack",
                requested: self.stack.len().saturating_add(1),
            })?;
        if let (Some(observation), Frame::Combine { terms, .. }) = (&mut self.observation, &frame) {
            observation.stored_state.deferred_rule_maps = observation
                .stored_state
                .deferred_rule_maps
                .and_then(|n| n.checked_add(1));
            observation.stored_state.deferred_child_integral_occurrences = observation
                .stored_state
                .deferred_child_integral_occurrences
                .and_then(|n| n.checked_add(terms.len()));
        }
        self.stack.push(frame);
        Ok(())
    }
}

fn retain_conditions(
    output: &mut Vec<Coefficient>,
    conditions: impl IntoIterator<Item = Coefficient>,
) {
    for condition in conditions {
        if !output.contains(&condition) {
            output.push(condition);
        }
    }
}

fn retain_polynomial(output: &mut Vec<Coefficient>, condition: &CoefficientPolynomial) {
    if !condition.is_constant() {
        retain_conditions(output, [condition.clone().into()]);
    }
}

impl<const N: usize> RoutedCandidateApplication<N> {
    pub fn reduce_linear_combination(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        self.reduce_linear_combination_with_entry_admission(
            terms,
            CandidateEntryAdmission::SavedGenerationScope,
        )
    }

    /// One shared bounded request, including all roots, transports, rule
    /// applications and coefficient sums. Every input crosses admission even
    /// when its coefficient is zero or another input cancels it exactly.
    pub fn reduce_linear_combination_with_entry_admission(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        self.reduce_observed_request(terms, admission, None, None, Default::default())
    }

    /// Cooperatively cancel an exact request between complete operational
    /// frames and root collections. All inputs cross entry admission before
    /// cancellation is checked. Completed cache entries and their guards remain
    /// reusable; pending nodes never become terminal results. A single owner
    /// preparation, transport, or CAS operation cannot be interrupted here.
    pub fn reduce_linear_combination_with_entry_admission_cancellable(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        cancel: &AtomicBool,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        self.reduce_observed_request(terms, admission, None, Some(cancel), Default::default())
    }

    /// Observe one exact request on its calling thread. No observation runs
    /// inside CAS or a combine. Clock checks occur every 256 frame boundaries;
    /// the requested interval is a minimum, not a heartbeat guarantee. Zero
    /// requests every boundary. Initial and terminal snapshots are unconditional.
    /// Failure snapshots precede dependency cleanup. If the observer panics,
    /// pins are released and completed-cache work is retained before unwinding.
    pub fn reduce_linear_combination_with_entry_admission_and_observer(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        interval: Duration,
        mut observer: impl FnMut(&CandidateRoutedApplicationSnapshot),
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        self.reduce_observed_request(
            terms,
            admission,
            Some(Progress::new(interval, &mut observer)),
            None,
            Default::default(),
        )
    }

    /// Observe and cooperatively cancel the same exact traversal as the
    /// ordinary application API. The flag is checked at every operational
    /// frame boundary, independently of the observer interval, and between
    /// root collections. Cancellation returns `CandidateRoutedError::Cancelled`
    /// with a final failed snapshot before normal dependency-pin cleanup.
    /// It cannot interrupt an individual owner preparation, transport or CAS
    /// operation. Input admission still precedes the first cancellation check.
    pub fn reduce_linear_combination_with_entry_admission_and_control(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        cancel: &AtomicBool,
        interval: Duration,
        mut observer: impl FnMut(&CandidateRoutedApplicationSnapshot),
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        self.reduce_observed_request(
            terms,
            admission,
            Some(Progress::new(interval, &mut observer)),
            Some(cancel),
            Default::default(),
        )
    }

    /// Optional detailed telemetry over the ordinary exact direct-entry
    /// traversal. Observer faults detach telemetry and preserve the typed
    /// reduction result; cancellation keeps its existing safe-boundary rules.
    pub fn reduce_linear_combination_with_observation(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        observer: &mut CandidateRoutedApplicationObserver<'_>,
        cancel: Option<&AtomicBool>,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        self.reduce_observed_request(
            terms,
            admission,
            Progress::from_observer(observer),
            cancel,
            Default::default(),
        )
    }

    pub(super) fn reduce_observed_request<'a, 'storage>(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        progress: Option<Progress<'a>>,
        cancel: Option<&'a AtomicBool>,
        persistent: super::persistent::PersistentRequest<'a, 'storage>,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        self.reduce_observed_request_inner(terms, admission, progress, cancel, persistent, false)
    }

    pub(super) fn reduce_observed_routed_request<'a, 'storage>(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        progress: Option<Progress<'a>>,
        cancel: Option<&'a AtomicBool>,
        persistent: super::persistent::PersistentRequest<'a, 'storage>,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        self.reduce_observed_request_inner(terms, admission, progress, cancel, persistent, true)
    }

    fn reduce_observed_request_inner<'a, 'storage>(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        progress: Option<Progress<'a>>,
        cancel: Option<&'a AtomicBool>,
        persistent: super::persistent::PersistentRequest<'a, 'storage>,
        initial_routing: bool,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        let observed = progress.is_some();
        let observation = progress
            .as_ref()
            .filter(|progress| progress.is_advanced())
            .map(|progress| EngineObservation {
                path_limit: progress.options().max_path_nodes,
                ..Default::default()
            });
        let has_callbacks =
            observed || persistent.backing.is_some() || persistent.capture.is_some();
        let mut request = Request {
            progress,
            observation,
            cancel,
            persistent,
            ..Default::default()
        };
        if observed && self.cache_policy.evicts() {
            request.pins.census = Some(CandidateRoutedCacheCensus::default());
        }
        let run = || {
            request.observe(
                self,
                Some(CandidateRoutedApplicationPhase::Admission),
                CandidateRoutedApplicationStatus::Running,
                true,
            );
            let result = if initial_routing {
                self.reduce_routed_request(terms, admission, &mut request)
            } else {
                self.reduce_request(terms, admission, &mut request, true)
            }
            .and_then(|result| {
                symbolica::utils::resources::checkpoint().map_err(ReductionError::from)?;
                Ok(result)
            });
            if result.is_err()
                && let Some(observation) = &mut request.observation
            {
                // These maps belonged to reduce_request locals, now dropped.
                // Remaining queued frames still belong to the live request.
                observation.stored_state.unstarted_roots = None;
                observation.stored_state.collected_output_integrals = None;
            }
            request.observe(
                self,
                None,
                if result.is_ok() {
                    CandidateRoutedApplicationStatus::Succeeded
                } else {
                    CandidateRoutedApplicationStatus::Failed
                },
                true,
            );
            result
        };
        // The ordinary path has no clock, snapshot construction or unwind
        // boundary. Observed callbacks must not strand the eviction index.
        let result = if has_callbacks {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(run))
        } else {
            Ok(run())
        };
        self.release_request_pins(&mut request.pins);
        // Failed attempts retain valid completed cache entries and their
        // measured work, never active or partially combined nodes.
        self.work.merge_work(request.stats.work);
        self.cache_work.merge(request.stats.cache);
        if result.is_ok() {
            request.persistent.report(request.stats.persistent_cache);
        }
        match result {
            Ok(result) => result,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    /// Explicitly transport initial keys using installed routes, then admit
    /// every endpoint to the selected entry union before cancellation. All
    /// transport, reduction and addition work shares one bounded request.
    pub fn reduce_routed_linear_combination_with_entry_admission(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        self.reduce_observed_routed_request(terms, admission, None, None, Default::default())
    }

    /// The same detailed observations for the existing initial-entry transport
    /// workflow. Routed endpoints retain the ordinary entry-admission checks.
    pub fn reduce_routed_linear_combination_with_observation(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        observer: &mut CandidateRoutedApplicationObserver<'_>,
        cancel: Option<&AtomicBool>,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        self.reduce_observed_routed_request(
            terms,
            admission,
            Progress::from_observer(observer),
            cancel,
            Default::default(),
        )
    }

    pub(super) fn reduce_routed_request(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        request: &mut Request<'_, '_, N>,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        let context = self.context.clone();
        let coefficients = context.shared.context.base();
        let limits = context.limits;
        let mut entries = BTreeMap::new();
        let mut conditions = Vec::new();
        for (target, factor) in terms {
            coefficients
                .validate_with_limits(&factor, limits.exact_algebra)
                .map_err(CandidateReductionError::from)?;
            retain_polynomial(&mut conditions, &factor.denominator);
            request.set_entry_root(&target);
            request.observe_activity(self, CandidateRoutedActivity::Transport, None);
            let routed =
                self.route_entry_in_request(&target, &mut request.stats, self.unresolved_policy)?;
            retain_conditions(&mut conditions, routed.nonzero_conditions);
            for (endpoint, weight) in routed.terms {
                // Even zero caller factors cannot bypass endpoint admission.
                self.admit_entry(admission, &endpoint)?;
                let contribution = coefficients
                    .try_mul(&factor, &weight, limits.exact_algebra)
                    .map_err(CandidateReductionError::from)?;
                accumulate_master_in_request(
                    coefficients,
                    &mut entries,
                    &endpoint,
                    contribution,
                    limits,
                    &mut request.algebra,
                    &mut request.stats.work,
                )?;
            }
        }
        let mut result = self.reduce_request(entries, admission, request, false)?;
        retain_conditions(&mut conditions, result.nonzero_conditions);
        result.nonzero_conditions = conditions;
        Ok(result)
    }

    pub(super) fn admit_entry(
        &self,
        admission: CandidateEntryAdmission<'_, N>,
        target: &IntegralKey,
    ) -> Result<(), CandidateRoutedError> {
        use crate::solver::RootAdmissionError;
        match admission.validate_entry(target, self.context.scope.max_numerator_rank) {
            Err(CandidateRoutedError::EntryAdmission(
                RootAdmissionError::OutsideSelectedSupport { .. }
                | RootAdmissionError::OutsideStartingDomain { .. },
            ))
            | Err(CandidateRoutedError::Candidate(
                CandidateReductionError::OutsideNumeratorRank { .. },
            )) if self.unresolved_policy == UnresolvedPolicy::Preserve => Ok(()),
            result => result,
        }
    }

    fn reduce_request(
        &mut self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        admission: CandidateEntryAdmission<'_, N>,
        request: &mut Request<'_, '_, N>,
        count_inputs: bool,
    ) -> Result<CandidateRoutedReduction, CandidateRoutedError> {
        let context = self.context.clone();
        let shared = &context.shared;
        let limits = context.limits;
        let base = CandidateEvaluator {
            context: &shared.context,
            root_sector: [true; N],
            ordering: &Default::default(),
            rules: &[],
            whole_piece_alternatives: &[],
            source_conditions: &shared.source_conditions,
            zero_sectors: &shared.zero_sectors,
            limits,
        };
        let coefficients = shared.context.base();
        let mut entries = BTreeMap::new();
        let mut conditions = Vec::new();
        for (target, coefficient) in terms {
            request.set_entry_root(&target);
            if count_inputs {
                charge(
                    &mut request.stats.input_terms,
                    1,
                    self.limits.diagnostic_limit(self.limits.max_input_targets),
                    "input terms",
                )?;
            }
            base.validate_target_with_conditions(&target, &mut |condition| {
                retain_polynomial(&mut conditions, condition)
            })?;
            self.admit_entry(admission, &target)?;
            coefficients
                .validate_with_limits(&coefficient, limits.exact_algebra)
                .map_err(CandidateReductionError::from)?;
            retain_polynomial(&mut conditions, &coefficient.denominator);
            if !entries.contains_key(&target) && !coefficient.is_zero() {
                let mut size = request.stats.operational_nodes;
                charge(
                    &mut size,
                    entries.len(),
                    self.limits.diagnostic_limit(self.limits.max_unique_nodes),
                    "initial operational nodes",
                )?;
                charge(
                    &mut size,
                    1,
                    self.limits.diagnostic_limit(self.limits.max_unique_nodes),
                    "initial operational nodes",
                )?;
            }
            accumulate_master_in_request(
                coefficients,
                &mut entries,
                &target,
                coefficient,
                limits,
                &mut request.algebra,
                &mut request.stats.work,
            )?;
        }
        request.check_cancel()?;
        request.stats.requested_targets = entries.len();
        if let Some(observation) = &mut request.observation {
            observation.metrics.requested_roots = Some(entries.len());
            observation.stored_state.unstarted_roots = Some(entries.len());
            observation.stored_state.collected_output_integrals = Some(0);
            observation.metrics.output_terms = Some(0);
        }
        if request.persistent.backing.is_some() {
            request.persistent.requested_roots.extend(
                entries
                    .keys()
                    .map(crate::persistence::application_cache::ApplicationCacheKey::route),
            );
        }
        let mut output = BTreeMap::new();
        for (target, factor) in entries {
            if let Some(observation) = &mut request.observation {
                observation.stored_state.unstarted_roots = observation
                    .stored_state
                    .unstarted_roots
                    .and_then(|n| n.checked_sub(1));
            }
            request.check_cancel()?;
            let root = Node::Route(target);
            request.set_root(&root);
            self.pin_dependency(&root, &mut request.pins)?;
            request.push(
                Frame::Enter(root.clone()),
                limits.diagnostic_limit(limits.max_pending_frames),
            )?;
            self.run_stack(request)?;
            request.set_current(&root);
            request.observe(
                self,
                Some(CandidateRoutedApplicationPhase::RootCollection),
                CandidateRoutedApplicationStatus::Running,
                false,
            );
            request.check_cancel()?;
            request.observe_activity(self, CandidateRoutedActivity::RootMaterialization, None);
            let entry = self
                .cache
                .get(&root)
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "routed root is absent after application",
                })?;
            retain_conditions(&mut conditions, entry.conditions.iter().cloned());
            let terms = match &entry.terms {
                CachedTerms::Sparse(terms) => Cow::Borrowed(terms),
                _ => Cow::Owned(entry.terms.materialize(coefficients, limits)?),
            };
            request.observe_activity(
                self,
                CandidateRoutedActivity::RootCollection,
                Some(terms.len()),
            );
            for (index, (terminal, coefficient)) in terms.iter().enumerate() {
                if let Some(observation) = &mut request.observation {
                    observation.complexity.contribution_index = Some(index + 1);
                    observation.complexity.contribution_total = Some(terms.len());
                    request.observe_event(
                        self,
                        None,
                        CandidateRoutedApplicationStatus::Running,
                        true,
                        CandidateRoutedApplicationEvent::ActivityStarted,
                        false,
                    );
                }
                let contribution = coefficients
                    .try_mul(&factor, coefficient, limits.exact_algebra)
                    .map_err(CandidateReductionError::from)?;
                accumulate_master_in_request(
                    coefficients,
                    &mut output,
                    terminal,
                    contribution,
                    limits,
                    &mut request.algebra,
                    &mut request.stats.work,
                )?;
                if let Some(observation) = &mut request.observation {
                    observation.metrics.output_terms = Some(output.len());
                    observation.stored_state.collected_output_integrals = Some(output.len());
                }
            }
            // The output owns its coefficients and guards now. The current
            // root may be reused or evicted while processing later roots.
            drop(terms);
            self.touch_cache(&root)?;
            self.release_dependency(&root, &mut request.pins)?;
            if let Some(observation) = &mut request.observation {
                observation.metrics.completed_roots =
                    observation.metrics.completed_roots.saturating_add(1);
                observation.metrics.output_terms = Some(output.len());
                observation.activity = CandidateRoutedActivity::RootCollection;
                request.observe_event(
                    self,
                    None,
                    CandidateRoutedApplicationStatus::Running,
                    true,
                    CandidateRoutedApplicationEvent::RootCollected,
                    true,
                );
            }
        }
        request.check_cancel()?;
        charge(
            &mut request.stats.operational_nodes,
            request.seen.len(),
            self.limits.diagnostic_limit(self.limits.max_unique_nodes),
            "operational nodes",
        )?;
        let mut unresolved = BTreeSet::new();
        for key in output.keys() {
            if !self.terminal_is_declared(key)? {
                unresolved.insert(key.clone());
            }
        }
        if let Some(observation) = &mut request.observation {
            observation.metrics.output_terms = Some(output.len());
        }
        Ok(CandidateRoutedReduction {
            family_fingerprint: context.family.fingerprint_owner(),
            terms: output,
            unresolved,
            nonzero_conditions: conditions,
            statistics: request.stats,
        })
    }

    fn run_stack(&mut self, request: &mut Request<'_, '_, N>) -> Result<(), CandidateRoutedError> {
        let result = self.run_stack_inner(request);
        if let Some(observation) = &mut request.observation {
            // The popped frame has been dropped on success and every error path.
            observation.combining_child_occurrences = None;
        }
        result
    }

    fn run_stack_inner(
        &mut self,
        request: &mut Request<'_, '_, N>,
    ) -> Result<(), CandidateRoutedError> {
        let context = self.context.clone();
        let shared = &context.shared;
        let limits = context.limits;
        let coefficients = shared.context.base();
        let base = CandidateEvaluator {
            context: &shared.context,
            root_sector: [true; N],
            ordering: &Default::default(),
            rules: &[],
            whole_piece_alternatives: &[],
            source_conditions: &shared.source_conditions,
            zero_sectors: &shared.zero_sectors,
            limits,
        };
        while let Some(frame) = request.stack.pop() {
            if let (Some(observation), Frame::Combine { terms, .. }) =
                (&mut request.observation, &frame)
            {
                observation.stored_state.deferred_rule_maps = observation
                    .stored_state
                    .deferred_rule_maps
                    .and_then(|n| n.checked_sub(1));
                observation.stored_state.deferred_child_integral_occurrences = observation
                    .stored_state
                    .deferred_child_integral_occurrences
                    .and_then(|n| n.checked_sub(terms.len()));
                observation.combining_child_occurrences = Some(terms.len());
            }
            request.algebra.release_pending_frame()?;
            let (phase, current) = match &frame {
                Frame::Enter(node) => (CandidateRoutedApplicationPhase::Expansion, node),
                Frame::Combine { node, .. } => (CandidateRoutedApplicationPhase::Combination, node),
            };
            request.set_current(current);
            request.observe(
                self,
                Some(phase),
                CandidateRoutedApplicationStatus::Running,
                false,
            );
            request.check_cancel()?;
            let node = match frame {
                Frame::Combine {
                    node,
                    terms,
                    mut conditions,
                    first_visit,
                } => {
                    // One shallow membership index for the complete frame,
                    // including every child. Cache guards are borrowed until
                    // a genuinely new condition is retained in first-seen order.
                    request.observe_activity(
                        self,
                        CandidateRoutedActivity::CoefficientCombination,
                        Some(terms.len()),
                    );
                    let mut guards = GuardUnion::try_new(&mut conditions)?;
                    let output = cache::combine_values(
                        coefficients,
                        self.cache_representation,
                        terms.iter().map(|(child, factor)| {
                            let entry =
                                self.cache
                                    .get(child)
                                    .ok_or(ReductionError::ReducerInvariant {
                                        detail: "routed child is absent at combine frame",
                                    })?;
                            guards.extend(&entry.conditions)?;
                            Ok((&entry.terms, factor.clone()))
                        }),
                        limits,
                        &mut request.algebra,
                        &mut request.stats.work,
                    )?;
                    drop(guards);
                    // Every queued parent retains its own reference. Release
                    // this frame's children only after all arithmetic and
                    // guards have been copied, before admitting the parent.
                    for child in terms.keys() {
                        self.touch_cache(child)?;
                        self.release_dependency(child, &mut request.pins)?;
                    }
                    if !request.active.remove(&node) {
                        return Err(ReductionError::ReducerInvariant {
                            detail: "routed combine node is not active",
                        }
                        .into());
                    }
                    request.release_observed_path();
                    self.cache_insert(
                        node.clone(),
                        output,
                        conditions,
                        &mut request.pins,
                        &mut request.stats.cache,
                    )?;
                    request.record_completion(first_visit);
                    request.observe_completion(self, CandidateRoutedCompletionSource::Computed);
                    self.capture_persistent_node(
                        &node,
                        &mut request.persistent,
                        &mut request.stats.persistent_cache,
                    )?;
                    if let Some(observation) = &mut request.observation {
                        observation.combining_child_occurrences = None;
                    }
                    continue;
                }
                Frame::Enter(node) => node,
            };
            if request.active.contains(&node) {
                if request.observation.is_some() {
                    request.observe_event(
                        self,
                        None,
                        CandidateRoutedApplicationStatus::Running,
                        true,
                        CandidateRoutedApplicationEvent::CycleDetected,
                        true,
                    );
                }
                return Err(CandidateRoutedError::Cycle {
                    target: node.key().clone(),
                });
            }
            let seen_before = request.seen.contains(&node);
            if !seen_before {
                let mut size = request.stats.operational_nodes;
                charge(
                    &mut size,
                    request.seen.len(),
                    self.limits.diagnostic_limit(self.limits.max_unique_nodes),
                    "operational nodes",
                )?;
                charge(
                    &mut size,
                    1,
                    self.limits.diagnostic_limit(self.limits.max_unique_nodes),
                    "operational nodes",
                )?;
                request.seen.insert(node.clone());
            }
            let key = node.key();
            let mut conditions = Vec::new();
            base.validate_target_with_conditions(key, &mut |condition| {
                retain_polynomial(&mut conditions, condition)
            })?;
            if self.cache.contains_key(&node) {
                self.reuse_cache(&node)?;
                request.stats.work.record_cache_hit();
                request.record_completion(!seen_before);
                request.observe_completion(self, CandidateRoutedCompletionSource::RamHit);
                continue;
            }
            if seen_before {
                request.stats.cache.recomputed_nodes =
                    request.stats.cache.recomputed_nodes.saturating_add(1);
            }
            if base.is_zero(key) {
                self.cache_insert_sparse(
                    node,
                    BTreeMap::new(),
                    conditions,
                    !seen_before,
                    CandidateRoutedCompletionSource::Zero,
                    request,
                )?;
                continue;
            }
            if self.lookup_persistent_node(
                &node,
                &mut conditions,
                &mut request.persistent,
                &mut request.stats.persistent_cache,
            )? {
                let record = request
                    .persistent
                    .loaded
                    .take()
                    .expect("lookup sets completed record");
                self.cache_insert_sparse(
                    node.clone(),
                    record.terms,
                    conditions,
                    !seen_before,
                    CandidateRoutedCompletionSource::BackingHit,
                    request,
                )?;
                request.stats.work.record_cache_hit();
                request.observe_completion(self, CandidateRoutedCompletionSource::BackingHit);
                continue;
            }
            request.active.insert(node.clone());
            request.retain_observed_path(&node);
            let parent_support = support::<N>(key);
            let terms = match &node {
                Node::Route(_) => {
                    if self.owners.contains(&parent_support) {
                        BTreeMap::from([(
                            Node::Apply(parent_support, key.clone()),
                            coefficients.one(),
                        )])
                    } else if let Some(route) = self.routes.get(&parent_support) {
                        charge(
                            &mut request.stats.transport_calls,
                            1,
                            self.limits
                                .diagnostic_limit(self.limits.max_transport_calls),
                            "transport calls",
                        )?;
                        request.observe_activity(self, CandidateRoutedActivity::Transport, None);
                        let mapped = route.transport.transport_with_usage(
                            key,
                            self.limits.expansion,
                            |usage| {
                                let operations = request
                                    .stats
                                    .transport_operations
                                    .saturating_add(usage.operations);
                                let endpoints = request
                                    .stats
                                    .transport_endpoints
                                    .saturating_add(usage.endpoints);
                                for (resource, requested, limit) in [
                                    (
                                        "aggregate routed operations",
                                        operations,
                                        self.limits.max_transport_operations,
                                    ),
                                    (
                                        "aggregate routed endpoints",
                                        endpoints,
                                        self.limits.max_transport_endpoints,
                                    ),
                                ] {
                                    if self.limits.diagnostics_enabled && requested > limit {
                                        return Err(ExpansionError::ResourceLimit {
                                            resource,
                                            requested,
                                            limit,
                                        });
                                    }
                                }
                                request.stats.transport_operations = operations;
                                request.stats.transport_endpoints = endpoints;
                                Ok(())
                            },
                        )?;
                        let owner_support = std::array::from_fn(|i| {
                            route
                                .owner_sector
                                .active_bits()
                                .get(i)
                                .copied()
                                .unwrap_or(false)
                        });
                        let mut terms = BTreeMap::new();
                        for endpoint in mapped.terms() {
                            let child = endpoint.key();
                            base.validate_target(child)?;
                            let child_node = successor(key, child.clone(), owner_support)?;
                            terms.insert(child_node, endpoint.coefficient().clone());
                        }
                        terms
                    } else if self.unresolved_policy == UnresolvedPolicy::Preserve {
                        request.active.remove(&node);
                        request.release_observed_path();
                        self.cache_insert_sparse(
                            node.clone(),
                            BTreeMap::from([(key.clone(), coefficients.one())]),
                            conditions,
                            !seen_before,
                            CandidateRoutedCompletionSource::UnresolvedPreserved,
                            request,
                        )?;
                        continue;
                    } else {
                        return Err(CandidateRoutedError::MissingOwner {
                            target: key.clone(),
                        });
                    }
                }
                Node::Apply(owner_sector, _) => {
                    if &parent_support != owner_sector {
                        return Err(CandidateRoutedError::InvalidInput(
                            "apply node does not belong to its owner support".into(),
                        ));
                    }
                    request.observe_activity(self, CandidateRoutedActivity::OwnerPreparation, None);
                    let owner = self.owners.owner(owner_sector)?.ok_or_else(|| {
                        CandidateRoutedError::InvalidInput("admitted owner disappeared".into())
                    })?;
                    let result = if owner.batches[0].terminals.contains(key) {
                        OwnerStep::Terminal
                    } else {
                        request.algebra.record_rule_application(
                            limits.diagnostic_limit(limits.max_rule_applications),
                        )?;
                        request.observe_activity(
                            self,
                            CandidateRoutedActivity::RuleEvaluation,
                            None,
                        );
                        owner.evaluate_step::<CandidateReductionError>(key, |batch| {
                            let evaluator = CandidateEvaluator {
                                context: &shared.context,
                                root_sector: owner.root,
                                ordering: &owner.ordering,
                                rules: &batch.rules,
                                whole_piece_alternatives: &batch.whole_piece_alternatives,
                                source_conditions: &shared.source_conditions,
                                zero_sectors: &shared.zero_sectors,
                                limits,
                            };
                            Ok(evaluator
                                .apply_with_conditions(
                                    key,
                                    &mut request.algebra,
                                    &mut request.stats.work,
                                )
                                .map(|(terms, local)| {
                                    retain_conditions(&mut conditions, local);
                                    terms
                                }))
                        })?
                    };
                    match result {
                        OwnerStep::Terminal => {
                            request.active.remove(&node);
                            request.release_observed_path();
                            self.cache_insert_sparse(
                                node.clone(),
                                BTreeMap::from([(key.clone(), coefficients.one())]),
                                conditions,
                                !seen_before,
                                CandidateRoutedCompletionSource::Terminal,
                                request,
                            )?;
                            continue;
                        }
                        OwnerStep::Uncovered
                            if self.unresolved_policy == UnresolvedPolicy::Preserve =>
                        {
                            request.active.remove(&node);
                            request.release_observed_path();
                            self.cache_insert_sparse(
                                node.clone(),
                                BTreeMap::from([(key.clone(), coefficients.one())]),
                                conditions,
                                !seen_before,
                                CandidateRoutedCompletionSource::UnresolvedPreserved,
                                request,
                            )?;
                            continue;
                        }
                        OwnerStep::Uncovered => {
                            return Err(CandidateReductionError::Uncovered {
                                target: key.clone(),
                            }
                            .into());
                        }
                        OwnerStep::Applied(terms) => {
                            request.stats.work.record_rule_application();
                            let mut children = BTreeMap::new();
                            for (child, coefficient) in terms {
                                let child_node = successor(key, child, *owner_sector)?;
                                request.record_local_descent();
                                children.insert(child_node, coefficient);
                            }
                            children
                        }
                    }
                }
            };
            // Schedule in sorted order; Route and Apply(owner, key) are distinct
            // memoized states even when they refer to the same physical key.
            let terms = Arc::new(terms);
            request.push(
                Frame::Combine {
                    node,
                    terms: terms.clone(),
                    conditions,
                    first_visit: !seen_before,
                },
                limits.diagnostic_limit(limits.max_pending_frames),
            )?;
            for child in terms.keys().rev() {
                self.pin_dependency(child, &mut request.pins)?;
                request.push(
                    Frame::Enter(child.clone()),
                    limits.diagnostic_limit(limits.max_pending_frames),
                )?;
            }
        }
        Ok(())
    }

    fn cache_insert_sparse(
        &mut self,
        node: Node<N>,
        terms: BTreeMap<IntegralKey, Coefficient>,
        conditions: Vec<Coefficient>,
        first_visit: bool,
        source: CandidateRoutedCompletionSource,
        request: &mut Request<'_, '_, N>,
    ) -> Result<(), CandidateRoutedError> {
        let context = &self.context;
        let terms = CachedTerms::from_sparse(
            terms,
            self.cache_representation,
            context.shared.context.base(),
            context.limits,
        )?;
        self.cache_insert(
            node.clone(),
            terms,
            conditions,
            &mut request.pins,
            &mut request.stats.cache,
        )?;
        request.record_completion(first_visit);
        if source != CandidateRoutedCompletionSource::BackingHit {
            request.observe_completion(self, source);
        }
        if source == CandidateRoutedCompletionSource::BackingHit
            && request
                .persistent
                .capture
                .as_ref()
                .is_some_and(|capture| !capture.capture_backing_hits())
        {
            return Ok(());
        }
        self.capture_persistent_node(
            &node,
            &mut request.persistent,
            &mut request.stats.persistent_cache,
        )
    }
}
