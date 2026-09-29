//! Resume-time frontier rescue (`walking/rescue.rs`): digest-chained
//! amendments, the frontier-taint quarantine and per-query certification on
//! a real walk with frontiers (the missing-route fixture of the A10 tests).
use super::*;
use crate::{
    OWNER_DOMAIN_WALK_AMENDMENT_SCHEMA, OwnerDomainWalkAmendment, OwnerDomainWalkCheckpointOptions,
    OwnerDomainWalkRescuePlanOptions, OwnerDomainWalkVerifyOptions, OwnerDomainWalkVerifyScope,
    owner_domain_walk_rescue_plan, owner_domain_walk_verify_closure,
};
use std::cell::RefCell;

/// The owner-110 route ray (a `missing_route_cover` frontier), a literal
/// owner-011 Apply box (closes) and an owner-101 ray (another frontier).
fn request(fixture: &Fixture) -> OwnerDomainWalkRequest {
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
    let auxiliary: Vec<_> = rows
        .iter()
        .filter(|row| row["id"] != "literal-apply")
        .map(|row| row["id"].clone())
        .collect();
    queries["query_roles"] = json!({"required":["literal-apply"],"auxiliary":auxiliary});
    request.matching.queries_json = queries.to_string();
    request
}

fn walk(request: OwnerDomainWalkRequest) -> Result<(Value, Vec<Value>), String> {
    let events = RefCell::new(Vec::new());
    let result = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |event| {
        events.borrow_mut().push(event)
    })
    .map_err(|e| e.to_string())?;
    Ok((result.into_document().unwrap(), events.into_inner()))
}

fn manifest(directory: &std::path::Path) -> Value {
    serde_json::from_slice(&std::fs::read(directory.join("latest.json")).unwrap()).unwrap()
}

/// One valid amended row (a rewritten or foreign amendment needs a row: an
/// empty amendment is refused by the row parser before the chain check).
fn other_row() -> Value {
    json!([{"id":"route-rescue-other","owner":"011","lower":[0,0,0],"upper":[0,1,0],"max_numerator_rank":0}])
}

fn amendment(sequence: u64, parent: &str, rows: Value) -> OwnerDomainWalkAmendment {
    OwnerDomainWalkAmendment {
        path: format!("amendment-{sequence}.json").into(),
        text: json!({"schema":OWNER_DOMAIN_WALK_AMENDMENT_SCHEMA,"sequence":sequence,
            "parent":parent,"queries":rows,"provenance":{"generator":"test"}})
        .to_string(),
    }
}

#[test]
fn amended_resume_quarantines_the_frontier_taint_and_certifies_per_query() {
    let fixture = noninvolutive_route_fixture();
    let directory = fixture.directory.join("rescue");
    let mut first = request(&fixture);
    first.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&directory));
    let (plain, _) = walk(first.clone()).unwrap();
    let frontiers = plain["frontiers"].as_u64().unwrap();
    assert!(frontiers >= 2, "{plain}");
    // An unamended walk: no rescue key anywhere, historical manifest shape.
    for key in [
        "amendments",
        "query_certification",
        "rescue_quarantined_domains",
    ] {
        assert!(plain.get(key).is_none(), "{key}");
    }
    let saved = manifest(&directory);
    assert!(saved.get("amendments").is_none(), "{saved}");
    let request_digest = saved["request"].as_str().unwrap().to_owned();
    let domains_before = plain["scheduled_nodes"].as_u64().unwrap();

    // An amendment needs --resume.
    let mut fresh = request(&fixture);
    fresh.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(
        fixture.directory.join("fresh"),
    ));
    fresh.amendments = vec![amendment(1, &request_digest, other_row())];
    assert!(walk(fresh).unwrap_err().contains("require --resume"));

    // Amendment 1: an exact copy of the frontier-bearing owner-110 ray (its
    // original ID is quarantined, so the copy gets a fresh ID) and a copy of
    // the literal box (resolves to the existing, untainted, closed record).
    let rows = json!([
        {"id":"route-rescue-ray","owner":"110","lower":[0,0,0],"upper":[null,null,null],"max_numerator_rank":0},
        {"id":"route-rescue-literal","owner":"011","lower":[0,0,0],"upper":[0,2,2],"max_numerator_rank":1}]);
    let first_amendment = amendment(1, &request_digest, rows);
    let mut resume = first.clone();
    resume.checkpoint.as_mut().unwrap().resume = true;
    // Wrong parent, wrong sequence: refused before anything runs.
    let mut wrong = resume.clone();
    wrong.amendments = vec![amendment(1, &"0".repeat(64), other_row())];
    assert!(walk(wrong).unwrap_err().contains("request binding"));
    let mut wrong = resume.clone();
    wrong.amendments = vec![amendment(2, &request_digest, other_row())];
    assert!(walk(wrong).unwrap_err().contains("sequence"));
    assert_eq!(
        manifest(&directory),
        saved,
        "a refused resume writes nothing"
    );

    resume.amendments = vec![first_amendment.clone()];
    let (amended, events) = walk(resume.clone()).unwrap();
    let quarantine = events
        .iter()
        .find(|e| e["event"] == "rescue_quarantine")
        .expect("quarantine receipt");
    assert!(quarantine["quarantined_domains"].as_u64().unwrap() >= 2);
    let applied = events
        .iter()
        .find(|e| e["event"] == "rescue_amendment_applied")
        .expect("amendment receipt");
    assert_eq!(applied["admitted_new_domains"], 1);
    assert_eq!(applied["resolved_to_existing_domains"], 1);
    assert_eq!(applied["digest"], first_amendment.digest());
    assert_eq!(amended["amendments"][0]["digest"], first_amendment.digest());
    assert_eq!(amended["amendments"][0]["parent"], request_digest.as_str());
    assert!(amended["scheduled_nodes"].as_u64().unwrap() > domains_before);
    let inputs = amended["inputs"].as_array().unwrap();
    assert_eq!(inputs.len(), 5);
    assert_eq!(inputs[3]["amendment"], 1);
    assert_eq!(
        inputs[4]["domain"], inputs[1]["domain"],
        "existing closed record"
    );
    assert_ne!(
        inputs[3]["domain"], inputs[0]["domain"],
        "fresh copy of the tainted ray"
    );
    let certification = &amended["query_certification"];
    let certified = |id: &str| {
        certification["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == id)
            .unwrap()["certified_via_root"]
            .clone()
    };
    assert_eq!(certified("literal-apply"), inputs[1]["domain"]);
    assert_eq!(certified("route-rescue-literal"), inputs[1]["domain"]);
    assert!(certified("routed-positive-ray").is_null());
    assert_eq!(certification["queries_total"], 5);
    let saved = manifest(&directory);
    assert_eq!(saved["amendments"].as_array().unwrap().len(), 1);

    // Resume again: the chain must be re-supplied; nothing is re-applied,
    // and the restore accepts the exact duplicate of the quarantined ray.
    let mut omitted = resume.clone();
    omitted.amendments.clear();
    assert!(
        walk(omitted)
            .unwrap_err()
            .contains("must be supplied again")
    );
    let mut rewritten = resume.clone();
    rewritten.amendments = vec![amendment(1, &request_digest, other_row())];
    assert!(walk(rewritten).unwrap_err().contains("append-only"));
    let (again, events) = walk(resume.clone()).unwrap();
    assert!(events.iter().any(|e| e["event"] == "rescue_quarantine"));
    assert!(
        !events
            .iter()
            .any(|e| e["event"] == "rescue_amendment_applied")
    );
    assert_eq!(again["scheduled_nodes"], amended["scheduled_nodes"]);
    assert_eq!(again["inputs"], amended["inputs"]);

    // A second amendment chains from the first file's digest.
    let second = amendment(
        2,
        &first_amendment.digest(),
        json!([{"id":"route-rescue-second","owner":"011","lower":[0,0,0],"upper":[0,1,1],"max_numerator_rank":0}]),
    );
    let mut chained = resume.clone();
    chained.amendments = vec![first_amendment.clone(), second.clone()];
    let (twice, _) = walk(chained.clone()).unwrap();
    assert_eq!(twice["amendments"].as_array().unwrap().len(), 2);
    assert_eq!(twice["amendments"][1]["parent"], first_amendment.digest());

    // Only the immutable required query must certify. Names cannot change
    // this scope; modifying the declaration breaks the request binding.
    let mut verify = OwnerDomainWalkVerifyOptions::new(&directory);
    verify.require_closure = true;
    let report =
        owner_domain_walk_verify_closure(&chained, &verify, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(report["verdict"], "PASS", "{report}");
    assert_eq!(
        report["certification_scope"],
        "physics_queries_through_closed_containing_roots"
    );
    assert_eq!(
        report["roots_total"],
        report["roots_independently_verified"]
    );
    assert_eq!(report["physics_queries"]["certified"], 1);
    assert!(report["helper_roots"]["total"].as_u64().unwrap() >= 3);
    let mut relabelled = chained.clone();
    let mut changed: Value = serde_json::from_str(&relabelled.matching.queries_json).unwrap();
    changed["query_roles"]["auxiliary"]
        .as_array_mut()
        .unwrap()
        .push(json!("literal-apply"));
    changed["query_roles"]["required"] = json!([]);
    relabelled.matching.queries_json = changed.to_string();
    let report =
        owner_domain_walk_verify_closure(&relabelled, &verify, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(report["verdict"], "FAIL", "{report}");
    // The verifier refuses a command whose chain differs from the checkpoint.
    let report =
        owner_domain_walk_verify_closure(&resume, &verify, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(report["verdict"], "FAIL", "{report}");
    assert!(
        report["violations_by_class"]["amendment_chain"]
            .as_u64()
            .unwrap()
            >= 1
    );
    // All-roots scope keeps the historical gate: the tainted rays fail it.
    verify.certification_scope = OwnerDomainWalkVerifyScope::AllRoots;
    let report =
        owner_domain_walk_verify_closure(&chained, &verify, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(report["verdict"], "FAIL", "{report}");

    // The planner: missing-route frontiers have no known rescue.
    let mut options = OwnerDomainWalkRescuePlanOptions::new(&directory);
    options.helper_id_prefix = "route".into();
    let plan = owner_domain_walk_rescue_plan(&chained, &options).unwrap();
    assert_eq!(
        plan.plan["verdict"], "unknown_frontier_class",
        "{}",
        plan.plan
    );
    assert!(plan.amendment.is_none());
    assert_eq!(plan.plan["family_closure_claim"], false);
}

/// Under the A10 stop policy: the first stop is rescued, later stops resume
/// with the same chain; obligations no live root reaches are abandoned
/// (well-formed, never sealed) and the verifier certifies the physics query.
#[test]
fn stop_policy_rescue_abandons_dead_cones_and_verifies() {
    let fixture = noninvolutive_route_fixture();
    for (workers, ready) in [(1, false), (2, false), (3, true)] {
        if !crate::test_gates::workers_or_skip("frontier rescue", workers) {
            continue;
        }
        stop_policy_rescue(&fixture, workers, ready);
    }
}

fn stop_policy_rescue(fixture: &Fixture, workers: usize, ready: bool) {
    let directory = fixture
        .directory
        .join(format!("rescue-stop-{workers}-{ready}"));
    let mut first = request(fixture);
    first.workers = workers;
    if ready {
        first.publication_policy = OwnerDomainWalkPublicationPolicy::Ready;
        first.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
            lookahead: std::num::NonZeroUsize::new(2).unwrap(),
        };
    }
    first.frontier_policy = crate::OwnerDomainWalkFrontierPolicy::Stop;
    first.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&directory));
    let (paused, _) = walk(first.clone()).unwrap();
    assert_eq!(paused["status"], "paused", "{paused}");
    let request_digest = manifest(&directory)["request"].as_str().unwrap().to_owned();
    let rows = json!([{"id":"route-rescue-literal","owner":"011","lower":[0,0,0],"upper":[0,2,2],
        "max_numerator_rank":1}]);
    let mut resume = first.clone();
    resume.checkpoint.as_mut().unwrap().resume = true;
    resume.amendments = vec![amendment(1, &request_digest, rows)];
    let mut sessions = 0;
    let mut reported = 0u64;
    let last = loop {
        sessions += 1;
        assert!(sessions <= 12, "the rescued walk must drain");
        let (document, events) = walk(resume.clone()).unwrap();
        let quarantine = events
            .iter()
            .find(|e| e["event"] == "rescue_quarantine")
            .expect("every amended resume quarantines");
        for key in [
            "dead_pending_domains",
            "abandoned_domains",
            "live_input_roots",
        ] {
            assert!(quarantine[key].is_u64(), "{key}: {quarantine}");
        }
        reported += document["rescue_abandoned_domains_this_session"]
            .as_u64()
            .unwrap();
        if document["status"] != "paused" {
            break document;
        }
        assert_eq!(document["stop_reason"], "frontier_policy");
    };
    assert_eq!(last["recursive_worklist_exhausted"], true, "{last}");
    let abandoned: Vec<&Value> = last["domains"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|record| record["rescue_abandoned"] == true)
        .collect();
    assert_eq!(abandoned.len() as u64, reported);
    for record in &abandoned {
        assert_eq!(record["frontiers"].as_array().unwrap().len(), 1);
        assert_eq!(record["frontiers"][0]["kind"], "rescue_abandoned_dead_cone");
        assert_eq!(record["local_inspection_finished"], false);
        assert_eq!(record["descendant_closed"], false);
    }
    let mut verify = OwnerDomainWalkVerifyOptions::new(&directory);
    verify.require_closure = true;
    let report =
        owner_domain_walk_verify_closure(&resume, &verify, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(report["verdict"], "PASS", "{report}");
    assert_eq!(report["physics_queries"]["certified"], 1);
    assert_eq!(
        report["roots_total"],
        report["roots_independently_verified"]
    );
}
