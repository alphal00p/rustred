use std::collections::BTreeSet;

use super::*;

fn complete(pool: &Pool<1>, slot: usize, id: usize, events: usize) {
    assert_eq!(pool.take(slot).unwrap().0, id);
    let mut emitter = Emitter {
        pool,
        slot,
        id,
        phase: Phase::Apply,
        chunk: Vec::new(),
        bytes: 0,
        events: 0,
    };
    if events != 0 {
        assert!(
            emitter
                .emit(Event {
                    count: events,
                    effect: Effect::Count
                })
                .is_continue()
        );
    }
    assert!(emitter.flush());
    pool.finish(slot, finished(None));
}

#[test]
fn owner_retention_protects_each_head_without_global_ticket_ordering() {
    let pool = Pool::<1>::new(3);
    for (slot, id) in [70, 3, 1].into_iter().enumerate() {
        assert!(pool.dispatch(id, domain(id as u64)));
        complete(&pool, slot, id, 2);
    }
    let heads = BTreeSet::from([70, 3]);
    assert_eq!(pool.reclaim_completed_except(&heads), [1]);
    assert_eq!(pool.snapshot()["completed_escrow_entries"], 1);
    assert_eq!(pool.snapshot()["worker_buffered_events"], 6);
    assert_eq!(pool.snapshot()["returned_inspections"], 3);
    assert!(pool.reclaim_completed_except(&heads).is_empty());
    assert!(pool.dispatch(72, domain(72)));
    assert!(pool.wait_for_owner_progress(&BTreeSet::from([1])));
    for id in [1, 3, 70] {
        assert!(matches!(pool.poll(id), Poll::Events(_)));
        assert!(matches!(pool.poll(id), Poll::Finished(_)));
    }
    assert_eq!(pool.snapshot()["worker_buffered_events"], 0);
    assert_eq!(pool.snapshot()["completed_escrow_accounted_bytes"], 0);
}

#[test]
fn owner_retention_wait_distinguishes_refill_progress_from_full_store() {
    let pool = Pool::<1>::with_limits(
        2,
        EscrowLimits {
            entries: 0,
            bytes: 1 << 20,
        },
    );
    assert!(pool.dispatch(0, domain(0)));
    assert!(pool.dispatch(1, domain(1)));
    complete(&pool, 1, 1, 2);
    let heads = BTreeSet::from([0]);
    // Buffered later data cannot be published and the store cannot accept it.
    assert!(!pool.wait_for_owner_progress(&heads));
    assert!(pool.reclaim_completed_except(&heads).is_empty());
    pool.lock().escrow.limits.entries = 1;
    assert!(pool.wait_for_owner_progress(&heads));
    assert_eq!(pool.reclaim_completed_except(&heads), [1]);
    assert!(!pool.wait_for_owner_progress(&heads));
    assert!(pool.wait_for_owner_progress(&BTreeSet::from([1])));
    assert!(matches!(pool.poll(1), Poll::Events(_)));
    assert!(pool.wait_for_owner_progress(&BTreeSet::from([1])));
    assert!(matches!(pool.poll(1), Poll::Finished(_)));
}

#[test]
fn owner_retention_skips_oversized_result_but_reclaims_another_fitting_slot() {
    let pool = Pool::<1>::new(3);
    assert!(pool.dispatch(0, domain(0)));
    assert!(pool.dispatch(1, domain(1)));
    assert!(pool.dispatch(2, domain(2)));
    complete(&pool, 1, 1, 1);
    complete(&pool, 2, 2, 0);
    {
        let mut state = pool.lock();
        let small = Escrow::charge(&state.slots[2]).unwrap().bytes;
        assert!(Escrow::charge(&state.slots[1]).unwrap().bytes > small);
        state.escrow.limits.bytes = small;
    }
    let heads = BTreeSet::from([0]);
    assert!(pool.wait_for_owner_progress(&heads));
    assert_eq!(pool.reclaim_completed_except(&heads), [2]);
    assert!(!pool.wait_for_owner_progress(&heads));
    assert!(matches!(pool.poll(2), Poll::Finished(_)));
    assert!(matches!(pool.poll(1), Poll::Events(_)));
    assert!(matches!(pool.poll(1), Poll::Finished(_)));
}

#[test]
fn owner_retention_never_detaches_an_unfinished_final_flush_or_failed_result() {
    let pool = Pool::<1>::new(2);
    assert!(pool.dispatch(0, domain(0)));
    assert!(pool.dispatch(1, domain(1)));
    assert_eq!(pool.take(1).unwrap().0, 1);
    let mut emitter = Emitter {
        pool: &pool,
        slot: 1,
        id: 1,
        phase: Phase::Apply,
        chunk: Vec::new(),
        bytes: 0,
        events: 0,
    };
    assert!(emitter.emit(count()).is_continue());
    assert!(emitter.flush());
    // The final chunk exists but the worker has not returned from inspection.
    let heads = BTreeSet::from([0]);
    assert!(pool.reclaim_completed_except(&heads).is_empty());
    assert!(!pool.wait_for_owner_progress(&heads));
    pool.finish(1, finished(Some("failed after flush")));
    assert!(pool.reclaim_completed_except(&heads).is_empty());
    assert!(!pool.wait_for_owner_progress(&heads));
    assert!(matches!(pool.poll(1), Poll::Events(_)));
    let Poll::Finished(value) = pool.poll(1) else {
        panic!("failure retained")
    };
    assert_eq!(value.error.as_deref(), Some("failed after flush"));
}

#[test]
fn owner_retention_checked_accounting_overflow_declines_without_mutation() {
    let pool = Pool::<1>::new(2);
    assert!(pool.dispatch(0, domain(0)));
    assert!(pool.dispatch(1, domain(1)));
    complete(&pool, 1, 1, 1);
    for counter in ["bytes", "payload", "events", "reclaimed"] {
        {
            let mut state = pool.lock();
            match counter {
                "bytes" => state.escrow.bytes = usize::MAX,
                "payload" => state.escrow.payload = usize::MAX,
                "events" => state.escrow.events = usize::MAX,
                _ => state.escrow.reclaimed = usize::MAX,
            }
        }
        assert!(
            pool.reclaim_completed_except(&BTreeSet::from([0]))
                .is_empty()
        );
        let mut state = pool.lock();
        assert_eq!(state.slots[1].id, Some(1));
        assert_eq!(state.escrow.len(), 0);
        state.escrow.bytes = 0;
        state.escrow.payload = 0;
        state.escrow.events = 0;
        state.escrow.reclaimed = 0;
    }
    // Charge construction itself is checked, including summed logical events.
    let slot = Slot::<1> {
        id: Some(2),
        finished: Some(finished(None)),
        chunk: Some(vec![
            Event {
                count: usize::MAX,
                effect: Effect::Count,
            },
            count(),
        ]),
        ..Default::default()
    };
    assert!(Escrow::charge(&slot).is_none());
    assert_eq!(pool.reclaim_completed_except(&BTreeSet::from([0])), [1]);
    assert!(matches!(pool.poll(1), Poll::Events(_)));
    assert!(matches!(pool.poll(1), Poll::Finished(_)));
}

#[test]
fn owner_retention_sticky_allocation_fallback_does_not_report_refill_readiness() {
    let pool = Pool::<1>::new(2);
    assert!(pool.dispatch(0, domain(0)));
    assert!(pool.dispatch(1, domain(1)));
    complete(&pool, 1, 1, 0);
    // Simulate the sticky state shared by entry and return-vector allocation
    // fallback; this is not a real allocator failure injection.
    pool.lock().escrow.reserve_failed = true;
    let heads = BTreeSet::from([0]);
    assert!(pool.reclaim_completed_except(&heads).is_empty());
    assert!(!pool.wait_for_owner_progress(&heads));
    assert!(pool.wait_for_owner_progress(&BTreeSet::from([1])));
    assert!(matches!(pool.poll(1), Poll::Finished(_)));
}
