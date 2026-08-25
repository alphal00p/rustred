use std::collections::{BTreeSet, HashSet};
use std::str::FromStr;

use rustred::{CoefficientContext, ExactRational, ExactRationalError};
use symbolica::domains::{integer::Integer, rational::Rational};

fn integer(decimal: &str) -> Integer {
    Integer::from_str(decimal).expect("test integer must be valid decimal input")
}

fn assert_same_rational(actual: &ExactRational, expected: &Rational) {
    assert_eq!(actual.as_rational(), expected);
    assert_eq!(actual.numerator(), expected.numerator_ref());
    assert_eq!(actual.denominator(), expected.denominator_ref());
    assert_eq!(actual.to_string(), expected.to_string());
}

#[test]
fn canonical_scalars_match_symbolica_rationals() {
    for (numerator, denominator) in [
        ("0", "-17"),
        ("6", "-8"),
        ("-6", "-8"),
        ("42", "30"),
        ("-9223372036854775808", "-1"),
        ("1", "-9223372036854775808"),
    ] {
        let numerator = integer(numerator);
        let denominator = integer(denominator);
        let actual = ExactRational::try_new(numerator.clone(), denominator.clone())
            .expect("the fixture denominator is nonzero");
        let expected = Rational::new(numerator, denominator);
        assert_same_rational(&actual, &expected);
    }

    let half = ExactRational::try_new(2, 4).unwrap();
    let canonical_half = ExactRational::try_new(1, 2).unwrap();
    assert_eq!(half, canonical_half);
    assert_eq!(canonical_half.numerator_i64(), Some(1));
    assert_eq!(canonical_half.denominator_i64(), Some(2));

    let mut ordered = BTreeSet::new();
    ordered.insert(half.clone());
    ordered.insert(canonical_half.clone());
    assert_eq!(ordered.len(), 1, "canonical equals must have one order key");

    let mut hashed = HashSet::new();
    hashed.insert(half);
    hashed.insert(canonical_half);
    assert_eq!(hashed.len(), 1, "canonical equals must have one hash key");
}

#[test]
fn scalar_arithmetic_is_arbitrary_precision_symbolica_arithmetic() {
    const LEFT_NUMERATOR: &str = "1606938044258990275541962092341162602522202993782792835301377";
    const LEFT_DENOMINATOR: &str = "340282366920938463463374607431768211456";
    const RIGHT_NUMERATOR: &str = "-1532495540865888858358347027150309183618739122183602171";
    const RIGHT_DENOMINATOR: &str = "18446744073709551629";

    let left_numerator = integer(LEFT_NUMERATOR);
    let left_denominator = integer(LEFT_DENOMINATOR);
    let right_numerator = integer(RIGHT_NUMERATOR);
    let right_denominator = integer(RIGHT_DENOMINATOR);

    let left = ExactRational::try_new(left_numerator.clone(), left_denominator.clone()).unwrap();
    let right = ExactRational::try_new(right_numerator.clone(), right_denominator.clone()).unwrap();
    let native_left = Rational::new(left_numerator, left_denominator);
    let native_right = Rational::new(right_numerator, right_denominator);

    assert!(left.numerator().to_i64().is_none());
    assert!(left.denominator().to_i64().is_none());
    assert_eq!(left.numerator_i64(), None);
    assert_eq!(left.denominator_i64(), None);
    assert_same_rational(
        &(left.clone() + right.clone()),
        &(native_left.clone() + native_right.clone()),
    );
    assert_same_rational(
        &(left.clone() - right.clone()),
        &(native_left.clone() - native_right.clone()),
    );
    assert_same_rational(
        &(left.clone() * right.clone()),
        &(native_left.clone() * native_right.clone()),
    );
    assert_same_rational(
        &(left.clone() / right.clone()),
        &(native_left.clone() / native_right.clone()),
    );
    assert_same_rational(
        &left.try_div(&right).unwrap(),
        &(native_left.clone() / native_right.clone()),
    );
    assert_same_rational(&left.try_reciprocal().unwrap(), &native_left.inv());
    assert_same_rational(&(-left.clone()), &(-native_left.clone()));

    let crossed_i64 = ExactRational::from(i64::MAX) + ExactRational::from(1);
    let native_crossed_i64 = Rational::from(i64::MAX) + Rational::from(1);
    assert_same_rational(&crossed_i64, &native_crossed_i64);

    let normalized_minimum = ExactRational::new(i64::MIN, -1);
    let native_normalized_minimum = Rational::new(i64::MIN, -1);
    assert_same_rational(&normalized_minimum, &native_normalized_minimum);
}

#[test]
fn fallible_scalar_boundaries_reject_zero() {
    assert!(matches!(
        ExactRational::try_new(1, 0),
        Err(ExactRationalError::ZeroDenominator)
    ));
    assert!(matches!(
        ExactRational::zero().try_reciprocal(),
        Err(ExactRationalError::DivisionByZero)
    ));
    assert!(matches!(
        ExactRational::one().try_div(&ExactRational::zero()),
        Err(ExactRationalError::DivisionByZero)
    ));
}

#[test]
fn coefficient_bridge_preserves_gmp_numerator_and_denominator() {
    const NUMERATOR: &str = "1606938044258990275541962092341162602522202993782792835301377";
    const DENOMINATOR: &str = "340282366920938463463374607431768211456";

    let value = ExactRational::try_new(integer(NUMERATOR), integer(DENOMINATOR)).unwrap();
    assert!(value.numerator().to_i64().is_none());
    assert!(value.denominator().to_i64().is_none());

    let context = CoefficientContext::new(["d"]);
    let expected = context
        .parse(&format!("({NUMERATOR})/({DENOMINATOR})"))
        .unwrap();
    assert_eq!(context.rational(&value), expected);
    assert_eq!(context.rational(value.clone()), expected);

    let dimension = context.parameter("d").unwrap();
    let expected_scaled = context
        .parse(&format!("d*({NUMERATOR})/({DENOMINATOR})"))
        .unwrap();
    assert_eq!(context.scale_rational(&dimension, &value), expected_scaled);
    assert_eq!(context.scale_rational(&dimension, value), expected_scaled);
}
