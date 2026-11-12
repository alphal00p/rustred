//! Public control for the opt-in actual-rule trial probe; no mocked admission.
use rustred::algebra::CoefficientContext;
use rustred::solver::{
    Case, CaseSeedSchedule, Integral, RuleTrialDescriptor, RuleTrialOutcome, SectorConfig,
    SectorEvent, SectorSolveOptions, SectorSolver, SourceDiscoveryStrategy, SourceSystem, Term,
};
use std::time::Instant;

fn system() -> SourceSystem<2> {
    let c = CoefficientContext::try_new(["n", "m"]).unwrap();
    let pivot = c
        .try_sub(&c.parameter("m").unwrap(), &c.one(), Default::default())
        .unwrap()
        .numerator;
    SourceSystem::new(
        vec![vec![
            Term {
                integral: Integral::symbolic([0, 0]).unwrap(),
                coefficient: pivot,
            },
            Term {
                integral: Integral::symbolic([-1, 0]).unwrap(),
                coefficient: c.one().numerator,
            },
        ]],
        [0, 1],
    )
    .unwrap()
}

#[test]
fn admitted_reference_probe_is_the_unchanged_first_valid_rule() {
    let source = system();
    let solver = SectorSolver::new(&source, [true, false], SectorConfig::default()).unwrap();
    let options = SectorSolveOptions::default();
    let legacy = solver.solve_sector(options).unwrap();
    assert_eq!(legacy.rules.len(), 1);
    let descriptor = RuleTrialDescriptor {
        source_discovery: SourceDiscoveryStrategy::InputOrder,
        seed_schedule: CaseSeedSchedule::Reference,
    };
    let mut summaries = Vec::new();
    let mut publications = 0;
    let (rule, children, discarded) = solver
        .probe_admitted_rule_with_observer(
            &Case::generic(),
            options,
            &descriptor,
            None,
            0,
            None,
            Instant::now(),
            |event| match event {
                SectorEvent::RuleTrialFinished { summary, .. } => summaries.push(summary),
                SectorEvent::RuleFound { .. } => publications += 1,
                _ => {}
            },
        )
        .unwrap();
    let old = &legacy.rules[0];
    assert_eq!(rule.candidate.case, old.candidate.case);
    assert_eq!(rule.candidate.target, old.candidate.target);
    assert_eq!(rule.candidate.rhs, old.candidate.rhs);
    assert_eq!(rule.candidate.sources, old.candidate.sources);
    assert_eq!(rule.exceptions, old.exceptions);
    assert_eq!(rule.dispatch_policy, old.dispatch_policy);
    assert!(children.is_empty());
    assert!(discarded > 0); // The exact m-1=0 exception lies outside m<=0.
    assert_eq!(publications, 0); // A diagnostic probe never mutates/publishes a queue.
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].outcome, RuleTrialOutcome::Admitted);
    assert!(summaries[0].stats.score.is_some());
}

#[test]
fn probe_refuses_a_different_target_instead_of_scoring_it() {
    let source = system();
    let solver = SectorSolver::new(&source, [true, false], SectorConfig::default()).unwrap();
    let descriptor = RuleTrialDescriptor {
        source_discovery: SourceDiscoveryStrategy::InputOrder,
        seed_schedule: CaseSeedSchedule::Reference,
    };
    let wrong = Integral::symbolic([-1, 0]).unwrap();
    let mut outcomes = Vec::new();
    let result = solver.probe_admitted_rule_with_observer(
        &Case::generic(),
        SectorSolveOptions::default(),
        &descriptor,
        None,
        1,
        Some(&wrong),
        Instant::now(),
        |event| {
            if let SectorEvent::RuleTrialFinished { summary, .. } = event {
                outcomes.push(summary.outcome);
            }
        },
    );
    assert!(result.is_err());
    assert_eq!(outcomes, vec![RuleTrialOutcome::DifferentTarget]);
}
