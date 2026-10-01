//! Public-API regression checks for source-bound, residual-free owner repairs.
//!
//! These are actual one-loop IBPs, not synthetic replacement formulas. They
//! exercise the partial-result replay boundary without compiling the full
//! core unit-test harness or granting whole-sector closure to a partial solve.

use std::sync::Arc;

use rustred::algebra::CoefficientContext;
use rustred::family::{AffineDenominator, IntegralFamily, IntegralKey};
use rustred::foundry::artifact::SourcePortAudit;
use rustred::sector::OrderingPolicy;
use rustred::solver::{
    BoundOwnerOverlay, BoundOwnerSearch, CandidateOwnerContext, CandidateOwnerInput,
    CandidateOwnerPrograms, CandidateOwnerScope, Case, CoordinateCase, FiniteCasePolicy, Integral,
    IntegralOrder, OwnerDomainScope, OwnerFeedbackPolicy, RoutedCandidateReducer, SectorConfig,
    SectorSolution, SectorSolveOptions, SectorSolver, SectorStats, SourceSystem,
};

fn family() -> IntegralFamily {
    let coefficients = CoefficientContext::try_new(["d"]).unwrap();
    IntegralFamily::new(
        "owner-domain-repair-test",
        vec!["k".into()],
        vec![],
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.integer(-1),
            vec![coefficients.one()],
        )],
        vec![],
        vec![coefficients.zero()],
    )
    .unwrap()
}

fn programs(terminals: &[[i16; 1]]) -> Arc<CandidateOwnerPrograms<1>> {
    let context = Arc::new(
        CandidateOwnerContext::try_new(
            Arc::new(family()),
            CandidateOwnerScope {
                max_numerator_rank: Some(10),
                finite_case_policy: FiniteCasePolicy::SearchFinite,
            },
            vec![],
            Default::default(),
        )
        .unwrap(),
    );
    Arc::new(
        CandidateOwnerPrograms::try_new(
            context,
            [CandidateOwnerInput {
                sector: [true],
                saved_root: [true],
                ordering: OrderingPolicy::SpiredUncutV1,
                solution: SectorSolution {
                    order: IntegralOrder::new([true], [false]),
                    max_numerator_rank: Some(10),
                    finite_case_policy: FiniteCasePolicy::SearchFinite,
                    rules: vec![],
                    finite_residuals: terminals
                        .iter()
                        .map(|powers| Integral::numeric(*powers).unwrap())
                        .collect(),
                    stats: SectorStats::default(),
                },
            }],
        )
        .unwrap(),
    )
}

fn search(programs: &Arc<CandidateOwnerPrograms<1>>, depth: u32) -> BoundOwnerSearch<1> {
    programs
        .bind_owner_search(
            [true],
            OwnerFeedbackPolicy {
                numerical_depth: depth,
                ..Default::default()
            },
        )
        .unwrap()
}

fn solve(search: &BoundOwnerSearch<1>, case: Case<1>) -> BoundOwnerOverlay<1> {
    search
        .solve_domains_with_observer(
            vec![case],
            OwnerDomainScope {
                max_numerator_rank: Some(0),
                finite_case_policy: FiniteCasePolicy::SearchFinite,
            },
            Default::default(),
            |_| {},
        )
        .unwrap()
}

fn fixed_two() -> Case<1> {
    CoordinateCase::new([Some(2)]).unwrap().into()
}

#[test]
fn fixed_target_replays_installs_and_reaches_only_the_existing_terminal() {
    let base = programs(&[[1]]);
    let search = search(&base, 1);
    let overlay = solve(&search, fixed_two());
    assert_eq!(overlay.terminal_count(), 0);
    assert_eq!(overlay.rule_count(), 1);
    let replay = search
        .replay_overlay_rules(&overlay, Default::default())
        .unwrap();
    assert_eq!(replay.rules.len(), 1);
    assert!(replay.rules[0].original_source_entries > 0);
    assert_eq!(base.overlays(&[true]).count(), 0);

    let repaired = base
        .append_residual_free_domain_overlays(vec![overlay], Default::default())
        .unwrap();
    assert_eq!(repaired.terminal_count(), base.terminal_count());
    assert_eq!(repaired.overlays(&[true]).count(), 1);
    assert_eq!(base.overlays(&[true]).count(), 0);
    let reducer = RoutedCandidateReducer::try_new(repaired, [], Default::default()).unwrap();
    let trace = reducer
        .trace_targets([IntegralKey::try_new([2]).unwrap()])
        .unwrap();
    assert!(trace.frontier().is_empty());
    assert_eq!(trace.rule_applications(), 1);
    assert_eq!(trace.declared_terminals().len(), 1);
    assert!(
        trace
            .declared_terminals()
            .contains(&IntegralKey::try_new([1]).unwrap())
    );
}

#[test]
fn partial_rule_replay_rejects_mutated_rhs_against_original_sources() {
    let family = family();
    let sources = SourceSystem::<1>::from_family(&family).unwrap();
    let solver = SectorSolver::new(&sources, [true], SectorConfig::default()).unwrap();
    let mut partial = solver
        .solve_domains(
            vec![fixed_two()],
            SectorSolveOptions {
                numerical_depth: 1,
                ..Default::default()
            },
        )
        .unwrap();
    let audit = SourcePortAudit::try_new(&family, Arc::from([])).unwrap();
    assert_eq!(partial.rules.len(), 1);
    assert!(partial.finite_residuals.is_empty());
    assert_eq!(
        audit
            .replay_domain_rules([true], None, &partial)
            .unwrap()
            .rules
            .len(),
        1
    );
    assert!(!partial.rules[0].candidate.rhs.is_empty());
    partial.rules[0].candidate.rhs[0].coefficient =
        -partial.rules[0].candidate.rhs[0].coefficient.clone();
    let error = audit
        .replay_domain_rules([true], None, &partial)
        .unwrap_err();
    assert!(
        error.to_string().contains("original-source replay"),
        "{error}"
    );
}

#[test]
fn a_replayed_generic_partial_with_residuals_cannot_add_terminals() {
    let base = programs(&[]);
    let search = search(&base, 0);
    let overlay = solve(&search, Case::generic());
    assert_eq!(overlay.terminal_count(), 1);
    assert!(overlay.rule_count() > 0);
    let replay = search
        .replay_overlay_rules(&overlay, Default::default())
        .unwrap();
    assert_eq!(replay.rules.len(), overlay.rule_count());
    let error = base
        .append_residual_free_domain_overlays(vec![overlay], Default::default())
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("must not retain finite residual terminals"),
        "{error}"
    );
    assert_eq!(base.overlays(&[true]).count(), 0);
    assert_eq!(base.terminal_count(), 0);
}

#[test]
fn foreign_lineage_is_rejected_by_replay_and_residual_free_append() {
    let base = programs(&[[1]]);
    let foreign = programs(&[[1]]);
    let overlay = solve(&search(&foreign, 1), fixed_two());
    assert_eq!(overlay.terminal_count(), 0);
    let error = search(&base, 1)
        .replay_overlay_rules(&overlay, Default::default())
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("another program lineage or owner order"),
        "{error}"
    );
    let error = base
        .append_residual_free_domain_overlays(vec![overlay], Default::default())
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("another program lineage or owner order"),
        "{error}"
    );
    assert_eq!(base.overlays(&[true]).count(), 0);
    assert_eq!(base.terminal_count(), 1);
}
