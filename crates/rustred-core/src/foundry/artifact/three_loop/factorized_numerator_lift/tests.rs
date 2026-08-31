use crate::foundry::artifact::derive_k6_terminal_authority;
use crate::sector::{CoordinatePriority, CoordinatePriorityLimits};

use super::derive::{best_routed_basis, factorization_for_sector, replay_relation, routed_base};
use super::error::ProbeError;
use super::limits::{HARD_MAX_PHASE_DEGREE, ProbeLimits, checked_total};
use super::model::{AffinePowerState, CornerMomentEvaluator};
use super::recurrence::{affine_power_step, numerator_powers};
use super::{ARITY, exact_limits};

const PATH_SECTOR: [i64; ARITY] = [0, 0, 1, 0, 1, 1];
const STAR_SECTOR: [i64; ARITY] = [0, 0, 1, 1, 0, 1];
const K3_TIMES_K1_SECTOR: [i64; ARITY] = [0, 0, 1, 1, 1, 1];
const PATH_TRIPLE_NUMERATOR: [i64; ARITY] = [-1, -1, 1, -1, 1, 1];
const STAR_TRIPLE_NUMERATOR: [i64; ARITY] = [-1, -1, 1, 1, -1, 1];

#[test]
fn factorization_basis_derives_path_affine_lift_and_bounded_internal_step() {
    let limits = ProbeLimits::default();
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();
    let rule = factorization_for_sector(authority.factorization_rules(), &PATH_SECTOR);
    let routed = best_routed_basis(family, rule);
    assert_eq!(routed.unit_image_count, ARITY - 1);
    for (form, relation) in routed.transformed_forms.iter().zip(&routed.relations) {
        replay_relation(family, form, relation);
    }

    let selected = routed
        .unit_images
        .iter()
        .position(Option::is_none)
        .expect("a path tree has one long-chord affine row");
    assert_eq!(selected, 0);
    assert_eq!(
        routed.relations[selected].constant,
        family.coefficient_context().one()
    );
    assert_eq!(
        routed.relations[selected].denominator_coefficients,
        [1, 1, -1, 1, -1, 1]
            .map(|coefficient| family.coefficient_context().integer(coefficient))
            .into()
    );
    // Thus D1 = 1 + D1' + D2' - D3' + D4' - D5' + D6'. A single
    // admitted step has seven branches independently of remaining rank.
    assert!(PATH_TRIPLE_NUMERATOR[selected] < 0);
    let priority = CoordinatePriority::try_new(
        ARITY,
        &[0, 1, 2, 3, 4, 5],
        CoordinatePriorityLimits::default(),
    )
    .unwrap();
    assert_eq!(priority.rank_by_slot()[selected], selected);

    let state = AffinePowerState {
        remaining_power: 37,
        routed_powers: routed_base(&PATH_TRIPLE_NUMERATOR, selected, &routed),
    };
    let children = affine_power_step(
        family.coefficient_context(),
        &state,
        &routed.relations[selected],
        limits,
    )
    .unwrap();
    assert!(!children.is_empty());
    assert_eq!(children.len(), ARITY + 1);
    assert!(
        children
            .iter()
            .all(|child| child.state.remaining_power == 36)
    );
    assert_eq!(
        children
            .iter()
            .filter(|child| child.state.routed_powers == state.routed_powers)
            .count(),
        usize::from(!routed.relations[selected].constant.is_zero())
    );
}

#[test]
fn factorized_corner_recurrence_replays_path_and_star_triple_numerators() {
    let limits = ProbeLimits::default();
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();

    let path_rule = factorization_for_sector(authority.factorization_rules(), &PATH_SECTOR);
    let path = best_routed_basis(family, path_rule);
    let mut path_evaluator = CornerMomentEvaluator::try_new(
        family,
        path_rule,
        &path.transformed_forms,
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
    let star = best_routed_basis(family, star_rule);
    // A star tree is already the canonical K4 edge basis: no omitted affine
    // row exists, so the path rewrite alone cannot own this branch.
    assert_eq!(star.unit_image_count, ARITY);
    let mut star_evaluator = CornerMomentEvaluator::try_new(
        family,
        star_rule,
        &star.transformed_forms,
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
    // This census is evidence only: production still has to persist and
    // replay every exceptional guard d+r-2 != 0.
    assert!(!star_evaluator.angular_guard_ranks.is_empty());
}

#[test]
fn bounded_fixture_rejects_underflow_overflow_and_excess_work() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();
    let rule = factorization_for_sector(authority.factorization_rules(), &PATH_SECTOR);
    let routed = best_routed_basis(family, rule);
    let selected = routed.unit_images.iter().position(Option::is_none).unwrap();
    let relation = &routed.relations[selected];

    let empty = AffinePowerState {
        remaining_power: 0,
        routed_powers: [0; ARITY],
    };
    assert!(matches!(
        affine_power_step(
            family.coefficient_context(),
            &empty,
            relation,
            ProbeLimits::default(),
        ),
        Err(ProbeError::EmptyAffinePower)
    ));

    let lowered_slot = relation
        .denominator_coefficients
        .iter()
        .position(|coefficient| !coefficient.is_zero())
        .unwrap();
    let mut one_below = AffinePowerState {
        remaining_power: 1,
        routed_powers: [0; ARITY],
    };
    one_below.routed_powers[lowered_slot] = i64::MIN;
    assert!(matches!(
        affine_power_step(
            family.coefficient_context(),
            &one_below,
            relation,
            ProbeLimits::default(),
        ),
        Err(ProbeError::RoutedPowerUnderflow {
            slot,
            power: i64::MIN,
        }) if slot == lowered_slot
    ));

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
        numerator_powers([0, 0, 0, 0, 1, 1], rule, ProbeLimits::default()),
        Err(ProbeError::NonCornerActivePower { slot: 2, power: 0 })
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

    let transition_limits = ProbeLimits {
        max_affine_transitions: ARITY,
        ..ProbeLimits::default()
    };
    let seven_branch_state = AffinePowerState {
        remaining_power: 1,
        routed_powers: [0; ARITY],
    };
    assert!(matches!(
        affine_power_step(
            family.coefficient_context(),
            &seven_branch_state,
            relation,
            transition_limits,
        ),
        Err(ProbeError::CountLimit {
            resource: "one-step affine transitions",
            requested: 7,
            limit: 6,
        })
    ));

    let composite_rule =
        factorization_for_sector(authority.factorization_rules(), &K3_TIMES_K1_SECTOR);
    let composite = best_routed_basis(family, composite_rule);
    assert!(matches!(
        CornerMomentEvaluator::try_new(
            family,
            composite_rule,
            &composite.transformed_forms,
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
        &routed.transformed_forms,
        &[0, 1, 2, 3, 4, 5],
        cache_limits,
    )
    .unwrap();
    // Exercise the cache boundary directly after the K1^3 constructor has
    // authenticated the only corner shape admitted by this fixture.
    assert!(matches!(
        evaluator.evaluate([0; ARITY]),
        Err(ProbeError::CountLimit {
            resource: "affine recurrence cache entries",
            requested: 1,
            limit: 0,
        })
    ));
}
