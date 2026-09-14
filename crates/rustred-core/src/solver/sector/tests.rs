use crate::algebra::CoefficientContext;

use super::*;
use crate::solver::tests::{sunset, tadpole};
use crate::solver::{SearchStats, SectorConfig, SourceSystem, Term};

#[test]
fn tadpole_queue_covers_the_symbolic_ray_and_retains_only_the_fixed_corner() {
    let system = SourceSystem::<1>::from_family(&tadpole()).unwrap();
    let solver = SectorSolver::new(
        &system,
        [true],
        SectorConfig {
            zero_sectors: vec![[false]].into(),
            ..Default::default()
        },
    )
    .unwrap();
    let mut started = Vec::new();
    let result = solver
        .solve_sector_with_observer(SectorSolveOptions::default(), |event| {
            if let SectorEvent::CaseStarted { case, .. } = event {
                started.push(case);
            }
        })
        .unwrap();
    assert_eq!(started, [Case::generic()]);
    assert_eq!(result.rules.len(), 1);
    assert_eq!(result.finite_residuals, [Integral::numeric([1]).unwrap()]);
    assert_eq!(result.stats.symbolic_cases, 1);
    assert_eq!(result.stats.numerical_cases, 1);
}

#[test]
fn stored_generic_rule_does_not_cover_its_exceptional_face() {
    let system = SourceSystem::<1>::from_family(&tadpole()).unwrap();
    let solver = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    let candidate = solver
        .solve_case(CoordinateCase::generic(), SearchOptions::default())
        .unwrap();
    let (rule, _) = solver.finish_rule(candidate).unwrap();
    assert!(
        !solver
            .rule_covers(&rule, &CoordinateCase::new([Some(1)]).unwrap().into())
            .unwrap()
    );
    assert!(
        solver
            .rule_covers(&rule, &CoordinateCase::new([Some(2)]).unwrap().into())
            .unwrap()
    );
}

#[test]
fn pending_queue_subsumption_preserves_broader_and_incomparable_cases() {
    let system = SourceSystem::<3>::from_family(&sunset()).unwrap();
    let solver = SectorSolver::new(&system, [true; 3], SectorConfig::default()).unwrap();
    let narrow = CoordinateCase::new([Some(1), Some(1), None]).unwrap();
    let broad = CoordinateCase::new([None, Some(1), None]).unwrap();
    let other = CoordinateCase::new([Some(1), None, None]).unwrap();
    let mut pending = vec![Case::from(narrow)];
    let mut numerical = Vec::new();
    assert!(
        solver
            .enqueue(broad.into(), &mut pending, &mut numerical, &[])
            .unwrap()
    );
    assert_eq!(pending, [Case::from(broad)]);
    assert!(
        !solver
            .enqueue(narrow.into(), &mut pending, &mut numerical, &[])
            .unwrap()
    );
    assert!(
        solver
            .enqueue(other.into(), &mut pending, &mut numerical, &[])
            .unwrap()
    );
    assert_eq!(pending, [Case::from(other), Case::from(broad)]);
    let corner = CoordinateCase::new([Some(1); 3]).unwrap();
    assert!(
        !solver
            .enqueue(corner.into(), &mut pending, &mut numerical, &[])
            .unwrap()
    );
    pending.clear();
    assert!(
        solver
            .enqueue(corner.into(), &mut pending, &mut numerical, &[])
            .unwrap()
    );
    assert!(
        !solver
            .enqueue(corner.into(), &mut pending, &mut numerical, &[])
            .unwrap()
    );
    assert_eq!(numerical, [corner]);
}

#[test]
fn symbolic_case_budget_cannot_return_a_completed_sector() {
    let system = SourceSystem::<1>::from_family(&tadpole()).unwrap();
    let solver = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    assert!(matches!(
        solver.solve_sector(SectorSolveOptions {
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
fn coupled_exception_enters_the_case_queue_instead_of_being_dropped() {
    let context = CoefficientContext::new(["a", "b"]);
    let row = vec![
        Term {
            integral: Integral::symbolic([0, 0]).unwrap(),
            coefficient: context.coefficient_fixture("a-b").numerator,
        },
        Term {
            integral: Integral::symbolic([-1, 0]).unwrap(),
            coefficient: context.coefficient_fixture("1").numerator,
        },
    ];
    let sources = SourceSystem::new(vec![row], [0, 1]).unwrap();
    let solver = SectorSolver::new(&sources, [true; 2], SectorConfig::default()).unwrap();
    let error = solver
        .solve_sector(SectorSolveOptions {
            max_symbolic_cases: Some(1),
            ..Default::default()
        })
        .unwrap_err();
    assert!(matches!(
        error,
        SectorSolveError::CaseBudget {
            solved: 1,
            pending: 1
        }
    ));
}

fn case<const N: usize>(context: &CoefficientContext, equations: &[&str]) -> Case<N> {
    Case::generic()
        .intersect(
            &equations
                .iter()
                .map(|equation| context.coefficient_fixture(equation).numerator)
                .collect::<Vec<_>>(),
            &std::array::from_fn(|axis| axis),
            &[true; N],
        )
        .unwrap()
        .unwrap()
}

fn trivial<const N: usize>(context: &CoefficientContext) -> SourceSystem<N> {
    SourceSystem::new(
        vec![vec![Term {
            integral: Integral::symbolic([0; N]).unwrap(),
            coefficient: context.one().numerator,
        }]],
        std::array::from_fn(|axis| axis),
    )
    .unwrap()
}

#[test]
fn affine_queue_subsumption_preserves_rays_and_routes_fixed_leaves_to_numeric() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let sources = trivial::<3>(&context);
    let solver = SectorSolver::new(&sources, [true; 3], SectorConfig::default()).unwrap();
    let broad = case::<3>(&context, &["a-b"]);
    let narrow = case::<3>(&context, &["a-b", "c-1"]);
    let incomparable = case::<3>(&context, &["a-c"]);
    let mut pending = vec![narrow.clone()];
    let mut numerical = Vec::new();
    assert!(
        solver
            .enqueue(broad.clone(), &mut pending, &mut numerical, &[])
            .unwrap()
    );
    assert_eq!(pending, [broad.clone()]);
    assert!(
        !solver
            .enqueue(narrow, &mut pending, &mut numerical, &[])
            .unwrap()
    );
    assert!(
        solver
            .enqueue(incomparable.clone(), &mut pending, &mut numerical, &[])
            .unwrap()
    );
    assert_eq!(pending, [broad, incomparable]);
    let corner = CoordinateCase::new([Some(1); 3]).unwrap();
    assert!(
        !solver
            .enqueue(corner.into(), &mut pending, &mut numerical, &[])
            .unwrap()
    );
    pending.clear();
    assert!(
        solver
            .enqueue(corner.into(), &mut pending, &mut numerical, &[])
            .unwrap()
    );
    assert_eq!(numerical, [corner]);
}

#[test]
fn affine_rule_coverage_respects_the_exceptional_fixed_point() {
    let context = CoefficientContext::new(["a", "b"]);
    let sources = trivial::<2>(&context);
    let solver = SectorSolver::new(&sources, [true; 2], SectorConfig::default()).unwrap();
    let domain = case::<2>(&context, &["a-b"]);
    let candidate = RuleCandidate {
        target: domain.integral(),
        case: domain.clone(),
        rhs: vec![Term {
            integral: Integral::symbolic([-1, -1]).unwrap(),
            coefficient: context.coefficient_fixture("1/(b-1)"),
        }],
        sources: Vec::new(),
        stats: SearchStats::default(),
    };
    let (rule, _) = solver.finish_rule(candidate).unwrap();
    let excluded: Case<2> = CoordinateCase::new([Some(1); 2]).unwrap().into();
    let admitted: Case<2> = CoordinateCase::new([Some(2); 2]).unwrap().into();
    assert!(!solver.rule_covers(&rule, &excluded).unwrap());
    assert!(solver.rule_covers(&rule, &admitted).unwrap());
    assert!(!solver.rule_covers(&rule, &domain).unwrap());
    assert_eq!(
        rule.exceptional_cases(&[0, 1], &[true; 2]).unwrap(),
        [excluded]
    );
}

#[test]
fn exceptional_case_union_removes_only_exactly_subsumed_affine_branches() {
    let context = CoefficientContext::new(["a", "b"]);
    let rule = SectorRule {
        candidate: RuleCandidate {
            case: Case::generic(),
            target: Integral::symbolic([0; 2]).unwrap(),
            rhs: Vec::new(),
            sources: Vec::new(),
            stats: SearchStats::default(),
        },
        exceptions: ExceptionalConditions {
            branches: vec![
                vec![
                    context.coefficient_fixture("a-b").numerator,
                    context.coefficient_fixture("b-1").numerator,
                ],
                vec![context.coefficient_fixture("a").numerator],
                vec![context.coefficient_fixture("a-b").numerator],
            ],
        },
    };
    assert_eq!(
        rule.exceptional_cases(&[0, 1], &[true; 2]).unwrap(),
        [case::<2>(&context, &["a-b"])]
    );
}

#[test]
fn unsupported_guard_intersection_cannot_suppress_pending_work() {
    let context = CoefficientContext::new(["a", "b"]);
    let sources = trivial::<2>(&context);
    let solver = SectorSolver::new(&sources, [true; 2], SectorConfig::default()).unwrap();
    let rule = SectorRule {
        candidate: RuleCandidate {
            case: Case::generic(),
            target: Integral::symbolic([0; 2]).unwrap(),
            rhs: Vec::new(),
            sources: Vec::new(),
            stats: SearchStats::default(),
        },
        exceptions: ExceptionalConditions {
            branches: vec![vec![context.coefficient_fixture("a*b-2").numerator]],
        },
    };
    assert!(!solver.rule_covers(&rule, &Case::generic()).unwrap());
    assert!(matches!(
        rule.exceptional_cases(&[0, 1], &[true; 2]),
        Err(AffineGeometryError::UnsupportedNonlinear { .. })
    ));
}
