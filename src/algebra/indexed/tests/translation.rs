use crate::algebra::CoefficientContext;

use super::super::{IndexedAlgebraLimits, IndexedCoefficientContext};

#[test]
fn lift_translate_and_specialize_preserve_authenticated_maps() {
    let base = CoefficientContext::new(["d", "m2"]);
    let context = IndexedCoefficientContext::try_new(&base, "translation", 2).unwrap();
    let d = base.parameter("d").unwrap();
    let m2 = base.parameter("m2").unwrap();
    let family_value = &(&d + &base.integer(1)) / &m2;
    let lifted = context.lift(&family_value).unwrap();
    let n0 = context.index(0).unwrap();
    let value = context.mul(&n0, &lifted).unwrap();
    let translated = context
        .translate(&value, &[2, -3], IndexedAlgebraLimits::default())
        .unwrap();
    let specialized = context
        .specialize(&translated, &[5, 100], IndexedAlgebraLimits::default())
        .unwrap();
    let expected = &base.integer(7) * &family_value;
    assert_eq!(specialized.value, expected);
    assert_eq!(
        specialized.denominator_nonzero.unwrap().to_expression(),
        m2.to_expression()
    );
}
