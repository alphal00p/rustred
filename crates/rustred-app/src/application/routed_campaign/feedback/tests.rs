use super::*;
use rustred::solver::{
    CandidateOwnerContext, CandidateOwnerInput, CandidateOwnerScope, CoordinateCase,
    ExceptionalConditions, Integral, RuleCandidate, SearchStats, SectorRule, SectorSolution,
    SectorStats, Term,
};
use rustred::{family::IntegralFamily, reduction::ReductionLimits, sector::OrderingPolicy};

const K1: &str = r#"
schema="rustred.project.toml.v1"
[family]
name="owner_feedback_tiny"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;
const K3: &str = r#"
schema="rustred.project.toml.v1"
[family]
name="owner_feedback_sunset"
loop_momenta=["q1","q2"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P1"
expression="q1^2-1"
[[family.denominators]]
id="P2"
expression="q2^2-1"
[[family.denominators]]
id="P3"
expression="(q1+q2)^2-1"
[target]
powers=[1,1,0]
"#;
fn family(source: &str) -> Arc<IntegralFamily> {
    let prepared =
        crate::application::input::prepare_input(source, crate::InputFormat::Toml).unwrap();
    let (_, _, _, lowered) = crate::application::lowering::lower_project(prepared)
        .unwrap()
        .into_parts();
    Arc::new(lowered.into_family())
}
fn key<const N: usize>(powers: [i64; N]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}
fn options() -> RoutedFeedbackOptions {
    RoutedFeedbackOptions::new(
        OwnerFeedbackPolicy {
            numerical_depth: 0,
            ..Default::default()
        },
        FiniteCasePolicy::SearchFinite,
    )
}
// Deliberately incomplete test-only candidate inputs. They are not exported as
// complete artifacts; newly appended formulas come from the real bound solver.
fn session<const N: usize>(
    family: Arc<IntegralFamily>,
    owners: Vec<([bool; N], Vec<SectorRule<N>>)>,
    targets: Vec<IntegralKey>,
) -> RoutedFeedbackSession<N> {
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
    let inputs = owners
        .into_iter()
        .map(|(sector, rules)| CandidateOwnerInput {
            sector,
            saved_root: [true; N],
            ordering: OrderingPolicy::SpiredUncutV1,
            solution: SectorSolution {
                max_numerator_rank: Some(10),
                finite_case_policy: FiniteCasePolicy::SearchFinite,
                rules,
                finite_residuals: vec![],
                stats: SectorStats::default(),
            },
        });
    let programs = Arc::new(CandidateOwnerPrograms::try_new(context, inputs).unwrap());
    RoutedFeedbackSession {
        reducer: RoutedCandidateReducer::try_new(programs, [], Default::default()).unwrap(),
        targets,
        workers: 1,
        options: options(),
        installed: Vec::new(),
        rounds: 0,
    }
}
fn tiny() -> RoutedFeedbackSession<1> {
    session(
        family(K1),
        vec![([true], vec![])],
        vec![key([2]), key([3]), key([2])],
    )
}

#[test]
fn genuine_missing_rule_search_is_parametric_retained_and_not_repeated() {
    let mut run = tiny();
    let old = run.programs().clone();
    let context = old.context().clone();
    let before = RoutedCandidateReducer::try_new(old.clone(), [], Default::default())
        .unwrap()
        .trace_targets([key([3])])
        .unwrap();
    assert_eq!(before.frontier().len(), 1);
    let result = run.run_round(&AtomicBool::new(false), |_| {});
    assert!(result.completed_finite_trace, "{:?}", result.document);
    assert!(result.feedback_round_complete);
    assert_eq!(result.document["source_jobs_started"], 1);
    assert_eq!(result.document["nomination"]["duplicate_entries"], 1);
    assert_eq!(result.document["jobs"][0]["fixed"], json!([null]));
    assert_eq!(result.document["jobs"][0]["status"], "installed");
    assert_eq!(run.installed_jobs(), 1);
    assert!(Arc::ptr_eq(&context, run.programs().context()));
    assert!(!Arc::ptr_eq(&old, run.programs()));
    assert_eq!(old.overlay_usage().batches, 0);
    assert_eq!(run.programs().overlay_usage().batches, 1);
    assert_eq!(result.document["family_closure_claim"], false);
    assert_eq!(result.document["graph_memo_reused"], false);
    // New concrete dots, beyond the nominated observations, use the same real
    // parametric source formula; this is a test, not universal certification.
    assert!(
        run.reducer
            .trace_targets([key([7])])
            .unwrap()
            .frontier()
            .is_empty()
    );
    let second = run.run_round(&AtomicBool::new(false), |_| {});
    assert!(second.completed_finite_trace);
    assert_eq!(second.document["source_jobs_started"], 0);
    assert_eq!(run.installed_jobs(), 1);
}

#[test]
fn missing_owner_and_failed_trace_never_start_source_jobs() {
    let mut owner_gap = session(
        family(K3),
        vec![([true, true, false], vec![])],
        vec![key([2, 0, 2])],
    );
    let gap = owner_gap.run_round(&AtomicBool::new(false), |_| {});
    assert!(!gap.completed_finite_trace);
    assert_eq!(gap.document["source_jobs_started"], 0);
    assert_eq!(
        gap.document["initial_trace"]["snapshot"]["missing_owners"],
        1
    );
    let mut run = tiny();
    let before = run.programs().clone();
    run.reducer = RoutedCandidateReducer::try_new(
        before.clone(),
        [],
        rustred::solver::RoutedCandidateLimits {
            max_unique_nodes: 1,
            ..Default::default()
        },
    )
    .unwrap();
    let result = run.run_round(&AtomicBool::new(false), |_| {});
    assert_eq!(result.document["status"], "initial_trace_incomplete");
    assert_eq!(result.document["source_jobs_started"], 0);
    assert!(Arc::ptr_eq(&before, run.programs()));
}

#[test]
fn actual_above_entry_rank_successor_is_nominated_without_positive_power_narrowing() {
    let family = family(K3);
    let context = rustred::identity::ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    // Synthetic descent edge isolates the trace-to-nomination contract; it
    // makes no claim of IBP provenance for this test-only base formula.
    let rule = SectorRule {
        candidate: RuleCandidate {
            case: CoordinateCase::new([Some(1), Some(3), Some(-10)])
                .unwrap()
                .into(),
            target: Integral::numeric([1, 3, -10]).unwrap(),
            rhs: vec![Term {
                integral: Integral::numeric([1, 1, -11]).unwrap(),
                coefficient: context.one().raw().clone(),
            }],
            sources: vec![],
            stats: SearchStats::default(),
        },
        exceptions: ExceptionalConditions::default(),
    };
    let run = session(
        family,
        vec![([true, true, false], vec![rule])],
        vec![key([1, 3, -10])],
    );
    let report = run.reducer.trace_targets([key([1, 3, -10])]).unwrap();
    assert_eq!(report.max_negative_index_degree(), 11);
    let jobs = nominate(&report, &[], 4, 1024, &AtomicBool::new(false));
    assert_eq!(jobs.jobs.len(), 1);
    assert_eq!(jobs.jobs[0].rank, 11);
    assert_eq!(jobs.jobs[0].fixed, [None, None, Some(-11)]);
    let huge = Ray::from_target([true, true, false], &key([i64::MAX, 1, -11])).unwrap();
    assert_eq!(huge, jobs.jobs[0]);
    assert!(Ray::from_target([true, false], &key([1, -65])).is_err());
    assert!(Ray::from_target([true, false], &key([1, i64::MIN])).is_err());
    assert!(Ray::from_target([true, false], &key([0, -1])).is_err());
    let repeated = nominate(&report, &jobs.jobs, 4, 1024, &AtomicBool::new(false));
    assert!(repeated.jobs.is_empty());
    assert_eq!(repeated.already_installed_entries, 1);
}

#[test]
fn search_and_staging_failures_leave_old_snapshot_active() {
    let mut run = tiny();
    let before = run.programs().clone();
    run.options.attempt_limits.max_symbolic_cases = Some(0);
    let result = run.run_round(&AtomicBool::new(false), |_| {});
    assert!(!result.feedback_round_complete);
    assert_eq!(result.document["jobs"][0]["status"], "search_failed");
    assert!(Arc::ptr_eq(&before, run.programs()));
    assert_eq!(run.installed_jobs(), 0);
    let mut run = tiny();
    let before = run.programs().clone();
    run.options.max_staged_native_bytes = 1;
    let result = run.run_round(&AtomicBool::new(false), |_| {});
    assert_eq!(result.document["jobs"][0]["status"], "raw_staging_limit");
    assert!(Arc::ptr_eq(&before, run.programs()));
    assert_eq!(run.installed_jobs(), 0);
}

#[test]
fn first_source_failure_stops_later_jobs_with_explicit_receipts() {
    let mut run = session(
        family(K3),
        vec![([true, true, false], vec![]), ([true, false, true], vec![])],
        vec![key([2, 2, 0]), key([2, 0, 2])],
    );
    let before = run.programs().clone();
    run.options.attempt_limits.max_symbolic_cases = Some(0);
    let result = run.run_round(&AtomicBool::new(false), |_| {});
    assert!(!result.feedback_round_complete);
    assert_eq!(result.document["source_jobs_started"], 1);
    assert_eq!(result.document["jobs"].as_array().unwrap().len(), 2);
    assert_eq!(result.document["jobs"][0]["status"], "search_failed");
    assert_eq!(
        result.document["jobs"][1]["status"],
        "not_started_after_stop"
    );
    assert!(Arc::ptr_eq(&before, run.programs()));
    assert_eq!(run.installed_jobs(), 0);
}

#[test]
fn cancel_after_native_return_does_not_publish_new_work() {
    let mut run = tiny();
    let before = run.programs().clone();
    let cancellation = AtomicBool::new(false);
    let result = run.run_round(&cancellation, |event| {
        if event["event"] == "local_domain_complete" {
            cancellation.store(true, Ordering::Relaxed);
        }
    });
    assert_eq!(result.document["status"], "cancelled");
    assert_eq!(run.installed_jobs(), 0);
    assert!(Arc::ptr_eq(&before, run.programs()));
    cancellation.store(false, Ordering::Relaxed);
    assert!(run.run_round(&cancellation, |_| {}).completed_finite_trace);
}

#[test]
fn cancel_after_first_publication_preserves_that_epoch_and_all_job_receipts() {
    let mut run = session(
        family(K3),
        vec![([true, true, false], vec![]), ([true, false, true], vec![])],
        vec![key([2, 2, 0]), key([2, 0, 2])],
    );
    let old = run.programs().clone();
    let cancellation = AtomicBool::new(false);
    let result = run.run_round(&cancellation, |event| {
        if event["event"] == "overlay_installed" {
            cancellation.store(true, Ordering::Relaxed);
        }
    });
    assert_eq!(result.document["status"], "cancelled");
    assert_eq!(run.installed_jobs(), 1, "{:?}", result.document);
    assert_eq!(run.programs().overlay_usage().batches, 1);
    assert_eq!(old.overlay_usage().batches, 0);
    assert_eq!(result.document["jobs"].as_array().unwrap().len(), 2);
    assert_eq!(result.document["jobs"][0]["status"], "installed");
    assert_eq!(
        result.document["jobs"][1]["status"],
        "cancelled_before_search"
    );
    let retained = run.programs().clone();
    let stopped = run.run_round(&cancellation, |_| {});
    assert_eq!(stopped.document["source_jobs_started"], 0);
    assert!(Arc::ptr_eq(&retained, run.programs()));
}

#[test]
fn nomination_and_cumulative_ledger_limits_are_explicit_not_new_terminals() {
    let run = tiny();
    let report = run.reducer.trace_targets([key([2])]).unwrap();
    let denied = nominate(&report, &[], 1, 0, &AtomicBool::new(false));
    assert!(denied.jobs.is_empty() && denied.incomplete.is_some());
    let mut run = tiny();
    run.options.max_installed_jobs = 0; // private fault injection, not public admission.
    let result = run.run_round(&AtomicBool::new(false), |_| {});
    assert!(!result.feedback_round_complete && !result.completed_finite_trace);
    assert_eq!(result.document["source_jobs_started"], 0);
    assert_eq!(
        result.document["jobs"][0]["status"],
        "installed_ledger_limit"
    );
    let detail = bounded_debug(&"éééé", 5);
    assert!(detail["detail_truncated"].as_bool().unwrap());
    assert!(detail["detail"].as_str().unwrap().len() <= 5);
}

#[test]
fn feedback_snapshot_errors_and_json_escaping_are_admitted_before_formatting() {
    let snapshot = CandidateRoutedCampaignSnapshot::<1> {
        first_failure: Some(CandidateRoutedCampaignFailure::Trace(
            rustred::solver::CandidateRoutedError::InvalidInput("\n\"\\é".repeat(10000)),
        )),
        ..Default::default()
    };
    let value = feedback_snapshot(&snapshot, 17);
    assert!(value["first_failure"]["detail"].as_str().unwrap().len() <= 17);
    assert_eq!(value["first_failure"]["detail_truncated"], true);
    let mut denied = options();
    denied.max_jobs_per_round = 1024;
    denied.max_error_bytes = 64 * 1024;
    assert!(denied.validate::<1>().is_err()); // escaped payload, not raw bytes.
    assert!(options().validate::<16>().is_ok());
    assert!(receipt_bound(usize::MAX, 1).is_none());
    let detail = bounded_debug(&"\0\n\t\"\\".repeat(100), 31);
    assert!(serde_json::to_vec(&detail).unwrap().len() <= 31 * 6 + 128);
}

#[test]
fn public_prepare_loads_once_and_zero_frontier_round_does_not_generate() {
    use std::sync::atomic::AtomicU64;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "rustred-feedback-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    let mut generation = crate::FamilyCandidatesRequest::new(K1);
    generation.numerical_depth = 0;
    generation.max_numerator_rank = Some(10);
    let saved = crate::family_candidates(generation).unwrap();
    let inspection =
        crate::inspect_generated_candidate_bundle(saved.bundle(), Default::default()).unwrap();
    let file = path.join("owner.rrbin");
    std::fs::write(&file, saved.bundle()).unwrap();
    let selection = json!({"family_fingerprint":inspection.family_fingerprint,
        "owners":[{"path":"owner.rrbin","bytes":saved.bundle().len(),"mask":"1"}],"initial_frontier_routes":[]});
    let mut request = RoutedCampaignRequest::new(selection.to_string(), "2\n3\n".into());
    request.owner_base = path.clone();
    let mut session = RoutedFeedbackSession::<1>::prepare(
        request.clone(),
        options(),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap()
    .unwrap();
    assert!(
        RoutedFeedbackSession::<2>::prepare(request, options(), &AtomicBool::new(false), |_| {})
            .is_err()
    );
    // All following work must use the retained library, not reopen this file.
    std::fs::remove_file(file).unwrap();
    for _ in 0..2 {
        let result = session.run_round(&AtomicBool::new(false), |_| {});
        assert!(result.completed_finite_trace);
        assert_eq!(result.document["source_jobs_started"], 0);
        assert_eq!(session.installed_jobs(), 0);
    }
    std::fs::remove_dir(path).unwrap();
}
