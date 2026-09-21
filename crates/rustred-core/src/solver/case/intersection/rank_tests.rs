//! Independent scoped-geometry checks. Finite integer enumeration here is a
//! test oracle for the declared rank domain, not a production algebra service.

use std::collections::BTreeSet;

use symbolica::prelude::Integer;

use crate::algebra::{CoefficientContext, CoefficientPolynomial};
use crate::solver::{AffineGeometryError, Case, CoordinateCase, GeometryError, Power};

use super::{CaseIntersectionBudget, CaseIntersectionFailure, CaseIntersectionLimits};

fn equations(context: &CoefficientContext, values: &[&str]) -> Vec<CoefficientPolynomial> {
    values
        .iter()
        .map(|value| context.coefficient_fixture(value).numerator)
        .collect()
}

fn point<const N: usize>(powers: [i16; N]) -> Case<N> {
    CoordinateCase::new(powers.map(Some)).unwrap().into()
}

fn covers<const N: usize>(cases: &[Case<N>], powers: [i16; N]) -> bool {
    let target = point(powers);
    cases.iter().any(|case| case.contains(&target).unwrap())
}

fn implies<const N: usize>(
    case: &Case<N>,
    equation: &CoefficientPolynomial,
    indices: &[usize; N],
) -> bool {
    let restricted = if let Some(affine) = case.affine() {
        affine.restrict_equation(equation).unwrap()
    } else {
        let mut restricted = equation.clone();
        for (axis, value) in case.fixed().iter().enumerate() {
            if let Some(value) = value {
                restricted = restricted.replace(indices[axis], &Integer::from(*value));
            }
        }
        restricted
    };
    restricted.is_zero()
}

fn visit_simplex<const N: usize>(maximum: u32, mut visit: impl FnMut([i16; N])) {
    fn descend<const N: usize>(
        axis: usize,
        remaining: u32,
        powers: &mut [i16; N],
        visit: &mut impl FnMut([i16; N]),
    ) {
        if axis == N {
            visit(*powers);
            return;
        }
        for degree in 0..=remaining {
            powers[axis] = i16::try_from(-i64::from(degree)).unwrap();
            descend(axis + 1, remaining - degree, powers, visit);
        }
    }
    descend(0, maximum, &mut [0; N], &mut visit);
}

fn natural_locus(powers: [i16; 5]) -> bool {
    let [n3, n4, n7, n8, n10] = powers.map(i64::from);
    let u = 1 + n10;
    3 * n3 == 2 * u + 3 * n8 + n7
        && 8 * n4 == -3 * u + n8 + 10 * n7
        && 164 * n7 * n7 - 308 * n7 * n8 + 189 * n8 * n8 + 124 * n7 * u - 94 * n8 * u - 75 * u * u
            == 0
}

#[test]
fn natural_conic_rank_ten_and_twenty_match_the_entire_original_integer_simplex() {
    // These five original inactive coordinates are the only free coordinates
    // of the captured face; all six positive denominator powers are fixed at
    // one and the remaining original coordinates at zero. d is not an index.
    let context = CoefficientContext::new(["d", "n3", "n4", "n7", "n8", "n10"]);
    let indices = [1, 2, 3, 4, 5];
    let sector = [false; 5];
    let conjunction = equations(
        &context,
        &[
            "3*n3-2*(1+n10)-3*n8-n7",
            "8*n4+3*(1+n10)-n8-10*n7",
            "164*n7^2-308*n7*n8+189*n8^2+124*n7*(1+n10)-94*n8*(1+n10)-75*(1+n10)^2",
        ],
    );
    let parent = Case::<5>::generic();
    let unlimited = parent
        .intersect_many(&conjunction, &indices, &sector, Default::default())
        .unwrap_err();
    assert_eq!(
        unlimited.failure,
        CaseIntersectionFailure::UnsupportedGeometry
    );
    assert_eq!(unlimited.max_numerator_rank, None);
    let known = [
        [0, 0, 0, 0, -1],
        [-1, -3, -2, -1, 0],
        [-2, -1, -1, -1, -2],
        [-4, -2, -2, -2, -3],
        [-6, -3, -3, -3, -4],
    ];
    for (maximum, count, simplex_size) in [(10, 3, 3003), (20, 5, 53130)] {
        let result = parent
            .intersect_many_with_max_numerator_rank(
                &conjunction,
                &indices,
                &sector,
                Default::default(),
                maximum,
            )
            .unwrap();
        assert_eq!(result.max_numerator_rank, Some(maximum));
        assert!(result.stats.rank_splits > 0);
        for case in &result.cases {
            assert!(case.is_in_sector(&sector));
            for equation in &conjunction {
                assert!(implies(case, equation, &indices));
            }
        }
        let mut visited = 0;
        let mut expected = BTreeSet::new();
        let mut actual = BTreeSet::new();
        visit_simplex(maximum, |powers| {
            visited += 1;
            if natural_locus(powers) {
                expected.insert(powers);
            }
            if covers(&result.cases, powers) {
                actual.insert(powers);
            }
        });
        assert_eq!(visited, simplex_size);
        assert_eq!(expected.len(), count);
        assert_eq!(actual, expected);
        assert_eq!(expected, known[..count].iter().copied().collect());
    }
}

#[test]
fn total_rank_is_not_a_coordinate_box_and_fixed_negative_powers_consume_it() {
    let context = CoefficientContext::new(["x", "y"]);
    let conjunction = equations(&context, &["x^2+y^2-8"]);
    let parent = Case::<2>::generic();
    for (maximum, expected) in [(3, false), (4, true)] {
        let result = parent
            .intersect_many_with_max_numerator_rank(
                &conjunction,
                &[0, 1],
                &[false; 2],
                Default::default(),
                maximum,
            )
            .unwrap();
        assert_eq!(covers(&result.cases, [-2, -2]), expected);
        assert_eq!(result.cases.is_empty(), !expected);
    }
    let fixed: Case<2> = CoordinateCase::new([Some(-2), None]).unwrap().into();
    assert!(
        fixed
            .intersect_many_with_max_numerator_rank(
                &conjunction,
                &[0, 1],
                &[false; 2],
                Default::default(),
                3,
            )
            .unwrap()
            .cases
            .is_empty()
    );
    assert!(
        point([-2, -2])
            .intersect_many_with_max_numerator_rank(
                &[],
                &[0, 1],
                &[false; 2],
                Default::default(),
                3,
            )
            .unwrap()
            .cases
            .is_empty()
    );
}

#[test]
fn an_unused_positive_axis_remains_symbolic_with_a_permuted_index_map() {
    let context = CoefficientContext::new(["d", "p", "y", "x"]);
    let result = Case::<3>::generic()
        .intersect_many_with_max_numerator_rank(
            &equations(&context, &["x^2+y^2-8"]),
            &[3, 2, 1],
            &[false, false, true],
            Default::default(),
            4,
        )
        .unwrap();
    assert_eq!(
        result.cases,
        vec![
            CoordinateCase::new([Some(-2), Some(-2), None])
                .unwrap()
                .into()
        ]
    );
    for positive in [1, 2, 17, Power::MAX] {
        assert!(covers(&result.cases, [-2, -2, positive]));
    }
    // None is a symbolic positive ray, not a claim proved only by the samples.
    assert_eq!(result.cases[0].fixed()[2], None);
    let rank_zero = Case::<3>::generic()
        .intersect_many_with_max_numerator_rank(
            &equations(&context, &["x", "y"]),
            &[3, 2, 1],
            &[false, false, true],
            Default::default(),
            0,
        )
        .unwrap();
    assert_eq!(
        rank_zero.cases,
        vec![
            CoordinateCase::new([Some(0), Some(0), None])
                .unwrap()
                .into()
        ]
    );
}

#[test]
fn splitting_retains_the_full_and_and_incoming_affine_parent() {
    let context = CoefficientContext::new(["x", "y", "z", "p"]);
    let indices = [0, 1, 2, 3];
    let sector = [false, false, false, true];
    let parent = Case::<4>::generic()
        .intersect(&equations(&context, &["z-x-y"]), &indices, &sector)
        .unwrap()
        .unwrap();
    assert!(parent.affine().is_some());
    let conjunction = equations(&context, &["x^2+y^2-8", "p+x-1"]);
    for (maximum, expected) in [(7, false), (8, true)] {
        let result = parent
            .intersect_many_with_max_numerator_rank(
                &conjunction,
                &indices,
                &sector,
                Default::default(),
                maximum,
            )
            .unwrap();
        assert!(result.stats.rank_splits > 0);
        assert_eq!(covers(&result.cases, [-2, -2, -4, 3]), expected);
        assert_eq!(result.cases.is_empty(), !expected);
        for case in &result.cases {
            assert!(parent.contains(case).unwrap());
            assert!(
                conjunction
                    .iter()
                    .all(|equation| implies(case, equation, &indices))
            );
        }
        assert!(!covers(&result.cases, [-2, -2, 0, 3]));
        assert!(!covers(&result.cases, [-2, -2, -4, 1]));
    }
}

#[test]
fn scoped_work_budget_failure_preserves_the_complete_original_obligation() {
    let context = CoefficientContext::new(["x", "y"]);
    let conjunction = equations(&context, &["(x+1)*(x^2+y^2-8)"]);
    let parent = Case::<2>::generic();
    let error = parent
        .intersect_many_with_max_numerator_rank(
            &conjunction,
            &[0, 1],
            &[false; 2],
            CaseIntersectionLimits {
                max_work_items: 3,
                ..Default::default()
            },
            4,
        )
        .unwrap_err();
    assert_eq!(
        error.failure,
        CaseIntersectionFailure::Budget {
            kind: CaseIntersectionBudget::WorkItems,
            limit: 3,
        }
    );
    assert_eq!(error.max_numerator_rank, Some(4));
    assert_eq!(error.original_parent, parent);
    assert_eq!(error.original_conjunction.as_ref(), conjunction);
    assert!(error.stats.work_items <= 3);
    // No partial OR cover is returned on budget exhaustion.
    let complete = parent
        .intersect_many_with_max_numerator_rank(
            &conjunction,
            &[0, 1],
            &[false; 2],
            Default::default(),
            4,
        )
        .unwrap();
    assert!(covers(&complete.cases, [-1, -3]));
    assert!(covers(&complete.cases, [-2, -2]));
}

#[test]
fn a_nonlinear_positive_power_ray_stays_unsupported_after_all_numerators_are_fixed() {
    let context = CoefficientContext::new(["p", "q", "x"]);
    let parent: Case<3> = CoordinateCase::new([None, None, Some(0)]).unwrap().into();
    let conjunction = equations(&context, &["p^2-2*q^2-1"]);
    for maximum in [0, 20] {
        let error = parent
            .intersect_many_with_max_numerator_rank(
                &conjunction,
                &[0, 1, 2],
                &[true, true, false],
                Default::default(),
                maximum,
            )
            .unwrap_err();
        assert_eq!(error.failure, CaseIntersectionFailure::UnsupportedGeometry);
        assert_eq!(error.max_numerator_rank, Some(maximum));
        assert_eq!(error.stats.rank_splits, 0);
        assert_eq!(error.original_parent, parent);
        assert_eq!(error.original_conjunction.as_ref(), conjunction);
    }
    // There are genuine positive integer witnesses, not merely an unproved
    // empty branch: (p,q)=(3,2), (17,12), ... on the Pell curve.
    assert_eq!(3 * 3 - 2 * 2 * 2 - 1, 0);
}

#[test]
fn compact_negative_boundary_is_exact_and_a_larger_split_is_not_truncated() {
    assert_eq!(Power::MIN, -64);
    let context = CoefficientContext::new(["x", "y"]);
    let conjunction = equations(&context, &["x*y-1"]);
    let parent = Case::<2>::generic();
    let result = parent
        .intersect_many_with_max_numerator_rank(
            &conjunction,
            &[0, 1],
            &[false; 2],
            Default::default(),
            64,
        )
        .unwrap();
    assert_eq!(result.cases, vec![point([-1, -1])]);
    assert_eq!(result.stats.rank_children, 65);
    let error = parent
        .intersect_many_with_max_numerator_rank(
            &conjunction,
            &[0, 1],
            &[false; 2],
            Default::default(),
            65,
        )
        .unwrap_err();
    assert!(matches!(
        error.failure,
        CaseIntersectionFailure::Admission(AffineGeometryError::Coordinate(
            GeometryError::CompactOverflow { axis: 0, .. }
        ))
    ));
    assert_eq!(error.max_numerator_rank, Some(65));
    assert_eq!(error.stats.rank_children, 0);
}
