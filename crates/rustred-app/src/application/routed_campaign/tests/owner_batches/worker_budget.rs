//! Both native dispatch paths use the same explicit reservation.
use super::*;

#[test]
fn explicit_inspector_partition_is_wired_to_both_native_publication_paths() {
    let fixture = noninvolutive_route_fixture();
    let before = saved_owner_bytes(&fixture);
    for workers in available_workers().filter(|&workers| workers <= 6) {
        let available = workers.saturating_sub(1).max(1);
        for inspectors in [1, available] {
            for policy in [
                OwnerDomainWalkPublicationPolicy::Ordered,
                OwnerDomainWalkPublicationPolicy::OwnerBatched,
            ] {
                let mut request = route_walk_request(&fixture, 0);
                request.workers = workers;
                request.inspection_workers = Some(inspectors);
                request.publication_policy = policy;
                let result =
                    owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {})
                        .unwrap();
                assert_resolved(&result);
                let allocation = &result.document["worker_allocation"];
                assert_eq!(allocation["requested_inspection_workers"], inspectors);
                assert_eq!(allocation["inspection_worker_limit"], inspectors);
                assert_eq!(allocation["admission_worker_limit"], available - inspectors);
                assert_eq!(
                    allocation["coordinator_worker_limit"],
                    usize::from(workers > 1)
                );
                assert_eq!(allocation["total_compute_worker_limit"], workers);
                let parallel = &result.document["parallel"];
                if policy == OwnerDomainWalkPublicationPolicy::Ordered {
                    let actual = &parallel["admission_preparation"];
                    assert_eq!(actual["inspection_worker_limit"], inspectors);
                    assert_eq!(actual["lookup_worker_limit"], available - inspectors);
                    assert_eq!(actual["total_compute_worker_limit"], workers);
                } else {
                    assert_eq!(parallel["inspection_worker_limit"], inspectors);
                    assert_eq!(parallel["admission_worker_limit"], available - inspectors);
                    assert_eq!(parallel["total_compute_worker_limit"], workers);
                }
                assert_eq!(parallel["active_workers"], 0);
            }
        }
    }
    assert_unchanged(&before);
}

#[test]
fn explicit_partition_event_cap_keeps_unfinished_responsibilities_incomplete() {
    let fixture = noninvolutive_route_fixture();
    let before = saved_owner_bytes(&fixture);
    for workers in available_workers().filter(|&workers| workers <= 6) {
        for inspectors in [1, workers.saturating_sub(1).max(1)] {
            for policy in [
                OwnerDomainWalkPublicationPolicy::Ordered,
                OwnerDomainWalkPublicationPolicy::OwnerBatched,
            ] {
                let mut request = route_walk_request(&fixture, 0);
                request.workers = workers;
                request.inspection_workers = Some(inspectors);
                request.publication_policy = policy;
                request.max_events = 1;
                let result =
                    owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {})
                        .unwrap();
                assert!(
                    !result.all_scheduled_domains_resolved,
                    "{}",
                    result.document
                );
                assert_eq!(result.document["recursive_worklist_exhausted"], false);
                assert!(
                    result.document["error"]
                        .as_str()
                        .unwrap()
                        .contains("event allowance")
                );
                assert_no_authority_claim(&result.document);
            }
        }
    }
    assert_unchanged(&before);
}
