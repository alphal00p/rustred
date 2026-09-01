use crate::foundry::artifact::derive_k6_terminal_authority;
use crate::foundry::artifact::factorized_numerator_lift::{
    FactorizedNumeratorLiftLimits, compile_factorized_numerator_lift,
};

use super::derive::factorization_for_sector;
use super::error::ProbeError;
use super::limits::{HARD_MAX_PHASE_DEGREE, ProbeLimits, checked_total};
use super::model::CornerMomentEvaluator;
use super::recurrence::numerator_powers;
use super::{ARITY, exact_limits};

const PATH_SECTOR: [i64; ARITY] = [0, 0, 1, 0, 1, 1];
const STAR_SECTOR: [i64; ARITY] = [0, 0, 1, 1, 0, 1];
const K3_TIMES_K1_SECTOR: [i64; ARITY] = [0, 0, 1, 1, 1, 1];
const PATH_TRIPLE_NUMERATOR: [i64; ARITY] = [-1, -1, 1, -1, 1, 1];
const STAR_TRIPLE_NUMERATOR: [i64; ARITY] = [-1, -1, 1, 1, -1, 1];

#[test]
fn angular_oracle_consumes_production_routing_for_path_and_star() {
    let limits = ProbeLimits::default();
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();

    let path_rule = factorization_for_sector(authority.factorization_rules(), &PATH_SECTOR);
    let path = compile_factorized_numerator_lift(
        family,
        path_rule,
        FactorizedNumeratorLiftLimits::default(),
    )
    .unwrap();
    let mut path_evaluator = CornerMomentEvaluator::try_new(
        family,
        path_rule,
        path.routing().transformed_denominators(),
        &[0, 1, 2, 3, 4, 5],
        limits,
    )
    .unwrap();
    let path_value = path_evaluator
        .evaluate(numerator_powers(PATH_TRIPLE_NUMERATOR, path_rule, limits).unwrap())
        .unwrap();
    let path_expected = family
        .coefficient_context()
        .coefficient_fixture("2*(d+2)^2/d^2");
    assert!(
        family
            .coefficient_context()
            .try_sub(&path_value, &path_expected, exact_limits())
            .unwrap()
            .is_zero()
    );
    assert!(path_evaluator.affine_transition_count > 0);
    assert!(path_evaluator.angular_transition_count > 0);
    assert!(
        path_evaluator
            .angular_guard_ranks
            .iter()
            .all(|rank| *rank >= 2 && rank % 2 == 0)
    );

    let star_rule = factorization_for_sector(authority.factorization_rules(), &STAR_SECTOR);
    let star = compile_factorized_numerator_lift(
        family,
        star_rule,
        FactorizedNumeratorLiftLimits::default(),
    )
    .unwrap();
    let mut star_evaluator = CornerMomentEvaluator::try_new(
        family,
        star_rule,
        star.routing().transformed_denominators(),
        &[0, 1, 2, 3, 4, 5],
        limits,
    )
    .unwrap();
    let star_value = star_evaluator
        .evaluate(numerator_powers(STAR_TRIPLE_NUMERATOR, star_rule, limits).unwrap())
        .unwrap();
    let star_expected = family
        .coefficient_context()
        .coefficient_fixture("(d^2-8)/d^2");
    assert!(
        family
            .coefficient_context()
            .try_sub(&star_value, &star_expected, exact_limits())
            .unwrap()
            .is_zero()
    );
    assert!(star_evaluator.affine_transition_count > 0);
    assert!(star_evaluator.angular_transition_count > 0);
    assert!(!star_evaluator.angular_guard_ranks.is_empty());
}

#[test]
fn bounded_angular_oracle_rejects_noncorner_or_excess_work() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();
    let rule = factorization_for_sector(authority.factorization_rules(), &PATH_SECTOR);
    let routed =
        compile_factorized_numerator_lift(family, rule, FactorizedNumeratorLiftLimits::default())
            .unwrap();

    assert_eq!(
        checked_total("test degree", &[u64::MAX, 1]),
        Err(ProbeError::DegreeOverflow {
            resource: "test degree",
        })
    );
    let degree_limits = ProbeLimits {
        max_affine_degree: 2,
        ..ProbeLimits::default()
    };
    assert_eq!(
        numerator_powers([-1, -1, 1, 0, 1, 1], rule, degree_limits).unwrap(),
        [1, 1, 0, 0, 0, 0]
    );
    assert!(matches!(
        numerator_powers([-1, -1, 1, -1, 1, 1], rule, degree_limits),
        Err(ProbeError::DegreeLimit {
            resource: "corner numerator degree",
            requested: 3,
            limit: 2,
        })
    ));
    assert_eq!(
        numerator_powers([0, 0, 2, 0, 1, 1], rule, ProbeLimits::default()),
        Err(ProbeError::NonCornerActivePower { slot: 2, power: 2 })
    );
    assert_eq!(
        numerator_powers([1, 0, 1, 0, 1, 1], rule, ProbeLimits::default()),
        Err(ProbeError::ForeignActivePower { slot: 0, power: 1 })
    );
    let raised_structural_limit = ProbeLimits {
        max_affine_degree: HARD_MAX_PHASE_DEGREE + 1,
        ..ProbeLimits::default()
    };
    assert!(matches!(
        numerator_powers(PATH_SECTOR, rule, raised_structural_limit),
        Err(ProbeError::DegreeLimit {
            resource: "configured affine recursion ceiling",
            ..
        })
    ));

    let composite_rule =
        factorization_for_sector(authority.factorization_rules(), &K3_TIMES_K1_SECTOR);
    let composite = compile_factorized_numerator_lift(
        family,
        composite_rule,
        FactorizedNumeratorLiftLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        CornerMomentEvaluator::try_new(
            family,
            composite_rule,
            composite.routing().transformed_denominators(),
            &[0, 1, 2, 3, 4, 5],
            ProbeLimits::default(),
        ),
        Err(ProbeError::UnsupportedCornerFactorization { .. })
    ));

    let cache_limits = ProbeLimits {
        max_affine_cache_entries: 0,
        ..ProbeLimits::default()
    };
    let mut evaluator = CornerMomentEvaluator::try_new(
        family,
        rule,
        routed.routing().transformed_denominators(),
        &[0, 1, 2, 3, 4, 5],
        cache_limits,
    )
    .unwrap();
    assert!(matches!(
        evaluator.evaluate([0; ARITY]),
        Err(ProbeError::CountLimit {
            resource: "affine recurrence cache entries",
            requested: 1,
            limit: 0,
        })
    ));
}
