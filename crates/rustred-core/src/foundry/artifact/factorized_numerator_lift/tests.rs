use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::artifact::{
    FactorizationRule, UnimodularLoopBasis, derive_k6_terminal_authority,
    derive_two_loop_unit_mass_sunset,
};

use super::{
    FactorizedNumeratorLiftCompilation, FactorizedNumeratorLiftError,
    FactorizedNumeratorLiftLimits, FactorizedNumeratorLiftStart, compile_factorized_numerator_lift,
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
