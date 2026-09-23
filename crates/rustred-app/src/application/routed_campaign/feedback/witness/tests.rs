use super::super::{RoutedFeedbackFixedResidualPolicy, RoutedFeedbackOptions};
use super::*;
use rustred::family::IntegralFamily;
use rustred::solver::{
    CandidateOwnerContext, CandidateOwnerInput, CandidateOwnerScope, CoordinateCase,
    DomainPowerBounds, ExceptionalConditions, Integral, OwnerFeedbackPolicy, RuleCandidate,
    SearchStats, SectorRule, SectorSolution, SectorStats, Term,
};
use rustred::{reduction::ReductionLimits, sector::OrderingPolicy};
use std::cell::Cell;
use std::ops::ControlFlow;

const K1: &str = r#"
schema="rustred.project.toml.v1"
[family]
name="entry_witness_tadpole"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[2]
"#;

fn family() -> Arc<IntegralFamily> {
    let prepared = crate::application::input::prepare_input(K1, crate::InputFormat::Toml).unwrap();
    let (_, _, _, lowered) = crate::application::lowering::lower_project(prepared)
        .unwrap()
        .into_parts();
    Arc::new(lowered.into_family())
}
fn key(power: i64) -> IntegralKey {
    IntegralKey::try_new([power]).unwrap()
}
fn region(low: u64, high: u64) -> RootRegionInput<1> {
    RootRegionInput {
        support: [true],
        lower: vec![low - 1],
        upper: vec![Some(high - 1)],
        rank: Some(0),
        powers: DomainPowerBounds::default(),
    }
}
fn session_with(
    family: Arc<IntegralFamily>,
    rules: Vec<SectorRule<1>>,
    terminals: &[i16],
    members: &[(u64, u64)],
) -> RoutedFeedbackSession<1> {
    let context = Arc::new(
        CandidateOwnerContext::try_new(
            family,
            CandidateOwnerScope {
                max_numerator_rank: Some(10),
                finite_case_policy: FiniteCasePolicy::SearchFinite,
            },
            vec![],
            ReductionLimits::default(),
        )
        .unwrap(),
    );
    let programs = Arc::new(
        CandidateOwnerPrograms::try_new(
            context,
            [CandidateOwnerInput {
                sector: [true],
                saved_root: [true],
                ordering: OrderingPolicy::SpiredUncutV1,
                solution: SectorSolution {
                    max_numerator_rank: Some(10),
                    finite_case_policy: FiniteCasePolicy::SearchFinite,
                    rules,
                    finite_residuals: terminals
                        .iter()
                        .map(|&n| Integral::numeric([n]).unwrap())
                        .collect(),
                    stats: SectorStats::default(),
                },
            }],
        )
        .unwrap(),
    );
    let mut options = RoutedFeedbackOptions::new(
        OwnerFeedbackPolicy {
            numerical_depth: 1,
            ..Default::default()
        },
        FiniteCasePolicy::SearchFinite,
    );
    options.nomination = RoutedFeedbackNomination::FixedTargets;
    options.fixed_residual_policy =
        RoutedFeedbackFixedResidualPolicy::DeclareSearchedFiniteTerminals;
    let text = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":
        members.iter().enumerate().map(|(id, &(low, high))| json!({"id":format!("member{id}"),
            "owner":"1", "lower":[low-1], "upper":[high-1], "max_numerator_rank":0})).collect::<Vec<_>>()}).to_string();
    RoutedFeedbackSession {
        reducer: rustred::solver::RoutedCandidateReducer::try_new(programs, [], Default::default())
            .unwrap(),
        targets: vec![key(members.last().unwrap().1 as i64)],
        entry_domain: Some(super::super::super::entry::RequestedEntryDomain::parse(&text).unwrap()),
        workers: 1,
        options,
        installed: Vec::new(),
        rounds: 0,
    }
}
fn session(members: &[(u64, u64)]) -> RoutedFeedbackSession<1> {
    session_with(family(), vec![], &[], members)
}
fn proposals(
    run: &RoutedFeedbackSession<1>,
    query: RootRegionInput<1>,
) -> Vec<RoutedEntryWitnessProposal<1>> {
    let programs = run.programs().clone();
    let mut out = Vec::new();
    programs
        .visit_power_bounded_owner_domain_matches(
            query.support,
            &query.lower,
            &query.upper,
            query.rank,
            query.powers,
            Default::default(),
            &AtomicBool::new(false),
            |piece| {
                out.push(RoutedEntryWitnessProposal::from_match(&programs, &piece).unwrap());
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    out
}

#[test]
fn typed_gap_runs_real_fixed_search_overlay_and_retrace_without_region_claim() {
    let mut observations = Vec::new();
    for workers in [1, 2, 6] {
        let mut run = session(&[(2, 4)]);
        run.workers = workers;
        let context = run.programs().context().clone();
        let original = run.programs().clone();
        let inputs = proposals(&run, region(2, 4));
        assert_eq!(inputs.len(), 1);
        assert_eq!(
            inputs[0].disposition(),
            OwnerDomainMatchDisposition::ExactGap
        );
        let first = run
            .run_entry_witness_round(&inputs, Default::default(), &AtomicBool::new(false), |_| {})
            .unwrap();
        assert_eq!(first.selected_roots, [key(2)]);
        assert!(first.more_region_work_required);
        let feedback = first.feedback.unwrap();
        assert!(feedback.feedback_round_complete, "{}", feedback.document);
        assert!(!feedback.completed_finite_trace); // I(1) was not an initial root.
        assert_eq!(feedback.document["source_jobs_started"], 1);
        assert_eq!(feedback.document["nomination"]["cases"], 1);
        assert_eq!(feedback.document["jobs"][0]["fixed_targets"], json!([[2]]));
        assert!(feedback.document["jobs"][0]["rules"].as_u64().unwrap() > 0);
        assert!(Arc::ptr_eq(&context, run.programs().context()));
        assert!(!Arc::ptr_eq(&original, run.programs()));
        let old_batch = run.targets.clone();
        assert!(
            run.run_entry_witness_round(
                &inputs,
                Default::default(),
                &AtomicBool::new(false),
                |_| {}
            )
            .is_err()
        );
        assert_eq!(run.targets, old_batch); // Stale snapshots cannot be reused.

        let current = proposals(&run, region(2, 2));
        let second = run
            .run_entry_witness_round(
                &current,
                Default::default(),
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
        let feedback = second.feedback.unwrap();
        assert!(feedback.completed_finite_trace, "{}", feedback.document);
        assert_eq!(feedback.document["jobs"][0]["fixed_targets"], json!([[1]]));
        assert!(
            run.entry_domain
                .as_ref()
                .unwrap()
                .validate(&key(1))
                .is_err()
        );
        assert!(second.more_region_work_required); // Root sample is not the whole [2,4].
        let trace = run
            .trace("check", &AtomicBool::new(false), &|_| {})
            .unwrap();
        assert_eq!(
            trace.trace().declared_terminals(),
            &std::collections::BTreeSet::from([key(1)])
        );
        observations.push((trace.trace().rule_applications(), run.installed_jobs()));
    }
    assert!(observations.windows(2).all(|p| p[0] == p[1]));
}

#[test]
fn successful_point_never_causes_source_search_or_region_coverage() {
    let mut run = session_with(family(), vec![], &[2], &[(2, 4)]);
    let inputs = proposals(&run, region(2, 2));
    assert!(matches!(
        inputs[0].disposition(),
        OwnerDomainMatchDisposition::Terminal { .. }
    ));
    let original = run.programs().clone();
    let result = run
        .run_entry_witness_round(&inputs, Default::default(), &AtomicBool::new(false), |_| {})
        .unwrap();
    assert_eq!(result.selected_roots, [key(2)]);
    let feedback = result.feedback.unwrap();
    assert!(feedback.completed_finite_trace);
    assert_eq!(feedback.document["source_jobs_started"], 0);
    assert_eq!(run.installed_jobs(), 0);
    assert!(Arc::ptr_eq(&original, run.programs()));
    assert!(result.more_region_work_required);
    // Same program still has an actual gap elsewhere in the input region.
    assert_eq!(
        run.reducer
            .trace_targets([key(3)])
            .unwrap()
            .frontier()
            .len(),
        1
    );
}

#[test]
fn union_hole_or_outside_frontier_does_not_replace_or_trace_previous_batch() {
    let mut run = session(&[(2, 2), (4, 4)]);
    let previous = run.targets.clone();
    let original = run.programs().clone();
    for query in [region(3, 3), region(5, 8)] {
        let inputs = proposals(&run, query);
        let events = Cell::new(0);
        let result = run
            .run_entry_witness_round(
                &inputs,
                Default::default(),
                &AtomicBool::new(false),
                |event| {
                    events.set(events.get() + 1);
                    assert_eq!(event["event"], "entry_witness_selected");
                },
            )
            .unwrap();
        assert!(result.feedback.is_none());
        assert!(result.selected_roots.is_empty());
        assert_eq!(result.statistics.empty_intersections, 2);
        assert!(result.more_region_work_required);
        assert_eq!(run.targets, previous);
        assert_eq!(run.rounds, 0);
        assert_eq!(run.installed_jobs(), 0);
        assert!(Arc::ptr_eq(&original, run.programs()));
        assert_eq!(events.get(), 1);
    }
}

#[test]
fn root_dedup_and_all_selection_allowances_preserve_atomicity() {
    let mut run = session(&[(2, 2), (4, 4)]);
    let mut inputs = proposals(&run, region(2, 4));
    inputs.extend(inputs.clone());
    let previous = run.targets.clone();
    let original = run.programs().clone();
    for limits in [
        RoutedEntryWitnessLimits {
            max_proposals: 1,
            ..Default::default()
        },
        RoutedEntryWitnessLimits {
            max_intersections: 1,
            ..Default::default()
        },
        RoutedEntryWitnessLimits {
            max_projection_calls: 1,
            ..Default::default()
        },
        RoutedEntryWitnessLimits {
            max_roots: 1,
            ..Default::default()
        },
    ] {
        assert!(
            run.run_entry_witness_round(&inputs, limits, &AtomicBool::new(false), |_| {})
                .is_err()
        );
        assert_eq!(run.targets, previous);
        assert_eq!(run.rounds, 0);
        assert!(Arc::ptr_eq(&original, run.programs()));
    }
    let result = run
        .run_entry_witness_round(&inputs, Default::default(), &AtomicBool::new(false), |_| {})
        .unwrap();
    assert_eq!(result.selected_roots, [key(2), key(4)]);
    assert_eq!(result.statistics.intersections, 4);
    assert_eq!(result.statistics.points, 4);
    assert_eq!(result.statistics.duplicate_roots, 2);
}

#[test]
fn cancelled_selection_and_foreign_snapshot_leave_old_state_untouched() {
    let mut run = session(&[(2, 4)]);
    let inputs = proposals(&run, region(2, 2));
    let old = run.targets.clone();
    assert!(
        run.run_entry_witness_round(&inputs, Default::default(), &AtomicBool::new(true), |_| {})
            .is_err()
    );
    assert_eq!(run.targets, old);
    let cancel = AtomicBool::new(false);
    assert!(
        run.run_entry_witness_round(&inputs, Default::default(), &cancel, |_| {
            cancel.store(true, Ordering::Release);
        })
        .is_err()
    );
    assert_eq!(run.targets, old);
    assert_eq!(run.rounds, 0);
    let other = session(&[(2, 4)]);
    let foreign = proposals(&other, region(2, 2));
    // A later bad proposal must not publish the valid root accumulated first.
    let mut mixed = inputs.clone();
    mixed.extend(foreign.clone());
    assert!(
        run.run_entry_witness_round(&mixed, Default::default(), &AtomicBool::new(false), |_| {})
            .is_err()
    );
    assert_eq!(run.targets, old);
    assert_eq!(run.rounds, 0);
    assert!(
        run.run_entry_witness_round(
            &foreign,
            Default::default(),
            &AtomicBool::new(false),
            |_| {}
        )
        .is_err()
    );
    assert_eq!(run.targets, old);
    assert_eq!(run.installed_jobs(), 0);
}

#[test]
fn explicit_policy_and_fixed_search_are_required_not_silently_enabled() {
    let mut run = session(&[(2, 4)]);
    let inputs = proposals(&run, region(2, 2));
    run.options.nomination = RoutedFeedbackNomination::PositiveRays;
    assert!(
        run.run_entry_witness_round(&inputs, Default::default(), &AtomicBool::new(false), |_| {})
            .is_err()
    );
    run.options.nomination = RoutedFeedbackNomination::FixedTargets;
    run.entry_domain = None;
    assert!(
        run.run_entry_witness_round(&inputs, Default::default(), &AtomicBool::new(false), |_| {})
            .is_err()
    );
    assert_eq!(run.rounds, 0);
}

#[test]
fn original_rule_pole_is_checked_before_any_sample_can_appear_solved() {
    let family = family();
    let context = rustred::identity::ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    // Deliberately invalid-at-I(2) synthetic base formula. This isolates native
    // pole handling, not the provenance of the fixture's base rule.
    let denominator = context
        .sub(&context.index(0).unwrap(), &context.integer(2))
        .unwrap();
    let rule = SectorRule {
        candidate: RuleCandidate {
            case: CoordinateCase::new([Some(2)]).unwrap().into(),
            target: Integral::numeric([2]).unwrap(),
            rhs: vec![Term {
                integral: Integral::numeric([1]).unwrap(),
                coefficient: context
                    .div(&context.one(), &denominator)
                    .unwrap()
                    .raw()
                    .clone(),
            }],
            sources: vec![],
            stats: SearchStats::default(),
        },
        exceptions: ExceptionalConditions::default(),
    };
    let mut run = session_with(family, vec![rule], &[1], &[(2, 2)]);
    let inputs = proposals(&run, region(2, 2));
    let result = run
        .run_entry_witness_round(&inputs, Default::default(), &AtomicBool::new(false), |_| {})
        .unwrap();
    let feedback = result.feedback.unwrap();
    assert_eq!(
        feedback.document["initial_trace"]["snapshot"]["missing_rules"],
        1
    );
    assert_eq!(feedback.document["source_jobs_started"], 1);
    assert!(feedback.completed_finite_trace, "{}", feedback.document);
    assert!(result.more_region_work_required);
}
