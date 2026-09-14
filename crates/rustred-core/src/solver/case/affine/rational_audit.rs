//! Independent adversarial checks for rational charts over integer domains.

use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::Case;
use std::cmp::Ordering;

fn polynomial(context: &CoefficientContext, expression: &str) -> CoefficientPolynomial {
    context.coefficient_fixture(expression).numerator
}

fn domain<const N: usize>(
    context: &CoefficientContext,
    equations: &[&str],
    indices: [usize; N],
) -> AffineCase<N> {
    let equations = equations
        .iter()
        .map(|equation| polynomial(context, equation))
        .collect::<Vec<_>>();
    let result =
        AffineCase::from_coordinate(&CoordinateCase::generic(), &equations, &indices, &[true; N])
            .unwrap();
    let AffineIntersection::Affine(case) = result else {
        panic!("expected a retained affine equality domain");
    };
    case
}

#[test]
fn incompatible_parities_are_not_mistaken_for_a_numerical_terminal() {
    let context = CoefficientContext::new(["n0", "n1", "n2"]);
    // These two equations have a rational line but no integer point. The
    // conservative representation may retain the line, never certify a point.
    let affine = domain(&context, &["2*n0-n2", "2*n1-n2-1"], [0, 1, 2]);
    let case = Case::from(affine.clone());
    assert!(!case.is_numerical());
    assert_eq!(case.fixed(), &[None; 3]);
    assert!(affine.is_tangent(&[1, 1, 2]));
    assert!(!affine.is_tangent(&[1, 1, 1]));
    // Fixing either parity exposes a noninteger coordinate and proves empty.
    for equation in ["n2-2", "n2-3"] {
        assert_eq!(
            affine
                .intersect(&[polynomial(&context, equation)], &[true; 3])
                .unwrap(),
            AffineIntersection::Empty
        );
    }
}

#[test]
fn exact_rational_value_and_required_domain_pole_remain_distinct() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let case = domain(&context, &["2*n0-n1-4"], [0, 1]);
    let value = context.coefficient_fixture("n0/(n1+1)");
    assert_eq!(
        case.restrict_coefficient(&value).unwrap(),
        context.coefficient_fixture("(n1+4)/(2*n1+2)")
    );
    let pole = context.coefficient_fixture("1/(2*n0-n1-4)");
    assert!(case.restrict_coefficient(&pole).is_err());
    // Clearing content is legitimate for a zero equation, not for its value.
    assert_eq!(
        case.restrict_polynomial_value(&polynomial(&context, "n0-3"))
            .unwrap(),
        context.coefficient_fixture("(n1-2)/2")
    );
    assert_eq!(
        case.intersect(&[polynomial(&context, "n0-3")], &[true; 2])
            .unwrap(),
        AffineIntersection::Coordinate(CoordinateCase::new([Some(3), Some(2)]).unwrap())
    );
}

#[test]
fn multiple_rational_pivots_are_simultaneous_and_preserve_integral_axes() {
    let context = CoefficientContext::new(["n0", "n1", "n2"]);
    let case = domain(&context, &["2*n0-n2-4", "3*n1-n2-6"], [0, 1, 2]);
    assert!(!case.has_integral_chart());
    assert_eq!(
        case.restrict_polynomial_value(&polynomial(&context, "6*n0*n1"))
            .unwrap(),
        context.coefficient_fixture("(n2+4)*(n2+6)")
    );
    assert!(case.is_tangent(&[3, 2, 6]));
    assert!(!case.is_tangent(&[1, 1, 2]));
    assert_eq!(
        Case::from(case.clone()).integral(),
        Integral::symbolic([0; 3]).unwrap()
    );
    assert_eq!(
        case.intersect(&[polynomial(&context, "n2-6")], &[true; 3])
            .unwrap(),
        AffineIntersection::Coordinate(CoordinateCase::new([Some(5), Some(4), Some(6)]).unwrap())
    );
}

#[test]
fn rational_domain_identity_is_independent_of_equation_basis() {
    let context = CoefficientContext::new(["n0", "n1", "n2"]);
    let left = domain(&context, &["2*n0-n2-4", "3*n1-n2-6"], [0, 1, 2]);
    let right = domain(
        &context,
        &["6*n1-2*n2-12", "-4*n0+2*n2+8", "2*n0+3*n1-2*n2-10"],
        [0, 1, 2],
    );
    assert_eq!(left, right);
    assert!(left.contains_affine(&right).unwrap());
    assert!(right.contains_affine(&left).unwrap());
    let left = Case::from(left);
    let right = Case::from(right);
    assert_eq!(left.queue_cmp(&right), Ordering::Equal);
}

#[test]
fn primitive_integer_rows_determine_queue_order_not_monic_rational_rows() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let left = Case::from(domain(&context, &["2*n0-n1-4"], [0, 1]));
    let right = Case::from(domain(&context, &["3*n0-2*n1-6"], [0, 1]));
    // Primitive leading coefficients 2 < 3 decide this. Comparing monic
    // RREFs would instead compare -1/2 > -2/3 and reverse the queue order.
    assert_eq!(left.queue_cmp(&right), Ordering::Less);
    assert_eq!(right.queue_cmp(&left), Ordering::Greater);
}

#[test]
fn rational_chart_respects_nonprefix_maps_and_generic_parameters() {
    let context = CoefficientContext::new(["n1", "d", "n2", "n0"]);
    let case = domain(&context, &["2*n0-n2-4", "3*n1-n2-6"], [3, 0, 2]);
    let value = case
        .restrict_polynomial_value(&polynomial(&context, "d*n0-n1"))
        .unwrap();
    assert_eq!(value, context.coefficient_fixture("d*(n2+4)/2-(n2+6)/3"));
    assert_eq!(
        value.numerator.variables(),
        context.one().numerator.variables()
    );
    assert!(
        case.restrict_equation(&polynomial(&context, "2*n0-n2-4"))
            .unwrap()
            .is_zero()
    );
}
