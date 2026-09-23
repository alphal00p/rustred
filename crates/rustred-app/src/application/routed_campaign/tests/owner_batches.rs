//! Actual-native acceptance controls for the opt-in owner publication policy.
//! Reuse the parent fixtures: no alternate source generator or algebra oracle.
use super::*;
use crate::{OwnerDomainWalkPublicationPolicy, OwnerDomainWalkSchedulingPolicy};
use std::num::NonZeroUsize;

mod worker_budget;

fn available_workers() -> impl Iterator<Item = usize> {
    [1, 2, 6, 50].into_iter().filter(|&workers| {
        let available =
            rustred::campaign::ParallelExecution::preflight_requested_core_budget(workers).is_ok();
        if !available {
            eprintln!("owner-batched native acceptance: {workers} workers unavailable");
        }
        available
    })
}

fn saved_owner_bytes(fixture: &Fixture) -> Vec<(PathBuf, Vec<u8>)> {
    let selection: Value = serde_json::from_str(&fixture.request.selection_json).unwrap();
    selection["owners"]
        .as_array()
        .unwrap()
        .iter()
        .map(|owner| {
            let path = fixture.directory.join(owner["path"].as_str().unwrap());
            let bytes = std::fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}

fn assert_unchanged(before: &[(PathBuf, Vec<u8>)]) {
    for (path, bytes) in before {
        // Byte equality is stronger than comparing a lossy summary or a hash.
        assert_eq!(&std::fs::read(path).unwrap(), bytes, "{}", path.display());
    }
}

fn assert_no_authority_claim(document: &Value) {
    for flag in [
        "family_closure_claim",
        "ibp_generation",
        "independent_certification",
    ] {
        assert_eq!(document[flag], false, "{flag}: {document}");
    }
}

fn assert_resolved(result: &OwnerDomainWalkResult) {
    let document = &result.document;
    assert!(result.all_scheduled_domains_resolved, "{document}");
    assert_eq!(document["all_scheduled_domains_resolved"], true);
    assert_eq!(document["status"], "locally_resolved");
    assert_eq!(document["recursive_worklist_exhausted"], true);
    assert_eq!(document["queued_nodes"], 0);
    assert_eq!(document["failed_nodes"], 0);
    assert_eq!(document["frontiers"], 0);
    assert!(document["error"].is_null(), "{document}");
    assert_eq!(document["input_frontiers"], json!([]));
    assert_eq!(document["uncommitted_inspections"], json!([]));
    assert_eq!(
        document["traversal_timing_boundary"],
        "after_owner_preparation_through_initial_admission_walk_report_and_queue_cleanup; excludes_owner_unload_and_output_write"
    );
    let prepared = document["prepared_seconds"].as_f64().unwrap();
    let traversal = document["traversal_seconds"].as_f64().unwrap();
    let elapsed = document["elapsed_seconds"].as_f64().unwrap();
    assert!(prepared >= 0.0 && traversal >= 0.0);
    assert!((prepared + traversal - elapsed).abs() < 1e-9);
    if let Some(driver) = document["native_driver_seconds"].as_f64() {
        assert!(driver <= traversal + 1e-9);
    }
    assert_no_authority_claim(document);
}

fn batched(mut request: OwnerDomainWalkRequest, workers: usize) -> OwnerDomainWalkRequest {
    request.publication_policy = OwnerDomainWalkPublicationPolicy::OwnerBatched;
    request.workers = workers;
    request
}

#[test]
fn owner_batched_native_ray_reuses_one_slot_without_changing_saved_rules() {
    let fixture = Fixture::new();
    let before = saved_owner_bytes(&fixture);
    for workers in available_workers() {
        let mut request = batched(
            OwnerDomainWalkRequest::new(match_request(&fixture)),
            workers,
        );
        request.max_domains = 1;
        let result =
            owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
        assert_resolved(&result);
        assert_eq!(result.document["scheduled_nodes"], 1);
        assert_eq!(result.document["completed_nodes"], 1);
        assert!(result.document["successors"].as_u64().unwrap() > 0);
        assert!(result.document["deduplication_hits"].as_u64().unwrap() > 0);
        let rows = result.document["domains"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["phase"], "Apply");
        assert_eq!(rows[0]["owner"], "1");
        assert_eq!(rows[0]["lower"], json!([0]));
        assert_eq!(rows[0]["upper"], json!([null]));
    }
    assert_unchanged(&before);
}

#[test]
fn owner_batched_initial_domain_cap_never_closes_only_the_retained_prefix() {
    let fixture = noninvolutive_route_fixture();
    let before = saved_owner_bytes(&fixture);
    for workers in available_workers() {
        let mut request = batched(route_walk_request(&fixture, 0), workers);
        // The first input is already a terminal. Silently discarding the
        // admission error for the second owner would make this retained
        // prefix appear resolved, despite never processing the Route root.
        request.matching.queries_json = json!({
            "schema":"rustred.owner-domain-queries.json.v2", "queries":[
                {"id":"retained-terminal", "owner":"011", "lower":[0,0,0],
                 "upper":[0,0,0], "max_numerator_rank":0},
                {"id":"unadmitted-route", "owner":"110", "lower":[0,0,0],
                 "upper":[null,null,null], "max_numerator_rank":0}
            ]
        })
        .to_string();
        request.max_domains = 1;
        let result =
            owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
        assert!(
            !result.all_scheduled_domains_resolved,
            "{}",
            result.document
        );
        assert_eq!(result.document["status"], "incomplete");
        assert_eq!(result.document["all_scheduled_domains_resolved"], false);
        assert_eq!(result.document["recursive_worklist_exhausted"], false);
        assert!(
            result.document["error"]
                .as_str()
                .unwrap()
                .contains("domain allowance")
        );
        assert_eq!(result.document["scheduled_nodes"], 1);
        assert_eq!(result.document["completed_nodes"], 0);
        assert_eq!(result.document["processed_nodes"], 0);
        assert_eq!(result.document["queued_nodes"], 1);
        assert_eq!(result.document["domains"], json!([]));
        assert_eq!(
            result.document["requested_publication_policy"],
            "owner_batched"
        );
        assert_eq!(result.document["owner_batched_traversal_started"], false);
        assert_no_authority_claim(&result.document);
    }
    assert_unchanged(&before);
}

#[test]
fn owner_batched_native_two_loop_route_delivers_to_shared_apply_owner() {
    let fixture = noninvolutive_route_fixture();
    let before = saved_owner_bytes(&fixture);
    let mut route_statistics = None;
    for workers in available_workers() {
        for policy in [
            OwnerDomainWalkPublicationPolicy::Ordered,
            OwnerDomainWalkPublicationPolicy::OwnerBatched,
        ] {
            let mut request = route_walk_request(&fixture, 0);
            let mut queries: Value = serde_json::from_str(&request.matching.queries_json).unwrap();
            queries["queries"].as_array_mut().unwrap().push(json!({
                "id":"initial-destination-terminal", "owner":"011",
                "lower":[0,0,0], "upper":[0,0,0], "max_numerator_rank":0
            }));
            request.matching.queries_json = queries.to_string();
            request.workers = workers;
            request.publication_policy = policy;
            let result =
                owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
            assert_resolved(&result);
            assert_eq!(result.document["routing_expanded"], false);
            let rows = result.document["domains"].as_array().unwrap();
            let route = rows
                .iter()
                .find(|row| row["phase"] == "Route" && row["owner"] == "110")
                .expect("the genuine nonidentity source route must be inspected");
            assert!(route["stats"]["apply_domains"].as_u64().unwrap() > 0);
            assert_eq!(route["stats"]["missing_routes"], 0);
            if let Some(expected) = &route_statistics {
                // This original input region and native route operation are
                // identical even if later publication IDs or covers differ.
                assert_eq!(&route["stats"], expected);
            } else {
                route_statistics = Some(route["stats"].clone());
            }
            let applied = rows
                .iter()
                .find(|row| {
                    row["phase"] == "Apply"
                        && row["owner"] == "011"
                        && row["upper"] == json!([null, null, null])
                })
                .expect("cross-key delivery must reopen the destination Apply queue");
            assert_eq!(applied["rank"], 0);
            assert_eq!(applied["upper"], json!([null, null, null]));
            assert!(result.document["successors"].as_u64().unwrap() > 0);
            if policy == OwnerDomainWalkPublicationPolicy::OwnerBatched {
                assert!(result.document["owner_bucket_count"].as_u64().unwrap() >= 2);
                let destination = result.document["owner_buckets"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|bucket| bucket["phase"] == "Apply" && bucket["owner"] == "011")
                    .unwrap();
                assert!(destination["cross_owner_requests"].as_u64().unwrap() > 0);
            }
            // Publication IDs and cover fragmentation are policy-dependent.
            // Require actual native routing and exhausted obligations instead
            // of accidentally pinning the old global diagnostic order.
        }
    }
    assert_unchanged(&before);
}

#[test]
fn owner_batched_native_descendants_are_not_clipped_to_the_entry_power_band() {
    let fixture = Fixture::new();
    let before = saved_owner_bytes(&fixture);
    for workers in available_workers() {
        let mut matching = match_request(&fixture);
        matching.queries_json = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
            {"id":"finite-entry-three-four", "owner":"1", "lower":[0], "upper":[null],
             "max_numerator_rank":11, "power_bounds":{"max_positive_power":4,
             "min_power_difference":3,"max_power_difference":4}}]})
        .to_string();
        let result = owner_domain_walk_with_progress(
            batched(OwnerDomainWalkRequest::new(matching), workers),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        assert_resolved(&result);
        let rows = result.document["domains"].as_array().unwrap();
        assert!(
            rows.iter().any(|row| {
                row["phase"] == "Apply"
                    && row["lower"][0] == 0
                    && row["power_bounds"]["min_power_difference"]
                        .as_i64()
                        .is_some_and(|power| power < 3)
            }),
            "terminal-reaching descendants outside n=3..4 were lost: {}",
            result.document
        );
    }
    assert_unchanged(&before);
}

#[test]
fn owner_batched_native_route_keeps_rank_above_saved_generation_scope_on_refusal() {
    let fixture = noninvolutive_route_fixture();
    let before = saved_owner_bytes(&fixture);
    for workers in available_workers() {
        // The reused fixture was generated with R=2. An external regional
        // obligation at R=11 must remain R=11, even on an incomplete run.
        let mut request = batched(route_walk_request(&fixture, 11), workers);
        request.max_route_masks = 1;
        let result =
            owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
        assert!(
            !result.all_scheduled_domains_resolved,
            "{}",
            result.document
        );
        assert_eq!(result.document["status"], "incomplete");
        assert_eq!(result.document["recursive_worklist_exhausted"], false);
        assert!(
            result.document["error"]
                .as_str()
                .unwrap()
                .contains("route masks")
        );
        assert!(
            result.document["domains"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| {
                    row["phase"] == "Route" && row["owner"] == "110" && row["rank"] == 11
                })
        );
        assert_no_authority_claim(&result.document);
    }
    assert_unchanged(&before);
}

#[test]
fn owner_batched_native_missing_route_is_a_frontier_not_a_terminal() {
    let fixture = noninvolutive_route_fixture();
    for workers in available_workers() {
        let mut request = batched(route_walk_request(&fixture, 0), workers);
        let mut selection: Value = serde_json::from_str(&request.matching.selection_json).unwrap();
        selection["initial_frontier_routes"] = json!([]);
        request.matching.selection_json = selection.to_string();
        let result =
            owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
        assert!(
            !result.all_scheduled_domains_resolved,
            "{}",
            result.document
        );
        assert_eq!(result.document["status"], "incomplete");
        assert!(result.document["frontiers"].as_u64().unwrap() > 0);
        let missing = result.document["domains"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|row| row["frontiers"].as_array().unwrap())
            .find(|frontier| frontier["kind"] == "missing_route_cover")
            .expect("missing route must survive owner-local completion");
        assert_eq!(missing["reached_missing_rule_claim"], false);
        assert_no_authority_claim(&result.document);
    }
}

#[test]
fn owner_batched_native_source_validity_is_not_lost_before_partition_admission() {
    let fixture = noninvolutive_route_fixture_with_scale(true);
    let mut request = batched(route_walk_request(&fixture, 1), 1);
    let mut selection: Value = serde_json::from_str(&request.matching.selection_json).unwrap();
    selection["initial_frontier_routes"] = json!([]);
    request.matching.selection_json = selection.to_string();
    let result = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(
        !result.all_scheduled_domains_resolved,
        "{}",
        result.document
    );
    assert_eq!(result.document["status"], "incomplete");
    assert!(
        result.document["input_frontiers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|frontier| {
                frontier["kind"] == "initial_route_source_validity_obligation"
                    && frontier["reached_missing_rule_claim"] == false
            })
    );
    assert_eq!(result.document["routed_domains"], 0);
    assert_no_authority_claim(&result.document);
}

#[test]
fn owner_batched_native_event_cap_and_active_cancel_never_report_exhaustion() {
    let fixture = Fixture::new();
    for workers in available_workers() {
        let mut request = batched(
            OwnerDomainWalkRequest::new(match_request(&fixture)),
            workers,
        );
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
        assert!(
            limited.document["error"]
                .as_str()
                .unwrap()
                .contains("event allowance")
        );
        assert_no_authority_claim(&limited.document);

        let cancellation = AtomicBool::new(false);
        let stopped = owner_domain_walk_with_progress(
            batched(
                OwnerDomainWalkRequest::new(match_request(&fixture)),
                workers,
            ),
            &cancellation,
            |event| {
                if event["event"] == "domain_started" {
                    cancellation.store(true, Ordering::Release);
                }
            },
        )
        .unwrap();
        assert!(
            cancellation.load(Ordering::Acquire),
            "active cancellation was never exercised"
        );
        assert!(
            !stopped.all_scheduled_domains_resolved,
            "{}",
            stopped.document
        );
        assert_eq!(stopped.document["status"], "incomplete");
        assert_eq!(stopped.document["recursive_worklist_exhausted"], false);
        assert!(
            stopped.document["error"]
                .as_str()
                .unwrap()
                .contains("cancel")
        );
        assert_no_authority_claim(&stopped.document);
    }
}

#[test]
fn owner_batched_native_initial_band_reuse_keeps_its_pinned_anchor_obligation() {
    let fixture = Fixture::new();
    let before = saved_owner_bytes(&fixture);
    for workers in available_workers() {
        let mut matching = match_request(&fixture);
        matching.queries_json = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
            {"id":"initial-n-three-six", "owner":"1", "lower":[2], "upper":[5],
             "max_numerator_rank":11}]})
        .to_string();
        let mut request = batched(OwnerDomainWalkRequest::new(matching), workers);
        request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
            lookahead: NonZeroUsize::new(2).unwrap(),
        };
        request.reuse_initial_d_bands = true;
        let result =
            owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
        assert_resolved(&result);
        assert_eq!(result.document["reuse_initial_d_bands"], true);
        assert_eq!(
            result.document["delegation"]["all_ledger_obligations_discharged"],
            true
        );
        let rows = result.document["domains"].as_array().unwrap();
        let anchor = rows
            .iter()
            .find(|row| {
                row["phase"] == "Apply" && row["lower"] == json!([2]) && row["upper"] == json!([5])
            })
            .expect("the actual initial anchor must be inspected, not skipped as reusable");
        assert_ne!(anchor["record_kind"], "partial_initial_overlap_inspection");
        assert_eq!(anchor["local_classification_discharged"], true);
        let partials: Vec<_> = rows
            .iter()
            .filter(|row| row["record_kind"] == "partial_initial_overlap_inspection")
            .collect();
        assert!(
            !partials.is_empty(),
            "actual descending K1 children must cross the initial D band: {}",
            result.document
        );
        for partial in partials {
            assert_eq!(partial["initial_overlap"]["cut"], 3);
            assert_eq!(partial["local_inspection_finished"], false);
            assert_eq!(partial["residual_inspection_finished"], true);
            assert_eq!(partial["local_classification_discharged"], true);
        }
    }
    assert_unchanged(&before);
}
