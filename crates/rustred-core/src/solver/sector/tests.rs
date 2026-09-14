use crate::algebra::CoefficientContext;

use super::*;
use crate::solver::tests::{sunset, tadpole};
use crate::solver::{SectorConfig, SourceSystem, Term};

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
    assert_eq!(started, [CoordinateCase::generic()]);
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
            .rule_covers(&rule, &CoordinateCase::new([Some(1)]).unwrap())
            .unwrap()
    );
    assert!(
        solver
            .rule_covers(&rule, &CoordinateCase::new([Some(2)]).unwrap())
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
    let mut pending = vec![narrow];
    let mut numerical = Vec::new();
    assert!(
        solver
            .enqueue(broad, &mut pending, &mut numerical, &[])
            .unwrap()
    );
    assert_eq!(pending, [broad]);
    assert!(
        !solver
            .enqueue(narrow, &mut pending, &mut numerical, &[])
            .unwrap()
    );
    assert!(
        solver
            .enqueue(other, &mut pending, &mut numerical, &[])
            .unwrap()
    );
    assert_eq!(pending, [other, broad]);
    let corner = CoordinateCase::new([Some(1); 3]).unwrap();
    assert!(
        !solver
            .enqueue(corner, &mut pending, &mut numerical, &[])
            .unwrap()
    );
    pending.clear();
    assert!(
        solver
            .enqueue(corner, &mut pending, &mut numerical, &[])
            .unwrap()
    );
    assert!(
        !solver
            .enqueue(corner, &mut pending, &mut numerical, &[])
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
fn coupled_exception_is_reported_exactly_not_silently_dropped() {
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
        .solve_sector(SectorSolveOptions::default())
        .unwrap_err();
    assert!(matches!(error, SectorSolveError::Geometry {
        source: GeometryError::UnsupportedGeometry { equations }, ..
    } if equations.len() == 1));
}
