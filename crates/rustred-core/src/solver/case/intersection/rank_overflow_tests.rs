//! Scoped emptiness must precede compact-key conversion, never hide a
//! feasible out-of-range point, and never weaken full-conjunction admission.

use crate::algebra::{CoefficientContext, CoefficientPolynomial};
use crate::solver::{AffineGeometryError, Case, CoordinateCase, GeometryError};

use super::CaseIntersectionFailure;

fn equations(context: &CoefficientContext, expressions: &[&str]) -> Vec<CoefficientPolynomial> {
    expressions
        .iter()
        .map(|expression| context.coefficient_fixture(expression).numerator)
        .collect()
}

fn overflow(failure: &CaseIntersectionFailure) -> bool {
    matches!(
        failure,
        CaseIntersectionFailure::Admission(AffineGeometryError::Coordinate(
            GeometryError::CompactOverflow { .. }
        ))
    )
}

#[test]
fn captured_full_parent_conjunction_prunes_only_out_of_rank_leaf() {
    let context = CoefficientContext::new(
        std::iter::once("d".to_owned()).chain((0..15).map(|axis| format!("n{axis}"))),
    );
    let indices = std::array::from_fn(|axis| axis + 1);
    let fixed = [
        Some(1),
        Some(-8),
        Some(1),
        Some(0),
        Some(0),
        None,
        Some(1),
        Some(0),
        Some(0),
        Some(1),
        Some(1),
        Some(1),
        Some(1),
        None,
        Some(0),
    ];
    let sector = std::array::from_fn(|axis| matches!(axis, 0 | 2 | 6 | 9 | 10 | 11 | 12));
    let parent: Case<15> = CoordinateCase::new(fixed).unwrap().into();
    let conjunction = equations(
        &context,
        &["66+n13", "-9-9*n13+16*n5-2*n5*n13+3*n5^2", "45+n5"],
    );
    assert!(
        parent
            .intersect_many_with_max_numerator_rank(
                &conjunction,
                &indices,
                &sector,
                Default::default(),
                10,
            )
            .unwrap()
            .cases
            .is_empty()
    );
    // The unique forced point has negative degree 8+66+45=119. It is not
    // empty when admitted at rank119 or without a bound; compactness must
    // still fail instead of silently dropping that legitimate point.
    let in_scope = parent
        .intersect_many_with_max_numerator_rank(
            &conjunction,
            &indices,
            &sector,
            Default::default(),
            119,
        )
        .unwrap_err();
    assert!(overflow(&in_scope.failure));
    assert!(overflow(
        &parent
            .intersect_many(&conjunction, &indices, &sector, Default::default(),)
            .unwrap_err()
            .failure
    ));
}

#[test]
fn captured_original_guard_matches_full_rank_ten_integer_simplex() {
    // The three original free inactive coordinates of the captured parent.
    // Fixed positive/zero coordinates do not enter this guard or the rank.
    let context = CoefficientContext::new(["n1", "n5", "n13"]);
    let source = equations(&context, &["1+n13+2*n5*n13-3*n5^2-n1-n1*n13+2*n1*n5"]);
    let result = Case::<3>::generic()
        .intersect_many_with_max_numerator_rank(
            &source,
            &[0, 1, 2],
            &[false; 3],
            Default::default(),
            10,
        )
        .unwrap();
    let mut matches = 0;
    for a in 0_i16..=10 {
        for b in 0_i16..=10 - a {
            for c in 0_i16..=10 - a - b {
                let [x, y, z] = [-a, -b, -c].map(i64::from);
                let expected = 1 + z + 2 * y * z - 3 * y * y - x - x * z + 2 * x * y == 0;
                let point: Case<3> = CoordinateCase::new([-a, -b, -c].map(Some)).unwrap().into();
                let covered = result
                    .cases
                    .iter()
                    .any(|case| case.contains(&point).unwrap());
                assert_eq!(covered, expected, "original point {:?}", [-a, -b, -c]);
                matches += usize::from(expected);
            }
        }
    }
    assert!(matches > 0, "do not accidentally erase every OR branch");
}

#[test]
fn negative_large_roots_are_pruned_natively_but_positive_overflow_is_preserved() {
    let context = CoefficientContext::new(["x"]);
    for expression in [
        "x+66",
        "x+100000000000000000000000000000000000000000000000000000000000000000000",
    ] {
        let conjunction = equations(&context, &[expression]);
        assert!(
            Case::<1>::generic()
                .intersect_many_with_max_numerator_rank(
                    &conjunction,
                    &[0],
                    &[false],
                    Default::default(),
                    10,
                )
                .unwrap()
                .cases
                .is_empty()
        );
        assert!(overflow(
            &Case::<1>::generic()
                .intersect_many(&conjunction, &[0], &[false], Default::default(),)
                .unwrap_err()
                .failure
        ));
    }
    let positive = Case::<1>::generic()
        .intersect_many_with_max_numerator_rank(
            &equations(&context, &["x-66"]),
            &[0],
            &[true],
            Default::default(),
            0,
        )
        .unwrap_err();
    assert!(overflow(&positive.failure));
}

#[test]
fn out_of_rank_or_sibling_is_removed_without_discarding_valid_sibling() {
    let context = CoefficientContext::new(["x"]);
    let result = Case::<1>::generic()
        .intersect_many_with_max_numerator_rank(
            &equations(&context, &["(x+66)*(x+1)"]),
            &[0],
            &[false],
            Default::default(),
            10,
        )
        .unwrap();
    assert_eq!(
        result.cases,
        vec![CoordinateCase::new([Some(-1)]).unwrap().into()]
    );
}

#[test]
fn distinct_forced_negative_coordinates_add_once_including_fixed_parent() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let parent: Case<3> = CoordinateCase::new([Some(-8), None, None]).unwrap().into();
    // Native RREF, not coordinate substitution, exposes y=z=-4. The parent
    // contribution makes rank16; duplicate-counting it would reject rank16.
    let conjunction = equations(&context, &["y+z+8", "y-z"]);
    for (maximum, empty) in [(10, true), (15, true), (16, false)] {
        let result = parent
            .intersect_many_with_max_numerator_rank(
                &conjunction,
                &[0, 1, 2],
                &[false; 3],
                Default::default(),
                maximum,
            )
            .unwrap();
        assert_eq!(result.cases.is_empty(), empty);
        if !empty {
            assert_eq!(
                result.cases,
                vec![
                    CoordinateCase::new([Some(-8), Some(-4), Some(-4)])
                        .unwrap()
                        .into()
                ]
            );
        }
    }
}

#[test]
fn affine_singletons_are_checked_before_any_compact_conversion() {
    let context = CoefficientContext::new(["d", "y", "x"]);
    let indices = [2, 1];
    // Both sources are coupled, so native RREF is necessary. Their exact
    // solution x=100,y=-66 is rank-empty at10 although the earlier pivot is
    // a positive overflow; at66 the same branch must still report overflow.
    for values in [["x+y-34", "x-y-166"], ["x-y-166", "x+y-34"]] {
        let conjunction = equations(&context, &values);
        assert!(
            Case::<2>::generic()
                .intersect_many_with_max_numerator_rank(
                    &conjunction,
                    &indices,
                    &[true, false],
                    Default::default(),
                    10,
                )
                .unwrap()
                .cases
                .is_empty()
        );
        assert!(overflow(
            &Case::<2>::generic()
                .intersect_many_with_max_numerator_rank(
                    &conjunction,
                    &indices,
                    &[true, false],
                    Default::default(),
                    66,
                )
                .unwrap_err()
                .failure
        ));
    }
}

#[test]
fn coordinate_order_and_incoming_affine_parent_preserve_the_full_conjunction() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    for values in [["x-100", "y+66"], ["y+66", "x-100"]] {
        assert!(
            Case::<3>::generic()
                .intersect_many_with_max_numerator_rank(
                    &equations(&context, &values),
                    &[0, 1, 2],
                    &[true, false, false],
                    Default::default(),
                    10,
                )
                .unwrap()
                .cases
                .is_empty()
        );
    }
    let parent = Case::<3>::generic()
        .intersect(
            &equations(&context, &["y-z"]),
            &[0, 1, 2],
            &[true, false, false],
        )
        .unwrap()
        .unwrap();
    assert!(parent.affine().is_some());
    assert!(
        parent
            .intersect_many_with_max_numerator_rank(
                &equations(&context, &["y+66"]),
                &[0, 1, 2],
                &[true, false, false],
                Default::default(),
                10,
            )
            .unwrap()
            .cases
            .is_empty()
    );
}

#[test]
fn rank_empty_conjunction_still_receives_complete_input_preflight() {
    let context = CoefficientContext::new(["x", "d"]);
    let error = Case::<1>::generic()
        .intersect_many_with_max_numerator_rank(
            &equations(&context, &["x+66", "d"]),
            &[0],
            &[false],
            Default::default(),
            10,
        )
        .unwrap_err();
    assert!(matches!(
        error.failure,
        CaseIntersectionFailure::InvalidInput(_)
    ));
}
