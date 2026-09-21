//! Independent native-field equivalence and conservative ownership checks.

use std::cell::Cell;
use std::collections::BTreeMap;

use symbolica::prelude::{Integer, IntegerRing, RationalPolynomialField, Z};

use super::*;

#[test]
fn native_heap_pow_many_variable_square_matches_native_multiplication() {
    // Public native regression: the old u32 radix accumulator wrapped here
    // (3^21 > 2^32), despite a result with only 253 exact terms.
    let variables = Arc::new((0..22).map(PolyVariable::Temporary).collect());
    let mut affine = MultivariatePolynomial::<_, u32>::new(&Q, None, variables);
    for axis in 0..22 {
        let mut exponents = vec![0; 22];
        exponents[axis] = 1;
        affine.append_monomial(Rational::one(), &exponents);
    }
    let expected = &affine * &affine;
    assert_eq!(expected.nterms(), 253);
    assert_eq!(affine.pow(2), expected);
}

#[test]
fn native_heap_pow_rational_affine_nonuniform_degree_and_zero_gaps_match_multiplication() {
    let variables = Arc::new((0..26).map(PolyVariable::Temporary).collect());
    let mut affine = MultivariatePolynomial::<_, u32>::new(&Q, None, variables);
    affine.append_monomial(Rational::from((-3, 7)), &vec![0; 26]);
    // Zero-degree gaps give radix one; alternating degree one/two gives
    // nonuniform radices. Every degree is <=2 so native pow takes heap_pow.
    for axis in (0..26).filter(|axis| axis % 5 != 2) {
        let mut exponents = vec![0; 26];
        exponents[axis] = if axis % 2 == 0 { 1 } else { 2 };
        affine.append_monomial(Rational::from((axis as i64 + 1, 11)), &exponents);
    }
    let expected = &affine * &affine;
    assert_eq!(affine.pow(2), expected);
    assert!(expected.nterms() < 300);
}

fn family() -> IntegralFamily {
    crate::foundry::artifact::canonical_three_loop_family().unwrap()
}

fn factor(
    family: &IntegralFamily,
    constant: &str,
    entries: &[(usize, &str)],
    power: u64,
) -> MultiAffineNumeratorFactor {
    let context = family.coefficient_context();
    let mut row = vec![context.zero(); family.denominator_count()];
    for &(axis, expression) in entries {
        row[axis] = context.coefficient_fixture(expression);
    }
    MultiAffineNumeratorFactor::try_new(context.coefficient_fixture(constant), row, power).unwrap()
}

/// Small test-only oracle using the PREVIOUS native Symbolica coefficient
/// field. Symbolica still owns every expansion/coalescing operation here.
fn legacy_native_reference(
    family: &IntegralFamily,
    base: &IntegralKey,
    factors: &[MultiAffineNumeratorFactor],
) -> BTreeMap<IntegralKey, Coefficient> {
    type Legacy = MultivariatePolynomial<RationalPolynomialField<IntegerRing, u16>, u32>;
    let arity = family.denominator_count();
    let variables = Arc::new((0..arity).map(PolyVariable::Temporary).collect());
    let template = Legacy::new(&RationalPolynomialField::new(Z), None, variables);
    let mut polynomial = template.constant(family.coefficient_context().one());
    for factor in factors {
        let mut affine = template.constant(factor.constant().clone());
        for (axis, coefficient) in factor.denominator_coefficients().iter().enumerate() {
            let mut exponents = vec![0; arity];
            exponents[axis] = 1;
            affine = &affine + &template.monomial(coefficient.clone(), exponents);
        }
        polynomial = &polynomial * &affine.pow(usize::try_from(factor.power()).unwrap());
    }
    polynomial
        .coefficients
        .iter()
        .zip(polynomial.exponents_iter())
        .map(|(coefficient, exponents)| {
            let key = IntegralKey::try_new(
                base.powers()
                    .iter()
                    .zip(exponents)
                    .map(|(&n, &e)| n - i64::from(e)),
            )
            .unwrap();
            (key, coefficient.clone())
        })
        .collect()
}

#[test]
fn native_q_matches_legacy_native_field_and_exact_context() {
    let family = family();
    let context = family.coefficient_context();
    let base =
        IntegralKey::try_new((0..family.denominator_count()).map(|axis| 11 + axis as i64)).unwrap();
    for power in [0, 1, 2, 4, 7] {
        let factors = [
            factor(&family, "-7/11", &[(0, "2/3"), (1, "-5/13")], power),
            factor(&family, "3/7", &[(0, "-2/3"), (2, "1/19")], 2),
        ];
        let actual =
            try_expand_multi_affine_numerator(&family, &base, &factors, Default::default())
                .unwrap();
        let expected = legacy_native_reference(&family, &base, &factors);
        assert_eq!(actual.len(), expected.len());
        for endpoint in &actual {
            assert_eq!(Some(endpoint.coefficient()), expected.get(endpoint.key()));
            context
                .validate_with_limits(endpoint.coefficient(), Default::default())
                .unwrap();
            assert!(Arc::ptr_eq(
                endpoint.coefficient().get_variables(),
                context.variables()
            ));
        }
        assert!(actual.windows(2).all(|pair| pair[0].key() < pair[1].key()));
    }
}

#[test]
fn native_large_rational_coefficients_and_cancellation_are_exact() {
    let family = family();
    let base = IntegralKey::try_new(vec![5; family.denominator_count()]).unwrap();
    for factors in [
        vec![factor(
            &family,
            "(2^180+13)/(3^100+1)",
            &[(0, "-(2^150+1)/(5^75+2)")],
            3,
        )],
        vec![
            factor(&family, "1/3", &[(0, "2/7")], 3),
            factor(&family, "1/3", &[(0, "-2/7")], 3),
        ],
    ] {
        let actual =
            try_expand_multi_affine_numerator(&family, &base, &factors, Default::default())
                .unwrap();
        let expected = legacy_native_reference(&family, &base, &factors);
        assert_eq!(actual.len(), expected.len());
        for endpoint in &actual {
            assert_eq!(Some(endpoint.coefficient()), expected.get(endpoint.key()));
        }
    }
}

#[test]
fn authenticated_unreduced_negative_constant_normalizes_without_map_loss() {
    let family = family();
    let context = family.coefficient_context();
    // Exact structural authentication does not require pre-cancelled integer
    // content. Native Q, not RustRed arithmetic, owns this normalization.
    let constant = Coefficient {
        numerator: context.integer(2).numerator,
        denominator: context.integer(-4).numerator,
    };
    context
        .validate_with_limits(&constant, Default::default())
        .unwrap();
    let factors = [MultiAffineNumeratorFactor::try_new(
        constant,
        vec![context.zero(); family.denominator_count()],
        1,
    )
    .unwrap()];
    let base = IntegralKey::try_new(vec![1; family.denominator_count()]).unwrap();
    let actual =
        try_expand_multi_affine_numerator(&family, &base, &factors, Default::default()).unwrap();
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].key(), &base);
    assert_eq!(
        actual[0].coefficient(),
        &context.coefficient_fixture("-1/2")
    );
    let expected = legacy_native_reference(&family, &base, &factors);
    assert!(
        context
            .try_sub(
                actual[0].coefficient(),
                &expected[&base],
                Default::default()
            )
            .unwrap()
            .is_zero()
    );
    assert!(Arc::ptr_eq(
        actual[0].coefficient().get_variables(),
        context.variables()
    ));
}

#[test]
fn virtual_wrapper_bounds_actual_native_large_output_ownership() {
    for names in [vec![], vec!["d"], vec!["d", "s", "m", "t"]] {
        let context = CoefficientContext::new(names);
        let fixed = coefficient_weight(&context.one())
            .unwrap()
            .clone_owned_bytes
            .max(size_of::<Rational>());
        let big = Integer::from(2).pow(180) + Integer::from(7);
        let denominator = Integer::from(3).pow(140) + Integer::from(2);
        for value in [
            Rational::zero(),
            Rational::one(),
            Rational::from((-7, 11)),
            Rational::from((big.clone(), denominator.clone())),
            Rational::from((-big, denominator)),
        ] {
            let prospective = rational_weight(&value, fixed).unwrap();
            let output = contextual_coefficient(&context, &value);
            context
                .validate_with_limits(&output, Default::default())
                .unwrap();
            let actual = coefficient_weight(&output).unwrap();
            assert!(actual.terms <= prospective.terms);
            assert!(actual.clone_owned_bytes <= prospective.clone_owned_bytes);
            assert!(Arc::ptr_eq(output.get_variables(), context.variables()));
            if value.numerator_ref().significant_bits() > 128 {
                assert!(prospective.clone_owned_bytes > fixed);
            }
        }
    }
}

#[test]
fn output_copy_bound_survives_warmed_large_native_integer_cache() {
    use symbolica::domains::integer::{MultiPrecisionInteger, RawMultiPrecisionInteger};

    // Hold pre-existing cached allocations out of circulation before creating
    // the small source value, so it cannot already own a poisoned large buffer.
    let held: Vec<_> = (0..64).map(|_| MultiPrecisionInteger::default()).collect();
    let context = CoefficientContext::new(["d", "s"]);
    let value = Rational::from((
        Integer::from(2).pow(180) + Integer::from(7),
        Integer::from(3).pow(140) + Integer::from(2),
    ));
    let fixed = coefficient_weight(&context.one())
        .unwrap()
        .clone_owned_bytes
        .max(size_of::<Rational>());
    let prospective = rational_weight(&value, fixed).unwrap();

    // Drain any previous pooled allocations, then return much larger native
    // buffers. These are test-only native allocation operations, not a
    // production dependency on the private cache size or its capacity cap.
    let arithmetic_scratch: Vec<_> = (0..64).map(|_| MultiPrecisionInteger::default()).collect();
    let poison: Vec<_> = (0..64)
        .map(|_| {
            let mut raw = RawMultiPrecisionInteger::from(1);
            raw <<= 16_384;
            MultiPrecisionInteger::from_raw(raw)
        })
        .collect();
    drop(poison);

    let Integer::Large(source) = value.numerator_ref() else {
        panic!("fixture numerator must use the native large representation");
    };
    let cached_copy = value.numerator();
    let Integer::Large(cached) = &cached_copy else {
        panic!("fixture clone must use the native large representation");
    };
    assert!(cached.as_raw().capacity() > source.as_raw().capacity());
    drop(cached_copy);

    let output = contextual_coefficient(&context, &value);
    let actual = coefficient_weight(&output).unwrap();
    assert!(actual.terms <= prospective.terms);
    assert!(actual.clone_owned_bytes <= prospective.clone_owned_bytes);
    context
        .validate_with_limits(&output, Default::default())
        .unwrap();
    assert_eq!(output.numerator.get_constant(), value.numerator());
    assert_eq!(output.denominator.get_constant(), value.denominator());
    assert!(Arc::ptr_eq(output.get_variables(), context.variables()));
    drop(arithmetic_scratch);
    drop(held);
}

#[test]
fn output_clone_budget_remains_prospective_and_simultaneous() {
    let family = family();
    let base = IntegralKey::try_new(vec![1; family.denominator_count()]).unwrap();
    let fixed = coefficient_weight(&family.coefficient_context().one())
        .unwrap()
        .clone_owned_bytes
        .max(size_of::<Rational>());
    let limits = MultiAffineNumeratorExpansionLimits {
        max_retained_coefficient_clone_owned_bytes: 2 * fixed - 1,
        ..Default::default()
    };
    assert_eq!(
        try_expand_multi_affine_numerator(&family, &base, &[], limits),
        Err(MultiAffineNumeratorExpansionError::ResourceLimit {
            resource: "multi-affine retained coefficient clone-owned bytes",
            requested: 2 * fixed,
            limit: 2 * fixed - 1,
        }),
    );
    assert!(
        try_expand_multi_affine_numerator(
            &family,
            &base,
            &[],
            MultiAffineNumeratorExpansionLimits {
                max_retained_coefficient_clone_owned_bytes: 2 * fixed,
                ..limits
            }
        )
        .is_ok()
    );
}

#[test]
fn reservation_callback_keeps_conservative_work_and_can_reject_before_native_work() {
    let family = family();
    let base = IntegralKey::try_new(vec![5; family.denominator_count()]).unwrap();
    let factors = [
        factor(&family, "1", &[(0, "1"), (1, "1")], 2),
        factor(&family, "1", &[(0, "-1")], 1),
    ];
    let usage = Cell::new(None);
    let actual = try_expand_multi_affine_numerator_with_usage(
        &family,
        &base,
        &factors,
        Default::default(),
        |used| {
            assert!(
                usage
                    .replace(Some((used.operations, used.endpoints)))
                    .is_none()
            );
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(usage.get(), Some((58, 12)));
    assert!(actual.len() <= 12);
    let rejected = Cell::new(false);
    assert_eq!(
        try_expand_multi_affine_numerator_with_usage(
            &family,
            &base,
            &factors,
            Default::default(),
            |_| {
                rejected.set(true);
                Err(MultiAffineNumeratorExpansionError::Invariant {
                    detail: "test reservation rejection",
                })
            }
        ),
        Err(MultiAffineNumeratorExpansionError::Invariant {
            detail: "test reservation rejection"
        }),
    );
    assert!(rejected.get());
}

#[test]
fn native_q_does_not_bypass_zero_inner_term_limits_or_foreign_constants() {
    let family = family();
    let base = IntegralKey::try_new(vec![1; family.denominator_count()]).unwrap();
    let mut limits = MultiAffineNumeratorExpansionLimits::default();
    limits.exact_algebra.max_polynomial_terms = 0;
    assert!(matches!(
        try_expand_multi_affine_numerator(&family, &base, &[], limits),
        Err(MultiAffineNumeratorExpansionError::ExactAlgebra(_))
    ));
    let foreign = CoefficientContext::new(["foreign"]);
    for power in [0, 2] {
        let factor = MultiAffineNumeratorFactor::try_new(
            foreign.one(),
            vec![family.coefficient_context().zero(); family.denominator_count()],
            power,
        )
        .unwrap();
        assert!(matches!(
            try_expand_multi_affine_numerator(&family, &base, &[factor], Default::default()),
            Err(MultiAffineNumeratorExpansionError::ExactAlgebra(_))
        ));
    }
}
