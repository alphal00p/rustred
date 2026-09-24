use super::*;
use crate::{OwnerDomainWalkPublicationPolicy, OwnerDomainWalkSchedulingPolicy};
use std::cell::RefCell;
use std::num::NonZeroUsize;

#[test]
fn query_byte_admission_precedes_selection_and_native_preparation_in_both_paths() {
    let mut request = OwnerDomainMatchRequest::new("invalid selection".into(), "oversize".into());
    request.max_query_bytes = 2;
    let matching =
        owner_domain_match_with_progress(request.clone(), &AtomicBool::new(false), |_| {
            panic!("invalid input must not emit native progress")
        })
        .unwrap_err();
    let walking = owner_domain_walk_with_progress(
        OwnerDomainWalkRequest::new(request),
        &AtomicBool::new(false),
        |_| panic!("invalid input must not emit native progress"),
    )
    .unwrap_err();
    for error in [matching, walking] {
        assert!(error.to_string().contains("2-byte allowance"), "{error}");
    }
}

#[test]
fn native_index_build_metadata_survives_both_execution_policies() {
    let fixture = Fixture::new();
    for policy in [
        OwnerDomainWalkPublicationPolicy::Ordered,
        OwnerDomainWalkPublicationPolicy::OwnerBatched,
    ] {
        let mut request = OwnerDomainWalkRequest::new(match_request(&fixture));
        request.publication_policy = policy;
        request.matching.max_queries = 20_000;
        request.matching.max_query_bytes = 2_000_000;
        request.reuse_initial_d_bands = true;
        request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
            lookahead: NonZeroUsize::new(2).unwrap(),
        };
        let events = RefCell::new(Vec::new());
        let result = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |event| {
            if event["event"] == "initial_overlap_prepared" {
                events.borrow_mut().push(event);
            }
        })
        .unwrap();
        assert!(result.all_scheduled_domains_resolved, "{}", result.document);
        let events = events.into_inner();
        assert_eq!(events.len(), 1);
        for document in [&events[0], &result.document] {
            assert_eq!(document["requested_max_queries"], 20_000);
            assert_eq!(document["requested_max_query_bytes"], 2_000_000);
        }
        let completion = OwnerDomainWalkResult::completion_progress(&result.document);
        assert_eq!(completion["requested_max_queries"], 20_000);
        assert_eq!(completion["requested_max_query_bytes"], 2_000_000);
        assert_eq!(
            events[0]["initial_overlap_index"],
            result.document["initial_overlap_index"]
        );
        let report = if policy == OwnerDomainWalkPublicationPolicy::Ordered {
            assert_eq!(
                result.document["initial_overlap_index"]["identity_scope"],
                "global_initial_domain_id"
            );
            &result.document["initial_overlap_index"]
        } else {
            let summary = &result.document["initial_overlap_index"];
            assert_eq!(summary["scope"], "per_bucket_indices");
            assert_eq!(summary["bucket_status_counts"]["active"], 1);
            assert_eq!(summary["limits_apply_separately_per_index"], true);
            let report = &result.document["owner_buckets"][0]["initial_overlap_index"];
            assert_eq!(report["identity_scope"], "bucket_local_initial_domain_id");
            report
        };
        assert_eq!(report["status"], "active");
        assert_eq!(report["total_initial"], 1);
        assert_eq!(report["eligible_apply"], 1);
        assert_eq!(report["eligibility_complete"], true);
        assert_eq!(report["retained_membership"], 1);
        assert_eq!(report["usable_anchors"], 1);
        assert_eq!(report["coverage_authority"], false);
        assert_eq!(
            OwnerDomainWalkResult::completion_progress(&result.document)["initial_overlap_index"],
            result.document["initial_overlap_index"]
        );
    }
}
