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
use crate::reduction::{ReductionLimits, ReductionRequest, ReductionStatistics};
use crate::sector::OrderingPolicy;

/// The outcome of one owner step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AppliedOwnerStep {
    /// `key` is a terminal of the owner.
    Terminal,
    /// The rule's children, strictly below `key` in the owner's order, and
    /// the applied rule's position in its chart's rule list.
    Applied(BTreeMap<IntegralKey, Coefficient>, usize),
    /// No rule applies and `key` is no terminal.
    Uncovered,
}

/// Apply the owner of `sector` to `key` (the owner is prepared once, on first
/// use). Conditions of the step are appended to `conditions` without
/// duplicates.
///
/// `sector` is the raw chart of `key`, its slots of positive power: candidate
/// dispatch is keyed by it, and each chart of an eps owner is a sector of its
/// own. Never pass the support with the forced axes (plan sec.3.4). A key of
/// another chart is `InvalidInput`, as in the routed Apply arm.
///
/// The rule is evaluated under the caller's `limits` (the request's, M3-I1),
/// not the owner context's build-time limits; preparing the owner on first
/// use still runs under the context's.
pub(crate) fn apply_prepared_owner<const N: usize>(
    programs: &DeferredCandidateOwnerPrograms<N>,
    sector: &[bool; N],
    key: &IntegralKey,
    conditions: &mut Vec<Coefficient>,
    request: &mut ReductionRequest,
    work: &mut ReductionStatistics,
    limits: &ReductionLimits,
) -> Result<AppliedOwnerStep, CandidateReductionError> {
    let chart: [bool; N] =
        std::array::from_fn(|slot| key.powers().get(slot).copied().unwrap_or(0) > 0);
    if chart != *sector {
        return Err(CandidateReductionError::InvalidInput(
            "apply node does not belong to its owner support".into(),
        ));
    }
    let context = programs.context();
    let shared = &context.shared;
    let limits = *limits;
    let owner = crate::prof_wrap!(OwnerPrepare, programs.owner(sector))?.ok_or_else(|| {
        CandidateReductionError::InvalidInput("the owner sector has no prepared program".into())
    })?;
    if owner.batches[0].terminals.contains(key) {
        return Ok(AppliedOwnerStep::Terminal);
    }
    request.record_rule_application(limits.diagnostic_limit(limits.max_rule_applications))?;
    let mut position = 0;
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
            .apply_with_conditions_rule(key, request, work)
            .map(|(terms, local, rule)| {
                position = rule;
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
            AppliedOwnerStep::Applied(terms, position)
        }
    })
}

/// The order `sector`'s rules were generated in, without preparing the owner.
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
    use crate::sector::{Mask, zero};
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

    /// The chart `(0, 1)` with its one key as a terminal.
    #[derive(Debug)]
    struct ChartLoader;

    impl DeferredCandidateOwnerLoader<2> for ChartLoader {
        fn prepare(
            &self,
            sector: [bool; 2],
            context: Arc<CandidateOwnerContext<2>>,
        ) -> Result<CandidateOwnerPrograms<2>, CandidateReductionError> {
            CandidateOwnerPrograms::try_new(context, [input(sector, None, vec![], &[[0, 1]])])
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
                &ReductionLimits::default(),
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
            AppliedOwnerStep::Applied(BTreeMap::from([(key([1, 1]), three)]), 0)
        );
        // A key of another chart is a misuse, not an uncovered key.
        for powers in [[1, 0], [2, -1], [0, 1], [0, 0]] {
            assert!(matches!(
                apply_prepared_owner(
                    &programs,
                    &sector,
                    &key(powers),
                    &mut conditions,
                    &mut request,
                    &mut work,
                    &ReductionLimits::default(),
                ),
                Err(CandidateReductionError::InvalidInput(_))
            ));
        }
        assert!(conditions.is_empty());
        assert_eq!(work.rule_applications(), 1);
    }

    /// A guard of the owner reaches the caller, once however often the step runs.
    #[test]
    fn the_conditions_of_a_step_reach_the_caller() {
        let family = bubble(0);
        let mut context = CandidateOwnerContext::<2>::try_new_p(
            Arc::clone(&family),
            scope(),
            vec![],
            Default::default(),
        )
        .unwrap();
        let guard = family.coefficient_context().coefficient_fixture("d-4");
        let lifted = context
            .shared
            .context
            .lift_base_polynomial(&guard.numerator)
            .unwrap();
        context.shared.source_conditions.push(lifted);
        let programs = DeferredCandidateOwnerPrograms::try_new(
            Arc::new(context),
            [DeferredCandidateOwnerDescriptor {
                sector: [true, true],
                saved_root: [true; 2],
                ordering: OrderingPolicy::SpiredUncutV1,
                terminals: BTreeSet::from([key([1, 1])]),
            }],
            Arc::new(Loader),
        )
        .unwrap();
        let mut request = ReductionRequest::default();
        let mut work = ReductionStatistics::default();
        let mut conditions = Vec::new();
        for _ in 0..2 {
            let step = apply_prepared_owner(
                &programs,
                &[true, true],
                &key([2, 1]),
                &mut conditions,
                &mut request,
                &mut work,
                &ReductionLimits::default(),
            )
            .unwrap();
            assert!(matches!(step, AppliedOwnerStep::Applied(..)));
            assert_eq!(conditions, std::slice::from_ref(&guard));
        }
    }

    /// The chart `n <= 0` of an eps axis is its own sector, inside the full root:
    /// its key passes, and the full support is refused for it.
    #[test]
    fn a_chart_of_an_eps_owner_is_its_own_sector() {
        let family = bubble(1);
        let context = Arc::new(
            CandidateOwnerContext::<2>::try_new_p(
                Arc::clone(&family),
                scope(),
                vec![],
                Default::default(),
            )
            .unwrap(),
        );
        let chart = [false, true];
        let programs = DeferredCandidateOwnerPrograms::try_new(
            context,
            [DeferredCandidateOwnerDescriptor {
                sector: chart,
                saved_root: [true; 2],
                ordering: OrderingPolicy::SpiredUncutV1,
                terminals: BTreeSet::from([key([0, 1])]),
            }],
            Arc::new(ChartLoader),
        )
        .unwrap();
        let mut request = ReductionRequest::default();
        let mut work = ReductionStatistics::default();
        let mut conditions = Vec::new();
        assert_eq!(
            apply_prepared_owner(
                &programs,
                &chart,
                &key([0, 1]),
                &mut conditions,
                &mut request,
                &mut work,
                &ReductionLimits::default(),
            )
            .unwrap(),
            AppliedOwnerStep::Terminal
        );
        assert!(matches!(
            apply_prepared_owner(
                &programs,
                &[true, true],
                &key([0, 1]),
                &mut conditions,
                &mut request,
                &mut work,
                &ReductionLimits::default(),
            ),
            Err(CandidateReductionError::InvalidInput(_))
        ));
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

    /// A shift the p admission refuses, on a family of p shape.
    #[test]
    fn the_p_constructor_refuses_a_shift_off_the_p_path() {
        let plain = bubble(0);
        let context = plain.coefficient_context().clone();
        let bad = IntegralFamily::new(
            "shifted by one",
            vec!["k1".to_owned()],
            vec!["q".to_owned()],
            context.clone(),
            context.parameter("d").unwrap(),
            plain.denominators().to_vec(),
            vec![vec![context.integer(-1)]],
            vec![context.integer(1), context.zero()],
        )
        .unwrap();
        assert!(matches!(
            CandidateOwnerContext::<2>::try_new_p(
                Arc::new(bad),
                scope(),
                vec![],
                Default::default()
            ),
            Err(CandidateReductionError::UnsupportedFamily(_))
        ));
    }

    /// Rank-finite retention needs a rank, on both branches.
    #[test]
    fn the_p_constructor_requires_a_rank_for_rank_finite_owners() {
        for units in [0, 1] {
            let refused = CandidateOwnerContext::<2>::try_new_p(
                bubble(units),
                CandidateOwnerScope {
                    max_numerator_rank: None,
                    finite_case_policy: crate::solver::FiniteCasePolicy::RetainRankFinite,
                },
                vec![],
                Default::default(),
            );
            assert!(matches!(
                refused,
                Err(CandidateReductionError::InvalidInput(_))
            ));
        }
    }

    fn proved_zero(family: &IntegralFamily, sector: [bool; 2]) -> zero::Certificate {
        let decision = zero::Analyzer::try_unrestricted(family)
            .unwrap()
            .analyze(&Mask::try_new(sector).unwrap())
            .unwrap();
        let zero::Decision::ProvedZero(certificate) = decision else {
            panic!("sector {sector:?} is scaleless, got {decision:?}");
        };
        certificate
    }

    /// The eps branch has its own copy of the certificate binding, and a sector
    /// in `zero_sectors` evaluates to zero: a foreign certificate there would be
    /// a silent zero. Every family here has the arity of the owner, so only the
    /// family decides.
    #[test]
    fn the_p_constructor_binds_zero_certificates_to_their_family() {
        let owner = bubble(1);
        for other in [bubble(0), bubble(2)] {
            assert!(matches!(
                CandidateOwnerContext::<2>::try_new_p(
                    Arc::clone(&owner),
                    scope(),
                    vec![proved_zero(&other, [false, false])],
                    Default::default(),
                ),
                Err(CandidateReductionError::InvalidInput(_))
            ));
        }
    }

    /// A certificate of the family marks its raw chart. The eps line stays in the
    /// support of every chart, so the chart of the other line has no proof.
    #[test]
    fn the_p_constructor_marks_the_certified_charts_of_an_eps_family() {
        let owner = bubble(1);
        let charts = [[false, false], [true, false]];
        let context = CandidateOwnerContext::<2>::try_new_p(
            Arc::clone(&owner),
            scope(),
            charts
                .iter()
                .map(|&chart| proved_zero(&owner, chart))
                .collect(),
            Default::default(),
        )
        .unwrap();
        assert_eq!(context.shared.zero_sectors, BTreeSet::from(charts));
        assert!(matches!(
            zero::Analyzer::try_unrestricted(&owner)
                .unwrap()
                .analyze(&Mask::try_new([false, true]).unwrap())
                .unwrap(),
            zero::Decision::Inconclusive(_)
        ));
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
