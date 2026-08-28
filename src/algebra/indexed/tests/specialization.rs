use symbolica::prelude::RationalPolynomial;

use crate::algebra::CoefficientContext;

use super::super::{
    IndexedAlgebraError, IndexedAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext,
};

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
    assert_eq!(generic.value, base.one());
    assert!(
        generic.denominator_nonzero.is_none(),
        "constant nonzero conditions are tautologies"
    );
    assert!(matches!(
        context.specialize(&fabricated, &[1], IndexedAlgebraLimits::default(),),
        Err(IndexedAlgebraError::ZeroDenominator)
    ));
}
