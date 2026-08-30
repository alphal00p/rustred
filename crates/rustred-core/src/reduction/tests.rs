use std::collections::BTreeMap;

use crate::algebra::{CoefficientContext, ExactAlgebraError, ExactAlgebraLimits};
use crate::family::IntegralKey;
use crate::foundry::artifact::{
    derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};

use super::reducer::{accumulate_master, begin_expansion, convolve_factor_expansion};
use super::{Reducer, ReductionError, ReductionLimits};

#[test]
fn factorization_convolves_every_typed_master_of_a_lower_family() {
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let d = context.parameter("d").unwrap();
    let mut products = BTreeMap::new();
    products.insert(
        IntegralKey::try_new([0, 0, 0, 0, 0, 0]).unwrap(),
        context.one(),
    );
    let mut two_master_dependency = BTreeMap::new();
    two_master_dependency.insert(IntegralKey::try_new([0, 1, 1]).unwrap(), context.integer(2));
    two_master_dependency.insert(IntegralKey::try_new([1, 1, 1]).unwrap(), d.clone());
    assert_eq!(
        convolve_factor_expansion(
            &context,
            &products,
            &two_master_dependency,
            &[3, 4, 5],
            6,
            ReductionLimits {
                max_factorization_terms: 1,
                ..ReductionLimits::default()
            },
        ),
        Err(ReductionError::FactorizationTermLimit {
            requested: 2,
            limit: 1,
        })
    );
    products = convolve_factor_expansion(
        &context,
        &products,
        &two_master_dependency,
        &[3, 4, 5],
        6,
        ReductionLimits::default(),
    )
    .unwrap();

    let mut one_master_dependency = BTreeMap::new();
    one_master_dependency.insert(IntegralKey::try_new([1]).unwrap(), context.integer(3));
    let products = convolve_factor_expansion(
        &context,
        &products,
        &one_master_dependency,
        &[2],
        6,
        ReductionLimits::default(),
    )
    .unwrap();

    assert_eq!(products.len(), 2);
    assert_eq!(
        products.get(&IntegralKey::try_new([0, 0, 1, 0, 1, 1]).unwrap()),
        Some(&context.integer(6))
    );
    assert_eq!(
        products.get(&IntegralKey::try_new([0, 0, 1, 1, 1, 1]).unwrap()),
        Some(
            &context
                .try_mul(&context.integer(3), &d, ExactAlgebraLimits::default())
                .unwrap()
        )
    );
}

#[test]
fn sunset_artifact_closes_a_bounded_complete_lattice_without_foreign_terminals() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let mut reducer = Reducer::new(&artifact).unwrap();
    for a in -3..=4 {
        for b in -3..=4 {
            for c in -3..=4 {
                let target = IntegralKey::try_new([a, b, c]).unwrap();
                let reduction = reducer.reduce_unit_mass(&target).unwrap();
                assert_eq!(reduction.target(), &target);
                assert!(
                    reduction
                        .terms()
                        .keys()
                        .all(|master| artifact.masters().contains(master)),
                    "foreign terminal while reducing {:?}",
                    target.powers()
                );
            }
        }
    }
}

#[test]
fn sunset_rejects_only_the_uncertified_machine_endpoint_before_rule_selection() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    assert_eq!(artifact.supported_root_power_bounds().len(), 3);
    assert!(
        artifact
            .supported_root_power_bounds()
            .iter()
            .all(|bounds| bounds.lower() == i64::MIN && bounds.upper() == i64::MAX - 1)
    );
    let target = IntegralKey::try_new([1, i64::MAX, i64::MAX]).unwrap();
    assert_eq!(
        Reducer::new(&artifact).unwrap().reduce_unit_mass(&target),
        Err(ReductionError::OutsideCertifiedRootDomain {
            position: 1,
            value: i64::MAX,
            lower: i64::MIN,
            upper: i64::MAX - 1,
        })
    );
}

#[test]
fn sunset_dependency_work_and_cache_limits_are_aggregate() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    assert_eq!(
        Reducer::new(&artifact)
            .unwrap()
            .statistics()
            .cached_integrals(),
        3
    );
    assert!(matches!(
        Reducer::with_limits(
            &artifact,
            ReductionLimits {
                max_cached_integrals: 2,
                ..ReductionLimits::default()
            }
        ),
        Err(ReductionError::CacheLimit {
            requested: 3,
            limit: 2,
        })
    ));

    let target = IntegralKey::try_new([0, 2, 2]).unwrap();
    let mut limited = Reducer::with_limits(
        &artifact,
        ReductionLimits {
            max_rule_applications: 1,
            ..ReductionLimits::default()
        },
    )
    .unwrap();
    assert_eq!(
        limited.reduce_unit_mass(&target),
        Err(ReductionError::RuleApplicationLimit {
            requested: 2,
            limit: 1,
        })
    );

    let mut reducer = Reducer::new(&artifact).unwrap();
    reducer.reduce_unit_mass(&target).unwrap();
    assert_eq!(
        reducer.statistics().rule_applications(),
        2,
        "one parent factorization plus one memoized lower-family recurrence"
    );
    assert!(reducer.statistics().cache_hits() >= 1);
}

#[test]
fn sunset_rules_match_exact_top_pair_factorization_and_corner_goldens() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let context = artifact.coefficient_context();
    let s = IntegralKey::try_new([1, 1, 1]).unwrap();
    let p = IntegralKey::try_new([0, 1, 1]).unwrap();
    let d_minus_2 = d_minus(context, 2);
    let d_minus_3 = d_minus(context, 3);
    let d_minus_8 = d_minus(context, 8);
    let d_minus_2_squared = context
        .try_mul(&d_minus_2, &d_minus_2, ExactAlgebraLimits::default())
        .unwrap();
    let mut reducer = Reducer::new(&artifact).unwrap();

    let j211 = reducer
        .reduce_unit_mass(&IntegralKey::try_new([2, 1, 1]).unwrap())
        .unwrap();
    assert_eq!(j211.coefficient(&s), Some(&div(context, &d_minus_3, 3)));
    assert_eq!(j211.terms().len(), 1);

    let j221 = reducer
        .reduce_unit_mass(&IntegralKey::try_new([2, 2, 1]).unwrap())
        .unwrap();
    let s221 = context
        .try_mul(&d_minus_2, &d_minus_3, ExactAlgebraLimits::default())
        .map(|value| div(context, &value, 9))
        .unwrap();
    let p221 = context
        .try_neg(
            &div(context, &d_minus_2_squared, 12),
            ExactAlgebraLimits::default(),
        )
        .unwrap();
    assert_eq!(j221.coefficient(&s), Some(&s221));
    assert_eq!(j221.coefficient(&p), Some(&p221));

    let j311 = reducer
        .reduce_unit_mass(&IntegralKey::try_new([3, 1, 1]).unwrap())
        .unwrap();
    let s311 = context
        .try_mul(&d_minus_8, &d_minus_3, ExactAlgebraLimits::default())
        .map(|value| div(context, &value, 18))
        .unwrap();
    assert_eq!(j311.coefficient(&s), Some(&s311));
    assert_eq!(
        j311.coefficient(&p),
        Some(&div(context, &d_minus_2_squared, 12))
    );

    for (target, expected) in [
        ([0, 2, 1], div(context, &d_minus_2, 2)),
        ([0, 2, 2], div(context, &d_minus_2_squared, 4)),
        ([-1, 1, 1], context.one()),
    ] {
        let result = reducer
            .reduce_unit_mass(&IntegralKey::try_new(target).unwrap())
            .unwrap();
        assert_eq!(result.terms().len(), 1);
        assert_eq!(result.coefficient(&p), Some(&expected));
    }

    let j_minus_2 = reducer
        .reduce_unit_mass(&IntegralKey::try_new([-2, 1, 1]).unwrap())
        .unwrap();
    let expected = context
        .try_add(
            &context.one(),
            &context
                .try_div(
                    &context.integer(4),
                    &context.parameter("d").unwrap(),
                    ExactAlgebraLimits::default(),
                )
                .unwrap(),
            ExactAlgebraLimits::default(),
        )
        .unwrap();
    assert_eq!(j_minus_2.coefficient(&p), Some(&expected));
    assert_eq!(j_minus_2.terms().len(), 1);
}

#[test]
fn sunset_symmetry_memoization_and_mass_restoration_preserve_typed_outputs() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let mut reducer = Reducer::new(&artifact).unwrap();
    let canonical = IntegralKey::try_new([1, 1, 3]).unwrap();
    let routed = IntegralKey::try_new([3, 1, 1]).unwrap();
    let first = reducer.reduce_unit_mass(&canonical).unwrap();
    let before = reducer.statistics();
    let second = reducer.reduce_unit_mass(&routed).unwrap();
    assert_eq!(first.terms(), second.terms());
    assert_eq!(second.target(), &routed);
    assert_eq!(
        reducer.statistics().rule_applications(),
        before.rule_applications()
    );
    assert!(reducer.statistics().cache_hits() > before.cache_hits());

    let target = IntegralKey::try_new([2, 2, 1]).unwrap();
    let homogeneous = reducer
        .reduce_with_common_mass_homogeneity(&target)
        .unwrap();
    assert_eq!(
        homogeneous
            .coefficient(&IntegralKey::try_new([1, 1, 1]).unwrap())
            .unwrap()
            .common_mass_squared_power(),
        -2
    );
    assert_eq!(
        homogeneous
            .coefficient(&IntegralKey::try_new([0, 1, 1]).unwrap())
            .unwrap()
            .common_mass_squared_power(),
        -3
    );

    let unit = reducer.reduce_unit_mass(&target).unwrap();
    let mass_squared = artifact.coefficient_context().integer(4);
    let restored = reducer
        .reduce_with_common_mass_squared(&target, &mass_squared)
        .unwrap();
    for (master, mass_denominator) in [
        (IntegralKey::try_new([1, 1, 1]).unwrap(), 16_i64),
        (IntegralKey::try_new([0, 1, 1]).unwrap(), 64_i64),
    ] {
        let expected = artifact
            .coefficient_context()
            .try_div(
                unit.coefficient(&master).unwrap(),
                &artifact.coefficient_context().integer(mass_denominator),
                ExactAlgebraLimits::default(),
            )
            .unwrap();
        assert_eq!(restored.coefficient(&master), Some(&expected));
    }
}

fn d_minus(context: &CoefficientContext, value: i64) -> crate::algebra::Coefficient {
    context
        .try_sub(
            &context.parameter("d").unwrap(),
            &context.integer(value),
            ExactAlgebraLimits::default(),
        )
        .unwrap()
}

fn div(
    context: &CoefficientContext,
    numerator: &crate::algebra::Coefficient,
    denominator: i64,
) -> crate::algebra::Coefficient {
    context
        .try_div(
            numerator,
            &context.integer(denominator),
            ExactAlgebraLimits::default(),
        )
        .unwrap()
}

#[test]
fn generated_rule_reduces_positive_powers_and_memoizes_intermediates() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let context = artifact.coefficient_context();
    let master = artifact.masters().first().unwrap().clone();
    let target = IntegralKey::try_new([4]).unwrap();
    let mut reducer = Reducer::new(&artifact).unwrap();
    let scans_before = artifact.indexed_context().authentication_scan_counts();

    let reduction = reducer.reduce_unit_mass(&target).unwrap();
    assert_eq!(reduction.target(), &target);
    assert_eq!(reduction.terms().len(), 1);
    let actual = reduction.coefficient(&master).unwrap();
    // (d-2)(d-4)(d-6)/(2*4*6), for D = k^2-1.
    let mut expected = context.one();
    for (offset, denominator) in [(2, 2), (4, 4), (6, 6)] {
        let factor = context
            .try_sub(
                &context.parameter("d").unwrap(),
                &context.integer(offset),
                ExactAlgebraLimits::default(),
            )
            .unwrap();
        let factor = context
            .try_div(
                &factor,
                &context.integer(denominator),
                ExactAlgebraLimits::default(),
            )
            .unwrap();
        expected = context
            .try_mul(&expected, &factor, ExactAlgebraLimits::default())
            .unwrap();
    }
    assert_eq!(actual, &expected);
    assert_eq!(reducer.statistics().rule_applications(), 3);
    assert_eq!(reducer.statistics().cached_integrals(), 4);
    assert_eq!(
        artifact.indexed_context().authentication_scan_counts(),
        scans_before,
        "sealed artifact application must not rescan indexed coefficient payloads"
    );

    let before = reducer.statistics();
    assert_eq!(reducer.reduce_unit_mass(&target).unwrap(), reduction);
    let after = reducer.statistics();
    assert_eq!(after.rule_applications(), before.rule_applications());
    assert_eq!(after.cache_hits(), before.cache_hits() + 1);
}

#[test]
fn nonpositive_powers_are_explicit_scaleless_zero_terminals() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let mut reducer = Reducer::new(&artifact).unwrap();
    for power in [0, -1, i64::MIN] {
        let target = IntegralKey::try_new([power]).unwrap();
        let reduction = reducer.reduce_unit_mass(&target).unwrap();
        assert_eq!(reduction.target(), &target);
        assert!(reduction.is_zero());
    }
    assert_eq!(reducer.statistics().rule_applications(), 0);
}

#[test]
fn common_mass_homogeneity_is_restored_without_an_artifact_mass_symbol() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let master = artifact.masters().first().unwrap().clone();
    let target = IntegralKey::try_new([3]).unwrap();
    let mut reducer = Reducer::new(&artifact).unwrap();

    let homogeneous = reducer
        .reduce_with_common_mass_homogeneity(&target)
        .unwrap();
    let coefficient = homogeneous.coefficient(&master).unwrap();
    assert_eq!(coefficient.common_mass_squared_power(), -2);

    // Materializing m^2=2 must multiply the unit-mass result by 2^-2.
    let unit = coefficient.unit_mass_coefficient().clone();
    let restored = reducer
        .reduce_with_common_mass_squared(&target, &artifact.coefficient_context().integer(2))
        .unwrap();
    let expected = artifact
        .coefficient_context()
        .try_div(
            &unit,
            &artifact.coefficient_context().integer(4),
            ExactAlgebraLimits::default(),
        )
        .unwrap();
    assert_eq!(restored.coefficient(&master), Some(&expected));
}

#[test]
fn reducer_reports_arity_context_and_work_limits_with_typed_errors() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let wrong_arity = IntegralKey::try_new([1, 1]).unwrap();
    let mut reducer = Reducer::new(&artifact).unwrap();
    assert_eq!(
        reducer.reduce_unit_mass(&wrong_arity),
        Err(ReductionError::WrongArity {
            expected: 1,
            actual: 2,
        })
    );

    let foreign = CoefficientContext::try_new(["x"]).unwrap();
    assert!(matches!(
        reducer.reduce_with_common_mass_squared(
            &IntegralKey::try_new([2]).unwrap(),
            &foreign.parameter("x").unwrap(),
        ),
        Err(ReductionError::ExactAlgebra(
            ExactAlgebraError::VariableMapMismatch { .. }
        ))
    ));
    assert_eq!(
        reducer.reduce_with_common_mass_squared(
            &IntegralKey::try_new([1]).unwrap(),
            &artifact.coefficient_context().zero(),
        ),
        Err(ReductionError::ZeroCommonMass)
    );

    let mut limited = Reducer::with_limits(
        &artifact,
        ReductionLimits {
            max_rule_applications: 1,
            ..ReductionLimits::default()
        },
    )
    .unwrap();
    assert_eq!(
        limited.reduce_unit_mass(&IntegralKey::try_new([3]).unwrap()),
        Err(ReductionError::RuleApplicationLimit {
            requested: 2,
            limit: 1,
        })
    );
}

#[test]
fn cache_capacity_is_bounded_and_can_be_cleared_back_to_explicit_masters() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let seed_statistics = Reducer::new(&artifact).unwrap().statistics();
    assert!(seed_statistics.cached_coefficient_terms() > 0);
    assert!(seed_statistics.cached_coefficient_bytes() > 0);
    assert!(matches!(
        Reducer::with_limits(
            &artifact,
            ReductionLimits {
                max_cached_integrals: 0,
                ..ReductionLimits::default()
            },
        ),
        Err(ReductionError::CacheLimit {
            requested: 1,
            limit: 0,
        })
    ));
    assert_eq!(
        Reducer::with_limits(
            &artifact,
            ReductionLimits {
                max_cached_coefficient_terms: seed_statistics.cached_coefficient_terms() - 1,
                ..ReductionLimits::default()
            },
        )
        .err(),
        Some(ReductionError::CacheCoefficientTermLimit {
            requested: seed_statistics.cached_coefficient_terms(),
            limit: seed_statistics.cached_coefficient_terms() - 1,
        })
    );
    assert_eq!(
        Reducer::with_limits(
            &artifact,
            ReductionLimits {
                max_cached_coefficient_bytes: seed_statistics.cached_coefficient_bytes() - 1,
                ..ReductionLimits::default()
            },
        )
        .err(),
        Some(ReductionError::CacheCoefficientByteLimit {
            requested: seed_statistics.cached_coefficient_bytes(),
            limit: seed_statistics.cached_coefficient_bytes() - 1,
        })
    );

    let mut coefficient_limited = Reducer::with_limits(
        &artifact,
        ReductionLimits {
            max_cached_coefficient_terms: seed_statistics.cached_coefficient_terms(),
            ..ReductionLimits::default()
        },
    )
    .unwrap();
    assert!(matches!(
        coefficient_limited.reduce_unit_mass(&IntegralKey::try_new([2]).unwrap()),
        Err(ReductionError::CacheCoefficientTermLimit { .. })
    ));
    let mut byte_limited = Reducer::with_limits(
        &artifact,
        ReductionLimits {
            max_cached_coefficient_bytes: seed_statistics.cached_coefficient_bytes(),
            ..ReductionLimits::default()
        },
    )
    .unwrap();
    assert!(matches!(
        byte_limited.reduce_unit_mass(&IntegralKey::try_new([2]).unwrap()),
        Err(ReductionError::CacheCoefficientByteLimit { .. })
    ));

    let mut reducer = Reducer::with_limits(
        &artifact,
        ReductionLimits {
            max_cached_integrals: 2,
            ..ReductionLimits::default()
        },
    )
    .unwrap();
    assert!(
        reducer
            .reduce_unit_mass(&IntegralKey::try_new([0]).unwrap())
            .unwrap()
            .is_zero()
    );
    assert_eq!(reducer.statistics().cached_integrals(), 2);
    assert_eq!(
        reducer.reduce_unit_mass(&IntegralKey::try_new([-1]).unwrap()),
        Err(ReductionError::CacheLimit {
            requested: 3,
            limit: 2,
        })
    );
    reducer.clear_cache().unwrap();
    assert_eq!(reducer.statistics().cached_integrals(), 1);
    assert_eq!(
        reducer.statistics().cached_coefficient_terms(),
        seed_statistics.cached_coefficient_terms()
    );
    assert_eq!(
        reducer.statistics().cached_coefficient_bytes(),
        seed_statistics.cached_coefficient_bytes()
    );
}

#[test]
fn generic_failure_and_collection_primitives_are_typed_and_exact() {
    let mut incomplete = derive_one_loop_unit_mass_tadpole().unwrap();
    incomplete.clear_rules_for_test();
    let target = IntegralKey::try_new([2]).unwrap();
    assert_eq!(
        Reducer::new(&incomplete).unwrap().reduce_unit_mass(&target),
        Err(ReductionError::UncoveredIntegral {
            target: target.clone(),
        })
    );
    assert_eq!(
        Reducer::with_limits(
            &incomplete,
            ReductionLimits {
                max_rule_applications: 0,
                ..ReductionLimits::default()
            },
        )
        .unwrap()
        .reduce_unit_mass(&target),
        Err(ReductionError::UncoveredIntegral {
            target: target.clone(),
        }),
        "rule budget must be charged only after an applicable rule is selected"
    );

    let mut duplicate = derive_one_loop_unit_mass_tadpole().unwrap();
    duplicate.duplicate_first_rule_for_test();
    let reducer = Reducer::new(&duplicate).unwrap();
    let selected = reducer.select_first_rule(&target).unwrap();
    assert!(
        std::ptr::eq(selected.rule, &duplicate.rules()[0]),
        "selection must deterministically choose the first applicable installed rule"
    );

    let mut guard_failing = derive_one_loop_unit_mass_tadpole().unwrap();
    let zero_at_assignment_one = {
        let context = guard_failing.indexed_context();
        let coefficient = context
            .sub(&context.index(0).unwrap(), &context.one())
            .unwrap();
        context
            .numerator_condition_with_limits(&coefficient, Default::default())
            .unwrap()
    };
    guard_failing.replace_first_raw_rule_guard_for_test(zero_at_assignment_one);
    assert_eq!(
        Reducer::new(&guard_failing)
            .unwrap()
            .reduce_unit_mass(&target),
        Err(ReductionError::UncoveredIntegral {
            target: target.clone(),
        }),
        "a guard-failing raw rule must not bypass guarded-cell selection"
    );

    let mut mixed = derive_two_loop_unit_mass_sunset().unwrap();
    let top_target = IntegralKey::try_new([1, 1, 2]).unwrap();
    let zero_at_top_assignment = {
        let context = mixed.indexed_context();
        let coefficient = context
            .sub(&context.index(0).unwrap(), &context.one())
            .unwrap();
        context
            .numerator_condition_with_limits(&coefficient, Default::default())
            .unwrap()
    };
    mixed.inject_guard_failing_cell_raw_fallback_for_test(zero_at_top_assignment);
    assert!(
        matches!(
            Reducer::new(&mixed)
                .unwrap()
                .select_first_rule(&top_target),
            Err(ReductionError::UncoveredIntegral { target }) if target == top_target
        ),
        "a raw fallback must not bypass the same failed guard on a rule cell"
    );

    let mut no_homogeneity_proof = derive_one_loop_unit_mass_tadpole().unwrap();
    no_homogeneity_proof.clear_common_mass_homogeneity_for_test();
    assert_eq!(
        Reducer::new(&no_homogeneity_proof)
            .unwrap()
            .reduce_with_common_mass_homogeneity(&target),
        Err(ReductionError::MissingCommonMassHomogeneityProof)
    );

    let mut active = std::collections::BTreeSet::new();
    begin_expansion(&mut active, &target).unwrap();
    assert_eq!(
        begin_expansion(&mut active, &target),
        Err(ReductionError::CycleDetected {
            target: target.clone(),
        })
    );

    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let context = artifact.coefficient_context();
    let master = artifact.masters().first().unwrap();
    let mut terms = std::collections::BTreeMap::new();
    accumulate_master(
        context,
        &mut terms,
        master,
        context.one(),
        ReductionLimits::default(),
    )
    .unwrap();
    accumulate_master(
        context,
        &mut terms,
        master,
        context.one(),
        ReductionLimits::default(),
    )
    .unwrap();
    assert_eq!(terms.get(master), Some(&context.integer(2)));
    accumulate_master(
        context,
        &mut terms,
        master,
        context.integer(-2),
        ReductionLimits::default(),
    )
    .unwrap();
    assert!(terms.is_empty(), "zero like-master sums must be pruned");
}
