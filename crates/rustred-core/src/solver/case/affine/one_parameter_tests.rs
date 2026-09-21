//! Adversarial search-only interval refinement; no sampled proof authority.

use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{Case, CaseIntersectionLimits};
use symbolica::prelude::Z;

fn equations(context: &CoefficientContext, expressions: &[&str]) -> Vec<CoefficientPolynomial> {
    expressions
        .iter()
        .map(|expression| context.coefficient_fixture(expression).numerator)
        .collect()
}

#[test]
fn captured_case391_is_one_original_integer_point() {
    let context = CoefficientContext::new(
        std::iter::once("d".to_owned()).chain((0..15).map(|axis| format!("n{axis}"))),
    );
    let indices = std::array::from_fn(|axis| axis + 1);
    let face = CoordinateCase::new([
        None,
        None,
        None,
        Some(0),
        Some(1),
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
    let sector = std::array::from_fn(|axis| b"111011000010111"[axis] == b'1');
    let expected =
        CoordinateCase::new([3, 1, 3, 0, 1, 1, 0, 0, 0, 0, 1, 0, 1, 1, 1].map(Some)).unwrap();
    let started = std::time::Instant::now();
    let mut admissions = 0;
    for expressions in [["n0-3*n2+6", "n1+n2-4"], ["-n1-n2+4", "-n0+3*n2-6"]] {
        for rank in [None, Some(0), Some(10)] {
            let result = AffineCase::from_coordinate_in_rank(
                &face,
                &equations(&context, &expressions),
                &indices,
                &sector,
                rank,
            )
            .unwrap();
            assert_eq!(result, AffineIntersection::Coordinate(expected));
            let AffineIntersection::Coordinate(point) = result else {
                unreachable!("exact coordinate result asserted above");
            };
            assert!(
                point.is_numerical(),
                "the complete admitted domain is finite"
            );
            admissions += 1;
        }
    }
    println!(
        "captured_case391_geometry_only admissions={admissions} original_integer_points=1 numerator_rank=0 elapsed_s={:.9} ibp_generation_performed=false",
        started.elapsed().as_secs_f64(),
    );
}

#[test]
fn stored_declared_chart_refines_only_at_search_admission() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let input = equations(&context, &["x-3*z+6", "y+z-4"]);
    let AffineIntersection::Affine(declared) =
        AffineCase::from_coordinate(&CoordinateCase::generic(), &input, &[0, 1, 2], &[true; 3])
            .unwrap()
    else {
        panic!("declared chart must retain its symbolic layout");
    };
    let stored: Case<3> = declared.into();
    let before = stored.clone();
    let target = stored.integral();
    assert_eq!(stored.fixed(), &[None; 3]);
    assert_eq!(
        stored
            .intersect(&[], &[0, 1, 2], &[true; 3])
            .unwrap()
            .unwrap()
            .fixed(),
        &[Some(3), Some(1), Some(3)],
    );
    assert_eq!(stored, before);
    assert_eq!(stored.integral(), target);

    // Even a declared chart with no positive integer point retains its
    // previous import representation; only search admission rejects it.
    let AffineIntersection::Affine(empty_declared) = AffineCase::from_coordinate(
        &CoordinateCase::generic(),
        &equations(&context, &["x-3*z+6", "y+z-3"]),
        &[0, 1, 2],
        &[true; 3],
    )
    .unwrap() else {
        panic!("do not strengthen declared-chart import");
    };
    let empty_stored: Case<3> = empty_declared.into();
    assert!(
        empty_stored
            .intersect(&[], &[0, 1, 2], &[true; 3])
            .unwrap()
            .is_none()
    );
}

#[test]
fn integer_rounding_handles_negative_parameters_and_original_axis_maps() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    for (expressions, sector, expected) in [
        (["x+2*z+2", "y-3*z-7"], [true, true, false], [2, 1, -2]),
        (["x-3*z-7", "y+z+3"], [false; 3], [-2, 0, -3]),
        (["x-z", "y+z-2"], [true; 3], [1, 1, 1]),
    ] {
        let result = Case::<3>::generic()
            .intersect(&equations(&context, &expressions), &[0, 1, 2], &sector)
            .unwrap()
            .unwrap();
        assert_eq!(*result.fixed(), expected.map(Some));
    }
    let context = CoefficientContext::new(["t", "d", "u", "v"]);
    let input = equations(&context, &["u-3*t+6", "v+t-4"]);
    for (indices, expected) in [([2, 3, 0], [3, 1, 3]), ([0, 2, 3], [3, 3, 1])] {
        let result = Case::<3>::generic()
            .intersect(&input, &indices, &[true; 3])
            .unwrap()
            .unwrap();
        assert_eq!(*result.fixed(), expected.map(Some));
    }
}

#[test]
fn empty_intervals_and_fractional_dependent_singletons_are_not_points() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    for expressions in [
        ["x-3*z+6", "y+z-3"],
        ["x-2*z+2", "y+3*z-5"],
        ["2*x-3*z+6", "y+z-4"],
    ] {
        assert!(
            Case::<3>::generic()
                .intersect(&equations(&context, &expressions), &[0, 1, 2], &[true; 3])
                .unwrap()
                .is_none()
        );
    }
    let context = CoefficientContext::new(["x", "y", "w", "z"]);
    let result = Case::<4>::generic()
        .intersect(
            &equations(&context, &["2*x-z-2", "y+z-3", "w-2*z+3"]),
            &[0, 1, 2, 3],
            &[true; 4],
        )
        .unwrap()
        .unwrap();
    assert_eq!(result.fixed(), &[Some(2), Some(1), Some(1), Some(2)]);
}

#[test]
fn multiple_integer_parameters_rays_and_higher_dimension_remain_unresolved() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    for (expressions, sector) in [
        (["x-3*z+6", "y+z-5"], [true; 3]),
        (["x-3*z+6", "y-z"], [true; 3]),
        (["x-3*z-6", "y+z+4"], [false; 3]),
        // Only one of the two candidate integers satisfies divisibility.
        // This service does not invent a congruence sieve or select a point.
        (["2*x-3*z+6", "y+z-5"], [true; 3]),
    ] {
        let result = Case::<3>::generic()
            .intersect(&equations(&context, &expressions), &[0, 1, 2], &sector)
            .unwrap()
            .unwrap();
        assert!(matches!(result, Case::Affine(_)));
        assert_eq!(result.fixed(), &[None; 3]);
    }
    let result = Case::<3>::generic()
        .intersect(&equations(&context, &["x+2*y-5"]), &[0, 1, 2], &[true; 3])
        .unwrap()
        .unwrap();
    assert!(matches!(result, Case::Affine(_)));
    assert_eq!(result.fixed(), &[None; 3]);
}

#[test]
fn native_large_singleton_reaches_rank_before_compact_conversion() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let big = Integer::from(10).pow(80) + Integer::from(7);
    let first = format!("x+z+{}", &big - Integer::one());
    let second = format!("y-z-{}", &big + Integer::one());
    let input = equations(&context, &[&first, &second]);
    assert_eq!(
        AffineCase::from_coordinate_in_rank(
            &CoordinateCase::generic(),
            &input,
            &[0, 1, 2],
            &[true, true, false],
            Some(10),
        )
        .unwrap(),
        AffineIntersection::Empty,
    );
    assert!(matches!(
        AffineCase::from_coordinate_in_rank(
            &CoordinateCase::generic(), &input, &[0, 1, 2], &[true, true, false], None,
        ),
        Err(AffineGeometryError::Coordinate(GeometryError::CompactOverflow { axis: 2, value }))
            if value == -big,
    ));
    assert!(matches!(
        Case::<3>::generic().intersect(
            &equations(&context, &["x-z+155", "y+z-157"]), &[0, 1, 2], &[true; 3],
        ),
        Err(AffineGeometryError::Coordinate(GeometryError::CompactOverflow { axis: 2, value }))
            if value == Integer::from(156),
    ));
}

#[test]
fn full_parent_and_invalid_payload_are_checked_before_refinement() {
    let context = CoefficientContext::new(["d", "x", "y", "z"]);
    let input = equations(&context, &["x-3*z+6", "y+z-4"]);
    assert_eq!(
        AffineCase::from_coordinate_in_rank(
            &CoordinateCase::new([Some(2), None, None]).unwrap(),
            &input,
            &[1, 2, 3],
            &[true; 3],
            None,
        )
        .unwrap(),
        AffineIntersection::Empty,
    );
    let AffineIntersection::Affine(declared) =
        AffineCase::from_coordinate(&CoordinateCase::generic(), &input, &[1, 2, 3], &[true; 3])
            .unwrap()
    else {
        panic!("declared affine fixture");
    };
    let stored: Case<3> = declared.into();
    assert!(
        stored
            .intersect(&equations(&context, &["d"]), &[1, 2, 3], &[true; 3])
            .is_err()
    );
    assert!(
        Case::<3>::generic()
            .intersect(
                &equations(&context, &["x-3*z+6", "y+z-3", "d"]),
                &[1, 2, 3],
                &[true; 3],
            )
            .is_err(),
        "the empty interval must not hide an invalid parameter equation"
    );
}

#[test]
fn factor_union_retains_every_integer_sibling() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let result = Case::<3>::generic()
        .intersect_many(
            &equations(&context, &["(x-1)*(x-3*z+6)", "y+z-4"]),
            &[0, 1, 2],
            &[true; 3],
            CaseIntersectionLimits::default(),
        )
        .unwrap();
    let expected = [[1, 3, 1], [1, 2, 2], [1, 1, 3], [3, 1, 3]];
    for x in 1..=5 {
        for y in 1..=5 {
            for z in 1..=5 {
                let values = [x, y, z];
                let point: Case<3> = CoordinateCase::new(values.map(Some)).unwrap().into();
                assert_eq!(
                    result
                        .cases
                        .iter()
                        .any(|case| case.contains(&point).unwrap()),
                    expected.contains(&values),
                    "unexpected union membership at {values:?}",
                );
            }
        }
    }
}

#[test]
fn primitive_constant_rows_and_free_axis_sign_are_included() {
    for (constant, sector, expected) in [
        (0, [true, false], one_parameter::Refinement::Empty),
        (1, [true, false], one_parameter::Refinement::Unresolved),
        (1, [false, true], one_parameter::Refinement::Empty),
    ] {
        let matrix =
            Matrix::from_linear([1, 0, constant].map(Integer::from).to_vec(), 1, 3, Z).unwrap();
        assert_eq!(one_parameter::classify(&matrix, &sector), expected);
    }
    // n0=2-n1 with n0,n1>=1 fixes the FREE index itself to one.
    let matrix = Matrix::from_linear([1, 1, 2].map(Integer::from).to_vec(), 1, 3, Z).unwrap();
    assert_eq!(
        one_parameter::classify(&matrix, &[true; 2]),
        one_parameter::Refinement::Fix {
            axis: 1,
            value: Integer::one(),
        }
    );
}

#[test]
fn native_integer_rounding_never_discards_constructed_witnesses() {
    // An independent finite test census, not the production proof strategy.
    for a in -2_i16..=2 {
        for c in -2_i16..=2 {
            for b in [-2_i16, 0, 2] {
                for d in [-2_i16, 0, 2] {
                    let matrix = Matrix::from_linear(
                        [1, 0, -a, b, 0, 1, -c, d].map(Integer::from).to_vec(),
                        2,
                        4,
                        Z,
                    )
                    .unwrap();
                    for mask in 0_u8..8 {
                        let sector: [bool; 3] = std::array::from_fn(|axis| mask & (1 << axis) != 0);
                        let classification = one_parameter::classify(&matrix, &sector);
                        for t in -4_i16..=4 {
                            let point = [a * t + b, c * t + d, t];
                            if point
                                .iter()
                                .zip(sector)
                                .all(|(&value, positive)| (value > 0) == positive)
                            {
                                match &classification {
                                    one_parameter::Refinement::Empty => {
                                        panic!("discarded witness {point:?}")
                                    }
                                    one_parameter::Refinement::Fix { axis, value } => {
                                        assert_eq!(*axis, 2);
                                        assert_eq!(*value, Integer::from(t));
                                    }
                                    one_parameter::Refinement::Unresolved => {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
