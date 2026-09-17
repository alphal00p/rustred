//! Replay guard admission must retain the same complete OR as discovery.

use super::*;
use crate::algebra::{CoefficientContext, CoefficientPolynomial};
use crate::solver::{AffineGeometryError, CoordinateCase, ExceptionalConditions};

fn equations(context: &CoefficientContext, input: &[&str]) -> Vec<CoefficientPolynomial> {
    input
        .iter()
        .map(|text| context.coefficient_fixture(text).numerator)
        .collect()
}

fn candidate<const N: usize>(case: Case<N>) -> RuleCandidate<N> {
    RuleCandidate {
        target: case.integral(),
        case,
        rhs: Vec::new(),
        sources: Vec::new(),
        stats: Default::default(),
    }
}

#[test]
fn nonlinear_guard_conjunction_uses_its_linear_sibling_and_existing_parent_exception() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let indices = [1, 2, 3];
    let sector = [false; 3];
    let parent = Case::generic()
        .intersect(&equations(&context, &["-1-c-b+2*a"]), &indices, &sector)
        .unwrap()
        .unwrap();
    let branch = equations(&context, &["-c-c^2+14*b+3*b*c+6*b^2", "b"]);
    // An exact captured algebra pattern, with generic variable names: b=0
    // exposes c*(c+1)=0; c=0 would force a=1/2 and is inadmissible.
    assert!(matches!(
        parent.intersect(&branch, &indices, &sector),
        Err(AffineGeometryError::UnsupportedNonlinear { .. })
    ));
    let existing = parent
        .intersect(&equations(&context, &["a"]), &indices, &sector)
        .unwrap()
        .unwrap();
    let rule = candidate(parent);
    let exceptions = ExceptionalConditions {
        branches: vec![branch],
    };
    let mut additional = Vec::new();
    append_exceptions(
        &rule,
        exceptions.clone(),
        &[],
        &mut additional,
        &indices,
        &sector,
    )
    .unwrap();
    assert_eq!(
        additional,
        vec![
            CoordinateCase::new([Some(0), Some(0), Some(-1)])
                .unwrap()
                .into()
        ]
    );
    additional.clear();
    append_exceptions(
        &rule,
        exceptions,
        &[existing],
        &mut additional,
        &indices,
        &sector,
    )
    .unwrap();
    assert!(
        additional.is_empty(),
        "the complete branch is already excluded"
    );
}

#[test]
fn nonlinear_guard_with_zero_axis_keeps_exact_integer_child() {
    let context = CoefficientContext::new(["n0", "n3", "n7"]);
    let indices = [0, 1, 2];
    let sector = [false; 3];
    let parent = Case::generic()
        .intersect(&equations(&context, &["-1-n7-n3+2*n0"]), &indices, &sector)
        .unwrap()
        .unwrap();
    let branch = equations(&context, &["-n7-n7^2+14*n3+3*n3*n7+6*n3^2", "n3"]);

    // The singleton API cannot soundly admit this mixed nonlinear/affine
    // conjunction, while the exact disjunctive service refines it to the
    // only integer child n0=0, n3=0, n7=-1.
    assert!(matches!(
        parent.intersect(&branch, &indices, &sector),
        Err(AffineGeometryError::UnsupportedNonlinear { .. })
    ));
    let rule = candidate(parent);
    let mut additional = Vec::new();
    append_exceptions(
        &rule,
        ExceptionalConditions {
            branches: vec![branch],
        },
        &[],
        &mut additional,
        &indices,
        &sector,
    )
    .unwrap();
    assert_eq!(
        additional,
        vec![
            CoordinateCase::new([Some(0), Some(0), Some(-1)])
                .unwrap()
                .into()
        ]
    );
}

#[test]
fn split_guard_retains_all_children_and_every_and_condition() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let rule = candidate(Case::<3>::generic());
    let exceptions = ExceptionalConditions {
        branches: vec![equations(&context, &["a*(a+1)", "b"])],
    };
    let mut additional = Vec::new();
    append_exceptions(
        &rule,
        exceptions.clone(),
        &[],
        &mut additional,
        &[0, 1, 2],
        &[false; 3],
    )
    .unwrap();
    let expected =
        [0, -1].map(|a| Case::from(CoordinateCase::new([Some(a), Some(0), None]).unwrap()));
    assert_eq!(additional.len(), expected.len());
    for child in expected {
        assert!(additional.contains(&child));
    }
    append_exceptions(
        &rule,
        exceptions,
        &[],
        &mut additional,
        &[0, 1, 2],
        &[false; 3],
    )
    .unwrap();
    assert_eq!(
        additional.len(),
        2,
        "already appended complete cases deduplicate"
    );
    let non_exception = CoordinateCase::new([Some(0), Some(-1), Some(-4)])
        .unwrap()
        .into();
    assert!(
        additional
            .iter()
            .all(|case| !case.contains(&non_exception).unwrap())
    );
}

#[test]
fn one_preexisting_child_does_not_suppress_another_or_a_partially_overlapping_face() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let rule = candidate(Case::<3>::generic());
    let exceptions = ExceptionalConditions {
        branches: vec![equations(&context, &["a*(a+1)", "b"])],
    };
    let covered: Case<3> = CoordinateCase::new([Some(0), None, None]).unwrap().into();
    let mut additional = Vec::new();
    append_exceptions(
        &rule,
        exceptions.clone(),
        &[covered],
        &mut additional,
        &[0, 1, 2],
        &[false; 3],
    )
    .unwrap();
    assert_eq!(
        additional,
        vec![
            CoordinateCase::new([Some(-1), Some(0), None])
                .unwrap()
                .into()
        ]
    );
    let partial: Case<3> = CoordinateCase::new([Some(0), Some(0), Some(-1)])
        .unwrap()
        .into();
    additional.clear();
    append_exceptions(
        &rule,
        exceptions,
        &[partial],
        &mut additional,
        &[0, 1, 2],
        &[false; 3],
    )
    .unwrap();
    assert_eq!(
        additional.len(),
        2,
        "a single point cannot cover a free-axis child"
    );
}

#[test]
fn unsupported_factor_sibling_rejects_the_guard_without_retaining_a_partial_union() {
    let context = CoefficientContext::new(["a", "b"]);
    let rule = candidate(Case::<2>::generic());
    let exceptions = ExceptionalConditions {
        branches: vec![equations(&context, &["(a-1)*(a*b-2)"])],
    };
    let mut additional = Vec::new();
    assert!(
        append_exceptions(&rule, exceptions, &[], &mut additional, &[0, 1], &[true; 2]).is_err()
    );
    assert!(
        additional.is_empty(),
        "unsupported factor must not leak an admitted sibling"
    );
}
