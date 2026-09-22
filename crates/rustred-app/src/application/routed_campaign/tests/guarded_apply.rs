use super::*;

fn request(fixture: &Fixture) -> OwnerGuardedApplyRequest {
    // Discover an actual saved selector via the existing public local matcher;
    // the guarded API still rebinds it internally to the loaded immutable owner.
    let matching =
        owner_domain_match_with_progress(match_request(fixture), &AtomicBool::new(false), |_| {})
            .unwrap();
    let selected = matching.document["queries"][0]["pieces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["disposition"]["kind"] == "selected_rule")
        .unwrap();
    let queries = json!({"schema":"rustred.owner-guarded-rule-queries.json.v1","queries":[{
        "id":"own-rule","owner":"1","lower":[1],"upper":[null],"max_numerator_rank":11,
        "batch":selected["disposition"]["batch"],"rule":selected["disposition"]["rule"]}]});
    let mut r =
        OwnerGuardedApplyRequest::new(fixture.request.selection_json.clone(), queries.to_string());
    r.owner_base = fixture.directory.clone();
    r
}

#[test]
fn guarded_apply_saved_rule_reports_complement_and_one_definition_without_closure() {
    let fixture = Fixture::new();
    let r = request(&fixture);
    let before = std::fs::read(fixture.directory.join("owner.rrbin")).unwrap();
    let progress = std::cell::RefCell::new(Vec::new());
    let result = owner_guarded_apply_with_progress(r, &AtomicBool::new(false), |e| {
        progress.borrow_mut().push(e)
    })
    .unwrap();
    assert!(result.diagnostic_complete, "{}", result.document);
    assert!(
        serde_json::to_vec(&result.document).unwrap().len()
            <= result.document["report_payload_bytes_charged"]
                .as_u64()
                .unwrap() as usize
    );
    assert!(
        result.document["report_payload_bytes_charged"]
            .as_u64()
            .unwrap()
            <= result.document["max_report_bytes"].as_u64().unwrap()
    );
    assert_eq!(result.document["completed_queries"], 1);
    let q = &result.document["queries"][0];
    assert_eq!(q["requested_max_numerator_rank"], 11);
    assert_eq!(q["guarded_domain"]["max_numerator_rank"], 11);
    assert!(
        !q["guarded_domain"]["original_denominators"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let events = q["events"].as_array().unwrap();
    assert!(events.iter().any(|e| e["residual"] == "IncomingComplement"));
    assert!(events.iter().any(|e| e["kind"] == "guarded_rule_finished"));
    for e in events {
        assert!(e.get("equalities_zero").is_none());
        assert_eq!(e["domain_ref"], "own-rule");
    }
    for flag in [
        "family_closure_claim",
        "first_priority_dispatch_claim",
        "integer_feasibility_claim",
        "work_queue_insertion",
    ] {
        assert_eq!(result.document[flag], false);
    }
    let completed = progress.borrow().last().unwrap().clone();
    assert_eq!(completed["diagnostic_complete"], true);
    assert!(completed.get("queries").is_none());
    assert!(completed.get("events").is_none());
    assert_eq!(
        before,
        std::fs::read(fixture.directory.join("owner.rrbin")).unwrap()
    );
}

#[test]
fn guarded_apply_event_and_byte_caps_retain_truthful_incomplete_prefix() {
    let fixture = Fixture::new();
    let mut r = request(&fixture);
    r.max_report_events = 1;
    let result = owner_guarded_apply_with_progress(r, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!result.diagnostic_complete);
    assert_eq!(result.document["retained_events"], 1);
    assert_eq!(result.document["queries"][0]["inspection_finished"], false);
    assert_eq!(
        result.document["queries"][0]["native_error_kind"],
        "stopped_by_consumer"
    );
    assert!(
        result.document["queries"][0]["stats"]["events"]
            .as_u64()
            .unwrap()
            > 1
    );
    let mut r = request(&fixture);
    r.max_report_bytes = 131_072;
    let result = owner_guarded_apply_with_progress(r, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!result.diagnostic_complete);
    assert_eq!(result.document["processed_queries"], 0);
    assert_eq!(result.document["report_payload_bytes_charged"], 131_072);
}

#[test]
fn guarded_apply_unknown_selector_and_cancel_never_claim_finished_inspection() {
    let fixture = Fixture::new();
    let mut r = request(&fixture);
    let mut queries: Value = serde_json::from_str(&r.queries_json).unwrap();
    queries["queries"][0]["rule"] = json!(usize::MAX);
    r.queries_json = queries.to_string();
    let result = owner_guarded_apply_with_progress(r, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!result.diagnostic_complete);
    assert_eq!(result.document["error_kind"], "unknown_candidate");
    let r = request(&fixture);
    let cancel = AtomicBool::new(false);
    let result = owner_guarded_apply_with_progress(r, &cancel, |event| {
        if event["event"] == "guarded_query_started" {
            cancel.store(true, Ordering::Release);
        }
    })
    .unwrap();
    assert!(!result.diagnostic_complete);
    assert_eq!(result.document["error_kind"], "cancelled");
    assert_eq!(result.document["retained_events"], 0);
}
