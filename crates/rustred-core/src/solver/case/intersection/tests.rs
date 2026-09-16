use crate::algebra::{CoefficientContext, CoefficientPolynomial};

use super::super::{AffineGeometryError, Case, CoordinateCase};
use super::{CaseIntersectionBudget, CaseIntersectionFailure, CaseIntersectionLimits, native};

fn equations(context: &CoefficientContext, input: &[&str]) -> Vec<CoefficientPolynomial> {
    input
        .iter()
        .map(|expression| context.coefficient_fixture(expression).numerator)
        .collect()
}

fn expected<const N: usize>(
    context: &CoefficientContext,
    expressions: &[&str],
    indices: &[usize; N],
    sector: &[bool; N],
) -> Case<N> {
    Case::generic()
        .intersect(&equations(context, expressions), indices, sector)
        .unwrap()
        .unwrap()
}

fn same_union<const N: usize>(actual: &[Case<N>], expected: &[Case<N>]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?} != {expected:?}");
    for case in expected {
        assert!(actual.contains(case), "missing {case:?} from {actual:?}");
    }
}

#[test]
fn product_is_a_complete_union_and_not_an_intersection() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let result = Case::<3>::generic()
        .intersect_many(
            &equations(&context, &["(a-b)*(c-1)"]),
            &[0, 1, 2],
            &[true; 3],
            CaseIntersectionLimits::default(),
        )
        .unwrap();
    same_union(
        &result.cases,
        &[
            expected(&context, &["a-b"], &[0, 1, 2], &[true; 3]),
            expected(&context, &["c-1"], &[0, 1, 2], &[true; 3]),
        ],
    );
    assert_eq!(result.stats.factor_children, 2);
    assert_eq!(result.stats.normalizations, 0);
}

#[test]
fn every_and_sibling_and_the_parent_survive_distribution() {
    let context = CoefficientContext::new(["a", "b", "c", "d"]);
    let parent: Case<4> = CoordinateCase::new([Some(3), None, None, None])
        .unwrap()
        .into();
    let result = parent
        .intersect_many(
            &equations(&context, &["(a-b)*(c-1)", "d-2"]),
            &[0, 1, 2, 3],
            &[true; 4],
            CaseIntersectionLimits::default(),
        )
        .unwrap();
    same_union(
        &result.cases,
        &[
            expected(&context, &["a-3", "b-3", "d-2"], &[0, 1, 2, 3], &[true; 4]),
            expected(&context, &["a-3", "c-1", "d-2"], &[0, 1, 2, 3], &[true; 4]),
        ],
    );
}

#[test]
fn repeated_content_and_overlapping_products_prune_only_proved_containment() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let result = Case::<3>::generic()
        .intersect_many(
            &equations(&context, &["-65537*(a-1)^2*(b-2)", "(a-1)*(c-3)"]),
            &[0, 1, 2],
            &[true; 3],
            CaseIntersectionLimits::default(),
        )
        .unwrap();
    same_union(
        &result.cases,
        &[
            expected(&context, &["a-1"], &[0, 1, 2], &[true; 3]),
            expected(&context, &["b-2", "c-3"], &[0, 1, 2], &[true; 3]),
        ],
    );
}

#[test]
fn zero_constant_and_empty_conjunction_have_distinct_meanings() {
    let context = CoefficientContext::new(["a", "b"]);
    let parent: Case<2> = CoordinateCase::new([Some(2), None]).unwrap().into();
    for input in [vec![], vec!["0"], vec!["0", "0"]] {
        let result = parent
            .intersect_many(
                &equations(&context, &input),
                &[0, 1],
                &[true; 2],
                CaseIntersectionLimits::default(),
            )
            .unwrap();
        assert_eq!(result.cases, vec![parent.clone()]);
        assert_eq!(result.stats.factorizations, 0);
        assert_eq!(result.stats.normalizations, 0);
    }
    for input in [vec!["1"], vec!["a^2-2", "-7"], vec!["-7", "a^2-2"]] {
        let result = parent
            .intersect_many(
                &equations(&context, &input),
                &[0, 1],
                &[true; 2],
                CaseIntersectionLimits::default(),
            )
            .unwrap();
        assert!(result.cases.is_empty());
        assert_eq!(result.stats.factorizations, 0);
    }
}

#[test]
fn all_linear_information_is_collected_before_nonlinear_admission() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let input = ["a*b+c-5", "a-b", "b-2"];
    let mut baseline = None;
    for permutation in [[0, 1, 2], [2, 0, 1], [1, 2, 0]] {
        let result = Case::<3>::generic()
            .intersect_many(
                &equations(&context, &permutation.map(|slot| input[slot])),
                &[0, 1, 2],
                &[true; 3],
                CaseIntersectionLimits::default(),
            )
            .unwrap();
        assert_eq!(
            result.cases,
            vec![
                CoordinateCase::new([Some(2), Some(2), Some(1)])
                    .unwrap()
                    .into()
            ]
        );
        assert_eq!(result.stats.normalizations, 0);
        assert_eq!(result.stats.factorizations, 0);
        if let Some(previous) = &baseline {
            assert_eq!(&result.cases, previous);
        }
        baseline = Some(result.cases);
    }
}

#[test]
fn a_joint_groebner_basis_exposes_a_coupled_affine_domain() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let result = Case::<3>::generic()
        .intersect_many(
            &equations(&context, &["(a-b)*c", "(a-b)*(c-1)"]),
            &[0, 1, 2],
            &[true; 3],
            CaseIntersectionLimits::default(),
        )
        .unwrap();
    assert_eq!(
        result.cases,
        vec![expected(&context, &["a-b"], &[0, 1, 2], &[true; 3])]
    );
    assert_eq!(result.stats.normalizations, 1);
    assert_eq!(result.stats.factorizations, 0);
}

#[test]
fn chart_restriction_can_expose_new_factors() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let parent = expected(&context, &["a-b"], &[0, 1, 2], &[true; 3]);
    let result = parent
        .intersect_many(
            &equations(&context, &["a*b-c^2"]),
            &[0, 1, 2],
            &[true; 3],
            CaseIntersectionLimits::default(),
        )
        .unwrap();
    // The b=-c branch is exactly excluded by the positive sector bounds.
    assert_eq!(
        result.cases,
        vec![expected(&context, &["a-b", "b-c"], &[0, 1, 2], &[true; 3])]
    );
}

#[test]
fn rational_chart_preserves_integer_parity_and_physical_integral_axes() {
    let context = CoefficientContext::new(["a", "b"]);
    let parent = expected(&context, &["2*a-b-1"], &[0, 1], &[true; 2]);
    assert!(
        parent
            .integral()
            .powers()
            .iter()
            .all(|power| power.is_symbolic())
    );
    assert!(!parent.affine().unwrap().has_integral_chart());
    let accepted = parent
        .intersect_many(
            &equations(&context, &["b^2-9"]),
            &[0, 1],
            &[true; 2],
            CaseIntersectionLimits::default(),
        )
        .unwrap();
    assert_eq!(
        accepted.cases,
        vec![CoordinateCase::new([Some(2), Some(3)]).unwrap().into()]
    );
    let empty = parent
        .intersect_many(
            &equations(&context, &["b^2-4"]),
            &[0, 1],
            &[true; 2],
            CaseIntersectionLimits::default(),
        )
        .unwrap();
    assert!(empty.cases.is_empty());
    // Values and zero equations deliberately have separate APIs.
    let value = equations(&context, &["a"]).remove(0);
    assert_eq!(
        parent
            .affine()
            .unwrap()
            .restrict_polynomial_value(&value)
            .unwrap(),
        context.coefficient_fixture("(b+1)/2")
    );
    assert_eq!(
        native::primitive(parent.affine().unwrap().restrict_equation(&value).unwrap()),
        equations(&context, &["b+1"]).remove(0)
    );
}

#[test]
fn an_unsupported_sibling_rejects_the_entire_union_atomically() {
    let context = CoefficientContext::new(["a", "b"]);
    let parent = Case::<2>::generic();
    let conjunction = equations(&context, &["(a-1)*(a^2+b^2-1)"]);
    let error = parent
        .intersect_many(
            &conjunction,
            &[0, 1],
            &[true; 2],
            CaseIntersectionLimits::default(),
        )
        .unwrap_err();
    assert_eq!(error.failure, CaseIntersectionFailure::UnsupportedGeometry);
    assert_eq!(error.original_parent, parent);
    assert_eq!(error.original_conjunction.as_ref(), conjunction);
    assert!(!error.unresolved_conjunction.is_empty());
}

#[test]
fn compact_overflow_is_atomic_even_when_another_factor_is_admissible() {
    let context = CoefficientContext::new(["a"]);
    let error = Case::<1>::generic()
        .intersect_many(
            &equations(&context, &["(a-1)*(a-1000)"]),
            &[0],
            &[true],
            CaseIntersectionLimits::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error.failure,
        CaseIntersectionFailure::Admission(AffineGeometryError::Coordinate(
            crate::solver::GeometryError::CompactOverflow { .. }
        ))
    ));
}

#[test]
fn work_factorization_normalization_and_term_budgets_fail_closed() {
    let context = CoefficientContext::new(["a", "b"]);
    for (input, limits, kind) in [
        (
            vec!["a-1"],
            CaseIntersectionLimits {
                max_work_items: 0,
                ..Default::default()
            },
            CaseIntersectionBudget::WorkItems,
        ),
        (
            vec!["(a-1)*(a-2)"],
            CaseIntersectionLimits {
                max_work_items: 2,
                ..Default::default()
            },
            CaseIntersectionBudget::WorkItems,
        ),
        (
            vec!["a^2-1"],
            CaseIntersectionLimits {
                max_factorizations: 0,
                ..Default::default()
            },
            CaseIntersectionBudget::Factorizations,
        ),
        (
            vec!["a*b-1", "a*b-2"],
            CaseIntersectionLimits {
                max_normalizations: 0,
                ..Default::default()
            },
            CaseIntersectionBudget::Normalizations,
        ),
        (
            vec!["a+b-1"],
            CaseIntersectionLimits {
                max_terms_per_conjunction: 2,
                ..Default::default()
            },
            CaseIntersectionBudget::ConjunctionTerms,
        ),
    ] {
        let error = Case::<2>::generic()
            .intersect_many(&equations(&context, &input), &[0, 1], &[true; 2], limits)
            .unwrap_err();
        assert!(
            matches!(error.failure, CaseIntersectionFailure::Budget {kind: actual, ..} if actual == kind)
        );
    }
}

#[test]
fn admission_rejects_parameter_or_mismatched_index_maps_even_for_zero_work() {
    let context = CoefficientContext::new(["a", "d", "b"]);
    for (input, indices) in [
        (vec!["d-4"], [0, 2]),
        (vec!["a"], [0, 0]),
        (vec!["a"], [0, 3]),
    ] {
        let error = Case::<2>::generic()
            .intersect_many(
                &equations(&context, &input),
                &indices,
                &[true; 2],
                Default::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error.failure,
            CaseIntersectionFailure::InvalidInput(_)
        ));
    }
    let parent = expected(&context, &["a-b"], &[0, 2], &[true; 2]);
    let error = parent
        .intersect_many(&[], &[2, 0], &[true; 2], Default::default())
        .unwrap_err();
    assert!(matches!(
        error.failure,
        CaseIntersectionFailure::InvalidInput(_)
    ));
}

#[test]
fn actual_fam112_conjunction_splits_after_its_fixed_coordinate_is_absorbed() {
    // Captured sector 111100001010111; indices are not the variable-map prefix.
    let names = ["d".to_owned(), "x".to_owned()]
        .into_iter()
        .chain((0..15).map(|axis| format!("n{axis}")))
        .collect::<Vec<_>>();
    let context = CoefficientContext::new(names.iter().map(String::as_str));
    let indices = std::array::from_fn(|axis| axis + 2);
    let sector = std::array::from_fn(|axis| b"111100001010111"[axis] == b'1');
    let parent: Case<15> = CoordinateCase::new([
        Some(1),
        Some(1),
        Some(1),
        None,
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        None,
        Some(0),
        None,
        Some(0),
        None,
        None,
        None,
    ])
    .unwrap()
    .into();
    let conjunction = equations(
        &context,
        &[
            "-1+n14-n14^2-2*n12+n12*n14+2*n12^2-4*n10+2*n10*n14+2*n10*n12+4*n8-2*n8*n14-2*n8*n12-n3+n3*n12",
            "n12-1",
        ],
    );
    let result = parent
        .intersect_many(&conjunction, &indices, &sector, Default::default())
        .unwrap();
    let required = [
        parent
            .intersect(&equations(&context, &["n12-1", "n14-1"]), &indices, &sector)
            .unwrap()
            .unwrap(),
        parent
            .intersect(
                &equations(&context, &["n12-1", "n14-1-2*n10+2*n8"]),
                &indices,
                &sector,
            )
            .unwrap()
            .unwrap(),
    ];
    same_union(&result.cases, &required);
    assert!(
        result
            .cases
            .iter()
            .all(|case| parent.contains(case).unwrap())
    );
    assert_eq!(result.stats.factor_children, 2);
}
