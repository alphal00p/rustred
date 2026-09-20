//! Independent soundness checks for the one-sided quadratic emptiness proof.
//! Inputs are generic polynomials and cases, never production family dispatch.

use crate::algebra::CoefficientContext;
use crate::solver::{Case, CoordinateCase};

use super::super::{CaseIntersectionBudget, CaseIntersectionFailure, CaseIntersectionLimits};
use super::proves_empty;

#[test]
fn physical_axis_permutation_and_unused_variables_preserve_the_zero_set() {
    let context = CoefficientContext::new(["parameter", "b", "x", "a", "z"]);
    let equation = context.coefficient_fixture("(a-1)^2+(b+2)^2+z^2").numerator;
    let parent = Case::<4>::generic();
    assert!(!proves_empty(
        &parent,
        &equation,
        &[3, 1, 4, 2],
        &[true, false, false, true],
    ));
    assert!(!proves_empty(
        &parent,
        &equation,
        &[1, 3, 2, 4],
        &[false, true, true, false],
    ));
    assert!(proves_empty(
        &parent,
        &equation,
        &[3, 1, 4, 2],
        &[false, false, false, true],
    ));
}

#[test]
fn negative_orientation_keeps_exact_minimum_and_integer_conditions() {
    let context = CoefficientContext::new(["x", "y"]);
    let parent = Case::<2>::generic();
    for (input, expected) in [
        ("-7*((x-3)^2+(y+2)^2+1)", true),
        ("-7*((x-3)^2+(y+2)^2)", false),
        ("-7*((2*x-1)^2+(3*y+1)^2)", true),
        ("-7*(x^2+y^2-100)", false),
    ] {
        assert_eq!(
            proves_empty(
                &parent,
                &context.coefficient_fixture(input).numerator,
                &[0, 1],
                &[true, false],
            ),
            expected,
            "{input}",
        );
    }
}

#[test]
fn zero_index_is_admitted_only_on_the_nonpositive_sector_side() {
    let context = CoefficientContext::new(["x", "y"]);
    let equation = context.coefficient_fixture("x^2+y^2").numerator;
    let parent = Case::<2>::generic();
    assert!(!proves_empty(&parent, &equation, &[0, 1], &[false; 2]));
    for sector in [[true, false], [false, true], [true, true]] {
        assert!(proves_empty(&parent, &equation, &[0, 1], &sector));
    }
    let matching: Case<2> = CoordinateCase::new([Some(0), None]).unwrap().into();
    assert!(!proves_empty(&matching, &equation, &[0, 1], &[false; 2]));
    let conflicting: Case<2> = CoordinateCase::new([Some(-1), None]).unwrap().into();
    assert!(proves_empty(&conflicting, &equation, &[0, 1], &[false; 2],));
}

#[test]
fn a_negative_minimum_is_never_an_empty_domain_certificate() {
    let context = CoefficientContext::new(["x", "y"]);
    let parent = Case::<2>::generic();
    for input in ["x^2+y^2-100", "(2*x-1)^2+(2*y-1)^2-2"] {
        let equation = context.coefficient_fixture(input).numerator;
        for sector in [[false, false], [true, false], [false, true], [true, true]] {
            assert!(!proves_empty(&parent, &equation, &[0, 1], &sector));
        }
    }
}

#[test]
fn singular_indefinite_parameter_dependent_and_nonquadratic_inputs_stay_unknown() {
    let context = CoefficientContext::new(["d", "x", "y"]);
    let parent = Case::<2>::generic();
    for input in [
        "(x+y)^2+1",
        "x^2-y^2+1",
        "2*x*y+1",
        "x^2+y+1",
        "d*(x^2+y^2+1)",
        "x^2+y^2+d",
        "x^4+y^2+1",
        "x+y+1",
        "0",
    ] {
        assert!(
            !proves_empty(
                &parent,
                &context.coefficient_fixture(input).numerator,
                &[1, 2],
                &[true, false],
            ),
            "{input}",
        );
    }
}

#[test]
fn malformed_index_maps_do_not_create_a_proof() {
    let context = CoefficientContext::new(["x", "y"]);
    let equation = context.coefficient_fixture("x^2+y^2+1").numerator;
    let parent = Case::<2>::generic();
    for indices in [[0, 0], [0, 2], [usize::MAX, 1]] {
        assert!(!proves_empty(&parent, &equation, &indices, &[false; 2]));
    }
}

#[test]
fn unrelated_arity_and_quadratic_storage_ceiling_are_conservative() {
    const LARGE: usize = 65;
    let names = (0..LARGE)
        .map(|axis| format!("x{axis}"))
        .collect::<Vec<_>>();
    let context = CoefficientContext::new(names.iter().map(String::as_str));
    let input = names
        .iter()
        .map(|name| format!("{name}^2"))
        .chain(std::iter::once("1".to_owned()))
        .collect::<Vec<_>>()
        .join("+");
    let equation = context.coefficient_fixture(&input).numerator;
    assert!(!proves_empty(
        &Case::<LARGE>::generic(),
        &equation,
        &std::array::from_fn(|axis| axis),
        &[false; LARGE],
    ));
    // The limit counts supported coordinates, not family arity. A large
    // family with a small quadratic support is still eligible.
    let small_support = context.coefficient_fixture("x3^2+x61^2+1").numerator;
    assert!(proves_empty(
        &Case::<LARGE>::generic(),
        &small_support,
        &std::array::from_fn(|axis| axis),
        &[false; LARGE],
    ));
}

#[test]
fn empty_factor_cannot_hide_an_unresolved_or_sibling() {
    let context = CoefficientContext::new(["x", "y"]);
    let equation = context
        .coefficient_fixture("(x^2+y^2+1)*(x^2+y^2-1)")
        .numerator;
    let error = Case::<2>::generic()
        .intersect_many(
            &[equation.clone()],
            &[0, 1],
            &[true, false],
            Default::default(),
        )
        .unwrap_err();
    assert_eq!(error.failure, CaseIntersectionFailure::UnsupportedGeometry);
    assert_eq!(error.original_conjunction.as_ref(), &[equation]);
}

#[test]
fn existing_work_factor_and_term_budgets_cannot_be_bypassed() {
    let context = CoefficientContext::new(["x", "y"]);
    let equation = context.coefficient_fixture("x^2+y^2+1").numerator;
    for (limits, kind, limit) in [
        (
            CaseIntersectionLimits {
                max_work_items: 0,
                ..Default::default()
            },
            CaseIntersectionBudget::WorkItems,
            0,
        ),
        (
            CaseIntersectionLimits {
                max_factorizations: 0,
                ..Default::default()
            },
            CaseIntersectionBudget::Factorizations,
            0,
        ),
        (
            CaseIntersectionLimits {
                max_terms_per_conjunction: 1,
                ..Default::default()
            },
            CaseIntersectionBudget::ConjunctionTerms,
            1,
        ),
    ] {
        let error = Case::<2>::generic()
            .intersect_many(&[equation.clone()], &[0, 1], &[false; 2], limits)
            .unwrap_err();
        assert_eq!(
            error.failure,
            CaseIntersectionFailure::Budget { kind, limit }
        );
    }
}
