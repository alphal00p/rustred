use crate::algebra::{CoefficientContext, CoefficientPolynomial};

use super::super::{Case, CoordinateCase};
use super::{CaseIntersectionBudget, CaseIntersectionFailure, CaseIntersectionLimits, native};

fn equations(context: &CoefficientContext, input: &[&str]) -> Vec<CoefficientPolynomial> {
    input
        .iter()
        .map(|expression| context.coefficient_fixture(expression).numerator)
        .collect()
}

fn ten_index_context() -> CoefficientContext {
    let names = ["d".to_owned(), "x".to_owned()]
        .into_iter()
        .chain((0..10).map(|axis| format!("n{axis}")))
        .collect::<Vec<_>>();
    CoefficientContext::new(names.iter().map(String::as_str))
}

#[test]
fn actual_bmw_quadratic_has_no_integer_exceptional_case() {
    let context = ten_index_context();
    let indices = std::array::from_fn(|axis| axis + 2);
    let sector = std::array::from_fn(|axis| (223 & (1 << axis)) != 0);
    let parent: Case<10> = CoordinateCase::new([
        Some(1),
        None,
        Some(1),
        Some(1),
        Some(1),
        Some(0),
        Some(1),
        Some(1),
        Some(0),
        Some(0),
    ])
    .unwrap()
    .into();
    let result = parent
        .intersect_many(
            &equations(&context, &["n1^2-3*n1+4"]),
            &indices,
            &sector,
            Default::default(),
        )
        .unwrap();
    assert!(result.cases.is_empty());
    assert_eq!(result.stats.factorizations, 1);
    assert_eq!(result.stats.factor_children, 0);
    assert_eq!(result.stats.work_items, 1);
}

#[test]
fn actual_x_quadratic_has_no_integer_exceptional_case_on_its_rational_chart() {
    let context = ten_index_context();
    let indices = std::array::from_fn(|axis| axis + 2);
    let sector = std::array::from_fn(|axis| (214 & (1 << axis)) != 0);
    let coordinate: Case<10> = CoordinateCase::new([
        Some(0),
        Some(1),
        Some(1),
        Some(0),
        Some(1),
        None,
        Some(1),
        Some(1),
        None,
        Some(0),
    ])
    .unwrap()
    .into();
    let parent = coordinate
        .intersect(&equations(&context, &["2*n5-n8-1"]), &indices, &sector)
        .unwrap()
        .unwrap();
    assert!(!parent.affine().unwrap().has_integral_chart());
    let result = parent
        .intersect_many(
            &equations(&context, &["n8^2-8*n8+3"]),
            &indices,
            &sector,
            Default::default(),
        )
        .unwrap();
    assert!(result.cases.is_empty());
    assert_eq!(result.stats.factorizations, 1);
    assert_eq!(result.stats.factor_children, 0);
}

#[test]
fn root_free_factors_preserve_linear_roots_and_the_entire_parent_conjunction() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let parent: Case<3> = CoordinateCase::new([None, None, Some(3)]).unwrap().into();
    for polynomial in ["(a-1)*(a^2+1)", "-7*(a-1)^2*(a^2+1)^2"] {
        let result = parent
            .intersect_many(
                &equations(&context, &[polynomial, "b-2"]),
                &[0, 1, 2],
                &[true; 3],
                Default::default(),
            )
            .unwrap();
        assert_eq!(
            result.cases,
            [CoordinateCase::new([Some(1), Some(2), Some(3)])
                .unwrap()
                .into()]
        );
        assert_eq!(result.stats.factor_children, 1);
    }
}

#[test]
fn a_reducible_univariate_quadratic_keeps_both_sector_appropriate_roots() {
    let context = CoefficientContext::new(["a"]);
    for (positive, root) in [(true, 1), (false, -1)] {
        let result = Case::<1>::generic()
            .intersect_many(
                &equations(&context, &["a^2-1"]),
                &[0],
                &[positive],
                Default::default(),
            )
            .unwrap();
        assert_eq!(
            result.cases,
            [CoordinateCase::new([Some(root)]).unwrap().into()]
        );
        assert_eq!(result.stats.factor_children, 2);
    }
}

#[test]
fn nonintegral_linear_roots_still_pass_through_integer_admission() {
    let context = CoefficientContext::new(["a"]);
    let result = Case::<1>::generic()
        .intersect_many(
            &equations(&context, &["(2*a-1)*(a^2+1)"]),
            &[0],
            &[true],
            Default::default(),
        )
        .unwrap();
    assert!(result.cases.is_empty());
    assert_eq!(result.stats.factor_children, 1);
    assert_eq!(result.stats.affine_admissions, 1);
}

#[test]
fn an_integer_root_free_factor_does_not_discard_coupled_or_siblings() {
    let context = CoefficientContext::new(["a", "b"]);
    for polynomial in [
        "a^2+b^2-1",
        "(a^2+1)*(a^2+b^2-1)",
        "(a-1)*(b^2+1)*(a^2+b^2-1)",
    ] {
        let conjunction = equations(&context, &[polynomial]);
        let error = Case::<2>::generic()
            .intersect_many(&conjunction, &[0, 1], &[true, false], Default::default())
            .unwrap_err();
        assert_eq!(error.failure, CaseIntersectionFailure::UnsupportedGeometry);
        assert_eq!(error.original_conjunction.as_ref(), conjunction);
    }
}

#[test]
fn root_free_decisions_follow_coordinate_and_affine_restriction() {
    let context = CoefficientContext::new(["a", "b"]);
    let coordinate: Case<2> = CoordinateCase::new([None, Some(1)]).unwrap().into();
    let affine = Case::<2>::generic()
        .intersect(&equations(&context, &["a-b"]), &[0, 1], &[true; 2])
        .unwrap()
        .unwrap();
    for (parent, polynomial) in [(coordinate, "a^2+b^2-3*a+3"), (affine, "a^2+b^2+1")] {
        let result = parent
            .intersect_many(
                &equations(&context, &[polynomial]),
                &[0, 1],
                &[true; 2],
                Default::default(),
            )
            .unwrap();
        assert!(result.cases.is_empty());
        assert_eq!(result.stats.factorizations, 1);
    }
}

#[test]
fn parameter_support_and_invalid_or_foreign_maps_fail_before_root_free_filtering() {
    let context = CoefficientContext::new(["a", "d", "b"]);
    let foreign = CoefficientContext::new(["a", "x", "b"]);
    for (conjunction, indices) in [
        (equations(&context, &["d^2+1"]), [0, 2]),
        (equations(&context, &["a^2+d"]), [0, 2]),
        (equations(&context, &["a^2+1"]), [0, 0]),
        (equations(&context, &["a^2+1"]), [0, 3]),
        (
            vec![
                equations(&context, &["a^2+1"]).remove(0),
                equations(&foreign, &["a^2+1"]).remove(0),
            ],
            [0, 2],
        ),
    ] {
        let error = Case::<2>::generic()
            .intersect_many(&conjunction, &indices, &[true; 2], Default::default())
            .unwrap_err();
        assert!(matches!(
            error.failure,
            CaseIntersectionFailure::InvalidInput(_)
        ));
        assert_eq!(error.stats.factorizations, 0);
    }
    let parent = Case::<2>::generic()
        .intersect(&equations(&context, &["a-b"]), &[0, 2], &[true; 2])
        .unwrap()
        .unwrap();
    for (conjunction, indices) in [
        (equations(&foreign, &["a^2+1"]), [0, 2]),
        (equations(&context, &["a^2+1"]), [2, 0]),
    ] {
        let error = parent
            .intersect_many(&conjunction, &indices, &[true; 2], Default::default())
            .unwrap_err();
        assert!(matches!(
            error.failure,
            CaseIntersectionFailure::InvalidInput(_)
        ));
    }
}

#[test]
fn root_free_proofs_preserve_all_existing_budgets() {
    let context = CoefficientContext::new(["a", "b"]);
    for (input, limits, expected) in [
        (
            vec!["a^2+1"],
            CaseIntersectionLimits {
                max_work_items: 0,
                ..Default::default()
            },
            CaseIntersectionBudget::WorkItems,
        ),
        (
            vec!["a^2+1"],
            CaseIntersectionLimits {
                max_factorizations: 0,
                ..Default::default()
            },
            CaseIntersectionBudget::Factorizations,
        ),
        (
            vec!["a^2+1", "b^2+1"],
            CaseIntersectionLimits {
                max_normalizations: 0,
                ..Default::default()
            },
            CaseIntersectionBudget::Normalizations,
        ),
        (
            vec!["a^2+1"],
            CaseIntersectionLimits {
                max_terms_per_conjunction: 1,
                ..Default::default()
            },
            CaseIntersectionBudget::ConjunctionTerms,
        ),
        (
            // The two-term input factors into six terms, including the two
            // root-free terms. Charge them before the filter removes them.
            vec!["a^4-1"],
            CaseIntersectionLimits {
                max_terms_per_conjunction: 4,
                ..Default::default()
            },
            CaseIntersectionBudget::ConjunctionTerms,
        ),
    ] {
        let error = Case::<2>::generic()
            .intersect_many(&equations(&context, &input), &[0, 1], &[true; 2], limits)
            .unwrap_err();
        assert!(
            matches!(error.failure, CaseIntersectionFailure::Budget { kind, .. } if kind == expected)
        );
    }
}

#[test]
fn unexpected_empty_native_factorization_is_not_an_empty_locus_certificate() {
    let context = CoefficientContext::new(["a"]);
    for polynomial in equations(&context, &["0", "1"]) {
        assert_eq!(
            native::factors(&polynomial),
            Err(CaseIntersectionFailure::NativeAlgebra)
        );
    }
}
