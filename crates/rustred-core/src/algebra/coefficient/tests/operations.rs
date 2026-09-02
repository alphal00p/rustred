use super::super::{
    Coefficient, CoefficientContext, ExactAlgebraError, ExactAlgebraLimits, ExactAlgebraOperation,
    SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT, operations::sum_uses_denominator_gcd_fallback_for_test,
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

fn shared_denominator_fixture() -> (CoefficientContext, Coefficient, Coefficient) {
    let context = CoefficientContext::new(["x", "y"]);
    let left = context.coefficient_fixture("(y^3+y^2+y+1)/((x+1)*(x^2+1))");
    let right = context.coefficient_fixture("(2*y^3+3*y^2+5*y+7)/((x+1)*(x^2+2))");
    (context, left, right)
}

#[test]
fn shared_denominator_gcd_reduces_sum_preflight_support() {
    let (context, left, right) = shared_denominator_fixture();
    let limit = 20;
    let old_cartesian_numerator = left.numerator.nterms() * right.denominator.nterms()
        + right.numerator.nterms() * left.denominator.nterms();
    assert_eq!(left.denominator.nterms() * right.denominator.nterms(), 16);
    assert_eq!(old_cartesian_numerator, 32);
    assert!(old_cartesian_numerator > limit);

    let limits = ExactAlgebraLimits {
        max_polynomial_terms: 64,
        max_term_operations: limit,
        ..ExactAlgebraLimits::default()
    };
    assert!(
        sum_uses_denominator_gcd_fallback_for_test(
            &left,
            &right,
            context.variables(),
            ExactAlgebraOperation::Add,
            limits,
        )
        .unwrap()
    );
    let addition = context.try_add(&left, &right, limits).unwrap();
    let subtraction = context.try_sub(&left, &right, limits).unwrap();
    assert_eq!(addition, &left + &right);
    assert_eq!(subtraction, &left - &right);
    assert!(context.contains(&addition));
    assert!(context.contains(&subtraction));
}

#[test]
fn old_safe_unequal_denominator_sum_stays_on_cheap_preflight() {
    let (context, left, right) = shared_denominator_fixture();
    let limits = ExactAlgebraLimits {
        max_polynomial_terms: 128,
        max_term_operations: 128,
        ..ExactAlgebraLimits::default()
    };

    assert_ne!(left.denominator, right.denominator);
    assert!(left.denominator.gcd(&right.denominator).nterms() > 1);
    for operation in [ExactAlgebraOperation::Add, ExactAlgebraOperation::Subtract] {
        assert!(
            !sum_uses_denominator_gcd_fallback_for_test(
                &left,
                &right,
                context.variables(),
                operation,
                limits,
            )
            .unwrap()
        );
    }
    assert_eq!(
        context.try_add(&left, &right, limits).unwrap(),
        &left + &right
    );
    assert_eq!(
        context.try_sub(&left, &right, limits).unwrap(),
        &left - &right
    );
}

#[test]
fn shared_high_degree_factor_can_trigger_and_pass_gcd_fallback() {
    let context = CoefficientContext::new(["x"]);
    let left = context.coefficient_fixture("1/((x^4+1)*(x+1))");
    let right = context.coefficient_fixture("2/((x^4+1)*(x+2))");
    let limits = ExactAlgebraLimits {
        max_exponent: 6,
        max_polynomial_terms: 128,
        max_term_operations: 128,
    };

    // Every input degree is admitted, but the old unreduced denominator
    // projection has degree ten. Removing the shared quartic leaves the true
    // degree-six denominator product admitted by the fallback.
    assert!(left.denominator.degree(0) <= limits.max_exponent);
    assert!(right.denominator.degree(0) <= limits.max_exponent);
    assert_eq!(left.denominator.degree(0) + right.denominator.degree(0), 10);
    assert!(
        sum_uses_denominator_gcd_fallback_for_test(
            &left,
            &right,
            context.variables(),
            ExactAlgebraOperation::Add,
            limits,
        )
        .unwrap()
    );
    assert_eq!(
        context.try_add(&left, &right, limits).unwrap(),
        &left + &right
    );
}

#[test]
fn gcd_fallback_covers_both_symbolica_denominator_product_orientations() {
    let context = CoefficientContext::new(["x", "y"]);
    let larger = context.coefficient_fixture("(y^3+y^2+y+1)/((x+1)*(x^2+x+1))");
    let smaller = context.coefficient_fixture("(2*y^3+3*y^2+5*y+7)/((x+1)*(x+2))");
    let limits = ExactAlgebraLimits {
        max_polynomial_terms: 64,
        max_term_operations: 20,
        ..ExactAlgebraLimits::default()
    };

    let gcd = larger.denominator.gcd(&smaller.denominator);
    let larger_reduced = larger.denominator.try_div(&gcd).unwrap();
    let smaller_reduced = smaller.denominator.try_div(&gcd).unwrap();
    assert!(
        larger.denominator.nterms() > smaller.denominator.nterms()
            && larger.denominator.nterms() > larger_reduced.nterms()
    );
    assert!(
        !(smaller.denominator.nterms() > larger.denominator.nterms()
            && smaller.denominator.nterms() > smaller_reduced.nterms())
    );

    // The first ordering takes Symbolica's `right_reduced * left` branch;
    // swapping operands takes its `left_reduced * right` branch. Both old
    // numerator projections need 28 slots, while both reduced projections
    // need exactly the admitted 20 slots.
    for (left, right) in [(&larger, &smaller), (&smaller, &larger)] {
        let old_numerator_terms = left.numerator.nterms() * right.denominator.nterms()
            + right.numerator.nterms() * left.denominator.nterms();
        assert_eq!(old_numerator_terms, 28);
        assert!(
            sum_uses_denominator_gcd_fallback_for_test(
                left,
                right,
                context.variables(),
                ExactAlgebraOperation::Add,
                limits,
            )
            .unwrap()
        );
        assert_eq!(context.try_add(left, right, limits).unwrap(), left + right);
    }
}

#[test]
fn coprime_denominators_preserve_cartesian_sum_rejection() {
    let context = CoefficientContext::new(["x", "y"]);
    let left = context.coefficient_fixture("(y^5+y^4+y^3+y^2+y+1)/(x+1)");
    let right = context.coefficient_fixture("(2*y^5+3*y^4+5*y^3+7*y^2+11*y+13)/(x^2+1)");
    assert!(left.denominator.gcd(&right.denominator).is_one());

    let error = context
        .try_add(
            &left,
            &right,
            ExactAlgebraLimits {
                max_polynomial_terms: 64,
                max_term_operations: 20,
                ..ExactAlgebraLimits::default()
            },
        )
        .unwrap_err();
    assert!(matches!(
        error,
        ExactAlgebraError::ResourceLimit {
            resource: "exact addition numerator terms",
            requested: 24,
            limit: 20,
        }
    ));
}

#[test]
fn denominator_gcd_work_cap_rejects_sum_transactionally() {
    let (context, left, right) = shared_denominator_fixture();
    let left_before = left.clone();
    let right_before = right.clone();
    let limits = ExactAlgebraLimits {
        max_polynomial_terms: 64,
        max_term_operations: 15,
        ..ExactAlgebraLimits::default()
    };

    for error in [
        context.try_add(&left, &right, limits).unwrap_err(),
        context.try_sub(&left, &right, limits).unwrap_err(),
    ] {
        assert!(matches!(
            error,
            ExactAlgebraError::ResourceLimit {
                resource: "exact sum denominator GCD term pairs",
                requested: 16,
                limit: 15,
            }
        ));
    }
    assert_eq!(left, left_before);
    assert_eq!(right, right_before);
}

#[test]
fn gcd_reduced_sum_matches_direct_symbolica_arithmetic() {
    let (context, left, right) = shared_denominator_fixture();
    let expected_addition = &left + &right;
    let expected_subtraction = &left - &right;
    let limits = ExactAlgebraLimits {
        max_polynomial_terms: 64,
        max_term_operations: 20,
        ..ExactAlgebraLimits::default()
    };

    assert_eq!(
        context.try_add(&left, &right, limits).unwrap(),
        expected_addition
    );
    assert_eq!(
        context.try_sub(&left, &right, limits).unwrap(),
        expected_subtraction
    );
}
