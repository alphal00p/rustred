use std::collections::BTreeSet;

use symbolica::prelude::Integer;

use crate::algebra::{CoefficientContext, CoefficientPolynomial};
use crate::solver::{AffineGeometryError, Case, CoordinateCase, GeometryError};

use super::super::{CaseIntersectionBudget, CaseIntersectionFailure, CaseIntersectionLimits};

fn polynomial(context: &CoefficientContext, input: &str) -> CoefficientPolynomial {
    context.coefficient_fixture(input).numerator
}

fn solve(input: &str, sector: [bool; 2]) -> Vec<Case<2>> {
    let context = CoefficientContext::new(["x", "y"]);
    Case::generic()
        .intersect_many(
            &[polynomial(&context, input)],
            &[0, 1],
            &sector,
            Default::default(),
        )
        .unwrap()
        .cases
}

fn fixed(cases: &[Case<2>]) -> BTreeSet<[i16; 2]> {
    cases
        .iter()
        .map(|case| case.fixed().map(Option::unwrap))
        .collect()
}

#[test]
fn captured_positive_pairs_are_exact_without_a_rank_bound() {
    let expected = BTreeSet::from([[1, 2], [2, 4]]);
    for input in ["8-3*y-6*x+2*x*y", "-8+3*y+6*x-2*x*y", "24-9*y-18*x+6*x*y"] {
        assert_eq!(fixed(&solve(input, [true; 2])), expected);
    }
    for sector in [[false, true], [true, false], [false, false]] {
        assert!(solve("8-3*y-6*x+2*x*y", sector).is_empty());
    }
    assert_eq!(
        fixed(&solve("8-3*x-6*y+2*x*y", [true; 2])),
        BTreeSet::from([[2, 1], [4, 2]])
    );
}

#[test]
fn additional_captured_bilinear_guards_have_exact_positive_points() {
    assert_eq!(
        fixed(&solve("-13+7*y-7*x+x*y", [true; 2])),
        BTreeSet::from([[2, 3], [5, 4], [11, 5], [29, 6]])
    );
    assert_eq!(
        fixed(&solve("-1-y-x+2*x*y", [true; 2])),
        BTreeSet::from([[1, 2], [2, 1]])
    );
}

#[test]
fn signed_unit_divisors_negative_a_and_congruences() {
    assert_eq!(fixed(&solve("x*y-1", [true; 2])), BTreeSet::from([[1, 1]]));
    assert_eq!(
        fixed(&solve("x*y-1", [false; 2])),
        BTreeSet::from([[-1, -1]])
    );
    assert_eq!(
        fixed(&solve("-x*y-1", [true, false])),
        BTreeSet::from([[1, -1]])
    );
    assert_eq!(
        fixed(&solve("-x*y-1", [false, true])),
        BTreeSet::from([[-1, 1]])
    );
    for sector in [[false, false], [false, true], [true, false], [true, true]] {
        assert!(solve("2*x*y+1", sector).is_empty());
    }
}

#[test]
fn direct_refinement_checks_negative_a_zero_k_and_pending_work_overflow() {
    let context = CoefficientContext::new(["x", "y"]);
    for (input, expected) in [
        ("-8+3*y+6*x-2*x*y", BTreeSet::from([[1, 2], [2, 4]])),
        ("-x*y-1", BTreeSet::from([[1, -1]])),
    ] {
        let equation = polynomial(&context, input);
        let branches = super::refine(
            &equation,
            &[0, 1],
            Default::default(),
            &mut Default::default(),
            0,
        )
        .unwrap()
        .unwrap();
        let sector = if input == "-x*y-1" {
            [true, false]
        } else {
            [true, true]
        };
        let cases: Vec<Case<2>> = branches
            .into_iter()
            .filter_map(|branch| {
                Case::<2>::generic()
                    .intersect(&branch, &[0, 1], &sector)
                    .unwrap()
            })
            .collect();
        assert_eq!(fixed(&cases), expected);
    }
    let equation = polynomial(&context, "2*x*y-6*x-3*y+9");
    let branches = super::refine(
        &equation,
        &[0, 1],
        Default::default(),
        &mut Default::default(),
        0,
    )
    .unwrap()
    .unwrap();
    let cases: Vec<Case<2>> = branches
        .into_iter()
        .filter_map(|branch| {
            Case::<2>::generic()
                .intersect(&branch, &[0, 1], &[true; 2])
                .unwrap()
        })
        .collect();
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].fixed(), &[None, Some(3)]);
    let error = super::refine(
        &polynomial(&context, "x*y-1"),
        &[0, 1],
        Default::default(),
        &mut Default::default(),
        usize::MAX,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        CaseIntersectionFailure::Budget {
            kind: CaseIntersectionBudget::WorkItems,
            ..
        }
    ));
}

#[test]
fn zero_k_retains_infinite_affine_faces_and_affine_input_uses_existing_service() {
    let cases = solve("2*x*y-6*x-3*y+9", [true; 2]);
    assert_eq!(cases.len(), 1);
    // x=3/2 is not integral; y=3 leaves x free.
    assert_eq!(cases[0].fixed(), &[None, Some(3)]);
    let cases = solve("x*y-3*x-2*y+6", [true; 2]);
    assert_eq!(cases.len(), 2);
    assert!(cases.iter().any(|case| case.fixed() == &[Some(2), None]));
    assert!(cases.iter().any(|case| case.fixed() == &[None, Some(3)]));
    let cases = solve("2*x-4", [true; 2]);
    assert_eq!(cases[0].fixed(), &[Some(2), None]);
}

#[test]
fn third_axis_stays_free_and_parent_congruence_is_not_lost() {
    let context = CoefficientContext::new(["d", "x", "y", "z"]);
    let indices = [1, 2, 3];
    let sector = [true; 3];
    let equation = polynomial(&context, "8-3*y-6*x+2*x*y");
    let result = Case::<3>::generic()
        .intersect_many(&[equation.clone()], &indices, &sector, Default::default())
        .unwrap();
    assert_eq!(result.cases.len(), 2);
    assert!(result.cases.iter().all(|case| case.fixed()[2].is_none()));
    let parent = Case::<3>::generic()
        .intersect(&[polynomial(&context, "2*z-x")], &indices, &sector)
        .unwrap()
        .unwrap();
    let result = parent
        .intersect_many(&[equation], &indices, &sector, Default::default())
        .unwrap();
    assert_eq!(
        result.cases,
        vec![
            CoordinateCase::new([Some(2), Some(4), Some(1)])
                .unwrap()
                .into()
        ]
    );
}

#[test]
fn entire_sibling_conjunction_and_fixed_parent_are_rechecked() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let equations = [
        polynomial(&context, "8-3*y-6*x+2*x*y"),
        polynomial(&context, "(x-1)*(z^2+1)"),
    ];
    let result = Case::<3>::generic()
        .intersect_many(&equations, &[0, 1, 2], &[true; 3], Default::default())
        .unwrap();
    assert_eq!(
        result.cases,
        vec![
            CoordinateCase::new([Some(1), Some(2), None])
                .unwrap()
                .into()
        ]
    );
    let parent: Case<3> = CoordinateCase::new([Some(3), None, None]).unwrap().into();
    assert!(
        parent
            .intersect_many(&equations, &[0, 1, 2], &[true; 3], Default::default())
            .unwrap()
            .cases
            .is_empty()
    );
}

#[test]
fn unsupported_support_and_large_k_remain_fail_closed() {
    let context = CoefficientContext::new(["d", "x", "y", "z"]);
    for input in ["d*x*y-1", "x^2*y-1", "x*y*z-1", "x*y-18446744073709551616"] {
        let equation = polynomial(&context, input);
        let mut stats = Default::default();
        assert!(
            super::refine(&equation, &[1, 2, 3], Default::default(), &mut stats, 0)
                .unwrap()
                .is_none()
        );
    }
    let error = Case::<3>::generic()
        .intersect_many(
            &[polynomial(&context, "x*y-18446744073709551616")],
            &[1, 2, 3],
            &[true; 3],
            Default::default(),
        )
        .unwrap_err();
    assert_eq!(error.failure, CaseIntersectionFailure::UnsupportedGeometry);
}

#[test]
fn work_and_factor_budgets_fail_atomically_before_a_partial_union() {
    let context = CoefficientContext::new(["x", "y"]);
    let equations = [polynomial(&context, "x*y-12")];
    for (limits, expected) in [
        (
            CaseIntersectionLimits {
                max_work_items: 12,
                ..Default::default()
            },
            CaseIntersectionBudget::WorkItems,
        ),
        (
            CaseIntersectionLimits {
                max_factorizations: 1,
                ..Default::default()
            },
            CaseIntersectionBudget::Factorizations,
        ),
    ] {
        let error = Case::<2>::generic()
            .intersect_many(&equations, &[0, 1], &[true; 2], limits)
            .unwrap_err();
        assert!(
            matches!(error.failure, CaseIntersectionFailure::Budget { kind, .. } if kind == expected)
        );
        assert_eq!(error.original_conjunction.as_ref(), equations);
    }
    // Divisor visits fit, but their children do not: still no successful prefix.
    let error = Case::<2>::generic()
        .intersect_many(
            &equations,
            &[0, 1],
            &[true; 2],
            CaseIntersectionLimits {
                max_work_items: 13,
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(matches!(
        error.failure,
        CaseIntersectionFailure::Budget {
            kind: CaseIntersectionBudget::WorkItems,
            ..
        }
    ));
    assert_eq!(error.stats.integer_divisors, 12);
}

#[test]
fn feasible_compact_overflow_is_not_dropped_and_sector_exclusion_precedes_conversion() {
    let context = CoefficientContext::new(["x", "y"]);
    let equation = polynomial(&context, "(x-1000)*(y-1000)-1");
    let error = Case::<2>::generic()
        .intersect_many(&[equation.clone()], &[0, 1], &[true; 2], Default::default())
        .unwrap_err();
    assert!(matches!(
        error.failure,
        CaseIntersectionFailure::Admission(AffineGeometryError::Coordinate(
            GeometryError::CompactOverflow { .. }
        ))
    ));
    assert!(
        Case::<2>::generic()
            .intersect_many(&[equation], &[0, 1], &[false; 2], Default::default(),)
            .unwrap()
            .cases
            .is_empty()
    );
}

#[test]
fn exact_divisor_results_match_small_integer_oracle_for_many_coefficients() {
    let context = CoefficientContext::new(["x", "y"]);
    for a in [-2i64, -1, 1, 2] {
        for b in [-2i64, 0, 2] {
            for c in [-1i64, 1] {
                for d in [-2i64, -1, 1, 2] {
                    let k = b * c - a * d;
                    if k == 0 {
                        continue;
                    }
                    let input = format!("({a})*x*y+({b})*x+({c})*y+({d})");
                    let equation = polynomial(&context, &input);
                    for sector in [[false, false], [false, true], [true, false], [true, true]] {
                        let cases = Case::<2>::generic()
                            .intersect_many(
                                &[equation.clone()],
                                &[0, 1],
                                &sector,
                                Default::default(),
                            )
                            .unwrap()
                            .cases;
                        let points = fixed(&cases);
                        // For these coefficients the divisor formula bounds
                        // both coordinates inside this box. This oracle is a
                        // differential test, not the completeness argument.
                        let expected: BTreeSet<_> = (-20i16..=20)
                            .flat_map(|x| (-20i16..=20).map(move |y| [x, y]))
                            .filter(|[x, y]| {
                                (*x > 0) == sector[0]
                                    && (*y > 0) == sector[1]
                                    && a * i64::from(*x) * i64::from(*y)
                                        + b * i64::from(*x)
                                        + c * i64::from(*y)
                                        + d
                                        == 0
                            })
                            .collect();
                        assert_eq!(points, expected, "{input}, {sector:?}");
                        for [x, y] in points {
                            assert!(
                                equation
                                    .replace_all(&[Integer::from(x), Integer::from(y)])
                                    .is_zero()
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn original_coordinate_permutation_and_rank_scope_do_not_change_integer_refinement() {
    let context = CoefficientContext::new(["d", "y", "x"]);
    let equations = [polynomial(&context, "8-3*y-6*x+2*x*y")];
    for rank in [0, 10, 20] {
        let result = Case::<2>::generic()
            .intersect_many_with_max_numerator_rank(
                &equations,
                &[2, 1],
                &[true; 2],
                Default::default(),
                rank,
            )
            .unwrap();
        assert_eq!(fixed(&result.cases), BTreeSet::from([[1, 2], [2, 4]]));
        assert_eq!(result.stats.rank_splits, 0);
        assert_eq!(result.stats.integer_factorizations, 1);
        assert_eq!(result.stats.integer_divisors, 4);
    }
    // This valid solution lies outside the differential test's box.
    assert_eq!(
        fixed(&solve("(x-40)*(y-40)-1", [true; 2])),
        BTreeSet::from([[39, 39], [41, 41]])
    );
}
