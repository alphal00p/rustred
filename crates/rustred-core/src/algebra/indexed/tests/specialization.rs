use symbolica::prelude::RationalPolynomial;

use crate::algebra::{CoefficientContext, ExactAlgebraLimits};

use super::super::limits::integer_magnitude_bits;
use super::super::{
    IndexedAlgebraError, IndexedAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext,
    IndexedPolynomial,
};

#[test]
fn polynomial_specialization_preserves_the_pre_cancellation_condition() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "polynomial-condition", 1).unwrap();
    let n = context.index(0).unwrap();
    let x = context.lift(&base.parameter("x").unwrap()).unwrap();
    let condition = context.add(&n, &x).unwrap();
    let condition = IndexedPolynomial {
        raw: condition.raw.numerator,
        context: context.fingerprint.clone(),
    };

    let specialized = context
        .specialize_polynomial(&condition, &[2], IndexedAlgebraLimits::default())
        .unwrap();
    assert_eq!(
        specialized,
        (&base.parameter("x").unwrap() + &base.integer(2)).numerator
    );
}

#[test]
fn zero_and_constant_specializations_rebind_the_exact_base_map() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "constant-maps", 1).unwrap();
    let index = context.index(0).unwrap();
    let values = [context.sub(&index, &index).unwrap(), context.one()];

    for value in values {
        let (specialized, denominator_nonzero) = context
            .specialize(&value, &[7], IndexedAlgebraLimits::default())
            .unwrap();
        assert!(denominator_nonzero.is_none());
        assert_eq!(
            specialized.numerator.variables.as_ref(),
            base.variables().as_ref()
        );
        assert_eq!(
            specialized.denominator.variables.as_ref(),
            base.variables().as_ref()
        );
    }
}

#[test]
fn specialization_retains_a_cancelled_index_dependent_pole() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "cancelled-pole", 1).unwrap();
    let n = context.index(0).unwrap();
    let one = context.one();
    let n_minus_one = context.sub(&n, &one).unwrap();
    let fabricated = IndexedCoefficient {
        raw: RationalPolynomial {
            numerator: n_minus_one.raw.numerator.clone(),
            denominator: n_minus_one.raw.numerator.clone(),
        },
        context: context.fingerprint.clone(),
    };
    let generic = context
        .specialize(&fabricated, &[2], IndexedAlgebraLimits::default())
        .unwrap();
    assert_eq!(generic.0, base.one());
    assert!(
        generic.1.is_none(),
        "constant nonzero conditions are tautologies"
    );
    assert!(matches!(
        context.specialize(&fabricated, &[1], IndexedAlgebraLimits::default(),),
        Err(IndexedAlgebraError::ZeroDenominator)
    ));
}

#[test]
fn specialization_retains_a_nonconstant_denominator_cancelled_by_normalization() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "cancelled-base-pole", 1).unwrap();
    let n = context.index(0).unwrap();
    let x = context.lift(&base.parameter("x").unwrap()).unwrap();
    let n_plus_x = context.add(&n, &x).unwrap();
    let fabricated = IndexedCoefficient {
        raw: RationalPolynomial {
            numerator: n_plus_x.raw.numerator.clone(),
            denominator: n_plus_x.raw.numerator,
        },
        context: context.fingerprint.clone(),
    };

    let (value, denominator_nonzero) = context
        .specialize(&fabricated, &[1], IndexedAlgebraLimits::default())
        .unwrap();
    assert_eq!(value, base.one());
    assert_eq!(
        denominator_nonzero.unwrap(),
        (&base.parameter("x").unwrap() + &base.one()).numerator
    );
}

#[test]
fn specialization_limits_accept_exact_boundaries_and_reject_one_below() {
    let base = CoefficientContext::new(Vec::<String>::new());
    let context = IndexedCoefficientContext::try_new(&base, "specialization-limits", 1).unwrap();
    let n = context.index(0).unwrap();
    let n_squared = context.mul(&n, &n).unwrap();
    let exact = IndexedAlgebraLimits {
        exact_algebra: ExactAlgebraLimits {
            max_polynomial_terms: 1,
            max_term_operations: 1,
            ..ExactAlgebraLimits::default()
        },
        max_specialization_power_operations: 1,
        max_specialization_integer_bits: 5,
    };

    let (value, denominator_nonzero) = context.specialize(&n_squared, &[2], exact).unwrap();
    assert_eq!(value, base.integer(4));
    assert!(denominator_nonzero.is_none());

    let mut one_below = exact;
    one_below.max_specialization_power_operations = 0;
    assert!(matches!(
        context.specialize(&n_squared, &[2], one_below),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "coefficient specialization power operations",
            requested: 1,
            limit: 0,
        })
    ));

    let mut one_below = exact;
    one_below.max_specialization_integer_bits = 4;
    assert!(matches!(
        context.specialize(&n_squared, &[2], one_below),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "coefficient specialization integer bits",
            requested: 5,
            limit: 4,
        })
    ));

    let mut one_below = exact;
    one_below.exact_algebra.max_term_operations = 0;
    assert!(matches!(
        context.specialize(&n_squared, &[2], one_below),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "coefficient specialization normalization input term pairs",
            requested: 1,
            limit: 0,
        })
    ));
}

#[test]
fn specialization_bounds_the_full_u16_exponent_range() {
    let base = CoefficientContext::new(Vec::<String>::new());
    let context = IndexedCoefficientContext::try_new(&base, "u16-max-exponent", 1).unwrap();
    let mut highest_power = context.index(0).unwrap();
    highest_power.raw.numerator.exponents[0] = u16::MAX;

    let exact = IndexedAlgebraLimits {
        exact_algebra: ExactAlgebraLimits {
            max_polynomial_terms: 1,
            max_term_operations: 1,
            ..ExactAlgebraLimits::default()
        },
        max_specialization_power_operations: 1,
        max_specialization_integer_bits: 131_071,
    };
    let mut one_below = exact;
    one_below.max_specialization_integer_bits -= 1;
    assert!(matches!(
        context.specialize(&highest_power, &[2], one_below),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "coefficient specialization integer bits",
            requested: 131_071,
            limit: 131_070,
        })
    ));

    let (value, denominator_nonzero) = context.specialize(&highest_power, &[2], exact).unwrap();
    assert!(denominator_nonzero.is_none());
    assert_eq!(value.numerator.coefficients.len(), 1);
    assert_eq!(
        integer_magnitude_bits(&value.numerator.coefficients[0]),
        65_536
    );
}

#[test]
fn fixed_index_specialization_retains_unfixed_indices_and_denominator_guard() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "fixed-index", 3).unwrap();
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let n2 = context.index(2).unwrap();
    let x = context.lift(&base.parameter("x").unwrap()).unwrap();
    let numerator = context.mul(&n0, &context.add(&n1, &x).unwrap()).unwrap();
    let denominator = context.sub(&n2, &context.one()).unwrap();
    let value = context.div(&numerator, &denominator).unwrap();

    let (zero, guard) = context
        .specialize_fixed_indices(&value, &[(0, 0)], Default::default())
        .unwrap();
    assert!(zero.is_zero());
    assert_eq!(guard.raw(), &denominator.raw().numerator);
    assert_eq!(
        guard.raw().variables.as_ref(),
        context.variables.as_ref(),
        "partial specialization must keep the authenticated indexed map"
    );

    let (specialized, guard) = context
        .specialize_fixed_indices(&value, &[(1, 1)], Default::default())
        .unwrap();
    let expected = context
        .div(
            &context
                .mul(&n0, &context.add(&context.one(), &x).unwrap())
                .unwrap(),
            &denominator,
        )
        .unwrap();
    assert_eq!(specialized, expected);
    assert_eq!(guard.raw(), &denominator.raw().numerator);
}

#[test]
fn fixed_index_assignments_are_canonical_and_fail_closed() {
    let base = CoefficientContext::new(Vec::<String>::new());
    let context = IndexedCoefficientContext::try_new(&base, "fixed-index-errors", 2).unwrap();
    let value = context
        .add(&context.index(0).unwrap(), &context.index(1).unwrap())
        .unwrap();
    let left = context
        .specialize_fixed_indices(&value, &[(1, 3), (0, 2)], Default::default())
        .unwrap();
    let right = context
        .specialize_fixed_indices(&value, &[(0, 2), (1, 3)], Default::default())
        .unwrap();
    assert_eq!(left, right);
    assert_eq!(
        context.specialize_fixed_indices(&value, &[(0, 1), (0, 1)], Default::default()),
        Err(IndexedAlgebraError::DuplicateFixedIndex { position: 0 })
    );
    assert_eq!(
        context.specialize_fixed_indices(&value, &[(2, 1)], Default::default()),
        Err(IndexedAlgebraError::FixedIndexOutOfRange {
            position: 2,
            index_count: 2,
        })
    );
}
