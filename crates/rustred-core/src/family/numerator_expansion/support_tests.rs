//! Structural envelopes are checked independently of native cancellation.
use super::*;

fn factor(
    context: &CoefficientContext,
    arity: usize,
    constant: i64,
    entries: &[(usize, i64)],
    power: u64,
) -> MultiAffineNumeratorFactor {
    let mut row = vec![context.zero(); arity];
    for &(axis, value) in entries {
        row[axis] = context.integer(value);
    }
    MultiAffineNumeratorFactor::try_new(context.integer(constant), row, power).unwrap()
}

#[test]
fn prefix_support_rank_nine_shape_admits_below_four_million_without_native_expansion() {
    // Same powers and individual widths as the observed 4,241,160 product
    // admission. Synthetic coefficients/incidence, not a replay of its map.
    let context = CoefficientContext::new(["d"]);
    let factors = [
        factor(
            &context,
            15,
            1,
            &(0..8).map(|i| (i, 1)).collect::<Vec<_>>(),
            4,
        ),
        factor(
            &context,
            15,
            1,
            &(2..15).map(|i| (i, 1)).collect::<Vec<_>>(),
            5,
        ),
    ];
    let (plans, support, zero, _, operations) =
        preflight_factors(&context, &factors, 15, 9, Default::default()).unwrap();
    assert!(!zero);
    assert_eq!(plans[0].support_bound, 495);
    assert_eq!(plans[1].support_bound, 8568);
    assert_eq!(plans[0].support_bound * plans[1].support_bound, 4_241_160);
    assert_eq!(support, 1_307_504); // C(9+15,15)
    assert_eq!(plans[1].prefix_support_bound, support);
    assert_eq!(operations, 4_859_235); // Includes all 4,241,160 product pairs.
}

#[test]
fn prefix_support_exact_overlap_preserves_native_cancellation() {
    let family = crate::foundry::artifact::canonical_three_loop_family().unwrap();
    let context = family.coefficient_context();
    let factors = [
        factor(context, 6, 1, &[(0, 1), (1, 1)], 2),
        factor(context, 6, 1, &[(0, -1)], 1),
    ];
    let base = IntegralKey::try_new([7; 6]).unwrap();
    let tight = try_expand_multi_affine_numerator(
        &family,
        &base,
        &factors,
        MultiAffineNumeratorExpansionLimits {
            max_native_polynomial_terms: 10,
            max_endpoints: 10,
            ..Default::default()
        },
    )
    .unwrap();
    let reference = rational_tests::legacy_native_reference(&family, &base, &factors);
    assert!(tight.len() < 10); // Native cancellation, not used in admission.
    assert_eq!(tight.len(), reference.len());
    for endpoint in tight {
        assert_eq!(reference.get(endpoint.key()), Some(endpoint.coefficient()));
    }
}

#[test]
fn prefix_support_homogeneous_constants_and_zero_power_incidence() {
    let family = crate::foundry::artifact::canonical_three_loop_family().unwrap();
    let context = family.coefficient_context();
    let factors = [
        factor(context, 6, 0, &[(0, 1), (1, 1)], 2),
        factor(context, 6, 2, &[], 3), // Nonzero constant remains degree zero.
        factor(context, 6, 1, &[(5, 1)], 0), // No variable/degree contribution.
        factor(context, 6, 0, &[(0, 1), (1, 1)], 3),
    ];
    let (_, support, _, _, _) =
        preflight_factors(context, &factors, 6, 8, Default::default()).unwrap();
    assert_eq!(support, 6); // Homogeneous degree five in two variables.
    let base = IntegralKey::try_new([7; 6]).unwrap();
    let actual = try_expand_multi_affine_numerator(
        &family,
        &base,
        &factors,
        MultiAffineNumeratorExpansionLimits {
            max_native_polynomial_terms: 6,
            max_endpoints: 6,
            ..Default::default()
        },
    )
    .unwrap();
    let reference = rational_tests::legacy_native_reference(&family, &base, &factors);
    assert_eq!(actual.len(), 6);
    for endpoint in actual {
        assert_eq!(reference.get(endpoint.key()), Some(endpoint.coefficient()));
    }
}

#[test]
fn prefix_support_keeps_pair_work_and_full_arity_peak_exponent_limits() {
    let context = CoefficientContext::new(["d"]);
    let factors = [
        factor(&context, 6, 1, &[(0, 1)], 2),
        factor(&context, 6, 1, &[(0, 1)], 2),
    ];
    let (_, support, _, _, work) =
        preflight_factors(&context, &factors, 6, 4, Default::default()).unwrap();
    assert_eq!((support, work), (5, 36));
    for (limits, resource, requested, limit) in [
        (
            MultiAffineNumeratorExpansionLimits {
                max_native_polynomial_operations: 35,
                ..Default::default()
            },
            "multi-affine native polynomial operations",
            36,
            35,
        ),
        (
            MultiAffineNumeratorExpansionLimits {
                max_native_exponent_entries: 77,
                ..Default::default()
            },
            "multi-affine native exponent entries",
            78,
            77,
        ),
        (
            MultiAffineNumeratorExpansionLimits {
                max_native_polynomial_terms: 4,
                ..Default::default()
            },
            "multi-affine projected polynomial support",
            5,
            4,
        ),
    ] {
        assert_eq!(
            preflight_factors(&context, &factors, 6, 4, limits).unwrap_err(),
            MultiAffineNumeratorExpansionError::ResourceLimit {
                resource,
                requested,
                limit
            }
        );
    }
    // Inputs + affine + powered + result have 3+2+3+5 rows at the peak.
    assert!(
        preflight_factors(
            &context,
            &factors,
            6,
            4,
            MultiAffineNumeratorExpansionLimits {
                max_native_exponent_entries: 78,
                ..Default::default()
            }
        )
        .is_ok()
    );
}

#[test]
fn prefix_support_sparse_disjoint_product_survives_dense_bound_overflow() {
    let context = CoefficientContext::new(["d"]);
    let mut prefix = PrefixSupport::try_new(128).unwrap();
    for axis in 0..128 {
        prefix
            .include(&factor(&context, 128, 0, &[(axis, 1)], 64))
            .unwrap();
        assert_eq!(prefix.refine(1), 1);
    }
    assert!(multiset_support(8192, 128).is_err());
}

#[test]
fn prefix_support_disjoint_inhomogeneous_factors_keep_smaller_product_bound() {
    let context = CoefficientContext::new(["d"]);
    let factors = [
        factor(&context, 6, 1, &[(0, 1)], 1),
        factor(&context, 6, 1, &[(5, 1)], 1),
    ];
    let (_, support, _, _, _) =
        preflight_factors(&context, &factors, 6, 2, Default::default()).unwrap();
    assert_eq!(support, 4); // Pair product4 < degree envelope C(4,2)=6.
}
