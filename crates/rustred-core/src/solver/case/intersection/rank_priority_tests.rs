//! Finite rank-domain priority without changing SearchFinite terminal policy.

use super::{CaseIntersectionFailure, CaseIntersectionLimits};
use crate::algebra::{CoefficientContext, CoefficientPolynomial};
use crate::solver::{Case, CoordinateCase};
use symbolica::prelude::Integer;

fn captured(narrowed: bool) -> (CoefficientContext, Case<15>, [usize; 15], [bool; 15]) {
    let context = CoefficientContext::new(
        std::iter::once("d".to_owned()).chain((0..15).map(|axis| format!("n{axis}"))),
    );
    let face: Case<15> = CoordinateCase::new([
        if narrowed { Some(-2) } else { None },
        Some(0),
        Some(1),
        Some(1),
        None,
        Some(0),
        Some(1),
        Some(1),
        Some(1),
        None,
        Some(1),
        Some(0),
        Some(0),
        None,
        Some(0),
    ])
    .unwrap()
    .into();
    let indices = std::array::from_fn(|axis| axis + 1);
    let sector = std::array::from_fn(|axis| b"001100111010000"[axis] == b'1');
    let row = context.coefficient_fixture("-1-n13-n9+2*n4").numerator;
    let parent = face.intersect(&[row], &indices, &sector).unwrap().unwrap();
    (context, parent, indices, sector)
}

fn polynomials(context: &CoefficientContext, narrowed: bool) -> Vec<CoefficientPolynomial> {
    let values: &[&str] = if narrowed {
        &[
            "10195+2184*n13-20177*n13^2-15090*n13^3-2924*n13^4+5031*n9+2043*n9*n13",
            "400+777*n13+462*n13^2+85*n13^3+172*n9+179*n9*n13+43*n9*n13^2",
            "-35-52*n13-17*n13^2-8*n9-2*n9*n13+3*n9^2",
        ]
    } else {
        &[
            "-105-191*n13-103*n13^2-17*n13^3-34*n9-28*n9*n13-6*n9*n13^2+9*n9^2+5*n9^2*n13+2*n9^3-25*n0-38*n0*n13-13*n0*n13^2-8*n0*n9-4*n0*n9*n13+n0*n9^2",
            "-35-52*n13-17*n13^2-8*n9-2*n9*n13+3*n9^2",
        ]
    };
    values
        .iter()
        .map(|value| context.coefficient_fixture(value).numerator)
        .collect()
}

fn all_zero(equations: &[CoefficientPolynomial], point: &[i16; 15]) -> bool {
    equations.iter().all(|equation| {
        let mut value = equation.clone();
        for (axis, &power) in point.iter().enumerate() {
            value = value.replace(axis + 1, &Integer::from(power));
        }
        value.is_zero()
    })
}

#[test]
fn captured_narrowed_rank_ten_uses_existing_splitter_before_divisor_work() {
    let (context, parent, indices, sector) = captured(true);
    let equations = polynomials(&context, true);
    let result = parent
        .intersect_many_with_max_numerator_rank(
            &equations,
            &indices,
            &sector,
            CaseIntersectionLimits {
                max_work_items: 8192,
                ..Default::default()
            },
            10,
        )
        .unwrap();
    let expected = [-2, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1, 0, 0, -1, 0];
    let point: Case<15> = CoordinateCase::new(expected.map(Some)).unwrap().into();
    assert!(all_zero(&equations, &expected));
    assert_eq!(result.cases, vec![point]);
    assert!(result.stats.rank_splits > 0);
    assert_eq!(result.stats.integer_resultants, 0);
    assert_eq!(result.stats.integer_divisors, 0);
    assert!(result.stats.work_items <= 220);
}

#[test]
fn complete_original_rank_simplex_matches_native_equation_evaluation() {
    let (context, parent, indices, sector) = captured(false);
    let equations = polynomials(&context, false);
    let result = parent
        .intersect_many_with_max_numerator_rank(
            &equations,
            &indices,
            &sector,
            CaseIntersectionLimits {
                max_work_items: 8192,
                ..Default::default()
            },
            10,
        )
        .unwrap();
    assert_eq!(result.stats.integer_resultants, 0);
    assert!(result.stats.work_items <= 1365);
    // Native affine admission may keep an exact compressed face rather than
    // enumerate all its points. Do not impose a terminal-retention policy.
    let mut visited = 0usize;
    let mut accepted = 0usize;
    for n0 in 0_i16..=10 {
        for n4 in 0_i16..=10 - n0 {
            for n9 in 0_i16..=10 - n0 - n4 {
                for n13 in 0_i16..=10 - n0 - n4 - n9 {
                    visited += 1;
                    let powers = [-n0, 0, 1, 1, -n4, 0, 1, 1, 1, -n9, 1, 0, 0, -n13, 0];
                    let point: Case<15> = CoordinateCase::new(powers.map(Some)).unwrap().into();
                    let expected =
                        parent.contains(&point).unwrap() && all_zero(&equations, &powers);
                    let actual = result
                        .cases
                        .iter()
                        .any(|case| case.contains(&point).unwrap());
                    assert_eq!(actual, expected, "powers={powers:?}");
                    accepted += usize::from(expected);
                }
            }
        }
    }
    assert_eq!(visited, 1001);
    assert!(accepted > 0);
    assert!(!result.cases.is_empty());
}

#[test]
fn exhausted_budget_retains_original_conjunction_and_no_partial_union() {
    let (context, parent, indices, sector) = captured(true);
    let equations = polynomials(&context, true);
    let error = parent
        .intersect_many_with_max_numerator_rank(
            &equations,
            &indices,
            &sector,
            CaseIntersectionLimits {
                max_work_items: 1,
                ..Default::default()
            },
            10,
        )
        .unwrap_err();
    assert!(matches!(
        error.failure,
        CaseIntersectionFailure::Budget { .. }
    ));
    assert_eq!(error.original_parent, parent);
    assert_eq!(error.original_conjunction.as_ref(), equations);
}
