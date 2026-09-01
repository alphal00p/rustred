use std::collections::BTreeMap;

use crate::algebra::{
    Coefficient, ExactAlgebraLimits, coefficient_clone_owned_retained_byte_bound,
};
use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::artifact::{
    FactorizationRule, UnimodularLoopBasis, derive_k6_terminal_authority,
    derive_two_loop_unit_mass_sunset,
};

use super::{
    FactorizedNumeratorLiftCompilation, FactorizedNumeratorLiftError,
    FactorizedNumeratorLiftExpansionLimits, FactorizedNumeratorLiftLimits,
    FactorizedNumeratorLiftStart, compile_factorized_numerator_lift,
};

const PATH_SECTOR: [i64; 6] = [0, 0, 1, 0, 1, 1];
const STAR_SECTOR: [i64; 6] = [0, 0, 1, 1, 0, 1];
const K3_TIMES_K1_SECTOR: [i64; 6] = [0, 0, 1, 1, 1, 1];
// These are the two independent frontier representatives observed by the
// compact K6 campaign.  B is deliberately absent from compilation itself.
const PATH_A: [i64; 6] = [-2, -6, 1, -2, 3, 3];
const HELD_OUT_PATH_B: [i64; 6] = [-4, -6, 7, 0, 3, 3];

fn factorization_for_sector<'a>(
    rules: &'a [FactorizationRule],
    sector: &[i64],
) -> &'a FactorizationRule {
    rules
        .iter()
        .find(|rule| {
            rule.application_domain()
                .sector()
                .active_bits()
                .iter()
                .zip(sector)
                .all(|(&active, &power)| active == (power >= 1))
        })
        .unwrap()
}

fn path_action(
    family: &IntegralFamily,
    rule: &FactorizationRule,
) -> super::FactorizedNumeratorLiftAction {
    match compile_factorized_numerator_lift(family, rule, FactorizedNumeratorLiftLimits::default())
        .unwrap()
    {
        FactorizedNumeratorLiftCompilation::Action(action) => action,
        other => panic!("path factorization did not compile an action: {other:?}"),
    }
}

fn replay_rank_two_recurrence(
    family: &IntegralFamily,
    action: &super::FactorizedNumeratorLiftAction,
    source: &IntegralKey,
) -> BTreeMap<IntegralKey, Coefficient> {
    let FactorizedNumeratorLiftStart::Auxiliary(initial) = action.try_start(source).unwrap() else {
        panic!("rank-two source must start a recurrence")
    };
    assert_eq!(initial.measure().remaining_power(), 2);
    let context = family.coefficient_context();
    let mut replay = BTreeMap::<IntegralKey, Coefficient>::new();
    for first in action.try_step(&initial).unwrap() {
        for second in action.try_step(first.state()).unwrap() {
            let key = second
                .state()
                .try_integral_key()
                .unwrap()
                .expect("the second rank-two step must emit an ordinary key");
            let coefficient = context
                .try_mul(
                    first.coefficient(),
                    second.coefficient(),
                    ExactAlgebraLimits::default(),
                )
                .unwrap();
            match replay.entry(key) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(coefficient);
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    let sum = context
                        .try_add(entry.get(), &coefficient, ExactAlgebraLimits::default())
                        .unwrap();
                    if sum.is_zero() {
                        entry.remove();
                    } else {
                        *entry.get_mut() = sum;
                    }
                }
            }
        }
    }
    replay
}

fn replace_with_width_one_relation(
    mut action: super::FactorizedNumeratorLiftAction,
    family: &IntegralFamily,
    constant: Coefficient,
    denominator: Option<(usize, Coefficient)>,
) -> super::FactorizedNumeratorLiftAction {
    let mut denominator_coefficients = Vec::new();
    denominator_coefficients
        .try_reserve_exact(family.denominator_count())
        .unwrap();
    denominator_coefficients
        .extend((0..family.denominator_count()).map(|_| family.coefficient_context().zero()));
    if let Some((position, coefficient)) = denominator {
        denominator_coefficients[position] = coefficient;
    }
    let affine_source = action.affine_source;
    action.routing.relations[affine_source] = super::model::CanonicalDenominatorRelation {
        constant,
        denominator_coefficients: denominator_coefficients.into_boxed_slice(),
    };
    action.branch_width = 1;
    action
}

#[test]
fn generic_path_action_admits_frontier_a_and_held_out_b_with_exact_width_seven_relation() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();
    let rule = factorization_for_sector(authority.factorization_rules(), &PATH_SECTOR);
    let action = path_action(family, rule);
    let context = family.coefficient_context();

    assert_eq!(action.affine_source(), 0);
    assert_eq!(action.branch_width(), 7);
    assert!(
        action.routing().loop_basis_determinant() == &context.one()
            || action.routing().loop_basis_determinant() == &context.integer(-1)
    );
    assert_eq!(action.affine_relation().constant(), &context.one());
    assert_eq!(
        action.affine_relation().denominator_coefficients(),
        [1, 1, -1, 1, -1, 1].map(|coefficient| context.integer(coefficient))
    );
    assert_eq!(
        action.routing().unit_images(),
        [None, Some(3), Some(0), Some(5), Some(1), Some(2)]
    );

    // The action owns the complete sector, not the old undotted-corner cell.
    for (position, (&active, bounds)) in action
        .application_domain()
        .sector()
        .active_bits()
        .iter()
        .zip(action.application_domain().bounds())
        .enumerate()
    {
        if active {
            assert_eq!((bounds.lower(), bounds.upper()), (1, i64::MAX));
        } else {
            assert_eq!((bounds.lower(), bounds.upper()), (i64::MIN, 0));
        }
        assert_eq!(active, PATH_SECTOR[position] >= 1);
    }

    for (target, remaining, routed_base) in [
        (PATH_A, 2_u64, [1, 3, 3, -6, 0, -2]),
        (HELD_OUT_PATH_B, 4_u64, [7, 3, 3, -6, 0, 0]),
    ] {
        let state = action
            .try_start(&IntegralKey::try_new(target).unwrap())
            .unwrap();
        let FactorizedNumeratorLiftStart::Auxiliary(state) = state else {
            panic!("a negative affine source must start an auxiliary lift")
        };
        assert_eq!(state.measure().remaining_power(), remaining);
        assert_eq!(state.routed_powers(), routed_base);
        let children = action.try_step(&state).unwrap();
        assert_eq!(children.len(), 7);
        assert!(
            children
                .iter()
                .all(|child| child.state().measure() < state.measure())
        );
        assert_eq!(
            children
                .iter()
                .map(|child| child.coefficient().clone())
                .collect::<Vec<_>>(),
            [1, 1, 1, -1, 1, -1, 1].map(|coefficient| context.integer(coefficient))
        );
    }
}

#[test]
fn fully_unit_routed_star_returns_an_explicit_no_lift_disposition() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();
    let rule = factorization_for_sector(authority.factorization_rules(), &STAR_SECTOR);
    let compiled =
        compile_factorized_numerator_lift(family, rule, FactorizedNumeratorLiftLimits::default())
            .unwrap();
    let FactorizedNumeratorLiftCompilation::NoAffineLiftRequired(routing) = compiled else {
        panic!("star routing needs no affine lift: {compiled:?}");
    };
    assert_eq!(routing.unit_images().iter().flatten().count(), 6);
    assert_eq!(
        routing.unit_images(),
        [Some(4), Some(3), Some(0), Some(1), Some(5), Some(2)]
    );
    assert_eq!(routing.relations().len(), 6);
    assert_eq!(routing.transformed_denominators().len(), 6);
    assert_eq!(routing.family_fingerprint(), family.fingerprint());
    let target = IntegralKey::try_new([-2, -3, 1, 4, 0, 2]).unwrap();
    let routed = routing.try_route_key(&target).unwrap();
    let mut expected = [0_i64; 6];
    for (source, image) in routing.unit_images().iter().enumerate() {
        expected[image.unwrap()] = target.powers()[source];
    }
    assert_eq!(routed.powers(), expected);
    assert_eq!(routed.powers(), [1, 4, 2, -3, -2, 0]);
}

#[test]
fn compilation_and_auxiliary_states_retain_typed_resource_and_owner_boundaries() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();
    let rule = factorization_for_sector(authority.factorization_rules(), &PATH_SECTOR);
    let unauthenticated = FactorizationRule::new(
        rule.application_domain().clone(),
        [],
        family.coefficient_context().one(),
        UnimodularLoopBasis::new(
            rule.loop_basis().dimension(),
            rule.loop_basis().row_major().iter().copied(),
        ),
    );
    assert_eq!(
        compile_factorized_numerator_lift(
            family,
            &unauthenticated,
            FactorizedNumeratorLiftLimits::default(),
        )
        .unwrap_err(),
        FactorizedNumeratorLiftError::UnauthenticatedFactorizationRule
    );
    let foreign = derive_two_loop_unit_mass_sunset().unwrap();
    assert_eq!(
        compile_factorized_numerator_lift(
            foreign.family(),
            rule,
            FactorizedNumeratorLiftLimits::default(),
        )
        .unwrap_err(),
        FactorizedNumeratorLiftError::WrongFactorizationFamily
    );
    let gauge_limited = FactorizedNumeratorLiftLimits {
        max_sign_gauges: 7,
        ..FactorizedNumeratorLiftLimits::default()
    };
    assert_eq!(
        compile_factorized_numerator_lift(family, rule, gauge_limited).unwrap_err(),
        FactorizedNumeratorLiftError::ResourceLimit {
            resource: "factorized numerator row-sign gauges",
            requested: 8,
            limit: 7,
        }
    );
    let branch_limited = FactorizedNumeratorLiftLimits {
        max_recurrence_branches: 6,
        ..FactorizedNumeratorLiftLimits::default()
    };
    assert_eq!(
        compile_factorized_numerator_lift(family, rule, branch_limited).unwrap_err(),
        FactorizedNumeratorLiftError::ResourceLimit {
            resource: "factorized numerator recurrence branches",
            requested: 7,
            limit: 6,
        }
    );

    let first = path_action(family, rule);
    let second = path_action(family, rule);
    let state = first
        .try_start(&IntegralKey::try_new(PATH_A).unwrap())
        .unwrap();
    let FactorizedNumeratorLiftStart::Auxiliary(state) = state else {
        panic!("path A must start an auxiliary lift")
    };
    assert_eq!(
        second.try_step(&state).unwrap_err(),
        FactorizedNumeratorLiftError::ForeignAuxiliaryState
    );
    assert_eq!(
        first
            .try_start(&IntegralKey::try_new([1, 0, 1, 0, 1, 1]).unwrap())
            .unwrap_err(),
        FactorizedNumeratorLiftError::OutsideApplicationDomain {
            position: 0,
            power: 1,
            active: false,
        }
    );
}

#[test]
fn rank_one_action_terminates_in_typed_keys_and_rejects_empty_underflow_and_wrong_arity() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();
    let rule = factorization_for_sector(authority.factorization_rules(), &PATH_SECTOR);
    let action = path_action(family, rule);
    let zero_boundary = action
        .try_start(&IntegralKey::try_new(PATH_SECTOR).unwrap())
        .unwrap();
    let FactorizedNumeratorLiftStart::Routed(zero_boundary) = zero_boundary else {
        panic!("zero affine power must return an exact pure-routed key")
    };
    assert_eq!(zero_boundary.powers(), [1, 1, 1, 0, 0, 0]);
    assert_eq!(
        zero_boundary,
        action
            .routing()
            .try_route_key(&IntegralKey::try_new(PATH_SECTOR).unwrap())
            .unwrap()
    );

    let mut rank_one = PATH_SECTOR;
    rank_one[action.affine_source()] = -1;
    let state = action
        .try_start(&IntegralKey::try_new(rank_one).unwrap())
        .unwrap();
    let FactorizedNumeratorLiftStart::Auxiliary(state) = state else {
        panic!("rank-one affine power must start an auxiliary lift")
    };
    assert_eq!(state.measure().remaining_power(), 1);
    let children = action.try_step(&state).unwrap();
    assert_eq!(children.len(), action.branch_width());
    let expected_children = [
        (1, [1, 1, 1, 0, 0, 0]),
        (1, [0, 1, 1, 0, 0, 0]),
        (1, [1, 0, 1, 0, 0, 0]),
        (-1, [1, 1, 0, 0, 0, 0]),
        (1, [1, 1, 1, -1, 0, 0]),
        (-1, [1, 1, 1, 0, -1, 0]),
        (1, [1, 1, 1, 0, 0, -1]),
    ];
    for (child, (coefficient, powers)) in children.iter().zip(expected_children) {
        assert_eq!(child.state().measure().remaining_power(), 0);
        let key = child
            .state()
            .try_integral_key()
            .unwrap()
            .expect("an exhausted auxiliary state is an ordinary integral key");
        assert_eq!(
            child.coefficient(),
            &family.coefficient_context().integer(coefficient)
        );
        assert_eq!(key.powers(), powers);
    }
    assert_eq!(
        action.try_step(children[0].state()).unwrap_err(),
        FactorizedNumeratorLiftError::EmptyAuxiliaryState
    );

    let relation = action.affine_relation();
    let (underflow_source, underflow_image) = action
        .routing()
        .unit_images()
        .iter()
        .enumerate()
        .filter_map(|(source, image)| image.map(|image| (source, image)))
        .find(|&(source, image)| {
            !action.application_domain().sector().active_bits()[source]
                && !relation.denominator_coefficients()[image].is_zero()
        })
        .expect("the path routing has an inactive unit image in the affine relation");
    let mut underflow_target = rank_one;
    underflow_target[underflow_source] = i64::MIN;
    let underflow_state = action
        .try_start(&IntegralKey::try_new(underflow_target).unwrap())
        .unwrap();
    let FactorizedNumeratorLiftStart::Auxiliary(underflow_state) = underflow_state else {
        panic!("negative affine power must start an auxiliary lift")
    };
    assert_eq!(
        action.try_step(&underflow_state).unwrap_err(),
        FactorizedNumeratorLiftError::RoutedPowerUnderflow {
            position: underflow_image,
            power: i64::MIN,
        }
    );

    assert_eq!(
        action
            .try_start(&IntegralKey::try_new([1, 1, 1, 1, 1]).unwrap())
            .unwrap_err(),
        FactorizedNumeratorLiftError::WrongTargetArity {
            expected: 6,
            actual: 5,
        }
    );
}

#[test]
fn authenticated_k6_factorization_routing_census_is_explicit() {
    let authority = derive_k6_terminal_authority().unwrap();
    let mut seen = [false; 3];
    for rule in authority.factorization_rules() {
        let compiled = compile_factorized_numerator_lift(
            authority.family(),
            rule,
            FactorizedNumeratorLiftLimits::default(),
        )
        .unwrap();
        let routing = compiled.routing();
        let missing = routing
            .unit_images()
            .iter()
            .enumerate()
            .filter_map(|(source, image)| image.is_none().then_some(source))
            .collect::<Vec<_>>();
        let missing_active = missing
            .iter()
            .copied()
            .filter(|&source| routing.application_domain().sector().active_bits()[source])
            .collect::<Vec<_>>();
        let active = routing.application_domain().sector().active_bits();
        if active == K3_TIMES_K1_SECTOR.map(|power| power >= 1) {
            seen[0] = true;
            assert_eq!(routing.unit_images().iter().flatten().count(), 5);
            assert_eq!(missing, [1]);
            assert!(missing_active.is_empty());
            assert!(matches!(
                compiled,
                FactorizedNumeratorLiftCompilation::Action(ref action)
                    if action.affine_source() == 1
            ));
        } else if active == STAR_SECTOR.map(|power| power >= 1) {
            seen[1] = true;
            assert_eq!(routing.unit_images().iter().flatten().count(), 6);
            assert!(missing.is_empty());
            assert!(missing_active.is_empty());
            assert!(matches!(
                compiled,
                FactorizedNumeratorLiftCompilation::NoAffineLiftRequired(_)
            ));
        } else if active == PATH_SECTOR.map(|power| power >= 1) {
            seen[2] = true;
            assert_eq!(routing.unit_images().iter().flatten().count(), 5);
            assert_eq!(missing, [0]);
            assert!(missing_active.is_empty());
            assert!(matches!(
                compiled,
                FactorizedNumeratorLiftCompilation::Action(ref action)
                    if action.affine_source() == 0
            ));
        } else {
            panic!("unexpected authenticated K6 factorization sector: {active:?}");
        }
    }
    assert_eq!(seen, [true; 3]);
}

#[test]
fn symbolica_endpoint_expansion_pins_both_path_frontiers_without_canonicalization() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();
    let rule = factorization_for_sector(authority.factorization_rules(), &PATH_SECTOR);
    let action = path_action(family, rule);

    for (source, expected_support) in [(PATH_A, 28), (HELD_OUT_PATH_B, 210)] {
        let expansion = action
            .try_expand_endpoints(
                family,
                &IntegralKey::try_new(source).unwrap(),
                FactorizedNumeratorLiftExpansionLimits::default(),
            )
            .unwrap();
        assert_eq!(expansion.family_fingerprint(), family.fingerprint());
        assert_eq!(expansion.source().powers(), source);
        assert!(expansion.belongs_to_action(&action));
        assert_eq!(expansion.endpoints().len(), expected_support);
        assert!(
            expansion
                .endpoints()
                .windows(2)
                .all(|window| window[0].key() < window[1].key())
        );
        assert!(
            expansion
                .endpoints()
                .iter()
                .all(|endpoint| !endpoint.coefficient().is_zero())
        );
    }
}

#[test]
fn symbolica_power_reconstructs_the_exact_auxiliary_recurrence() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();
    let rule = factorization_for_sector(authority.factorization_rules(), &PATH_SECTOR);
    let action = path_action(family, rule);
    let source = IntegralKey::try_new(PATH_A).unwrap();
    let expanded = action
        .try_expand_endpoints(
            family,
            &source,
            FactorizedNumeratorLiftExpansionLimits::default(),
        )
        .unwrap();

    let replay = replay_rank_two_recurrence(family, &action, &source);
    let materialized = expanded
        .endpoints()
        .iter()
        .map(|endpoint| (endpoint.key().clone(), endpoint.coefficient().clone()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(materialized, replay);
}

#[test]
fn k3_times_k1_symbolica_expansion_reconstructs_the_exact_recurrence() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();
    let rule = factorization_for_sector(authority.factorization_rules(), &K3_TIMES_K1_SECTOR);
    let action = path_action(family, rule);
    let mut powers = K3_TIMES_K1_SECTOR;
    powers[action.affine_source()] = -2;
    let source = IntegralKey::try_new(powers).unwrap();
    let expanded = action
        .try_expand_endpoints(
            family,
            &source,
            FactorizedNumeratorLiftExpansionLimits::default(),
        )
        .unwrap();
    let replay = replay_rank_two_recurrence(family, &action, &source);
    let materialized = expanded
        .endpoints()
        .iter()
        .map(|endpoint| (endpoint.key().clone(), endpoint.coefficient().clone()))
        .collect::<BTreeMap<_, _>>();

    assert!(action.branch_width() > 1);
    assert_eq!(materialized, replay);
}

#[test]
fn endpoint_order_is_deterministic_across_independent_action_compilations() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();
    let rule = factorization_for_sector(authority.factorization_rules(), &PATH_SECTOR);
    let first = path_action(family, rule);
    let second = path_action(family, rule);
    let source = IntegralKey::try_new(HELD_OUT_PATH_B).unwrap();
    let limits = FactorizedNumeratorLiftExpansionLimits::default();
    let left = first.try_expand_endpoints(family, &source, limits).unwrap();
    let right = second
        .try_expand_endpoints(family, &source, limits)
        .unwrap();
    assert_eq!(left.source(), right.source());
    assert_eq!(left.endpoints(), right.endpoints());
    assert_ne!(left, right);
    assert!(left.belongs_to_action(&first));
    assert!(!left.belongs_to_action(&second));
}

#[test]
fn zero_boundary_and_pure_star_routing_are_explicit_one_endpoint_identities() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();
    let path_rule = factorization_for_sector(authority.factorization_rules(), &PATH_SECTOR);
    let action = path_action(family, path_rule);
    let zero_source = IntegralKey::try_new(PATH_SECTOR).unwrap();
    let zero = action
        .try_expand_endpoints(
            family,
            &zero_source,
            FactorizedNumeratorLiftExpansionLimits::default(),
        )
        .unwrap();
    assert_eq!(zero.endpoints().len(), 1);
    assert_eq!(zero.endpoints()[0].key().powers(), [1, 1, 1, 0, 0, 0]);
    assert_eq!(
        zero.endpoints()[0].coefficient(),
        &family.coefficient_context().one()
    );

    let star_rule = factorization_for_sector(authority.factorization_rules(), &STAR_SECTOR);
    let FactorizedNumeratorLiftCompilation::NoAffineLiftRequired(star) =
        compile_factorized_numerator_lift(
            family,
            star_rule,
            FactorizedNumeratorLiftLimits::default(),
        )
        .unwrap()
    else {
        panic!("star must be a pure routing")
    };
    let star_source = IntegralKey::try_new([-2, -3, 1, 4, 0, 2]).unwrap();
    let routed = star
        .try_expand_pure_routing(
            family,
            &star_source,
            FactorizedNumeratorLiftExpansionLimits::default(),
        )
        .unwrap();
    assert_eq!(routed.source(), &star_source);
    assert!(routed.belongs_to_routing(&star));
    assert_eq!(routed.endpoints().len(), 1);
    assert_eq!(routed.endpoints()[0].key().powers(), [1, 4, 2, -3, -2, 0]);
    assert_eq!(
        routed.endpoints()[0].coefficient(),
        &family.coefficient_context().one()
    );

    let FactorizedNumeratorLiftCompilation::NoAffineLiftRequired(second_star) =
        compile_factorized_numerator_lift(
            family,
            star_rule,
            FactorizedNumeratorLiftLimits::default(),
        )
        .unwrap()
    else {
        panic!("star must remain a pure routing")
    };
    let independently_compiled = second_star
        .try_expand_pure_routing(
            family,
            &star_source,
            FactorizedNumeratorLiftExpansionLimits::default(),
        )
        .unwrap();
    assert_eq!(routed.endpoints(), independently_compiled.endpoints());
    assert_ne!(routed, independently_compiled);
    assert!(!routed.belongs_to_routing(&second_star));
}

#[test]
fn expansion_preflights_support_work_exponents_and_routed_i64_edges() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();
    let rule = factorization_for_sector(authority.factorization_rules(), &PATH_SECTOR);
    let action = path_action(family, rule);
    let source = IntegralKey::try_new(PATH_A).unwrap();
    let retained_key_bytes = 28
        * (std::mem::size_of::<IntegralKey>()
            + family.denominator_count() * std::mem::size_of::<i64>());

    for (limits, resource, requested, limit) in [
        (
            FactorizedNumeratorLiftExpansionLimits {
                max_endpoints: 27,
                ..FactorizedNumeratorLiftExpansionLimits::default()
            },
            "factorized numerator endpoints",
            28,
            27,
        ),
        (
            FactorizedNumeratorLiftExpansionLimits {
                max_endpoint_power_entries: 167,
                ..FactorizedNumeratorLiftExpansionLimits::default()
            },
            "factorized numerator endpoint power entries",
            168,
            167,
        ),
        (
            FactorizedNumeratorLiftExpansionLimits {
                max_retained_endpoint_key_bytes: retained_key_bytes - 1,
                ..FactorizedNumeratorLiftExpansionLimits::default()
            },
            "factorized numerator retained endpoint key bytes",
            retained_key_bytes,
            retained_key_bytes - 1,
        ),
        (
            FactorizedNumeratorLiftExpansionLimits {
                max_exponent_entries: 167,
                ..FactorizedNumeratorLiftExpansionLimits::default()
            },
            "factorized numerator exponent entries",
            168,
            167,
        ),
        (
            FactorizedNumeratorLiftExpansionLimits {
                max_structural_term_operations: 391,
                ..FactorizedNumeratorLiftExpansionLimits::default()
            },
            "factorized numerator structural term operations",
            392,
            391,
        ),
    ] {
        assert_eq!(
            action.try_expand_endpoints(family, &source, limits),
            Err(FactorizedNumeratorLiftError::ResourceLimit {
                resource,
                requested,
                limit,
            })
        );
    }

    let relation = action.affine_relation();
    let (underflow_source, underflow_image) = action
        .routing()
        .unit_images()
        .iter()
        .enumerate()
        .filter_map(|(source, image)| image.map(|image| (source, image)))
        .find(|&(source, image)| {
            !action.application_domain().sector().active_bits()[source]
                && !relation.denominator_coefficients()[image].is_zero()
        })
        .unwrap();
    let mut underflow = PATH_SECTOR;
    underflow[action.affine_source()] = -1;
    underflow[underflow_source] = i64::MIN;
    assert_eq!(
        action.try_expand_endpoints(
            family,
            &IntegralKey::try_new(underflow).unwrap(),
            FactorizedNumeratorLiftExpansionLimits::default(),
        ),
        Err(FactorizedNumeratorLiftError::RoutedPowerShiftUnderflow {
            position: underflow_image,
            power: i64::MIN,
            decrement: 1,
        })
    );

    let mut maximal_rank = PATH_SECTOR;
    maximal_rank[action.affine_source()] = i64::MIN;
    assert!(matches!(
        action.try_expand_endpoints(
            family,
            &IntegralKey::try_new(maximal_rank).unwrap(),
            FactorizedNumeratorLiftExpansionLimits::default(),
        ),
        Err(FactorizedNumeratorLiftError::ResourceLimit {
            resource: "factorized numerator endpoints",
            ..
        })
    ));
}

#[test]
fn retained_coefficient_limits_cover_native_and_canonical_live_clone_peaks() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();
    let rule = factorization_for_sector(authority.factorization_rules(), &PATH_SECTOR);
    let action = path_action(family, rule);
    let source = IntegralKey::try_new(PATH_A).unwrap();
    let raw = action
        .try_expand_endpoints(
            family,
            &source,
            FactorizedNumeratorLiftExpansionLimits::default(),
        )
        .unwrap();
    let context = family.coefficient_context();
    assert!(raw.endpoints().iter().any(|endpoint| {
        endpoint.coefficient() != &context.one() && endpoint.coefficient() != &context.integer(-1)
    }));

    let returned_terms = raw
        .endpoints()
        .iter()
        .map(|endpoint| {
            endpoint.coefficient().numerator.nterms() + endpoint.coefficient().denominator.nterms()
        })
        .sum::<usize>();
    let native_plus_output_terms = returned_terms * 2;
    assert_eq!(
        action.try_expand_endpoints(
            family,
            &source,
            FactorizedNumeratorLiftExpansionLimits {
                max_retained_endpoint_coefficient_terms: native_plus_output_terms - 1,
                ..FactorizedNumeratorLiftExpansionLimits::default()
            },
        ),
        Err(FactorizedNumeratorLiftError::ResourceLimit {
            resource: "factorized numerator retained endpoint coefficient terms",
            requested: native_plus_output_terms,
            limit: native_plus_output_terms - 1,
        })
    );

    // Clone-owned byte bounds depend on native Vec capacities. Discover the
    // exact admitted peak through typed failures, then pin its one-below edge.
    let mut native_plus_output_bytes = 0_usize;
    loop {
        match action.try_expand_endpoints(
            family,
            &source,
            FactorizedNumeratorLiftExpansionLimits {
                max_retained_endpoint_coefficient_clone_owned_bytes: native_plus_output_bytes,
                ..FactorizedNumeratorLiftExpansionLimits::default()
            },
        ) {
            Ok(_) => break,
            Err(FactorizedNumeratorLiftError::ResourceLimit {
                resource: "factorized numerator retained endpoint coefficient clone-owned bytes",
                requested,
                limit,
            }) if limit == native_plus_output_bytes && requested > native_plus_output_bytes => {
                native_plus_output_bytes = requested;
            }
            other => panic!("unexpected coefficient-byte boundary result: {other:?}"),
        }
    }
    assert!(native_plus_output_bytes > 0);
    assert_eq!(
        action.try_expand_endpoints(
            family,
            &source,
            FactorizedNumeratorLiftExpansionLimits {
                max_retained_endpoint_coefficient_clone_owned_bytes: native_plus_output_bytes - 1,
                ..FactorizedNumeratorLiftExpansionLimits::default()
            },
        ),
        Err(FactorizedNumeratorLiftError::ResourceLimit {
            resource: "factorized numerator retained endpoint coefficient clone-owned bytes",
            requested: native_plus_output_bytes,
            limit: native_plus_output_bytes - 1,
        })
    );

    let input_terms = returned_terms;
    assert!(matches!(
        raw.try_canonicalize_endpoints(
            family,
            authority.canonicalizer().unwrap(),
            FactorizedNumeratorLiftExpansionLimits {
                max_retained_endpoint_coefficient_terms: input_terms,
                ..FactorizedNumeratorLiftExpansionLimits::default()
            },
        ),
        Err(FactorizedNumeratorLiftError::ResourceLimit {
            resource: "factorized numerator retained endpoint coefficient terms",
            requested,
            limit,
        }) if limit == input_terms && requested > input_terms
    ));
    let input_bytes = raw
        .endpoints()
        .iter()
        .try_fold(0_usize, |total, endpoint| {
            total.checked_add(
                coefficient_clone_owned_retained_byte_bound(endpoint.coefficient()).unwrap(),
            )
        })
        .unwrap();
    assert!(matches!(
        raw.try_canonicalize_endpoints(
            family,
            authority.canonicalizer().unwrap(),
            FactorizedNumeratorLiftExpansionLimits {
                max_retained_endpoint_coefficient_clone_owned_bytes: input_bytes,
                ..FactorizedNumeratorLiftExpansionLimits::default()
            },
        ),
        Err(FactorizedNumeratorLiftError::ResourceLimit {
            resource: "factorized numerator retained endpoint coefficient clone-owned bytes",
            requested,
            limit,
        }) if limit == input_bytes && requested > input_bytes
    ));
}

#[test]
fn width_one_lane_is_rank_generic_but_keeps_exact_work_and_shift_limits() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();
    let rule = factorization_for_sector(authority.factorization_rules(), &PATH_SECTOR);
    let context = family.coefficient_context();

    let plus =
        replace_with_width_one_relation(path_action(family, rule), family, context.one(), None);
    let mut plus_powers = PATH_SECTOR;
    plus_powers[plus.affine_source()] = i64::MIN;
    let unit_limits = FactorizedNumeratorLiftExpansionLimits {
        max_direct_coefficient_power: 0,
        ..FactorizedNumeratorLiftExpansionLimits::default()
    };
    let plus_expansion = plus
        .try_expand_endpoints(
            family,
            &IntegralKey::try_new(plus_powers).unwrap(),
            unit_limits,
        )
        .unwrap();
    assert_eq!(plus_expansion.endpoints().len(), 1);
    assert_eq!(plus_expansion.endpoints()[0].coefficient(), &context.one());

    let minus = replace_with_width_one_relation(
        path_action(family, rule),
        family,
        context.integer(-1),
        None,
    );
    let mut minus_powers = PATH_SECTOR;
    minus_powers[minus.affine_source()] = -i64::MAX;
    let minus_expansion = minus
        .try_expand_endpoints(
            family,
            &IntegralKey::try_new(minus_powers).unwrap(),
            unit_limits,
        )
        .unwrap();
    assert_eq!(minus_expansion.endpoints().len(), 1);
    assert_eq!(
        minus_expansion.endpoints()[0].coefficient(),
        &context.integer(-1)
    );

    let nonunit = replace_with_width_one_relation(
        path_action(family, rule),
        family,
        context.integer(2),
        None,
    );
    let mut nonunit_powers = PATH_SECTOR;
    nonunit_powers[nonunit.affine_source()] = -3;
    assert_eq!(
        nonunit.try_expand_endpoints(
            family,
            &IntegralKey::try_new(nonunit_powers).unwrap(),
            FactorizedNumeratorLiftExpansionLimits {
                max_direct_coefficient_power: 2,
                ..FactorizedNumeratorLiftExpansionLimits::default()
            },
        ),
        Err(FactorizedNumeratorLiftError::ResourceLimit {
            resource: "factorized numerator direct coefficient power",
            requested: 3,
            limit: 2,
        })
    );

    let nonconstant = replace_with_width_one_relation(
        path_action(family, rule),
        family,
        family.dimension().clone(),
        None,
    );
    let mut nonconstant_powers = PATH_SECTOR;
    nonconstant_powers[nonconstant.affine_source()] = -2;
    assert_eq!(
        nonconstant.try_expand_endpoints(
            family,
            &IntegralKey::try_new(nonconstant_powers).unwrap(),
            FactorizedNumeratorLiftExpansionLimits::default(),
        ),
        Err(FactorizedNumeratorLiftError::NonconstantExpansionCoefficient)
    );

    let native_boundary = replace_with_width_one_relation(
        path_action(family, rule),
        family,
        context.integer(2),
        None,
    );
    let beyond_u32 = u64::from(u32::MAX) + 1;
    let mut native_boundary_powers = PATH_SECTOR;
    native_boundary_powers[native_boundary.affine_source()] = -i64::try_from(beyond_u32).unwrap();
    assert_eq!(
        native_boundary.try_expand_endpoints(
            family,
            &IntegralKey::try_new(native_boundary_powers).unwrap(),
            FactorizedNumeratorLiftExpansionLimits {
                max_direct_coefficient_power: usize::MAX,
                ..FactorizedNumeratorLiftExpansionLimits::default()
            },
        ),
        Err(
            FactorizedNumeratorLiftError::NativeDirectCoefficientPowerExponentLimit {
                requested: beyond_u32,
                limit: u32::MAX,
            }
        )
    );

    let base = path_action(family, rule);
    let (source_position, routed_position) = base
        .routing()
        .unit_images()
        .iter()
        .enumerate()
        .find_map(|(source, image)| {
            (!base.application_domain().sector().active_bits()[source])
                .then_some(image.map(|image| (source, image)))
                .flatten()
        })
        .expect("the path chart has another inactive unit image");
    let affine_source = base.affine_source();
    let shifted = replace_with_width_one_relation(
        base,
        family,
        context.zero(),
        Some((routed_position, context.one())),
    );
    let mut shifted_powers = PATH_SECTOR;
    shifted_powers[affine_source] = i64::MIN;
    shifted_powers[source_position] = i64::MIN;
    assert_eq!(
        shifted.try_expand_endpoints(
            family,
            &IntegralKey::try_new(shifted_powers).unwrap(),
            FactorizedNumeratorLiftExpansionLimits::default(),
        ),
        Err(FactorizedNumeratorLiftError::RoutedPowerShiftUnderflow {
            position: routed_position,
            power: i64::MIN,
            decrement: i64::MIN.unsigned_abs(),
        })
    );
}

#[test]
fn optional_canonicalization_coalesces_exactly_and_rejects_foreign_authority() {
    let authority = derive_k6_terminal_authority().unwrap();
    let family = authority.family();
    let rule = factorization_for_sector(authority.factorization_rules(), &PATH_SECTOR);
    let action = path_action(family, rule);
    let raw = action
        .try_expand_endpoints(
            family,
            &IntegralKey::try_new(PATH_A).unwrap(),
            FactorizedNumeratorLiftExpansionLimits::default(),
        )
        .unwrap();
    let canonicalizer = authority.canonicalizer().unwrap();
    let route_count = raw.endpoints().len() * canonicalizer.group_order();
    assert_eq!(
        raw.try_canonicalize_endpoints(
            family,
            canonicalizer,
            FactorizedNumeratorLiftExpansionLimits {
                max_canonicalization_routes: route_count - 1,
                ..FactorizedNumeratorLiftExpansionLimits::default()
            },
        ),
        Err(FactorizedNumeratorLiftError::ResourceLimit {
            resource: "factorized numerator canonicalization routes",
            requested: route_count,
            limit: route_count - 1,
        })
    );
    let transported_entries = route_count * family.denominator_count();
    assert_eq!(
        raw.try_canonicalize_endpoints(
            family,
            canonicalizer,
            FactorizedNumeratorLiftExpansionLimits {
                max_canonicalization_power_entries: transported_entries - 1,
                ..FactorizedNumeratorLiftExpansionLimits::default()
            },
        ),
        Err(FactorizedNumeratorLiftError::ResourceLimit {
            resource: "factorized numerator canonicalization transported power entries",
            requested: transported_entries,
            limit: transported_entries - 1,
        })
    );
    assert!(matches!(
        raw.try_canonicalize_endpoints(
            family,
            canonicalizer,
            FactorizedNumeratorLiftExpansionLimits {
                exact_algebra: ExactAlgebraLimits {
                    max_polynomial_terms: 0,
                    ..ExactAlgebraLimits::default()
                },
                ..FactorizedNumeratorLiftExpansionLimits::default()
            },
        ),
        Err(FactorizedNumeratorLiftError::ExactAlgebra(_))
    ));
    let canonical = raw
        .try_canonicalize_endpoints(
            family,
            canonicalizer,
            FactorizedNumeratorLiftExpansionLimits::default(),
        )
        .unwrap();
    assert_eq!(canonical.source(), raw.source());
    assert!(canonical.belongs_to_action(&action));
    assert!(canonical.endpoints().len() <= raw.endpoints().len());
    assert!(
        canonical
            .endpoints()
            .windows(2)
            .all(|window| window[0].key() < window[1].key())
    );
    assert!(
        canonical
            .endpoints()
            .iter()
            .all(|endpoint| !endpoint.coefficient().is_zero())
    );

    let coefficient = raw.endpoints()[0].coefficient().clone();
    let opposite = family
        .coefficient_context()
        .try_neg(&coefficient, ExactAlgebraLimits::default())
        .unwrap();
    let cancelling = super::FactorizedNumeratorLiftExpansion {
        family_fingerprint: raw.family_fingerprint.clone(),
        routing_identity: raw.routing_identity.clone(),
        source: raw.source().clone(),
        endpoints: Box::new([
            super::FactorizedNumeratorLiftEndpoint {
                key: raw.endpoints()[0].key().clone(),
                coefficient,
            },
            super::FactorizedNumeratorLiftEndpoint {
                key: raw.endpoints()[0].key().clone(),
                coefficient: opposite,
            },
        ]),
    };
    assert!(
        cancelling
            .try_canonicalize_endpoints(
                family,
                canonicalizer,
                FactorizedNumeratorLiftExpansionLimits::default(),
            )
            .unwrap()
            .endpoints()
            .is_empty()
    );

    let (first_orbit_key, second_orbit_key) = raw
        .endpoints()
        .iter()
        .find_map(|endpoint| {
            let orbit = canonicalizer.orbit(endpoint.key()).unwrap();
            orbit
                .images()
                .iter()
                .find(|image| image.integral() != endpoint.key())
                .map(|image| {
                    (
                        IntegralKey::try_new(endpoint.key().powers().iter().copied()).unwrap(),
                        IntegralKey::try_new(image.integral().powers().iter().copied()).unwrap(),
                    )
                })
        })
        .expect("the authenticated K6 action has a nontrivial endpoint orbit");
    let distinct_orbit = super::FactorizedNumeratorLiftExpansion {
        family_fingerprint: raw.family_fingerprint.clone(),
        routing_identity: raw.routing_identity.clone(),
        source: IntegralKey::try_new(raw.source().powers().iter().copied()).unwrap(),
        endpoints: Box::new([
            super::FactorizedNumeratorLiftEndpoint {
                key: first_orbit_key,
                coefficient: family.coefficient_context().one(),
            },
            super::FactorizedNumeratorLiftEndpoint {
                key: second_orbit_key,
                coefficient: family.coefficient_context().one(),
            },
        ]),
    };
    let coalesced_orbit = distinct_orbit
        .try_canonicalize_endpoints(
            family,
            canonicalizer,
            FactorizedNumeratorLiftExpansionLimits::default(),
        )
        .unwrap();
    assert_eq!(coalesced_orbit.endpoints().len(), 1);
    assert_eq!(
        coalesced_orbit.endpoints()[0].coefficient(),
        &family.coefficient_context().integer(2)
    );

    let foreign = derive_two_loop_unit_mass_sunset().unwrap();
    assert_eq!(
        raw.try_canonicalize_endpoints(
            family,
            foreign.canonicalizer().unwrap(),
            FactorizedNumeratorLiftExpansionLimits::default(),
        ),
        Err(FactorizedNumeratorLiftError::WrongCanonicalizerFamily)
    );
    assert_eq!(
        action.try_expand_endpoints(
            foreign.family(),
            &IntegralKey::try_new(PATH_A).unwrap(),
            FactorizedNumeratorLiftExpansionLimits::default(),
        ),
        Err(FactorizedNumeratorLiftError::WrongExpansionFamily)
    );
}
