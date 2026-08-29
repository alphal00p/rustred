use super::super::{
    CoefficientContext, ExactAlgebraError, ExactAlgebraLimits, ExactAlgebraOperation,
    SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
};

#[test]
fn checked_exact_multiplication_reports_u16_exponent_overflow() {
    let context = CoefficientContext::new(["x"]);
    let maximal = context.coefficient_fixture("x^65535");
    let x = context.parameter("x").unwrap();
    assert!(matches!(
        context.try_mul(&maximal, &x, ExactAlgebraLimits::default()),
        Err(ExactAlgebraError::ExponentLimit {
            operation: ExactAlgebraOperation::Multiply,
            variable: 0,
            requested: 65_536,
            limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
        })
    ));
}

#[test]
fn checked_power_preflight_uses_u64_before_native_arithmetic() {
    let context = CoefficientContext::new(["x"]);
    let x_squared = context.coefficient_fixture("x^2");
    assert!(matches!(
        context.preflight_power_with_limits(&x_squared, u64::MAX, ExactAlgebraLimits::default(),),
        Err(ExactAlgebraError::ExponentArithmeticOverflow {
            operation: ExactAlgebraOperation::Power,
            variable: 0,
            width: 64,
        })
    ));

    let x_40_000 = context.coefficient_fixture("x^40000");
    assert!(matches!(
        context.preflight_power_with_limits(&x_40_000, 2, ExactAlgebraLimits::default()),
        Err(ExactAlgebraError::ExponentLimit {
            operation: ExactAlgebraOperation::Power,
            variable: 0,
            requested: 80_000,
            limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
        })
    ));
}

#[test]
fn rational_normalization_can_densify_beyond_input_pair_counts() {
    let context = CoefficientContext::new(["x"]);
    let geometric_numerator = context.coefficient_fixture("x^8-1");
    let linear = context.coefficient_fixture("x-1");
    let reciprocal_linear = context.coefficient_fixture("1/(x-1)");

    let division = context
        .try_div(&geometric_numerator, &linear, ExactAlgebraLimits::default())
        .unwrap();
    let multiplication = context
        .try_mul(
            &geometric_numerator,
            &reciprocal_linear,
            ExactAlgebraLimits::default(),
        )
        .unwrap();
    assert_eq!(division.numerator.nterms(), 8);
    assert_eq!(multiplication.numerator.nterms(), 8);
    assert_eq!(division.denominator.nterms(), 1);
    assert_eq!(multiplication, division);
    assert!(
        division.numerator.nterms()
            > geometric_numerator.numerator.nterms() * linear.denominator.nterms()
    );

    let left = context.coefficient_fixture("1/(x-1)");
    let right = context.coefficient_fixture("(x^8-2)/(x-1)");
    let addition = context
        .try_add(&left, &right, ExactAlgebraLimits::default())
        .unwrap();
    assert_eq!(addition.numerator.nterms(), 8);
    assert_eq!(addition.denominator.nterms(), 1);
    assert!(addition.numerator.nterms() > left.numerator.nterms() + right.numerator.nterms());

    // These one-step input counts are not sound retained-output bounds for
    // rational arithmetic. The checked path must still reject the dense
    // normalized result during post-authentication.
    for error in [
        context
            .try_mul(
                &geometric_numerator,
                &reciprocal_linear,
                ExactAlgebraLimits {
                    max_polynomial_terms: 2,
                    ..ExactAlgebraLimits::default()
                },
            )
            .unwrap_err(),
        context
            .try_div(
                &geometric_numerator,
                &linear,
                ExactAlgebraLimits {
                    max_polynomial_terms: 2,
                    ..ExactAlgebraLimits::default()
                },
            )
            .unwrap_err(),
        context
            .try_add(
                &left,
                &right,
                ExactAlgebraLimits {
                    max_polynomial_terms: 3,
                    ..ExactAlgebraLimits::default()
                },
            )
            .unwrap_err(),
    ] {
        assert!(matches!(
            error,
            ExactAlgebraError::ResourceLimit {
                resource: "authenticated polynomial terms",
                requested: 8,
                ..
            }
        ));
    }
}
