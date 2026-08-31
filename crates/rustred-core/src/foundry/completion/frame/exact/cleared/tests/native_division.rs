use crate::algebra::{
    CoefficientContext, ExactAlgebraError, IndexedAlgebraError, IndexedCoefficient,
    IndexedCoefficientContext, IndexedPolynomial,
};

use super::super::{
    budget::PolynomialBudget,
    model::{ClearedCircuitError, ClearedCircuitLimits},
};

const RETAINED_POLYNOMIAL_TERMS: &str = "cleared-circuit retained polynomial terms";

fn context(scope: &str) -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(&CoefficientContext::new(Vec::<String>::new()), scope, 2)
        .unwrap()
}

fn polynomial(
    context: &IndexedCoefficientContext,
    coefficient: &IndexedCoefficient,
) -> IndexedPolynomial {
    context
        .numerator_condition_with_limits(coefficient, Default::default())
        .unwrap()
}

fn add(
    context: &IndexedCoefficientContext,
    left: &IndexedCoefficient,
    right: &IndexedCoefficient,
) -> IndexedCoefficient {
    context.add(left, right).unwrap()
}

fn sub(
    context: &IndexedCoefficientContext,
    left: &IndexedCoefficient,
    right: &IndexedCoefficient,
) -> IndexedCoefficient {
    context.sub(left, right).unwrap()
}

fn mul(
    context: &IndexedCoefficientContext,
    left: &IndexedCoefficient,
    right: &IndexedCoefficient,
) -> IndexedCoefficient {
    context.mul(left, right).unwrap()
}

fn pow(
    context: &IndexedCoefficientContext,
    base: &IndexedCoefficient,
    exponent: usize,
) -> IndexedCoefficient {
    let mut result = context.one();
    for _ in 0..exponent {
        result = mul(context, &result, base);
    }
    result
}

fn divide(
    context: &IndexedCoefficientContext,
    numerator: &IndexedCoefficient,
    denominator: &IndexedCoefficient,
) -> Result<(IndexedPolynomial, usize, usize), ClearedCircuitError> {
    let numerator = polynomial(context, numerator);
    let denominator = polynomial(context, denominator);
    let mut budget = PolynomialBudget::new(context, ClearedCircuitLimits::default());
    let quotient = budget.exact_polynomial_division(&numerator, &denominator)?;
    Ok((quotient, budget.operations, budget.retained_terms))
}

#[test]
fn native_exact_division_returns_a_nontrivial_multivariate_quotient() {
    let context = context("cleared-native-exact-nontrivial");
    let x = context.index(0).unwrap();
    let y = context.index(1).unwrap();
    let numerator = sub(&context, &pow(&context, &x, 2), &pow(&context, &y, 2));
    let denominator = sub(&context, &x, &y);
    let expected = polynomial(&context, &add(&context, &x, &y));

    let (quotient, operations, retained_terms) =
        divide(&context, &numerator, &denominator).unwrap();
    assert_eq!(quotient, expected);
    assert_eq!(operations, 1, "one native quotient is one exact operation");
    assert_eq!(retained_terms, quotient.raw().nterms());
}

#[test]
fn native_exact_division_handles_integer_content_and_a_negative_divisor() {
    let context = context("cleared-native-exact-content");
    let x = context.index(0).unwrap();
    let one = context.one();
    let numerator = mul(
        &context,
        &context.integer(6),
        &sub(&context, &pow(&context, &x, 2), &one),
    );
    let denominator = mul(&context, &context.integer(-3), &sub(&context, &x, &one));
    let expected = polynomial(
        &context,
        &mul(&context, &context.integer(-2), &add(&context, &x, &one)),
    );

    assert_eq!(
        divide(&context, &numerator, &denominator).unwrap().0,
        expected
    );
}

#[test]
fn native_exact_division_distinguishes_nonexact_coefficients_and_remainders() {
    let context = context("cleared-native-nonexact");
    let x = context.index(0).unwrap();
    let one = context.one();

    assert_eq!(
        divide(&context, &x, &context.integer(2)).unwrap_err(),
        ClearedCircuitError::NonExactPolynomialDivision
    );

    let numerator = add(&context, &pow(&context, &x, 2), &one);
    let denominator = add(&context, &x, &one);
    assert_eq!(
        divide(&context, &numerator, &denominator).unwrap_err(),
        ClearedCircuitError::NonExactPolynomialDivision
    );
}

#[test]
fn native_exact_division_preserves_typed_zero_and_foreign_context_errors() {
    let foreign = context("cleared-native-errors-foreign");
    let context = context("cleared-native-errors");
    let x = context.index(0).unwrap();
    let zero = context.zero();

    assert_eq!(
        divide(&context, &x, &zero).unwrap_err(),
        ClearedCircuitError::IndexedAlgebra(IndexedAlgebraError::ExactAlgebra(
            ExactAlgebraError::DivisionByZero
        ))
    );

    let foreign_numerator = polynomial(&foreign, &foreign.index(0).unwrap());
    let denominator = polynomial(&context, &context.one());
    let mut budget = PolynomialBudget::new(&context, ClearedCircuitLimits::default());
    assert_eq!(
        budget
            .exact_polynomial_division(&foreign_numerator, &denominator)
            .unwrap_err(),
        ClearedCircuitError::IndexedAlgebra(IndexedAlgebraError::WrongContext)
    );
    assert_eq!(budget.operations, 0, "validation precedes native execution");
    assert_eq!(budget.retained_terms, 0);
}

#[test]
fn native_exact_division_enforces_exact_and_retained_output_caps() {
    let context = context("cleared-native-caps");
    let x = context.index(0).unwrap();
    let y = context.index(1).unwrap();
    let numerator = polynomial(
        &context,
        &sub(&context, &pow(&context, &x, 4), &pow(&context, &y, 4)),
    );
    let denominator = polynomial(&context, &sub(&context, &x, &y));

    let mut exact_limits = ClearedCircuitLimits::default();
    exact_limits.exact_algebra.max_polynomial_terms = 4;
    let mut exact = PolynomialBudget::new(&context, exact_limits);
    let quotient = exact
        .exact_polynomial_division(&numerator, &denominator)
        .unwrap();
    assert_eq!(quotient.raw().nterms(), 4);

    let mut one_below_exact = exact_limits;
    one_below_exact.exact_algebra.max_polynomial_terms = 3;
    assert_eq!(
        PolynomialBudget::new(&context, one_below_exact)
            .exact_polynomial_division(&numerator, &denominator)
            .unwrap_err(),
        ClearedCircuitError::IndexedAlgebra(IndexedAlgebraError::ExactAlgebra(
            ExactAlgebraError::ResourceLimit {
                resource: "authenticated polynomial terms",
                requested: 4,
                limit: 3,
            }
        ))
    );

    let mut retained_limits = ClearedCircuitLimits::default();
    retained_limits.max_retained_polynomial_terms = 4;
    PolynomialBudget::new(&context, retained_limits)
        .exact_polynomial_division(&numerator, &denominator)
        .unwrap();

    let mut one_below_retained = retained_limits;
    one_below_retained.max_retained_polynomial_terms = 3;
    assert_eq!(
        PolynomialBudget::new(&context, one_below_retained)
            .exact_polynomial_division(&numerator, &denominator)
            .unwrap_err(),
        ClearedCircuitError::ResourceLimit {
            resource: RETAINED_POLYNOMIAL_TERMS,
            requested: 4,
            limit: 3,
        }
    );

    let mut operation_limits = ClearedCircuitLimits::default();
    operation_limits.max_polynomial_operations = 1;
    PolynomialBudget::new(&context, operation_limits)
        .exact_polynomial_division(&numerator, &denominator)
        .unwrap();
    operation_limits.max_polynomial_operations = 0;
    assert_eq!(
        PolynomialBudget::new(&context, operation_limits)
            .exact_polynomial_division(&numerator, &denominator)
            .unwrap_err(),
        ClearedCircuitError::ResourceLimit {
            resource: "cleared-circuit polynomial operations",
            requested: 1,
            limit: 0,
        }
    );
}

#[test]
fn native_exact_division_replays_small_deterministic_products() {
    let context = context("cleared-native-product-replay");
    let x = context.index(0).unwrap();
    let y = context.index(1).unwrap();
    let factors = [
        add(&context, &x, &context.one()),
        sub(&context, &x, &y),
        add(&context, &y, &context.integer(2)),
    ];
    let product = factors.iter().fold(context.one(), |accumulator, factor| {
        mul(&context, &accumulator, factor)
    });

    for (omitted, divisor) in factors.iter().enumerate() {
        let expected = factors
            .iter()
            .enumerate()
            .filter(|(ordinal, _)| *ordinal != omitted)
            .fold(context.one(), |accumulator, (_, factor)| {
                mul(&context, &accumulator, factor)
            });
        assert_eq!(
            divide(&context, &product, divisor).unwrap().0,
            polynomial(&context, &expected)
        );
    }
}
