use super::*;
use crate::application::routed_campaign::walking::{
    OwnerDomainWalkFiniteReplayLimits,
    epoch::{
        inspector::{Context, inspect_job},
        job::Job,
    },
    finite_replay::{
        Account, OWNER_DOMAIN_WALK_FINITE_REPLAY_VERSION, Outcome as FiniteOutcome, Recipe, run,
    },
    initial_overlap::InitialOverlapIndex,
};
use std::sync::atomic::AtomicBool;

fn limits() -> OwnerDomainWalkFiniteReplayLimits {
    OwnerDomainWalkFiniteReplayLimits {
        max_nodes: 1000,
        max_rule_applications: 1000,
        max_transport_calls: 1000,
        max_transport_operations: 10000,
        max_transport_endpoints: 10000,
        max_coalescing_additions: 10000,
        max_positive_layers: 64,
        max_seed_points: 1024,
        max_seed_bytes: 1024 * 1024,
    }
}
fn fixture() -> Fixture {
    let mut fixture = controller_tests::native_equivalence::closed_fixture();
    fixture.request.finite_replay = Some(limits());
    fixture
}

#[test]
fn finite_replay_private_runtime_resume_is_refused_without_changing_cold_read_authority() {
    let mut f = Fixture::new();
    f.request.checkpoint = Some(crate::OwnerDomainWalkCheckpointOptions::new(&f.directory.0));
    f.request.finite_replay = Some(limits());
    f.save(3, 0);
    let latest = f.directory.0.join(publication::LATEST);
    let before = fs::read(&latest).unwrap();
    let error = match f.open() {
        Ok(_) => panic!("finite replay resumed"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("finite replay runtime resume"));
    assert_eq!(fs::read(&latest).unwrap(), before);
    // The shared read/decode path is still usable; only runtime adoption is
    // gated. Actual typed-summary cold replay has its own end-to-end fixture.
    assert!(
        decoded(
            &f.directory.0,
            publication::LATEST,
            &f.identity(),
            &f.reducer,
            16
        )
        .is_ok()
    );
}
fn job(f: &Fixture) -> Job<1> {
    let q = &f.queries[0];
    Job {
        seq: 1,
        parent: 0,
        v0: 0,
        attempts: 0,
        flags: 0,
        image: crate::application::routed_campaign::walking::queue::CompactDomain::try_from_domain(
            &Domain {
                phase: Phase::Apply,
                owner: [true],
                lower: q.lower.clone(),
                upper: q.upper.clone(),
                rank: q.rank,
                powers: q.powers,
            },
        )
        .unwrap(),
    }
}
fn inspect(f: &Fixture, j: &Job<1>, account: &Account, cancel: &AtomicBool) -> JobResult<1> {
    let overlap = InitialOverlapIndex::empty();
    JobResult::decode(&inspect_job(
        &Context {
            reducer: &f.reducer,
            request: &f.request,
            overlap: &overlap,
            cancellation: cancel,
            g2: None,
            finite_account: Some(account),
        },
        &j.encode(),
    ))
    .unwrap()
}

#[test]
fn finite_replay_real_native_success_is_typed_and_attempt_is_unique() {
    let f = fixture();
    let j = job(&f);
    let account = Account::default();
    let result = inspect(&f, &j, &account, &AtomicBool::new(false));
    assert_eq!(result.kind, NativeKind::FiniteReplay);
    assert_eq!(result.finite_replay.unwrap().limits, limits());
    assert_eq!(
        (result.emitted, result.accepted, result.stats_events),
        (1, 1, 1)
    );
    assert!(result.misses.is_empty() && result.frontiers.is_empty() && result.error.is_none());
    assert_eq!(JobResult::<1>::decode(&result.encode()).unwrap(), result);
    assert!(
        account.report()["work"]["rule_applications"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(account.report()["attempts"], 1);
    // Even a repeated same first-attempt job cannot reset finite work.
    assert_ne!(
        inspect(&f, &j, &account, &AtomicBool::new(false)).kind,
        NativeKind::FiniteReplay
    );
    assert_eq!(account.report()["attempts"], 1);
}

#[test]
fn finite_replay_budget_declines_but_cancel_never_falls_through() {
    let mut f = fixture();
    let j = job(&f);
    f.request.finite_replay.as_mut().unwrap().max_nodes = 0;
    let account = Account::default();
    let result = inspect(&f, &j, &account, &AtomicBool::new(false));
    assert_eq!(result.kind, NativeKind::Apply);
    assert!(result.error.is_none());
    assert_eq!(account.report()["work"]["status"], "aggregate_budget");
    let account = Account::default();
    let result = inspect(&f, &j, &account, &AtomicBool::new(true));
    assert_eq!(result.error_kind, ErrorKind::Cancelled);
    assert!(result.error.is_some());
    assert_eq!(account.report()["attempts"], 1);
}

#[test]
fn finite_replay_wrong_id_caps_partial_context_and_noninitial_query_cannot_substitute() {
    let f = fixture();
    let mut j = job(&f);
    j.parent = 1;
    let account = Account::default();
    assert_ne!(
        inspect(&f, &j, &account, &AtomicBool::new(false)).kind,
        NativeKind::FiniteReplay
    );
    assert_eq!(account.report()["attempts"], 0);
    let mut j = job(&f);
    let mut domain = j.image.expand();
    domain.powers.max_positive_power = Some(1);
    j.image = crate::application::routed_campaign::walking::queue::CompactDomain::try_from_domain(
        &domain,
    )
    .unwrap();
    let account = Account::default();
    assert_ne!(
        inspect(&f, &j, &account, &AtomicBool::new(false)).kind,
        NativeKind::FiniteReplay
    );
    assert_eq!(
        account.report()["work"]["status"],
        "not_original_whole_initial"
    );
    let overlap = InitialOverlapIndex::empty();
    let cancel = AtomicBool::new(false);
    let account = Account::default();
    let g2 = crate::application::routed_campaign::walking::g2::Store::<1>::new(None, 0, 0);
    let j = job(&f);
    let result = JobResult::<1>::decode(&inspect_job(
        &Context {
            reducer: &f.reducer,
            request: &f.request,
            overlap: &overlap,
            cancellation: &cancel,
            g2: Some(&g2),
            finite_account: Some(&account),
        },
        &j.encode(),
    ))
    .unwrap();
    assert_ne!(result.kind, NativeKind::FiniteReplay);
    assert_eq!(account.report()["attempts"], 0);
    let mut wide = job(&f).image.expand();
    wide.lower = vec![0];
    wide.upper = vec![Some(4)];
    let mut anchor = wide.clone();
    anchor.lower = vec![2];
    let overlap = InitialOverlapIndex::from_initial(&[Arc::new(anchor)], &cancel);
    assert!(overlap.plan(&wide, &cancel).is_some());
    let mut partial_job = job(&f);
    partial_job.image =
        crate::application::routed_campaign::walking::queue::CompactDomain::try_from_domain(&wide)
            .unwrap();
    let mut partial_request = f.request.clone();
    partial_request.reuse_initial_d_bands = true;
    let account = Account::default();
    let result = JobResult::<1>::decode(&inspect_job(
        &Context {
            reducer: &f.reducer,
            request: &partial_request,
            overlap: &overlap,
            cancellation: &cancel,
            g2: None,
            finite_account: Some(&account),
        },
        &partial_job.encode(),
    ))
    .unwrap();
    assert_ne!(result.kind, NativeKind::FiniteReplay);
    assert_eq!(account.report()["attempts"], 0);
    let mut f = Fixture::with_source_condition(true);
    f.request.finite_replay = Some(limits());
    f.request.route_domain_overcover = true;
    let j = job(&f);
    let mut queries: Value = serde_json::from_str(&f.request.matching.queries_json).unwrap();
    queries["queries"][0]["owner"] = "0".into();
    f.request.matching.queries_json = queries.to_string();
    let account = Account::default();
    assert_ne!(
        inspect(&f, &j, &account, &cancel).kind,
        NativeKind::FiniteReplay
    );
    assert_eq!(
        account.report()["work"]["status"],
        "not_original_whole_initial"
    );
}

#[test]
fn finite_replay_unsupported_envelope_and_native_frontier_are_not_closed() {
    let f = fixture();
    let mut domain = job(&f).image.expand();
    let recipe = Recipe {
        version: OWNER_DOMAIN_WALK_FINITE_REPLAY_VERSION,
        limits: limits(),
    };
    domain.upper[0] = None;
    assert!(matches!(
        run(&f.reducer, &domain, recipe, &AtomicBool::new(false), None),
        FiniteOutcome::Declined { .. }
    ));
    domain = job(&f).image.expand();
    let context = f.reducer.programs().context().clone();
    let scope = context.scope();
    let empty = rustred::solver::CandidateOwnerPrograms::try_new(
        context,
        [rustred::solver::CandidateOwnerInput {
            sector: [true],
            saved_root: [true],
            ordering: rustred::sector::OrderingPolicy::SpiredUncutV1,
            solution: rustred::solver::SectorSolution {
                order: rustred::solver::IntegralOrder::new([true], [false]),
                max_numerator_rank: scope.max_numerator_rank,
                finite_case_policy: scope.finite_case_policy,
                rules: vec![],
                finite_residuals: vec![rustred::solver::Integral::numeric([1]).unwrap()],
                stats: Default::default(),
            },
        }],
    )
    .unwrap();
    let reducer = RoutedCandidateReducer::try_new(Arc::new(empty), [], Default::default()).unwrap();
    // The positive root has neither an applicable rule nor terminal authority.
    let outcome = run(&reducer, &domain, recipe, &AtomicBool::new(false), None);
    match outcome {
        FiniteOutcome::Declined { work } => assert_eq!(work["status"], "frontier"),
        other => panic!("unexpected absent-owner outcome: {}", other.work()),
    }
}

#[test]
fn finite_replay_hard_native_descent_error_is_not_symbolic_fallback() {
    // Synthetic public-API formula for error propagation only, not a proved IBP.
    // Admission authenticates its native representation; concrete application
    // must reject the non-descending successor rather than decline this policy.
    use rustred::solver::{
        CandidateOwnerInput, CandidateOwnerPrograms, CoordinateCase, ExceptionalConditions,
        Integral, IntegralOrder, RuleCandidate, SearchStats, SectorRule, SectorSolution,
        SectorStats, Term,
    };
    let mut f = fixture();
    let context = f.reducer.programs().context().clone();
    let rule = SectorRule {
        dispatch_policy: Default::default(),
        candidate: RuleCandidate {
            case: CoordinateCase::new([Some(3)]).unwrap().into(),
            target: Integral::numeric([3]).unwrap(),
            rhs: vec![Term {
                integral: Integral::numeric([4]).unwrap(),
                coefficient: context.coefficient_context().integer(1).raw().clone(),
            }],
            sources: Vec::new(),
            stats: SearchStats::default(),
        },
        exceptions: ExceptionalConditions::default(),
    };
    let programs = CandidateOwnerPrograms::try_new(
        context,
        [CandidateOwnerInput {
            sector: [true],
            saved_root: [true],
            ordering: rustred::sector::OrderingPolicy::SpiredUncutV1,
            solution: SectorSolution {
                order: IntegralOrder::new([true], [false]),
                max_numerator_rank: Some(2),
                finite_case_policy: Default::default(),
                rules: vec![rule],
                finite_residuals: vec![Integral::numeric([1]).unwrap()],
                stats: SectorStats::default(),
            },
        }],
    )
    .unwrap();
    f.reducer =
        RoutedCandidateReducer::try_new(Arc::new(programs), [], Default::default()).unwrap();
    let account = Account::default();
    let result = inspect(&f, &job(&f), &account, &AtomicBool::new(false));
    assert_eq!(result.error_kind, ErrorKind::NativeFailure);
    assert!(result.error.as_deref().unwrap().contains("NonDescending"));
    assert_eq!(result.emitted, 0);
    assert!(result.misses.is_empty());
    assert_eq!(account.report()["work"]["status"], "native_error");
    assert_eq!(account.report()["work"]["rule_attempts"], 1);
}

#[test]
fn finite_replay_empty_original_cap_declines_without_summarizing_excluded_point() {
    let mut f = fixture();
    // Physical root is n=3. Its unchanged coordinate box intersects A<=2 in
    // the empty set; the optional trace must not strip that cap.
    f.queries[0].powers.max_positive_power = Some(2);
    let mut q: Value = serde_json::from_str(&f.request.matching.queries_json).unwrap();
    q["queries"][0]["power_bounds"] = json!({"max_positive_power":2});
    f.request.matching.queries_json = q.to_string();
    let account = Account::default();
    let j = job(&f);
    let enabled = inspect(&f, &j, &account, &AtomicBool::new(false));
    assert_eq!(account.report()["work"]["status"], "empty_capped_domain");
    assert_ne!(enabled.kind, NativeKind::FiniteReplay);
    assert!(enabled.finite_replay.is_none());
    f.request.finite_replay = None;
    let legacy = inspect(&f, &j, &Account::default(), &AtomicBool::new(false));
    assert_eq!(
        (
            enabled.kind,
            enabled.error_kind,
            enabled.emitted,
            enabled.accepted,
            enabled.misses
        ),
        (
            legacy.kind,
            legacy.error_kind,
            legacy.emitted,
            legacy.accepted,
            legacy.misses
        )
    );
}
