//! Pure structural admission tests never allocate the large polynomial whose
//! prospective payload crossed a historical default. These exercise policies,
//! not a claim that arbitrarily large exact expansion is cheap.
use super::*;

#[test]
fn caller_unbounded_policy_admits_observed_exponent_entries_but_keeps_overflow_checks() {
    let default = MultiAffineNumeratorExpansionLimits::default();
    let trusted = MultiAffineNumeratorExpansionLimits::unbounded_resources();
    let label = "multi-affine native exponent entries";
    assert_eq!(
        admit_product_limit(label, 4_576_278, 15, default.max_native_exponent_entries),
        Err(MultiAffineNumeratorExpansionError::ResourceLimit {
            resource: label,
            requested: 68_644_170,
            limit: 64_000_000,
        })
    );
    assert!(admit_product_limit(label, 4_576_278, 15, trusted.max_native_exponent_entries).is_ok());
    assert_eq!(
        admit_product_limit(label, usize::MAX, 2, trusted.max_native_exponent_entries),
        Err(MultiAffineNumeratorExpansionError::ResourceCountOverflow { resource: label })
    );
}

#[test]
fn factor_preflight_over_old_exponent_cap_needs_no_large_native_materialization() {
    let context = CoefficientContext::new(["d"]);
    // A degree-11 homogeneous polynomial in 15 variables has C(25,14)
    // monomials. Two simultaneously live outputs exceed 64M exponent entries.
    let factors =
        [
            MultiAffineNumeratorFactor::try_new(context.zero(), vec![context.one(); 15], 11)
                .unwrap(),
        ];
    let trusted = MultiAffineNumeratorExpansionLimits::unbounded_resources();
    let default = MultiAffineNumeratorExpansionLimits::default();
    let capped = MultiAffineNumeratorExpansionLimits {
        max_native_exponent_entries: default.max_native_exponent_entries,
        ..trusted
    };
    assert!(
        matches!(preflight_factors(&context, &factors, 15, 11, capped),
        Err(MultiAffineNumeratorExpansionError::ResourceLimit {
            resource: "multi-affine native exponent entries", requested, limit: 64_000_000,
        }) if requested > 64_000_000)
    );
    let (_, support, zero, _, _) = preflight_factors(&context, &factors, 15, 11, trusted).unwrap();
    assert_eq!(support, 4_457_400);
    assert!(!zero);
}

#[test]
fn explicit_transport_coefficient_and_endpoint_caps_still_refuse_and_overflows_do_not_wrap() {
    let trusted = MultiAffineNumeratorExpansionLimits::unbounded_resources();
    let weight = CoefficientWeight {
        terms: 64_000_001,
        clone_owned_bytes: 1_000_000_001,
    };
    assert!(admit_live_coefficients(weight, trusted).is_ok());
    for capped in [
        MultiAffineNumeratorExpansionLimits {
            max_retained_coefficient_terms: 64_000_000,
            ..trusted
        },
        MultiAffineNumeratorExpansionLimits {
            max_retained_coefficient_clone_owned_bytes: 1_000_000_000,
            ..trusted
        },
    ] {
        assert!(admit_live_coefficients(weight, capped).is_err());
    }
    assert!(preflight_endpoint_key_storage(4_576_278, 15, trusted).is_ok());
    assert!(
        preflight_endpoint_key_storage(
            4_576_278,
            15,
            MultiAffineNumeratorExpansionLimits {
                max_endpoint_power_entries: 64_000_000,
                ..trusted
            }
        )
        .is_err()
    );
    assert!(matches!(
        preflight_endpoint_key_storage(usize::MAX, 2, trusted),
        Err(MultiAffineNumeratorExpansionError::ResourceCountOverflow { .. })
    ));
}

#[test]
fn trusted_policy_preserves_small_exact_expansion_and_representation_failures() {
    let family = crate::foundry::artifact::canonical_three_loop_family().unwrap();
    let context = family.coefficient_context();
    let mut row = vec![context.zero(); 6];
    row[0] = context.one();
    row[1] = context.integer(-1);
    let factors = [MultiAffineNumeratorFactor::try_new(context.one(), row, 3).unwrap()];
    let base = IntegralKey::try_new([4; 6]).unwrap();
    assert_eq!(
        try_expand_multi_affine_numerator(&family, &base, &factors, Default::default()).unwrap(),
        try_expand_multi_affine_numerator(
            &family,
            &base,
            &factors,
            MultiAffineNumeratorExpansionLimits::unbounded_resources()
        )
        .unwrap()
    );
    let too_large = [MultiAffineNumeratorFactor::try_new(
        context.zero(),
        vec![context.one(); 6],
        i32::MAX as u64 + 1,
    )
    .unwrap()];
    assert!(matches!(
        try_expand_multi_affine_numerator(
            &family,
            &base,
            &too_large,
            MultiAffineNumeratorExpansionLimits::unbounded_resources()
        ),
        Err(MultiAffineNumeratorExpansionError::NativeExponentLimit { .. })
    ));
}
