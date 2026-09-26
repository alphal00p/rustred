use super::*;
use crate::{FamilyCandidatesRequest, family_candidates, inspect_generated_candidate_bundle};
use std::sync::atomic::{AtomicU64, Ordering};
mod bounded_routing;
mod guarded_apply;
mod index_admission;
mod owner_batches;
mod power_domains;

#[test]
fn shared_snapshot_exposes_first_failure_while_native_calls_drain() {
    let snapshot = CandidateRoutedCampaignSnapshot::<3> {
        active_nodes: 2,
        failed_nodes: 1,
        first_failure: Some(CandidateRoutedCampaignFailure::Trace(
            rustred::solver::CandidateRoutedError::ResourceLimit {
                resource: "transport endpoints",
                requested: 101,
                limit: 100,
            },
        )),
        first_failure_work: Some(CandidateRoutedWork::Apply {
            owner_sector: [true, false, true],
            target: rustred::family::IntegralKey::try_new([2, -1, 3]).unwrap(),
        }),
        ..Default::default()
    };
    let value = snapshot_json(&snapshot);
    assert_eq!(value["first_failure"]["kind"], "trace");
    let detail = value["first_failure"]["detail"].as_str().unwrap();
    assert!(detail.contains("transport endpoints"));
    assert!(detail.contains("requested: 101"));
    assert!(detail.contains("limit: 100"));
    assert_eq!(value["first_failure_work"]["phase"], "apply");
    assert_eq!(value["first_failure_work"]["owner_mask"], "101");
    assert_eq!(value["first_failure_work"]["target"], json!([2, -1, 3]));
    assert_eq!(value["active_nodes"], 2);
    assert_eq!(value["finished"], false);
    assert_eq!(value["target_completion_known"], false);
    let empty = snapshot_json(&CandidateRoutedCampaignSnapshot::<3>::default());
    assert!(empty["first_failure"].is_null());
    assert!(empty["first_failure_work"].is_null());
}

const K1: &str = r#"
schema="rustred.project.toml.v1"
[family]
name="generic_shared_campaign_test"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;

struct Fixture {
    directory: PathBuf,
    request: RoutedCampaignRequest,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "rustred-routed-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let mut request = FamilyCandidatesRequest::new(K1);
        request.numerical_depth = 0;
        request.max_numerator_rank = Some(2);
        let bundle = family_candidates(request).unwrap();
        let inspection =
            inspect_generated_candidate_bundle(bundle.bundle(), Default::default()).unwrap();
        let path = directory.join("owner.rrbin");
        std::fs::write(&path, bundle.bundle()).unwrap();
        let selection = json!({"family_fingerprint":inspection.family_fingerprint,
            "owners":[{"path":"owner.rrbin","bytes":bundle.bundle().len(),"mask":"1"}],
            "initial_frontier_routes":[]});
        let mut request = RoutedCampaignRequest::new(selection.to_string(), "2\n3\n2\n".into());
        request.owner_base = directory.clone();
        Self { directory, request }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn match_request(fixture: &Fixture) -> OwnerDomainMatchRequest {
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"positive-ray", "owner":"1", "lower":[0], "upper":[null],
            "max_numerator_rank":11}]});
    let mut request =
        OwnerDomainMatchRequest::new(fixture.request.selection_json.clone(), queries.to_string());
    request.owner_base = fixture.directory.clone();
    request
}

#[test]
fn owner_domain_walk_reuses_pending_unbounded_ray_without_closure_claim() {
    let fixture = Fixture::new();
    let before = std::fs::read(fixture.directory.join("owner.rrbin")).unwrap();
    let mut request = OwnerDomainWalkRequest::new(match_request(&fixture));
    // The generic K1 rule's child ray is contained in this already pending
    // domain. Inclusion reuse must work even at the scheduled-domain cap.
    request.max_domains = 1;
    let events = std::cell::RefCell::new(Vec::new());
    let result = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |event| {
        events.borrow_mut().push(event);
    })
    .unwrap();
    assert!(result.all_scheduled_domains_resolved, "{}", result.document);
    assert_eq!(result.document["status"], "locally_resolved");
    assert_eq!(result.document["scheduled_nodes"], 1);
    assert_eq!(result.document["completed_nodes"], 1);
    assert_eq!(result.document["queued_nodes"], 0);
    assert_eq!(result.document["failed_nodes"], 0);
    assert_eq!(result.document["frontiers"], 0);
    assert_eq!(result.document["recursive_worklist_exhausted"], true);
    assert!(result.document["successors"].as_u64().unwrap() > 0);
    assert!(result.document["deduplication_hits"].as_u64().unwrap() > 0);
    assert_eq!(result.document["domains"][0]["lower"], json!([0]));
    assert_eq!(result.document["domains"][0]["upper"], json!([null]));
    assert_eq!(result.document["domains"][0]["rank"], 11);
    for flag in [
        "family_closure_claim",
        "ibp_generation",
        "routing_expanded",
        "independent_certification",
    ] {
        assert_eq!(result.document[flag], false, "{flag}");
    }
    let events = events.into_inner();
    let started = events
        .iter()
        .find(|event| event["event"] == "domain_started")
        .unwrap();
    assert_eq!(started["scheduled_nodes"], 1);
    assert_eq!(started["completed_nodes"], 0);
    let finished = events.last().unwrap();
    assert_eq!(finished["all_scheduled_domains_resolved"], true);
    for counter in [
        "exact_domain_hits",
        "full_orthant_hits",
        "containment_checks",
    ] {
        assert!(result.document[counter].is_u64(), "{counter}");
        assert_eq!(finished[counter], result.document[counter], "{counter}");
        assert!(started[counter].is_u64(), "{counter}");
    }
    for counter in [
        "optional_coefficient_refusals",
        "optional_original_refusals",
        "optional_coalesced_refusals",
    ] {
        assert_eq!(result.document[counter], 0, "{counter}");
        assert_eq!(finished[counter], result.document[counter], "{counter}");
        assert_eq!(
            result.document["domains"][0]["stats"][counter], 0,
            "{counter}"
        );
    }
    assert_eq!(
        result.document["domains"][0]["optional_refusals"],
        json!([])
    );
    assert_eq!(
        result.document["domains"][0]["optional_refusal_provenance_truncated"],
        false
    );
    assert_eq!(
        result.document["domains"][0]["optional_refusal_provenance_scope"],
        "first_per_phase_per_query"
    );
    assert!(finished.get("domains").is_none());
    assert!(serde_json::to_vec(finished).unwrap().len() < 8192);
    assert_eq!(
        std::fs::read(fixture.directory.join("owner.rrbin")).unwrap(),
        before
    );
}

#[test]
fn owner_domain_walk_identical_queries_share_one_schedule() {
    let fixture = Fixture::new();
    let mut matching = match_request(&fixture);
    let mut queries: Value = serde_json::from_str(&matching.queries_json).unwrap();
    let mut duplicate = queries["queries"][0].clone();
    duplicate["id"] = json!("same-ray-second-input");
    queries["queries"].as_array_mut().unwrap().push(duplicate);
    matching.queries_json = queries.to_string();
    let mut request = OwnerDomainWalkRequest::new(matching);
    request.max_domains = 1;
    let result = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(result.all_scheduled_domains_resolved, "{}", result.document);
    assert_eq!(result.document["scheduled_nodes"], 1);
    assert_eq!(result.document["processed_nodes"], 1);
    assert_eq!(result.document["completed_nodes"], 1);
    assert_eq!(result.document["inputs"].as_array().unwrap().len(), 2);
    assert_eq!(result.document["inputs"][0]["domain"], 0);
    assert_eq!(result.document["inputs"][1]["domain"], 0);
    assert_ne!(
        result.document["inputs"][0]["id"],
        result.document["inputs"][1]["id"]
    );
    assert!(result.document["deduplication_hits"].as_u64().unwrap() >= 1);
    assert!(result.document["exact_domain_hits"].as_u64().unwrap() >= 1);
    assert_eq!(result.document["family_closure_claim"], false);
}

#[test]
fn owner_domain_walk_event_cap_and_active_cancellation_remain_incomplete() {
    let fixture = Fixture::new();
    let mut request = OwnerDomainWalkRequest::new(match_request(&fixture));
    request.max_events = 1;
    let limited =
        owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(
        !limited.all_scheduled_domains_resolved,
        "{}",
        limited.document
    );
    assert_eq!(limited.document["status"], "incomplete");
    assert_eq!(limited.document["recursive_worklist_exhausted"], false);
    assert_eq!(limited.document["events"], 1);
    assert_eq!(limited.document["completed_nodes"], 0);
    assert!(
        limited.document["error"]
            .as_str()
            .unwrap()
            .contains("event allowance")
    );
    assert_eq!(limited.document["family_closure_claim"], false);

    let cancellation = AtomicBool::new(false);
    let stopped = owner_domain_walk_with_progress(
        OwnerDomainWalkRequest::new(match_request(&fixture)),
        &cancellation,
        |event| {
            // Cancel after preparation and admission, while the exact native
            // domain operation is pending, not at the input preflight.
            if event["event"] == "domain_started" {
                cancellation.store(true, Ordering::Release);
            }
        },
    )
    .unwrap();
    assert!(cancellation.load(Ordering::Acquire));
    assert!(
        !stopped.all_scheduled_domains_resolved,
        "{}",
        stopped.document
    );
    assert_eq!(stopped.document["status"], "incomplete");
    assert_eq!(stopped.document["scheduled_nodes"], 1);
    assert_eq!(stopped.document["completed_nodes"], 0);
    assert_eq!(stopped.document["recursive_worklist_exhausted"], false);
    assert!(
        stopped.document["error"]
            .as_str()
            .unwrap()
            .to_ascii_lowercase()
            .contains("cancel")
    );
    assert_eq!(stopped.document["family_closure_claim"], false);
}

#[test]
fn owner_domain_match_classifies_unbounded_ray_without_generation_or_rhs_claim() {
    let fixture = Fixture::new();
    let before = std::fs::read(fixture.directory.join("owner.rrbin")).unwrap();
    let request = match_request(&fixture);
    let events = std::cell::RefCell::new(Vec::new());
    let first =
        owner_domain_match_with_progress(request.clone(), &AtomicBool::new(false), |event| {
            events.borrow_mut().push(event);
        })
        .unwrap();
    let repeated =
        owner_domain_match_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(first.classification_complete, "{}", first.document);
    assert!(first.all_queries_locally_applicable, "{}", first.document);
    assert_eq!(first.document["queries"], repeated.document["queries"]);
    assert_eq!(first.document["counts"], repeated.document["counts"]);
    assert_eq!(first.document["completed_queries"], 1);
    for flag in [
        "family_closure_claim",
        "ibp_generation",
        "rhs_successors_expanded",
    ] {
        assert_eq!(first.document[flag], false, "{flag}");
    }
    let pieces = first.document["queries"][0]["pieces"].as_array().unwrap();
    assert!(pieces.iter().all(|p| p["max_numerator_rank"] == 11));
    assert!(pieces.iter().any(|p| p["upper"][0].is_null()));
    assert!(
        pieces
            .iter()
            .any(|p| p["disposition"]["kind"] == "selected_rule")
    );
    assert!(
        pieces
            .iter()
            .any(|p| p["disposition"]["kind"] == "terminal")
    );
    let final_event = events.borrow().last().unwrap().clone();
    assert_eq!(final_event["classification_complete"], true);
    assert!(final_event.get("queries").is_none());
    assert!(serde_json::to_vec(&final_event).unwrap().len() < 8192);
    assert_eq!(
        std::fs::read(fixture.directory.join("owner.rrbin")).unwrap(),
        before
    );
}

#[test]
fn owner_domain_match_opt_in_refinement_roundtrips_native_guards_without_positive_sampling() {
    use rustred::identity::ParametricIbpGenerator;
    use rustred::solver::{
        CoordinateCase, ExceptionalConditions, RuleCandidate, SectorRule, SectorSolution,
    };
    // Synthetic candidate formulas test transport and applicability only, not
    // physical IBP provenance. Native Symbolica owns the coupled guard.
    let fixture = Fixture::new();
    let source = r#"
schema="rustred.project.toml.v1"
[family]
name="native_refinement_app_fixture"
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
expression="(q1-q2)^2-1"
[target]
powers=[1,1,0]
"#;
    let parsed =
        crate::application::input::prepare_input(source, crate::InputFormat::Toml).unwrap();
    let (_, _, _, lowered) = crate::application::lowering::lower_project(parsed)
        .unwrap()
        .into_parts();
    let family = lowered.into_family();
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let guard = context
        .sub(
            &context
                .add(&context.index(0).unwrap(), &context.index(2).unwrap())
                .unwrap(),
            &context.one(),
        )
        .unwrap()
        .raw()
        .numerator
        .clone();
    let rules = [true, false]
        .into_iter()
        .map(|excluded| {
            let case = CoordinateCase::<3>::new([None, Some(1), None]).unwrap();
            SectorRule {
                candidate: RuleCandidate {
                    target: case.integral(),
                    case: case.into(),
                    rhs: vec![],
                    sources: vec![],
                    stats: Default::default(),
                },
                exceptions: ExceptionalConditions {
                    branches: if excluded {
                        vec![vec![guard.clone()]]
                    } else {
                        vec![]
                    },
                },
            }
        })
        .collect();
    let mut generation = FamilyCandidatesRequest::new(source);
    generation.max_numerator_rank = Some(2);
    let solution = SectorSolution::<3> {
        max_numerator_rank: Some(2),
        finite_case_policy: generation.finite_case_policy,
        rules,
        finite_residuals: vec![],
        stats: Default::default(),
    };
    let bytes = crate::encode_generated_candidate_sector(
        &generation,
        &family,
        [true, true, false],
        &solution,
    )
    .unwrap();
    let path = fixture.directory.join("native-refinement.rrbin");
    std::fs::write(&path, &bytes).unwrap();
    let selection = json!({"family_fingerprint":family.fingerprint(),"owners":[{"path":"native-refinement.rrbin","bytes":bytes.len(),"mask":"110"}],"initial_frontier_routes":[]});
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[{"id":"coupled-inactive-ray","owner":"110","lower":[0,0,0],"upper":[null,0,null],"max_numerator_rank":2}]});
    let mut request = OwnerDomainMatchRequest::new(selection.to_string(), queries.to_string());
    request.owner_base = fixture.directory.clone();
    let disabled =
        owner_domain_match_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!disabled.classification_complete, "{}", disabled.document);
    assert_eq!(disabled.document["counts"]["unresolved"], 1);
    request.match_limits.max_bounded_refinement_cells = 2;
    let insufficient =
        owner_domain_match_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!insufficient.classification_complete);
    assert_eq!(
        insufficient.document["queries"][0]["pieces"],
        disabled.document["queries"][0]["pieces"]
    );
    assert_eq!(
        insufficient.document["queries"][0]["stats"]["refinement_cells"],
        0
    );
    request.match_limits.max_bounded_refinement_cells = 3;
    let refined =
        owner_domain_match_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(refined.classification_complete, "{}", refined.document);
    assert!(refined.all_queries_locally_applicable);
    assert_eq!(refined.document["counts"]["unresolved"], 0);
    assert_eq!(
        refined.document["queries"][0]["stats"]["refinement_cells"],
        3
    );
    assert_eq!(
        refined.document["queries"][0]["stats"]["refinement_steps"],
        1
    );
    let pieces = refined.document["queries"][0]["pieces"].as_array().unwrap();
    assert_eq!(pieces.len(), 8);
    assert!(pieces.iter().any(|p| p["upper"][0].is_null()));
    assert!(pieces.iter().all(|p| p["max_numerator_rank"] == 2));
    for flag in [
        "family_closure_claim",
        "ibp_generation",
        "rhs_successors_expanded",
    ] {
        assert_eq!(refined.document[flag], false);
    }
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn owner_domain_match_cancellation_and_piece_budget_remain_incomplete() {
    let fixture = Fixture::new();
    let request = match_request(&fixture);
    let cancelled =
        owner_domain_match_with_progress(request.clone(), &AtomicBool::new(true), |_| {}).unwrap();
    assert!(!cancelled.classification_complete);
    assert!(!cancelled.all_queries_locally_applicable);
    assert_eq!(cancelled.document["family_closure_claim"], false);
    let mut limited = request.clone();
    limited.max_total_pieces = 1;
    let partial =
        owner_domain_match_with_progress(limited, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!partial.classification_complete, "{}", partial.document);
    assert!(!partial.all_queries_locally_applicable);
    assert_eq!(partial.document["retained_pieces"], 1);
    assert_eq!(partial.document["completed_queries"], 0);
    assert_eq!(partial.document["processed_queries"], 1);
    assert_eq!(partial.document["error_kind"], "consumer_limit");
    assert_eq!(partial.document["error_query_id"], "positive-ray");
    assert_eq!(partial.document["queries"][0]["summary_limit"], true);
    assert_eq!(partial.document["queries"][0]["stats"]["pieces"], 2);
    assert!(
        partial.document["queries"][0]["error"]
            .as_str()
            .unwrap()
            .contains("StoppedByConsumer")
    );
    let mut zero_budget = request;
    zero_budget.max_queries = 0;
    assert!(owner_domain_match_with_progress(zero_budget, &AtomicBool::new(true), |_| {}).is_err());
}

#[test]
fn owner_domain_match_rejects_queries_before_loading_owners() {
    let fixture = Fixture::new();
    let mut request = match_request(&fixture);
    request.queries_json = "{}".into();
    let events = std::cell::RefCell::new(Vec::new());
    let error = owner_domain_match_with_progress(request, &AtomicBool::new(false), |event| {
        events.borrow_mut().push(event);
    })
    .unwrap_err();
    assert!(error.to_string().contains("query schema"));
    assert!(events.borrow().is_empty());
}

#[test]
fn owner_domain_match_final_progress_has_no_large_query_payload() {
    let document = json!({"schema":"rustred.owner-domain-match.json.v2", "status":"incomplete",
        "classification_complete":false, "all_queries_locally_applicable":false,
        "queries":[{"pieces":vec![json!({"lower":[0], "upper":[null]});10000]}],
        "counts":{"unresolved":1}, "error":"x".repeat(10000), "family_closure_claim":false});
    let event = OwnerDomainMatchResult::completion_progress(&document);
    assert!(event.get("queries").is_none());
    assert_eq!(event["error_truncated"], true);
    assert_eq!(event["error"].as_str().unwrap().len(), 512);
    assert_eq!(event["classification_complete"], false);
    assert!(serde_json::to_vec(&event).unwrap().len() < 8192);
}

#[test]
fn shared_owner_campaign_is_generic_deduplicated_and_never_claims_family_closure() {
    let fixture = Fixture::new();
    let before = std::fs::read(fixture.directory.join("owner.rrbin")).unwrap();
    let serial =
        routed_campaign_with_progress(fixture.request.clone(), &AtomicBool::new(false), |_| {})
            .unwrap();
    let mut parallel_request = fixture.request.clone();
    parallel_request.workers = 2;
    let parallel =
        routed_campaign_with_progress(parallel_request, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(serial.completed_finite_trace && parallel.completed_finite_trace);
    for field in [
        "reachable_integrals",
        "rule_applications",
        "declared_terminals",
        "missing_owners",
        "missing_rules",
        "scheduled_nodes",
    ] {
        assert_eq!(
            serial.document["snapshot"][field], parallel.document["snapshot"][field],
            "{field}"
        );
    }
    assert!(
        serial.document["snapshot"]["deduplication_hits"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(serial.document["family_closure_claim"], false);
    assert_eq!(serial.document["work_checkpoint"], false);
    assert_eq!(
        std::fs::read(fixture.directory.join("owner.rrbin")).unwrap(),
        before
    );
}

#[test]
fn shared_owner_campaign_cancel_and_tiny_budget_are_incomplete() {
    let fixture = Fixture::new();
    let stopped =
        routed_campaign_with_progress(fixture.request.clone(), &AtomicBool::new(true), |_| {})
            .unwrap();
    assert!(!stopped.completed_finite_trace);
    let mut limited = fixture.request.clone();
    limited.trace_limits.max_unique_nodes = 1;
    let failed = routed_campaign_with_progress(limited, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!failed.completed_finite_trace);
    assert_eq!(failed.document["status"], "incomplete");
    assert!(failed.document["error"].as_str().is_some());
}

#[test]
fn owner_domain_scan_keeps_positive_rays_without_concrete_target_input() {
    let fixture = Fixture::new();
    let before = std::fs::read(fixture.directory.join("owner.rrbin")).unwrap();
    let mut request = OwnerDomainScanRequest::new(fixture.request.selection_json.clone(), Some(10));
    request.owner_base = fixture.directory.clone();
    let events = std::cell::RefCell::new(Vec::new());
    let result =
        owner_domain_scan_with_progress(request.clone(), &AtomicBool::new(false), |event| {
            events.borrow_mut().push(event);
        })
        .unwrap();
    assert!(result.scan_complete);
    assert_eq!(result.document["family_closure_claim"], false);
    assert_eq!(result.document["positive_powers_unbounded"], true);
    assert_eq!(result.document["priority_overapproximation"], true);
    assert_eq!(result.document["guard_satisfiability_decided"], false);
    assert_eq!(result.document["requested_max_numerator_rank"], 10);
    assert_eq!(result.document["saved_entry_rank"], 2);
    assert_eq!(result.document["installed_owners"], 1);
    assert!(result.document["structural_census"]["complete_unbounded_shift_l1_bound"].is_null());
    assert_eq!(result.document["structural_census"]["scan_complete"], true);
    assert_eq!(
        result.document["structural_census"]["observed_callback_regions"],
        result.document["retained_regions"]
    );
    assert!(result.document["retained_regions"].as_u64().unwrap() > 1);
    assert!(
        !result.document["owners"][0]["successor_groups"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let events = events.into_inner();
    let final_event = events.last().unwrap();
    assert_eq!(final_event["event"], "finished");
    assert_eq!(final_event["scan_complete"], true);
    assert_eq!(
        final_event["retained_regions"],
        result.document["retained_regions"]
    );
    assert_eq!(
        final_event["summary_groups"],
        result.document["summary_groups"]
    );
    assert_eq!(final_event["completed_owners"], 1);
    assert!(final_event.get("owners").is_none());
    assert!(
        events
            .iter()
            // Existing preparation events legitimately use a scalar owner count.
            .all(|event| !event["owners"].is_array() && event.get("successor_groups").is_none())
    );
    assert!(serde_json::to_vec(final_event).unwrap().len() < 8192);
    assert_eq!(
        std::fs::read(fixture.directory.join("owner.rrbin")).unwrap(),
        before
    );

    let mut region_limited = request.clone();
    region_limited.max_total_regions = 1;
    let partial =
        owner_domain_scan_with_progress(region_limited, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!partial.scan_complete);
    assert_eq!(partial.document["retained_regions"], 1);
    assert_eq!(
        partial.document["owners"][0]["summary_limit"],
        "total successor regions"
    );
    assert_eq!(partial.document["owners"][0]["regions"], 2);
    assert_eq!(
        partial.document["structural_census"]["scan_complete"],
        false
    );
    assert!(partial.document["structural_census"]["complete_unbounded_shift_l1_bound"].is_null());
    assert_eq!(
        partial.document["structural_census"]["observed_callback_regions"],
        1
    );
    let mut group_limited = request.clone();
    group_limited.max_summary_groups = 1;
    let partial =
        owner_domain_scan_with_progress(group_limited, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!partial.scan_complete);
    assert_eq!(partial.document["summary_groups"], 1);
    assert_eq!(
        partial.document["owners"][0]["summary_limit"],
        "summary groups"
    );
    assert!(partial.document["structural_census"]["complete_unbounded_shift_l1_bound"].is_null());

    request.scan_limits.max_terms = 0;
    let limited =
        owner_domain_scan_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!limited.scan_complete);
    assert_eq!(limited.document["status"], "incomplete");
    assert!(
        limited.document["owners"][0]["error"]
            .as_str()
            .unwrap()
            .contains("terms")
    );
}

#[test]
fn owner_domain_unbounded_census_is_a_shift_bound_not_a_closure_claim() {
    let fixture = Fixture::new();
    let mut request = OwnerDomainScanRequest::new(fixture.request.selection_json.clone(), None);
    request.owner_base = fixture.directory.clone();
    let result =
        owner_domain_scan_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert!(result.scan_complete);
    let census = &result.document["structural_census"];
    assert_eq!(census["scope"], "unbounded_saved_rule_geometry");
    assert!(
        census["complete_unbounded_shift_l1_bound"]
            .as_str()
            .is_some()
    );
    assert_eq!(census["guard_satisfiability_decided"], false);
    assert_eq!(census["first_applicable_priority_resolved"], false);
    assert_eq!(census["source_validity_proved"], false);
    assert_eq!(census["family_closure_claim"], false);
    assert_eq!(result.document["requested_max_numerator_rank"], Value::Null);
    assert_eq!(result.document["owners"][0]["rank_empty_prefilters"], 0);
    for limit in ["regions", "groups", "terms"] {
        let mut limited = request.clone();
        match limit {
            "regions" => limited.max_total_regions = 1,
            "groups" => limited.max_summary_groups = 1,
            _ => limited.scan_limits.max_terms = 0,
        }
        let partial =
            owner_domain_scan_with_progress(limited, &AtomicBool::new(false), |_| {}).unwrap();
        assert!(!partial.scan_complete, "{limit}");
        assert!(
            partial.document["structural_census"]["complete_unbounded_shift_l1_bound"].is_null()
        );
    }
    let stopped = owner_domain_scan_with_progress(request, &AtomicBool::new(true), |_| {}).unwrap();
    assert!(!stopped.scan_complete);
    assert!(stopped.document["structural_census"]["complete_unbounded_shift_l1_bound"].is_null());
}

fn structural_census_fixture() -> Fixture {
    use rustred::identity::ParametricIbpGenerator;
    use rustred::solver::{
        CoordinateCase, ExceptionalConditions, Integral, Power, RuleCandidate, SectorRule,
        SectorSolution, Term,
    };
    // Synthetic native transport only: these formulas do not claim IBP provenance.
    let mut fixture = Fixture::new();
    let source = r#"
schema="rustred.project.toml.v1"
[family]
name="structural_census_fixture"
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
expression="(q1-q2)^2-1"
[target]
powers=[1,1,0]
"#;
    let parsed =
        crate::application::input::prepare_input(source, crate::InputFormat::Toml).unwrap();
    let (_, _, _, lowered) = crate::application::lowering::lower_project(parsed)
        .unwrap()
        .into_parts();
    let family = lowered.into_family();
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let mut generation = FamilyCandidatesRequest::new(source);
    generation.max_numerator_rank = Some(4);
    let mut owners = Vec::new();
    for sector in [[true, false, true], [true, true, false]] {
        let inactive = sector.iter().position(|&active| !active).unwrap();
        let mut rules = Vec::new();
        for restricted in [false, true] {
            let mut fixed = [None; 3];
            if restricted {
                fixed[inactive] = Some(-3);
            }
            let case = CoordinateCase::<3>::new(fixed).unwrap();
            let target = case.integral();
            let terms: &[(i16, i64)] = if restricted {
                &[(-7, 1)]
            } else {
                &[(-1, 1), (-12, 0)]
            };
            let rhs = terms
                .iter()
                .map(|&(shift, coefficient)| Term {
                    integral: Integral::new(std::array::from_fn(|axis| {
                        Power::new(
                            target[axis].is_symbolic(),
                            target[axis].value() + if axis == 0 { shift } else { 0 },
                        )
                        .unwrap()
                    })),
                    coefficient: context.integer(coefficient).raw().clone(),
                })
                .collect();
            rules.push(SectorRule {
                candidate: RuleCandidate {
                    target,
                    case: case.into(),
                    rhs,
                    sources: vec![],
                    stats: Default::default(),
                },
                exceptions: ExceptionalConditions::default(),
            });
        }
        let solution = SectorSolution::<3> {
            max_numerator_rank: Some(4),
            finite_case_policy: generation.finite_case_policy,
            rules,
            finite_residuals: vec![],
            stats: Default::default(),
        };
        let bytes =
            crate::encode_generated_candidate_sector(&generation, &family, sector, &solution)
                .unwrap();
        let mask: String = sector
            .iter()
            .map(|&active| if active { '1' } else { '0' })
            .collect();
        let path = format!("census-{mask}.rrbin");
        std::fs::write(fixture.directory.join(&path), &bytes).unwrap();
        owners.push(json!({"path":path,"bytes":bytes.len(),"mask":mask}));
    }
    fixture.request.selection_json = json!({"family_fingerprint":family.fingerprint(),
        "owners":owners,"initial_frontier_routes":[]})
    .to_string();
    fixture
}

#[test]
fn owner_domain_census_unbounded_scope_restores_rank_prefiltered_larger_shift() {
    let fixture = structural_census_fixture();
    for (rank, maximum, filtered, terms) in [(Some(2), "1", 1, 2), (None, "7", 0, 3)] {
        let mut request = OwnerDomainScanRequest::new(fixture.request.selection_json.clone(), rank);
        request.owner_base = fixture.directory.clone();
        let result =
            owner_domain_scan_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
        assert!(result.scan_complete, "{}", result.document);
        assert_eq!(
            result.document["structural_census"]["observed_max_shift_l1"],
            maximum
        );
        assert_eq!(
            result.document["structural_census"]["complete_unbounded_shift_l1_bound"],
            if rank.is_none() {
                json!("7")
            } else {
                Value::Null
            }
        );
        for owner in result.document["owners"].as_array().unwrap() {
            assert_eq!(owner["rank_empty_prefilters"], filtered);
            assert_eq!(owner["terms"], terms);
            assert_eq!(owner["exact_zero_terms"], 1); // Shift12 is not a nonzero RHS shift.
        }
    }
}

#[test]
fn owner_domain_census_late_owner_failure_withholds_global_bound() {
    let fixture = structural_census_fixture();
    for cancel_after_first in [true, false] {
        let mut request = OwnerDomainScanRequest::new(fixture.request.selection_json.clone(), None);
        request.owner_base = fixture.directory.clone();
        if !cancel_after_first {
            request.max_total_regions = 4; // Two sign cells for each of the first owner's nonzero terms.
        }
        let cancellation = AtomicBool::new(false);
        let result = owner_domain_scan_with_progress(request, &cancellation, |event| {
            if cancel_after_first
                && event["event"] == "owner_scan_finished"
                && event["completed_owners"] == 1
            {
                cancellation.store(true, Ordering::Release);
            }
        })
        .unwrap();
        assert!(!result.scan_complete);
        assert_eq!(
            result.document["structural_census"]["observed_max_shift_l1"],
            "7"
        );
        assert!(
            result.document["structural_census"]["complete_unbounded_shift_l1_bound"].is_null()
        );
        let owners = result.document["owners"].as_array().unwrap();
        assert_eq!(owners.len(), 2);
        assert_eq!(owners[0]["scan_complete"], true);
        assert_eq!(
            owners[0]["structural_census"]["observed_callback_regions"],
            4
        );
        assert_eq!(
            owners[0]["structural_census"]["complete_unbounded_shift_l1_bound"],
            "7"
        );
        assert_eq!(owners[1]["scan_complete"], false);
        assert!(owners[1]["structural_census"]["complete_unbounded_shift_l1_bound"].is_null());
    }
}

#[test]
fn owner_domain_scan_cancel_and_bad_summary_budget_do_not_claim_success() {
    let fixture = Fixture::new();
    let mut request = OwnerDomainScanRequest::new(fixture.request.selection_json.clone(), Some(0));
    request.owner_base = fixture.directory.clone();
    let stopped =
        owner_domain_scan_with_progress(request.clone(), &AtomicBool::new(true), |_| {}).unwrap();
    assert!(!stopped.scan_complete);
    assert_eq!(stopped.document["status"], "cancelled_during_preparation");
    request.max_summary_groups = 0;
    assert!(
        owner_domain_scan_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).is_err()
    );
    request.max_summary_groups = 1_000_001;
    assert!(
        owner_domain_scan_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).is_err()
    );
    for maximum in [100_001, 1_000_000] {
        request.max_summary_groups = maximum;
        let stopped =
            owner_domain_scan_with_progress(request.clone(), &AtomicBool::new(true), |_| {})
                .unwrap();
        assert!(!stopped.scan_complete); // New ceiling admitted, no native load.
    }
    assert_eq!(
        OwnerDomainScanRequest::new(String::new(), Some(10)).max_summary_groups,
        16_384
    );
}

#[test]
fn owner_domain_completion_progress_is_bounded_and_preserves_full_failure_document() {
    let error = "\0\n\"\\😀".repeat(10_000);
    let document = json!({"schema":"rustred.owner-domain-scan.json.v1", "status":"incomplete",
        "scan_complete":false, "family_closure_claim":false, "priority_overapproximation":true,
        "guard_satisfiability_decided":false, "installed_owners":67, "retained_regions":12345,
        "summary_groups":1234, "owners":[
            {"owner":"1", "scan_complete":true, "successor_groups":[{"payload":"x".repeat(1_000_000)}]},
            {"owner":"0", "scan_complete":false, "rules":12,"terms":100,"regions":12346,
             "split_operations":17,"summary_limit":"summary groups", "error":error,
             "successor_groups":[{"payload":"y".repeat(1_000_000)}]}]});
    let before = document.clone();
    let event = OwnerDomainScanResult::completion_progress(&document);
    assert_eq!(event["scan_complete"], false);
    assert_eq!(event["completed_owners"], 1);
    assert_eq!(event["total_owners"], 67);
    assert_eq!(event["retained_regions"], 12345);
    assert_eq!(event["incomplete_owner"], "0");
    assert_eq!(event["summary_limit"], "summary groups");
    assert_eq!(event["regions"], 12346);
    assert_eq!(event["error_truncated"], true);
    assert_eq!(event["error"].as_str().unwrap().chars().count(), 512);
    assert!(event.get("owners").is_none());
    assert!(event.get("successor_groups").is_none());
    assert!(serde_json::to_vec(&event).unwrap().len() < 8192);
    assert_eq!(document, before);
    // The CLI also uses this projection for preparation errors.
    let failure = json!({"status":"preparation_error", "scan_complete":false,
        "error_kind":"input", "error":"bad input"});
    let event = OwnerDomainScanResult::completion_progress(&failure);
    assert_eq!(event["error_kind"], "input");
    assert_eq!(event["error"], "bad input");
    assert_eq!(event["error_truncated"], false);
}

#[test]
fn shared_owner_campaign_admits_all_steering_before_native_load() {
    let fixture = Fixture::new();
    let mut broken = fixture.request.clone();
    // Invalid CSV wins before trying to open the deliberately missing bundle.
    broken.owner_base = fixture.directory.join("missing");
    broken.targets_csv = "trace,1".into();
    let error = routed_campaign_with_progress(broken, &AtomicBool::new(false), |_| {}).unwrap_err();
    assert!(error.to_string().contains("target CSV"));
    let mut selection: Value = serde_json::from_str(&fixture.request.selection_json).unwrap();
    selection["load_limits"] = json!({"max_total_input_bytes":1});
    assert!(
        input::Selection::parse(&selection.to_string())
            .unwrap_err()
            .to_string()
            .contains("ingress")
    );
    selection["load_limits"] = json!({"max_bundle_bytes":2usize*1024*1024*1024});
    assert!(input::Selection::parse(&selection.to_string()).is_err());
    selection["load_limits"] = json!({"max_bundle_bytes":true});
    assert!(input::Selection::parse(&selection.to_string()).is_err());
    selection["load_limits"] = json!({});
    selection["initial_frontier_routes"] = json!([{"source_mask":"1","owner_mask":"1","requires_transport":true,
        "source_to_representative":[["1","0"]],"owner_to_representative":[["1"]]}]);
    assert!(input::Selection::parse(&selection.to_string()).is_err());
}

fn noninvolutive_route_fixture() -> Fixture {
    noninvolutive_route_fixture_with_scale(false)
}

fn noninvolutive_route_fixture_with_scale(scaled: bool) -> Fixture {
    let mut fixture = Fixture::new();
    let source = r#"
schema="rustred.project.toml.v1"
[family]
name="generic_two_loop_route_test"
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
expression="(q1-q2)^2-1"
[target]
powers=[0,1,1]
"#;
    let source = if scaled {
        source
            .replace("q1^2-1", "d*q1^2-1")
            .replace("q2^2-1", "d*q2^2-1")
            .replace("(q1-q2)^2-1", "d*(q1-q2)^2-1")
    } else {
        source.to_owned()
    };
    let mut generation = FamilyCandidatesRequest::new(source);
    generation.nonpositive_indices = vec![0];
    generation.numerical_depth = 0;
    generation.max_numerator_rank = Some(2);
    let bundle = family_candidates(generation).unwrap();
    let inspection =
        inspect_generated_candidate_bundle(bundle.bundle(), Default::default()).unwrap();
    assert_eq!(inspection.solved_sectors, 1);
    std::fs::write(fixture.directory.join("two_loop.rrbin"), bundle.bundle()).unwrap();
    let selection = json!({"family_fingerprint":inspection.family_fingerprint,
        "owners":[{"path":"two_loop.rrbin","bytes":bundle.bundle().len(),"mask":"011"}],
        "initial_frontier_routes":[{"source_mask":"110","owner_mask":"011","requires_transport":true,
            "source_to_representative":[["1","0"],["0","1"]],
            "owner_to_representative":[["1","-1"],["1","0"]]}]});
    let mut request = RoutedCampaignRequest::new(selection.to_string(), "2,2,0\n".into());
    request.owner_base = fixture.directory.clone();
    fixture.request = request;
    fixture
}

#[test]
fn shared_owner_campaign_composes_noninvolutive_native_map_and_rejects_forgery() {
    let fixture = noninvolutive_route_fixture();
    let mut request = fixture.request.clone();
    let mut selection: Value = serde_json::from_str(&request.selection_json).unwrap();
    let result =
        routed_campaign_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert!(result.completed_finite_trace, "{:?}", result.document);
    assert!(
        result.document["snapshot"]["transport_calls"]
            .as_u64()
            .unwrap()
            > 0
    );
    // Per-native-call and aggregate allowances are independent. A genuine
    // nonidentity numerator map must report the narrow native budget first;
    // an explicit sufficient native policy then permits the same exact trace.
    let mut ranked = request.clone();
    ranked.targets_csv = "2,2,-1\n".into();
    ranked
        .trace_limits
        .expansion
        .max_native_polynomial_operations = 1;
    let limited =
        routed_campaign_with_progress(ranked.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!limited.completed_finite_trace);
    assert!(
        limited.document["error"]
            .as_str()
            .unwrap()
            .contains("multi-affine native polynomial operations"),
        "{}",
        limited.document
    );
    ranked.trace_limits.expansion = Default::default();
    let enough =
        routed_campaign_with_progress(ranked.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert!(enough.completed_finite_trace, "{}", enough.document);
    ranked.trace_limits.max_transport_operations = 1;
    let aggregate = routed_campaign_with_progress(ranked, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!aggregate.completed_finite_trace);
    assert!(
        aggregate.document["error"]
            .as_str()
            .unwrap()
            .contains("aggregate routed operations"),
        "{}",
        aggregate.document
    );
    selection["initial_frontier_routes"][0]["owner_to_representative"] =
        json!([["1", "0"], ["0", "0"]]);
    request.selection_json = selection.to_string();
    assert!(
        routed_campaign_with_progress(request.clone(), &AtomicBool::new(false), |_| {})
            .unwrap_err()
            .to_string()
            .contains("inverse")
    );
    selection["initial_frontier_routes"][0]["owner_to_representative"] =
        json!([["1", "0"], ["0", "1"]]);
    request.selection_json = selection.to_string();
    assert!(routed_campaign_with_progress(request, &AtomicBool::new(false), |_| {}).is_err());
}

fn route_walk_request(fixture: &Fixture, rank: u32) -> OwnerDomainWalkRequest {
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"routed-positive-ray", "owner":"110", "lower":[0,0,0],
            "upper":[null,null,null], "max_numerator_rank":rank}]});
    let mut matching =
        OwnerDomainMatchRequest::new(fixture.request.selection_json.clone(), queries.to_string());
    matching.owner_base = fixture.directory.clone();
    let mut walk = OwnerDomainWalkRequest::new(matching);
    walk.route_domain_overcover = true;
    walk
}

#[test]
fn owner_domain_walk_routes_whole_rank_zero_orthant_without_native_expansion() {
    let fixture = noninvolutive_route_fixture();
    let request = route_walk_request(&fixture, 0);
    let result = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(result.all_scheduled_domains_resolved, "{}", result.document);
    assert_eq!(result.document["route_domain_overcover"], true);
    assert_eq!(result.document["routing_expanded"], false);
    assert_eq!(result.document["family_closure_claim"], false);
    assert_eq!(result.document["domains"][0]["phase"], "Route");
    assert_eq!(result.document["domains"][0]["stats"]["apply_domains"], 1);
    assert_eq!(result.document["domains"][0]["stats"]["route_domains"], 0);
    let target = result.document["domains"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["phase"] == "Apply")
        .unwrap();
    assert_eq!(target["owner"], "011");
    assert_eq!(target["upper"], json!([null, null, null]));
    assert_eq!(target["rank"], 0);
}

#[test]
fn owner_domain_walk_route_budget_preserves_above_entry_rank_and_incomplete_prefix() {
    let fixture = noninvolutive_route_fixture();
    let mut request = route_walk_request(&fixture, 11);
    request.max_route_masks = 1;
    let result = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!result.all_scheduled_domains_resolved);
    assert_eq!(result.document["recursive_worklist_exhausted"], false);
    assert_eq!(result.document["completed_nodes"], 0);
    assert_eq!(result.document["queued_nodes"], 1);
    assert_eq!(result.document["route_masks"], 1);
    assert_eq!(result.document["domains"][0]["rank"], 11);
    assert!(
        result.document["error"]
            .as_str()
            .unwrap()
            .contains("route masks")
    );
}

#[test]
fn owner_domain_walk_missing_route_stays_frontier_not_terminal() {
    let fixture = noninvolutive_route_fixture();
    let mut request = route_walk_request(&fixture, 0);
    let mut selection: Value = serde_json::from_str(&request.matching.selection_json).unwrap();
    selection["initial_frontier_routes"] = json!([]);
    request.matching.selection_json = selection.to_string();
    let result = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!result.all_scheduled_domains_resolved);
    assert_eq!(result.document["recursive_worklist_exhausted"], true);
    assert_eq!(result.document["frontiers"], 1);
    assert_eq!(
        result.document["domains"][0]["frontiers"][0]["kind"],
        "missing_route_cover"
    );
    assert_eq!(
        result.document["domains"][0]["frontiers"][0]["reached_missing_rule_claim"],
        false
    );
}

#[test]
fn owner_domain_walk_initial_route_source_conditions_remain_explicit() {
    let fixture = noninvolutive_route_fixture_with_scale(true);
    let mut request = route_walk_request(&fixture, 1);
    // Nonconstant generic map conditions are intentionally unsupported by
    // transport admission. No map is needed here: original source validity
    // must remain an obligation even before missing-route dispatch.
    let mut selection: Value = serde_json::from_str(&request.matching.selection_json).unwrap();
    selection["initial_frontier_routes"] = json!([]);
    request.matching.selection_json = selection.to_string();
    let result = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(
        !result.all_scheduled_domains_resolved,
        "{}",
        result.document
    );
    assert_eq!(result.document["routed_domains"], 0);
    assert_eq!(
        result.document["input_frontiers"][0]["kind"],
        "initial_route_source_validity_obligation"
    );
    assert_eq!(
        result.document["input_frontiers"][0]["reached_missing_rule_claim"],
        false
    );
}

mod parallel_walk;
