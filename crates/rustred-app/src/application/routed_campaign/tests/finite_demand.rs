use super::*;
use crate::application::routed_campaign::finite_demand::trace::{self, Mode, Policy, Request};
use crate::application::routed_campaign::walking::finite_scope;
use std::sync::atomic::{AtomicBool, Ordering};

fn policy(points: usize) -> Policy {
    serde_json::from_value(json!({"mode":"compare","workers":1,
        "windows":{"max_seed_points":points,"max_seed_bytes":65536,"max_positive_layers":64,"max_partition_checks":10000},
        "per_batch_work":{"max_nodes":10000,"max_rule_applications":10000,"max_transport_calls":10000,
            "max_transport_operations":1000000,"max_transport_endpoints":100000,"max_coalescing_additions":100000},
        "cumulative_census":{"max_unique_selections":1000,"max_identity_payload_bytes":1048576},
        "cumulative_retention":{"max_completed_work":10000,"max_merge_payload_bytes":1048576},
        "max_total_rows":100,"max_source_bytes":1048576,"max_evidence_payload_bytes":1048576})).unwrap()
}
fn query(id: &str, a: u64) -> Value {
    json!({"id":id,"owner":"1","lower":[0],"upper":[a.saturating_sub(1)],
    "max_numerator_rank":0,"power_bounds":{"max_positive_power":a,"min_power_difference":0,"max_power_difference":10}})
}
fn source(f: &Fixture) -> OwnerDomainWalkRequest {
    let document = json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[query("original",4),query("empty",0)],
        "query_roles":{"required":["original","empty"],"auxiliary":[]}});
    let mut matching =
        OwnerDomainMatchRequest::new(f.request.selection_json.clone(), document.to_string());
    matching.owner_base = f.directory.clone();
    let mut source = OwnerDomainWalkRequest::new(matching);
    source.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(
        f.directory.join("must-not-open"),
    ));
    source.amendments.push(OwnerDomainWalkAmendment {path:f.directory.join("scope.json"),text:json!({
        "schema":OWNER_DOMAIN_WALK_SCOPE_EXTENSION_SCHEMA,"sequence":1,"parent":source.checkpoint_binding(),
        "queries":[query("appended",5)],"query_roles":{"required":["appended"],"auxiliary":[]},
        "provenance":{"test":"append preserves every original row"}}).to_string()});
    source
}

#[test]
fn finite_demand_shared_windows_match_separate_full_sets_across_append_and_empty_rows() {
    let f = Fixture::new();
    let source = source(&f);
    for (points, workers) in [(1, 1), (2, 4), (9, 1)] {
        let mut policy = policy(points);
        policy.workers = workers;
        let result = trace::run_with_progress(
            Request {
                source: &source,
                policy: &policy,
            },
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        assert!(result.completed, "{}", result.document);
        assert_eq!(result.document["rows"].as_array().unwrap().len(), 3);
        assert_eq!(result.document["sources"].as_array().unwrap().len(), 2);
        assert_eq!(result.document["input_occurrences"], "9");
        assert_eq!(result.document["rows"][1]["empty"], true);
        for name in ["terminals", "visited_zeros", "composite_rule_identity_sets"] {
            assert_eq!(result.document["exact_set_parity"][name], true);
        }
        assert_eq!(result.document["work_checkpoint"], false);
        assert!(!f.directory.join("must-not-open").exists());
    }
}

#[test]
fn finite_demand_source_bridge_rejects_chain_role_and_budget_mutations_before_owner_load() {
    let f = Fixture::new();
    let source = source(&f);
    assert_eq!(
        finite_scope::parse(&source, 1, 3, 1048576)
            .unwrap()
            .rows
            .len(),
        3
    );
    assert!(finite_scope::parse(&source, 1, 2, 1048576).is_err());
    assert!(finite_scope::parse(&source, 1, 3, 1).is_err());
    for (field, value) in [
        ("parent", json!("0".repeat(64))),
        ("sequence", json!(2)),
        (
            "query_roles",
            json!({"required":[],"auxiliary":["appended"]}),
        ),
    ] {
        let mut changed = source.clone();
        let mut doc: Value = serde_json::from_str(&changed.amendments[0].text).unwrap();
        doc[field] = value;
        changed.amendments[0].text = doc.to_string();
        assert!(
            finite_scope::parse(&changed, 1, 3, 1048576).is_err(),
            "{field}"
        );
    }
    let mut changed = source.clone();
    let mut doc: Value = serde_json::from_str(&changed.matching.queries_json).unwrap();
    doc.as_object_mut().unwrap().remove("query_roles");
    changed.matching.queries_json = doc.to_string();
    assert!(finite_scope::parse(&changed, 1, 3, 1048576).is_err());
}

#[test]
fn finite_demand_failed_and_cancelled_windows_never_report_full_scope_complete() {
    let f = Fixture::new();
    let source = source(&f);
    let mut p = policy(1);
    p.mode = Mode::SharedWindows;
    p.cumulative_retention.max_completed_work = 0;
    let result = trace::run_with_progress(
        Request {
            source: &source,
            policy: &p,
        },
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert!(!result.completed);
    assert_eq!(
        result.document["shared"]["rows"].as_array().unwrap().len(),
        3
    );
    assert_eq!(
        result.document["shared"]["rows"][0]["completed_occurrences"],
        "0"
    );
    p = policy(1);
    p.mode = Mode::SharedWindows;
    let cancel = AtomicBool::new(false);
    let result = trace::run_with_progress(
        Request {
            source: &source,
            policy: &p,
        },
        &cancel,
        |event| {
            if event["event"] == "finite_demand_window_complete" {
                cancel.store(true, Ordering::Release);
            }
        },
    )
    .unwrap();
    assert!(!result.completed);
    assert_eq!(result.document["shared"]["rows"][0]["complete"], false);
    assert_eq!(result.document["shared"]["rows"][2]["status"], "pending");
    assert!(!f.directory.join("must-not-open").exists());
}

#[test]
fn finite_demand_policy_is_strict_and_does_not_rename_per_batch_caps_as_total_grants() {
    let p = policy(2);
    let mut value = serde_json::to_value(&p).unwrap();
    value["total_work"] = value["per_batch_work"].clone();
    assert!(serde_json::from_value::<Policy>(value).is_err());
}

#[test]
fn finite_demand_checks_every_original_geometry_before_starting_any_window() {
    let f = Fixture::new();
    let mut source = source(&f);
    source.amendments.clear();
    let mut document: Value = serde_json::from_str(&source.matching.queries_json).unwrap();
    document["queries"][1] = query("empty", 4);
    document["queries"][1]["upper"] = json!([1]);
    source.matching.queries_json = document.to_string();
    let native_seen = AtomicBool::new(false);
    let p = policy(1);
    let result = trace::run_with_progress(
        Request {
            source: &source,
            policy: &p,
        },
        &AtomicBool::new(false),
        |event| {
            if event["event"] == "finite_demand_native" {
                native_seen.store(true, Ordering::Release);
            }
        },
    );
    assert!(result.is_err());
    assert!(!native_seen.load(Ordering::Acquire));
    assert!(!f.directory.join("must-not-open").exists());
}

#[test]
fn finite_demand_per_batch_rule_grant_reaches_native_nonterminal_work() {
    let f = Fixture::new();
    let source = source(&f);
    let mut p = policy(2);
    p.mode = Mode::SharedWindows;
    let normal = trace::run_with_progress(
        Request {
            source: &source,
            policy: &p,
        },
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert!(normal.completed);
    assert!(
        normal.document["shared"]["evidence"]["completed_work_totals"]["rule_applications"]
            .as_u64()
            .unwrap()
            > 0
    );
    p.per_batch_work.max_rule_applications = 0;
    let refused = trace::run_with_progress(
        Request {
            source: &source,
            policy: &p,
        },
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert!(!refused.completed);
    assert_eq!(refused.document["shared"]["rows"][0]["complete"], false);
    assert!(
        refused.document["shared"]["error"]
            .as_str()
            .unwrap()
            .contains("rule")
    );
}
