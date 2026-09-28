//! A10 frontier policy: Record is the unbound historical default; Stop saves
//! the checkpoint and stops at the first frontier a session commits.
use super::*;
use crate::{OwnerDomainWalkCheckpointOptions, OwnerDomainWalkFrontierPolicy};
use std::cell::RefCell;

/// Three inputs with no installed initial route: the owner-110 route ray
/// (its Route inspection reports a `missing_route_cover` frontier), a literal
/// owner-011 Apply box and an owner-101 route ray (another missing route), so
/// work remains after the first frontier.
fn missing_route_request(fixture: &Fixture, workers: usize) -> OwnerDomainWalkRequest {
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
    request.workers = workers;
    request
}

fn walk(request: OwnerDomainWalkRequest) -> (OwnerDomainWalkResult, Vec<Value>) {
    let events = RefCell::new(Vec::new());
    let result = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |event| {
        events.borrow_mut().push(event)
    })
    .unwrap();
    (result, events.into_inner())
}

fn manifest(directory: &std::path::Path) -> Value {
    serde_json::from_slice(&std::fs::read(directory.join("latest.json")).unwrap()).unwrap()
}

fn without_seconds(mut value: Value) -> Value {
    fn strip(value: &mut Value) {
        match value {
            Value::Object(o) => {
                o.remove("seconds");
                o.remove("physical_seconds_sum");
                o.remove("accepted_events");
                for v in o.values_mut() {
                    strip(v);
                }
            }
            Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {}
        }
    }
    strip(&mut value);
    value
}

fn workers() -> impl Iterator<Item = usize> {
    [1, 2]
        .into_iter()
        .filter(|&workers| crate::test_gates::workers_or_skip("frontier policy", workers))
}

/// Input frontiers and the cumulative frontier count after each record of an
/// Ordered record walk (record `i` committed means `i + 1` committed domains).
fn frontier_prefix(document: &Value) -> (u64, Vec<u64>) {
    let input = document["input_frontiers"]
        .as_array()
        .map_or(0, |inputs| inputs.len() as u64);
    let mut total = input;
    let cumulative = document["domains"]
        .as_array()
        .expect("record walk document lists its records")
        .iter()
        .map(|record| {
            total += record["frontiers"].as_array().map_or_else(
                || record["frontiers"].as_u64().unwrap_or(0),
                |frontiers| frontiers.len() as u64,
            );
            total
        })
        .collect();
    (input, cumulative)
}

/// A10 pin against a "late stop": a Stop session that started at `start`
/// committed frontiers stops at the first checkpoint opportunity after the
/// next frontier record of the Ordered record walk. Its frontier count lies
/// within that record (a chunked publication may stop inside it) and it
/// committed at most that record; one more commit fails.
fn assert_stops_at_the_next_frontier_record(
    document: &Value,
    start: u64,
    prefix: &(u64, Vec<u64>),
) {
    let (input, cumulative) = prefix;
    let frontiers = document["frontiers"].as_u64().unwrap();
    let committed = document["committed_domains"].as_u64().unwrap();
    if *input > start {
        assert_eq!((committed, frontiers), (0, *input), "{document}");
        return;
    }
    let next = cumulative
        .iter()
        .position(|&total| total > start)
        .expect("a paused Stop session has a new frontier record");
    let before = if next == 0 {
        *input
    } else {
        cumulative[next - 1]
    };
    assert!(
        before < frontiers && frontiers <= cumulative[next],
        "frontiers {frontiers} outside record {next} ({before}, {}]: {document}",
        cumulative[next]
    );
    assert!(
        committed == next as u64 || committed == next as u64 + 1,
        "late stop: {committed} committed domains, next frontier record {next}: {document}"
    );
}

#[test]
fn frontier_stop_pauses_at_each_new_frontier_and_resumes_to_the_record_result() {
    let fixture = noninvolutive_route_fixture();
    for workers in workers() {
        let record = missing_route_request(&fixture, workers);
        let (recorded, events) = walk(record.clone());
        let total = recorded.document["frontiers"].as_u64().unwrap();
        assert!(total > 0, "{}", recorded.document);
        let prefix = frontier_prefix(&recorded.document);
        assert_eq!(prefix.1.last().copied(), Some(total));
        // Record (the default) adds no key anywhere.
        for document in std::iter::once(&recorded.document).chain(&events) {
            assert!(document.get("frontier_policy").is_none(), "{document}");
            assert!(document.get("stop_reason").is_none(), "{document}");
        }
        assert!(events.iter().all(|event| event["event"] != "frontier_stop"));

        let directory = fixture.directory.join(format!("frontier-stop-{workers}"));
        let mut stop = record.clone();
        stop.frontier_policy = OwnerDomainWalkFrontierPolicy::Stop;
        stop.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&directory));
        let mut session_start = 0;
        let mut stops = 0;
        let last = loop {
            let (result, events) = walk(stop.clone());
            assert_eq!(result.document["frontier_policy"], "stop");
            let admitted = events.iter().find(|e| e["event"] == "admitted").unwrap();
            assert_eq!(admitted["frontier_policy"], "stop");
            let fired = events.iter().find(|e| e["event"] == "frontier_stop");
            let frontiers = result.document["frontiers"].as_u64().unwrap();
            if result.document["status"] != "paused" {
                assert!(fired.is_none() || result.document["stop_reason"] == "frontier_policy");
                break result;
            }
            stops += 1;
            let fired = fired.expect("a paused Stop session journals its frontier stop");
            assert_eq!(fired["stop_reason"], "frontier_policy");
            assert_eq!(fired["session_start_frontiers"], session_start);
            assert!(fired["frontiers"].as_u64().unwrap() > session_start);
            assert_eq!(result.document["stop_reason"], "frontier_policy");
            assert_eq!(result.document["resume_supported"], true);
            assert!(!result.all_scheduled_domains_resolved);
            assert!(frontiers > session_start && frontiers <= total);
            // Both the state the stop saved (its journal event) and the
            // session's final paused state (a serial heartbeat stop may
            // finish the frontier record's inspection first).
            assert_stops_at_the_next_frontier_record(fired, session_start, &prefix);
            assert_stops_at_the_next_frontier_record(&result.document, session_start, &prefix);
            let finished = events.iter().rfind(|e| e["event"] == "finished").unwrap();
            assert_eq!(finished["stop_reason"], "frontier_policy");
            assert_eq!(finished["status"], "paused");
            let saved = manifest(&directory);
            assert_eq!(saved["metadata"]["paused"], true, "{saved}");
            assert_eq!(saved["metadata"]["stop_reason"], "frontier_policy");
            assert!(stops as u64 <= total, "one stop per new frontier at most");
            session_start = frontiers;
            stop.checkpoint.as_mut().unwrap().resume = true;
        };
        assert!(stops >= 1, "the first frontier must stop a fresh walk");
        assert_eq!(last.document["frontiers"].as_u64().unwrap(), total);
        assert_eq!(
            last.document["recursive_worklist_exhausted"],
            recorded.document["recursive_worklist_exhausted"]
        );
        // A resumed session does not inherit the stop label.
        let saved = manifest(&directory);
        if last.document.get("stop_reason").is_none() {
            assert!(saved["metadata"].get("stop_reason").is_none(), "{saved}");
        }
        let document = last.into_document().unwrap();
        assert_eq!(
            without_seconds(document["domains"].clone()),
            without_seconds(recorded.document["domains"].clone()),
            "{workers} workers: stop/resume sessions must reach the record walk"
        );
        for field in ["scheduled_nodes", "completed_nodes", "successors", "events"] {
            assert_eq!(document[field], recorded.document[field], "{field}");
        }
    }
}

/// Ready publication: identities may depend on scheduling, so the check is
/// the stop protocol itself plus the final frontier and completion counts.
#[test]
fn ready_frontier_stop_pauses_and_resumes_to_exhaustion() {
    let fixture = noninvolutive_route_fixture();
    for workers in workers().filter(|&workers| workers > 1) {
        let mut record = missing_route_request(&fixture, workers);
        record.publication_policy = OwnerDomainWalkPublicationPolicy::Ready;
        record.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
            lookahead: std::num::NonZeroUsize::new(2).unwrap(),
        };
        let (recorded, _) = walk(record.clone());
        let total = recorded.document["frontiers"].as_u64().unwrap();
        assert!(total > 0, "{}", recorded.document);
        let directory = fixture
            .directory
            .join(format!("ready-frontier-stop-{workers}"));
        let mut stop = record;
        stop.frontier_policy = OwnerDomainWalkFrontierPolicy::Stop;
        stop.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&directory));
        let mut stops = 0;
        let last = loop {
            let (result, events) = walk(stop.clone());
            if result.document["status"] != "paused" {
                break result;
            }
            stops += 1;
            assert!(stops as u64 <= total);
            assert_eq!(result.document["stop_reason"], "frontier_policy");
            assert!(events.iter().any(|e| e["event"] == "frontier_stop"));
            assert_eq!(
                manifest(&directory)["metadata"]["stop_reason"],
                "frontier_policy"
            );
            stop.checkpoint.as_mut().unwrap().resume = true;
        };
        assert!(stops >= 1);
        assert_eq!(last.document["frontiers"].as_u64().unwrap(), total);
        assert_eq!(last.document["recursive_worklist_exhausted"], true);
        assert_eq!(
            last.document["recursive_worklist_exhausted"],
            recorded.document["recursive_worklist_exhausted"]
        );
    }
}

#[test]
fn frontier_stop_is_bound_into_the_checkpoint_request() {
    let fixture = noninvolutive_route_fixture();
    let directory = fixture.directory.join("frontier-binding");
    let mut stop = missing_route_request(&fixture, 1);
    stop.frontier_policy = OwnerDomainWalkFrontierPolicy::Stop;
    stop.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&directory));
    let (paused, _) = walk(stop.clone());
    assert_eq!(paused.document["status"], "paused");
    let mut record = stop.clone();
    record.frontier_policy = OwnerDomainWalkFrontierPolicy::Record;
    record.checkpoint.as_mut().unwrap().resume = true;
    let error = owner_domain_walk_with_progress(record, &AtomicBool::new(false), |_| {})
        .unwrap_err()
        .to_string();
    assert!(error.contains("request or policy differs"), "{error}");
    stop.checkpoint.as_mut().unwrap().resume = true;
    walk(stop);
}

#[test]
fn frontier_stop_fires_on_initial_input_frontiers_before_any_inspection() {
    let fixture = noninvolutive_route_fixture_with_scale(true);
    let mut request = route_walk_request(&fixture, 11);
    request.matching.queries_json = json!({
        "schema":"rustred.owner-domain-queries.json.v2", "queries":[{
            "id":"bounded-route", "owner":"110", "lower":[2,3,0],
            "upper":[4,5,0], "max_numerator_rank":11
        }]
    })
    .to_string();
    let mut selection: Value = serde_json::from_str(&request.matching.selection_json).unwrap();
    selection["initial_frontier_routes"] = json!([]);
    request.matching.selection_json = selection.to_string();
    let directory = fixture.directory.join("frontier-input");
    request.frontier_policy = OwnerDomainWalkFrontierPolicy::Stop;
    request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&directory));
    let (result, events) = walk(request.clone());
    let fired = events
        .iter()
        .find(|e| e["event"] == "frontier_stop")
        .expect("initial input frontier stops the walk");
    assert_eq!(fired["session_start_frontiers"], 0);
    assert_eq!(fired["input_frontiers"], 1);
    assert_eq!(fired["completed_native_inspections"], 0);
    assert_eq!(result.document["stop_reason"], "frontier_policy");
    assert_eq!(result.document["completed_nodes"], 0);
    assert!(!result.all_scheduled_domains_resolved);
    // Like every frontier stop: a paused, resumable session (CLI exit 4),
    // even though the run loop never started.
    assert_eq!(result.document["status"], "paused", "{}", result.document);
    assert_eq!(result.document["resume_supported"], true);
    let saved = manifest(&directory);
    assert_eq!(saved["metadata"]["stop_reason"], "frontier_policy");
    assert_eq!(saved["metadata"]["paused"], true, "{saved}");
    // Resuming continues past the restored frontier to the walk's end.
    request.checkpoint.as_mut().unwrap().resume = true;
    let (resumed, events) = walk(request);
    assert!(events.iter().all(|e| e["event"] != "frontier_stop"));
    assert_ne!(resumed.document["status"], "paused", "{}", resumed.document);
    assert!(resumed.document.get("stop_reason").is_none());
    assert_eq!(resumed.document["frontiers"], 1);
    assert_eq!(resumed.document["recursive_worklist_exhausted"], true);
}

/// The only frontier of the walk comes with its last commit: the run loop
/// ends without observing the stop's cancellation, and the session must
/// still report a paused, resumable stop (not a finished incomplete walk).
#[test]
fn frontier_stop_on_the_last_commit_reports_a_paused_session() {
    let fixture = noninvolutive_route_fixture();
    for workers in workers() {
        let mut record = route_walk_request(&fixture, 0);
        let mut selection: Value = serde_json::from_str(&record.matching.selection_json).unwrap();
        selection["initial_frontier_routes"] = json!([]);
        record.matching.selection_json = selection.to_string();
        record.workers = workers;
        let (recorded, _) = walk(record.clone());
        let (input, cumulative) = frontier_prefix(&recorded.document);
        let total = recorded.document["frontiers"].as_u64().unwrap();
        // Precondition of this case: no input frontier, and the last record
        // is the walk's only frontier record.
        assert_eq!(input, 0, "{}", recorded.document);
        assert!(total > 0, "{}", recorded.document);
        let first = cumulative.iter().position(|&count| count > 0).unwrap();
        assert_eq!(first, cumulative.len() - 1, "{}", recorded.document);

        let directory = fixture.directory.join(format!("frontier-last-{workers}"));
        let mut stop = record.clone();
        stop.frontier_policy = OwnerDomainWalkFrontierPolicy::Stop;
        stop.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&directory));
        let (paused, events) = walk(stop.clone());
        assert!(events.iter().any(|e| e["event"] == "frontier_stop"));
        assert_eq!(paused.document["status"], "paused", "{}", paused.document);
        assert_eq!(paused.document["resume_supported"], true);
        assert_eq!(paused.document["stop_reason"], "frontier_policy");
        assert_stops_at_the_next_frontier_record(&paused.document, 0, &(input, cumulative));
        let saved = manifest(&directory);
        assert_eq!(saved["metadata"]["paused"], true, "{saved}");
        assert_eq!(saved["metadata"]["stop_reason"], "frontier_policy");

        stop.checkpoint.as_mut().unwrap().resume = true;
        let (last, events) = walk(stop);
        assert!(events.iter().all(|e| e["event"] != "frontier_stop"));
        assert_eq!(last.document["status"], recorded.document["status"]);
        assert!(last.document.get("stop_reason").is_none());
        let document = last.into_document().unwrap();
        assert_eq!(
            without_seconds(document["domains"].clone()),
            without_seconds(recorded.document["domains"].clone())
        );
        assert_eq!(document["frontiers"], total);
    }
}

#[test]
fn frontier_stop_requires_a_checkpoint() {
    let fixture = noninvolutive_route_fixture();
    let mut request = missing_route_request(&fixture, 1);
    request.frontier_policy = OwnerDomainWalkFrontierPolicy::Stop;
    let error = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {})
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("frontier stop requires a checkpointed"),
        "{error}"
    );
}
