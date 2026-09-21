use super::*;
use crate::solver::candidate_reduction::owner_test_support::{input, key, programs};
use std::sync::{Arc, Barrier};

fn fixture(
    limits: ReductionLimits,
    routing: super::super::super::RoutedCandidateLimits,
) -> RoutedCandidateReducer<1> {
    let family = Arc::new(crate::solver::tests::tadpole());
    let owner = input([true], Some(0), vec![], &[[1]]);
    RoutedCandidateReducer::try_new(programs(family, Some(0), vec![owner], limits), [], routing)
        .unwrap()
}

fn route(power: i64) -> Work<1> {
    Work::Route(key([power]))
}

fn drain(shared: &Shared<'_, 1>, expected: &[Work<1>]) {
    for node in expected {
        let actual = shared.take().unwrap();
        assert_eq!(&actual, node);
        shared.finish(actual, Ok(()));
    }
    assert!(shared.take().is_none());
}

#[test]
fn membership_preprobe_racing_absent_keys_commit_once() {
    let reducer = fixture(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = Shared::new(&reducer, 2, &cancel);
    let barrier = Barrier::new(2);
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let shared = &shared;
            let barrier = &barrier;
            scope.spawn(move || {
                let batch = shared.preprobe_batch(vec![route(2)]).unwrap();
                assert!(matches!(&batch.0[0], ProbedWork::NeedsCommit { .. }));
                barrier.wait();
                shared.commit_batch(batch).unwrap();
            });
        }
    });
    let snapshot = shared.snapshot();
    assert_eq!(snapshot.scheduled_nodes, 1);
    assert_eq!(snapshot.reachable_integrals, 1);
    assert_eq!(snapshot.queued_nodes, 1);
    assert_eq!(snapshot.deduplication_hits, 1);
    drain(&shared, &[route(2)]);
    assert!(shared.into_result().unwrap().snapshot().finished);
}

#[test]
fn membership_preprobe_racing_new_keys_cannot_overbook_global_limits() {
    for pending in [false, true] {
        let reducer = fixture(
            ReductionLimits {
                max_pending_frames: if pending { 1 } else { 10 },
                ..Default::default()
            },
            super::super::super::RoutedCandidateLimits {
                max_unique_nodes: if pending { 10 } else { 1 },
                ..Default::default()
            },
        );
        let cancel = AtomicBool::new(false);
        let shared = Shared::new(&reducer, 2, &cancel);
        let barrier = Barrier::new(2);
        let results = std::thread::scope(|scope| {
            let mut threads = Vec::new();
            for value in [1, 2] {
                let shared = &shared;
                let barrier = &barrier;
                threads.push(scope.spawn(move || {
                    let batch = shared.preprobe_batch(vec![route(value)]).unwrap();
                    barrier.wait();
                    shared.commit_batch(batch)
                }));
            }
            threads
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        let expected = resource_error(
            if pending {
                "pending nodes"
            } else {
                "operational nodes"
            },
            2,
            1,
        );
        assert_eq!(
            results
                .into_iter()
                .filter_map(Result::err)
                .collect::<Vec<_>>(),
            vec![expected]
        );
        assert_eq!(shared.snapshot().scheduled_nodes, 1);
        assert_eq!(shared.snapshot().reachable_integrals, 1);
        assert_eq!(shared.snapshot().queued_nodes, 1);
        assert_eq!(shared.snapshot().deduplication_hits, 0);
        let node = shared.take().unwrap();
        shared.finish(node, Ok(()));
    }
}

#[test]
fn membership_preprobe_same_batch_absent_duplicates_recheck_and_keep_fifo() {
    let reducer = fixture(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = Shared::new(&reducer, 1, &cancel);
    let batch = shared
        .preprobe_batch(vec![route(3), route(3), route(2)])
        .unwrap();
    assert!(
        batch
            .0
            .iter()
            .all(|item| matches!(item, ProbedWork::NeedsCommit { .. }))
    );
    shared.commit_batch(batch).unwrap();
    assert_eq!(shared.snapshot().scheduled_nodes, 2);
    assert_eq!(shared.snapshot().reachable_integrals, 2);
    assert_eq!(shared.snapshot().deduplication_hits, 1);
    drain(&shared, &[route(3), route(2)]);
}

#[test]
fn membership_preprobe_racing_route_and_apply_keep_both_phases_one_physical_key() {
    let reducer = fixture(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = Shared::new(&reducer, 2, &cancel);
    let barrier = Barrier::new(2);
    let apply = Work::Apply {
        owner_sector: [true],
        target: key([1]),
    };
    std::thread::scope(|scope| {
        for node in [route(1), apply.clone()] {
            let shared = &shared;
            let barrier = &barrier;
            scope.spawn(move || {
                let batch = shared.preprobe_batch(vec![node]).unwrap();
                assert!(matches!(&batch.0[0], ProbedWork::NeedsCommit { .. }));
                barrier.wait();
                shared.commit_batch(batch).unwrap();
            });
        }
    });
    shared
        .schedule_batch(vec![route(1), apply.clone()])
        .unwrap();
    let snapshot = shared.snapshot();
    assert_eq!(snapshot.scheduled_nodes, 2);
    assert_eq!(snapshot.reachable_integrals, 1);
    assert_eq!(snapshot.deduplication_hits, 2);
    let mut received = BTreeSet::new();
    for _ in 0..2 {
        let node = shared.take().unwrap();
        received.insert(node.clone());
        shared.finish(node, Ok(()));
    }
    assert_eq!(received, BTreeSet::from([route(1), apply]));
    assert!(shared.into_result().unwrap().snapshot().finished);
}

#[test]
fn membership_preprobe_known_unknown_cap_failure_keeps_exact_prefix() {
    for pending in [false, true] {
        let reducer = fixture(
            ReductionLimits {
                max_pending_frames: if pending { 5 } else { 100 },
                ..Default::default()
            },
            super::super::super::RoutedCandidateLimits {
                max_unique_nodes: if pending { 100 } else { 5 },
                ..Default::default()
            },
        );
        let cancel = AtomicBool::new(false);
        let shared = Shared::new(&reducer, 1, &cancel);
        shared
            .schedule_batch(vec![route(1), route(2), route(3), route(4)])
            .unwrap();
        let active = shared.take().unwrap();
        // The last known key must not be counted across the earlier cap error.
        let batch = shared
            .preprobe_batch(vec![route(1), route(5), route(2), route(6), route(3)])
            .unwrap();
        assert!(matches!(&batch.0[0], ProbedWork::KnownPresent));
        assert!(matches!(&batch.0[1], ProbedWork::NeedsCommit { .. }));
        assert!(matches!(&batch.0[2], ProbedWork::KnownPresent));
        let error = shared.commit_batch(batch).unwrap_err();
        assert_eq!(
            error,
            resource_error(
                if pending {
                    "pending nodes"
                } else {
                    "operational nodes"
                },
                6,
                5
            )
        );
        let snapshot = shared.snapshot();
        assert_eq!(snapshot.scheduled_nodes, 5);
        assert_eq!(snapshot.reachable_integrals, 5);
        assert_eq!(snapshot.queued_nodes, 4);
        assert_eq!(snapshot.active_nodes, 1);
        assert_eq!(snapshot.deduplication_hits, 2);
        assert!(!shared.membership.probe(&route(6)).1);
        // Even exhausted caps permit already-committed joins.
        shared.schedule_batch(vec![route(1), route(5)]).unwrap();
        assert_eq!(shared.snapshot().deduplication_hits, 4);
        shared.finish(active.clone(), Err(error.clone()));
        let result = shared.into_result().unwrap_err();
        assert_eq!(result.reason(), &error);
        assert_eq!(result.snapshot().first_failure_work.as_ref(), Some(&active));
        assert!(!result.snapshot().finished);
    }
}

#[test]
fn membership_preprobe_late_invalid_owner_and_arity_preserve_prefix() {
    for invalid in [
        Work::Apply {
            owner_sector: [false],
            target: key([1]),
        },
        Work::Route(key([1, 1])),
    ] {
        let reducer = fixture(Default::default(), Default::default());
        let cancel = AtomicBool::new(false);
        let shared = Shared::new(&reducer, 1, &cancel);
        shared.schedule(route(1)).unwrap();
        let batch = shared
            .preprobe_batch(vec![route(1), route(2), invalid, route(3)])
            .unwrap();
        assert_eq!(batch.0.len(), 3);
        assert!(matches!(&batch.0[2], ProbedWork::Invalid(_)));
        assert!(matches!(
            shared.commit_batch(batch),
            Err(Failure::Trace(CandidateRoutedError::InvalidInput(_)))
        ));
        assert_eq!(shared.snapshot().scheduled_nodes, 2);
        assert_eq!(shared.snapshot().reachable_integrals, 2);
        assert_eq!(shared.snapshot().deduplication_hits, 1);
        assert!(!shared.membership.probe(&route(3)).1);
        drain(&shared, &[route(1), route(2)]);
    }
}

#[test]
fn membership_preprobe_cancel_or_peer_failure_before_commit_changes_no_counts() {
    for cancellation in [false, true] {
        let reducer = fixture(Default::default(), Default::default());
        let cancel = AtomicBool::new(false);
        let shared = Shared::new(&reducer, 1, &cancel);
        shared.schedule(route(1)).unwrap();
        let active = shared.take().unwrap();
        let batch = shared.preprobe_batch(vec![route(1), route(2)]).unwrap();
        let error = if cancellation {
            cancel.store(true, Ordering::Release);
            Failure::Cancelled
        } else {
            let original = Failure::Trace(CandidateRoutedError::InvalidInput(
                "first peer error".into(),
            ));
            shared.fail(original.clone());
            original
        };
        assert_eq!(shared.commit_batch(batch), Err(error.clone()));
        let snapshot = shared.snapshot();
        assert_eq!(snapshot.scheduled_nodes, 1);
        assert_eq!(snapshot.reachable_integrals, 1);
        assert_eq!(snapshot.deduplication_hits, 0);
        assert!(!shared.membership.probe(&route(2)).1);
        shared.finish(active, Err(Failure::WorkerPanicked));
        assert_eq!(shared.into_result().unwrap_err().reason(), &error);
    }
}

#[test]
fn membership_preprobe_duplicate_counter_overflow_keeps_exact_committed_prefix() {
    let reducer = fixture(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = Shared::new(&reducer, 1, &cancel);
    shared.schedule(route(1)).unwrap();
    let batch = shared
        .preprobe_batch(vec![route(1), route(1), route(2)])
        .unwrap();
    shared.lock().dedup = usize::MAX - 1;
    assert_eq!(
        shared.commit_batch(batch),
        Err(resource_error(
            "deduplication counter",
            usize::MAX,
            usize::MAX
        ))
    );
    assert_eq!(shared.snapshot().deduplication_hits, usize::MAX);
    assert_eq!(shared.snapshot().scheduled_nodes, 1);
    assert!(!shared.membership.probe(&route(2)).1);
    drain(&shared, &[route(1)]);
}

#[test]
fn membership_preprobe_batch_bound_applies_even_to_all_known_keys() {
    let reducer = fixture(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = Shared::new(&reducer, 1, &cancel);
    shared.schedule(route(1)).unwrap();
    assert!(
        shared
            .preprobe_batch(vec![route(1); PUBLICATION_BATCH_SIZE + 1])
            .is_err()
    );
    assert_eq!(shared.snapshot().scheduled_nodes, 1);
    assert_eq!(shared.snapshot().deduplication_hits, 0);
}

#[test]
fn membership_preprobe_does_not_hold_global_lock_and_probe_guard_is_released() {
    let reducer = fixture(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = Shared::new(&reducer, 1, &cancel);
    shared.schedule(route(1)).unwrap();
    // A regression must fail without leaving the test suite deadlocked.
    let guard = shared.lock();
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    std::thread::scope(|scope| {
        let shared = &shared;
        scope.spawn(move || {
            send.send(shared.preprobe_batch(vec![route(1), route(2)]).unwrap())
                .unwrap();
        });
        let result = receive.recv_timeout(Duration::from_secs(5));
        drop(guard);
        let batch = result.expect("preprobe must not wait for the global state lock");
        // Its shard guard was dropped: ordinary ordered commit reacquires it.
        shared.commit_batch(batch).unwrap();
    });
    assert_eq!(shared.snapshot().deduplication_hits, 1);
    drain(&shared, &[route(1), route(2)]);
}
