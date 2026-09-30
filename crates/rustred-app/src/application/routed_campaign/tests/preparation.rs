//! Exact routing remains independent of preparation width and observer type.
use super::*;
use crate::{
    OwnerDomainWalkCheckpointOptions, OwnerDomainWalkPublicationPolicy,
    OwnerDomainWalkSchedulingPolicy, OwnerDomainWalkVerifyOptions,
};
use std::cell::{Cell, RefCell};

fn with_both_routes(fixture: &Fixture) -> RoutedCampaignRequest {
    let mut request = fixture.request.clone();
    let mut selection: Value = serde_json::from_str(&request.selection_json).unwrap();
    selection["initial_frontier_routes"]
        .as_array_mut()
        .unwrap()
        .extend([
            json!({"source_mask":"101","owner_mask":"011","requires_transport":true,
            "source_to_representative":[["1","0"],["0","1"]],
            "owner_to_representative":[["0","1"],["-1","1"]]}),
            json!({"source_mask":"011","owner_mask":"011","requires_transport":false,
            "source_to_representative":[["1","0"],["0","1"]],
            "owner_to_representative":[["1","0"],["0","1"]]}),
        ]);
    request.selection_json = selection.to_string();
    request
}

#[test]
fn native_route_preparation_has_identical_exact_traces_at_one_two_four_workers() {
    let fixture = noninvolutive_route_fixture();
    let mut request = with_both_routes(&fixture);
    let (selection, _, limits) = input::Selection::parse(&request.selection_json).unwrap();
    let coordinator = std::thread::current().id();
    let mut baseline = None;
    for workers in [1, 2, 4] {
        if !crate::test_gates::workers_or_skip("exact route preparation widths", workers) {
            continue;
        }
        request.workers = workers;
        let events = RefCell::new(Vec::new());
        let reducer = prepare::prepare::<3>(
            &request,
            &selection,
            limits,
            &AtomicBool::new(false),
            &|event| {
                assert_eq!(std::thread::current().id(), coordinator);
                events.borrow_mut().push(event);
            },
        )
        .unwrap()
        .unwrap();
        let trace = reducer
            .trace_targets(
                [[2, 2, 0], [2, 0, 2], [2, 2, -1], [2, -1, 2]]
                    .map(|powers| rustred::family::IntegralKey::try_new(powers).unwrap()),
            )
            .unwrap();
        assert!(trace.frontier().is_empty());
        assert!(trace.transport_calls() > 0);
        if let Some(expected) = &baseline {
            assert_eq!(&trace, expected);
        } else {
            baseline = Some(trace);
        }
        assert!(events.borrow().iter().any(|event| {
            event["phase"] == "native_map_verification"
                && event["workers"] == workers
                && event["completed"] == 3
                && event["verified"] == 2
        }));
        assert!(
            events
                .borrow()
                .iter()
                .any(|event| { event["event"] == "routes_verified" && event["routes"] == 2 })
        );
    }
}

#[test]
fn no_transport_routes_still_require_native_family_matrix_dimensions() {
    let fixture = noninvolutive_route_fixture();
    let mut request = fixture.request.clone();
    let mut selection: Value = serde_json::from_str(&request.selection_json).unwrap();
    selection["initial_frontier_routes"] = json!([
        {"source_mask":"011","owner_mask":"011","requires_transport":false,
         "source_to_representative":[["1"]],"owner_to_representative":[["1"]]}
    ]);
    request.selection_json = selection.to_string();
    // Structural JSON cannot know the true number of loop momenta.
    let (selection, _, limits) = input::Selection::parse(&request.selection_json).unwrap();
    for workers in [1, 2] {
        if !crate::test_gates::workers_or_skip("no-transport dimension validation", workers) {
            continue;
        }
        request.workers = workers;
        let error = prepare::prepare::<3>(
            &request,
            &selection,
            limits,
            &AtomicBool::new(false),
            &|_| {},
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("route matrix dimensions differ from native family")
        );
    }
}

#[test]
fn route_preparation_cancellation_does_not_publish_routes_or_a_reducer() {
    let fixture = noninvolutive_route_fixture();
    let request = with_both_routes(&fixture);
    let (selection, _, limits) = input::Selection::parse(&request.selection_json).unwrap();
    for stop_at in [0, 1, 3] {
        let cancellation = AtomicBool::new(false);
        let route_publication = Cell::new(false);
        let result = prepare::prepare::<3>(&request, &selection, limits, &cancellation, &|event| {
            if event["phase"] == "native_map_verification" && event["completed"] == stop_at {
                cancellation.store(true, Ordering::Relaxed);
            }
            if event["event"] == "routes_verified" {
                route_publication.set(true);
            }
        })
        .unwrap();
        assert!(result.is_none());
        assert!(!route_publication.get());
    }
}

#[test]
fn ready_epoch_and_cold_preparation_honor_their_own_worker_budgets() {
    if !crate::test_gates::workers_or_skip("route preparation caller budgets", 2) {
        return;
    }
    let fixture = noninvolutive_route_fixture();
    let coordinator = std::thread::current().id();
    for (name, policy) in [
        ("ready", OwnerDomainWalkPublicationPolicy::Ready),
        ("epoch", OwnerDomainWalkPublicationPolicy::Epoch),
    ] {
        let mut request = route_walk_request(&fixture, 0);
        request.workers = 2;
        request.publication_policy = policy;
        request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
            lookahead: std::num::NonZeroUsize::new(8).unwrap(),
        };
        let directory = fixture.directory.join(format!("route-preparation-{name}"));
        request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&directory));
        let walk_budgets = RefCell::new(Vec::new());
        let result =
            owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |event| {
                assert_eq!(std::thread::current().id(), coordinator);
                if event["phase"] == "native_map_verification" {
                    walk_budgets
                        .borrow_mut()
                        .push(event["workers"].as_u64().unwrap());
                }
            })
            .unwrap();
        assert_eq!(
            result.document["recursive_worklist_exhausted"], true,
            "{}",
            result.document
        );
        assert!(!walk_budgets.borrow().is_empty());
        assert!(walk_budgets.borrow().iter().all(|&budget| budget == 2));

        let mut options = OwnerDomainWalkVerifyOptions::new(&directory);
        options.require_closure = true;
        // A saved W2 campaign must not override this independent W1 verifier.
        options.threads = 1;
        let cold_budgets = RefCell::new(Vec::new());
        let verified = crate::owner_domain_walk_verify_closure(
            &request,
            &options,
            &AtomicBool::new(false),
            |event| {
                assert_eq!(std::thread::current().id(), coordinator);
                if event["phase"] == "native_map_verification" {
                    cold_budgets
                        .borrow_mut()
                        .push(event["workers"].as_u64().unwrap());
                }
            },
        )
        .unwrap();
        assert_eq!(verified["verdict"], "PASS", "{verified}");
        assert!(!cold_budgets.borrow().is_empty());
        assert!(cold_budgets.borrow().iter().all(|&budget| budget == 1));
    }
}
