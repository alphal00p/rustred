//! Sector half-lines can turn a coupled equality into exact coordinate faces.

use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::Case;

fn equations(context: &CoefficientContext, expressions: &[&str]) -> Vec<CoefficientPolynomial> {
    expressions
        .iter()
        .map(|expression| context.coefficient_fixture(expression).numerator)
        .collect()
}

#[test]
fn captured_fifteen_axis_case_is_one_rank_zero_corner() {
    let context = CoefficientContext::new(
        std::iter::once("d".to_owned()).chain((0..15).map(|axis| format!("n{axis}"))),
    );
    let indices = std::array::from_fn(|axis| axis + 1);
    let parent = CoordinateCase::new([
        None,
        None,
        Some(1),
        None,
        None,
        Some(1),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(1),
        Some(0),
        Some(1),
        Some(1),
        Some(1),
    ])
    .unwrap();
    let sector = std::array::from_fn(|axis| matches!(axis, 1 | 2 | 5 | 10 | 12 | 13 | 14));
    let expected =
        CoordinateCase::new([0, 1, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0, 1, 1, 1].map(Some)).unwrap();
    for expression in ["1+n0-n1+n3+n4", "-1-n0+n1-n3-n4"] {
        for rank in [None, Some(0), Some(10)] {
            assert_eq!(
                AffineCase::from_coordinate_in_rank(
                    &parent,
                    &equations(&context, &[expression]),
                    &indices,
                    &sector,
                    rank,
                )
                .unwrap(),
                AffineIntersection::Coordinate(expected),
            );
        }
    }
}

#[test]
fn lower_upper_and_huge_coefficient_endpoints_are_exact() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    for (expression, sector, expected) in [
        ("2*x+3*y-5", [true, true, false], [Some(1), Some(1), None]),
        ("-2*x-3*y+5", [true, true, true], [Some(1), Some(1), None]),
        ("2*x+3*y", [false, false, true], [Some(0), Some(0), None]),
        ("x-7*y+7", [false, true, false], [Some(0), Some(1), None]),
        (
            "100000000000000000000000000000000000000000000000000000000000000000000000000000007*x+5*y-5",
            [true, false, true],
            [None, None, None],
        ),
    ] {
        let result = AffineCase::from_coordinate_in_rank(
            &CoordinateCase::generic(),
            &equations(&context, &[expression]),
            &[0, 1, 2],
            &sector,
            None,
        )
        .unwrap();
        if expected == [None; 3] {
            // Opposite half-lines keep both bounds infinite, even with huge
            // coefficients. Do not confuse large size with infeasibility.
            assert!(matches!(result, AffineIntersection::Affine(_)));
        } else {
            assert_eq!(
                result,
                AffineIntersection::Coordinate(CoordinateCase::new(expected).unwrap())
            );
        }
    }
    let big = Integer::from(10).pow(80) + Integer::from(7);
    let face = CoordinateCase::new([Some(-3), Some(2), None]).unwrap();
    let row = [
        big.clone(),
        &big * Integer::from(2),
        Integer::from(5),
        &big + Integer::from(5),
    ];
    assert_eq!(
        bounds::classify(&row, &face, &[false, true, true]),
        bounds::RowBounds::Saturated
    );
    assert_eq!(
        bounds::classify(
            &row.each_ref().map(|value| -value),
            &face,
            &[false, true, true]
        ),
        bounds::RowBounds::Saturated
    );
}

#[test]
fn saturation_retains_fixed_contributions_and_interleaved_axis_map() {
    let context = CoefficientContext::new(["z", "d", "y", "x"]);
    let parent = CoordinateCase::new([Some(-3), None, None]).unwrap();
    assert_eq!(
        AffineCase::from_coordinate_in_rank(
            &parent,
            &equations(&context, &["7*x+2*y+3*z+16"]),
            &[3, 2, 0],
            &[false, true, true],
            None,
        )
        .unwrap(),
        AffineIntersection::Coordinate(CoordinateCase::new([Some(-3), Some(1), Some(1)]).unwrap()),
    );
}

#[test]
fn repeated_saturation_preserves_every_conjunct_and_unused_axis() {
    let context = CoefficientContext::new(["x", "y", "z", "w"]);
    for expressions in [["x+z", "y-z"], ["y-z", "x+z"]] {
        assert_eq!(
            AffineCase::from_coordinate_in_rank(
                &CoordinateCase::generic(),
                &equations(&context, &expressions),
                &[0, 1, 2, 3],
                &[false; 4],
                None,
            )
            .unwrap(),
            AffineIntersection::Coordinate(
                CoordinateCase::new([Some(0), Some(0), Some(0), None]).unwrap()
            ),
        );
    }
    assert_eq!(
        AffineCase::from_coordinate_in_rank(
            &CoordinateCase::generic(),
            &equations(&context, &["x+z", "y-z-1"]),
            &[0, 1, 2, 3],
            &[false; 4],
            None,
        )
        .unwrap(),
        AffineIntersection::Empty,
    );
}

#[test]
fn empty_incoming_conjunction_refines_a_stored_affine_parent() {
    let context = CoefficientContext::new(["x", "y", "z", "w"]);
    let case = Case::<4>::generic()
        .intersect(
            &equations(&context, &["1+x-y+z+w"]),
            &[0, 1, 2, 3],
            &[true; 4],
        )
        .unwrap()
        .unwrap();
    assert!(case.affine().is_some());
    let refined = case
        .intersect(&[], &[0, 1, 2, 3], &[false, true, false, false])
        .unwrap()
        .unwrap();
    assert_eq!(refined.fixed(), &[Some(0), Some(1), Some(0), Some(0)]);
    assert!(refined.is_numerical());
    // The stored domain is immutable and retains its original open-sector
    // interpretation; only the returned branch acquires the new endpoints.
    assert_eq!(case.fixed(), &[None; 4]);
}

#[test]
fn declared_chart_preserves_symbolic_target_until_explicit_search_intersection() {
    let context = CoefficientContext::new(["x", "y"]);
    let AffineIntersection::Affine(declared) = AffineCase::from_coordinate(
        &CoordinateCase::generic(),
        &equations(&context, &["x+y-2"]),
        &[0, 1],
        &[true; 2],
    )
    .unwrap() else {
        panic!("declared-chart reconstruction must preserve the exact symbolic chart");
    };
    assert_eq!(declared.face().fixed(), &[None; 2]);
    assert_eq!(
        declared
            .restrict_polynomial_value(&equations(&context, &["x"]).remove(0))
            .unwrap(),
        context.coefficient_fixture("2-y"),
    );
    let stored: Case<2> = declared.into();
    let target = Integral::symbolic([0; 2]).unwrap();
    assert_eq!(stored.integral(), target);
    let inferred = stored.intersect(&[], &[0, 1], &[true; 2]).unwrap().unwrap();
    assert_eq!(inferred.fixed(), &[Some(1), Some(1)]);
    assert!(inferred.is_numerical());
    // Search creates a separate refined domain; never rewrite the saved rule
    // target or coefficient chart behind the caller's immutable owner.
    assert_eq!(stored.integral(), target);
    assert!(stored.affine().is_some());
}

#[test]
fn slack_and_both_infinite_rows_remain_unresolved() {
    let context = CoefficientContext::new(["x", "y"]);
    for (expression, sector, witness) in [
        ("x+y-3", [true; 2], [1, 2]),
        ("x+y+1", [false; 2], [0, -1]),
        ("x-y", [true; 2], [4, 4]),
        ("x-y", [false; 2], [-4, -4]),
    ] {
        let AffineIntersection::Affine(case) = AffineCase::from_coordinate_in_rank(
            &CoordinateCase::generic(),
            &equations(&context, &[expression]),
            &[0, 1],
            &sector,
            None,
        )
        .unwrap() else {
            panic!("unsaturated case must remain affine: {expression}");
        };
        assert!(case.contains_coordinate(&CoordinateCase::new(witness.map(Some)).unwrap()));
        assert!(!case.has_saturated_sector_row(&sector));
    }
}

#[test]
fn all_fixed_and_zero_rows_do_not_claim_new_progress() {
    let fixed = CoordinateCase::new([Some(1), Some(0)]).unwrap();
    assert_eq!(
        bounds::classify(&[1, 2, 1].map(Integer::from), &fixed, &[true, false]),
        bounds::RowBounds::Unresolved,
    );
    for face in [fixed, CoordinateCase::generic()] {
        assert_eq!(
            bounds::classify(&[0, 0, 0].map(Integer::from), &face, &[true, false]),
            bounds::RowBounds::Unresolved
        );
        assert_eq!(
            bounds::classify(&[0, 0, 1].map(Integer::from), &face, &[true, false]),
            bounds::RowBounds::Excluded
        );
    }
}

#[test]
fn full_admission_precedes_saturation_and_or_siblings_survive() {
    let context = CoefficientContext::new(["x", "y", "d"]);
    assert!(matches!(
        AffineCase::from_coordinate(
            &CoordinateCase::generic(),
            &equations(&context, &["x+y-2", "d*x+y"]),
            &[0, 1],
            &[true; 2],
        ),
        Err(AffineGeometryError::Coordinate(
            GeometryError::InvalidInput(_)
        )),
    ));
    let branches = Case::<2>::generic()
        .intersect_many(
            &equations(&context, &["(x+y)*(x+y+1)"]),
            &[0, 1],
            &[false; 2],
            Default::default(),
        )
        .unwrap();
    for (key, expected) in [
        ([0, 0], true),
        ([0, -1], true),
        ([-1, 0], true),
        ([-1, -1], false),
    ] {
        let point: Case<2> = CoordinateCase::new(key.map(Some)).unwrap().into();
        assert_eq!(
            branches
                .cases
                .iter()
                .any(|case| case.contains(&point).unwrap()),
            expected
        );
    }
}
