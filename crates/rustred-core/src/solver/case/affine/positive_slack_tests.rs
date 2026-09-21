//! Exact integer endpoint consequences of a finite, possibly nonzero slack.

use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{Case, CaseIntersectionFailure, CaseIntersectionLimits};

fn equations(context: &CoefficientContext, expressions: &[&str]) -> Vec<CoefficientPolynomial> {
    expressions
        .iter()
        .map(|expression| context.coefficient_fixture(expression).numerator)
        .collect()
}

fn captured() -> (
    CoefficientContext,
    CoordinateCase<15>,
    [usize; 15],
    [bool; 15],
) {
    let context = CoefficientContext::new(
        std::iter::once("d".to_owned()).chain((0..15).map(|axis| format!("n{axis}"))),
    );
    let face = CoordinateCase::new([
        None,
        Some(1),
        Some(2),
        Some(0),
        Some(1),
        Some(1),
        Some(1),
        None,
        None,
        Some(1),
        None,
        Some(1),
        None,
        Some(0),
        None,
    ])
    .unwrap();
    (
        context,
        face,
        std::array::from_fn(|axis| axis + 1),
        std::array::from_fn(|axis| b"011011100101000"[axis] == b'1'),
    )
}

#[test]
fn captured_narrowed_parent_is_empty_before_any_resultant_work() {
    let (context, face, indices, sector) = captured();
    let affine = equations(
        &context,
        &[
            "2+3*n14+n7+2*n0",
            "9+12*n14+5*n8",
            "2-9*n14+5*n10",
            "6+8*n14+5*n12",
        ],
    );
    let AffineIntersection::Affine(declared) =
        AffineCase::from_coordinate(&face, &affine, &indices, &sector).unwrap()
    else {
        panic!("declared chart must remain unchanged at import");
    };
    let parent: Case<15> = declared.into();
    let residual = equations(
        &context,
        &[
            "-34-89*n14-36*n14^2+75*n7",
            "638+1759*n14-321*n7+6102*n7*n14-4374*n7^2+648*n7^2*n14",
            "3378+10009*n14-4047*n7+32694*n7*n14-22590*n7^2+1080*n7^3",
        ],
    );
    let limits = CaseIntersectionLimits {
        max_work_items: 8192,
        ..Default::default()
    };
    for scoped in [false, true] {
        let result = if scoped {
            parent.intersect_many_with_max_numerator_rank(&residual, &indices, &sector, limits, 10)
        } else {
            parent.intersect_many(&residual, &indices, &sector, limits)
        }
        .unwrap();
        assert!(result.cases.is_empty());
        assert_eq!(result.stats.integer_resultants, 0);
        assert_eq!(result.stats.integer_divisors, 0);
        assert_eq!(result.stats.normalizations, 0);
    }
    assert_eq!(
        parent.fixed(),
        face.fixed(),
        "stored chart remains immutable"
    );
}

#[test]
fn wider_original_parent_keeps_its_exact_integer_witness() {
    let (context, face, indices, sector) = captured();
    let affine = equations(
        &context,
        &["2+3*n14+n7+2*n0", "3+4*n14+n12+n8", "-2-5*n14-2*n12+n10"],
    );
    let AffineIntersection::Affine(declared) =
        AffineCase::from_coordinate(&face, &affine, &indices, &sector).unwrap()
    else {
        panic!("expected original declared affine chart");
    };
    let parent: Case<15> = declared.into();
    let point: Case<15> =
        CoordinateCase::new([-1, 1, 2, 0, 1, 1, 1, 0, -2, 1, 0, 1, -1, 0, 0].map(Some))
            .unwrap()
            .into();
    assert!(parent.contains(&point).unwrap());
    let refined = parent.intersect(&[], &indices, &sector).unwrap().unwrap();
    assert!(refined.contains(&point).unwrap());
    assert_eq!(refined.fixed()[14], Some(0));
    assert_eq!(parent.fixed()[14], None);
}

#[test]
fn partial_endpoint_masks_use_strict_comparison_and_native_large_integers() {
    let face = CoordinateCase::<2>::generic();
    for (row, expected) in [
        ([12, 5, -9], bounds::RowBounds::Endpoints([true, false])),
        ([10, 3, -6], bounds::RowBounds::Endpoints([true, false])),
        ([10, 3, -10], bounds::RowBounds::Unresolved),
        ([-12, 5, 9], bounds::RowBounds::Unresolved),
    ] {
        let row = row.map(Integer::from);
        assert_eq!(bounds::classify(&row, &face, &[false; 2]), expected);
        assert_eq!(
            bounds::classify(&row.each_ref().map(|v| -v), &face, &[false; 2]),
            expected
        );
    }
    let big = Integer::from(10).pow(80) + Integer::from(7);
    let row = [big.clone(), Integer::one(), -(&big - Integer::one())];
    assert_eq!(
        bounds::classify(&row, &face, &[false; 2]),
        bounds::RowBounds::Endpoints([true, false])
    );
    let row = [big.clone(), Integer::one(), -&big];
    assert_eq!(
        bounds::classify(&row, &face, &[false; 2]),
        bounds::RowBounds::Unresolved
    );
    let fixed = CoordinateCase::new([Some(-3), None, None, None]).unwrap();
    let row = [
        big.clone(),
        Integer::from(10),
        Integer::zero(),
        Integer::from(3),
        -(&big * Integer::from(3)) - Integer::from(6),
    ];
    assert_eq!(
        bounds::classify(&row, &fixed, &[false; 4]),
        bounds::RowBounds::Endpoints([false, true, false, false])
    );
}

#[test]
fn positive_slack_preserves_declared_chart_and_refines_only_search_domain() {
    let context = CoefficientContext::new(["x", "y"]);
    let AffineIntersection::Affine(declared) = AffineCase::from_coordinate(
        &CoordinateCase::generic(),
        &equations(&context, &["10*x+3*y+6"]),
        &[0, 1],
        &[false; 2],
    )
    .unwrap() else {
        panic!("preserve declared symbolic chart");
    };
    let stored: Case<2> = declared.into();
    let symbolic_target = stored.integral();
    assert_eq!(stored.fixed(), &[None; 2]);
    for rank in [None, Some(10)] {
        let refined = stored
            .intersect_in_rank(&[], &[0, 1], &[false; 2], rank)
            .unwrap()
            .unwrap();
        assert_eq!(refined.fixed(), &[Some(0), Some(-2)]);
        assert!(refined.is_numerical());
    }
    assert_eq!(stored.integral(), symbolic_target);
    assert_eq!(stored.fixed(), &[None; 2]);
}

#[test]
fn active_and_mixed_endpoints_respect_equal_slack_and_unbounded_cancellation() {
    let context = CoefficientContext::new(["x", "y"]);
    for (expression, sector, expected) in [
        ("12*x+5*y-26", [true, true], None),
        ("-12*x-5*y+26", [true, true], None),
        ("10*x+3*y-19", [true, true], Some([Some(1), Some(3)])),
        ("10*x-3*y-16", [true, false], Some([Some(1), Some(-2)])),
    ] {
        let result = Case::<2>::generic()
            .intersect(&equations(&context, &[expression]), &[0, 1], &sector)
            .unwrap();
        assert_eq!(result.as_ref().map(|case| *case.fixed()), expected);
    }
    for (expression, witness) in [("10*x+3*y+10", [-1, 0]), ("-12*x+5*y-9", [-2, -3])] {
        let result = Case::<2>::generic()
            .intersect(&equations(&context, &[expression]), &[0, 1], &[false; 2])
            .unwrap()
            .unwrap();
        let point: Case<2> = CoordinateCase::new(witness.map(Some)).unwrap().into();
        assert!(result.contains(&point).unwrap());
        assert_eq!(result.fixed(), &[None; 2]);
    }
}

#[test]
fn repeated_positive_slack_retains_all_rows_and_unused_coordinates() {
    let context = CoefficientContext::new(["x", "y", "z", "w"]);
    for input in [["10*x+3*y+6", "z-y"], ["z-y", "10*x+3*y+6"]] {
        let result = Case::<4>::generic()
            .intersect(&equations(&context, &input), &[0, 1, 2, 3], &[false; 4])
            .unwrap()
            .unwrap();
        assert_eq!(result.fixed(), &[Some(0), Some(-2), Some(-2), None]);
    }
    assert!(
        Case::<4>::generic()
            .intersect(
                &equations(&context, &["10*x+3*y+6", "z-y", "z-y-1"]),
                &[0, 1, 2, 3],
                &[false; 4],
            )
            .unwrap()
            .is_none()
    );
}

#[test]
fn positive_slack_empty_branch_preserves_valid_siblings_and_atomic_errors() {
    let context = CoefficientContext::new(["x", "y", "d"]);
    let input = equations(&context, &["(12*x+5*y+9)*(x+y+1)"]);
    let result = Case::<2>::generic()
        .intersect_many(&input, &[0, 1], &[false; 2], Default::default())
        .unwrap();
    for (key, expected) in [([0, -1], true), ([-1, 0], true), ([0, 0], false)] {
        let point: Case<2> = CoordinateCase::new(key.map(Some)).unwrap().into();
        assert_eq!(
            result
                .cases
                .iter()
                .any(|case| case.contains(&point).unwrap()),
            expected
        );
    }
    let input = equations(&context, &["(12*x+5*y+9)*(x^2-2*y^2-1)"]);
    let error = Case::<2>::generic()
        .intersect_many(&input, &[0, 1], &[false; 2], Default::default())
        .unwrap_err();
    assert_eq!(error.failure, CaseIntersectionFailure::UnsupportedGeometry);
    assert_eq!(error.original_conjunction.as_ref(), input);
    assert!(matches!(
        AffineCase::from_coordinate_in_rank(
            &CoordinateCase::generic(),
            &equations(&context, &["12*x+5*y+9", "d*x+y"]),
            &[0, 1],
            &[false; 2],
            None,
        ),
        Err(AffineGeometryError::Coordinate(
            GeometryError::InvalidInput(_)
        ))
    ));
}
