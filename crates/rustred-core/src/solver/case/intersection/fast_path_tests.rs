use crate::algebra::CoefficientContext;

use super::super::{Case, CoordinateCase};
use super::{CaseIntersectionBudget, CaseIntersectionFailure, CaseIntersectionLimits};

#[test]
fn affine_only_admission_avoids_restriction_normalization_and_factorization() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equations = ["a-b", "b-c", "0"].map(|input| context.coefficient_fixture(input).numerator);
    let result = Case::<3>::generic()
        .intersect_many(&equations, &[1, 2, 3], &[true; 3], Default::default())
        .unwrap();
    let expected = Case::<3>::generic()
        .intersect(&equations, &[1, 2, 3], &[true; 3])
        .unwrap()
        .unwrap();
    assert_eq!(result.cases, [expected]);
    assert_eq!(result.stats.work_items, 1);
    assert_eq!(result.stats.affine_admissions, 1);
    assert_eq!(result.stats.restrictions, 0);
    assert_eq!(result.stats.normalizations, 0);
    assert_eq!(result.stats.factorizations, 0);
}

#[test]
fn affine_fast_path_keeps_rational_chart_integer_constraints() {
    let context = CoefficientContext::new(["a", "b"]);
    let parent = Case::<2>::generic()
        .intersect(
            &[context.coefficient_fixture("2*a-b-1").numerator],
            &[0, 1],
            &[true; 2],
        )
        .unwrap()
        .unwrap();
    assert!(!parent.affine().unwrap().has_integral_chart());
    for (equation, expected) in [
        (
            "b-3",
            vec![CoordinateCase::new([Some(2), Some(3)]).unwrap().into()],
        ),
        ("b-2", Vec::new()),
    ] {
        let result = parent
            .intersect_many(
                &[context.coefficient_fixture(equation).numerator],
                &[0, 1],
                &[true; 2],
                Default::default(),
            )
            .unwrap();
        assert_eq!(result.cases, expected);
        assert_eq!(result.stats.restrictions, 0);
        assert_eq!(result.stats.factorizations, 0);
    }
}

#[test]
fn affine_fast_path_rejects_an_integer_empty_incoming_domain_and_bad_map() {
    let context = CoefficientContext::new(["a", "b"]);
    let equation = context.coefficient_fixture("a-b").numerator;
    let parent = Case::<2>::generic()
        .intersect(std::slice::from_ref(&equation), &[0, 1], &[true; 2])
        .unwrap()
        .unwrap();
    for conjunction in [Vec::new(), vec![equation]] {
        let result = parent
            .intersect_many(&conjunction, &[0, 1], &[false, true], Default::default())
            .unwrap();
        assert!(result.cases.is_empty());
        assert_eq!(result.stats.affine_admissions, 0);
        assert_eq!(result.stats.restrictions, 0);
    }
    let error = parent
        .intersect_many(&[], &[1, 0], &[true; 2], Default::default())
        .unwrap_err();
    assert!(matches!(
        error.failure,
        CaseIntersectionFailure::InvalidInput(_)
    ));
}

#[test]
fn affine_fast_path_still_spends_work_and_checks_term_budgets() {
    let context = CoefficientContext::new(["a", "b"]);
    let input = [context.coefficient_fixture("a-b-1").numerator];
    for (limits, expected) in [
        (
            CaseIntersectionLimits {
                max_work_items: 0,
                ..Default::default()
            },
            CaseIntersectionBudget::WorkItems,
        ),
        (
            CaseIntersectionLimits {
                max_terms_per_conjunction: 2,
                ..Default::default()
            },
            CaseIntersectionBudget::ConjunctionTerms,
        ),
    ] {
        let error = Case::<2>::generic()
            .intersect_many(&input, &[0, 1], &[true; 2], limits)
            .unwrap_err();
        assert!(
            matches!(error.failure, CaseIntersectionFailure::Budget {kind, ..} if kind == expected)
        );
        assert_eq!(error.stats.affine_admissions, 0);
        assert_eq!(error.original_conjunction.as_ref(), input);
    }
}
