use symbolica::prelude::Integer;

use super::*;

fn factor(context: &CoefficientContext, expression: &str) -> FactorizedCoefficient {
    FactorizedCoefficient::from_coefficient(
        context,
        context.coefficient_fixture(expression),
        ExactAlgebraLimits::default(),
    )
    .unwrap()
}

fn same(context: &CoefficientContext, value: &FactorizedCoefficient, expected: &Coefficient) {
    let actual = value
        .materialize(context, ExactAlgebraLimits::default())
        .unwrap();
    assert_eq!(actual.numerator.variables(), expected.numerator.variables());
    assert_eq!(
        actual.denominator.variables(),
        expected.denominator.variables()
    );
    assert_eq!(&actual, expected);
}

#[test]
fn native_factorized_arithmetic_matches_sparse_with_contents_powers_and_cancellation() {
    for names in [vec!["x"], vec!["x", "y"]] {
        let context = CoefficientContext::new(names);
        let expressions = [
            "0",
            "1",
            "-7/13",
            "(x+1)^3/(2*(x-2)^4)",
            "-3*(x-2)^2/(5*(x+1)^3)",
            "(x^8-1)/(x-1)",
        ];
        for left in expressions {
            let a = context.coefficient_fixture(left);
            let fa = factor(&context, left);
            same(&context, &fa, &a);
            for right in expressions {
                let b = context.coefficient_fixture(right);
                let fb = factor(&context, right);
                same(
                    &context,
                    &fa.try_add(&fb, &context, Default::default()).unwrap(),
                    &(&a + &b),
                );
                same(
                    &context,
                    &fa.try_mul(&fb, &context, Default::default()).unwrap(),
                    &(&a * &b),
                );
            }
        }
        let x = factor(&context, "1/(x+1)^3");
        let minus = factor(&context, "-1/(x+1)^3");
        assert!(
            x.try_add(&minus, &context, Default::default())
                .unwrap()
                .is_zero()
        );
    }
}

#[test]
fn native_factorized_multivariate_arithmetic_retains_exact_ordered_maps() {
    let context = CoefficientContext::new(["x", "y"]);
    let a = context.coefficient_fixture("(x+y)/(2*(x+1)^2*(y-2))");
    let b = context.coefficient_fixture("-3*(y-2)/((x+1)*(x-y))");
    let fa =
        FactorizedCoefficient::from_coefficient(&context, a.clone(), Default::default()).unwrap();
    let fb =
        FactorizedCoefficient::from_coefficient(&context, b.clone(), Default::default()).unwrap();
    same(
        &context,
        &fa.try_add(&fb, &context, Default::default()).unwrap(),
        &(&a + &b),
    );
    same(
        &context,
        &fa.try_mul(&fb, &context, Default::default()).unwrap(),
        &(&a * &b),
    );
}

#[test]
fn zero_and_constant_foreign_maps_are_rejected_before_native_unification() {
    let context = CoefficientContext::new(["x"]);
    let foreign = CoefficientContext::new(["other"]);
    for expression in ["0", "1", "-3"] {
        assert!(matches!(
            FactorizedCoefficient::from_coefficient(
                &context,
                foreign.coefficient_fixture(expression),
                Default::default()
            ),
            Err(ExactAlgebraError::VariableMapMismatch { .. })
        ));
        let value = factor(&foreign, expression);
        assert!(matches!(
            value.materialize(&context, Default::default()),
            Err(ExactAlgebraError::VariableMapMismatch { .. })
        ));
    }
    let mut value = factor(&context, "1/(x+1)").value;
    value.denominators[0].0 = foreign.coefficient_fixture("other+1").numerator;
    assert!(matches!(
        FactorizedCoefficient::admit(&context, value, Default::default()),
        Err(ExactAlgebraError::VariableMapMismatch { .. })
    ));
}

#[test]
fn malformed_native_factor_scalars_multiplicities_and_layout_fail_closed() {
    let context = CoefficientContext::new(["x"]);
    let original = factor(&context, "1/(x+1)").value;
    let mut scalar_zero = original.clone();
    scalar_zero.denom_coeff = Integer::Single(0);
    assert!(matches!(
        FactorizedCoefficient::admit(&context, scalar_zero, Default::default()),
        Err(ExactAlgebraError::ZeroDenominator)
    ));
    let mut false_zero = original.clone();
    false_zero.numer_coeff = Integer::Single(0);
    assert!(matches!(
        FactorizedCoefficient::admit(&context, false_zero, Default::default()),
        Err(ExactAlgebraError::InvalidFactorizedRepresentation { .. })
    ));
    let mut power_zero = original.clone();
    power_zero.denominators[0].1 = 0;
    assert!(matches!(
        FactorizedCoefficient::admit(&context, power_zero, Default::default()),
        Err(ExactAlgebraError::InvalidFactorizedRepresentation { .. })
    ));
    let mut constant = original.clone();
    constant.denominators[0].0 = context.one().numerator;
    assert!(matches!(
        FactorizedCoefficient::admit(&context, constant, Default::default()),
        Err(ExactAlgebraError::InvalidFactorizedRepresentation { .. })
    ));
    let mut zero = original.clone();
    zero.denominators[0].0 = context.zero().numerator;
    assert!(matches!(
        FactorizedCoefficient::admit(&context, zero, Default::default()),
        Err(ExactAlgebraError::ZeroDenominator)
    ));
    let mut layout = original;
    layout.denominators[0].0.exponents.clear();
    assert!(matches!(
        FactorizedCoefficient::admit(&context, layout, Default::default()),
        Err(ExactAlgebraError::MalformedExponentLayout { .. })
    ));
}

#[test]
fn expanded_exponent_and_multiplicity_overflow_never_enter_native_arithmetic() {
    let context = CoefficientContext::new(["x"]);
    let inverse = factor(&context, "1/x");
    let mut large = inverse.value.clone();
    large.denominators[0].1 = 65_535;
    let large = FactorizedCoefficient::admit(&context, large, Default::default()).unwrap();
    assert!(matches!(
        large.try_mul(&inverse, &context, Default::default()),
        Err(ExactAlgebraError::ExponentLimit {
            requested: 65_536,
            ..
        })
    ));
    let mut overflowed = factor(&context, "1/(x^2+1)").value;
    overflowed.denominators[0].1 = usize::MAX;
    assert!(matches!(
        FactorizedCoefficient::admit(&context, overflowed, Default::default()),
        Err(ExactAlgebraError::ResourceCountOverflow { .. })
    ));
}

#[test]
fn factorized_resource_boundaries_and_dense_native_quotients_are_checked() {
    let context = CoefficientContext::new(["x"]);
    let value = factor(&context, "1/(x+1)^4");
    let exact = ExactAlgebraLimits {
        max_polynomial_terms: 5,
        ..Default::default()
    };
    FactorizedCoefficient::admit(&context, value.value.clone(), exact).unwrap();
    assert!(matches!(
        FactorizedCoefficient::admit(
            &context,
            value.value.clone(),
            ExactAlgebraLimits {
                max_polynomial_terms: 4,
                ..exact
            }
        ),
        Err(ExactAlgebraError::ResourceLimit { .. })
    ));
    assert!(matches!(
        value.materialize(
            &context,
            ExactAlgebraLimits {
                max_term_operations: 24,
                ..exact
            }
        ),
        Err(ExactAlgebraError::ResourceLimit { .. })
    ));
    value
        .materialize(
            &context,
            ExactAlgebraLimits {
                max_term_operations: 25,
                ..exact
            },
        )
        .unwrap();
    let a = factor(&context, "x^8-1");
    let b = factor(&context, "1/(x-1)");
    assert!(matches!(
        a.try_mul(
            &b,
            &context,
            ExactAlgebraLimits {
                max_polynomial_terms: 2,
                ..Default::default()
            }
        ),
        Err(ExactAlgebraError::ResourceLimit { requested: 8, .. })
    ));
    assert!(matches!(
        a.try_add(
            &b,
            &context,
            ExactAlgebraLimits {
                max_term_operations: 0,
                ..Default::default()
            }
        ),
        Err(ExactAlgebraError::ResourceLimit { .. })
    ));
}

#[test]
fn cache_weight_covers_stored_factors_and_expanded_payload_terms() {
    let context = CoefficientContext::new(["x"]);
    for expression in ["1", "1/(x+1)^8", "123456789012345678901234567890/(x^3+1)"] {
        let value = factor(&context, expression);
        let ordinary = value.materialize(&context, Default::default()).unwrap();
        let (terms, bytes) = value.cache_weight().unwrap();
        assert!(terms >= ordinary.numerator.nterms() + ordinary.denominator.nterms());
        assert!(
            terms
                >= value.value.numerator.nterms()
                    + value
                        .value
                        .denominators
                        .iter()
                        .map(|(p, _)| p.nterms())
                        .sum::<usize>()
                    + 2
        );
        assert!(bytes >= size_of::<Native>());
        assert_eq!((terms, bytes), value.cache_weight().unwrap());
    }
}
