use std::collections::BTreeMap;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{IntegralFamily, IntegralKey};

use super::{
    MultiAffineNumeratorExpansionError, MultiAffineNumeratorExpansionLimits,
    MultiAffineNumeratorFactor, try_expand_multi_affine_numerator,
};

const BASE: [i64; 6] = [7, 11, 13, 17, 19, 23];

fn family() -> IntegralFamily {
    crate::foundry::artifact::canonical_three_loop_family().unwrap()
}

fn zero_row(family: &IntegralFamily) -> Vec<Coefficient> {
    (0..family.denominator_count())
        .map(|_| family.coefficient_context().zero())
        .collect()
}

fn rational(family: &IntegralFamily, numerator: i64, denominator: i64) -> Coefficient {
    let context = family.coefficient_context();
    context
        .try_div(
            &context.integer(numerator),
            &context.integer(denominator),
            family.construction_limits().exact_algebra,
        )
        .unwrap()
}

fn s5_factor(family: &IntegralFamily, power: u64) -> MultiAffineNumeratorFactor {
    // The exact S5 matcher relation is
    // q1.q3 = (1 + D2 + D3 - D6)/2.
    let half = rational(family, 1, 2);
    let minus_half = rational(family, -1, 2);
    let context = family.coefficient_context();
    MultiAffineNumeratorFactor::try_new(
        half.clone(),
        [
            context.zero(),
            half.clone(),
            half,
            context.zero(),
            context.zero(),
            minus_half,
        ],
        power,
    )
    .unwrap()
}

fn endpoint_map(
    endpoints: &[super::MultiAffineNumeratorEndpoint],
) -> BTreeMap<IntegralKey, Coefficient> {
    endpoints
        .iter()
        .map(|endpoint| (endpoint.key().clone(), endpoint.coefficient().clone()))
        .collect()
}

fn lower(exponents: [u64; 6]) -> IntegralKey {
    IntegralKey::try_new(
        BASE.into_iter()
            .zip(exponents)
            .map(|(base, exponent)| base - i64::try_from(exponent).unwrap()),
    )
    .unwrap()
}

#[test]
fn exact_s5_relation_expands_degrees_zero_one_two_and_four() {
    let family = family();
    let context = family.coefficient_context();
    let base = IntegralKey::try_new(BASE).unwrap();

    let degree_zero = try_expand_multi_affine_numerator(
        &family,
        &base,
        &[s5_factor(&family, 0)],
        MultiAffineNumeratorExpansionLimits::default(),
    )
    .unwrap();
    assert_eq!(degree_zero.len(), 1);
    assert_eq!(degree_zero[0].key(), &base);
    assert_eq!(degree_zero[0].coefficient(), &context.one());

    let degree_one = endpoint_map(
        &try_expand_multi_affine_numerator(
            &family,
            &base,
            &[s5_factor(&family, 1)],
            MultiAffineNumeratorExpansionLimits::default(),
        )
        .unwrap(),
    );
    assert_eq!(degree_one.len(), 4);
    for (exponents, coefficient) in [
        ([0, 0, 0, 0, 0, 0], rational(&family, 1, 2)),
        ([0, 1, 0, 0, 0, 0], rational(&family, 1, 2)),
        ([0, 0, 1, 0, 0, 0], rational(&family, 1, 2)),
        ([0, 0, 0, 0, 0, 1], rational(&family, -1, 2)),
    ] {
        assert_eq!(degree_one.get(&lower(exponents)), Some(&coefficient));
    }

    let degree_two = endpoint_map(
        &try_expand_multi_affine_numerator(
            &family,
            &base,
            &[s5_factor(&family, 2)],
            MultiAffineNumeratorExpansionLimits::default(),
        )
        .unwrap(),
    );
    assert_eq!(degree_two.len(), 10);
    for (exponents, numerator) in [
        ([0, 0, 0, 0, 0, 0], 1),
        ([0, 2, 0, 0, 0, 0], 1),
        ([0, 0, 2, 0, 0, 0], 1),
        ([0, 0, 0, 0, 0, 2], 1),
        ([0, 1, 0, 0, 0, 0], 2),
        ([0, 0, 1, 0, 0, 0], 2),
        ([0, 0, 0, 0, 0, 1], -2),
        ([0, 1, 1, 0, 0, 0], 2),
        ([0, 1, 0, 0, 0, 1], -2),
        ([0, 0, 1, 0, 0, 1], -2),
    ] {
        assert_eq!(
            degree_two.get(&lower(exponents)),
            Some(&rational(&family, numerator, 4))
        );
    }

    let degree_four = endpoint_map(
        &try_expand_multi_affine_numerator(
            &family,
            &base,
            &[s5_factor(&family, 4)],
            MultiAffineNumeratorExpansionLimits::default(),
        )
        .unwrap(),
    );
    assert_eq!(degree_four.len(), 35);
    for (exponents, numerator) in [
        ([0, 0, 0, 0, 0, 0], 1),
        ([0, 4, 0, 0, 0, 0], 1),
        ([0, 0, 0, 0, 0, 4], 1),
        ([0, 1, 1, 0, 0, 2], 12),
        ([0, 1, 0, 0, 0, 3], -4),
    ] {
        assert_eq!(
            degree_four.get(&lower(exponents)),
            Some(&rational(&family, numerator, 16))
        );
    }
}

#[test]
fn multiple_factors_coalesce_collisions_and_cancel_exactly() {
    let family = family();
    let context = family.coefficient_context();
    let base = IntegralKey::try_new(BASE).unwrap();
    let mut plus = zero_row(&family);
    plus[0] = context.one();
    let mut minus = zero_row(&family);
    minus[0] = context.integer(-1);
    let factors = [
        MultiAffineNumeratorFactor::try_new(context.one(), plus, 1).unwrap(),
        MultiAffineNumeratorFactor::try_new(context.one(), minus, 1).unwrap(),
    ];
    let first = try_expand_multi_affine_numerator(
        &family,
        &base,
        &factors,
        MultiAffineNumeratorExpansionLimits::default(),
    )
    .unwrap();
    let second = try_expand_multi_affine_numerator(
        &family,
        &base,
        &factors,
        MultiAffineNumeratorExpansionLimits::default(),
    )
    .unwrap();
    assert_eq!(first, second);
    let endpoints = endpoint_map(&first);
    assert_eq!(endpoints.len(), 2);
    assert_eq!(endpoints.get(&lower([0; 6])), Some(&context.one()));
    assert_eq!(
        endpoints.get(&lower([2, 0, 0, 0, 0, 0])),
        Some(&context.integer(-1))
    );
    assert!(!endpoints.contains_key(&lower([1, 0, 0, 0, 0, 0])));
}

#[test]
fn zero_factors_and_a_positive_power_zero_factor_have_exact_semantics() {
    let family = family();
    let context = family.coefficient_context();
    let base = IntegralKey::try_new(BASE).unwrap();
    let identity = try_expand_multi_affine_numerator(
        &family,
        &base,
        &[],
        MultiAffineNumeratorExpansionLimits::default(),
    )
    .unwrap();
    assert_eq!(identity.len(), 1);
    assert_eq!(identity[0].key(), &base);
    assert_eq!(identity[0].coefficient(), &context.one());

    let zero = MultiAffineNumeratorFactor::try_new(context.zero(), zero_row(&family), 3).unwrap();
    assert!(
        try_expand_multi_affine_numerator(
            &family,
            &base,
            &[zero],
            MultiAffineNumeratorExpansionLimits::default(),
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn parameter_dependent_coefficients_are_rejected_before_native_arithmetic() {
    let family = family();
    let context = family.coefficient_context();
    let d = context.parameter("d").unwrap();
    let mut row = zero_row(&family);
    row[0] = context.one();
    let factor = MultiAffineNumeratorFactor::try_new(d.clone(), row, 2).unwrap();
    assert_eq!(
        try_expand_multi_affine_numerator(
            &family,
            &IntegralKey::try_new(BASE).unwrap(),
            &[factor],
            MultiAffineNumeratorExpansionLimits::default(),
        ),
        Err(
            MultiAffineNumeratorExpansionError::NonconstantExpansionCoefficient {
                factor: 0,
                coefficient: 0,
            }
        )
    );
}

#[test]
fn arity_and_foreign_context_are_rejected_before_native_arithmetic() {
    let family = family();
    let context = family.coefficient_context();
    let base = IntegralKey::try_new(BASE).unwrap();
    let wrong_arity =
        MultiAffineNumeratorFactor::try_new(context.one(), (0..5).map(|_| context.zero()), 1)
            .unwrap();
    assert_eq!(
        try_expand_multi_affine_numerator(
            &family,
            &base,
            &[wrong_arity],
            MultiAffineNumeratorExpansionLimits::default(),
        ),
        Err(MultiAffineNumeratorExpansionError::WrongRelationArity {
            factor: 0,
            expected: 6,
            actual: 5,
        })
    );
    assert_eq!(
        try_expand_multi_affine_numerator(
            &family,
            &IntegralKey::try_new([1, 2]).unwrap(),
            &[],
            MultiAffineNumeratorExpansionLimits::default(),
        ),
        Err(MultiAffineNumeratorExpansionError::WrongBaseArity {
            expected: 6,
            actual: 2,
        })
    );

    let foreign = CoefficientContext::new(["x"]);
    let factor =
        MultiAffineNumeratorFactor::try_new(foreign.parameter("x").unwrap(), zero_row(&family), 1)
            .unwrap();
    assert!(matches!(
        try_expand_multi_affine_numerator(
            &family,
            &base,
            &[factor],
            MultiAffineNumeratorExpansionLimits::default(),
        ),
        Err(MultiAffineNumeratorExpansionError::ExactAlgebra(_))
    ));
}

#[test]
fn exponent_and_key_overflow_have_typed_failures() {
    let family = family();
    let context = family.coefficient_context();
    let mut row = zero_row(&family);
    row[0] = context.one();
    let underflow = MultiAffineNumeratorFactor::try_new(context.zero(), row.clone(), 1).unwrap();
    let mut base = BASE;
    base[0] = i64::MIN;
    assert_eq!(
        try_expand_multi_affine_numerator(
            &family,
            &IntegralKey::try_new(base).unwrap(),
            &[underflow],
            MultiAffineNumeratorExpansionLimits::default(),
        ),
        Err(MultiAffineNumeratorExpansionError::PowerShiftUnderflow {
            position: 0,
            power: i64::MIN,
            decrement: 1,
        })
    );

    let too_wide = u64::from(i32::MAX as u32) + 1;
    let native_overflow =
        MultiAffineNumeratorFactor::try_new(context.zero(), row.clone(), too_wide).unwrap();
    assert_eq!(
        try_expand_multi_affine_numerator(
            &family,
            &IntegralKey::try_new(BASE).unwrap(),
            &[native_overflow],
            MultiAffineNumeratorExpansionLimits {
                max_total_power: u64::MAX,
                ..MultiAffineNumeratorExpansionLimits::default()
            },
        ),
        Err(MultiAffineNumeratorExpansionError::NativeExponentLimit {
            factor: 0,
            requested: too_wide,
            limit: i32::MAX as u32,
        })
    );

    let aggregate = [
        MultiAffineNumeratorFactor::try_new(context.zero(), row.clone(), i32::MAX as u64).unwrap(),
        MultiAffineNumeratorFactor::try_new(context.zero(), row.clone(), 1).unwrap(),
    ];
    assert_eq!(
        try_expand_multi_affine_numerator(
            &family,
            &IntegralKey::try_new(BASE).unwrap(),
            &aggregate,
            MultiAffineNumeratorExpansionLimits {
                max_total_power: u64::MAX,
                max_native_polynomial_operations: usize::MAX,
                ..MultiAffineNumeratorExpansionLimits::default()
            },
        ),
        Err(MultiAffineNumeratorExpansionError::NativeExponentDegreeOverflow { position: 0 })
    );

    let factors = [
        MultiAffineNumeratorFactor::try_new(context.zero(), row.clone(), u64::MAX).unwrap(),
        MultiAffineNumeratorFactor::try_new(context.zero(), row, 1).unwrap(),
    ];
    assert_eq!(
        try_expand_multi_affine_numerator(
            &family,
            &IntegralKey::try_new(BASE).unwrap(),
            &factors,
            MultiAffineNumeratorExpansionLimits {
                max_total_power: u64::MAX,
                ..MultiAffineNumeratorExpansionLimits::default()
            },
        ),
        Err(MultiAffineNumeratorExpansionError::ResourceCountOverflow {
            resource: "multi-affine total power",
        })
    );
}

#[test]
fn caller_limits_reject_support_before_symbolica_expansion() {
    let family = family();
    let base = IntegralKey::try_new(BASE).unwrap();
    let error = try_expand_multi_affine_numerator(
        &family,
        &base,
        &[s5_factor(&family, 2)],
        MultiAffineNumeratorExpansionLimits {
            max_native_polynomial_terms: 9,
            ..MultiAffineNumeratorExpansionLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        error,
        MultiAffineNumeratorExpansionError::ResourceLimit {
            resource: "multi-affine factor support",
            requested: 10,
            limit: 9,
        }
    );

    let error = try_expand_multi_affine_numerator(
        &family,
        &base,
        &[s5_factor(&family, 1)],
        MultiAffineNumeratorExpansionLimits {
            max_factors: 0,
            ..MultiAffineNumeratorExpansionLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        error,
        MultiAffineNumeratorExpansionError::ResourceLimit {
            resource: "multi-affine factors",
            requested: 1,
            limit: 0,
        }
    );

    let error = try_expand_multi_affine_numerator(
        &family,
        &base,
        &[s5_factor(&family, 1)],
        MultiAffineNumeratorExpansionLimits {
            max_endpoints: 3,
            ..MultiAffineNumeratorExpansionLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        error,
        MultiAffineNumeratorExpansionError::ResourceLimit {
            resource: "multi-affine endpoints",
            requested: 4,
            limit: 3,
        }
    );
}

#[test]
fn retained_budget_counts_native_polynomial_and_output_clone_simultaneously() {
    let family = family();
    let base = IntegralKey::try_new(BASE).unwrap();
    assert_eq!(
        try_expand_multi_affine_numerator(
            &family,
            &base,
            &[],
            MultiAffineNumeratorExpansionLimits {
                // The native identity coefficient and its output clone each
                // retain one numerator and one denominator term.
                max_retained_coefficient_terms: 3,
                ..MultiAffineNumeratorExpansionLimits::default()
            },
        ),
        Err(MultiAffineNumeratorExpansionError::ResourceLimit {
            resource: "multi-affine retained coefficient terms",
            requested: 4,
            limit: 3,
        })
    );
}
