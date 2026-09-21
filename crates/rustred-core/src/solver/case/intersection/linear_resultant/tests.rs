use std::collections::BTreeSet;

use symbolica::poly::PolynomialResultant;
use symbolica::prelude::{Integer, IntegerRing};

use crate::algebra::{CoefficientContext, CoefficientPolynomial};
use crate::solver::{AffineGeometryError, Case, CoordinateCase, GeometryError};

use super::super::{CaseIntersectionBudget, CaseIntersectionFailure, CaseIntersectionLimits};

const CAPTURED: &str = "37-17*y+2*y^2-14*x+4*x*y";

fn polynomial(context: &CoefficientContext, input: &str) -> CoefficientPolynomial {
    context.coefficient_fixture(input).numerator
}

fn solve(input: &str, sector: [bool; 2]) -> Vec<Case<2>> {
    let context = CoefficientContext::new(["x", "y"]);
    Case::<2>::generic()
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
fn captured_guard_is_exact_without_a_rank_bound_and_rejects_noninteger_x() {
    assert_eq!(fixed(&solve(CAPTURED, [true; 2])), BTreeSet::from([[2, 3]]));
    for sector in [[false, false], [false, true], [true, false]] {
        assert!(solve(CAPTURED, sector).is_empty());
    }
    let context = CoefficientContext::new(["x", "y"]);
    let equation = polynomial(&context, CAPTURED);
    let data = super::authenticate(&equation, &[0, 1]).unwrap();
    let r = data
        .p
        .to_univariate_from_univariate(1)
        .resultant(&data.q.to_univariate_from_univariate(1));
    assert_eq!(r, Integer::from(32));
    let r2 = <IntegerRing as PolynomialResultant<u16>>::resultant(&data.p, &data.q, 1);
    assert!(r2.is_constant());
    assert_eq!(r2.get_constant(), r);
    // P is nonprimitive (content2). The candidate y=4 is discarded only
    // because x=-1/2 is noninteger, not by omitting a resultant divisor.
    assert_eq!(
        data.p.replace(1, &Integer::from(4)).get_constant(),
        Integer::from(2)
    );
    assert_eq!(
        data.q.replace(1, &Integer::from(4)).get_constant(),
        Integer::one()
    );
}

#[test]
fn negative_leading_coefficient_signed_unit_resultants_and_higher_degree() {
    let context = CoefficientContext::new(["x", "y"]);
    let equation = -polynomial(&context, CAPTURED);
    let branches = super::refine(
        &equation,
        &[0, 1],
        Default::default(),
        &mut Default::default(),
        0,
    )
    .unwrap()
    .unwrap();
    let cases: Vec<_> = branches
        .into_iter()
        .filter_map(|branch| {
            Case::<2>::generic()
                .intersect(&branch, &[0, 1], &[true; 2])
                .unwrap()
        })
        .collect();
    assert_eq!(fixed(&cases), BTreeSet::from([[2, 3]]));
    assert_eq!(
        fixed(&solve("(y+1)*x+y^2-2", [true, false])),
        BTreeSet::from([[2, 0], [2, -2]])
    );
    assert_eq!(
        fixed(&solve("(y+1)*x+y^2", [false; 2])),
        BTreeSet::from([[0, 0]])
    );
    assert_eq!(
        fixed(&solve("(y+1)*x+y^2", [true, false])),
        BTreeSet::from([[4, -2]])
    );
    assert_eq!(
        fixed(&solve("(y+1)*x+y^3+2", [false; 2])),
        BTreeSet::from([[-2, 0], [-6, -2]])
    );
}

#[test]
fn zero_resultant_preserves_factor_faces_or_remains_unknown() {
    let context = CoefficientContext::new(["x", "y"]);
    for input in ["(y-1)*x+y^2-1", "(y-1)*x+(y-1)*(y^2+1)"] {
        assert!(
            super::refine(
                &polynomial(&context, input),
                &[0, 1],
                Default::default(),
                &mut Default::default(),
                0
            )
            .unwrap()
            .is_none()
        );
    }
    let cases = solve("(y-1)*x+y^2-1", [true; 2]);
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].fixed(), &[None, Some(1)]);
    let equations = [polynomial(&context, "(y-1)*x+(y-1)*(y^2+1)")];
    let error = Case::<2>::generic()
        .intersect_many(&equations, &[0, 1], &[true; 2], Default::default())
        .unwrap_err();
    assert_eq!(error.failure, CaseIntersectionFailure::UnsupportedGeometry);
    assert_eq!(error.original_conjunction.as_ref(), equations);
}

#[test]
fn constant_or_nonlinear_p_foreign_support_and_large_resultants_fail_closed() {
    let context = CoefficientContext::new(["d", "x", "y", "z"]);
    for input in [
        "x+y^2-2",
        "(y^2+1)*x+y+1",
        "d*x*y+y^2+1",
        "x*y*z+y^2+1",
        "x*y+y^2+18446744073709551616",
        "x*y+1",
        "x*y",
    ] {
        assert!(
            super::refine(
                &polynomial(&context, input),
                &[1, 2, 3],
                Default::default(),
                &mut Default::default(),
                0
            )
            .unwrap()
            .is_none(),
            "{input}"
        );
    }
    let error = Case::<3>::generic()
        .intersect_many(
            &[polynomial(&context, "x*y+y^2+18446744073709551616")],
            &[1, 2, 3],
            &[true; 3],
            Default::default(),
        )
        .unwrap_err();
    assert_eq!(error.failure, CaseIntersectionFailure::UnsupportedGeometry);
}

#[test]
fn complete_parent_siblings_and_unused_original_axes_survive() {
    let context = CoefficientContext::new(["d", "x", "y", "z"]);
    let indices = [1, 2, 3];
    let sector = [true; 3];
    let equation = polynomial(&context, CAPTURED);
    let result = Case::<3>::generic()
        .intersect_many(&[equation.clone()], &indices, &sector, Default::default())
        .unwrap();
    assert_eq!(
        result.cases,
        vec![
            CoordinateCase::new([Some(2), Some(3), None])
                .unwrap()
                .into()
        ]
    );
    for (constraint, expected) in [("2*z-x", Some([2, 3, 1])), ("2*z-x-1", None)] {
        let parent = Case::<3>::generic()
            .intersect(&[polynomial(&context, constraint)], &indices, &sector)
            .unwrap()
            .unwrap();
        let result = parent
            .intersect_many(&[equation.clone()], &indices, &sector, Default::default())
            .unwrap();
        let expected: Vec<Case<3>> = expected
            .into_iter()
            .map(|point| CoordinateCase::new(point.map(Some)).unwrap().into())
            .collect();
        assert_eq!(result.cases, expected);
    }
    let equations = [equation, polynomial(&context, "z^2-1")];
    let result = Case::<3>::generic()
        .intersect_many(&equations, &indices, &sector, Default::default())
        .unwrap();
    assert_eq!(
        result.cases,
        vec![
            CoordinateCase::new([Some(2), Some(3), Some(1)])
                .unwrap()
                .into()
        ]
    );
}

#[test]
fn permutations_rank_scope_and_large_admissible_coordinates_are_not_cut_off() {
    let context = CoefficientContext::new(["d", "y", "x"]);
    let equations = [polynomial(&context, CAPTURED)];
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
        assert_eq!(fixed(&result.cases), BTreeSet::from([[2, 3]]));
        assert_eq!(result.stats.rank_splits, 0);
        assert_eq!(result.stats.integer_resultants, 1);
    }
    assert_eq!(
        fixed(&solve("(y-40)*x+(y-40)^2-2", [true; 2])),
        BTreeSet::from([[1, 41], [1, 38]])
    );
}

#[test]
fn compact_overflow_and_shared_work_algebra_factor_limits_are_atomic() {
    let context = CoefficientContext::new(["x", "y"]);
    let equation = polynomial(&context, CAPTURED);
    for (limits, kind) in [
        (
            CaseIntersectionLimits {
                max_work_items: 3,
                ..Default::default()
            },
            CaseIntersectionBudget::WorkItems,
        ),
        (
            CaseIntersectionLimits {
                max_normalizations: 0,
                ..Default::default()
            },
            CaseIntersectionBudget::Normalizations,
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
            .intersect_many(&[equation.clone()], &[0, 1], &[true; 2], limits)
            .unwrap_err();
        assert!(
            matches!(error.failure,CaseIntersectionFailure::Budget{kind:actual,..} if actual==kind)
        );
        assert_eq!(error.original_conjunction.as_ref(), [equation.clone()]);
    }
    let error = super::refine(
        &equation,
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
    let overflow = polynomial(&context, "(y-1000)*x+(y-1000)^2-2");
    let error = Case::<2>::generic()
        .intersect_many(&[overflow.clone()], &[0, 1], &[true; 2], Default::default())
        .unwrap_err();
    assert!(matches!(
        error.failure,
        CaseIntersectionFailure::Admission(AffineGeometryError::Coordinate(
            GeometryError::CompactOverflow { .. }
        ))
    ));
    assert!(
        Case::<2>::generic()
            .intersect_many(&[overflow], &[0, 1], &[true, false], Default::default())
            .unwrap()
            .cases
            .is_empty()
    );
    let error = super::refine(
        &polynomial(&context, "x*y+y^100+1"),
        &[0, 1],
        CaseIntersectionLimits {
            max_work_items: 10,
            ..Default::default()
        },
        &mut Default::default(),
        0,
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
fn small_nonprimitive_families_match_exhaustive_integer_oracle_and_native_replay() {
    let context = CoefficientContext::new(["x", "y"]);
    // Q=(a*y+b)*(y+s)+r yields F=P*(x+y+s)+r. With nonzero r,
    // P divides r, providing a small exact box for the differential oracle.
    for a in [-2i64, -1, 1, 2] {
        for b in [-2i64, 0, 2] {
            for s in [-1i64, 1] {
                for r in [-2i64, -1, 1, 2] {
                    let input = format!("({a}*y+({b}))*x+({a}*y+({b}))*(y+({s}))+({r})");
                    let equation = polynomial(&context, &input);
                    for sector in [[false, false], [false, true], [true, false], [true, true]] {
                        let result = Case::<2>::generic()
                            .intersect_many(
                                &[equation.clone()],
                                &[0, 1],
                                &sector,
                                Default::default(),
                            )
                            .unwrap();
                        let expected: BTreeSet<_> = (-12i16..=12)
                            .flat_map(|x| (-12i16..=12).map(move |y| [x, y]))
                            .filter(|[x, y]| {
                                (*x > 0) == sector[0]
                                    && (*y > 0) == sector[1]
                                    && (a * i64::from(*y) + b) * (i64::from(*x) + i64::from(*y) + s)
                                        + r
                                        == 0
                            })
                            .collect();
                        assert_eq!(fixed(&result.cases), expected, "{input}; {sector:?}");
                        for [x, y] in fixed(&result.cases) {
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
