//! Native support queries distinguish absent substitutions from nonvanishing.

use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};

fn prepared<const N: usize>(
    context: &CoefficientContext,
    fixed: [Option<i16>; N],
    indices: [usize; N],
    sector: [bool; N],
    equations: &[&str],
) -> AffineDomainRestriction {
    let equations = equations
        .iter()
        .map(|value| context.coefficient_fixture(value).numerator)
        .collect::<Vec<_>>();
    let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
        &CoordinateCase::new(fixed).unwrap(),
        &equations,
        &indices,
        &sector,
    )
    .unwrap() else {
        panic!("test requires a coupled chart")
    };
    AffineApplicationDomain::from_case(&case, &sector)
        .unwrap()
        .prepare_restriction()
        .unwrap()
}

#[test]
fn integral_and_rational_charts_query_actual_pivots() {
    let context = CoefficientContext::new(["d", "n0", "n1", "n2"]);
    for equation in ["n0-n1-n2-1", "2*n0-n1-n2-1"] {
        let chart = prepared(&context, [None; 3], [1, 2, 3], [true; 3], &[equation]);
        for value in ["n1", "d*n1^3+n2-1", "0", "6"] {
            let polynomial = context.coefficient_fixture(value).numerator;
            assert!(!chart.affects_polynomial(&polynomial).unwrap(), "{value}");
            assert_eq!(
                chart.restrict_polynomial_value(&polynomial).unwrap(),
                context.coefficient_fixture(value)
            );
        }
        for value in ["n0", "n0*n1", "n0-n1-n2-1"] {
            assert!(
                chart
                    .affects_polynomial(&context.coefficient_fixture(value).numerator)
                    .unwrap(),
                "{value}"
            );
        }
    }
}

#[test]
fn zero_and_nonzero_fixed_variables_are_not_mistaken_for_free_coordinates() {
    let context = CoefficientContext::new(["d", "n0", "n1", "n2"]);
    for fixed in [0, 3] {
        let chart = prepared(
            &context,
            [None, None, Some(fixed)],
            [1, 2, 3],
            [true, true, fixed > 0],
            &["2*n0-n1"],
        );
        let polynomial = context.coefficient_fixture("n2").numerator;
        assert!(chart.affects_polynomial(&polynomial).unwrap());
        assert_eq!(
            chart.restrict_polynomial_value(&polynomial).unwrap(),
            context.integer(i64::from(fixed))
        );
        assert!(
            !chart
                .affects_polynomial(&context.coefficient_fixture("n1+d").numerator)
                .unwrap()
        );
    }
}

#[test]
fn multiple_pivots_and_permuted_variable_map_keep_physical_axes_distinct() {
    let context = CoefficientContext::new(["n3", "d", "n1", "n0", "n2"]);
    let chart = prepared(
        &context,
        [None, None, None, Some(3)],
        [3, 2, 4, 0],
        [true; 4],
        &["2*n0-n2-4", "3*n1-n2-6"],
    );
    for variable in ["n0", "n1", "n3"] {
        assert!(
            chart
                .affects_polynomial(&context.coefficient_fixture(variable).numerator)
                .unwrap()
        );
    }
    for value in ["n2", "d*n2^2+1"] {
        assert!(
            !chart
                .affects_polynomial(&context.coefficient_fixture(value).numerator)
                .unwrap()
        );
    }
}

#[test]
fn foreign_maps_and_malformed_storage_fail_before_support_inspection() {
    let context = CoefficientContext::new(["d", "n0", "n1"]);
    let chart = prepared(&context, [None; 2], [1, 2], [true; 2], &["2*n0-n1"]);
    let foreign = CoefficientContext::new(["n0", "d", "n1"]);
    assert!(
        chart
            .affects_polynomial(&foreign.coefficient_fixture("1").numerator)
            .is_err()
    );
    let mut malformed = context.coefficient_fixture("n1").numerator;
    malformed.exponents.pop();
    assert!(chart.affects_polynomial(&malformed).is_err());
}

#[test]
fn no_op_query_does_not_normalize_values_or_certify_nonvanishing() {
    let context = CoefficientContext::new(["d", "n0", "n1"]);
    let chart = prepared(&context, [None; 2], [1, 2], [true; 2], &["2*n0-n1"]);
    let coefficient = context.coefficient_fixture("6*(n1-2)");
    let polynomial = coefficient.numerator.clone();
    assert!(!chart.affects_polynomial(&polynomial).unwrap());
    assert_eq!(
        chart.restrict_polynomial_value(&polynomial).unwrap(),
        coefficient
    );
    assert_eq!(
        chart.restrict_coefficient(&coefficient).unwrap(),
        coefficient
    );
    // Its root remains present; the query is not a guard proof.
    assert!(
        polynomial
            .replace(2, &symbolica::prelude::Integer::from(2))
            .is_zero()
    );
    assert_ne!(chart.restrict_equation(&polynomial).unwrap(), polynomial);
    assert!(!chart.affects_polynomial(&context.zero().numerator).unwrap());
}
