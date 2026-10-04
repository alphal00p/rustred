//! Actual prepared native reducer through the public CP6 lifecycle. Preparation
//! itself is the existing fixture import, not a claim of CLI/process coverage.
use super::super::{admission, controller, public};
use super::controller_tests::native_equivalence::{closed_fixture, completed_snapshot};
use super::*;
use crate::application::routed_campaign::walking::epoch::{
    self,
    inspector::{Context, inspect_job, inspect_job_with_snapshot},
    merge::StopReason,
    snapshot::Publication,
};
use crate::{OwnerDomainWalkCheckpointOptions, OwnerDomainWalkEpochInspectorLookup};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

struct NoFinalizers;
impl NoFinalizers {
    fn enter() -> Self {
        epoch::FORBID_LARGE_FINALIZATION.with(|flag| assert!(!flag.replace(true)));
        Self
    }
}
impl Drop for NoFinalizers {
    fn drop(&mut self) {
        epoch::FORBID_LARGE_FINALIZATION.with(|flag| flag.set(false));
    }
}

fn options(fixture: &mut Fixture, resume: bool, width: usize) {
    fixture.request.workers = width;
    let mut options = OwnerDomainWalkCheckpointOptions::new(fixture.directory.0.clone());
    options.resume = resume;
    fixture.request.checkpoint = Some(options);
}

fn run_public(
    fixture: &Fixture,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
) -> crate::OwnerDomainWalkResult {
    let _guard = NoFinalizers::enter();
    public::run(
        &fixture.request,
        &fixture.reducer,
        &fixture.owners,
        &fixture.queries,
        16,
        Instant::now(),
        0.0,
        cancel,
        observer,
    )
    .unwrap()
}

fn assert_summary(result: &crate::OwnerDomainWalkResult, drained: bool) {
    assert!(!result.all_scheduled_domains_resolved);
    let doc = &result.document;
    assert_eq!(doc["recursive_worklist_exhausted"], drained);
    assert_eq!(doc["full_result_in_output_document"], false);
    assert_eq!(doc["full_state_in_checkpoint"], true);
    assert_eq!(doc["finalization"], "not_evaluated");
    assert_eq!(doc["query_admission"]["required_closed"], Value::Null);
    assert!(doc.get("domains").is_none());
    assert_eq!(doc["checkpoint"]["format"], "RUSTRED-WALK-CP6");
    assert_eq!(doc["checkpoint"]["state"], "saved");
    assert_eq!(doc["parallel"]["workers_joined"], true);
    assert_eq!(doc["parallel"]["active_workers"], 0);
    assert_eq!(doc["parallel"]["computing_workers"], 0);
    assert_eq!(doc["parallel"]["finished_uncommitted_domains"], 0);
    assert_eq!(doc["parallel"]["occupied_native_slots"], 0);
    let progress = crate::OwnerDomainWalkResult::completion_progress(doc);
    assert_eq!(progress["full_result_in_output_document"], false);
    assert_eq!(progress["full_state_in_checkpoint"], true);
    let closure = &doc["descendant_closure"];
    assert_eq!(closure["initial_total"], doc["initial_entry_domains_total"]);
    assert_eq!(closure["total_domains"], doc["scheduled_nodes"]);
    if closure["available"] == true {
        let total = closure["total_domains"].as_u64().unwrap();
        let closed = closure["total_closed"].as_u64().unwrap();
        assert!(closure["initial_closed"].as_u64().unwrap() <= closed);
        assert!(closed <= total);
        assert_eq!(closure["unresolved_domains"], total - closed);
        assert!(closure["locally_inspected"].is_u64());
        assert_eq!(closure["reason"], Value::Null);
    } else {
        assert_eq!(closure["unresolved_domains"], Value::Null);
        assert!(closure["reason"].is_string());
    }
    assert_eq!(progress["descendant_closure"], *closure);
    assert_eq!(progress["epoch"], doc["epoch"]);
}

#[test]
fn public_snapshot_lookup_matches_all_miss_with_real_hits_and_misses() {
    const TEST: &str = "public CP6 lookup control W1/W2 differential";
    if !crate::test_gates::workers_or_skip(TEST, 2)
        || !std::thread::spawn(|| crate::test_gates::licensed_or_skip(TEST))
            .join()
            .unwrap()
    {
        return;
    }
    let mut fixture = closed_fixture();
    // Do not admit the whole ray here: it contains every successor and would
    // exercise hits only. These three points leave power 1 absent initially,
    // while retaining the real 3 -> 2 positive lookup and terminal at 0.
    fixture.queries[2].upper = vec![Some(0)];
    let mut queries: Value = serde_json::from_str(&fixture.request.matching.queries_json).unwrap();
    queries["queries"][2]["upper"] = json!([0]);
    fixture.request.matching.queries_json = queries.to_string();
    let mut expected = None;
    for mode in [
        OwnerDomainWalkEpochInspectorLookup::AllMiss,
        OwnerDomainWalkEpochInspectorLookup::Snapshot,
    ] {
        for width in [1, 2] {
            fixture.directory = Directory::new();
            fixture.request.epoch_inspector_lookup = mode;
            options(&mut fixture, false, width);
            let result = run_public(&fixture, &AtomicBool::new(false), &|_| {});
            assert_summary(&result, true);
            assert_eq!(
                result.document["epoch"]["inspector_lookup_mode"],
                mode.name()
            );
            let lookup = &result.document["epoch"]["inspector_lookup"];
            let hits = lookup["stored_hits"].as_u64().unwrap();
            let misses = lookup["coordinator_miss_rechecks_skipped"]
                .as_u64()
                .unwrap();
            let queries = lookup["queries"].as_u64().unwrap();
            assert_eq!(queries, hits + misses);
            match mode {
                OwnerDomainWalkEpochInspectorLookup::AllMiss => {
                    assert_eq!((queries, hits, misses), (0, 0, 0))
                }
                OwnerDomainWalkEpochInspectorLookup::Snapshot => {
                    assert!(hits > 0, "actual positive lookup required");
                    assert!(misses > 0, "actual same-view miss bypass required");
                }
            }
            let mut restored = fixture.open().unwrap();
            assert_eq!(
                restored.state.inspector_lookup.queries, 0,
                "report counters are invocation-local"
            );
            let mut actual = completed_snapshot(&mut restored);
            // Same exclusions as the reviewed private mode differential:
            // work accounting only. The comparator already omits top-level
            // record seconds and transport session; every record body,
            // ledger token, edge, image, root/role and frontier stays exact.
            actual.as_object_mut().unwrap().remove("lookup");
            actual.as_object_mut().unwrap().remove("verify");
            actual["walk_counters"]
                .as_object_mut()
                .unwrap()
                .remove("miss_requests");
            if let Some(expected) = &expected {
                assert_eq!(&actual, expected);
            } else {
                expected = Some(actual);
            }
        }
    }
}

#[test]
fn cross_mode_resume_refuses_compatible_previous_before_session_adoption() {
    for (saved, requested) in [
        (
            OwnerDomainWalkEpochInspectorLookup::AllMiss,
            OwnerDomainWalkEpochInspectorLookup::Snapshot,
        ),
        (
            OwnerDomainWalkEpochInspectorLookup::Snapshot,
            OwnerDomainWalkEpochInspectorLookup::AllMiss,
        ),
    ] {
        let mut fixture = closed_fixture();
        options(&mut fixture, false, 1);
        fixture.request.epoch_inspector_lookup = requested;
        fixture.save(3, 0);
        let mut restored = fixture.open().unwrap();
        // Test-only construction of a valid latest in the other mode over a
        // compatible previous. Ordinary cross-mode open must never do this.
        fixture.request.epoch_inspector_lookup = saved;
        resave(&fixture, &mut restored);
        drop(restored);
        let paths = [
            publication::LATEST,
            publication::PREVIOUS,
            "epoch-session.bin",
        ];
        let before = paths.map(|name| fs::read(fixture.directory.0.join(name)).unwrap());
        fixture.request.epoch_inspector_lookup = requested;
        let error = fixture
            .open()
            .err()
            .expect("cross-mode resume must refuse latest, not use previous");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        for (name, bytes) in paths.into_iter().zip(&before) {
            assert_eq!(
                &fs::read(fixture.directory.0.join(name)).unwrap(),
                bytes,
                "{name}"
            );
        }
        fixture.request.epoch_inspector_lookup = saved;
        let restored = fixture.open().unwrap();
        assert!(restored.warnings.is_empty());
        assert_eq!(restored.publisher.current_generation(), Some(2));
    }
}

#[test]
fn g2_cross_mode_resume_refuses_both_directions_before_session_adoption() {
    use crate::OwnerDomainWalkG2ResidualAnchors::{Off, Union};
    for (saved, requested) in [(Off, Union), (Union, Off)] {
        let mut fixture = closed_fixture();
        fixture.request.g2_residual_anchors = saved;
        fixture.save(3, 0);
        let before = fs::read(fixture.directory.0.join("epoch-session.bin")).unwrap();
        fixture.request.g2_residual_anchors = requested;
        assert_eq!(
            fixture.open().err().unwrap().kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            fs::read(fixture.directory.0.join("epoch-session.bin")).unwrap(),
            before
        );
    }
}

#[test]
fn public_cp6_real_native_w1_and_w2_preserve_full_snapshot_without_finalization() {
    const TEST: &str = "public CP6 real W1/W2 lifecycle";
    if !crate::test_gates::workers_or_skip(TEST, 2) {
        return;
    }
    // Only W2 needs actual worker authorization. W1 still executes on the
    // caller; this skip never counts as either real-native equivalence PASS.
    if !std::thread::spawn(|| crate::test_gates::licensed_or_skip(TEST))
        .join()
        .unwrap()
    {
        return;
    }
    let mut fixture = closed_fixture();
    let mut expected = None;
    for width in [1, 2] {
        fixture.directory = Directory::new();
        options(&mut fixture, false, width);
        let events = RefCell::new(Vec::new());
        let result = run_public(&fixture, &AtomicBool::new(false), &|event| {
            events.borrow_mut().push(event)
        });
        assert_summary(&result, true);
        assert_eq!(result.document["query_admission"]["required"], 2);
        assert_eq!(result.document["query_admission"]["auxiliary"], 1);
        let events = events.into_inner();
        let saved = events
            .iter()
            .find(|event| event["event"] == "checkpoint_saved")
            .unwrap();
        assert_eq!(saved["scheduled_nodes"], result.document["scheduled_nodes"]);
        assert_eq!(
            saved["committed_events"],
            result.document["committed_events"]
        );
        assert!(
            saved.get("progress").is_none(),
            "CLI supplies the one progress envelope"
        );
        assert_eq!(saved["parallel"]["active_workers"], Value::Null);
        assert_eq!(saved["parallel"]["workers_joined"], false);
        let actual = completed_snapshot(&mut fixture.open().unwrap());
        if let Some(expected) = &expected {
            assert_eq!(&actual, expected);
        } else {
            expected = Some(actual);
        }
    }
}

#[test]
fn real_inline_and_pool_interruption_resume_in_both_width_directions() {
    interrupted_public_mode(OwnerDomainWalkEpochInspectorLookup::AllMiss, false, false);
}

#[test]
fn snapshot_inline_and_pool_interruption_resume_in_both_width_directions() {
    interrupted_public_mode(OwnerDomainWalkEpochInspectorLookup::Snapshot, false, false);
}

#[test]
fn rolling_real_native_interruption_resumes_saved_window_in_both_width_directions() {
    for mode in [
        OwnerDomainWalkEpochInspectorLookup::AllMiss,
        OwnerDomainWalkEpochInspectorLookup::Snapshot,
    ] {
        interrupted_public_mode(mode, true, false);
    }
}

#[test]
fn rolling_adaptive_real_native_interruption_preserves_observations_on_resume() {
    interrupted_public_mode(OwnerDomainWalkEpochInspectorLookup::Snapshot, true, true);
}

fn interrupted_public_mode(
    mode: OwnerDomainWalkEpochInspectorLookup,
    rolling: bool,
    adaptive: bool,
) {
    let test = match mode {
        OwnerDomainWalkEpochInspectorLookup::AllMiss => {
            "public CP6 AllMiss W1/W2 interrupted native cut"
        }
        OwnerDomainWalkEpochInspectorLookup::Snapshot => {
            "public CP6 Snapshot W1/W2 interrupted native cut"
        }
    };
    if !crate::test_gates::workers_or_skip(test, 3)
        || !std::thread::spawn(move || crate::test_gates::licensed_or_skip(test))
            .join()
            .unwrap()
    {
        return;
    }
    let mut fixture = closed_fixture();
    fixture.request.epoch_inspector_lookup = mode;
    fixture.request.epoch_rolling = rolling;
    if adaptive {
        fixture.request.epoch_dispatch = crate::OwnerDomainWalkEpochDispatchPolicy::Adaptive;
    }
    for (first, next) in [(1, 2), (2, 1)] {
        // Compare against the same frozen logical window. W1 starts at one;
        // widening the physical pool on resume must not silently change cuts.
        fixture.directory = Directory::new();
        options(&mut fixture, false, first);
        assert_summary(
            &run_public(&fixture, &AtomicBool::new(false), &|_| {}),
            true,
        );
        let expected = completed_snapshot(&mut fixture.open().unwrap());
        fixture.directory = Directory::new();
        options(&mut fixture, false, first);
        let window = fixture.request.epoch_window(16);
        let reserved = window.min(3);
        let mut restored =
            admission::fresh(fixture.directory.0.clone(), &fixture.identity()).unwrap();
        assert_eq!(
            admission::continue_admission(
                &mut restored,
                &fixture.identity(),
                &fixture.reducer,
                window,
                || None,
                |_| panic!("no stop during admission")
            )
            .unwrap(),
            admission::Outcome::Ready
        );
        let overlap =
            admission::initial_overlap(&restored, &fixture.request, &AtomicBool::new(false))
                .unwrap();
        let cancel = AtomicBool::new(false);
        let caller = std::thread::current().id();
        let calls = AtomicUsize::new(0);
        let snapshots = Publication::new();
        let saved = AtomicBool::new(false);
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let release_rx = Mutex::new(release_rx);
        let directory = &fixture.directory.0;
        std::thread::scope(|scope| {
            let saved_ref = &saved;
            let cancel_ref = &cancel;
            let witness = scope.spawn(move || {
                entered_rx
                    .recv_timeout(Duration::from_secs(30))
                    .expect("actual native bytes returned before hold");
                assert!(
                    !directory.join(publication::LATEST).exists(),
                    "inline/pool result not committed while held"
                );
                assert!(!saved_ref.load(Ordering::Acquire));
                cancel_ref.store(true, Ordering::Release);
                // W1 cannot save while this caller-thread callback is held.
                // W2 must save before joining the held worker.
                if first == 2 {
                    let deadline = Instant::now() + Duration::from_secs(30);
                    while !saved_ref.load(Ordering::Acquire) {
                        assert!(Instant::now() < deadline, "save before held join");
                        std::thread::yield_now();
                    }
                }
                release_tx.send(()).unwrap();
            });
            let inspect = |bytes: &[u8], worker_stop: &AtomicBool| {
                if first == 1 {
                    assert_eq!(
                        std::thread::current().id(),
                        caller,
                        "W1 must be caller-thread CAS"
                    );
                }
                let call = calls.fetch_add(1, Ordering::Relaxed);
                let context = Context {
                    reducer: &fixture.reducer,
                    request: &fixture.request,
                    overlap: &overlap,
                    cancellation: if first == 1 { &cancel } else { worker_stop },
                    g2: None,
                    finite_account: None,
                };
                let result = match mode {
                    OwnerDomainWalkEpochInspectorLookup::AllMiss => inspect_job(&context, bytes),
                    OwnerDomainWalkEpochInspectorLookup::Snapshot => {
                        let snapshot = if rolling {
                            let job = epoch::job::Job::<1>::decode(bytes).unwrap();
                            snapshots.acquire_job(job.seq).unwrap()
                        } else {
                            snapshots.acquire().unwrap()
                        };
                        inspect_job_with_snapshot(&context, bytes, snapshot)
                    }
                };
                if call == 0 {
                    entered_tx.send(()).unwrap();
                    release_rx
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(30))
                        .expect("held inspection release");
                }
                result
            };
            let authorize = || {
                assert_ne!(
                    first, 1,
                    "inline executor must not start/authorize a worker"
                );
                if symbolica::license::LicenseManager::is_licensed() {
                    Ok(())
                } else {
                    Err("test worker license unavailable".into())
                }
            };
            assert_eq!(
                controller::run_observed(
                    &mut restored,
                    &fixture.identity(),
                    window,
                    first,
                    MergeConfig {
                        frontier_stop: false,
                        lockstep: !rolling,
                        g2: false,
                        finite_replay: None,
                    },
                    &authorize,
                    &inspect,
                    || super::super::super::super::stop::requested(&cancel, None),
                    |_| false,
                    |state, _, status| {
                        assert_eq!(state.state.k, 0);
                        assert_eq!(state.records.total(), 0);
                        assert_eq!(state.state.in_flight.len(), reserved);
                        assert_eq!(status.len(), reserved);
                        assert_eq!(status.iter().filter(|s| s.started).count(), 1);
                        assert_eq!(status.iter().filter(|s| !s.started).count(), reserved - 1);
                        if first == 1 {
                            assert_eq!(status.iter().filter(|s| s.returned).count(), 1);
                        }
                        saved.store(true, Ordering::Release);
                    },
                    (mode == OwnerDomainWalkEpochInspectorLookup::Snapshot).then_some(&snapshots),
                    |_, _, _, _| {},
                    |_| panic!("cancelled unpublished cut must not run monitor maintenance")
                )
                .unwrap(),
                controller::Outcome::Stopped(StopReason::Paused)
            );
            witness.join().unwrap();
        });
        assert!(saved.load(Ordering::Acquire));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(restored.state.edges.runs(), 0);
        assert_eq!(restored.state.k, 0);
        drop(restored);
        options(&mut fixture, true, next);
        let result = run_public(&fixture, &AtomicBool::new(false), &|_| {});
        assert_summary(&result, true);
        assert_eq!(result.document["epoch"]["schedule"]["window"], window);
        assert_eq!(
            result.document["epoch"]["schedule"]["kind"],
            if rolling { "rolling" } else { "lockstep" }
        );
        assert_eq!(
            result.document["epoch"]["schedule"]["dispatch"],
            fixture.request.epoch_dispatch.name()
        );
        let mut restored = fixture.open().unwrap();
        if adaptive {
            let saved = restored.dispatch.checkpoint_snapshot();
            let policy = saved
                .adaptive
                .expect("bound adaptive state survives save/open");
            let policy = serde_json::to_value(policy).unwrap();
            let samples: u64 = policy["buckets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|bucket| bucket["samples"].as_u64().unwrap())
                .sum();
            assert_eq!(
                samples, restored.state.counters.natives,
                "exactly successfully published native entries observed"
            );
            assert!(samples > 0);
        }
        assert_eq!(completed_snapshot(&mut restored), expected);
    }
}

#[test]
fn public_after_latest_observer_panic_keeps_checkpoint_reopenable() {
    let mut fixture = closed_fixture();
    options(&mut fixture, false, 1);
    let result = run_public(&fixture, &AtomicBool::new(false), &|event| {
        if event["event"] == "checkpoint_saved" {
            panic!("injected nonauthority sink panic after latest");
        }
    });
    assert_summary(&result, true);
    assert_eq!(result.document["observer_failed"], true);
    assert!(!fixture.directory.0.join("epoch-poison").exists());
    let mut state = fixture
        .open()
        .expect("reporting cannot invalidate durable math authority");
    assert!(
        !completed_snapshot(&mut state)["records"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn public_prefix_pause_preserves_unknown_closure_and_role_rows() {
    let mut fixture = closed_fixture();
    options(&mut fixture, false, 1);
    let result = run_public(&fixture, &AtomicBool::new(true), &|_| {});
    assert_summary(&result, false);
    assert_eq!(result.document["query_admission"]["unadmitted"], 3);
    assert_eq!(result.document["query_admission"]["admitted_required"], 0);
    let state = fixture.open().unwrap();
    assert!(state.roots.rows.is_empty());
    assert_eq!(state.state.watermark(), 0);
    assert_eq!(state.dispatch.checkpoint_snapshot().counter, 0);
    assert!(state.replay.is_empty());
    assert!(state.state.tracker.counters().unavailable.is_none());
}
