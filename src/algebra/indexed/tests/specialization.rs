use symbolica::prelude::RationalPolynomial;

use crate::algebra::CoefficientContext;

use super::super::{
    IndexedAlgebraError, IndexedAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext,
};

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
