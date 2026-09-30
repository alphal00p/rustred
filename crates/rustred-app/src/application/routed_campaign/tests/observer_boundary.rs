//! Public progress observers remain borrowed, non-Send coordinator callbacks.
use super::*;
use crate::{
    OwnerDomainWalkCheckpointOptions, OwnerDomainWalkPublicationPolicy,
    OwnerDomainWalkSchedulingPolicy, OwnerDomainWalkVerifyOptions,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn request(fixture: &Fixture, policy: OwnerDomainWalkPublicationPolicy) -> OwnerDomainWalkRequest {
    let mut matching = match_request(fixture);
    matching.queries_json = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"narrow","owner":"1","lower":[2],"upper":[2],"max_numerator_rank":11},
        {"id":"whole-ray","owner":"1","lower":[0],"upper":[null],"max_numerator_rank":11}
    ]})
    .to_string();
    let mut request = OwnerDomainWalkRequest::new(matching);
    request.publication_policy = policy;
    request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
        lookahead: std::num::NonZeroUsize::new(8).unwrap(),
    };
    request.workers = 2;
    request
}

#[test]
fn borrowed_non_send_observers_stay_on_coordinator_through_walk_cold_and_resume() {
    if !crate::test_gates::workers_or_skip("borrowed progress observers", 2) {
        return;
    }
    let fixture = Fixture::new();
    let coordinator = std::thread::current().id();
    for (name, policy) in [
        ("ready", OwnerDomainWalkPublicationPolicy::Ready),
        ("epoch", OwnerDomainWalkPublicationPolicy::Epoch),
    ] {
        let mut request = request(&fixture, policy);
        let directory = fixture.directory.join(format!("borrowed-observer-{name}"));
        request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&directory));
        let allowance_queries = json!(request.matching.max_queries);
        let allowance_bytes = json!(request.matching.max_query_bytes);
        // Rc is neither Send nor Sync. Capturing these local references also
        // rules out an accidental 'static requirement on the public APIs.
        let count = Rc::new(Cell::new(0usize));
        let coordinator_only = Rc::new(Cell::new(true));
        let events = RefCell::new(Vec::new());
        let borrowed_label = format!("local-{name}");
        let result =
            owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |event| {
                assert_eq!(borrowed_label, format!("local-{name}"));
                count.set(count.get() + 1);
                coordinator_only
                    .set(coordinator_only.get() && std::thread::current().id() == coordinator);
                assert_eq!(event["requested_max_queries"], allowance_queries);
                assert_eq!(event["requested_max_query_bytes"], allowance_bytes);
                events.borrow_mut().push(event);
            })
            .unwrap();
        assert!(coordinator_only.get());
        assert!(count.get() > 1);
        assert_eq!(events.borrow()[0]["event"], "admitted");
        assert_eq!(result.document["recursive_worklist_exhausted"], true);
        assert!(directory.join("latest.json").is_file());
        if policy == OwnerDomainWalkPublicationPolicy::Epoch {
            assert!(
                events
                    .borrow()
                    .iter()
                    .any(|event| event["event"] == "epoch_lifecycle")
            );
            assert!(
                events
                    .borrow()
                    .iter()
                    .any(|event| event["event"] == "checkpoint_saved")
            );
        } else {
            assert!(result.all_scheduled_domains_resolved);
            // Ready intentionally has no per-domain start event. Its final
            // durable checkpoint is an unthrottled coordinator callback after
            // actual native work, unlike optional 250 ms progress heartbeats.
            assert!(events.borrow().iter().any(|event| {
                event["event"] == "checkpoint_saved"
                    && event["checkpoint"]["completed_native_inspections"]
                        .as_u64()
                        .is_some_and(|completed| completed > 0)
            }));
        }

        let mut options = OwnerDomainWalkVerifyOptions::new(&directory);
        options.require_closure = true;
        options.threads = 2;
        let cold_count = Rc::new(Cell::new(0usize));
        let cold_events = RefCell::new(Vec::new());
        let verified = crate::owner_domain_walk_verify_closure(
            &request,
            &options,
            &AtomicBool::new(false),
            |event| {
                assert!(!borrowed_label.is_empty());
                cold_count.set(cold_count.get() + 1);
                coordinator_only
                    .set(coordinator_only.get() && std::thread::current().id() == coordinator);
                cold_events.borrow_mut().push(event);
            },
        )
        .unwrap();
        assert!(coordinator_only.get());
        assert!(cold_count.get() > 0);
        assert!(
            cold_events
                .borrow()
                .iter()
                .any(|event| event["event"] == "verify_loaded")
        );
        assert_eq!(verified["verdict"], "PASS", "{verified}");
        assert_eq!(
            verified["roots_independently_verified"],
            verified["roots_total"]
        );

        request.checkpoint.as_mut().unwrap().resume = true;
        let resume_count = Rc::new(Cell::new(0usize));
        let resumed = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |event| {
            assert!(!borrowed_label.is_empty());
            resume_count.set(resume_count.get() + 1);
            coordinator_only
                .set(coordinator_only.get() && std::thread::current().id() == coordinator);
            assert_eq!(event["requested_max_queries"], allowance_queries);
            assert_eq!(event["requested_max_query_bytes"], allowance_bytes);
        })
        .unwrap();
        assert!(coordinator_only.get());
        assert!(resume_count.get() > 1);
        assert_eq!(resumed.document["recursive_worklist_exhausted"], true);
    }
}

#[test]
fn borrowed_ordered_observer_can_cancel_after_native_dispatch() {
    if !crate::test_gates::workers_or_skip("borrowed observer cancellation", 2) {
        return;
    }
    let fixture = Fixture::new();
    // Ordered emits domain_started synchronously after dispatch and before
    // polling the first native result. This tests cancellation through the
    // same public borrowed-observer boundary without relying on a Ready
    // heartbeat (which a small successful fixture need not emit).
    let request = request(&fixture, OwnerDomainWalkPublicationPolicy::Ordered);
    let coordinator = std::thread::current().id();
    let dispatched = Rc::new(Cell::new(false));
    let cancellation = AtomicBool::new(false);
    let stopped = owner_domain_walk_with_progress(request, &cancellation, |event| {
        assert_eq!(std::thread::current().id(), coordinator);
        if event["event"] == "domain_started" {
            assert!(
                event["parallel"]["dispatched_uncommitted_domains"]
                    .as_u64()
                    .is_some_and(|count| count > 0)
            );
            dispatched.set(true);
            cancellation.store(true, Ordering::Release);
        }
    })
    .unwrap();
    assert!(dispatched.get());
    assert!(!stopped.all_scheduled_domains_resolved);
    assert_eq!(stopped.document["recursive_worklist_exhausted"], false);
    assert_eq!(stopped.document["parallel"]["active_workers"], 0);
    assert_eq!(
        stopped.document["parallel"]["first_failure"]["kind"],
        "cancelled"
    );
}
