//! Public native solver checks of opt-in pre-guard pruning. Independent source
//! rows have distinct leading keys so constructor preconditioning preserves
//! the alternatives; no private solver or mocked admission is used.
use rustred::algebra::CoefficientContext;
use rustred::solver::{
    Integral, PolynomialRow, RulePortfolioTrigger, RuleQualityFeature as F, RuleQualityPriority,
    RuleSelectionPolicy, RuleTrialLimits, RuleTrialOutcome, RuleTrialSummary, SectorConfig,
    SectorEvent, SectorPhase, SectorSolution, SectorSolveOptions, SectorSolver,
    SourceDiscoveryStrategy, SourceSystem, SourceVisitOrder, Term,
};

fn run(
    baseline_rhs: &[[i16; 2]],
    alternative_rhs: &[[i16; 2]],
    priorities: &[F],
    pre_screen: bool,
) -> (SectorSolution<2>, Vec<RuleTrialSummary>, usize) {
    let context = CoefficientContext::try_new(["n", "m"]).unwrap();
    // m<=0 in this sector: m-1 is a genuine stored exception polynomial,
    // whose zero locus is independently discarded by exact sign geometry.
    let pivot = context
        .try_sub(
            &context.parameter("m").unwrap(),
            &context.one(),
            Default::default(),
        )
        .unwrap()
        .numerator;
    let row = |head, rhs: &[[i16; 2]]| -> PolynomialRow<2> {
        std::iter::once(Term {
            integral: Integral::symbolic(head).unwrap(),
            coefficient: pivot.clone(),
        })
        .chain(rhs.iter().map(|&shift| Term {
            integral: Integral::symbolic(shift).unwrap(),
            coefficient: context.one().numerator,
        }))
        .collect()
    };
    let sources = SourceSystem::new(
        vec![row([0, 0], baseline_rhs), row([2, 0], alternative_rhs)],
        [0, 1],
    )
    .unwrap();
    let probe = SectorSolver::new(&sources, [true, false], SectorConfig::default()).unwrap();
    assert_eq!(probe.basis().len(), 2);
    let first = probe
        .basis()
        .iter()
        .position(|r| r[0].integral == Integral::symbolic([0, 0]).unwrap())
        .unwrap();
    let second = 1 - first;
    assert_eq!(
        probe.basis()[second][0].integral,
        Integral::symbolic([2, 0]).unwrap()
    );
    let config = SectorConfig {
        source_discovery: SourceDiscoveryStrategy::Materialized(
            SourceVisitOrder::new(vec![first, second], 2).unwrap(),
        ),
        rule_selection: RuleSelectionPolicy::BoundedPortfolio {
            alternatives: vec![
                SourceDiscoveryStrategy::Materialized(
                    SourceVisitOrder::new(vec![second, first], 2).unwrap(),
                )
                .into(),
            ],
            limits: RuleTrialLimits {
                max_depth: 0,
                max_rows: 32,
                max_exact_trace_rows: 32,
                max_exact_trace_terms: 256,
                max_alternatives: 2,
                max_case_elapsed: None,
            },
            quality: priorities
                .iter()
                .map(|&feature| RuleQualityPriority {
                    feature,
                    descending: false,
                })
                .collect(),
            trigger: RulePortfolioTrigger::Always,
            pre_screen,
        },
        ..Default::default()
    };
    let solver = SectorSolver::new(&sources, [true, false], config).unwrap();
    let mut trials = Vec::new();
    let mut guards = 0;
    let solution = solver
        .solve_sector_with_observer(SectorSolveOptions::default(), |event| match event {
            SectorEvent::RuleTrialFinished { summary, .. } => trials.push(summary),
            SectorEvent::PhaseStarted {
                phase: SectorPhase::GuardExtraction,
                ..
            } => guards += 1,
            _ => {}
        })
        .unwrap();
    assert_eq!(solution.rules.len(), 1);
    assert_eq!(trials.len(), 2);
    (solution, trials, guards)
}

fn same_winner(off: &SectorSolution<2>, on: &SectorSolution<2>) {
    let a = &off.rules[0];
    let b = &on.rules[0];
    assert_eq!(a.candidate.case, b.candidate.case);
    assert_eq!(a.candidate.target, b.candidate.target);
    assert_eq!(a.candidate.rhs, b.candidate.rhs);
    assert_eq!(a.exceptions.branches, b.exceptions.branches);
    assert!(!a.exceptions.branches.is_empty());
}

#[test]
fn worse_prefix_skips_guards_without_an_admission_or_identity() {
    for priorities in [
        vec![F::RhsTerms, F::PositiveMaxNetInactiveDegreeShift],
        vec![F::PositiveMaxNetInactiveDegreeShift, F::RhsTerms],
    ] {
        let (off, off_trials, off_guards) =
            run(&[[-1, 0]], &[[0, -1], [-1, -1]], &priorities, false);
        let (on, trials, guards) = run(&[[-1, 0]], &[[0, -1], [-1, -1]], &priorities, true);
        same_winner(&off, &on);
        assert_eq!((off_guards, guards), (2, 1));
        assert_eq!(off_trials[1].outcome, RuleTrialOutcome::Admitted);
        assert_eq!(trials[1].outcome, RuleTrialOutcome::PreScreened);
        assert_eq!(trials[1].stats.target_matches_baseline, Some(true));
        assert_eq!(
            (
                trials[1].stats.pre_guard_rhs_terms,
                trials[1].stats.pre_guard_numerator_rank_raise
            ),
            (Some(2), Some(1))
        );
        assert!(trials[1].stats.score.is_none());
        assert!(trials[1].stats.duplicate_of_trial.is_none());
        assert_eq!(
            (
                trials[1].stats.guard_branches,
                trials[1].stats.geometry_calls
            ),
            (0, 0)
        );
        assert!(trials[1].stats.search.rows > 0);
        let stats = on.stats.rule_selection.unwrap();
        assert_eq!(
            (stats.attempted, stats.admitted, stats.pre_screened),
            (2, 1, 1)
        );
        assert_eq!(
            (
                stats.completed_admissions,
                stats.canonical_distinct_completed
            ),
            (1, 1)
        );
        assert_eq!(
            stats.total.search.rows,
            trials.iter().map(|t| t.stats.search.rows).sum::<usize>()
        );
        assert_eq!(
            stats.total.exact_lifts,
            trials.iter().map(|t| t.stats.exact_lifts).sum::<usize>()
        );
        let full = off_trials[1].stats.score.unwrap();
        assert_eq!(
            (
                trials[1].stats.pre_guard_rhs_terms,
                trials[1].stats.pre_guard_numerator_rank_raise
            ),
            (Some(full[7]), Some(full[13]))
        );
        assert!(
            off_trials
                .iter()
                .all(|t| t.stats.pre_guard_rhs_terms.is_none())
        );
    }
}

#[test]
fn tied_prefix_still_runs_guards_and_keeps_stable_first_winner() {
    let quality = [F::RhsTerms, F::PositiveMaxNetInactiveDegreeShift];
    let (off, _, _) = run(&[[-1, 0]], &[[-1, 0]], &quality, false);
    let (on, trials, guards) = run(&[[-1, 0]], &[[-1, 0]], &quality, true);
    same_winner(&off, &on);
    assert_eq!(guards, 2);
    assert_eq!(trials[1].outcome, RuleTrialOutcome::Admitted);
    assert_eq!(trials[1].stats.pre_guard_rhs_terms, Some(1));
    assert_eq!(on.stats.rule_selection.unwrap().selected_alternatives, 0);
}

#[test]
fn numerator_rank_priority_is_literal_and_can_dominate_rhs_size() {
    for (quality, winner, pruned) in [
        (
            vec![F::RhsTerms, F::PositiveMaxNetInactiveDegreeShift],
            1,
            true,
        ),
        (
            vec![F::PositiveMaxNetInactiveDegreeShift, F::RhsTerms],
            2,
            false,
        ),
    ] {
        let (off, _, _) = run(&[[-2, -1]], &[[1, 0], [-1, 0]], &quality, false);
        let (on, trials, guards) = run(&[[-2, -1]], &[[1, 0], [-1, 0]], &quality, true);
        same_winner(&off, &on);
        assert_eq!(on.rules[0].candidate.rhs.len(), winner);
        assert_eq!(trials[0].stats.pre_guard_numerator_rank_raise, Some(1));
        assert_eq!(trials[1].stats.pre_guard_numerator_rank_raise, Some(0));
        assert_eq!(trials[1].outcome == RuleTrialOutcome::PreScreened, pruned);
        assert_eq!(guards, if pruned { 1 } else { 2 });
    }
}

#[test]
fn unknown_priority_stops_the_prefix_comparison() {
    // Equal A, unknown guards, worse C: C must not be used speculatively.
    let quality = [
        F::RhsTerms,
        F::GuardBranches,
        F::PositiveMaxNetInactiveDegreeShift,
    ];
    let (off, _, _) = run(&[[-1, 0]], &[[0, -1]], &quality, false);
    let (on, trials, guards) = run(&[[-1, 0]], &[[0, -1]], &quality, true);
    same_winner(&off, &on);
    assert_eq!(guards, 2);
    assert_eq!(trials[1].outcome, RuleTrialOutcome::Admitted);
}
