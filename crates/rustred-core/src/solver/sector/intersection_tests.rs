use crate::algebra::{CoefficientContext, CoefficientPolynomial};
use crate::solver::{SearchStats, SectorConfig, SourceSystem, Term};

use super::*;

fn equations(context: &CoefficientContext, expressions: &[&str]) -> Vec<CoefficientPolynomial> {
    expressions
        .iter()
        .map(|expression| context.coefficient_fixture(expression).numerator)
        .collect()
}

fn guarded<const N: usize>(context: &CoefficientContext, branches: &[&[&str]]) -> SectorRule<N> {
    SectorRule {
        candidate: RuleCandidate {
            case: Case::generic(),
            target: Integral::symbolic([0; N]).unwrap(),
            rhs: Vec::new(),
            sources: Vec::new(),
            stats: SearchStats::default(),
        },
        exceptions: ExceptionalConditions {
            branches: branches
                .iter()
                .map(|branch| equations(context, branch))
                .collect(),
        },
    }
}

fn source(context: &CoefficientContext, pivot: &str) -> SourceSystem<3> {
    SourceSystem::new(
        vec![vec![
            Term {
                integral: Integral::symbolic([0; 3]).unwrap(),
                coefficient: context.coefficient_fixture(pivot).numerator,
            },
            Term {
                integral: Integral::symbolic([-1, 0, 0]).unwrap(),
                coefficient: context.one().numerator,
            },
        ]],
        [0, 1, 2],
    )
    .unwrap()
}

#[test]
fn sector_options_transport_intersection_limits_and_fail_atomically() {
    let system = SourceSystem::<1>::from_family(&crate::solver::tests::tadpole()).unwrap();
    let solver = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    let limits = CaseIntersectionLimits {
        max_work_items: 0,
        max_terms_per_conjunction: 101,
        max_normalizations: 17,
        max_factorizations: 19,
    };
    for rank in [None, Some(10)] {
        let mut published = 0;
        let error = solver
            .solve_sector_with_observer(
                SectorSolveOptions {
                    max_numerator_rank: rank,
                    case_intersection_limits: limits,
                    ..Default::default()
                },
                |event| {
                    if matches!(event, SectorEvent::RuleFound { .. }) {
                        published += 1;
                    }
                },
            )
            .unwrap_err();
        let SectorSolveError::Intersection(error) = error else {
            panic!("expected real sector intersection failure: {error:?}");
        };
        assert_eq!(error.max_numerator_rank, rank);
        assert_eq!(error.limits, limits);
        assert_eq!(error.stats.work_items, 0);
        assert_eq!(
            error.failure,
            CaseIntersectionFailure::Budget {
                kind: crate::solver::CaseIntersectionBudget::WorkItems,
                limit: 0,
            }
        );
        assert_eq!(published, 0);
    }
    assert_eq!(
        solver.solve_sector(Default::default()).unwrap().rules.len(),
        1
    );
}

#[test]
fn coverage_uses_selected_intersection_limits_without_suppressing_unproved_work() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let sources = source(&context, "1");
    let solver = SectorSolver::new(&sources, [true; 3], SectorConfig::default()).unwrap();
    let rule = guarded::<3>(&context, &[&["a"]]);
    let target = Case::generic();
    assert!(
        solver
            .rule_covers(&rule, &target, Default::default())
            .unwrap()
    );
    let limits = CaseIntersectionLimits {
        max_work_items: 0,
        ..Default::default()
    };
    assert!(!solver.rule_covers(&rule, &target, limits).unwrap());
    let mut pending = Vec::new();
    let mut numerical = Vec::new();
    assert!(
        solver
            .enqueue(
                target.clone(),
                &mut pending,
                &mut numerical,
                &[rule],
                limits
            )
            .unwrap()
    );
    assert_eq!(pending, [target]);
    assert!(numerical.is_empty());
}

#[test]
fn the_queue_retains_both_factors_exposed_by_a_coupled_affine_sibling() {
    let context = CoefficientContext::new(["a", "b", "c", "d"]);
    // This homogeneous identity has a generic parameter d. Its exceptional
    // AND is a=b and a*b-3*a*c+2*c^2=0; restricting the latter gives
    // (b-c)*(b-2*c)=0, hence two positive-dimensional positive-sector cases.
    let sources = source(&context, "d*(a*b-3*a*c+2*c^2)+(a-b)");
    let original = sources.rows().to_vec();
    let solver = SectorSolver::new(&sources, [true; 3], SectorConfig::default()).unwrap();
    let mut published_pending = Vec::new();
    let error = solver
        .solve_sector_with_observer(
            SectorSolveOptions {
                max_symbolic_cases: Some(1),
                ..Default::default()
            },
            |event| {
                if let SectorEvent::RuleFound { pending, .. } = event {
                    published_pending.push(pending);
                }
            },
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SectorSolveError::CaseBudget {
            solved: 1,
            pending: 2
        }
    ));
    assert_eq!(published_pending, [2]);
    assert_eq!(sources.rows(), original);
}

#[test]
fn an_unresolved_factor_prevents_parent_publication_after_exact_discovery() {
    let context = CoefficientContext::new(["a", "b", "c", "d"]);
    let sources = source(&context, "d*(a-1)*(a^2+b^2-5)+(c-1)");
    let original = sources.rows().to_vec();
    let solver = SectorSolver::new(&sources, [true; 3], SectorConfig::default()).unwrap();
    let mut published = 0;
    let error = solver
        .solve_sector_with_observer(
            SectorSolveOptions {
                max_symbolic_cases: Some(1),
                ..Default::default()
            },
            |event| {
                if let SectorEvent::RuleFound { .. } = event {
                    published += 1;
                }
            },
        )
        .unwrap_err();
    match error {
        SectorSolveError::Intersection(source) => {
            assert_eq!(source.failure, CaseIntersectionFailure::UnsupportedGeometry);
            assert_eq!(source.original_parent, Case::generic());
            assert_eq!(source.original_conjunction.len(), 2);
        }
        other => panic!("expected atomic geometry failure, got {other:?}"),
    }
    assert_eq!(published, 0);
    assert_eq!(sources.rows(), original);
    assert_eq!(sources.rows()[0].len(), 2);
}

#[test]
fn all_original_or_branches_are_admitted_before_returning_any_children() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let rule = guarded::<3>(&context, &[&["a-1"], &["(b-1)*(b^2+c^2-5)"]]);
    let error = rule
        .admit_exceptional_cases(&[0, 1, 2], &[true; 3])
        .unwrap_err();
    assert!(matches!(
        error,
        SectorSolveError::Intersection(source)
            if source.failure == CaseIntersectionFailure::UnsupportedGeometry
    ));
    assert!(rule.exceptional_cases(&[0, 1, 2], &[true; 3]).is_err());
}

#[test]
fn coordinate_branch_chronology_matches_the_original_queue_for_every_permutation() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let sources = source(&context, "1");
    let solver = SectorSolver::new(&sources, [true; 3], SectorConfig::default()).unwrap();
    // Numeric-first versus broad-first intentionally need not leave identical
    // numerical seed banks. The new atomic admission must preserve whichever
    // chronology the original exceptional OR supplied, not silently prune it.
    let branches: [&[&str]; 4] = [
        &["a-1", "b-1", "c-1"],
        &["a-2", "b-1", "c-1"],
        &["b-1"],
        &["a-3"],
    ];
    let mut numerical_counts = Vec::new();
    for first in 0..4 {
        for second in 0..4 {
            for third in 0..4 {
                for fourth in 0..4 {
                    let order = [first, second, third, fourth];
                    if (0..4).any(|slot| order[..slot].contains(&order[slot])) {
                        continue;
                    }
                    let mut input = order.map(|slot| branches[slot]).to_vec();
                    input.push(&["a"]); // Empty positive-sector branch.
                    input.push(branches[first]); // Duplicate branch.
                    let rule = guarded::<3>(&context, &input);
                    let mut old_pending = Vec::new();
                    let mut old_numerical = Vec::new();
                    let mut old_discarded = 0;
                    for branch in &rule.exceptions.branches {
                        if let Some(child) = rule
                            .candidate
                            .case
                            .intersect(branch, &[0, 1, 2], &[true; 3])
                            .unwrap()
                        {
                            if !solver
                                .enqueue(
                                    child,
                                    &mut old_pending,
                                    &mut old_numerical,
                                    &[],
                                    Default::default(),
                                )
                                .unwrap()
                            {
                                old_discarded += 1;
                            }
                        } else {
                            old_discarded += 1;
                        }
                    }
                    numerical_counts.push(old_numerical.len());
                    for _repeat in 0..2 {
                        let (children, mut discarded) = rule
                            .admit_exceptional_cases(&[0, 1, 2], &[true; 3])
                            .unwrap();
                        let mut pending = Vec::new();
                        let mut numerical = Vec::new();
                        for child in children {
                            if !solver
                                .enqueue(
                                    child,
                                    &mut pending,
                                    &mut numerical,
                                    &[],
                                    Default::default(),
                                )
                                .unwrap()
                            {
                                discarded += 1;
                            }
                        }
                        assert_eq!(pending, old_pending, "permutation {order:?}");
                        assert_eq!(numerical, old_numerical, "permutation {order:?}");
                        assert_eq!(discarded, old_discarded, "permutation {order:?}");
                    }
                }
            }
        }
    }
    assert_eq!(numerical_counts.len(), 24);
    assert!(numerical_counts.contains(&0));
    assert!(numerical_counts.contains(&2));
}

#[test]
fn disjunctive_rule_coverage_proves_empty_not_merely_one_empty_factor() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let sources = source(&context, "1");
    let solver = SectorSolver::new(&sources, [true; 3], SectorConfig::default()).unwrap();
    let rule = guarded::<3>(&context, &[&["(a-1)*(b-2)"]]);
    let excluded_a: Case<3> = CoordinateCase::new([Some(1), Some(4), None])
        .unwrap()
        .into();
    let excluded_b: Case<3> = CoordinateCase::new([Some(3), Some(2), None])
        .unwrap()
        .into();
    let admitted: Case<3> = CoordinateCase::new([Some(3), Some(4), None])
        .unwrap()
        .into();
    assert!(
        !solver
            .rule_covers(&rule, &excluded_a, Default::default())
            .unwrap()
    );
    assert!(
        !solver
            .rule_covers(&rule, &excluded_b, Default::default())
            .unwrap()
    );
    assert!(
        solver
            .rule_covers(&rule, &admitted, Default::default())
            .unwrap()
    );
    assert!(
        !solver
            .rule_covers(&rule, &Case::generic(), Default::default())
            .unwrap()
    );
    let unsupported = guarded::<3>(&context, &[&["(a-1)*(a^2+b^2-5)"]]);
    assert!(
        !solver
            .rule_covers(&unsupported, &Case::generic(), Default::default())
            .unwrap()
    );
}

#[test]
fn root_free_exceptional_factors_preserve_other_or_branches_and_rule_coverage() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let sources = source(&context, "1");
    let solver = SectorSolver::new(&sources, [true; 3], SectorConfig::default()).unwrap();
    let empty = guarded::<3>(&context, &[&["a^2-3*a+4"], &["b^2-8*b+3"]]);
    assert!(
        empty
            .exceptional_cases(&[0, 1, 2], &[true; 3])
            .unwrap()
            .is_empty()
    );
    assert!(
        solver
            .rule_covers(&empty, &Case::generic(), Default::default())
            .unwrap()
    );

    let mixed = guarded::<3>(&context, &[&["a^2-3*a+4"], &["(a-1)*(b^2-2)"]]);
    let retained: Case<3> = CoordinateCase::new([Some(1), None, None]).unwrap().into();
    assert_eq!(
        mixed.exceptional_cases(&[0, 1, 2], &[true; 3]).unwrap(),
        [retained]
    );
    assert!(
        !solver
            .rule_covers(&mixed, &Case::generic(), Default::default())
            .unwrap()
    );
}

#[test]
fn distinct_finite_factor_leaves_enter_only_the_numerical_queue() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let sources = source(&context, "1");
    let solver = SectorSolver::new(&sources, [true; 3], SectorConfig::default()).unwrap();
    let rule = guarded::<3>(&context, &[&["(a-1)*(a-2)", "b-1", "c-1"]]);
    let children = rule.exceptional_cases(&[0, 1, 2], &[true; 3]).unwrap();
    assert_eq!(children.len(), 2);
    let mut pending = Vec::new();
    let mut numerical = Vec::new();
    for child in children {
        assert!(child.is_numerical());
        assert!(
            solver
                .enqueue(child, &mut pending, &mut numerical, &[], Default::default())
                .unwrap()
        );
    }
    assert!(pending.is_empty());
    let mut expected = vec![
        CoordinateCase::new([Some(1); 3]).unwrap(),
        CoordinateCase::new([Some(2), Some(1), Some(1)]).unwrap(),
    ];
    expected.sort_by(compare_cases);
    assert_eq!(numerical, expected);
}
