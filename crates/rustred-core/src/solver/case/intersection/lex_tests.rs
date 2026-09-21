//! Late elimination-order regression tests: native algebra, complete cases.

use symbolica::prelude::Integer;

use crate::algebra::{CoefficientContext, CoefficientPolynomial};
use crate::solver::{Case, CoordinateCase};

use super::{CaseIntersectionBudget, CaseIntersectionFailure, CaseIntersectionLimits, native};

#[path = "lex_tests/captured.rs"]
mod captured;

fn equations(context: &CoefficientContext, values: &[&str]) -> Vec<CoefficientPolynomial> {
    values
        .iter()
        .map(|value| context.coefficient_fixture(value).numerator)
        .collect()
}

fn captured_context() -> (CoefficientContext, Case<15>, [usize; 15], [bool; 15]) {
    let names: Vec<_> = std::iter::once("d".to_owned())
        .chain((0..15).map(|axis| format!("n{axis}")))
        .collect();
    let context = CoefficientContext::new(names.iter().map(String::as_str));
    let fixed = [
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
        Some(2),
        Some(0),
        Some(1),
        Some(1),
        Some(1),
    ];
    let parent = CoordinateCase::new(fixed).unwrap().into();
    let sector = std::array::from_fn(|axis| b"111011000010111"[axis] == b'1');
    (
        context,
        parent,
        std::array::from_fn(|axis| axis + 1),
        sector,
    )
}

#[test]
fn late_lex_proves_captured_full_residual_empty_without_bounding_positive_powers() {
    let (context, parent, indices, sector) = captured_context();
    let conjunction = equations(
        &context,
        &captured::ORIGINAL
            .into_iter()
            .chain(captured::RESIDUAL)
            .collect::<Vec<_>>(),
    );
    native::take_lex_calls();
    let result = parent
        .intersect_many_with_max_numerator_rank(
            &conjunction,
            &indices,
            &sector,
            CaseIntersectionLimits::default(),
            10,
        )
        .unwrap();
    assert!(result.cases.is_empty());
    assert!(
        native::take_lex_calls() > 0,
        "the captured missing order must actually be exercised"
    );
    assert_eq!(
        result.stats.rank_splits, 0,
        "all free powers are positive, not finite numerator coordinates"
    );
    println!("CAPTURED_LATE_LEX stats={:?}", result.stats);
}

#[test]
fn original_only_capture_keeps_its_positive_ray_or_fails_closed() {
    let (context, parent, indices, sector) = captured_context();
    let conjunction = equations(&context, &captured::ORIGINAL);
    let mut fixed = *parent.fixed();
    fixed[1] = Some(2);
    fixed[2] = Some(3);
    let ray: Case<15> = CoordinateCase::new(fixed).unwrap().into();
    // Prove the entire ray, not sampled integer points, satisfies every
    // original equation. This ray does NOT satisfy the narrowed residual.
    for polynomial in &conjunction {
        let mut restricted = polynomial.clone();
        for (axis, value) in ray.fixed().iter().enumerate() {
            if let Some(value) = value {
                restricted = restricted.replace(indices[axis], &Integer::from(*value));
            }
        }
        assert!(restricted.is_zero());
    }
    match parent.intersect_many_with_max_numerator_rank(
        &conjunction,
        &indices,
        &sector,
        CaseIntersectionLimits::default(),
        10,
    ) {
        Ok(result) => assert!(result.cases.iter().any(|case| case.contains(&ray).unwrap())),
        Err(error) => assert!(matches!(
            error.failure,
            CaseIntersectionFailure::UnsupportedGeometry
                | CaseIntersectionFailure::RepeatedState
                | CaseIntersectionFailure::Budget { .. }
        )),
    }
}

#[test]
fn late_lex_normalization_budget_is_shared_and_failure_retains_full_input() {
    let (context, parent, indices, sector) = captured_context();
    let conjunction = equations(
        &context,
        &captured::ORIGINAL
            .into_iter()
            .chain(captured::RESIDUAL)
            .collect::<Vec<_>>(),
    );
    native::take_lex_calls();
    let error = parent
        .intersect_many_with_max_numerator_rank(
            &conjunction,
            &indices,
            &sector,
            CaseIntersectionLimits {
                max_normalizations: 1,
                ..Default::default()
            },
            10,
        )
        .unwrap_err();
    assert_eq!(
        error.failure,
        CaseIntersectionFailure::Budget {
            kind: CaseIntersectionBudget::Normalizations,
            limit: 1
        }
    );
    assert_eq!(error.original_conjunction.as_ref(), conjunction);
    assert_eq!(error.original_parent, parent);
    assert_eq!(native::take_lex_calls(), 0, "charge before native fallback");
    let error = parent
        .intersect_many_with_max_numerator_rank(
            &conjunction,
            &indices,
            &sector,
            CaseIntersectionLimits {
                max_work_items: 1,
                ..Default::default()
            },
            10,
        )
        .unwrap_err();
    assert_eq!(
        error.failure,
        CaseIntersectionFailure::Budget {
            kind: CaseIntersectionBudget::WorkItems,
            limit: 1
        }
    );
    assert_eq!(error.original_conjunction.as_ref(), conjunction);
    assert_eq!(error.original_parent, parent);
}

#[test]
fn bounded_lex_does_not_turn_an_unsupported_or_sibling_into_success() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let input = equations(&context, &["(z-1)*(x^2-2*y^2-1)"]);
    native::take_lex_calls();
    let error = Case::<3>::generic()
        .intersect_many(
            &input,
            &[0, 1, 2],
            &[true; 3],
            CaseIntersectionLimits::default(),
        )
        .unwrap_err();
    // A successful z=1 sibling cannot authorize discarding a still unsupported
    // branch. The Pell curve remains unsupported, not an empty integer locus.
    assert_eq!(error.failure, CaseIntersectionFailure::UnsupportedGeometry);
    assert_eq!(error.original_conjunction.as_ref(), input);
    assert_eq!(native::take_lex_calls(), 0);
}

#[test]
fn late_lex_structural_admission_counts_support_not_full_context() {
    let context = CoefficientContext::new(["d", "a", "b", "c", "unused"]);
    assert!(native::lex_eligible(&equations(
        &context,
        &["a*b-1", "b*c-a"]
    )));
    assert!(!native::lex_eligible(&equations(&context, &["a*b-1"])));
    assert!(!native::lex_eligible(&equations(
        &context,
        &["a*b-1", "c*unused-1"]
    )));
    assert!(!native::lex_eligible(&equations(
        &context,
        &["a^14+b-1", "a*b-2"]
    )));
    assert!(!native::lex_eligible(&equations(
        &context,
        &["a^3*b^2-1", "a*b-2"]
    )));
    let sixteen = "a^4+b^4+c^4+a^3+b^3+c^3+a^2+b^2+c^2+a+b+c+a*b+a*c+b*c+1";
    let allowed = equations(&context, &[sixteen; 8]);
    assert_eq!(allowed.iter().map(|p| p.nterms()).sum::<usize>(), 128);
    assert!(native::lex_eligible(&allowed));
    let mut too_many_terms = allowed.clone();
    too_many_terms[0] = equations(&context, &[&format!("{sixteen}+a*b*c")]).remove(0);
    assert!(!native::lex_eligible(&too_many_terms));
    assert!(!native::lex_eligible(&equations(&context, &["a*b-1"; 9])));
    assert!(native::lex_eligible(&equations(
        &context,
        &["170141183460469231731687303715884105728*a+b", "a*b-1"]
    )));
    assert!(!native::lex_eligible(&equations(
        &context,
        &["340282366920938463463374607431768211456*a+b", "a*b-1"]
    )));
    assert!(!native::lex_eligible(&equations(
        &context,
        &["-340282366920938463463374607431768211456*a+b", "a*b-1"]
    )));
}

#[test]
fn identical_small_lex_basis_stops_after_one_attempt() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let input = equations(&context, &["x^2-2*y^2-1", "z^2-2*y^2-3"]);
    native::take_lex_calls();
    let error = Case::<3>::generic()
        .intersect_many(
            &input,
            &[0, 1, 2],
            &[true; 3],
            CaseIntersectionLimits::default(),
        )
        .unwrap_err();
    assert_eq!(error.failure, CaseIntersectionFailure::UnsupportedGeometry);
    assert_eq!(native::take_lex_calls(), 1);
    assert_eq!(error.original_conjunction.as_ref(), input);
}

#[test]
fn single_large_degree_conjunction_does_not_invoke_late_lex() {
    let context = CoefficientContext::new(["x", "y"]);
    let input = equations(&context, &["x^14-2*y^14-1"]);
    native::take_lex_calls();
    let error = Case::<2>::generic()
        .intersect_many(
            &input,
            &[0, 1],
            &[true; 2],
            CaseIntersectionLimits::default(),
        )
        .unwrap_err();
    assert_eq!(error.failure, CaseIntersectionFailure::UnsupportedGeometry);
    assert_eq!(native::take_lex_calls(), 0);
}
