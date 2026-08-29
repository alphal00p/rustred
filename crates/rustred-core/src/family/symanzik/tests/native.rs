use std::sync::Arc;

use symbolica::prelude::*;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};

use super::super::context::FeynmanPolynomialContext;
use super::super::error::FeynmanPolynomialError;
use super::super::model::{FeynmanPolynomial, FeynmanPolynomialLimits, FeynmanPolynomialRing};
use super::super::operations::{checked_adjugate, checked_determinant};
use super::super::work::FeynmanWorkBudget;

fn matrix_family(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "s"]);
    let denominators = (0..5)
        .map(|coordinate| {
            AffineDenominator::new(
                coefficients.zero(),
                (0..5)
                    .map(|candidate| {
                        if candidate == coordinate {
                            coefficients.one()
                        } else {
                            coefficients.zero()
                        }
                    })
                    .collect(),
            )
        })
        .collect();
    IntegralFamily::new(
        name,
        vec!["k0".into(), "k1".into()],
        vec!["p".into()],
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        denominators,
        vec![vec![coefficients.parameter("s").unwrap()]],
        vec![coefficients.zero(); 5],
    )
    .unwrap()
}

fn matrix_context(name: &str, limits: FeynmanPolynomialLimits) -> FeynmanPolynomialContext {
    FeynmanPolynomialContext::try_new(&matrix_family(name), limits).unwrap()
}

fn variable(context: &FeynmanPolynomialContext, parameter: usize) -> FeynmanPolynomial {
    context
        .parameter_monomial(parameter, &context.coefficients.one())
        .unwrap()
}

fn integer(context: &FeynmanPolynomialContext, value: i64) -> FeynmanPolynomial {
    context
        .constant(context.coefficients.integer(value))
        .unwrap()
}

fn symbolic_tridiagonal_four(context: &FeynmanPolynomialContext) -> Vec<Vec<FeynmanPolynomial>> {
    let zero = context.zero();
    let one = integer(context, 1);
    let x0 = variable(context, 0);
    let x1 = variable(context, 1);
    let x2 = variable(context, 2);
    let x3 = variable(context, 3);
    vec![
        vec![x0, one.clone(), zero.clone(), zero.clone()],
        vec![one.clone(), x1, one.clone(), zero.clone()],
        vec![zero.clone(), one.clone(), x2, one.clone()],
        vec![zero.clone(), zero, one, x3],
    ]
}

fn native_matrix(
    context: &FeynmanPolynomialContext,
    matrix: &[Vec<FeynmanPolynomial>],
) -> Matrix<FeynmanPolynomialRing> {
    Matrix::from_nested_vec(
        matrix
            .iter()
            .map(|row| row.iter().map(|entry| entry.raw.clone()).collect())
            .collect(),
        FeynmanPolynomialRing::from_poly(&context.template),
    )
    .unwrap()
}

#[test]
fn empty_determinant_is_the_authenticated_multiplicative_identity() {
    let limits = FeynmanPolynomialLimits::default();
    let context = matrix_context("feynman-native-empty-determinant", limits);
    let mut work = FeynmanWorkBudget::new(limits);
    let determinant = checked_determinant(&context, &[], &mut work).unwrap();

    assert_eq!(determinant, integer(&context, 1));
    context.authenticate(&determinant).unwrap();
    assert_eq!(work.determinant_ring_operations, 0);
}

#[test]
fn native_small_determinants_have_exact_structural_call_counts() {
    let limits = FeynmanPolynomialLimits::default();
    let context = matrix_context("feynman-native-small-counts", limits);
    let zero = context.zero();

    let two = vec![
        vec![variable(&context, 0), zero.clone()],
        vec![zero.clone(), variable(&context, 1)],
    ];
    let mut work = FeynmanWorkBudget::new(limits);
    checked_determinant(&context, &two, &mut work).unwrap();
    assert_eq!(work.determinant_ring_operations, 3);

    let three = vec![
        vec![variable(&context, 0), zero.clone(), zero.clone()],
        vec![zero.clone(), variable(&context, 1), zero.clone()],
        vec![zero.clone(), zero, variable(&context, 2)],
    ];
    let mut work = FeynmanWorkBudget::new(limits);
    checked_determinant(&context, &three, &mut work).unwrap();
    assert_eq!(work.determinant_ring_operations, 14);
}

#[test]
fn ragged_determinant_is_rejected_before_native_construction() {
    let limits = FeynmanPolynomialLimits::default();
    let context = matrix_context("feynman-native-ragged", limits);
    let matrix = vec![
        vec![variable(&context, 0), variable(&context, 1)],
        vec![variable(&context, 2)],
    ];
    let mut work = FeynmanWorkBudget::new(limits);

    assert!(matches!(
        checked_determinant(&context, &matrix, &mut work),
        Err(FeynmanPolynomialError::InternalVerificationFailure { detail })
            if detail == "determinant received a non-square matrix"
    ));
    assert_eq!(work.determinant_ring_operations, 0);
}

#[test]
fn symbolica_four_by_four_determinant_retains_symbolic_terms() {
    let limits = FeynmanPolynomialLimits::default();
    let context = matrix_context("feynman-native-symbolic-four", limits);
    let matrix = symbolic_tridiagonal_four(&context);
    let mut work = FeynmanWorkBudget::new(limits);
    let determinant = checked_determinant(&context, &matrix, &mut work).unwrap();
    let one = context.coefficients.one();
    let minus_one = context.coefficients.integer(-1);

    // det = x0*x1*x2*x3 - x2*x3 - x0*x3 - x0*x1 + 1.
    assert_eq!(determinant.term_count(), 5);
    assert_eq!(determinant.coefficient(&[1, 1, 1, 1, 0]), Some(&one));
    assert_eq!(determinant.coefficient(&[0, 0, 1, 1, 0]), Some(&minus_one));
    assert_eq!(determinant.coefficient(&[1, 0, 0, 1, 0]), Some(&minus_one));
    assert_eq!(determinant.coefficient(&[1, 1, 0, 0, 0]), Some(&minus_one));
    assert_eq!(determinant.coefficient(&[0, 0, 0, 0, 0]), Some(&one));
    assert_eq!(work.determinant_ring_operations, 56);
}

#[test]
fn singular_native_four_by_four_zero_is_rebound_to_the_context_variable_map() {
    let limits = FeynmanPolynomialLimits::default();
    let context = matrix_context("feynman-native-singular-four", limits);
    let zero = context.zero();
    let matrix = vec![
        vec![
            zero.clone(),
            variable(&context, 0),
            zero.clone(),
            zero.clone(),
        ],
        vec![
            zero.clone(),
            zero.clone(),
            variable(&context, 1),
            zero.clone(),
        ],
        vec![
            zero.clone(),
            zero.clone(),
            zero.clone(),
            variable(&context, 2),
        ],
        vec![zero.clone(), zero.clone(), zero, variable(&context, 3)],
    ];
    let mut work = FeynmanWorkBudget::new(limits);
    let determinant = checked_determinant(&context, &matrix, &mut work).unwrap();

    assert!(determinant.is_zero());
    assert_eq!(determinant.raw.variables, context.variables);
    context.authenticate(&determinant).unwrap();
}

#[test]
fn native_bareiss_row_swap_has_the_correct_sign() {
    let limits = FeynmanPolynomialLimits::default();
    let context = matrix_context("feynman-native-row-swap", limits);
    let zero = context.zero();
    let matrix = vec![
        vec![
            zero.clone(),
            variable(&context, 0),
            zero.clone(),
            zero.clone(),
        ],
        vec![
            variable(&context, 1),
            zero.clone(),
            zero.clone(),
            zero.clone(),
        ],
        vec![
            zero.clone(),
            zero.clone(),
            variable(&context, 2),
            zero.clone(),
        ],
        vec![zero.clone(), zero.clone(), zero, variable(&context, 3)],
    ];
    let mut work = FeynmanWorkBudget::new(limits);
    let determinant = checked_determinant(&context, &matrix, &mut work).unwrap();

    assert_eq!(determinant.term_count(), 1);
    assert_eq!(
        determinant.coefficient(&[1, 1, 1, 1, 0]),
        Some(&context.coefficients.integer(-1))
    );
}

#[test]
fn native_constant_four_by_four_retains_the_authenticated_variable_map() {
    let limits = FeynmanPolynomialLimits::default();
    let context = matrix_context("feynman-native-constant-four", limits);
    let zero = context.zero();
    let matrix = vec![
        vec![
            integer(&context, 1),
            zero.clone(),
            zero.clone(),
            zero.clone(),
        ],
        vec![
            zero.clone(),
            integer(&context, 2),
            zero.clone(),
            zero.clone(),
        ],
        vec![
            zero.clone(),
            zero.clone(),
            integer(&context, 3),
            zero.clone(),
        ],
        vec![zero.clone(), zero.clone(), zero, integer(&context, 4)],
    ];
    let mut work = FeynmanWorkBudget::new(limits);
    let determinant = checked_determinant(&context, &matrix, &mut work).unwrap();

    assert_eq!(determinant, integer(&context, 24));
    assert_eq!(determinant.raw.variables, context.variables);
    context.authenticate(&determinant).unwrap();
}

#[test]
fn one_by_one_adjugate_uses_the_empty_native_cofactor() {
    let limits = FeynmanPolynomialLimits::default();
    let context = matrix_context("feynman-native-one-adjugate", limits);
    let matrix = vec![vec![variable(&context, 0)]];
    let mut work = FeynmanWorkBudget::new(limits);
    let adjugate = checked_adjugate(&context, &matrix, &mut work).unwrap();

    assert_eq!(adjugate, vec![vec![integer(&context, 1)]]);
    assert_eq!(work.determinant_ring_operations, 0);
}

#[test]
fn asymmetric_adjugate_replays_a_times_adjugate_with_symbolica_matrix_multiplication() {
    let limits = FeynmanPolynomialLimits::default();
    let context = matrix_context("feynman-native-asymmetric-adjugate", limits);
    let zero = context.zero();
    let one = integer(&context, 1);
    let matrix = vec![
        vec![variable(&context, 0), one.clone(), zero.clone()],
        vec![zero.clone(), variable(&context, 1), one.clone()],
        vec![one, zero, variable(&context, 2)],
    ];
    let mut work = FeynmanWorkBudget::new(limits);
    let determinant = checked_determinant(&context, &matrix, &mut work).unwrap();
    let adjugate = checked_adjugate(&context, &matrix, &mut work).unwrap();
    assert_eq!(work.determinant_ring_operations, 14 + 9 * 3);

    // Matrix multiplication, including every polynomial product and sum,
    // is performed by Symbolica's public K[x] matrix/ring API.
    let product = &native_matrix(&context, &matrix) * &native_matrix(&context, &adjugate);
    for row in 0..3_u32 {
        for column in 0..3_u32 {
            if row == column {
                assert_eq!(&product[(row, column)], determinant.raw());
            } else {
                assert!(product[(row, column)].is_zero());
            }
        }
    }
}

#[test]
fn native_four_by_four_resource_preflight_has_exact_boundaries() {
    let below_operations = FeynmanPolynomialLimits {
        max_determinant_ring_operations: 55,
        ..FeynmanPolynomialLimits::default()
    };
    let context = matrix_context("feynman-native-four-below-operations", below_operations);
    let matrix = symbolic_tridiagonal_four(&context);
    let mut work = FeynmanWorkBudget::new(below_operations);
    assert!(matches!(
        checked_determinant(&context, &matrix, &mut work),
        Err(FeynmanPolynomialError::ResourceLimit {
            resource: "aggregate Symbolica determinant ring operations",
            requested: 56,
            limit: 55,
        })
    ));

    let exact = FeynmanPolynomialLimits {
        max_determinant_matrix_entries: 16,
        max_determinant_ring_operations: 56,
        ..FeynmanPolynomialLimits::default()
    };
    let context = matrix_context("feynman-native-four-exact", exact);
    let matrix = symbolic_tridiagonal_four(&context);
    let mut work = FeynmanWorkBudget::new(exact);
    checked_determinant(&context, &matrix, &mut work).unwrap();
    assert_eq!(work.determinant_ring_operations, 56);

    let below_entries = FeynmanPolynomialLimits {
        max_determinant_matrix_entries: 15,
        max_determinant_ring_operations: 56,
        ..FeynmanPolynomialLimits::default()
    };
    let context = matrix_context("feynman-native-four-below-entries", below_entries);
    let matrix = symbolic_tridiagonal_four(&context);
    let mut work = FeynmanWorkBudget::new(below_entries);
    assert!(matches!(
        checked_determinant(&context, &matrix, &mut work),
        Err(FeynmanPolynomialError::ResourceLimit {
            resource: "Symbolica determinant matrix entries",
            requested: 16,
            limit: 15,
        })
    ));
}

#[test]
fn adjugate_uses_transposed_cofactor_indices() {
    let coefficients = CoefficientContext::new(["d"]);
    let family = IntegralFamily::new(
        "feynman-private-adjugate-indexing",
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.zero(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap();
    let limits = FeynmanPolynomialLimits::default();
    let context = FeynmanPolynomialContext::try_new(&family, limits).unwrap();
    let entry = |value| context.constant(coefficients.integer(value)).unwrap();
    let matrix = vec![vec![entry(1), entry(2)], vec![entry(3), entry(4)]];
    let mut work = FeynmanWorkBudget::new(limits);
    let adjugate = checked_adjugate(&context, &matrix, &mut work).unwrap();

    assert_eq!(adjugate[0][0], entry(4));
    assert_eq!(adjugate[0][1], entry(-2));
    assert_eq!(adjugate[1][0], entry(-3));
    assert_eq!(adjugate[1][1], entry(1));
}

#[test]
fn native_outer_arithmetic_and_gradient_retain_the_context_map() {
    let limits = FeynmanPolynomialLimits::default();
    let context = matrix_context("feynman-native-outer-arithmetic", limits);
    let x0 = variable(&context, 0);
    let x1 = variable(&context, 1);
    let mut work = FeynmanWorkBudget::new(limits);

    let sum = context.add(&x0, &x1, &mut work).unwrap();
    let square = context.mul(&sum, &sum, &mut work).unwrap();
    assert_eq!(square.term_count(), 3);
    assert_eq!(
        square.coefficient(&[2, 0, 0, 0, 0]),
        Some(&context.coefficients.one())
    );
    assert_eq!(
        square.coefficient(&[1, 1, 0, 0, 0]),
        Some(&context.coefficients.integer(2))
    );
    assert_eq!(
        square.coefficient(&[0, 2, 0, 0, 0]),
        Some(&context.coefficients.one())
    );
    assert!(Arc::ptr_eq(&square.raw.variables, &context.variables));

    let scaled = context
        .scale(
            &square,
            &context.coefficients.parameter("s").unwrap(),
            &mut work,
        )
        .unwrap();
    assert!(Arc::ptr_eq(&scaled.raw.variables, &context.variables));

    let zero = context.sub(&scaled, &scaled, &mut work).unwrap();
    assert!(zero.is_zero());
    assert!(Arc::ptr_eq(&zero.raw.variables, &context.variables));

    let gradient = context.try_gradient(&square).unwrap();
    assert_eq!(gradient.len(), context.parameter_count());
    for derivative in &gradient {
        assert!(Arc::ptr_eq(&derivative.raw.variables, &context.variables));
        context.authenticate(derivative).unwrap();
    }
    assert_eq!(gradient[0].term_count(), 2);
    assert_eq!(
        gradient[0].coefficient(&[1, 0, 0, 0, 0]),
        Some(&context.coefficients.integer(2))
    );
    assert_eq!(
        gradient[0].coefficient(&[0, 1, 0, 0, 0]),
        Some(&context.coefficients.integer(2))
    );
    assert_eq!(gradient[0], gradient[1]);
    assert!(gradient[2..].iter().all(FeynmanPolynomial::is_zero));
}

#[test]
fn native_product_exponent_overflow_is_rejected_before_symbolica() {
    let limits = FeynmanPolynomialLimits {
        max_parameter_exponent: 1,
        ..FeynmanPolynomialLimits::default()
    };
    let context = matrix_context("feynman-native-product-exponent-limit", limits);
    let x0 = variable(&context, 0);
    let mut work = FeynmanWorkBudget::new(limits);

    assert_eq!(
        context.mul(&x0, &x0, &mut work),
        Err(FeynmanPolynomialError::ParameterExponentOverflow {
            variable: 0,
            requested: 2,
            limit: 1,
        })
    );
}

#[test]
fn native_coefficient_swell_is_rejected_by_post_authentication() {
    use crate::algebra::{ExactAlgebraError, ExactAlgebraLimits};

    let limits = FeynmanPolynomialLimits {
        exact_algebra: ExactAlgebraLimits {
            max_polynomial_terms: 2,
            ..ExactAlgebraLimits::default()
        },
        ..FeynmanPolynomialLimits::default()
    };
    let context = matrix_context("feynman-native-coefficient-post-check", limits);
    let one_plus_d = context.coefficients.coefficient_fixture("1+d");
    let x0 = context.parameter_monomial(0, &one_plus_d).unwrap();
    let mut work = FeynmanWorkBudget::new(limits);

    assert!(matches!(
        context.scale(&x0, &one_plus_d, &mut work),
        Err(FeynmanPolynomialError::ExactAlgebra(
            ExactAlgebraError::ResourceLimit {
                resource: "authenticated polynomial terms",
                requested: 3,
                limit: 2,
            }
        ))
    ));
}
