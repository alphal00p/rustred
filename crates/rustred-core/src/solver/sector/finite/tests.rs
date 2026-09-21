use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{
    SearchOptions, SectorConfig, SectorEvent, SectorSolveError, SectorSolveOptions, SectorSolver,
    SourceSystem, Term,
};

fn face<const N: usize>(fixed: [Option<i16>; N]) -> Case<N> {
    CoordinateCase::new(fixed).unwrap().into()
}

#[test]
fn policy_is_explicit_and_canonically_parsed() {
    assert_eq!(FiniteCasePolicy::default(), FiniteCasePolicy::SearchFinite);
    for policy in [
        FiniteCasePolicy::SearchFinite,
        FiniteCasePolicy::RetainRankFinite,
    ] {
        assert_eq!(policy.as_str().parse(), Ok(policy));
    }
    for invalid in ["", "retain", "Search", "retain-rank-finite "] {
        assert!(invalid.parse::<FiniteCasePolicy>().is_err());
    }
}

#[test]
fn coordinate_simplex_is_complete_deterministic_and_deduplicated() {
    let case = face([Some(1), None, None]);
    let mut retained = Retention::default();
    let limits = FiniteCaseLimits {
        max_retained_terminals: 6,
        ..Default::default()
    };
    assert!(
        retained
            .retain_case(&case, &[0, 1, 2], &[true, false, false], 2, limits)
            .unwrap()
    );
    let expected: BTreeSet<_> = (0..=2)
        .flat_map(|left| {
            (0..=2 - left).map(move |right| Integral::numeric([1, -left, -right]).unwrap())
        })
        .collect();
    assert_eq!(retained.points, expected);
    assert_eq!(retained.visited, 6);
    assert!(
        retained
            .retain_case(&case, &[0, 1, 2], &[true, false, false], 2, limits)
            .unwrap()
    );
    assert_eq!(retained.points, expected);
    assert_eq!(retained.visited, 12);
    assert_eq!(
        retained.into_points(),
        expected.into_iter().collect::<Vec<_>>()
    );
}

#[test]
fn full_affine_conjunction_filters_original_integer_coordinates() {
    let context = CoefficientContext::new(["a", "b", "c", "e"]);
    let indices = [0, 1, 2, 3];
    let sector = [true, false, false, false];
    let case = face([Some(1), None, None, None])
        .intersect(
            &["2*b-c", "c-e"].map(|text| context.coefficient_fixture(text).numerator),
            &indices,
            &sector,
        )
        .unwrap()
        .unwrap();
    assert!(case.affine().is_some());
    let mut retained = Retention::default();
    retained
        .retain_case(&case, &indices, &sector, 5, Default::default())
        .unwrap();
    // A face-only test would admit 56 points; full AND admits just these two.
    assert_eq!(
        retained.points,
        BTreeSet::from([
            Integral::numeric([1, 0, 0, 0]).unwrap(),
            Integral::numeric([1, -1, -2, -2]).unwrap(),
        ])
    );
    assert_eq!(retained.visited, 56);
}

#[test]
fn fixed_negative_degree_consumes_the_shared_rank_budget() {
    let case = face([Some(1), Some(-2), None]);
    let mut retained = Retention::default();
    assert!(
        retained
            .retain_case(
                &case,
                &[0, 1, 2],
                &[true, false, false],
                1,
                Default::default()
            )
            .unwrap()
    );
    assert_eq!(retained.len(), 0);
    assert_eq!(retained.visited(), 0);
    retained
        .retain_case(
            &case,
            &[0, 1, 2],
            &[true, false, false],
            2,
            Default::default(),
        )
        .unwrap();
    assert_eq!(
        retained.into_points(),
        [Integral::numeric([1, -2, 0]).unwrap()]
    );
}

#[test]
fn point_and_terminal_budget_errors_commit_no_partial_case() {
    let corner = face([Some(1), Some(0), Some(0)]);
    let case = face([Some(1), None, None]);
    for limits in [
        FiniteCaseLimits {
            max_visited_points: 6,
            ..Default::default()
        },
        FiniteCaseLimits {
            max_retained_terminals: 2,
            ..Default::default()
        },
    ] {
        let mut retained = Retention::default();
        retained
            .retain_case(
                &corner,
                &[0, 1, 2],
                &[true, false, false],
                2,
                Default::default(),
            )
            .unwrap();
        let before = retained.points.clone();
        assert!(
            retained
                .retain_case(&case, &[0, 1, 2], &[true, false, false], 2, limits)
                .is_err()
        );
        assert_eq!(retained.points, before);
        assert_eq!(retained.visited, 1);
    }
}

#[test]
fn positive_rays_remain_unbounded_and_compact_overflow_is_not_truncated() {
    let mut retained = Retention::default();
    assert!(
        !retained
            .retain_case(
                &face([None, None]),
                &[0, 1],
                &[true, false],
                0,
                Default::default()
            )
            .unwrap()
    );
    assert_eq!(retained.visited(), 0);
    assert!(matches!(
        retained.retain_case(
            &face([Some(1), None]),
            &[0, 1],
            &[true, false],
            65,
            Default::default()
        ),
        Err(FiniteRetentionError::CompactOverflow {
            axis: 1,
            remaining_rank: 65
        })
    ));
    assert_eq!(retained.visited(), 0);
    assert_eq!(retained.len(), 0);
    retained
        .retain_case(
            &face([Some(1), Some(-64)]),
            &[0, 1],
            &[true, false],
            65,
            Default::default(),
        )
        .unwrap();
    assert_eq!(retained.len(), 1);
}

#[test]
fn retention_requires_rank_before_observers_or_any_case_search() {
    let family = crate::solver::tests::tadpole();
    let source = SourceSystem::<1>::from_family(&family).unwrap();
    let solver = SectorSolver::new(&source, [true], Default::default()).unwrap();
    let mut events = 0;
    let result = solver.solve_sector_with_observer(
        SectorSolveOptions {
            finite_case_policy: FiniteCasePolicy::RetainRankFinite,
            ..Default::default()
        },
        |_| events += 1,
    );
    assert!(matches!(
        result,
        Err(SectorSolveError::FiniteRetention {
            source: FiniteRetentionError::MissingNumeratorRank,
            ..
        })
    ));
    assert_eq!(events, 0);
    assert!(matches!(
        solver.solve_sector(SectorSolveOptions {
            finite_case_policy: FiniteCasePolicy::RetainRankFinite,
            max_numerator_rank: Some(0),
            max_symbolic_cases: Some(0),
            ..Default::default()
        }),
        Err(SectorSolveError::CaseBudget {
            solved: 0,
            pending: 1
        })
    ));
}

#[test]
fn finite_exception_skips_both_symbolic_and_numerical_minimization() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let source = SourceSystem::new(
        vec![vec![
            Term {
                integral: Integral::symbolic([0; 3]).unwrap(),
                coefficient: context.coefficient_fixture("a-1").numerator,
            },
            Term {
                integral: Integral::symbolic([-1, 0, 0]).unwrap(),
                coefficient: context.one().numerator,
            },
        ]],
        [0, 1, 2],
    )
    .unwrap();
    let solver = SectorSolver::new(&source, [true, false, false], Default::default()).unwrap();
    let mut searches = Vec::new();
    let result = solver
        .solve_sector_with_observer(
            SectorSolveOptions {
                finite_case_policy: FiniteCasePolicy::RetainRankFinite,
                max_numerator_rank: Some(2),
                symbolic: SearchOptions {
                    max_depth: Some(0),
                    ..Default::default()
                },
                // Exhausting this search budget must not prevent finite retention.
                max_symbolic_cases: Some(1),
                ..Default::default()
            },
            |event| match event {
                SectorEvent::Search { case, .. } => searches.push(case.clone()),
                SectorEvent::NumericalStarted { .. } => {
                    panic!("retention must not start numeric search")
                }
                _ => {}
            },
        )
        .unwrap();
    assert_eq!(
        result.finite_case_policy,
        FiniteCasePolicy::RetainRankFinite
    );
    assert_eq!(result.max_numerator_rank, Some(2));
    assert_eq!(result.rules.len(), 1);
    assert_eq!(result.finite_residuals.len(), 6);
    assert_eq!(result.stats.symbolic_cases, 1);
    assert_eq!(result.stats.numerical_cases, 0);
    assert_eq!(result.stats.finite_points_visited, 6);
    assert_eq!(result.stats.retained_finite_terminals, 6);
    assert!(searches.iter().all(|case| case == &Case::generic()));
}

#[test]
fn already_fixed_initial_case_is_kept_without_numeric_search() {
    let context = CoefficientContext::new(["n"]);
    let source = SourceSystem::new(
        vec![vec![Term {
            integral: Integral::symbolic([0]).unwrap(),
            coefficient: context.one().numerator,
        }]],
        [0],
    )
    .unwrap();
    let solver = SectorSolver::new(
        &source,
        [true],
        SectorConfig {
            deltas: [true],
            removed_deltas: [true],
            ..Default::default()
        },
    )
    .unwrap();
    let result = solver
        .solve_sector_with_observer(
            SectorSolveOptions {
                finite_case_policy: FiniteCasePolicy::RetainRankFinite,
                max_numerator_rank: Some(0),
                max_symbolic_cases: Some(0),
                ..Default::default()
            },
            |event| match event {
                SectorEvent::Search { .. } | SectorEvent::NumericalStarted { .. } => {
                    panic!("retained corner must not search")
                }
                _ => {}
            },
        )
        .unwrap();
    assert!(result.rules.is_empty());
    assert_eq!(result.finite_residuals, [Integral::numeric([1]).unwrap()]);
    assert_eq!(result.stats.numerical_cases, 0);
    assert_eq!(result.stats.retained_finite_terminals, 1);
}
