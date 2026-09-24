use super::*;
use crate::{OwnerDomainWalkRequest, owner_domain_walk_with_progress};
use std::cell::Cell;
use std::sync::atomic::AtomicBool;

#[test]
fn larger_admitted_input_preserves_request_metadata_without_loading_on_cancellation() {
    let selection = json!({"family_fingerprint":"not-loaded",
        "owners":[{"path":"must-not-open-missing-owner.rrbin","bytes":1,"mask":"1"}],
        "initial_frontier_routes":[]})
    .to_string();
    let rows = (0..10_001)
        .map(|i| {
            json!({"id":format!("query-{i:064}"),"owner":"1",
        "lower":[0],"upper":[null],"max_numerator_rank":null})
        })
        .collect::<Vec<_>>();
    let text = json!({"schema":"rustred.owner-domain-queries.json.v2","queries":rows}).to_string();
    assert!(text.len() > 1024 * 1024);
    let mut request = OwnerDomainMatchRequest::new(selection, text);
    request.max_queries = 10_001;
    request.max_query_bytes = request.queries_json.len();
    let check = |event: Value| {
        assert_eq!(event["requested_max_queries"], 10_001);
        assert_eq!(event["requested_max_query_bytes"], request.max_query_bytes);
    };
    let local =
        owner_domain_match_with_progress(request.clone(), &AtomicBool::new(true), check).unwrap();
    assert!(!local.classification_complete);
    check(local.document);
    for policy in [
        crate::OwnerDomainWalkPublicationPolicy::Ordered,
        crate::OwnerDomainWalkPublicationPolicy::OwnerBatched,
    ] {
        let mut walk = OwnerDomainWalkRequest::new(request.clone());
        walk.publication_policy = policy;
        let result = owner_domain_walk_with_progress(walk, &AtomicBool::new(true), check).unwrap();
        assert!(!result.all_scheduled_domains_resolved);
        check(result.document);
    }
}

#[test]
fn query_defaults_and_preflight_are_independent_of_native_work_limits() {
    let mut request = OwnerDomainMatchRequest::new(String::new(), "{}".into());
    assert_eq!(
        (request.max_queries, request.max_query_bytes),
        (256, 1024 * 1024)
    );
    assert_eq!(request.max_total_pieces, 100_000);
    request.max_queries = usize::MAX;
    request.max_query_bytes = 2;
    assert!(request.preflight_queries().is_ok());
    request.max_query_bytes = 1;
    assert!(request.preflight_queries().is_err());
}

#[test]
fn both_library_entrypoints_reject_bad_bytes_before_invalid_selection_or_native_load() {
    for (count, bytes) in [(0, 4), (1, 0), (1, 1)] {
        let mut request = OwnerDomainMatchRequest::new("not a manifest".into(), "{}".into());
        request.max_queries = count;
        request.max_query_bytes = bytes;
        let events = Cell::new(0);
        let local =
            owner_domain_match_with_progress(request.clone(), &AtomicBool::new(false), |_| {
                events.set(events.get() + 1)
            })
            .unwrap_err();
        let walk = owner_domain_walk_with_progress(
            OwnerDomainWalkRequest::new(request),
            &AtomicBool::new(false),
            |_| events.set(events.get() + 1),
        )
        .unwrap_err();
        assert_eq!(local.to_string(), walk.to_string());
        assert_eq!(events.get(), 0);
    }
}

#[test]
fn invalid_last_query_above_old_count_limit_prevents_missing_bundle_load() {
    let selection = json!({"family_fingerprint":"not-loaded",
        "owners":[{"path":"must-not-open-missing-owner.rrbin","bytes":1,"mask":"1"}],
        "initial_frontier_routes":[]})
    .to_string();
    let mut rows = (0..10_001)
        .map(|i| {
            json!({"id":format!("query-{i:064}"),"owner":"1",
        "lower":[0],"upper":[null],"max_numerator_rank":null})
        })
        .collect::<Vec<_>>();
    rows[10_000]["max_numerator_rank"] = json!(-1);
    let text = json!({"schema":"rustred.owner-domain-queries.json.v2","queries":rows}).to_string();
    let mut request = OwnerDomainMatchRequest::new(selection, text);
    request.max_queries = 10_001;
    request.max_query_bytes = request.queries_json.len();
    let events = Cell::new(0);
    let local = owner_domain_match_with_progress(request.clone(), &AtomicBool::new(false), |_| {
        events.set(events.get() + 1)
    })
    .unwrap_err();
    let walk = owner_domain_walk_with_progress(
        OwnerDomainWalkRequest::new(request),
        &AtomicBool::new(false),
        |_| events.set(events.get() + 1),
    )
    .unwrap_err();
    assert!(local.to_string().contains("query rank"), "{local}");
    assert_eq!(local.to_string(), walk.to_string());
    assert_eq!(events.get(), 0);
}
