use super::*;
use crate::application::routed_campaign::finite_demand::trace::{
    self, Mode, Policy, ReductionAggregateOverrides, Request, WorkLimits,
};
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
    source_with_reduction(f, f.request.reduction_limits)
}
fn source_with_reduction(
    f: &Fixture,
    limits: rustred::reduction::ReductionLimits,
) -> OwnerDomainWalkRequest {
    let document = json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[query("original",4),query("empty",0)],
        "query_roles":{"required":["original","empty"],"auxiliary":[]}});
    let mut matching =
        OwnerDomainMatchRequest::new(f.request.selection_json.clone(), document.to_string());
    matching.owner_base = f.directory.clone();
    // Synthetic source policy is fixed BEFORE constructing its amendment chain.
    matching.reduction_limits = limits;
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
    assert!(p.prepared_reduction_aggregates.is_none());
    let mut value = serde_json::to_value(&p).unwrap();
    assert!(value.get("prepared_reduction_aggregates").is_none());
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

fn no_batch_count_stops(p: &mut Policy) {
    p.per_batch_work = WorkLimits {
        max_nodes: usize::MAX,
        max_rule_applications: usize::MAX,
        max_transport_calls: usize::MAX,
        max_transport_operations: usize::MAX,
        max_transport_endpoints: usize::MAX,
        max_coalescing_additions: usize::MAX,
    };
}

#[test]
fn finite_demand_explicit_aggregate_override_preserves_authenticated_source_and_raw_census() {
    let f = Fixture::new();
    let mut zero_limits = f.request.reduction_limits;
    zero_limits.max_rule_applications = 0;
    let source = source_with_reduction(&f, zero_limits);
    let binding = source.checkpoint_binding();
    let original_amendment = source.amendments[0].text.clone();
    let mut p = policy(2);
    p.mode = Mode::SharedWindows;
    no_batch_count_stops(&mut p);
    let run = |source: &OwnerDomainWalkRequest, policy: &Policy| {
        trace::run_with_progress(Request { source, policy }, &AtomicBool::new(false), |_| {})
            .unwrap()
    };
    // All per-batch counters at MAX do NOT silently override the source's zero.
    let refused = run(&source, &p);
    assert!(!refused.completed);
    assert_eq!(
        refused.document["effective_batch_aggregate_limits"]["max_rule_applications"],
        0
    );
    let funded_source = source_with_reduction(&f, f.request.reduction_limits);
    let control = run(&funded_source, &p);
    assert!(control.completed);
    p.prepared_reduction_aggregates = Some(ReductionAggregateOverrides {
        max_rule_applications: usize::MAX,
        max_pending_frames: usize::MAX,
        max_coalescing_additions: usize::MAX,
    });
    let result = run(&source, &p);
    assert!(result.completed, "{}", result.document);
    assert_eq!(source.matching.reduction_limits, zero_limits);
    assert_eq!(source.amendments[0].text, original_amendment);
    assert_eq!(source.checkpoint_binding(), binding);
    assert_eq!(result.document["scope_request_binding"], binding);
    assert_eq!(
        result.document["scope_request_binding_is_execution_identity"],
        false
    );
    assert_eq!(
        result.document["source_reduction_limits"],
        format!("{zero_limits:?}")
    );
    assert_eq!(result.document["rows"], control.document["rows"]);
    assert_eq!(
        result.document["shared"]["evidence"],
        control.document["shared"]["evidence"]
    );
    assert_eq!(
        result.document["local_expansion_limits"],
        control.document["local_expansion_limits"]
    );
    let effective = &result.document["effective_batch_aggregate_limits"];
    assert_eq!(effective["max_input_targets"], 2);
    for field in [
        "max_unique_nodes",
        "max_rule_applications",
        "max_pending_frames",
        "max_coalescing_additions",
        "max_transport_calls",
        "max_transport_operations",
        "max_transport_endpoints",
    ] {
        assert_eq!(effective[field], json!(usize::MAX), "{field}");
    }
    assert!(!f.directory.join("must-not-open").exists());
}

#[test]
fn finite_demand_aggregate_override_cannot_repair_or_bypass_an_invalid_original_chain() {
    let f = Fixture::new();
    let mut source = source(&f);
    // Invalidates the pre-existing chain: the diagnostic override must NOT rebind it.
    source.matching.reduction_limits.max_rule_applications = 0;
    let mut p = policy(2);
    p.prepared_reduction_aggregates = Some(ReductionAggregateOverrides {
        max_rule_applications: usize::MAX,
        max_pending_frames: usize::MAX,
        max_coalescing_additions: usize::MAX,
    });
    let observed = AtomicBool::new(false);
    let result = trace::run_with_progress(
        Request {
            source: &source,
            policy: &p,
        },
        &AtomicBool::new(false),
        |_| {
            observed.store(true, Ordering::Release);
        },
    );
    assert!(result.is_err());
    assert!(!observed.load(Ordering::Acquire));
    assert!(!f.directory.join("must-not-open").exists());
}

#[test]
fn finite_demand_override_zero_remains_zero_and_effective_native_minima_are_reported() {
    let f = Fixture::new();
    let source = source(&f);
    let mut p = policy(2);
    p.mode = Mode::SharedWindows;
    p.prepared_reduction_aggregates = Some(ReductionAggregateOverrides {
        max_rule_applications: 0,
        max_pending_frames: 20_000,
        max_coalescing_additions: 20_000,
    });
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
    let effective = &result.document["effective_batch_aggregate_limits"];
    assert_eq!(effective["max_rule_applications"], 0);
    assert_eq!(effective["max_pending_frames"], 10_000);
    assert_eq!(effective["max_coalescing_additions"], 20_000);
    assert_ne!(
        result.document["source_reduction_limits"],
        result.document["effective_reduction_limits"]
    );
}
