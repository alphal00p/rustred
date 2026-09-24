use super::*;

fn multiple_domains(fixture: &Fixture) -> OwnerDomainWalkRequest {
    let mut matching = match_request(fixture);
    matching.queries_json = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"narrow-a","owner":"1","lower":[2],"upper":[2],"max_numerator_rank":11},
        {"id":"narrow-b","owner":"1","lower":[3],"upper":[3],"max_numerator_rank":11},
        {"id":"whole-ray","owner":"1","lower":[0],"upper":[null],"max_numerator_rank":11},
        {"id":"duplicate","owner":"1","lower":[2],"upper":[2],"max_numerator_rank":11},
        {"id":"unbounded-rank","owner":"1","lower":[0],"upper":[null],"max_numerator_rank":null}
    ]})
    .to_string();
    OwnerDomainWalkRequest::new(matching)
}
fn without_seconds(mut value: Value) -> Value {
    fn visit(v: &mut Value) {
        match v {
            Value::Object(o) => {
                o.remove("seconds");
                for v in o.values_mut() {
                    visit(v);
                }
            }
            Value::Array(a) => {
                for v in a {
                    visit(v);
                }
            }
            _ => {}
        }
    }
    visit(&mut value);
    value
}

#[test]
fn owner_domain_walk_parallel_stable_domains_and_native_counters_match_serial() {
    let fixture = Fixture::new();
    let request = multiple_domains(&fixture);
    let baseline =
        owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert!(
        baseline.all_scheduled_domains_resolved,
        "{}",
        baseline.document
    );
    // This owner has no inactive axes: both rank labels describe R=0.
    // The two rays reuse one job, while already admitted narrow jobs remain.
    assert_eq!(baseline.document["scheduled_nodes"], 3);
    assert_eq!(
        baseline.document["inputs"],
        json!([
            {"id":"narrow-a","domain":0},
            {"id":"narrow-b","domain":1},
            {"id":"whole-ray","domain":2},
            {"id":"duplicate","domain":0},
            {"id":"unbounded-rank","domain":2}
        ])
    );
    for workers in [2, 6] {
        if rustred::campaign::ParallelExecution::preflight_requested_core_budget(workers).is_err() {
            continue;
        }
        let mut request = request.clone();
        request.workers = workers;
        let parallel =
            owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
        assert!(
            parallel.all_scheduled_domains_resolved,
            "{}",
            parallel.document
        );
        for field in [
            "domains",
            "inputs",
            "scheduled_nodes",
            "completed_nodes",
            "queued_nodes",
            "failed_nodes",
            "processed_nodes",
            "successors",
            "conditional_successors",
            "frontiers",
            "events",
            "deduplication_hits",
            "exact_domain_hits",
            "full_orthant_hits",
            "containment_checks",
            "containment_summary_builds",
            "containment_semantic_hits",
            "containment_semantic_retirements",
            "max_scheduled_finite_rank",
            "unbounded_rank_domains",
            "optional_coefficient_refusals",
            "optional_original_refusals",
            "optional_coalesced_refusals",
        ] {
            assert_eq!(
                without_seconds(parallel.document[field].clone()),
                without_seconds(baseline.document[field].clone()),
                "{workers} workers: {field}"
            );
        }
        assert_eq!(parallel.document["parallel"]["active_workers"], 0);
        assert_eq!(parallel.document["parallel"]["worker_buffered_events"], 0);
        assert_eq!(
            parallel.document["parallel"]["attempted_events"],
            parallel.document["events"]
        );
        assert_eq!(parallel.document["uncommitted_inspections"], json!([]));
        assert_eq!(parallel.document["family_closure_claim"], false);
    }
}

#[test]
fn owner_domain_walk_parallel_global_cap_and_cancel_retain_attempts_not_completion() {
    if rustred::campaign::ParallelExecution::preflight_requested_core_budget(2).is_err() {
        return;
    }
    let fixture = Fixture::new();
    let mut request = multiple_domains(&fixture);
    request.workers = 2;
    request.max_events = 1;
    let stopped =
        owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!stopped.all_scheduled_domains_resolved);
    assert_eq!(stopped.document["events"], 1);
    assert_eq!(stopped.document["completed_nodes"], 0);
    assert!(stopped.document["processed_nodes"].as_u64().unwrap() <= 1);
    assert!(
        stopped.document["parallel"]["attempted_events"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert_eq!(stopped.document["parallel"]["active_workers"], 0);
    assert_eq!(
        stopped.document["parallel"]["first_failure"]["kind"],
        "coordinator_admission"
    );
    assert_eq!(
        stopped.document["parallel"]["finished_uncommitted_domains"]
            .as_u64()
            .unwrap(),
        stopped.document["uncommitted_inspections"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| !v["stats"].is_null())
            .count() as u64
    );
    for attempted in stopped.document["uncommitted_inspections"]
        .as_array()
        .unwrap()
    {
        assert_eq!(attempted["committed"], false);
        assert!(attempted["lower"].is_array());
        assert_eq!(attempted["owner"], "1");
    }
    let cancel = AtomicBool::new(false);
    let mut request = multiple_domains(&fixture);
    request.workers = 2;
    let stopped = owner_domain_walk_with_progress(request, &cancel, |e| {
        if e["event"] == "domain_started" {
            cancel.store(true, Ordering::Release);
        }
    })
    .unwrap();
    assert!(!stopped.all_scheduled_domains_resolved);
    assert_eq!(stopped.document["parallel"]["active_workers"], 0);
    assert_eq!(
        stopped.document["parallel"]["first_failure"]["kind"],
        "cancelled"
    );
    assert_eq!(stopped.document["recursive_worklist_exhausted"], false);
}

#[test]
fn owner_domain_walk_worker_and_frontier_preflight_is_explicit() {
    let fixture = Fixture::new();
    for (workers, frontiers) in [(0, 1), (65, 1), (1, 0)] {
        let mut request = multiple_domains(&fixture);
        request.workers = workers;
        request.max_frontiers = frontiers;
        assert!(owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).is_err());
    }
    // Frontier retention has no arbitrary global hard ceiling. A large finite
    // allowance and the production unlimited sentinel do not preallocate it.
    for frontiers in [1_000_001, usize::MAX] {
        let mut request = multiple_domains(&fixture);
        request.max_events = 100_000_000;
        request.max_frontiers = frontiers;
        let out =
            owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
        assert!(out.all_scheduled_domains_resolved);
        assert_eq!(out.document["max_frontiers"], frontiers);
        assert_eq!(out.document["max_events"], 100_000_000);
        assert_eq!(out.document["applied_limits"]["max_events"], 1_000_000);
    }
}

#[test]
fn owner_domain_walk_parallel_native_route_apply_and_missing_frontier_match_serial() {
    if rustred::campaign::ParallelExecution::preflight_requested_core_budget(2).is_err() {
        return;
    }
    let fixture = noninvolutive_route_fixture();
    for missing in [false, true] {
        let mut request = route_walk_request(&fixture, 0);
        if missing {
            let mut selection: Value =
                serde_json::from_str(&request.matching.selection_json).unwrap();
            selection["initial_frontier_routes"] = json!([]);
            request.matching.selection_json = selection.to_string();
        }
        let serial =
            owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {})
                .unwrap();
        request.workers = 2;
        let parallel =
            owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(
            parallel.all_scheduled_domains_resolved, !missing,
            "{}",
            parallel.document
        );
        assert!(parallel.document["error"].is_null());
        assert_eq!(parallel.document["domains"][0]["phase"], "Route");
        for field in [
            "domains",
            "inputs",
            "events",
            "frontiers",
            "successors",
            "conditional_successors",
            "routed_domains",
            "route_masks",
            "scheduled_nodes",
            "completed_nodes",
            "deduplication_hits",
            "exact_domain_hits",
            "full_orthant_hits",
            "containment_checks",
        ] {
            assert_eq!(
                without_seconds(parallel.document[field].clone()),
                without_seconds(serial.document[field].clone()),
                "missing={missing}: {field}"
            );
        }
        if !missing {
            assert!(
                parallel.document["domains"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["phase"] == "Apply")
            );
        } else {
            assert_eq!(
                parallel.document["domains"][0]["frontiers"][0]["kind"],
                "missing_route_cover"
            );
        }
    }
}
