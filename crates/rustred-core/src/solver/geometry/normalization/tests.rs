use crate::algebra::{CoefficientContext, CoefficientPolynomial};

use super::super::intersect;
use super::{CoordinateCase, GeometryError, normalize};

const PM_EQUATIONS: [&str; 3] = [
    "-32-38*b-14*b^2+33*a+39*a*b+6*a*b^2+3*a^2+3*a^2*b",
    "-31-19*b-2*b^2+34*a+16*a*b+2*a^2",
    "-7-b+8*a",
];

fn equations(context: &CoefficientContext, expressions: &[&str]) -> Vec<CoefficientPolynomial> {
    expressions
        .iter()
        .map(|expression| context.coefficient_fixture(expression).numerator)
        .collect()
}

#[test]
fn actual_fam1_111_joint_exception_has_only_the_coordinate_leaf() {
    let context = CoefficientContext::new(["a", "b"]);
    let expected = Some(CoordinateCase::new([Some(1), Some(1)]).unwrap());
    for order in [[0, 1, 2], [2, 0, 1], [1, 2, 0]] {
        let inputs = equations(&context, &order.map(|i| PM_EQUATIONS[i]));
        assert_eq!(
            intersect(&CoordinateCase::generic(), &inputs, &[0, 1], &[true; 2]).unwrap(),
            expected
        );
        let normalized = normalize(&CoordinateCase::generic(), &inputs, &[0, 1])
            .unwrap()
            .unwrap();
        for equation in equations(&context, &["a-1", "b-1"]) {
            assert!(normalized.contains(&equation));
        }
        assert_eq!(normalized.len(), 2);
    }
}

#[test]
fn interleaved_index_positions_and_original_parent_are_retained() {
    let context = CoefficientContext::new(["b", "d", "c", "ep", "a"]);
    let parent = CoordinateCase::new([None, None, Some(-2)]).unwrap();
    let inputs = equations(&context, &PM_EQUATIONS);
    assert_eq!(
        intersect(&parent, &inputs, &[4, 0, 2], &[true, true, false]).unwrap(),
        Some(CoordinateCase::new([Some(1), Some(1), Some(-2)]).unwrap())
    );
    let normalized = normalize(&parent, &inputs, &[4, 0, 2]).unwrap().unwrap();
    assert!(
        normalized
            .iter()
            .all(|equation| equation.variables() == inputs[0].variables())
    );
}

#[test]
fn inconsistent_joint_ideal_is_exactly_empty_not_a_modular_guess() {
    let context = CoefficientContext::new(["a", "b"]);
    for expressions in [
        vec!["a*b-1", "a*b-2"],
        // These equations agree modulo 65537 but their Q-ideal contains 1.
        vec!["a*b-1", "a*b+65536"],
    ] {
        assert!(
            intersect(
                &CoordinateCase::generic(),
                &equations(&context, &expressions),
                &[0, 1],
                &[true; 2]
            )
            .unwrap()
            .is_none()
        );
    }
}

#[test]
fn large_integer_content_never_changes_the_exact_leaf() {
    let context = CoefficientContext::new(["a", "b"]);
    let inputs = equations(&context, &["65537*a*b-65537", "a*b+a-2"]);
    assert_eq!(
        intersect(&CoordinateCase::generic(), &inputs, &[0, 1], &[true; 2]).unwrap(),
        Some(CoordinateCase::new([Some(1), Some(1)]).unwrap())
    );
}

#[test]
fn positive_dimensional_and_finite_nonlinear_residuals_remain_unsupported() {
    let context = CoefficientContext::new(["a", "b"]);
    for expressions in [
        vec!["a*b-1", "2*a*b-2"],
        vec!["a^2-1", "b^2-1"],
        vec!["2*a*b-1", "2*a*b-1+a-b"],
    ] {
        let inputs = equations(&context, &expressions);
        assert_eq!(
            intersect(&CoordinateCase::generic(), &inputs, &[0, 1], &[true; 2]),
            Err(GeometryError::UnsupportedGeometry { equations: inputs })
        );
    }
}

#[test]
fn primitive_conversion_preserves_fractional_basis_equations() {
    let context = CoefficientContext::new(["a", "b"]);
    let inputs = equations(&context, &["2*a*b-1", "a-b"]);
    let normalized = normalize(&CoordinateCase::generic(), &inputs, &[0, 1])
        .unwrap()
        .unwrap();
    assert!(normalized.contains(&equations(&context, &["a-b"]).remove(0)));
    assert!(normalized.contains(&equations(&context, &["2*b^2-1"]).remove(0)));
}

#[test]
fn single_equations_and_linear_conjunctions_do_not_invoke_groebner() {
    let context = CoefficientContext::new(["a", "b"]);
    for expressions in [vec!["a^2-1"], vec!["a-b", "2*a-2*b"], vec!["a*b-1", "0"]] {
        let inputs = equations(&context, &expressions);
        assert!(
            normalize(&CoordinateCase::generic(), &inputs, &[0, 1])
                .unwrap()
                .is_none()
        );
        assert_eq!(
            intersect(&CoordinateCase::generic(), &inputs, &[0, 1], &[true; 2]),
            Err(GeometryError::UnsupportedGeometry { equations: inputs })
        );
    }
    assert!(
        normalize(&CoordinateCase::<0>::generic(), &[], &[])
            .unwrap()
            .is_none()
    );
}

#[test]
fn joint_leaf_still_obeys_integer_and_sector_requirements() {
    let context = CoefficientContext::new(["a", "b"]);
    // Exact joint equations imply a=b=1/2, not an integer leaf.
    let half = equations(&context, &["2*a*b-b", "2*a*b-a", "4*a*b-1"]);
    assert!(
        intersect(&CoordinateCase::generic(), &half, &[0, 1], &[true; 2])
            .unwrap()
            .is_none()
    );
    assert!(
        intersect(
            &CoordinateCase::generic(),
            &equations(&context, &PM_EQUATIONS),
            &[0, 1],
            &[true, false]
        )
        .unwrap()
        .is_none()
    );
}
