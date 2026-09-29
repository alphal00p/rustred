//! Public entry-pipeline integration of rescue amendments and G2 activation.
//! This reuses the missing-route family from the rescue tests. It establishes
//! binding, quarantine, resume and required-query behavior, without claiming
//! that this tiny fixture exercises a G2 loan; loan-specific regressions live
//! in walking/execution/g2_rescue_tests.rs.
use super::*;
use crate::{
    OWNER_DOMAIN_WALK_AMENDMENT_SCHEMA, OwnerDomainWalkAmendment, OwnerDomainWalkCheckpointOptions,
    OwnerDomainWalkG2ResidualAnchors, OwnerDomainWalkVerifyOptions,
    owner_domain_walk_verify_closure,
};
use std::cell::RefCell;
use std::num::NonZeroUsize;

fn request(fixture: &Fixture) -> OwnerDomainWalkRequest {
    let mut request = route_walk_request(fixture, 0);
    let mut selection: Value = serde_json::from_str(&request.matching.selection_json).unwrap();
    selection["initial_frontier_routes"] = json!([]);
    request.matching.selection_json = selection.to_string();
    let mut queries: Value = serde_json::from_str(&request.matching.queries_json).unwrap();
    queries["queries"].as_array_mut().unwrap().extend([
        json!({"id":"literal-apply", "owner":"011", "lower":[0,0,0],
            "upper":[0,2,2], "max_numerator_rank":1}),
        json!({"id":"second-missing-route", "owner":"101", "lower":[0,0,0],
            "upper":[null,null,null], "max_numerator_rank":0}),
    ]);
    queries["query_roles"] = json!({"required":["literal-apply"],
        "auxiliary":["routed-positive-ray","second-missing-route"]});
    request.matching.queries_json = queries.to_string();
    request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
        lookahead: NonZeroUsize::MIN,
    };
    request
}

fn walk(request: OwnerDomainWalkRequest) -> (Value, Vec<Value>) {
    let events = RefCell::new(Vec::new());
    let result = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |event| {
        events.borrow_mut().push(event)
    })
    .unwrap();
    (result.into_document().unwrap(), events.into_inner())
}

fn manifest(directory: &std::path::Path) -> Value {
    serde_json::from_slice(&std::fs::read(directory.join("latest.json")).unwrap()).unwrap()
}

fn amendment(parent: &str) -> OwnerDomainWalkAmendment {
    OwnerDomainWalkAmendment {
        path: "g2-rescue-amendment.json".into(),
        text: json!({"schema":OWNER_DOMAIN_WALK_AMENDMENT_SCHEMA,
            "sequence":1,"parent":parent,"queries":[
                {"id":"g2-rescue-ray","owner":"110","lower":[0,0,0],
                    "upper":[null,null,null],"max_numerator_rank":0},
                {"id":"g2-rescue-literal","owner":"011","lower":[0,0,0],
                    "upper":[0,2,2],"max_numerator_rank":1}],
            "provenance":{"generator":"combined-rescue-g2-test"}})
        .to_string(),
    }
}

fn assert_required_scope(document: &Value) {
    let certificate = &document["query_certification"];
    assert_eq!(certificate["required_queries_total"], 1, "{document}");
    assert_eq!(certificate["required_queries_certified"], 1, "{document}");
    assert_eq!(certificate["required_queries_uncovered"], json!([]));
    assert!(document["frontiers"].as_u64().unwrap() >= 2);
    assert_eq!(document["family_closure_claim"], false);
}

fn run_activation_order(g2_first: bool, activate_before_rescue: bool) {
    let fixture = noninvolutive_route_fixture();
    let directory = fixture.directory.join(if g2_first {
        "g2-before-rescue"
    } else {
        "rescue-before-g2"
    });
    let mut first = request(&fixture);
    first.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&directory));
    if g2_first {
        first.g2_residual_anchors = OwnerDomainWalkG2ResidualAnchors::Union;
    }
    let (initial, _) = walk(first.clone());
    assert!(initial["frontiers"].as_u64().unwrap() >= 2);
    if activate_before_rescue {
        assert!(!g2_first);
        let before = manifest(&directory);
        first.checkpoint.as_mut().unwrap().resume = true;
        first.g2_residual_anchors = OwnerDomainWalkG2ResidualAnchors::Union;
        first.g2_activate_on_resume = true;
        let (activated, events) = walk(first.clone());
        assert!(events.iter().any(|event| event["event"] == "g2_activated"));
        assert_eq!(activated["committed_domains"], initial["committed_domains"]);
        let after = manifest(&directory);
        assert!(after["generation"].as_u64().unwrap() > before["generation"].as_u64().unwrap());
        assert_ne!(after["request"], before["request"]);
        assert!(after["sections"]["anchors"].is_object());
        let activation = &after["metadata"]["g2_activation"];
        assert_eq!(activation["from"], "off");
        assert_eq!(activation["to"], "union");
        assert_eq!(activation["binding_before"], before["request"]);
        assert_eq!(activation["binding_after"], after["request"]);
        assert_eq!(activated["checkpoint"]["g2_activation"], *activation);
        first.g2_activate_on_resume = false;
        // Prove the durable zero-work activation can resume normally before
        // any amendment (whose own dirty flag would otherwise hide the bug).
        let (ordinary, events) = walk(first.clone());
        assert_eq!(ordinary["committed_domains"], initial["committed_domains"]);
        assert_eq!(ordinary["checkpoint"]["g2_activation"], *activation);
        assert!(events.iter().all(|event| event["event"] != "g2_activated"));
        assert_eq!(manifest(&directory)["generation"], after["generation"]);
    }
    let original_binding = manifest(&directory)["request"].as_str().unwrap().to_owned();
    let amendment = amendment(&original_binding);
    let mut resume = first;
    resume.checkpoint.as_mut().unwrap().resume = true;
    resume.amendments = vec![amendment.clone()];
    let (amended, events) = walk(resume.clone());
    assert_required_scope(&amended);
    assert!(
        events
            .iter()
            .any(|event| event["event"] == "rescue_quarantine")
    );
    let applied = events
        .iter()
        .find(|event| event["event"] == "rescue_amendment_applied")
        .unwrap();
    assert_eq!(applied["admitted_new_domains"], 1);
    assert_eq!(applied["resolved_to_existing_domains"], 1);
    assert_eq!(amended["amendments"][0]["digest"], amendment.digest());
    assert_eq!(amended["amendments"][0]["parent"], original_binding);

    let active = if g2_first || activate_before_rescue {
        amended.clone()
    } else {
        // The original amendment bytes and chain root MUST survive the G2
        // request-binding activation; neither is rewritten or re-signed.
        resume.g2_residual_anchors = OwnerDomainWalkG2ResidualAnchors::Union;
        resume.g2_activate_on_resume = true;
        let (activated, events) = walk(resume.clone());
        assert!(events.iter().any(|event| event["event"] == "g2_activated"));
        assert!(
            events
                .iter()
                .any(|event| event["event"] == "rescue_quarantine")
        );
        assert!(
            events
                .iter()
                .all(|event| event["event"] != "rescue_amendment_applied")
        );
        assert_required_scope(&activated);
        assert_eq!(activated["checkpoint"]["g2_activation"]["from"], "off");
        assert_eq!(activated["amendments"], amended["amendments"]);
        resume.g2_activate_on_resume = false;
        activated
    };
    let (again, events) = walk(resume.clone());
    assert_required_scope(&again);
    assert!(
        events
            .iter()
            .any(|event| event["event"] == "rescue_quarantine")
    );
    assert!(
        events
            .iter()
            .all(|event| event["event"] != "rescue_amendment_applied")
    );
    assert_eq!(again["amendments"], active["amendments"]);
    assert_eq!(again["inputs"], active["inputs"]);
    assert_eq!(again["scheduled_nodes"], active["scheduled_nodes"]);
    let mut verify = OwnerDomainWalkVerifyOptions::new(&directory);
    verify.require_closure = true;
    let report =
        owner_domain_walk_verify_closure(&resume, &verify, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(report["verdict"], "PASS", "{report}");
    assert_eq!(report["physics_queries"]["certified"], 1);
    // Report observed use; do not turn an activation test into an unsupported
    // claim that its tiny native family necessarily exercises G2 lending.
    eprintln!(
        "{}",
        json!({"g2_first":g2_first,"activate_before_rescue":activate_before_rescue,
        "observed_g2_records":again["g2_residual_anchors"]["logged_g2_records"],
        "scope":"actual amendment/activation/resume pipeline; loan-specific coverage is separate"})
    );
}

#[test]
fn g2_enabled_walk_accepts_and_replays_an_immutable_rescue_amendment() {
    run_activation_order(true, false);
}

#[test]
fn rescued_walk_activates_g2_without_rewriting_its_amendment_chain() {
    run_activation_order(false, false);
}

#[test]
fn newly_amended_g2_activated_walk_binds_its_new_chain_to_the_current_request() {
    run_activation_order(false, true);
}
