//! Current publication policy `epoch` end to end on
//! the one-axis fixture: drained and resolved, identical across widths, and
//! its final export certified by the closure verifier.
use super::*;
use crate::{
    OwnerDomainWalkCheckpointOptions, OwnerDomainWalkPublicationPolicy,
    OwnerDomainWalkSchedulingPolicy, OwnerDomainWalkVerifyOptions,
};

fn epoch_request(fixture: &Fixture, workers: usize) -> OwnerDomainWalkRequest {
    let mut matching = match_request(fixture);
    matching.queries_json = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"narrow-a","owner":"1","lower":[2],"upper":[2],"max_numerator_rank":11},
        {"id":"narrow-b","owner":"1","lower":[3],"upper":[3],"max_numerator_rank":11},
        {"id":"whole-ray","owner":"1","lower":[0],"upper":[null],"max_numerator_rank":11},
        {"id":"duplicate","owner":"1","lower":[2],"upper":[2],"max_numerator_rank":11}
    ]})
    .to_string();
    let mut request = OwnerDomainWalkRequest::new(matching);
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Epoch;
    request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
        lookahead: std::num::NonZeroUsize::new(8).unwrap(),
    };
    request.workers = workers;
    request
}

fn without_timing(mut value: Value) -> Value {
    fn visit(v: &mut Value) {
        match v {
            Value::Object(o) => {
                o.retain(|key, _| key != "seconds" && !key.ends_with("_seconds"));
                o.values_mut().for_each(visit);
            }
            Value::Array(a) => a.iter_mut().for_each(visit),
            _ => {}
        }
    }
    visit(&mut value);
    value
}

#[test]
fn epoch_walk_drains_resolved_and_is_identical_across_widths() {
    let fixture = Fixture::new();
    let mut baseline = None;
    for workers in [1, 2, 4] {
        if !crate::test_gates::workers_or_skip("epoch walk", workers) {
            continue;
        }
        let result = owner_domain_walk_with_progress(
            epoch_request(&fixture, workers),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        let document = result.into_document().unwrap();
        assert_eq!(document["status"], "locally_resolved", "{document}");
        assert_eq!(
            document["walk_semantics_version"],
            crate::OWNER_DOMAIN_WALK_EPOCH_SEMANTICS_VERSION
        );
        assert_eq!(document["family_closure_claim"], false);
        assert_eq!(document["epoch"]["ledger6"]["pending"], 0);
        assert_eq!(document["epoch"]["certified"], true);
        // Uncheckpointed control: one forced refresh at drain, none during merges.
        assert_eq!(document["descendant_closure"]["refresh_count"], 1);
        // The duplicate query shares the first narrow domain's root.
        assert_eq!(
            document["inputs"][3]["domain"],
            document["inputs"][0]["domain"]
        );
        let records = document["domains"].as_array().unwrap();
        assert_eq!(
            records.len() as u64,
            document["scheduled_nodes"].as_u64().unwrap()
        );
        for record in records {
            assert_eq!(record["descendant_closed"], true, "{record}");
            if record["record_kind"] == "delegated_not_inspected" {
                assert!(record["representative_id"].as_u64() > record["id"].as_u64());
            } else {
                assert_eq!(record["accepted_events"], record["stats"]["events"]);
            }
        }
        let comparable = without_timing(json!({"domains":document["domains"],
            "inputs":document["inputs"],"epoch_ledger":document["epoch"]["ledger6"],
            "digests":[document["epoch"]["records_digest"],document["epoch"]["edge_digest"]]}));
        match &baseline {
            None => baseline = Some(comparable),
            Some(expected) => assert_eq!(&comparable, expected, "W{workers} differs from W1"),
        }
    }
}

#[test]
fn epoch_cp6_raw_is_cold_certified_and_public_w1_resume_is_supported() {
    let fixture = Fixture::new();
    let directory = fixture.directory.join("epoch-cp6");
    let mut request = epoch_request(&fixture, 1);
    request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&directory));
    let result =
        owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert!(
        !result.all_scheduled_domains_resolved,
        "{}",
        result.document
    );
    assert!(!result.records.is_streamed());
    assert_eq!(result.document["recursive_worklist_exhausted"], true);
    assert_eq!(result.document["full_result_in_output_document"], false);
    assert_eq!(result.document["finalization"], "not_evaluated");
    assert!(directory.join("latest.json").is_file());
    assert!(!directory.join("epoch-export.json").exists());
    let mut options = OwnerDomainWalkVerifyOptions::new(&directory);
    options.require_closure = true;
    let report = crate::owner_domain_walk_verify_closure(
        &request,
        &options,
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(report["verdict"], "PASS", "{report}");
    assert_eq!(
        report["roots_independently_verified"],
        report["roots_total"]
    );
    assert_eq!(report["checkpoint"]["publication_policy"], "epoch");
    assert_eq!(report["checkpoint"]["request_binding_matches"], true);
    // Cold raw remains authoritative; a checkpoint-only document is NOT a
    // complete record inventory for --result equality. Outer adapter required.
    let summary_file = fixture.directory.join("epoch-summary.json");
    std::fs::write(&summary_file, serde_json::to_vec(&result.document).unwrap()).unwrap();
    let mut paired = options.clone();
    paired.result = Some(summary_file);
    let incomplete =
        crate::owner_domain_walk_verify_closure(&request, &paired, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(incomplete["verdict"], "INCOMPLETE", "{incomplete}");
    // Fresh Union is supported, but it cannot reinterpret an Off checkpoint.
    // Offline verification must report the request-binding mismatch as FAIL.
    let mut unsupported = request.clone();
    unsupported.g2_residual_anchors = crate::OwnerDomainWalkG2ResidualAnchors::Union;
    let mismatched = crate::owner_domain_walk_verify_closure(
        &unsupported,
        &options,
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(mismatched["verdict"], "FAIL", "{mismatched}");
    assert_eq!(mismatched["checkpoint"]["request_binding_matches"], false);
    assert_eq!(mismatched["violations_by_class"]["binding"], 1);
    assert!(
        mismatched["violations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| {
                value
                    .as_str()
                    .is_some_and(|text| {
                        text.contains("binding: checkpoint request digest or persisted schedule differs from the command's binding")
                    })
            }),
        "{mismatched}"
    );
    // Activation on resume is still unsupported by execution and cold read.
    unsupported.g2_residual_anchors = crate::OwnerDomainWalkG2ResidualAnchors::Off;
    unsupported.g2_activate_on_resume = true;
    assert!(
        crate::owner_domain_walk_verify_closure(
            &unsupported,
            &options,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap_err()
        .to_string()
        .contains("G2'")
    );
    // Fresh still refuses a nonempty destination. Explicit same-request W1
    // resume reopens validated state and writes a new durable generation.
    assert!(
        owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).is_err()
    );
    let mut resume = request;
    resume.checkpoint.as_mut().unwrap().resume = true;
    let resumed =
        owner_domain_walk_with_progress(resume.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(resumed.document["recursive_worklist_exhausted"], true);
    assert_eq!(resumed.document["full_result_in_output_document"], false);
    assert!(
        resumed.document["checkpoint"]["generation"]
            .as_u64()
            .unwrap()
            > result.document["checkpoint"]["generation"]
                .as_u64()
                .unwrap()
    );
    let reopened =
        crate::owner_domain_walk_verify_closure(&resume, &options, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(reopened["verdict"], "PASS", "{reopened}");
    assert_eq!(
        reopened["roots_independently_verified"],
        report["roots_independently_verified"]
    );
}

/// The frontier fixture of `frontier_policy.rs`: the owner-110 route ray and
/// an owner-101 route ray have no installed initial route (each Route
/// inspection reports a `missing_route_cover` frontier), plus a literal
/// owner-011 Apply box, so work remains after the first frontier.
fn missing_route_epoch(fixture: &Fixture) -> OwnerDomainWalkRequest {
    let mut request = route_walk_request(fixture, 0);
    let mut selection: Value = serde_json::from_str(&request.matching.selection_json).unwrap();
    selection["initial_frontier_routes"] = json!([]);
    request.matching.selection_json = selection.to_string();
    let mut queries: Value = serde_json::from_str(&request.matching.queries_json).unwrap();
    let rows = queries["queries"].as_array_mut().unwrap();
    rows.push(json!({"id":"literal-apply", "owner":"011", "lower":[0,0,0],
        "upper":[0,2,2], "max_numerator_rank":1}));
    rows.push(
        json!({"id":"second-missing-route", "owner":"101", "lower":[0,0,0],
        "upper":[null,null,null], "max_numerator_rank":0}),
    );
    request.matching.queries_json = queries.to_string();
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Epoch;
    request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
        lookahead: std::num::NonZeroUsize::new(8).unwrap(),
    };
    request
}

#[test]
fn epoch_frontier_policy_stops_at_the_first_frontier_merge_or_records_it() {
    let fixture = noninvolutive_route_fixture();
    // Stop (the epoch default): the merge that commits the first frontier
    // stops the walk (exit 4 semantics), no checkpoint needed.
    let mut stop = missing_route_epoch(&fixture);
    stop.frontier_policy = crate::OwnerDomainWalkFrontierPolicy::Stop;
    let document = owner_domain_walk_with_progress(stop, &AtomicBool::new(false), |_| {})
        .unwrap()
        .into_document()
        .unwrap();
    assert_eq!(document["status"], "stopped", "{document}");
    assert_eq!(document["stop_reason"], "frontier_stop");
    assert_eq!(document["all_scheduled_domains_resolved"], false);
    assert!(document["frontiers"].as_u64().unwrap() >= 1);
    assert_eq!(document["family_closure_claim"], false);
    // Record: the walk drains, uncertified; frontier records never seal.
    let mut record = missing_route_epoch(&fixture);
    record.frontier_policy = crate::OwnerDomainWalkFrontierPolicy::Record;
    let document = owner_domain_walk_with_progress(record, &AtomicBool::new(false), |_| {})
        .unwrap()
        .into_document()
        .unwrap();
    assert_eq!(document["status"], "incomplete", "{document}");
    assert_eq!(document["stop_reason"], "drained_uncertified");
    assert_eq!(document["recursive_worklist_exhausted"], true);
    assert!(
        document["epoch"]["ledger6"]["native_frontier"]
            .as_u64()
            .unwrap()
            >= 2
    );
    for row in document["domains"].as_array().unwrap() {
        if row["frontiers"].as_array().is_some_and(|f| !f.is_empty()) {
            assert_eq!(row["descendant_closed"], false, "{row}");
            assert_eq!(row["local_classification_discharged"], false);
        }
    }
}

/// `consumer_stop_parity` (§9.1) on the unchanged native visitor: a forced
/// resolver break at a job's first event must satisfy P1's break relation
/// (stats == emitted == accepted + 1: the solver charges the breaking event
/// before the visitor refuses it). The walk then merges C2 NativeErrors and
/// stops with `error_stop`; a wrong relation would be C5 (an Err here).
#[test]
fn consumer_stop_parity_on_the_real_native_visitor() {
    let fixture = Fixture::new();
    crate::application::routed_campaign::walking::force_resolver_break(Some(1));
    let outcome = owner_domain_walk_with_progress(
        epoch_request(&fixture, 1),
        &AtomicBool::new(false),
        |_| {},
    );
    crate::application::routed_campaign::walking::force_resolver_break(None);
    let document = outcome.unwrap().into_document().unwrap();
    assert_eq!(document["status"], "stopped", "{document}");
    assert_eq!(document["stop_reason"], "error_stop");
    assert!(document["failed_nodes"].as_u64().unwrap() >= 1);
    assert_eq!(
        document["failed_nodes"],
        document["epoch"]["ledger6"]["native_error"]
    );
    let errors: Vec<&Value> = document["domains"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["error"].is_string())
        .collect();
    assert!(!errors.is_empty());
    for row in errors {
        assert_eq!(row["epoch"]["class"], "C2", "{row}");
        assert_eq!(row["epoch"]["break_reason"], "resolver_range");
        assert_eq!(row["epoch"]["emitted_events"], 1);
        assert_eq!(row["accepted_events"], 0);
        assert_eq!(row["descendant_closed"], false);
    }
}

/// A9 (§16): helper roots and physics queries are reported separately, and
/// a physics query admitted as a hit on a helper is certified with (and only
/// through) the helper's root.
#[test]
fn a9_roots_reported_separately() {
    let fixture = Fixture::new();
    let directory = fixture.directory.join("epoch-a9");
    let mut request = epoch_request(&fixture, 1);
    request.matching.queries_json = json!({"schema":"rustred.owner-domain-queries.json.v2",
        "query_roles":{"required":["physics-narrow"],"auxiliary":["helper-ray"]},
        "queries":[
        {"id":"helper-ray","owner":"1","lower":[0],"upper":[null],"max_numerator_rank":11},
        {"id":"physics-narrow","owner":"1","lower":[2],"upper":[2],"max_numerator_rank":11}
    ]})
    .to_string();
    request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&directory));
    let result =
        owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert!(
        !result.all_scheduled_domains_resolved,
        "{}",
        result.document
    );
    assert_eq!(result.document["query_admission"]["required"], 1);
    assert_eq!(result.document["query_admission"]["auxiliary"], 1);
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(directory.join("latest.json")).unwrap()).unwrap();
    let input_file = manifest["manifest"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["key"] == "inputs")
        .unwrap()["file"]
        .as_str()
        .unwrap();
    let input_bytes = std::fs::read(directory.join(input_file)).unwrap();
    let inputs = serde_json::Deserializer::from_slice(&input_bytes)
        .into_iter::<Value>()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        inputs[1]["domain"], inputs[0]["domain"],
        "the physics query is admitted as a hit on the helper"
    );
    assert_eq!(inputs[0]["role"], "auxiliary");
    assert_eq!(inputs[1]["role"], "required");
    assert_eq!(inputs[0]["role_declared"], true);
    assert_eq!(inputs[1]["role_declared"], true);
    let mut options = OwnerDomainWalkVerifyOptions::new(&directory);
    options.require_closure = true;
    let report = crate::owner_domain_walk_verify_closure(
        &request,
        &options,
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(report["verdict"], "PASS", "{report}");
    assert_eq!(report["roots_total"], 1);
    let classes = &report["certification"]["classes"];
    assert_eq!(classes["helper"]["total"], 1);
    assert_eq!(classes["helper"]["admitting"], 1);
    assert_eq!(classes["helper"]["independently_verified"], 1);
    assert_eq!(classes["physics"]["total"], 1);
    assert_eq!(classes["physics"]["absorbed"], 1);
    assert_eq!(classes["physics"]["independently_verified"], 1);
}

/// The verifier's frontier mutations on an epoch export that HAS frontier
/// records (the record-policy frontier fixture): hiding a frontier and
/// sealing a frontier record must FAIL (FG has no frontier, so the FG
/// matrix could not exercise them).
#[test]
fn epoch_frontier_export_mutations_fail_the_verifier() {
    let fixture = noninvolutive_route_fixture();
    let directory = fixture.directory.join("epoch-frontier-export");
    let mut request = missing_route_epoch(&fixture);
    request.frontier_policy = crate::OwnerDomainWalkFrontierPolicy::Record;
    request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&directory));
    let result =
        owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!result.all_scheduled_domains_resolved);
    let verify = |mutation: Option<crate::OwnerDomainWalkVerifyMutation>| {
        let mut options = OwnerDomainWalkVerifyOptions::new(&directory);
        options.mutation = mutation;
        crate::owner_domain_walk_verify_closure(&request, &options, &AtomicBool::new(false), |_| {})
            .unwrap()
    };
    let clean = verify(None);
    assert_ne!(clean["verdict"], "FAIL", "{clean}");
    for mutation in [
        crate::OwnerDomainWalkVerifyMutation::HiddenFrontier,
        crate::OwnerDomainWalkVerifyMutation::SealWithFrontier,
    ] {
        let report = verify(Some(mutation));
        assert_eq!(report["verdict"], "FAIL", "{mutation:?}: {report}");
    }
}
