use super::super::e2e_tests::{Scratch, scratch};
use super::*;
use crate::{
    FamilyCandidatesRequest, OwnerDomainMatchRequest, OwnerDomainWalkCheckpointOptions,
    OwnerDomainWalkFiniteReplayLimits, OwnerDomainWalkPublicationPolicy, family_candidates,
    owner_domain_walk_with_progress,
};
use std::fs;

const SOURCE: &str = r#"
schema="rustred.project.toml.v1"
[family]
name="finite_replay_tadpole"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;
fn fixture(label: &str) -> (Scratch, OwnerDomainWalkRequest) {
    let dir = scratch(label);
    let mut generation = FamilyCandidatesRequest::new(SOURCE);
    generation.max_numerator_rank = Some(0);
    let generated = family_candidates(generation).unwrap();
    let shards =
        crate::application::candidate_bundle::split_generated_candidate_bundle(generated.bundle())
            .unwrap();
    let mut owners = Vec::new();
    let mut fingerprint = String::new();
    for (mask, bytes, family) in shards {
        fingerprint = family;
        let path = format!("owner-{mask}.rrbin");
        fs::write(dir.0.join(&path), &bytes).unwrap();
        owners.push(json!({"mask":mask,"path":path,"bytes":bytes.len()}));
    }
    let selection =
        json!({"family_fingerprint":fingerprint,"owners":owners,"initial_frontier_routes":[]});
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[{
        "id":"root","owner":"1","lower":[2],"upper":[2],"max_numerator_rank":0,
        "power_bounds":{"max_positive_power":3,"min_power_difference":3,"max_power_difference":3}
    }]});
    let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
        selection.to_string(),
        queries.to_string(),
    ));
    request.matching.owner_base = dir.0.clone();
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Epoch;
    request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(
        dir.0.join("checkpoint"),
    ));
    request.finite_replay = Some(OwnerDomainWalkFiniteReplayLimits {
        max_nodes: 1000,
        max_rule_applications: 1000,
        max_transport_calls: 1000,
        max_transport_operations: 10000,
        max_transport_endpoints: 10000,
        max_coalescing_additions: 10000,
    });
    request.reuse_initial_d_bands = true;
    request.g2_residual_anchors = crate::OwnerDomainWalkG2ResidualAnchors::Union;
    request.scheduling_policy = crate::OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
        lookahead: 1.try_into().unwrap(),
    };
    (dir, request)
}
fn walk(request: &OwnerDomainWalkRequest) -> Value {
    owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {})
        .unwrap()
        .document
}
fn options(request: &OwnerDomainWalkRequest) -> OwnerDomainWalkVerifyOptions {
    let mut options =
        OwnerDomainWalkVerifyOptions::new(&request.checkpoint.as_ref().unwrap().directory);
    options.require_closure = true;
    options
}
fn cold(request: &OwnerDomainWalkRequest) -> Value {
    owner_domain_walk_verify_closure(request, &options(request), &AtomicBool::new(false), |_| {})
        .unwrap()
}
fn rejected(request: &OwnerDomainWalkRequest) {
    if let Ok(report) = owner_domain_walk_verify_closure(
        request,
        &options(request),
        &AtomicBool::new(false),
        |_| {},
    ) {
        assert_eq!(report["verdict"], "FAIL", "{report}");
    }
}

#[test]
fn finite_replay_cp6_real_success_cold_reexecutes_and_rejects_changed_policy_caps_payload() {
    let (dir, request) = fixture("finite-replay-cold");
    let report = walk(&request);
    assert_eq!(report["finite_replay"]["attempts"], 1);
    assert_eq!(report["finite_replay"]["work"]["status"], "closed");
    assert_eq!(report["scheduled_nodes"], 1);
    assert_eq!(report["resume_supported"], false);
    let verified = cold(&request);
    assert_eq!(verified["verdict"], "PASS", "{verified}");
    let mut errors = Violations::new(50);
    let loaded = load::<1>(&options(&request), false, &mut errors).unwrap();
    assert!(errors.is_empty());
    assert!(loaded.finite_replay.is_some());
    assert!(loaded.raw.edges.is_empty());
    let mut partial = options(&request);
    partial.reinspect = OwnerDomainWalkVerifyReinspect::None;
    assert!(
        owner_domain_walk_verify_closure(&request, &partial, &AtomicBool::new(false), |_| {})
            .is_err()
    );
    let mut changed = request.clone();
    changed.finite_replay.as_mut().unwrap().max_nodes = 0;
    rejected(&changed);
    let mut changed = request.clone();
    let mut q: Value = serde_json::from_str(&changed.matching.queries_json).unwrap();
    q["queries"][0]["power_bounds"]["max_positive_power"] = 4.into();
    changed.matching.queries_json = q.to_string();
    rejected(&changed);
    let selection: Value = serde_json::from_str(&request.matching.selection_json).unwrap();
    let path = dir.0.join(selection["owners"][0]["path"].as_str().unwrap());
    let mut bytes = fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    fs::write(path, &bytes).unwrap();
    rejected(&request);
}

#[test]
fn finite_replay_zero_budget_fallback_matches_default_off_symbolic_graph() {
    let (_offdir, mut off) = fixture("finite-replay-off");
    off.finite_replay = None;
    let off_report = walk(&off);
    assert!(off_report.get("finite_replay").is_none());
    let (_ondir, mut on) = fixture("finite-replay-zero-budget");
    on.finite_replay.as_mut().unwrap().max_nodes = 0;
    let on_report = walk(&on);
    assert_eq!(on_report["finite_replay"]["attempts"], 1);
    assert_eq!(
        on_report["finite_replay"]["work"]["status"],
        "aggregate_budget"
    );
    let mut errors = Violations::new(50);
    let a = load::<1>(&options(&off), false, &mut errors).unwrap();
    let b = load::<1>(&options(&on), false, &mut errors).unwrap();
    assert!(errors.is_empty());
    assert!(a.finite_replay.is_none() && b.finite_replay.is_none());
    assert_eq!(a.domains, b.domains);
    assert_eq!(a.raw.edges, b.raw.edges);
    assert_eq!(a.raw.flags, b.raw.flags);
    assert_eq!(cold(&off)["verdict"], "PASS");
    assert_eq!(cold(&on)["verdict"], "PASS");
}

#[test]
fn finite_replay_cold_requires_actual_trace_not_success_counters() {
    let (_dir, request) = fixture("finite-replay-cold-refusal");
    walk(&request);
    let mut violations = Violations::new(50);
    let loaded = load::<1>(&options(&request), false, &mut violations).unwrap();
    assert!(violations.is_empty());
    let original_recipe = loaded.finite_replay.unwrap();
    let graph = Graph::from_edges(loaded.domains.len(), &loaded.raw.edges).unwrap();
    let containment = Containment::new(0, 0);
    let cancel = AtomicBool::new(false);
    let (selection, _, limits) = input::Selection::parse(&request.matching.selection_json).unwrap();
    let mut load_request = RoutedCampaignRequest::new(String::new(), String::new());
    load_request.owner_base = request.matching.owner_base.clone();
    load_request.reduction_limits = request.matching.reduction_limits;
    finite::configure_load(&request, &mut load_request);
    let reducer = prepare::prepare::<1>(&load_request, &selection, limits, &cancel, &|_| {})
        .unwrap()
        .unwrap();
    let ctx = Ctx {
        request: &request,
        reducer: &reducer,
        loaded: &loaded,
        graph: &graph,
        containment: &containment,
        cancellation: &cancel,
        failing: None,
        count_parity: true,
    };
    let mut tally = Tally::default();
    reinspect(&ctx, original_recipe, &mut tally, &mut violations);
    assert_eq!(tally.errors, 0);
    assert!(violations.is_empty());

    // Test the cold replay branch after its binding gate independently of the
    // global checkpoint digest tests above. Consistent local request/recipe
    // changes do not make the recorded successful counters a replay proof.
    let mut capped = request.clone();
    capped.finite_replay.as_mut().unwrap().max_nodes = 0;
    let recipe = Recipe {
        limits: capped.finite_replay.unwrap(),
        ..original_recipe
    };
    let capped_ctx = Ctx {
        request: &capped,
        ..ctx
    };
    let mut tally = Tally::default();
    let mut errors = Violations::new(50);
    reinspect(&capped_ctx, recipe, &mut tally, &mut errors);
    assert_eq!(tally.errors, 1);
    assert!(!errors.is_empty());
    assert_eq!(
        tally.finite_replay_work.unwrap()["status"],
        "aggregate_budget"
    );

    let context = reducer.programs().context().clone();
    let scope = context.scope();
    let programs = rustred::solver::CandidateOwnerPrograms::try_new(
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
    let missing =
        RoutedCandidateReducer::try_new(std::sync::Arc::new(programs), [], Default::default())
            .unwrap();
    let frontier_ctx = Ctx {
        reducer: &missing,
        ..ctx
    };
    let mut tally = Tally::default();
    let mut errors = Violations::new(50);
    reinspect(&frontier_ctx, original_recipe, &mut tally, &mut errors);
    assert_eq!(tally.errors, 1);
    assert!(!errors.is_empty());
    assert_eq!(tally.finite_replay_work.unwrap()["status"], "frontier");
}
