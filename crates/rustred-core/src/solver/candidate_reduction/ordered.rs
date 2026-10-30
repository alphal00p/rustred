//! Generic ordered adapter for immutable, possibly multi-sector candidate tables.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::{CandidateReducer, CandidateReductionError, UnresolvedPolicy};
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::IntegralKey;
use crate::reduction::guard_union::GuardIndex;
use crate::reduction::ordered_reduction::{
    self, OrderedKernel, OrderedKeyStage, OrderedPhaseDescriptor, OrderedPhaseEvent,
    OrderedPhaseInspector, OrderedPosition, OrderedStep,
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
        let priority = self
            .reducer
            .program
            .ordering
            .complexity_key(key.powers())
            .map_err(ReductionError::Ordering)?;
        let phase = if self.support_phases {
            self.reducer.physical_arity() - priority.propagators()
        } else {
            0
        };
        Ok(OrderedPosition { phase, priority })
    }
    fn step(
        &mut self,
        key: &IntegralKey,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<OrderedStep<IntegralKey>, Self::Error> {
        let mut conditions = self
            .reducer
            .target_conditions_with_limits(key, self.limits())?;
        if self.reducer.is_zero(key) {
            return Ok(OrderedStep {
                nonzero_conditions: conditions,
                ..Default::default()
            });
        }
        if self.reducer.program.terminals.contains(key) {
            let terminals = self.reducer.terminal_terms(key)?;
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
        request.record_rule_application(
            self.limits()
                .diagnostic_limit(self.limits().max_rule_applications),
        )?;
        let result = self
            .reducer
            .program
            .evaluator(key, self.limits())
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
                if self.reducer.unresolved_policy == UnresolvedPolicy::Preserve =>
            {
                Ok(OrderedStep {
                    terminals: BTreeMap::from([(key.clone(), self.context().one())]),
                    nonzero_conditions: conditions,
                    ..Default::default()
                })
            }
            Err(error) => Err(error),
        }
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

impl<const N: usize> CandidateReducer<N> {
    /// Reduce the complete weighted sum with early exact grouping. Existing
    /// resident and persistent depth-first results are neither read nor written.
    pub fn reduce_linear_combination_ordered(
        &self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
    ) -> Result<CandidateOrderedReduction, CandidateReductionError> {
        self.reduce_ordered_impl(terms, None, None)
    }

    pub fn reduce_linear_combination_ordered_with_observation<'observer>(
        &self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        observer: &'observer mut Observer<'observer>,
    ) -> Result<CandidateOrderedReduction, CandidateReductionError> {
        self.reduce_ordered_impl(terms, Some(observer), None)
    }

    /// Inspect both boundaries of every frozen phase. Inspection borrows the
    /// exact scheduler state; an inspector error stops this request only.
    pub fn reduce_linear_combination_ordered_with_inspector<'observer>(
        &self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        inspector: &'observer mut OrderedPhaseInspector<'observer, CandidateReductionError>,
    ) -> Result<CandidateOrderedReduction, CandidateReductionError> {
        self.reduce_ordered_impl(terms, None, Some(inspector))
    }

    fn reduce_ordered_impl<'observer>(
        &self,
        terms: impl IntoIterator<Item = (IntegralKey, Coefficient)>,
        observer: Option<&'observer mut Observer<'observer>>,
        inspector: Option<&'observer mut OrderedPhaseInspector<'observer, CandidateReductionError>>,
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
        let output = ordered_reduction::run(
            &mut Kernel {
                reducer: self,
                support_phases,
                observer,
                inspector,
            },
            &mut request,
            initial,
            conditions,
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
        let result = reducer
            .reduce_linear_combination_ordered([
                (key([3]), context.one()),
                (key([2]), context.integer(3)),
            ])
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
        use crate::reduction::{OrderedKeyStage, OrderedPhaseBoundary};
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
            .reduce_linear_combination_ordered_with_inspector(inputs.clone(), &mut |event| {
                boundaries.push((event.boundary, event.phase, event.progress.completed_phases));
                if boundaries.len() == 1 {
                    first_frontier = event
                        .frontier()
                        .map(|entry| {
                            assert_eq!(entry.stage, OrderedKeyStage::Integral);
                            (entry.integral.clone(), entry.weight.clone())
                        })
                        .collect();
                }
                final_terminals = event.terminals().clone();
                Ok(())
            })
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
        let stopped =
            reducer.reduce_linear_combination_ordered_with_inspector(inputs, &mut |event| {
                match event.boundary {
                    OrderedPhaseBoundary::Finished => Err(CandidateReductionError::InvalidInput(
                        "stop requested".into(),
                    )),
                    OrderedPhaseBoundary::Started => Ok(()),
                }
            });
        assert_eq!(
            stopped.unwrap_err(),
            CandidateReductionError::InvalidInput("stop requested".into())
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
