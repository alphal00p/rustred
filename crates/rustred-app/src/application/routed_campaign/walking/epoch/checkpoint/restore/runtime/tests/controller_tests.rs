use super::super::controller::{self, Outcome};
use super::*;
use crate::application::routed_campaign::walking::epoch::checkpoint::stop;
use crate::application::routed_campaign::walking::epoch::job::Job;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, mpsc};
use std::time::Duration;

pub(super) mod native_equivalence;
mod native_periodic;
mod native_prefix;
mod periodic_tests;
mod preparation_tests;
mod profiling_tests;
mod rolling_tests;

fn config() -> MergeConfig {
    MergeConfig {
        frontier_stop: true,
        lockstep: true,
        g2: false,
        finite_replay: None,
    }
}

fn result(job: &Job<1>, failure: bool) -> Vec<u8> {
    JobResult::<1> {
        seq: job.seq,
        parent: job.parent,
        v0: job.v0,
        kind: NativeKind::Apply,
        error_kind: if failure {
            ErrorKind::NativeFailure
        } else {
            ErrorKind::None
        },
        break_reason: BreakReason::None,
        panic: false,
        emitted: 0,
        accepted: 0,
        stats_events: 0,
        successors: 0,
        conditional: 0,
        known_reuse: 0,
        job_duplicates: 0,
        optional: [0; 3],
        route_masks: 0,
        route_joint_pruned: 0,
        seconds: 0.0,
        stats_json: b"{\"events\":0}".to_vec(),
        error: failure.then(|| "deterministic native failure".into()),
        frontiers: Vec::new(),
        refusals: Vec::new(),
        refusals_truncated: false,
        scope: None,
        g2: None,
        finite_replay: None,
        lookup: None,
        misses: Vec::new(),
    }
    .encode()
}

#[test]
fn controller_discards_polled_prefix_and_saves_context_before_held_join() {
    let fixture = Fixture::new();
    fixture.save(3, 3);
    let stop_path = fixture.directory.0.join("operator-stop.json");
    // The operational stop lives outside the checkpoint authority inventory.
    fs::write(&stop_path, br#"{"reason":"host_memory_reserve","ram_guard":{"host_wide":true,"own_memory_signal":false}}"#).unwrap();
    let mut restored = fixture.open().unwrap();
    let identity = fixture.identity();
    let gate = (Mutex::new(false), Condvar::new());
    let entered = AtomicBool::new(false);
    let returned = AtomicBool::new(false);
    let calls = AtomicUsize::new(0);
    let (saved, receiving_save) = mpsc::channel();
    let latest = fixture.directory.0.join(publication::LATEST);
    std::thread::scope(|scope| {
        let observed_gate = &gate;
        let observed_returned = &returned;
        let observer = scope.spawn(move || {
            let generation = receiving_save
                .recv_timeout(Duration::from_secs(10))
                .unwrap();
            assert!(!observed_returned.load(Ordering::Acquire));
            assert_eq!(
                publication::read_manifest(&latest).unwrap().generation,
                generation
            );
            *observed_gate.0.lock().unwrap() = true;
            observed_gate.1.notify_all();
        });
        let inspect = |bytes: &[u8], cancel: &AtomicBool| {
            calls.fetch_add(1, Ordering::Relaxed);
            let job = Job::<1>::decode(bytes).unwrap();
            if job.parent == 0 {
                // Invalid returned prefix: reaching P1 would poison the run.
                return vec![0xfe; 1024 * 1024];
            }
            assert_eq!(
                job.parent, 1,
                "third Reserved descriptor must remain queued"
            );
            entered.store(true, Ordering::Release);
            let (guard, timeout) = gate
                .1
                .wait_timeout_while(gate.0.lock().unwrap(), Duration::from_secs(10), |open| {
                    !*open
                })
                .unwrap();
            assert!(!timeout.timed_out() && *guard);
            assert!(cancel.load(Ordering::Acquire));
            returned.store(true, Ordering::Release);
            vec![0xfd; 1024 * 1024]
        };
        let mut observed_entry = 0;
        let outcome = controller::run_with(
            &mut restored,
            &identity,
            16,
            2,
            config(),
            &inspect,
            || {
                if entered.load(Ordering::Acquire) {
                    observed_entry += 1;
                }
                // Once worker1 entered, this one-worker sender has already
                // queued Started(0), Result(0), Started(1). Allow two further
                // polls before cancellation, regardless of earlier timeouts:
                // the invalid Result(0) is now definitely in the polled prefix.
                let cancel = AtomicBool::new(observed_entry >= 3);
                stop::requested(&cancel, Some(&stop_path))
            },
            |receipt, status| {
                assert_eq!(status.len(), 3);
                assert!(status[0].started && status[0].returned);
                assert!(status[1].started && !status[1].returned);
                assert!(!status[2].started && !status[2].returned);
                assert!(!returned.load(Ordering::Acquire));
                saved.send(receipt.generation).unwrap();
            },
        )
        .unwrap();
        assert_eq!(outcome, Outcome::Stopped(merge::StopReason::RamGuard));
        observer.join().unwrap();
    });
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert_eq!(restored.state.k, 0);
    assert_eq!(restored.records.total(), 0);
    drop(restored);
    let restored = fixture.open().unwrap();
    let context = restored.operational_stop.as_ref().unwrap();
    assert_eq!(context.reason.as_deref(), Some("host_memory_reserve"));
    assert!(!context.tree_attributed);
    assert_eq!(context.host_wide, Some(true));
    assert_eq!(restored.replay.len(), 3);
    for id in 0..3 {
        assert_eq!(
            restored.state.ledger.get(id).unwrap(),
            Entry6::Reserved(Default::default())
        );
    }
}

#[test]
fn controller_error_cut_is_saved_terminal_and_not_replayed() {
    let fixture = Fixture::new();
    fixture.save(3, 2);
    let mut restored = fixture.open().unwrap();
    let mut saves = 0;
    let inspect = |bytes: &[u8], _: &AtomicBool| {
        let job = Job::<1>::decode(bytes).unwrap();
        result(&job, job.parent == 0)
    };
    let outcome = controller::run_with(
        &mut restored,
        &fixture.identity(),
        16,
        3,
        config(),
        &inspect,
        || None,
        |_, _| saves += 1,
    )
    .unwrap();
    assert_eq!(outcome, Outcome::Stopped(merge::StopReason::ErrorStop));
    assert_eq!(saves, 1);
    assert_eq!(restored.state.k, 1);
    assert_eq!(restored.records.total(), 2);
    drop(restored);
    let mut restored = fixture.open().unwrap();
    assert!(restored.replay.is_empty());
    assert!(matches!(
        restored.state.ledger.get(0).unwrap(),
        Entry6::NativeError { .. }
    ));
    let Refill::Jobs(jobs) = restored.dispatch.refill(&mut restored.state, 16) else {
        panic!("remaining Pending")
    };
    assert_eq!(jobs.iter().map(|job| job.parent).collect::<Vec<_>>(), [2]);
}

#[test]
fn controller_c5_poison_prevents_old_generation_fallback_and_w1_is_refused() {
    let fixture = Fixture::new();
    fixture.save(3, 2);
    let mut restored = fixture.open().unwrap();
    let calls = AtomicUsize::new(0);
    let inspect = |_: &[u8], _: &AtomicBool| {
        calls.fetch_add(1, Ordering::Relaxed);
        Vec::new() // Impossible coordinator/native protocol: P1 fatal, not C2.
    };
    assert!(
        controller::run_with(
            &mut restored,
            &fixture.identity(),
            16,
            1,
            config(),
            &inspect,
            || None,
            |_, _| panic!("W1 must refuse before saving")
        )
        .is_err()
    );
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert_eq!(restored.replay.len(), 2);
    assert!(
        controller::run_with(
            &mut restored,
            &fixture.identity(),
            16,
            2,
            config(),
            &inspect,
            || None,
            |_, _| panic!("C5 cannot save a generation")
        )
        .is_err()
    );
    assert_eq!(
        publication::read_manifest(&fixture.directory.0.join(publication::LATEST))
            .unwrap()
            .generation,
        1
    );
    assert!(fixture.directory.0.join("epoch-poison").is_file());
    drop(restored);
    assert!(fixture.open().is_err());
}

#[test]
fn controller_replay_finishes_before_new_pending_and_drains_through_real_merges() {
    let fixture = Fixture::new();
    fixture.save(3, 2);
    let mut restored = fixture.open().unwrap();
    let seen = Mutex::new(Vec::new());
    let inspect = |bytes: &[u8], _: &AtomicBool| {
        let job = Job::<1>::decode(bytes).unwrap();
        seen.lock().unwrap().push((job.parent, job.v0));
        result(&job, false)
    };
    assert_eq!(
        controller::run_with(
            &mut restored,
            &fixture.identity(),
            16,
            2,
            config(),
            &inspect,
            || None,
            |_, _| {}
        )
        .unwrap(),
        Outcome::Drained
    );
    assert_eq!(*seen.lock().unwrap(), [(0, 0), (1, 0), (2, 1)]);
    drop(restored);
    let restored = fixture.open().unwrap();
    assert_eq!(restored.state.k, 2);
    assert_eq!(restored.records.total(), 3);
    assert!(restored.replay.is_empty());
    assert_eq!(restored.state.pending_or_reserved(), 0);
}

#[test]
fn private_native_controller_checks_actual_worker_license_and_saves_valid_state() {
    use crate::application::routed_campaign::walking::{
        epoch::inspector::Context, initial_overlap::InitialOverlapIndex,
    };
    let mut fixture = Fixture::new();
    fixture.request.workers = 2;
    fixture.save(3, 2);
    let mut restored = fixture.open().unwrap();
    let identity = fixture.identity();
    let overlap = InitialOverlapIndex::empty();
    let cancellation = AtomicBool::new(false);
    let context = Context {
        reducer: &fixture.reducer,
        request: &fixture.request,
        overlap: &overlap,
        cancellation: &cancellation,
        g2: None,
        finite_account: None,
    };
    let worker_authorized = std::thread::spawn(symbolica::license::LicenseManager::is_licensed)
        .join()
        .unwrap();
    let mut saves = 0;
    let result = controller::run_native(&mut restored, &identity, 16, &context, |_, _| saves += 1);
    if worker_authorized {
        assert!(result.is_ok(), "licensed native path: {result:?}");
        assert_eq!(saves, 1);
        drop(restored);
        assert!(fixture.open().is_ok(), "actual native records must restore");
    } else {
        assert!(result.is_err(), "unlicensed worker must not enter CAS");
        assert_eq!(saves, 0);
        assert!(!fixture.directory.0.join("epoch-poison").exists());
        assert_eq!(restored.replay.len(), 2);
        drop(restored);
        assert!(
            fixture.open().is_ok(),
            "worker capability refusal must leave prior checkpoint usable"
        );
    }
}

#[test]
fn worker_capability_refusal_precedes_dispatch_and_allows_changed_capability_restore() {
    let fixture = Fixture::new();
    fixture.save(3, 2);
    let mut restored = fixture.open().unwrap();
    let caller = std::thread::current().id();
    let entered_cas = AtomicUsize::new(0);
    let inspect = |bytes: &[u8], _: &AtomicBool| {
        entered_cas.fetch_add(1, Ordering::Relaxed);
        result(&Job::<1>::decode(bytes).unwrap(), false)
    };
    let refused = controller::run_authorized(
        &mut restored,
        &fixture.identity(),
        16,
        3,
        config(),
        &|| {
            assert_ne!(
                std::thread::current().id(),
                caller,
                "capability must be checked on the worker"
            );
            Err("synthetic missing worker capability".into())
        },
        &inspect,
        || None,
        |_, _| panic!("refusal must not save or dispatch"),
    );
    assert_eq!(refused.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(entered_cas.load(Ordering::Relaxed), 0);
    assert_eq!(restored.replay.len(), 2);
    assert_eq!(restored.state.k, 0);
    assert!(!fixture.directory.0.join("epoch-poison").exists());
    assert_eq!(
        publication::read_manifest(&fixture.directory.0.join(publication::LATEST))
            .unwrap()
            .generation,
        1
    );
    drop(restored);
    let mut restored = fixture.open().unwrap();
    assert_eq!(restored.dispatch.checkpoint_snapshot().session, 3);
    assert_eq!(
        controller::run_authorized(
            &mut restored,
            &fixture.identity(),
            16,
            3,
            config(),
            &|| Ok(()),
            &inspect,
            || None,
            |_, _| {}
        )
        .unwrap(),
        Outcome::Drained
    );
    assert_eq!(entered_cas.load(Ordering::Relaxed), 3);
    drop(restored);
    assert!(fixture.open().unwrap().replay.is_empty());
}

#[test]
fn complete_input_frontiers_keep_existing_startup_policy_gate() {
    assert_eq!(
        controller::initial_stop(config(), 1),
        Some(merge::StopReason::FrontierStop)
    );
    assert_eq!(controller::initial_stop(config(), 0), None);
    let mut record = config();
    record.frontier_stop = false;
    assert_eq!(controller::initial_stop(record, 1), None);
}

#[test]
fn pre_dispatch_handle_and_thread_resource_refusals_do_not_poison() {
    use crate::application::routed_campaign::walking::epoch::inspector::RunError;
    let fixture = Fixture::new();
    fixture.save(3, 2);
    let mut restored = fixture.open().unwrap();
    for reason in [
        "epoch worker handle allocation",
        "epoch inspector spawn: injected refusal",
    ] {
        let error = controller::pool_error(&mut restored, RunError::Resource(reason.into()));
        assert!(error.to_string().contains(reason));
        assert_eq!(restored.replay.len(), 2);
        assert_eq!(restored.state.k, 0);
        assert!(!fixture.directory.0.join("epoch-poison").exists());
        assert_eq!(
            publication::read_manifest(&fixture.directory.0.join(publication::LATEST))
                .unwrap()
                .generation,
            1
        );
    }
    drop(restored);
    assert_eq!(fixture.open().unwrap().replay.len(), 2);
}
