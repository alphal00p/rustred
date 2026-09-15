use super::*;
use crate::algebra::CoefficientContext;

#[test]
fn integral_nontrivial_chart_and_fixed_overflow_are_explicit() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let case = affine::<2>(&context, &["n0-2*n1-1"]);
    assert_eq!(
        case.restrict_polynomial_value(&equations(&context, &["n0"]).remove(0))
            .unwrap(),
        context.coefficient_fixture("2*n1+1")
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
    assert!(case.has_integral_chart());
    assert_eq!(
        case.restrict_polynomial_value(&equations(&context, &["n0"]).remove(0))
            .unwrap(),
        context.coefficient_fixture("n1")
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
fn primitive_rows_prove_empty_positive_sector_without_rectangularizing() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let result = AffineCase::from_coordinate(
        &CoordinateCase::generic(),
        &equations(&context, &["n0+n1+1"]),
        &[0, 1],
        &[false, false],
    )
    .unwrap();
    let AffineIntersection::Affine(case) = result else {
        panic!("the coupled equality must remain affine");
    };
    // Positive powers cannot satisfy n0+n1+1=0.  This is an exact primitive
    // row/sector contradiction and is safe for source-port omission; unlike
    // an unresolved affine case it does not claim any box or global cover.
    assert!(case.is_proved_empty_in_sector(&[true, true]));
    assert!(!case.is_proved_empty_in_sector(&[false, true]));
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
    // Integer feasibility is unresolved, not asserted by the computational chart.
    let AffineIntersection::Affine(case) = result.unwrap() else {
        panic!("expected conservatively retained integer equalities")
    };
    assert!(!case.has_integral_chart());
    for value in [1, 2] {
        assert_eq!(
            case.intersect(&equations(&context, &[&format!("n2-{value}")]), &[true; 3])
                .unwrap(),
            AffineIntersection::Empty
        );
    }
    // This one is feasible only with n1 even; that restriction stays implicit
    // in the exact original-coordinate equality, never dropped from the case.
    let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
        &CoordinateCase::generic(),
        &equations(&context, &["2*n0-n1"]),
        &[0, 1],
        &[true; 2],
    )
    .unwrap() else {
        panic!("expected rational computational chart")
    };
    assert!(!case.has_integral_chart());
    assert_eq!(case.equations(), equations(&context, &["2*n0-n1"]));
}

#[test]
fn shifted_sources_are_projected_after_translation() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let case = affine::<2>(&context, &["n0-n1"]);
    let source = equations(&context, &["n0-n1"]).remove(0);
    assert!(case.restrict_polynomial_value(&source).unwrap().is_zero());
    assert_eq!(
        case.restrict_polynomial_value(&source.shift_var(0, &Integer::one()))
            .unwrap(),
        context.one()
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
    assert!(case.has_integral_chart());
    assert_eq!(
        case.restrict_polynomial_value(&equations(&context, &["d*(n0-n1)"]).remove(0))
            .unwrap(),
        context.coefficient_fixture("d")
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
    let context = CoefficientContext::new(["n0", "n1", "n2", "n3"]);
    // Their sum implies n0+n1=-1, impossible in this sector. Each native RREF
    // row separately has both infinite bounds, so our necessary row test must
    // retain this unresolved conjunction, not pretend to solve a full LP.
    let case = affine::<4>(&context, &["n0-n2+n3+1", "n1+n2-n3"]);
    assert_eq!(case.equations().len(), 2);
}

#[test]
fn sector_sign_bounds_reject_empty_affine_branches_without_sampling() {
    let context = CoefficientContext::new(["n0", "n1"]);
    for (equation, sector) in [
        ("n0-n1", [false, true]),
        ("n0-n1", [true, false]),
        ("n0+n1-1", [true, true]),
        ("n0+n1-1", [false, false]),
        ("-2*n0-3*n1+4", [true, true]),
    ] {
        assert_eq!(
            AffineCase::from_coordinate(
                &CoordinateCase::generic(),
                &equations(&context, &[equation]),
                &[0, 1],
                &sector,
            )
            .unwrap(),
            AffineIntersection::Empty,
            "{equation} in {sector:?}",
        );
    }
    // No endpoint is replaced by Power's compact numerical range.
    assert!(matches!(
        AffineCase::from_coordinate(
            &CoordinateCase::generic(),
            &equations(&context, &["n0+n1-100000000000000000000000000000000"]),
            &[0, 1],
            &[true; 2],
        )
        .unwrap(),
        AffineIntersection::Affine(_),
    ));
}

#[test]
fn empty_intersection_rechecks_affine_sign_bounds_for_the_requested_sector() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let case = crate::solver::Case::from(affine::<2>(&context, &["n0-n1"]));
    assert!(case.intersect(&[], &[0, 1], &[true; 2]).unwrap().is_some());
    assert!(case.intersect(&[], &[0, 1], &[false; 2]).unwrap().is_some());
    assert!(
        case.intersect(&[], &[0, 1], &[false, true])
            .unwrap()
            .is_none()
    );
}

#[test]
fn rational_chart_keeps_exact_scale_and_primitive_integer_geometry_separate() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let case = affine::<2>(&context, &["2*n0-n1-4"]);
    assert!(!case.has_integral_chart());
    assert_eq!(case.equations(), equations(&context, &["2*n0-n1-4"]));
    assert_eq!(
        case.primitive_matrix().row_iter().next().unwrap(),
        &[Integer::from(2), Integer::from(-1), Integer::from(4)]
    );
    assert!(case.is_tangent(&[1, 2]));
    assert!(!case.is_tangent(&[1, 1]));
    for (input, output) in [("n0", "(n1+4)/2"), ("n0^2", "(n1+4)^2/4"), ("0", "0")] {
        assert_eq!(
            case.restrict_polynomial_value(&equations(&context, &[input]).remove(0))
                .unwrap(),
            context.coefficient_fixture(output)
        );
    }
    assert_eq!(
        case.restrict_coefficient(&context.coefficient_fixture("n0/(n0+1)"))
            .unwrap(),
        context.coefficient_fixture("(n1+4)/(n1+6)")
    );
    assert_eq!(
        case.restrict_equation(&equations(&context, &["n0-3"]).remove(0))
            .unwrap(),
        equations(&context, &["n1-2"]).remove(0)
    );
    assert!(
        case.restrict_equation(&equations(&context, &["2*n0-n1-4"]).remove(0))
            .unwrap()
            .is_zero()
    );
    assert!(matches!(
        case.restrict_coefficient(&context.coefficient_fixture("1/(2*n0-n1-4)")),
        Err(AffineGeometryError::UndefinedCoefficient)
    ));
}

#[test]
fn rational_chart_preserves_transverse_source_translation_and_integer_corners() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let case = affine::<2>(&context, &["2*n0-n1-4"]);
    let source = equations(&context, &["2*n0-n1-4"]).remove(0);
    assert!(case.restrict_polynomial_value(&source).unwrap().is_zero());
    assert_eq!(
        case.restrict_polynomial_value(&source.shift_var(0, &Integer::one()))
            .unwrap(),
        context.integer(2)
    );
    assert_eq!(
        case.intersect(&equations(&context, &["n1-1"]), &[true; 2])
            .unwrap(),
        AffineIntersection::Empty
    );
    assert_eq!(
        case.intersect(&equations(&context, &["n1-2"]), &[true; 2])
            .unwrap(),
        AffineIntersection::Coordinate(CoordinateCase::new([Some(3), Some(2)]).unwrap())
    );
    assert_eq!(
        case,
        affine::<2>(&context, &["-6*n0+3*n1+12", "4*n0-2*n1-8"])
    );
}
