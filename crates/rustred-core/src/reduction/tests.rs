use crate::algebra::{CoefficientContext, ExactAlgebraError, ExactAlgebraLimits};
use crate::family::IntegralKey;
use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;

use super::reducer::{accumulate_master, begin_expansion};
use super::{Reducer, ReductionError, ReductionLimits};

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
