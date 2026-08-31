use crate::algebra::{CoefficientContext, ExactAlgebraLimits};

use super::super::{IndexedAlgebraError, IndexedAlgebraLimits, IndexedCoefficientContext};

#[test]
fn lift_translate_and_specialize_preserve_authenticated_maps() {
    let base = CoefficientContext::new(["d", "m2"]);
    let context = IndexedCoefficientContext::try_new(&base, "translation", 2).unwrap();
    let d = base.parameter("d").unwrap();
    let m2 = base.parameter("m2").unwrap();
    let family_value = &(&d + &base.integer(1)) / &m2;
    let lifted = context.lift(&family_value).unwrap();
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let index_product = context.mul(&n0, &n1).unwrap();
    let value = context.mul(&index_product, &lifted).unwrap();
    let translated = context
        .translate(&value, &[2, -3], IndexedAlgebraLimits::default())
        .unwrap();
    let shifted_n0 = context.add(&n0, &context.integer(2)).unwrap();
    let shifted_n1 = context.sub(&n1, &context.integer(3)).unwrap();
    let expected_index_product = context.mul(&shifted_n0, &shifted_n1).unwrap();
    let expected_indexed = context.mul(&expected_index_product, &lifted).unwrap();
    assert_eq!(translated, expected_indexed);
    assert_eq!(
        translated.raw().numerator.variables.as_ref(),
        context.variables.as_ref()
    );
    assert_eq!(
        translated.raw().denominator.variables.as_ref(),
        context.variables.as_ref()
    );

    let (specialized, denominator_nonzero) = context
        .specialize(&translated, &[5, 100], IndexedAlgebraLimits::default())
        .unwrap();
    let expected = &base.integer(679) * &family_value;
    assert_eq!(specialized, expected);
    assert_eq!(denominator_nonzero.unwrap(), m2.numerator);
}

#[test]
fn absent_index_shift_is_an_exact_noop_for_coefficients_and_polynomials() {
    let base = CoefficientContext::new(Vec::<String>::new());
    let context = IndexedCoefficientContext::try_new(&base, "absent-index-translation", 2).unwrap();
    let n0 = context.index(0).unwrap();
    let n0_squared = context.mul(&n0, &n0).unwrap();
    let exact = IndexedAlgebraLimits {
        exact_algebra: ExactAlgebraLimits {
            max_polynomial_terms: 1,
            ..ExactAlgebraLimits::default()
        },
        max_specialization_power_operations: 0,
        max_specialization_integer_bits: 1,
    };
    let polynomial = context
        .numerator_condition_with_limits(&n0_squared, exact.exact_algebra)
        .unwrap();

    let translated = context
        .translate(&n0_squared, &[0, i64::MIN], exact)
        .unwrap();
    let translated_polynomial = context
        .translate_polynomial(&polynomial, &[0, i64::MIN], exact)
        .unwrap();

    assert_eq!(translated, n0_squared);
    assert_eq!(translated_polynomial, polynomial);
    assert_eq!(
        translated_polynomial.raw().variables.as_ref(),
        context.variables.as_ref()
    );
}

#[test]
fn translation_limits_accept_exact_boundaries_and_reject_one_below() {
    let base = CoefficientContext::new(Vec::<String>::new());
    let context = IndexedCoefficientContext::try_new(&base, "translation-limits", 1).unwrap();
    let n = context.index(0).unwrap();
    let n_squared = context.mul(&n, &n).unwrap();
    let exact = IndexedAlgebraLimits {
        exact_algebra: ExactAlgebraLimits {
            max_polynomial_terms: 3,
            ..ExactAlgebraLimits::default()
        },
        max_specialization_power_operations: 1,
        max_specialization_integer_bits: 9,
    };

    context.translate(&n_squared, &[2], exact).unwrap();

    let mut one_below = exact;
    one_below.exact_algebra.max_polynomial_terms = 2;
    assert!(matches!(
        context.translate(&n_squared, &[2], one_below),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "parametric translation output terms",
            requested: 3,
            limit: 2,
        })
    ));

    let mut one_below = exact;
    one_below.max_specialization_power_operations = 0;
    assert!(matches!(
        context.translate(&n_squared, &[2], one_below),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "parametric translation power operations",
            requested: 1,
            limit: 0,
        })
    ));

    let mut one_below = exact;
    one_below.max_specialization_integer_bits = 8;
    assert!(matches!(
        context.translate(&n_squared, &[2], one_below),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "parametric translation integer bits",
            requested: 9,
            limit: 8,
        })
    ));
}

#[test]
fn translation_bounds_i64_min_without_signed_overflow() {
    let base = CoefficientContext::new(Vec::<String>::new());
    let context = IndexedCoefficientContext::try_new(&base, "i64-min-translation", 1).unwrap();
    let index = context.index(0).unwrap();
    let exact = IndexedAlgebraLimits {
        exact_algebra: ExactAlgebraLimits {
            max_polynomial_terms: 2,
            ..ExactAlgebraLimits::default()
        },
        max_specialization_power_operations: 1,
        max_specialization_integer_bits: 67,
    };
    let mut one_below = exact;
    one_below.max_specialization_integer_bits -= 1;
    assert!(matches!(
        context.translate(&index, &[i64::MIN], one_below),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "parametric translation integer bits",
            requested: 67,
            limit: 66,
        })
    ));

    let translated = context.translate(&index, &[i64::MIN], exact).unwrap();
    let (value, denominator_nonzero) = context
        .specialize(&translated, &[0], IndexedAlgebraLimits::default())
        .unwrap();
    assert!(denominator_nonzero.is_none());
    assert_eq!(value, base.integer(i64::MIN));
}
