use super::*;
use crate::algebra::CoefficientContext;

#[test]
fn integral_nontrivial_chart_and_fixed_overflow_are_explicit() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let case = affine::<2>(&context, &["n0-2*n1-1"]);
    assert_eq!(
        case.specialize(&equations(&context, &["n0"]).remove(0))
            .unwrap(),
        equations(&context, &["2*n1+1"]).remove(0)
    );
    assert!(case.is_tangent(&[2, 1]));
    assert!(!case.is_tangent(&[1, 1]));
    assert!(matches!(
        case.intersect(&equations(&context, &["n1-40"]), &[true; 2]),
        Err(AffineGeometryError::Coordinate(
            GeometryError::CompactOverflow { axis: 0, .. }
        ))
    ));
}
fn equations(context: &CoefficientContext, expressions: &[&str]) -> Vec<CoefficientPolynomial> {
    expressions
        .iter()
        .map(|expression| context.coefficient_fixture(expression).numerator)
        .collect()
}

fn affine<const N: usize>(context: &CoefficientContext, expressions: &[&str]) -> AffineCase<N> {
    let result = AffineCase::from_coordinate(
        &CoordinateCase::generic(),
        &equations(context, expressions),
        &std::array::from_fn(|i| i),
        &[true; N],
    )
    .unwrap();
    let AffineIntersection::Affine(result) = result else {
        panic!("expected affine case")
    };
    result
}

#[test]
fn equal_indices_have_exact_integral_chart_and_tangent_targets() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let case = affine::<2>(&context, &["3*n0-3*n1"]);
    assert_eq!(case.equations(), equations(&context, &["n0-n1"]));
    assert_eq!(
        case.substitutions()[0],
        (0, equations(&context, &["n1"]).remove(0))
    );
    assert!(case.is_tangent(&[1, 1]));
    assert!(case.is_tangent(&[-1, -1]));
    assert!(!case.is_tangent(&[1, 0]));
    assert!(case.matches(&Integral::symbolic([1, 1]).unwrap()));
    assert!(!case.matches(&Integral::symbolic([1, 0]).unwrap()));
}

#[test]
fn intersection_fixes_both_equal_coordinates() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let case = affine::<2>(&context, &["n0-n1"]);
    assert_eq!(
        case.intersect(&equations(&context, &["n1-2"]), &[true; 2])
            .unwrap(),
        AffineIntersection::Coordinate(CoordinateCase::new([Some(2), Some(2)]).unwrap())
    );
    assert_eq!(
        case.intersect(&equations(&context, &["n1"]), &[true; 2])
            .unwrap(),
        AffineIntersection::Empty
    );
}

#[test]
fn divisibility_rejects_vacuous_reference_rule() {
    let context = CoefficientContext::new(["n0", "n1"]);
    assert_eq!(
        AffineCase::from_coordinate(
            &CoordinateCase::generic(),
            &equations(&context, &["2*n0-2*n1-1"]),
            &[0, 1],
            &[true; 2]
        )
        .unwrap(),
        AffineIntersection::Empty
    );
}

#[test]
fn native_elimination_finds_coordinates_and_contradictions() {
    let context = CoefficientContext::new(["n0", "n1"]);
    for (expressions, expected) in [
        (
            vec!["n0+n1-4", "n0-n1"],
            AffineIntersection::Coordinate(CoordinateCase::new([Some(2), Some(2)]).unwrap()),
        ),
        (vec!["n0+n1-1", "n0-n1"], AffineIntersection::Empty),
        (vec!["n0-n1", "n0-n1-1"], AffineIntersection::Empty),
    ] {
        assert_eq!(
            AffineCase::from_coordinate(
                &CoordinateCase::generic(),
                &equations(&context, &expressions),
                &[0, 1],
                &[true; 2]
            )
            .unwrap(),
            expected
        );
    }
}

#[test]
fn rational_chart_does_not_claim_integer_feasibility() {
    let context = CoefficientContext::new(["n0", "n1", "n2"]);
    // Each original row has gcd 1, but their conjunction has no integer point.
    let result = AffineCase::from_coordinate(
        &CoordinateCase::generic(),
        &equations(&context, &["2*n0-n2", "2*n1-n2-1"]),
        &[0, 1, 2],
        &[true; 3],
    );
    assert!(matches!(
        result,
        Err(AffineGeometryError::UnsupportedCongruence { .. })
    ));
    // This one is feasible with n1 even, but not in the admitted chart class.
    assert!(matches!(
        AffineCase::from_coordinate(
            &CoordinateCase::generic(),
            &equations(&context, &["2*n0-n1"]),
            &[0, 1],
            &[true; 2]
        ),
        Err(AffineGeometryError::UnsupportedCongruence { .. })
    ));
}

#[test]
fn shifted_sources_are_projected_after_translation() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let case = affine::<2>(&context, &["n0-n1"]);
    let source = equations(&context, &["n0-n1"]).remove(0);
    assert!(case.specialize(&source).unwrap().is_zero());
    assert_eq!(
        case.specialize(&source.shift_var(0, &Integer::one()))
            .unwrap(),
        source.one()
    );
}

#[test]
fn parent_chart_simplifies_nonlinear_guard_before_intersection() {
    let context = CoefficientContext::new(["n0", "n1", "n2"]);
    let parent = affine::<3>(&context, &["n0-n1"]);
    let result = parent
        .intersect(&equations(&context, &["(n0-n1)*n2+n2-1"]), &[true; 3])
        .unwrap();
    let AffineIntersection::Affine(child) = result else {
        panic!("expected affine child")
    };
    assert_eq!(child.face().fixed(), &[None, None, Some(1)]);
    assert!(parent.contains_affine(&child).unwrap());
    assert!(!child.contains_affine(&parent).unwrap());
    assert!(!child.is_tangent(&[1, 1, 1]));
}

#[test]
fn canonical_equality_is_independent_of_input_basis_and_order() {
    let context = CoefficientContext::new(["n0", "n1", "n2"]);
    let first = affine::<3>(&context, &["n0-n1", "n1-n2"]);
    let second = affine::<3>(&context, &["-2*n0+2*n2", "3*n1-3*n2", "n0-n1"]);
    assert_eq!(first, second);
    assert!(first.contains_affine(&second).unwrap());
    assert!(first.contains_coordinate(&CoordinateCase::new([Some(1); 3]).unwrap()));
    assert!(!first.contains_coordinate(&CoordinateCase::new([Some(1), Some(2), Some(1)]).unwrap()));
}

#[test]
fn interleaved_variable_map_preserves_axis_identity() {
    let context = CoefficientContext::new(["n1", "d", "n0"]);
    let result = AffineCase::from_coordinate(
        &CoordinateCase::generic(),
        &equations(&context, &["n0-n1-1"]),
        &[2, 0],
        &[true; 2],
    )
    .unwrap();
    let AffineIntersection::Affine(case) = result else {
        panic!("expected affine")
    };
    assert_eq!(case.substitutions()[0].0, 2);
    assert_eq!(
        case.specialize(&equations(&context, &["d*(n0-n1)"]).remove(0))
            .unwrap(),
        equations(&context, &["d"]).remove(0)
    );
}

#[test]
fn zero_constraints_take_coordinate_path_and_true_nonlinearity_fails_closed() {
    let context = CoefficientContext::new(["n0", "n1"]);
    assert_eq!(
        AffineCase::<0>::from_coordinate(&CoordinateCase::generic(), &[], &[], &[]).unwrap(),
        AffineIntersection::Coordinate(CoordinateCase::generic())
    );
    assert_eq!(
        AffineCase::<2>::from_coordinate(&CoordinateCase::generic(), &[], &[0, 1], &[true; 2])
            .unwrap(),
        AffineIntersection::Coordinate(CoordinateCase::generic())
    );
    assert!(matches!(
        AffineCase::from_coordinate(
            &CoordinateCase::generic(),
            &equations(&context, &["n0*n1-2"]),
            &[0, 1],
            &[true; 2]
        ),
        Err(AffineGeometryError::UnsupportedNonlinear { .. })
    ));
    assert!(matches!(
        AffineCase::from_coordinate(
            &CoordinateCase::generic(),
            &equations(&CoefficientContext::new(["n0", "d"]), &["n0+d"]),
            &[0],
            &[true]
        ),
        Err(AffineGeometryError::Coordinate(
            GeometryError::InvalidInput(_)
        ))
    ));
}

#[test]
fn sector_feasibility_is_not_claimed_for_coupled_inequalities() {
    let context = CoefficientContext::new(["n0", "n1"]);
    // Empty in the positive sector, but deliberately retained rather than
    // pretending to have an exact integer-polyhedron feasibility service.
    let case = affine::<2>(&context, &["n0+n1-1"]);
    assert_eq!(case.equations(), equations(&context, &["n0+n1-1"]));
}
