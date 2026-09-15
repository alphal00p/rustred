use super::*;
use crate::solver::tests::tadpole;
use crate::solver::{CoordinateCase, SectorEvent, SectorPhase, SectorSolveOptions};

#[test]
fn direct_observation_preserves_identity_without_constructing_gplu() {
    let system = SourceSystem::<1>::from_family(&tadpole()).unwrap();
    let solver = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    let baseline = solver
        .solve_case(CoordinateCase::generic(), SearchOptions::default())
        .unwrap();
    let mut events = Vec::new();
    let observed = solver
        .solve_case_with_observer(
            CoordinateCase::generic(),
            SearchOptions::default(),
            |event| {
                events.push(event);
            },
        )
        .unwrap();
    assert_eq!(baseline.target, observed.target);
    assert_eq!(baseline.rhs, observed.rhs);
    assert_eq!(baseline.sources, observed.sources);
    assert!(observed.stats.discovery.is_none());
    assert!(matches!(
        events.as_slice(),
        [
            SearchEvent::DiscoveryProgress {
                depth: 0,
                seeds: 1,
                rows: 0,
                discovery: None
            },
            SearchEvent::CanonicalizationStarted {
                direct_hit: true,
                ..
            },
        ]
    ));
}

#[test]
fn modular_observation_reports_the_winning_trace_before_exact_work() {
    let context = crate::algebra::CoefficientContext::new(["n"]);
    let term = |shift| Term {
        integral: Integral::symbolic([shift]).unwrap(),
        coefficient: context.one().numerator,
    };
    // Preserve two unpreconditioned rows deliberately: at n=1 neither has
    // target 1 as its leading column, but their difference eliminates I(2).
    let basis = vec![vec![term(1), term(0)], vec![term(1), term(-1)]];
    let system = SourceSystem::new(basis.clone(), [0]).unwrap();
    let solver = SectorSolver {
        system: &system,
        basis,
        order: IntegralOrder::new([true], [false]),
        config: SectorConfig::default(),
    };
    let case = CoordinateCase::new([Some(1)]).unwrap();
    let options = SearchOptions {
        max_depth: Some(0),
        ..Default::default()
    };
    let baseline = solver.solve_case(case, options).unwrap();
    let mut events = Vec::new();
    let observed = solver
        .solve_case_with_observer(case, options, |event| events.push(event))
        .unwrap();
    assert!(!observed.stats.direct_hit);
    assert_eq!(observed.stats.exact_trace_rows, 2);
    assert_eq!(observed.target, baseline.target);
    assert_eq!(observed.rhs, baseline.rhs);
    assert_eq!(observed.sources, baseline.sources);
    assert_eq!(observed.stats.discovery, baseline.stats.discovery);
    assert!(matches!(events.as_slice(), [
        SearchEvent::DiscoveryProgress { discovery: None, .. },
        SearchEvent::ExactStarted { pivot, trace_rows: 2, discovery },
        SearchEvent::CanonicalizationStarted { direct_hit: false, .. },
    ] if *pivot == observed.target && discovery.independent_rows == 2));
}

#[test]
fn bounded_miss_emits_progress_but_never_an_exact_hit() {
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
    let mut events = Vec::new();
    let outcome = solver.solve_case_with_observer(
        CoordinateCase::new([Some(1)]).unwrap(),
        SearchOptions {
            max_depth: Some(2),
            ..Default::default()
        },
        |event| events.push(event),
    );
    assert!(matches!(outcome, Err(SolverError::SearchExhausted { .. })));
    assert!(!events.is_empty());
    assert!(
        events
            .iter()
            .all(|event| matches!(event, SearchEvent::DiscoveryProgress { .. }))
    );
}

#[test]
fn sector_observation_forwards_case_and_preserves_phase_order() {
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
    let baseline = solver.solve_sector(SectorSolveOptions::default()).unwrap();
    let mut phases = Vec::new();
    let observed = solver
        .solve_sector_with_observer(SectorSolveOptions::default(), |event| match event {
            SectorEvent::CaseStarted { case, .. } => {
                assert_eq!(case, Case::generic());
                phases.push("case");
            }
            SectorEvent::Search { case, event } => {
                assert_eq!(case, &Case::generic());
                phases.push(match event {
                    SearchEvent::DiscoveryProgress { .. } => "discovery",
                    SearchEvent::CanonicalizationStarted { .. } => "canonicalization",
                    SearchEvent::ExactStarted { .. } => panic!("unexpected direct-hit GPLU"),
                });
            }
            SectorEvent::PhaseStarted { case, phase } => {
                assert_eq!(case, &Case::generic());
                phases.push(match phase {
                    SectorPhase::GuardExtraction => "guards",
                    SectorPhase::ExceptionalGeometry => "geometry",
                });
            }
            SectorEvent::RuleFound { .. } => phases.push("rule"),
            SectorEvent::NumericalStarted { .. } => phases.push("numerical"),
        })
        .unwrap();
    assert_eq!(
        phases,
        [
            "case",
            "discovery",
            "canonicalization",
            "guards",
            "geometry",
            "rule",
            "numerical"
        ]
    );
    assert_eq!(baseline.finite_residuals, observed.finite_residuals);
    assert_eq!(baseline.rules.len(), observed.rules.len());
    assert_eq!(
        baseline.rules[0].candidate.rhs,
        observed.rules[0].candidate.rhs
    );
    assert_eq!(baseline.rules[0].exceptions, observed.rules[0].exceptions);
}
