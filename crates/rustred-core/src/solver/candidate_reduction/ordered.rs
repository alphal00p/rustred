//! Generic ordered adapter for immutable, possibly multi-sector candidate tables.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::{CandidateReducer, CandidateReductionError, UnresolvedPolicy};
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::IntegralKey;
use crate::reduction::guard_union::GuardIndex;
use crate::reduction::ordered_reduction::step_cache::codec::NodeStage;
use crate::reduction::ordered_reduction::step_cache::failure;
use crate::reduction::ordered_reduction::step_cache::kernel::{
    ConcurrentStepCacheKernel, OrderedStepCacheKernel, run_with_step_cache_and_options,
};
use crate::reduction::ordered_reduction::{
    ConcurrentOrderedKernel, OrderedKernel, OrderedKeyStage, OrderedPhaseDescriptor,
    OrderedPhaseEvent, OrderedPhaseInspector, OrderedPosition, OrderedStep, OrderedStepper,
};
use crate::reduction::{OrderedSchedulerOptions, OrderedWeightArithmetic};
use crate::reduction::{
    OrderedStepCache, OrderedStepCacheAdapter, OrderedStepCacheBinding, OrderedStepShard,
    OrderedStepStage,
};
use crate::reduction::{
    ReductionError, ReductionLimits, ReductionRequest, ReductionStatistics,
    accumulate_master_in_request,
};
use crate::sector::ComplexityKey;
use crate::solver::{CandidateOrderedPhaseKind, CandidateOrderedReductionProgress};

/// Exact weighted candidate result, with no completed-result cache retained.
#[derive(Clone, Debug)]
pub struct CandidateOrderedReduction {
    family_fingerprint: Arc<String>,
    terms: BTreeMap<IntegralKey, Coefficient>,
    unresolved: BTreeSet<IntegralKey>,
    nonzero_conditions: Vec<Coefficient>,
    statistics: ReductionStatistics,
    progress: CandidateOrderedReductionProgress,
}
impl CandidateOrderedReduction {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn terms(&self) -> &BTreeMap<IntegralKey, Coefficient> {
        &self.terms
    }
    pub fn unresolved(&self) -> &BTreeSet<IntegralKey> {
        &self.unresolved
    }
    pub fn nonzero_conditions(&self) -> &[Coefficient] {
        &self.nonzero_conditions
    }
    pub fn statistics(&self) -> &ReductionStatistics {
        &self.statistics
    }
    pub fn progress(&self) -> &CandidateOrderedReductionProgress {
        &self.progress
    }
    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }
}

type Observer<'a> = dyn FnMut(
        &CandidateOrderedReductionProgress,
        Option<&IntegralKey>,
    ) -> Result<(), CandidateReductionError>
    + 'a;

struct Kernel<'a, 'observer, const N: usize> {
    reducer: &'a CandidateReducer<N>,
    support_phases: bool,
    observer: Option<&'observer mut Observer<'observer>>,
    inspector: Option<&'observer mut OrderedPhaseInspector<'observer, CandidateReductionError>>,
}
impl<const N: usize> OrderedKernel for Kernel<'_, '_, N> {
    type Key = IntegralKey;
    type Priority = ComplexityKey;
    type Error = CandidateReductionError;
    fn context(&self) -> &CoefficientContext {
        self.reducer.program.context.base()
    }
    fn limits(&self) -> ReductionLimits {
        self.reducer.limits
    }
    fn phase_count(&self) -> usize {
        if self.support_phases {
            self.reducer.physical_arity() + 1
        } else {
            1
        }
    }
    fn position(
        &mut self,
        key: &IntegralKey,
    ) -> Result<OrderedPosition<ComplexityKey>, Self::Error> {
        position(self.reducer, self.support_phases, key)
    }
    fn step(
        &mut self,
        key: &IntegralKey,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<OrderedStep<IntegralKey>, Self::Error> {
        step(self.reducer, key, request, work)
    }
    fn observe(
        &mut self,
        progress: &CandidateOrderedReductionProgress,
        current: Option<&IntegralKey>,
    ) -> Result<(), Self::Error> {
        if let Some(observer) = &mut self.observer {
            let mut progress = *progress;
            progress.phase_kind = CandidateOrderedPhaseKind::Reduction;
            progress.active_support =
                if self.support_phases && progress.completed_phases < progress.total_phases {
                    Some(self.reducer.physical_arity() - progress.completed_phases)
                } else {
                    current.map(|key| key.powers().iter().filter(|&&power| power > 0).count())
                };
            observer(&progress, current)?;
        }
        Ok(())
    }
    fn key_view(key: &IntegralKey) -> Option<(OrderedKeyStage<'_>, &IntegralKey)> {
        Some((OrderedKeyStage::Integral, key))
    }
    fn inspect_phase(&mut self, event: OrderedPhaseEvent<'_>) -> Result<(), Self::Error> {
        let Some(inspector) = self.inspector.as_deref_mut() else {
            return Ok(());
        };
        inspector(
            &event.with_descriptor(OrderedPhaseDescriptor {
                kind: CandidateOrderedPhaseKind::Reduction,
                support: self
                    .support_phases
                    .then(|| self.reducer.physical_arity() - event.phase),
                ..Default::default()
            }),
        )
    }
}

fn position<const N: usize>(
    reducer: &CandidateReducer<N>,
    support_phases: bool,
    key: &IntegralKey,
) -> Result<OrderedPosition<ComplexityKey>, CandidateReductionError> {
    let priority = reducer
        .program
        .ordering
        .complexity_key(key.powers())
        .map_err(ReductionError::Ordering)?;
    let phase = if support_phases {
        reducer.physical_arity() - priority.propagators()
    } else {
        0
    };
    Ok(OrderedPosition { phase, priority })
}

/// One checked candidate step. It reads only the immutable reducer; the
/// request and statistics receive its accounting.
fn step<const N: usize>(
    reducer: &CandidateReducer<N>,
    key: &IntegralKey,
    request: &mut ReductionRequest,
    work: &mut ReductionStatistics,
) -> Result<OrderedStep<IntegralKey>, CandidateReductionError> {
    let limits = reducer.limits;
    let mut conditions = reducer.target_conditions_with_limits(key, limits)?;
    if reducer.is_zero(key) {
        return Ok(OrderedStep {
            nonzero_conditions: conditions,
            ..Default::default()
        });
    }
    if reducer.program.terminals.contains(key) {
        let terminals = reducer.terminal_terms(key)?;
        for coefficient in terminals.values() {
            if !coefficient.denominator.is_constant() {
                conditions.push(coefficient.denominator.clone().into());
            }
        }
        return Ok(OrderedStep {
            terminals,
            nonzero_conditions: conditions,
            ..Default::default()
        });
    }
    request.record_rule_application(limits.diagnostic_limit(limits.max_rule_applications))?;
    let result = reducer
        .program
        .evaluator(key, limits)
        .and_then(|evaluator| evaluator.apply_with_conditions(key, request, work));
    match result {
        Ok((terms, local)) => {
            work.record_rule_application();
            conditions.extend(local);
            Ok(OrderedStep {
                terms,
                nonzero_conditions: conditions,
                ..Default::default()
            })
        }
        Err(CandidateReductionError::Uncovered { .. })
            if reducer.unresolved_policy == UnresolvedPolicy::Preserve =>
        {
            Ok(OrderedStep {
                terminals: BTreeMap::from([(key.clone(), reducer.program.context.base().one())]),
                nonzero_conditions: conditions,
                ..Default::default()
            })
        }
        Err(error) => Err(error),
    }
}

/// The candidate reducer is immutable during a request, so its steps are
/// pure functions of the key.
struct Stepper<'a, const N: usize> {
    reducer: &'a CandidateReducer<N>,
    support_phases: bool,
}

/// Accounting of one detached candidate step.
struct Account {
    request: ReductionRequest,
    work: ReductionStatistics,
}

impl<const N: usize> OrderedStepper for Stepper<'_, N> {
    type Key = IntegralKey;
    type Priority = ComplexityKey;
    type Error = CandidateReductionError;
    type Account = Account;

    fn position(
        &self,
        key: &IntegralKey,
    ) -> Result<OrderedPosition<ComplexityKey>, CandidateReductionError> {
        position(self.reducer, self.support_phases, key)
    }
    fn step(
        &self,
        key: &IntegralKey,
    ) -> (
        Result<OrderedStep<IntegralKey>, CandidateReductionError>,
        Account,
    ) {
        let mut account = Account {
            request: ReductionRequest::default(),
            work: ReductionStatistics::default(),
        };
        let result = step(self.reducer, key, &mut account.request, &mut account.work);
        (result, account)
    }
}

impl<const N: usize> ConcurrentOrderedKernel for Kernel<'_, '_, N> {
    type Account = Account;
    type Stepper<'s>
        = Stepper<'s, N>
    where
        Self: 's;

    fn stepper(&self) -> Option<Stepper<'_, N>> {
        Some(Stepper {
            reducer: self.reducer,
            support_phases: self.support_phases,
        })
    }
    fn absorb(
        &mut self,
        _: &IntegralKey,
        account: Account,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<(), CandidateReductionError> {
        request.absorb_detached(&account.request, self.reducer.limits)?;
        work.merge_work(account.work);
        Ok(())
    }
}

/// Candidate records are sharded by support level, the phase of their key.
fn step_cache_shard(key: &IntegralKey) -> OrderedStepShard {
    OrderedStepShard {
        stage: OrderedStepStage::Integral,
        support: key.powers().iter().filter(|&&power| power > 0).count() as u16,
        class: 0,
    }
}

fn step_cache_key(
    stage: NodeStage,
    integral: IntegralKey,
) -> Result<IntegralKey, CandidateReductionError> {
    if stage != NodeStage::Integral {
        return Err(failure("a recorded candidate identity is not a plain integral").into());
    }
    Ok(integral)
}

impl<const N: usize> OrderedStepCacheKernel for Kernel<'_, '_, N> {
    fn step_cache_shard(&mut self, key: &IntegralKey) -> Option<OrderedStepShard> {
        Some(step_cache_shard(key))
    }

    fn step_cache_key(
        &self,
        stage: NodeStage,
        integral: IntegralKey,
    ) -> Result<IntegralKey, Self::Error> {
        step_cache_key(stage, integral)
    }
}

impl<const N: usize> ConcurrentStepCacheKernel for Kernel<'_, '_, N> {
    fn stepper_shard<'s>(_: &Stepper<'s, N>, key: &IntegralKey) -> Option<OrderedStepShard>
    where
        Self: 's,
    {
        Some(step_cache_shard(key))
    }

    fn stepper_key<'s>(
        _: &Stepper<'s, N>,
        stage: NodeStage,
        integral: IntegralKey,
    ) -> Result<IntegralKey, CandidateReductionError>
    where
        Self: 's,
    {
        step_cache_key(stage, integral)
    }
}

impl<const N: usize> CandidateReducer<N> {
    /// Binding of an Ordered step cache to this session's program: candidate
    /// adapter, family, coefficient variables and unresolved policy (cheap
    /// identity fields; the stamp adds the cache schema and step semantics
    /// versions). Rule tables and the installed terminal convention are not
    /// digested: keep one cache per program and terminal convention.
    pub fn ordered_step_cache_binding(&self) -> OrderedStepCacheBinding {
        OrderedStepCacheBinding::new(
            OrderedStepCacheAdapter::CandidateProgram,
            self.program.family_fingerprint.as_str(),
            self.program.context.base(),
            Some(self.unresolved_policy),
        )
    }

    /// Attach (or detach) a persistent Ordered step cache. Ordered requests of
    /// this session and of its forks then replay recorded steps and record new
    /// ones; expanded keys, merges, guards and results are unchanged. The
    /// binding must be this session's [`Self::ordered_step_cache_binding`]:
    /// candidate adapter, family, coefficient context and unresolved policy
    /// are checked here and at every request. The cache must have been
    /// recorded with this rule program and terminal convention (the caller
    /// keeps one cache per program); terminal conventions cannot be installed
    /// while a cache is attached. Depth-first application ignores the cache.
    pub fn set_ordered_step_cache(
        &mut self,
        cache: Option<Arc<OrderedStepCache>>,
    ) -> Result<(), CandidateReductionError> {
        if let Some(cache) = &cache {
            self.check_ordered_step_cache(cache)?;
        }
        self.ordered_step_cache = cache;
        Ok(())
    }

    pub fn ordered_step_cache(&self) -> Option<&Arc<OrderedStepCache>> {
        self.ordered_step_cache.as_ref()
    }

    fn check_ordered_step_cache(
        &self,
        cache: &OrderedStepCache,
    ) -> Result<(), CandidateReductionError> {
        Ok(cache.check_binding(
            OrderedStepCacheAdapter::CandidateProgram,
            &self.program.family_fingerprint,
            self.program.context.base(),
            Some(self.unresolved_policy),
        )?)
    }

    /// Reduce the complete weighted sum with early exact grouping. Existing
    /// resident and persistent depth-first results are neither read nor written.
    pub fn reduce_linear_combination_ordered(
        &self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
    ) -> Result<CandidateOrderedReduction, CandidateReductionError> {
        self.reduce_ordered_impl(terms, None, None, Default::default(), Default::default())
    }

    /// Ordered reduction with an explicit frontier weight arithmetic.
    pub fn reduce_linear_combination_ordered_with_arithmetic<'observer>(
        &self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        arithmetic: OrderedWeightArithmetic,
        observer: Option<&'observer mut Observer<'observer>>,
    ) -> Result<CandidateOrderedReduction, CandidateReductionError> {
        self.reduce_ordered_impl(terms, observer, None, arithmetic, Default::default())
    }

    /// Ordered reduction with an explicit weight arithmetic and scheduler
    /// options. The result, guards, statistics, progress and every
    /// observation are independent of `scheduler.threads`; the observer and
    /// the inspector run on the calling thread.
    pub fn reduce_linear_combination_ordered_with_scheduler<'observer>(
        &self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        arithmetic: OrderedWeightArithmetic,
        scheduler: OrderedSchedulerOptions,
        observer: Option<&'observer mut Observer<'observer>>,
        inspector: Option<&'observer mut OrderedPhaseInspector<'observer, CandidateReductionError>>,
    ) -> Result<CandidateOrderedReduction, CandidateReductionError> {
        self.reduce_ordered_impl(terms, observer, inspector, arithmetic, scheduler)
    }

    pub fn reduce_linear_combination_ordered_with_observation<'observer>(
        &self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        observer: &'observer mut Observer<'observer>,
    ) -> Result<CandidateOrderedReduction, CandidateReductionError> {
        self.reduce_ordered_impl(
            terms,
            Some(observer),
            None,
            Default::default(),
            Default::default(),
        )
    }

    /// Inspect both boundaries of every frozen phase under the selected weight
    /// arithmetic. Inspection borrows the scheduler state (weights are exact
    /// unless the arithmetic is modular without the exact cross-check); an
    /// inspector error stops this request only.
    pub fn reduce_linear_combination_ordered_with_inspector<'observer>(
        &self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        arithmetic: OrderedWeightArithmetic,
        inspector: &'observer mut OrderedPhaseInspector<'observer, CandidateReductionError>,
    ) -> Result<CandidateOrderedReduction, CandidateReductionError> {
        self.reduce_ordered_impl(terms, None, Some(inspector), arithmetic, Default::default())
    }

    fn reduce_ordered_impl<'observer>(
        &self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        observer: Option<&'observer mut Observer<'observer>>,
        inspector: Option<&'observer mut OrderedPhaseInspector<'observer, CandidateReductionError>>,
        arithmetic: OrderedWeightArithmetic,
        scheduler: OrderedSchedulerOptions,
    ) -> Result<CandidateOrderedReduction, CandidateReductionError> {
        let mut request = ReductionRequest::default();
        let mut initial_work = ReductionStatistics::default();
        let mut initial = BTreeMap::new();
        let mut conditions = Vec::new();
        let mut guards = GuardIndex::new();
        for (key, coefficient) in terms {
            // Every original root is checked before cancellation, including a
            // zero-weight or duplicate root. Descendants do not cross this gate.
            self.validate_entry_target(&key)?;
            for condition in self.target_conditions_with_limits(&key, self.limits)? {
                guards.intern(&mut conditions, &condition, |_| Ok(()))?;
            }
            self.program
                .context
                .base()
                .validate_with_limits(&coefficient, self.limits.exact_algebra)?;
            if !coefficient.denominator.is_constant() {
                guards.intern(
                    &mut conditions,
                    &coefficient.denominator.clone().into(),
                    |_| Ok(()),
                )?;
            }
            accumulate_master_in_request(
                self.program.context.base(),
                &mut initial,
                &key,
                coefficient,
                self.limits,
                &mut request,
                &mut initial_work,
            )?;
        }
        let support_phases = self.program.ordering.is_support_primary();
        if let Some(cache) = &self.ordered_step_cache {
            self.check_ordered_step_cache(cache)?;
        }
        let output = run_with_step_cache_and_options(
            &mut Kernel {
                reducer: self,
                support_phases,
                observer,
                inspector,
            },
            &mut request,
            initial,
            conditions,
            arithmetic,
            scheduler,
            self.ordered_step_cache.as_deref(),
        )?;
        initial_work.merge_work(output.work);
        let unresolved = output
            .terms
            .keys()
            .filter(|key| !self.canonical_terminals().contains(*key))
            .cloned()
            .collect();
        let mut progress = output.progress;
        progress.phase_kind = CandidateOrderedPhaseKind::Reduction;
        Ok(CandidateOrderedReduction {
            family_fingerprint: self.program.family_fingerprint.clone(),
            terms: output.terms,
            unresolved,
            nonzero_conditions: output.nonzero_conditions,
            statistics: initial_work,
            progress,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sector::OrderingPolicy;
    use crate::solver::candidate_reduction::owner_test_support::*;

    fn fixture() -> CandidateReducer<1> {
        let family = crate::solver::tests::tadpole();
        let selected = input(
            [true],
            None,
            vec![
                rule(&family, [4], &[([3], 1), ([2], 3), ([1], -2)]),
                rule(&family, [3], &[([2], -3), ([1], 2)]),
                rule(&family, [2], &[([1], 2)]),
            ],
            &[[1]],
        );
        CandidateReducer::try_new(
            &family,
            [true],
            OrderingPolicy::SpiredUncutV1,
            [([true], selected.solution)],
            vec![],
            Default::default(),
        )
        .unwrap()
    }

    #[test]
    fn ordered_candidate_combines_roots_without_reading_or_populating_dfs_cache() {
        let mut reducer = fixture();
        let context = reducer.program.context.base().clone();
        let baseline = reducer.reduce_unit_mass(&key([3])).unwrap();
        let retained = reducer.statistics();
        // A forward-pass property: the adjoint tail (on by default) steps
        // erased keys after the run to complete its values.
        let result = reducer
            .reduce_linear_combination_ordered_with_scheduler(
                [(key([3]), context.one()), (key([2]), context.integer(3))],
                OrderedWeightArithmetic::default(),
                OrderedSchedulerOptions {
                    adjoint_tail: false,
                    ..Default::default()
                },
                None,
                None,
            )
            .unwrap();
        assert_eq!(
            result.terms(),
            &BTreeMap::from([(key([1]), context.integer(2))])
        );
        assert!(result.unresolved().is_empty());
        assert_eq!(result.statistics().rule_applications(), 1);
        assert_eq!(result.statistics().cache_hits(), 0);
        assert_eq!(reducer.statistics(), retained);
        assert_eq!(reducer.reduce_unit_mass(&key([3])).unwrap(), baseline);
        assert_eq!(
            result.progress().completed_phases,
            result.progress().total_phases
        );
        assert_eq!(
            result.progress().phase_kind,
            CandidateOrderedPhaseKind::Reduction
        );
    }

    #[test]
    fn ordered_candidate_cancellation_resolves_before_uncovered_leaf_classification() {
        let mut reducer = fixture();
        let context = reducer.program.context.base().clone();
        let program = Arc::get_mut(&mut reducer.program).unwrap();
        program.rules.get_mut(&[true]).unwrap().pop();
        reducer
            .set_unresolved_policy(UnresolvedPolicy::Preserve)
            .unwrap();
        let result = reducer
            .reduce_linear_combination_ordered([(key([4]), context.one())])
            .unwrap();
        assert!(result.is_zero());
        assert!(result.unresolved().is_empty());
        assert_eq!(result.statistics().rule_applications(), 2);
        assert_eq!(result.progress().cancelled_terms, 2);
    }

    #[test]
    fn ordered_phase_inspection_sees_exact_state_and_stops_with_its_error() {
        use crate::reduction::{OrderedKeyStage, OrderedPhaseBoundary, OrderedWeightArithmetic};
        let reducer = fixture();
        let context = reducer.program.context.base().clone();
        let inputs = [(key([3]), context.one()), (key([2]), context.integer(3))];
        let expected = reducer
            .reduce_linear_combination_ordered(inputs.clone())
            .unwrap();
        let mut boundaries = Vec::new();
        let mut first_frontier = Vec::new();
        let mut final_terminals = BTreeMap::new();
        let actual = reducer
            .reduce_linear_combination_ordered_with_inspector(
                inputs.clone(),
                OrderedWeightArithmetic::Exact,
                &mut |event| {
                    boundaries.push((event.boundary, event.phase, event.progress.completed_phases));
                    if boundaries.len() == 1 {
                        first_frontier = event
                            .frontier()
                            .map(|entry| {
                                assert_eq!(entry.stage, OrderedKeyStage::Integral);
                                (entry.integral.clone(), entry.weight.unwrap().clone())
                            })
                            .collect();
                    }
                    final_terminals = event
                        .terminals()
                        .map(|(key, weight)| (key.clone(), weight.unwrap().clone()))
                        .collect();
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(actual.terms(), expected.terms());
        assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
        assert_eq!(actual.progress(), expected.progress());
        assert_eq!(boundaries.len(), 2 * expected.progress().total_phases);
        for (phase, pair) in boundaries.chunks(2).enumerate() {
            assert_eq!(pair[0], (OrderedPhaseBoundary::Started, phase, phase));
            assert_eq!(pair[1], (OrderedPhaseBoundary::Finished, phase, phase + 1));
        }
        // Pop order: the larger key of the first phase comes first.
        assert_eq!(first_frontier, inputs);
        assert_eq!(&final_terminals, expected.terms());
        let stopped = reducer.reduce_linear_combination_ordered_with_inspector(
            inputs,
            OrderedWeightArithmetic::Exact,
            &mut |event| match event.boundary {
                OrderedPhaseBoundary::Finished => Err(CandidateReductionError::InvalidInput(
                    "stop requested".into(),
                )),
                OrderedPhaseBoundary::Started => Ok(()),
            },
        );
        assert_eq!(
            stopped.unwrap_err(),
            CandidateReductionError::InvalidInput("stop requested".into())
        );
    }

    #[test]
    fn ordered_phase_inspection_sees_the_same_keys_under_modular_weights() {
        use crate::reduction::{ModularCommon, OrderedWeightArithmetic, SampledOptions};
        let reducer = fixture();
        let context = reducer.program.context.base().clone();
        let inputs = [(key([4]), context.one()), (key([2]), context.integer(3))];
        // Keys of every frontier entry and terminal, and whether its weight is exact.
        type Snapshot = (Vec<(IntegralKey, bool)>, Vec<(IntegralKey, bool)>);
        let inspect = |arithmetic| {
            let mut snapshots: Vec<Snapshot> = Vec::new();
            let reduced = reducer
                .reduce_linear_combination_ordered_with_inspector(
                    inputs.clone(),
                    arithmetic,
                    &mut |event| {
                        snapshots.push((
                            event
                                .frontier()
                                .map(|entry| (entry.integral.clone(), entry.weight.is_some()))
                                .collect(),
                            event
                                .terminals()
                                .map(|(key, weight)| (key.clone(), weight.is_some()))
                                .collect(),
                        ));
                        Ok(())
                    },
                )
                .unwrap();
            (reduced, snapshots)
        };
        let (exact, exact_snapshots) = inspect(OrderedWeightArithmetic::Exact);
        for cross_check_exact in [false, true] {
            let (modular, snapshots) = inspect(OrderedWeightArithmetic::ModularSampled(
                ModularCommon {
                    threads: 2,
                    cross_check_exact,
                    ..Default::default()
                },
                SampledOptions {
                    batch_points: 4,
                    ..Default::default()
                },
            ));
            assert_eq!(modular.terms(), exact.terms());
            assert_eq!(modular.nonzero_conditions(), exact.nonzero_conditions());
            assert_eq!(snapshots.len(), exact_snapshots.len());
            for (seen, expected) in snapshots.iter().zip(&exact_snapshots) {
                // Same keys in the same order; exact weights only with the cross-check.
                let keys = |entries: &Vec<(IntegralKey, bool)>| {
                    entries
                        .iter()
                        .map(|(key, _)| key.clone())
                        .collect::<Vec<_>>()
                };
                assert_eq!(
                    (keys(&seen.0), keys(&seen.1)),
                    (keys(&expected.0), keys(&expected.1))
                );
                assert!(
                    seen.0
                        .iter()
                        .chain(&seen.1)
                        .all(|(_, exact_weight)| *exact_weight == cross_check_exact)
                );
            }
            assert!(snapshots.iter().any(|(frontier, _)| !frontier.is_empty()));
        }
    }

    #[test]
    fn ordered_step_cache_replays_candidate_steps_exactly() {
        use crate::reduction::ordered_reduction::step_cache::tests::phase_snapshot;
        use crate::reduction::{OrderedStepCacheBinding, OrderedWeightArithmetic};
        let mut reducer = fixture();
        reducer
            .set_unresolved_policy(UnresolvedPolicy::Preserve)
            .unwrap();
        let context = reducer.program.context.base().clone();
        let inputs = [
            (key([4]), context.coefficient_fixture("1/(d-5)")),
            (key([3]), context.coefficient_fixture("d")),
            (key([2]), context.integer(3)),
        ];
        // The forward pass's phase states: a warm cache's tail table would
        // close the single-terminal tail (adjoint tail, on by default; see
        // `ordered_reduction::tail` for its own cache tests).
        let run = |reducer: &CandidateReducer<1>| {
            let mut snapshots = Vec::new();
            let reduced = reducer
                .reduce_linear_combination_ordered_with_scheduler(
                    inputs.clone(),
                    OrderedWeightArithmetic::Exact,
                    crate::reduction::OrderedSchedulerOptions {
                        adjoint_tail: false,
                        ..Default::default()
                    },
                    None,
                    Some(&mut |event| {
                        snapshots.push(phase_snapshot(event));
                        Ok(())
                    }),
                )
                .unwrap();
            (reduced, snapshots)
        };
        let (expected, expected_snapshots) = run(&reducer);
        let binding = reducer.ordered_step_cache_binding();
        assert_eq!(binding.adapter, OrderedStepCacheAdapter::CandidateProgram);
        assert_eq!(binding.unresolved_policy, Some(UnresolvedPolicy::Preserve));
        let cache = OrderedStepCache::in_memory(binding.clone(), Default::default());
        reducer.set_ordered_step_cache(Some(cache.clone())).unwrap();
        // Forked sessions share the attached cache.
        let fork = reducer.fork_empty();
        for (pass, session) in [&reducer, &fork].into_iter().enumerate() {
            let (actual, snapshots) = run(session);
            assert_eq!(actual.terms(), expected.terms());
            assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
            assert_eq!(snapshots, expected_snapshots);
            let processed = expected.progress().processed_keys;
            let (hits, misses) = if pass == 0 {
                (0, processed)
            } else {
                (processed, 0)
            };
            assert_eq!(
                (
                    actual.progress().step_cache_hits,
                    actual.progress().step_cache_misses
                ),
                (hits, misses)
            );
        }
        assert_eq!(
            cache.statistics().records,
            expected.progress().processed_keys
        );
        // A cache bound to another family or adapter is refused.
        let mut session = reducer.fork_empty();
        for foreign in [
            OrderedStepCacheBinding {
                family_fingerprint: "another family".into(),
                ..binding.clone()
            },
            OrderedStepCacheBinding {
                adapter: OrderedStepCacheAdapter::SavedApplication,
                ..binding.clone()
            },
        ] {
            let foreign = OrderedStepCache::in_memory(foreign, Default::default());
            assert!(session.set_ordered_step_cache(Some(foreign)).is_err());
        }
        // A cache bound to another residual policy is refused.
        let mut rejecting = fixture();
        let other = OrderedStepCache::in_memory(binding, Default::default());
        assert!(rejecting.set_ordered_step_cache(Some(other)).is_err());
    }

    /// The concurrent scheduler replays and populates an attached step cache
    /// exactly like the sequential one: same terms, guards and phase states,
    /// the same hits and misses, and the same records (only expanded keys are
    /// recorded, never speculative steps).
    #[test]
    fn ordered_step_cache_under_concurrent_scheduler_matches_sequential() {
        use crate::reduction::{
            ModularCommon, OrderedSchedulerOptions, OrderedWeightArithmetic, PolynomialOptions,
            SampledOptions,
        };
        let mut reducer = fixture();
        reducer
            .set_unresolved_policy(UnresolvedPolicy::Preserve)
            .unwrap();
        let context = reducer.program.context.base().clone();
        let inputs = [
            (key([4]), context.coefficient_fixture("1/(d-5)")),
            (key([3]), context.coefficient_fixture("d")),
            (key([2]), context.integer(3)),
            (key([1]), context.integer(-1)),
        ];
        let snapshot = |event: &OrderedPhaseEvent<'_>| {
            let mut lines = vec![format!(
                "{:?} {} {:?}",
                event.boundary, event.phase, event.progress
            )];
            for entry in event.frontier() {
                lines.push(format!(
                    "f {} {:?} {:?}",
                    entry.phase,
                    entry.integral.powers(),
                    entry
                        .weight
                        .map(|weight| weight.to_expression().to_string())
                ));
            }
            for (key, weight) in event.terminals() {
                lines.push(format!(
                    "t {:?} {:?}",
                    key.powers(),
                    weight.map(|weight| weight.to_expression().to_string())
                ));
            }
            for guard in event.nonzero_conditions() {
                lines.push(format!("g {}", guard.to_expression()));
            }
            lines
        };
        let run = |reducer: &CandidateReducer<1>,
                   arithmetic: OrderedWeightArithmetic,
                   scheduler: OrderedSchedulerOptions| {
            let mut snapshots = Vec::new();
            let reduced = reducer
                .reduce_linear_combination_ordered_with_scheduler(
                    inputs.clone(),
                    arithmetic,
                    scheduler,
                    None,
                    Some(&mut |event| {
                        snapshots.push(snapshot(event));
                        Ok(())
                    }),
                )
                .unwrap();
            (reduced, snapshots)
        };
        let binding = reducer.ordered_step_cache_binding();
        let common = ModularCommon {
            threads: 2,
            ..Default::default()
        };
        // Forward-pass phase states and step records: a warm cache's tail
        // table would close the tail (adjoint tail, on by default).
        let sequential = OrderedSchedulerOptions {
            threads: 1,
            window_keys: 1,
            adjoint_tail: false,
            profile: Default::default(),
        };
        for arithmetic in [
            OrderedWeightArithmetic::Exact,
            OrderedWeightArithmetic::ModularSampled(common, SampledOptions::default()),
            OrderedWeightArithmetic::ModularPolynomial(common, PolynomialOptions::default()),
        ] {
            reducer.set_ordered_step_cache(None).unwrap();
            let (expected, expected_snapshots) = run(&reducer, arithmetic, sequential);
            let processed = expected.progress().processed_keys;
            let reference = OrderedStepCache::in_memory(binding.clone(), Default::default());
            reducer
                .set_ordered_step_cache(Some(reference.clone()))
                .unwrap();
            run(&reducer, arithmetic, sequential);
            let reference_records = reference.statistics().records;
            assert_eq!(reference_records, processed);
            for (threads, window_keys) in [(2, 1), (3, 2), (4, 4096)] {
                let scheduler = OrderedSchedulerOptions {
                    threads,
                    window_keys,
                    adjoint_tail: false,
                    profile: Default::default(),
                };
                let cache = OrderedStepCache::in_memory(binding.clone(), Default::default());
                reducer.set_ordered_step_cache(Some(cache.clone())).unwrap();
                for (pass, scheduler) in [scheduler, scheduler, sequential].into_iter().enumerate()
                {
                    let (actual, snapshots) = run(&reducer, arithmetic, scheduler);
                    let context = format!("{arithmetic:?} {scheduler:?} pass {pass}");
                    assert_eq!(actual.terms(), expected.terms(), "{context}");
                    assert_eq!(
                        actual.nonzero_conditions(),
                        expected.nonzero_conditions(),
                        "{context}"
                    );
                    // Phase states without the cache counters.
                    let strip = |snapshots: &Vec<Vec<String>>| -> Vec<Vec<String>> {
                        snapshots
                            .iter()
                            .map(|lines| {
                                let mut lines = lines.clone();
                                lines[0] = lines[0]
                                    .split(", step_cache_hits")
                                    .next()
                                    .unwrap()
                                    .to_owned();
                                lines
                            })
                            .collect()
                    };
                    assert_eq!(strip(&snapshots), strip(&expected_snapshots), "{context}");
                    let (hits, misses) = if pass == 0 {
                        (0, processed)
                    } else {
                        (processed, 0)
                    };
                    assert_eq!(
                        (
                            actual.progress().step_cache_hits,
                            actual.progress().step_cache_misses
                        ),
                        (hits, misses),
                        "{context}"
                    );
                    assert_eq!(cache.statistics().records, reference_records, "{context}");
                }
            }
        }
    }

    #[test]
    fn candidate_fixture_records_match_the_step_semantics_golden() {
        use crate::reduction::ordered_reduction::step_cache::tests::{
            assert_golden_records, records_digest,
        };
        let mut reducer = fixture();
        reducer
            .set_unresolved_policy(UnresolvedPolicy::Preserve)
            .unwrap();
        let cache =
            OrderedStepCache::in_memory(reducer.ordered_step_cache_binding(), Default::default());
        reducer.set_ordered_step_cache(Some(cache.clone())).unwrap();
        let context = reducer.program.context.base().clone();
        reducer
            .reduce_linear_combination_ordered([
                (key([4]), context.coefficient_fixture("1/(d-5)")),
                (key([3]), context.coefficient_fixture("d")),
                (key([2]), context.integer(3)),
            ])
            .unwrap();
        assert_golden_records(
            "candidate tadpole",
            &records_digest(&cache),
            "c52b354634ed04dfeb74221c3e4ac73ed585dd394e1807028f060cb518edec43",
        );
    }

    #[test]
    fn ordered_candidate_rejects_malformed_canceled_input() {
        let reducer = fixture();
        let context = reducer.program.context.base();
        assert!(
            reducer
                .reduce_linear_combination_ordered([(key([1, 0]), context.zero())])
                .is_err()
        );
    }
}
