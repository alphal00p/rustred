//! Full-conjunction native inconsistency can discharge only a false overflow.

use super::{CaseIntersectionBudget, CaseIntersectionFailure, CaseIntersectionLimits};
use crate::algebra::{CoefficientContext, CoefficientPolynomial};
use crate::solver::{AffineGeometryError, Case, CoordinateCase, GeometryError};
use symbolica::prelude::Integer;

fn equations(context: &CoefficientContext, values: &[&str]) -> Vec<CoefficientPolynomial> {
    values
        .iter()
        .map(|value| context.coefficient_fixture(value).numerator)
        .collect()
}

#[test]
fn captured_full_residual_discards_only_the_inconsistent_large_pair() {
    let context = CoefficientContext::new(
        std::iter::once("d".to_owned()).chain((0..15).map(|axis| format!("n{axis}"))),
    );
    let parent: Case<15> = CoordinateCase::new([
        None,
        None,
        None,
        Some(0),
        Some(1),
        Some(2),
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
    .unwrap()
    .into();
    let indices = std::array::from_fn(|axis| axis + 1);
    let sector = std::array::from_fn(|axis| b"111011000010111"[axis] == b'1');
    let conjunction = equations(
        &context,
        &[
            "n2-113",
            "-11370+14663*n2-6594*n2^2+901*n2^3+1200*n1",
            "-70+62*n2-24*n2^2-n1+17*n1*n2",
            "160-64*n2+16*n2^2-90*n1+17*n1^2",
            "-70-40*n2+10*n2^2+50*n1-51*n0+17*n0*n2",
            "300-256*n2+64*n2^2-54*n1-34*n0+17*n0*n1",
            "n1-156",
        ],
    );
    for rank in [None, Some(10)] {
        let result = if let Some(rank) = rank {
            parent.intersect_many_with_max_numerator_rank(
                &conjunction,
                &indices,
                &sector,
                CaseIntersectionLimits::default(),
                rank,
            )
        } else {
            parent.intersect_many(
                &conjunction,
                &indices,
                &sector,
                CaseIntersectionLimits::default(),
            )
        }
        .unwrap();
        assert!(result.cases.is_empty());
        assert_eq!(result.stats.normalizations, 1);
        assert_eq!(result.stats.rank_splits, 0);
    }
}

#[test]
fn genuine_large_positive_point_keeps_original_overflow() {
    let context = CoefficientContext::new(["x", "y"]);
    let parent = Case::<2>::generic();
    let conjunction = equations(&context, &["x-156", "y^2-1"]);
    let error = parent
        .intersect_many_with_max_numerator_rank(
            &conjunction,
            &[0, 1],
            &[true; 2],
            CaseIntersectionLimits::default(),
            10,
        )
        .unwrap_err();
    assert!(matches!(error.failure, CaseIntersectionFailure::Admission(
        AffineGeometryError::Coordinate(GeometryError::CompactOverflow { axis: 0, ref value })
    ) if value == &Integer::from(156)));
    assert_eq!(error.stats.normalizations, 1);
    assert_eq!(error.original_conjunction.as_ref(), conjunction);
    assert_eq!(error.original_parent, parent);
}

#[test]
fn pure_affine_large_point_does_not_invoke_new_native_work() {
    let context = CoefficientContext::new(["x", "y"]);
    let error = Case::<2>::generic()
        .intersect_many(
            &equations(&context, &["x-156", "y-1"]),
            &[0, 1],
            &[true; 2],
            CaseIntersectionLimits::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error.failure,
        CaseIntersectionFailure::Admission(AffineGeometryError::Coordinate(
            GeometryError::CompactOverflow { axis: 0, .. }
        ))
    ));
    assert_eq!(error.stats.normalizations, 0);
}

#[test]
fn integer_empty_nonunit_ideal_is_not_mistaken_for_exact_inconsistency() {
    let context = CoefficientContext::new(["x", "y"]);
    let error = Case::<2>::generic()
        .intersect_many(
            &equations(&context, &["x-156", "y^2+1"]),
            &[0, 1],
            &[true; 2],
            CaseIntersectionLimits::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error.failure,
        CaseIntersectionFailure::Admission(AffineGeometryError::Coordinate(
            GeometryError::CompactOverflow { axis: 0, .. }
        ))
    ));
    assert_eq!(error.stats.normalizations, 1);
}

#[test]
fn exhausted_native_budget_preserves_full_original_conjunction() {
    let context = CoefficientContext::new(["x", "y"]);
    let parent = Case::<2>::generic();
    let conjunction = equations(&context, &["x-156", "x^2+1", "y-1"]);
    let error = parent
        .intersect_many(
            &conjunction,
            &[0, 1],
            &[true; 2],
            CaseIntersectionLimits {
                max_normalizations: 0,
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(matches!(
        error.failure,
        CaseIntersectionFailure::Budget {
            kind: CaseIntersectionBudget::Normalizations,
            limit: 0,
        }
    ));
    assert_eq!(error.original_parent, parent);
    assert_eq!(error.original_conjunction.as_ref(), conjunction);
}

#[test]
fn valid_union_sibling_is_never_removed_with_the_inconsistent_component() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let result = Case::<3>::generic()
        .intersect_many(
            &equations(&context, &["(z-1)*(x-156)", "(z-1)*(x^2+1)"]),
            &[0, 1, 2],
            &[true; 3],
            CaseIntersectionLimits::default(),
        )
        .unwrap();
    assert_eq!(
        result.cases,
        vec![CoordinateCase::new([None, None, Some(1)]).unwrap().into()]
    );
}
