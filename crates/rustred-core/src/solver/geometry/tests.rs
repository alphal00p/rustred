use std::cmp::Ordering;

use crate::algebra::CoefficientContext;

use super::{CoordinateCase, GeometryError, compare_cases, contains, intersect};

fn equations(
    context: &CoefficientContext,
    expressions: &[&str],
) -> Vec<crate::algebra::CoefficientPolynomial> {
    expressions
        .iter()
        .map(|expression| context.coefficient_fixture(expression).numerator)
        .collect()
}

#[test]
fn coordinate_intersections_specialize_to_a_fixed_point() {
    let context = CoefficientContext::new(["d", "n0", "n1", "n2"]);
    let parent = CoordinateCase::new([None, None, Some(3)]).unwrap();
    let eqs = equations(&context, &["n0-n1", "2*n1-n2-1"]);
    assert_eq!(
        intersect(&parent, &eqs, &[1, 2, 3], &[true; 3]).unwrap(),
        Some(CoordinateCase::new([Some(2), Some(2), Some(3)]).unwrap())
    );
}

#[test]
fn unsupported_equations_are_revisited_after_later_constraints() {
    let context = CoefficientContext::new(["n0", "n1"]);
    for expressions in [vec!["n0*n1-2", "n1-1"], vec!["n1-1", "n0*n1-2"]] {
        let eqs = equations(&context, &expressions);
        assert_eq!(
            intersect(&CoordinateCase::generic(), &eqs, &[0, 1], &[true; 2]).unwrap(),
            Some(CoordinateCase::new([Some(2), Some(1)]).unwrap())
        );
    }
}

#[test]
fn contradictions_win_over_initially_unsupported_geometry() {
    let context = CoefficientContext::new(["n0", "n1"]);
    for expressions in [
        vec!["n0*n1-1", "1"],
        vec!["n0*n1-1", "n1"],
        vec!["n0-n1", "n0-2", "n1-1"],
        vec!["n0^2-4", "n0-1"],
        vec!["n0-n1", "n0-1", "2*n1-3"],
    ] {
        assert!(
            intersect(
                &CoordinateCase::generic(),
                &equations(&context, &expressions),
                &[0, 1],
                &[true; 2]
            )
            .unwrap()
            .is_none(),
            "{expressions:?}"
        );
    }
}

#[test]
fn integer_roots_use_native_exact_divisibility_and_sector_signs() {
    let context = CoefficientContext::new(["n0"]);
    for (expression, active, expected) in [
        ("2*n0-3", true, None),
        ("-3*n0+6", true, Some(2)),
        ("3*n0+6", false, Some(-2)),
        ("n0", false, Some(0)),
        ("n0", true, None),
        ("n0+1", true, None),
        ("n0-1", false, None),
    ] {
        let actual = intersect(
            &CoordinateCase::generic(),
            &equations(&context, &[expression]),
            &[0],
            &[active],
        )
        .unwrap();
        assert_eq!(
            actual,
            expected.map(|value| CoordinateCase::new([Some(value)]).unwrap()),
            "{expression} in sector {active}"
        );
    }
}

#[test]
fn true_false_and_inconsistent_parent_cases() {
    let context = CoefficientContext::new(["n0"]);
    let parent = CoordinateCase::new([Some(1)]).unwrap();
    for expressions in [&[][..], &["0"][..], &["n0-1", "0"][..]] {
        assert_eq!(
            intersect(&parent, &equations(&context, expressions), &[0], &[true]).unwrap(),
            Some(parent)
        );
    }
    assert!(
        intersect(&parent, &equations(&context, &["1"]), &[0], &[true])
            .unwrap()
            .is_none()
    );
    assert!(intersect(&parent, &[], &[0], &[false]).unwrap().is_none());
}

#[test]
fn affine_and_nonlinear_residuals_retain_original_exact_conjunction() {
    let context = CoefficientContext::new(["n0", "n1", "n2"]);
    for expressions in [
        vec!["n0-n1", "n2-1"],
        vec!["n0*n1-1", "n2-1"],
        vec!["n0^2-1", "n2-1"],
    ] {
        let original = equations(&context, &expressions);
        assert_eq!(
            intersect(
                &CoordinateCase::generic(),
                &original,
                &[0, 1, 2],
                &[true; 3]
            ),
            Err(GeometryError::UnsupportedGeometry {
                equations: original
            })
        );
    }
}

#[test]
fn reversed_interleaved_index_positions_preserve_coordinate_identity() {
    let context = CoefficientContext::new(["n1", "d", "n0"]);
    assert_eq!(
        intersect(
            &CoordinateCase::generic(),
            &equations(&context, &["n0+3", "n1-2"]),
            &[2, 0],
            &[false, true]
        )
        .unwrap(),
        Some(CoordinateCase::new([Some(-3), Some(2)]).unwrap())
    );
}

#[test]
fn feasible_large_roots_are_typed_overflow_not_empty_cases() {
    let context = CoefficientContext::new(["n0"]);
    for (expression, active) in [
        ("n0-64", true),
        ("n0+65", false),
        ("n0-18446744073709551616", true),
    ] {
        assert!(matches!(
            intersect(
                &CoordinateCase::generic(),
                &equations(&context, &[expression]),
                &[0],
                &[active]
            ),
            Err(GeometryError::CompactOverflow { axis: 0, .. })
        ));
    }
    for expression in ["n0-63", "n0+64"] {
        assert!(
            intersect(
                &CoordinateCase::generic(),
                &equations(&context, &[expression]),
                &[0],
                &[expression == "n0-63"]
            )
            .unwrap()
            .is_some()
        );
    }
}

#[test]
fn large_roots_still_expose_contradictions_before_compact_conversion() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let huge = "n0-18446744073709551616";
    for expressions in [vec![huge, "n0-1"], vec!["n0*n1-1", huge, "n1-1"]] {
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
    assert!(
        intersect(
            &CoordinateCase::generic(),
            &equations(&context, &[huge]),
            &[0, 1],
            &[false; 2]
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn invalid_maps_parameters_and_arrays_are_rejected() {
    let context = CoefficientContext::new(["d", "n0", "n1"]);
    let parent = CoordinateCase::generic();
    let eqs = equations(&context, &["n0-1"]);
    for indices in [[1, 1], [1, 3]] {
        assert!(matches!(
            intersect(&parent, &eqs, &indices, &[true; 2]),
            Err(GeometryError::InvalidInput(_))
        ));
    }
    assert!(matches!(
        intersect(
            &parent,
            &equations(&context, &["d*n0-1"]),
            &[1, 2],
            &[true; 2]
        ),
        Err(GeometryError::InvalidInput(_))
    ));
    let mut malformed = eqs;
    malformed[0].exponents.pop();
    assert!(matches!(
        intersect(&parent, &malformed, &[1, 2], &[true; 2]),
        Err(GeometryError::InvalidInput(_))
    ));
}

#[test]
fn containment_means_reverse_implication_and_does_not_compare_order() {
    let broad = CoordinateCase::new([Some(1), None]).unwrap();
    let narrow = CoordinateCase::new([Some(1), Some(2)]).unwrap();
    let disjoint = CoordinateCase::new([Some(2), None]).unwrap();
    assert!(contains(&CoordinateCase::generic(), &broad));
    assert!(contains(&broad, &narrow));
    assert!(contains(&broad, &broad));
    assert!(!contains(&narrow, &broad));
    assert!(!contains(&broad, &disjoint));
}

#[test]
fn case_order_matches_cpp_constraint_axis_then_negative_constant_order() {
    let ordered = [
        [None, None, None],
        [Some(2), None, None],
        [Some(1), None, None],
        [None, Some(1), None],
        [None, None, Some(1)],
        [Some(1), Some(2), None],
        [Some(1), Some(1), None],
        [Some(2), None, Some(1)],
        [None, Some(1), Some(1)],
        [Some(1), Some(1), Some(1)],
    ]
    .map(|fixed| CoordinateCase::new(fixed).unwrap());
    for (i, left) in ordered.iter().enumerate() {
        for (j, right) in ordered.iter().enumerate() {
            assert_eq!(compare_cases(left, right), i.cmp(&j));
        }
    }
    assert_eq!(
        compare_cases(&CoordinateCase::<0>::generic(), &CoordinateCase::generic()),
        Ordering::Equal
    );
}

#[test]
fn zero_index_case_needs_no_coefficient_variable_map() {
    let context = CoefficientContext::new(std::iter::empty::<&str>());
    let parent = CoordinateCase::<0>::generic();
    assert_eq!(intersect(&parent, &[], &[], &[]).unwrap(), Some(parent));
    assert_eq!(
        intersect(&parent, &equations(&context, &["0"]), &[], &[]).unwrap(),
        Some(parent)
    );
    assert!(
        intersect(&parent, &equations(&context, &["1"]), &[], &[])
            .unwrap()
            .is_none()
    );
}
