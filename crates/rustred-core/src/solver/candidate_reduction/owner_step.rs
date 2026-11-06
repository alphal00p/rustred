//! One owner step outside the routed application, for the p library's Custom
//! lane (owner: M2-T6 rr-plib).
//!
//! [`apply_prepared_owner`] is a copy of the Apply arm of the routed checked
//! expansion (`routed/application/engine/checked.rs`), not a shared helper, so
//! that the saved-application path stays untouched. Unlike the routed arm it
//! leaves the decision on an uncovered key to the caller: a p library rejects
//! it with a typed error.
use std::collections::BTreeMap;

use super::CandidateReductionError;
use super::evaluator::CandidateEvaluator;
use super::owners::{DeferredCandidateOwnerPrograms, OwnerStep};
use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use crate::reduction::{ReductionRequest, ReductionStatistics};
use crate::sector::OrderingPolicy;

/// The outcome of one owner step.
#[allow(dead_code)] // M2-T7c
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AppliedOwnerStep {
    /// `key` is a terminal of the owner.
    Terminal,
    /// The rule's children, strictly below `key` in the owner's order.
    Applied(BTreeMap<IntegralKey, Coefficient>),
    /// No rule applies and `key` is no terminal.
    Uncovered,
}

/// Apply the owner of `sector` to `key` (the owner is prepared once, on first
/// use). Conditions of the step are appended to `conditions` without
/// duplicates.
#[allow(dead_code)] // M2-T7c
pub(crate) fn apply_prepared_owner<const N: usize>(
    programs: &DeferredCandidateOwnerPrograms<N>,
    sector: &[bool; N],
    key: &IntegralKey,
    conditions: &mut Vec<Coefficient>,
    request: &mut ReductionRequest,
    work: &mut ReductionStatistics,
) -> Result<AppliedOwnerStep, CandidateReductionError> {
    let context = programs.context();
    let shared = &context.shared;
    let limits = context.limits;
    let owner = programs.owner(sector)?.ok_or_else(|| {
        CandidateReductionError::InvalidInput("the owner sector has no prepared program".into())
    })?;
    if owner.batches[0].terminals.contains(key) {
        return Ok(AppliedOwnerStep::Terminal);
    }
    request.record_rule_application(limits.diagnostic_limit(limits.max_rule_applications))?;
    let result = owner.evaluate_step::<CandidateReductionError>(key, |batch| {
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
            .apply_with_conditions(key, request, work)
            .map(|(terms, local)| {
                for condition in local {
                    if !conditions.contains(&condition) {
                        conditions.push(condition);
                    }
                }
                terms
            }))
    })?;
    Ok(match result {
        OwnerStep::Terminal => AppliedOwnerStep::Terminal,
        OwnerStep::Uncovered => AppliedOwnerStep::Uncovered,
        OwnerStep::Applied(terms) => {
            work.record_rule_application();
            AppliedOwnerStep::Applied(terms)
        }
    })
}

/// The order `sector`'s rules were generated in, without preparing the owner.
#[allow(dead_code)] // M2-T7c
pub(crate) fn owner_ordering<'p, const N: usize>(
    programs: &'p DeferredCandidateOwnerPrograms<N>,
    sector: &[bool; N],
) -> Option<&'p OrderingPolicy> {
    programs.owner_ordering(sector)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use super::*;
    use crate::algebra::CoefficientContext;
    use crate::family::IntegralFamily;
    use crate::propagator::PresentationId;
    use crate::propagator::presentation::{Presentation, PresentationKind, Slot, SlotLine};
    use crate::solver::candidate_reduction::owner_test_support::{input, key, rule};
    use crate::solver::candidate_reduction::{
        CandidateOwnerContext, CandidateOwnerPrograms, CandidateOwnerScope,
        DeferredCandidateOwnerDescriptor, DeferredCandidateOwnerLoader,
    };

    fn bubble(eps_units: u8) -> Arc<IntegralFamily> {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let line = |momentum: [i64; 2], eps_units| {
            Slot::Line(SlotLine {
                momentum: momentum.to_vec(),
                eps_units,
            })
        };
        Presentation::from_slots(
            PresentationId(0),
            PresentationKind::EpsNode,
            "bubble",
            1,
            vec![line([1, 0], eps_units), line([1, -1], 0)],
            &context,
            None,
        )
        .unwrap()
        .family
    }

    fn scope() -> CandidateOwnerScope {
        CandidateOwnerScope {
            max_numerator_rank: None,
            finite_case_policy: Default::default(),
        }
    }

    /// `I(2, 1) -> 3 I(1, 1)`, a terminal at `I(1, 1)`.
    #[derive(Debug)]
    struct Loader;

    impl DeferredCandidateOwnerLoader<2> for Loader {
        fn prepare(
            &self,
            sector: [bool; 2],
            context: Arc<CandidateOwnerContext<2>>,
        ) -> Result<CandidateOwnerPrograms<2>, CandidateReductionError> {
            let family = Arc::clone(context.family_owner());
            CandidateOwnerPrograms::try_new(
                context,
                [input(
                    sector,
                    None,
                    vec![rule(&family, [2, 1], &[([1, 1], 3)])],
                    &[[1, 1]],
                )],
            )
        }
    }

    #[test]
    fn a_prepared_owner_applies_outside_the_routed_application() {
        let family = bubble(0);
        let context = Arc::new(
            CandidateOwnerContext::<2>::try_new_p(
                Arc::clone(&family),
                scope(),
                vec![],
                Default::default(),
            )
            .unwrap(),
        );
        let coefficients = family.coefficient_context().clone();
        let programs = DeferredCandidateOwnerPrograms::try_new(
            context,
            [DeferredCandidateOwnerDescriptor {
                sector: [true, true],
                saved_root: [true; 2],
                ordering: OrderingPolicy::SpiredUncutV1,
                terminals: BTreeSet::from([key([1, 1])]),
            }],
            Arc::new(Loader),
        )
        .unwrap();
        let sector = [true, true];
        assert_eq!(
            owner_ordering(&programs, &sector),
            Some(&OrderingPolicy::SpiredUncutV1)
        );
        assert_eq!(owner_ordering(&programs, &[true, false]), None);
        let mut request = ReductionRequest::default();
        let mut work = ReductionStatistics::default();
        let mut conditions = Vec::new();
        let mut step = |powers| {
            apply_prepared_owner(
                &programs,
                &sector,
                &key(powers),
                &mut conditions,
                &mut request,
                &mut work,
            )
            .unwrap()
        };
        assert_eq!(step([1, 1]), AppliedOwnerStep::Terminal);
        assert_eq!(step([1, 2]), AppliedOwnerStep::Uncovered);
        let Some(three) = coefficients.parameter("d").map(|_| coefficients.integer(3)) else {
            unreachable!("the family has d");
        };
        assert_eq!(
            step([2, 1]),
            AppliedOwnerStep::Applied(BTreeMap::from([(key([1, 1]), three)]))
        );
        assert!(conditions.is_empty());
        assert_eq!(work.rule_applications(), 1);
    }

    #[test]
    fn the_p_constructor_refuses_what_the_p_path_does_not_admit() {
        // A vacuum family is no p family.
        let vacuum = Arc::new(crate::solver::tests::sunset());
        assert!(matches!(
            CandidateOwnerContext::<3>::try_new_p(vacuum, scope(), vec![], Default::default()),
            Err(CandidateReductionError::UnsupportedFamily(_))
        ));
        // A narrower storage than the family.
        assert!(matches!(
            CandidateOwnerContext::<1>::try_new_p(bubble(0), scope(), vec![], Default::default()),
            Err(CandidateReductionError::InvalidInput(_))
        ));
        // Every other consumer keeps refusing eps families.
        assert!(
            CandidateOwnerContext::<2>::try_new(bubble(1), scope(), vec![], Default::default())
                .is_err()
        );
    }

    #[test]
    fn the_p_constructor_admits_eps_families() {
        for units in 1..=3 {
            let family = bubble(units);
            let context = CandidateOwnerContext::<2>::try_new_p(
                Arc::clone(&family),
                scope(),
                vec![],
                Default::default(),
            )
            .unwrap();
            assert_eq!(context.family().fingerprint(), family.fingerprint());
            // The p admission is the only door: the same family stays refused
            // by the unit-scale predicate.
            assert!(
                family.unit_scale_kinematics().is_err()
                    || family
                        .unit_scale_kinematics()
                        .is_ok_and(|kinematics| kinematics.is_none())
            );
        }
        assert!(matches!(
            CandidateOwnerContext::<1>::try_new_p(bubble(1), scope(), vec![], Default::default()),
            Err(CandidateReductionError::InvalidInput(_))
        ));
    }
}
