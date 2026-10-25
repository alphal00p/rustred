//! Adversarial mixed-waiter tests: work and algebra reservations must not
//! consume each other's notifications, and every global stop wakes both.
use super::membership_tests::{fixture, route};
use super::*;
use std::sync::mpsc;

fn await_waiters(shared: &Shared<'_, 1>, work: usize, reservation: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let state = shared.lock();
        let ready = state.work_waiters == work && state.reservation_waiters == reservation;
        drop(state);
        if ready {
            return;
        }
        if Instant::now() >= deadline {
            shared.fail(Failure::Cancelled);
            panic!("scheduler did not reach expected mixed waiter state");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn separate_work_and_reservation_waiters_receive_their_own_progress() {
    let reducer = fixture(
        ReductionLimits {
            diagnostics_enabled: true,
            max_coalescing_additions: 1,
            ..Default::default()
        },
        Default::default(),
    );
    let cancel = AtomicBool::new(false);
    let shared = Shared::new(&reducer, 3, &cancel);
    shared.schedule(route(1)).unwrap();
    let first = shared.take().unwrap();
    shared.reserve_apply(1).unwrap();
    std::thread::scope(|scope| {
        let (work_send, work_receive) = mpsc::channel();
        let (grant_send, grant_receive) = mpsc::channel();
        let shared = &shared;
        scope.spawn(move || {
            let next = shared.take().expect("published queue work");
            work_send.send(next.clone()).unwrap();
            shared.finish(next, Ok(()));
        });
        scope.spawn(move || {
            shared.reserve_apply(1).unwrap();
            grant_send.send(()).unwrap();
            shared.settle_apply(1, 0, true).unwrap();
        });
        await_waiters(shared, 1, 1);
        shared.schedule(route(2)).unwrap();
        let received = work_receive.recv_timeout(Duration::from_secs(5));
        if received.is_err() {
            shared.fail(Failure::Cancelled);
        }
        assert_eq!(received.unwrap(), route(2));
        assert!(
            grant_receive.try_recv().is_err(),
            "queue work cannot grant algebra capacity"
        );
        shared.settle_apply(1, 0, true).unwrap();
        let granted = grant_receive.recv_timeout(Duration::from_secs(5));
        if granted.is_err() {
            shared.fail(Failure::Cancelled);
        }
        granted.unwrap();
        shared.finish(first, Ok(()));
    });
    assert_eq!(shared.lock().work_waiters, 0);
    assert_eq!(shared.lock().reservation_waiters, 0);
    assert!(shared.into_result().unwrap().snapshot().finished);
}

#[test]
fn cancellation_and_release_failure_drain_both_waiter_classes() {
    for release_failure in [false, true] {
        let reducer = fixture(
            ReductionLimits {
                diagnostics_enabled: true,
                max_coalescing_additions: 1,
                ..Default::default()
            },
            Default::default(),
        );
        let cancel = AtomicBool::new(false);
        let shared = Shared::new(&reducer, 3, &cancel);
        shared.schedule(route(1)).unwrap();
        let first = shared.take().unwrap();
        shared.reserve_apply(1).unwrap();
        std::thread::scope(|scope| {
            let shared = &shared;
            let work = scope.spawn(|| shared.take());
            let reservation = scope.spawn(|| shared.reserve_apply(1));
            await_waiters(shared, 1, 1);
            if release_failure {
                let failure = shared.settle_apply(1, 2, false).unwrap_err();
                shared.finish(first, Err(failure));
            } else {
                cancel.store(true, Ordering::Release);
                shared.check().unwrap_err();
                shared.settle_apply(1, 0, false).unwrap();
                shared.finish(first, Err(Failure::Cancelled));
            }
            assert!(work.join().unwrap().is_none());
            // A released grant may race the failure publication. Either it
            // sees the typed failure or obtains a valid grant to settle.
            if reservation.join().unwrap().is_ok() {
                shared.settle_apply(1, 0, false).unwrap();
            }
        });
        assert_eq!(shared.lock().work_waiters, 0);
        assert_eq!(shared.lock().reservation_waiters, 0);
        assert_eq!(shared.snapshot().active_nodes, 0);
        assert!(shared.into_result().is_err());
    }
}
