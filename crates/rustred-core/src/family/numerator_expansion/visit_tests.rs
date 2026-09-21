use super::*;

fn fixture() -> (IntegralFamily, IntegralKey) {
    let family = crate::foundry::artifact::canonical_three_loop_family().unwrap();
    let base = IntegralKey::try_new([1, 0, -2, 3, 0, 1]).unwrap();
    (family, base)
}

fn factor(
    family: &IntegralFamily,
    constant: &str,
    entries: &[(usize, &str)],
    power: u64,
) -> MultiAffineNumeratorFactor {
    let context = family.coefficient_context();
    let mut row = vec![context.zero(); family.denominator_count()];
    for &(axis, value) in entries {
        row[axis] = context.coefficient_fixture(value);
    }
    MultiAffineNumeratorFactor::try_new(context.coefficient_fixture(constant), row, power).unwrap()
}

fn visited(
    family: &IntegralFamily,
    base: &IntegralKey,
    factors: &[MultiAffineNumeratorFactor],
    limits: MultiAffineNumeratorExpansionLimits,
) -> Result<Vec<IntegralKey>, MultiAffineNumeratorExpansionError> {
    try_expand_multi_affine_support_with_usage(family, base, factors, limits, |_| Ok(()))?.collect()
}

fn materialized(
    family: &IntegralFamily,
    base: &IntegralKey,
    factors: &[MultiAffineNumeratorFactor],
    limits: MultiAffineNumeratorExpansionLimits,
) -> Result<Vec<IntegralKey>, MultiAffineNumeratorExpansionError> {
    Ok(
        try_expand_multi_affine_numerator_with_usage(family, base, factors, limits, |_| Ok(()))?
            .iter()
            .map(|term| term.key().clone())
            .collect(),
    )
}

#[test]
fn trace_support_matches_full_keys_order_constants_pinches_and_cancellation() {
    let (family, base) = fixture();
    let cases = [
        vec![],
        vec![factor(&family, "0", &[], 0)],
        vec![factor(&family, "0", &[], 3)],
        vec![factor(&family, "-2/3", &[], 4)],
        vec![factor(&family, "0", &[(0, "3/7")], 5)],
        vec![factor(&family, "-3/5", &[(0, "2/7"), (5, "-7/11")], 4)],
        // Exact native cancellation removes odd powers before support is read.
        vec![
            factor(&family, "1", &[(0, "1")], 3),
            factor(&family, "1", &[(0, "-1")], 3),
        ],
        vec![factor(
            &family,
            "(2^180+13)/(3^100+1)",
            &[(4, "-(2^150+1)/(5^75+2)")],
            2,
        )],
    ];
    for factors in cases {
        let expected = materialized(&family, &base, &factors, Default::default()).unwrap();
        let actual = visited(&family, &base, &factors, Default::default()).unwrap();
        assert_eq!(actual, expected);
        assert!(actual.windows(2).all(|pair| pair[0] < pair[1]));
    }
    let cancelled = visited(
        &family,
        &base,
        &[
            factor(&family, "1", &[(0, "1")], 3),
            factor(&family, "1", &[(0, "-1")], 3),
        ],
        Default::default(),
    )
    .unwrap();
    assert_eq!(cancelled.len(), 4);
    assert!(
        cancelled
            .iter()
            .all(|key| (base.powers()[0] - key.powers()[0]) % 2 == 0)
    );
}

#[test]
fn trace_support_preserves_logical_usage_and_reservation_failure_atomicity() {
    let (family, base) = fixture();
    let factors = [factor(&family, "1", &[(0, "1"), (2, "-1")], 3)];
    let mut full_usage = None;
    let full = try_expand_multi_affine_numerator_with_usage(
        &family,
        &base,
        &factors,
        Default::default(),
        |u| {
            full_usage = Some((u.operations, u.endpoints));
            Ok(())
        },
    )
    .unwrap();
    let mut support_usage = None;
    let keys = try_expand_multi_affine_support_with_usage(
        &family,
        &base,
        &factors,
        Default::default(),
        |u| {
            support_usage = Some((u.operations, u.endpoints));
            Ok(())
        },
    )
    .unwrap()
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
    assert_eq!(full_usage, support_usage);
    assert_eq!(
        keys,
        full.iter().map(|t| t.key().clone()).collect::<Vec<_>>()
    );
    let failure = MultiAffineNumeratorExpansionError::ResourceLimit {
        resource: "test shared reservation",
        requested: 2,
        limit: 1,
    };
    let mut callbacks = 0;
    let result = try_expand_multi_affine_support_with_usage(
        &family,
        &base,
        &factors,
        Default::default(),
        |_| Err(failure.clone()),
    )
    .and_then(|mut keys| {
        keys.try_for_each(|key| {
            key?;
            callbacks += 1;
            Ok(())
        })
    });
    assert_eq!(result, Err(failure));
    assert_eq!(callbacks, 0);
}

#[test]
fn trace_support_late_virtual_output_limits_fail_before_any_key() {
    let (family, base) = fixture();
    let factors = [
        factor(&family, "1", &[(0, "1")], 3),
        factor(&family, "1", &[(1, "1")], 3),
    ];
    let expanded = expand_native(&family, &base, &factors, Default::default(), |_| Ok(()))
        .unwrap()
        .unwrap();
    let native = polynomial_weight(&expanded.polynomial, expanded.constant_wrapper_bytes).unwrap();
    let all = expanded
        .input_weight
        .checked_add(native)
        .unwrap()
        .checked_add(native)
        .unwrap();
    for limits in [
        MultiAffineNumeratorExpansionLimits {
            max_retained_coefficient_terms: all.terms - 1,
            ..Default::default()
        },
        MultiAffineNumeratorExpansionLimits {
            max_retained_coefficient_clone_owned_bytes: all.clone_owned_bytes - 1,
            ..Default::default()
        },
    ] {
        // Demonstrate a finish-pass limit, not merely an early input rejection.
        assert!(expand_native(&family, &base, &factors, limits, |_| Ok(())).is_ok());
        let expected = materialized(&family, &base, &factors, limits).unwrap_err();
        let mut callbacks = 0;
        let actual = try_expand_multi_affine_support_with_usage(
            &family,
            &base,
            &factors,
            limits,
            |_| Ok(()),
        )
        .and_then(|mut keys| {
            keys.try_for_each(|key| {
                key?;
                callbacks += 1;
                Ok(())
            })
        });
        assert_eq!(actual, Err(expected));
        assert_eq!(callbacks, 0);
    }
}

#[test]
fn trace_support_preflight_errors_match_full_output_without_callbacks() {
    let (family, base) = fixture();
    let factors = [factor(&family, "1", &[(0, "1"), (2, "-1")], 3)];
    for limits in [
        MultiAffineNumeratorExpansionLimits {
            max_endpoints: 2,
            ..Default::default()
        },
        MultiAffineNumeratorExpansionLimits {
            max_endpoint_power_entries: 10,
            ..Default::default()
        },
        MultiAffineNumeratorExpansionLimits {
            max_retained_endpoint_key_bytes: 1,
            ..Default::default()
        },
        MultiAffineNumeratorExpansionLimits {
            max_native_polynomial_operations: 1,
            ..Default::default()
        },
        MultiAffineNumeratorExpansionLimits {
            max_native_exponent_entries: 1,
            ..Default::default()
        },
        MultiAffineNumeratorExpansionLimits {
            max_factors: 0,
            ..Default::default()
        },
    ] {
        assert_eq!(
            visited(&family, &base, &factors, limits),
            materialized(&family, &base, &factors, limits)
        );
    }
    for bad_base in [
        IntegralKey::try_new([1]).unwrap(),
        IntegralKey::try_new([i64::MIN, 0, 0, 0, 0, 0]).unwrap(),
    ] {
        let error = visited(&family, &bad_base, &factors, Default::default()).unwrap_err();
        assert_eq!(
            Err(error),
            materialized(&family, &bad_base, &factors, Default::default())
        );
    }
    let bad_factors = [factor(&family, "d", &[], 1)];
    assert_eq!(
        visited(&family, &base, &bad_factors, Default::default()),
        materialized(&family, &base, &bad_factors, Default::default())
    );
    let limits = MultiAffineNumeratorExpansionLimits {
        exact_algebra: crate::algebra::ExactAlgebraLimits {
            max_polynomial_terms: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        visited(&family, &base, &[], limits),
        materialized(&family, &base, &[], limits)
    );
}

#[test]
fn trace_support_tiny_constant_virtual_caps_match_every_boundary() {
    let (family, base) = fixture();
    for cap in 0..=8 {
        let limits = MultiAffineNumeratorExpansionLimits {
            max_retained_coefficient_terms: cap,
            ..Default::default()
        };
        assert_eq!(
            visited(&family, &base, &[], limits),
            materialized(&family, &base, &[], limits)
        );
    }
    let one = family.coefficient_context().one();
    let threshold = coefficient_weight(&one).unwrap().clone_owned_bytes * 2;
    for cap in [threshold - 1, threshold, threshold + 1] {
        let limits = MultiAffineNumeratorExpansionLimits {
            max_retained_coefficient_clone_owned_bytes: cap,
            ..Default::default()
        };
        assert_eq!(
            visited(&family, &base, &[], limits),
            materialized(&family, &base, &[], limits)
        );
    }
}

#[test]
fn trace_support_rejects_malformed_native_layout_order_and_scalar() {
    let (family, base) = fixture();
    let factors = [factor(&family, "1", &[(0, "1")], 1)];
    for mutation in 0..4 {
        let mut expanded = expand_native(&family, &base, &factors, Default::default(), |_| Ok(()))
            .unwrap()
            .unwrap();
        match mutation {
            0 => {
                expanded.polynomial.exponents.pop();
            }
            1 => {
                let first = expanded.polynomial.exponents(0).to_vec();
                expanded.polynomial.exponents_mut(1).copy_from_slice(&first);
            }
            2 => expanded.polynomial.coefficients[0] = Rational::zero(),
            _ => {
                let a = expanded.polynomial.exponents(0).to_vec();
                let b = expanded.polynomial.exponents(1).to_vec();
                expanded.polynomial.exponents_mut(0).copy_from_slice(&b);
                expanded.polynomial.exponents_mut(1).copy_from_slice(&a);
            }
        }
        assert!(admit_support(&expanded, &base, Default::default()).is_err());
    }
    let expanded = expand_native(&family, &base, &factors, Default::default(), |_| Ok(()))
        .unwrap()
        .unwrap();
    let bad_base = IntegralKey::try_new([i64::MIN, 0, 0, 0, 0, 0]).unwrap();
    assert!(matches!(
        admit_support(&expanded, &bad_base, Default::default()),
        Err(MultiAffineNumeratorExpansionError::PowerShiftUnderflow { .. })
    ));
}

#[test]
fn trace_support_consumer_can_cancel_without_visiting_remaining_keys() {
    let (family, base) = fixture();
    let factors = [factor(&family, "1", &[(0, "1"), (5, "-1")], 4)];
    let mut keys = try_expand_multi_affine_support_with_usage(
        &family,
        &base,
        &factors,
        Default::default(),
        |_| Ok(()),
    )
    .unwrap();
    let total = keys.size_hint().1.unwrap();
    let mut visited = 0;
    let result = keys.try_for_each(|key| {
        key.unwrap();
        visited += 1;
        Err::<(), _>("cancelled")
    });
    assert_eq!(result, Err("cancelled"));
    assert_eq!(visited, 1);
    assert_eq!(keys.size_hint().1, Some(total - 1));
    drop(keys);
}
