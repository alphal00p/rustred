use super::*;

fn complete(pool: &Pool<1>, slot: usize, id: usize, count: usize) {
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
    if count != 0 {
        assert!(
            emitter
                .emit(Event {
                    count,
                    effect: Effect::Count
                })
                .is_continue()
        );
    }
    assert!(emitter.flush());
    let mut done = finished(None);
    if let NativeStats::Apply(s) = &mut done.stats {
        s.native_operations = id + 1;
    }
    pool.finish(slot, done);
}

#[test]
fn completed_escrow_moves_ownership_once_and_refills_past_finished_slots() {
    let pool = Pool::<1>::with_limits(
        2,
        EscrowLimits {
            entries: 8,
            bytes: 1 << 20,
        },
    );
    assert!(pool.dispatch(0, domain(0)));
    for id in 1..=5 {
        assert!(pool.dispatch(id, domain(id as u64)));
        complete(&pool, 1, id, id + 1);
        let before = pool.snapshot();
        pool.reclaim_finished(0);
        let after = pool.snapshot();
        assert_eq!(
            after["worker_buffered_events"],
            before["worker_buffered_events"]
        );
        assert_eq!(
            after["attempted_native_operations"],
            before["attempted_native_operations"]
        );
        assert_eq!(after["completed_escrow_entries"], id);
        assert_eq!(after["finished_uncommitted_domains"], id);
        assert_eq!(after["dispatched_uncommitted_domains"], id + 1);
        assert!(matches!(pool.poll(0), Poll::Waiting));
    }
    // Initial pending domain still has to run and finish; escrow is not closure.
    complete(&pool, 0, 0, 1);
    for id in 0..=5 {
        let Poll::Events(events) = pool.poll(id) else {
            panic!("ordered events")
        };
        assert_eq!(events.iter().map(|e| e.count).sum::<usize>(), id + 1);
        let Poll::Finished(done) = pool.poll(id) else {
            panic!("ordered terminal")
        };
        assert_eq!(done.native_operations(), id + 1);
    }
    let s = pool.snapshot();
    assert_eq!(s["returned_inspections"], 6);
    assert_eq!(s["attempted_native_operations"], 21);
    assert_eq!(s["attempted_events"], 21);
    assert_eq!(s["completed_slots_reclaimed"], 5);
    assert_eq!(s["completed_escrow_accounted_bytes"], 0);
    assert_eq!(s["worker_buffered_events"], 0);
    assert_eq!(s["worker_buffered_logical_bytes"], 0);
    assert!(pool.uncommitted().is_empty());
}

#[test]
fn completed_escrow_disabled_reference_has_identical_ordered_stream_and_totals() {
    let run = |entries| {
        let pool = Pool::<1>::with_limits(
            2,
            EscrowLimits {
                entries,
                bytes: 1 << 20,
            },
        );
        let mut dispatched = 0;
        let mut committed = Vec::new();
        for publisher in 0..6 {
            pool.reclaim_finished(publisher);
            while dispatched < 6 && pool.dispatch(dispatched, domain(dispatched as u64)) {
                let slot = pool
                    .lock()
                    .slots
                    .iter()
                    .position(|s| s.id == Some(dispatched))
                    .unwrap();
                complete(&pool, slot, dispatched, dispatched + 1);
                dispatched += 1;
                pool.reclaim_finished(publisher);
            }
            let Poll::Events(chunk) = pool.poll(publisher) else {
                panic!("ordered stream")
            };
            let Poll::Finished(done) = pool.poll(publisher) else {
                panic!("ordered terminal")
            };
            committed.push((
                publisher,
                chunk.iter().map(|e| e.count).sum::<usize>(),
                done.native_operations(),
            ));
        }
        assert!(pool.uncommitted().is_empty());
        (committed, pool.snapshot())
    };
    let (off, off_stats) = run(0);
    let (on, on_stats) = run(8);
    assert_eq!(off, on);
    for key in [
        "attempted_events",
        "returned_inspections",
        "attempted_native_operations",
        "worker_buffered_events",
        "finished_uncommitted_domains",
        "dispatched_uncommitted_domains",
    ] {
        assert_eq!(off_stats[key], on_stats[key], "{key}");
    }
    assert_eq!(off_stats["completed_escrow_peak_entries"], 0);
    assert_eq!(on_stats["completed_escrow_peak_entries"], 5);
}

#[test]
fn completed_escrow_entry_cap_bounds_empty_jobs_and_recovers_after_poll() {
    let pool = Pool::<1>::with_limits(
        2,
        EscrowLimits {
            entries: 2,
            bytes: 1 << 20,
        },
    );
    assert!(pool.dispatch(0, domain(0)));
    for id in 1..=3 {
        assert!(pool.dispatch(id, domain(id as u64)));
        complete(&pool, 1, id, 0);
        pool.reclaim_finished(0);
    }
    assert_eq!(pool.snapshot()["completed_escrow_entries"], 2);
    assert!(!pool.dispatch(4, domain(4)));
    assert!(matches!(pool.poll(1), Poll::Finished(_)));
    pool.reclaim_finished(0);
    assert!(pool.dispatch(4, domain(4)));
    assert_eq!(pool.snapshot()["completed_escrow_entries"], 2);
    assert!(
        pool.snapshot()["completed_escrow_accounted_bytes"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(pool.snapshot()["worker_buffered_events"], 0);
    pool.clear_buffers();
    let mut leftovers = pool.uncommitted();
    leftovers.sort_by_key(|(id, _)| *id);
    assert_eq!(
        leftovers.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        vec![2, 3]
    );
}

#[test]
fn completed_escrow_byte_cap_and_optional_reserve_fallback_leave_slot_intact() {
    for reserve_fallback in [false, true] {
        let pool = Pool::<1>::with_limits(
            2,
            EscrowLimits {
                entries: 8,
                bytes: 1 << 20,
            },
        );
        pool.dispatch(0, domain(0));
        pool.dispatch(1, domain(1));
        complete(&pool, 1, 1, 7);
        let bytes = {
            let mut s = pool.lock();
            let charge = Escrow::charge(&s.slots[1]).unwrap();
            let chunk = s.slots[1].chunk.as_ref().unwrap();
            // Actual Vec spare capacity participates in the admission charge.
            assert!(charge.bytes >= chunk.capacity() * size_of::<Event<1>>());
            s.escrow.limits.bytes = if reserve_fallback {
                charge.bytes
            } else {
                charge.bytes - 1
            };
            s.escrow.reserve_failed = reserve_fallback;
            charge.bytes
        };
        pool.reclaim_finished(0);
        assert_eq!(pool.snapshot()["completed_escrow_entries"], 0);
        assert!(!pool.dispatch(2, domain(2)));
        assert_eq!(pool.snapshot()["worker_buffered_events"], 7);
        if !reserve_fallback {
            pool.lock().escrow.limits.bytes = bytes;
            pool.reclaim_finished(0);
            assert_eq!(pool.snapshot()["completed_escrow_accounted_bytes"], bytes);
        }
        assert!(matches!(pool.poll(1), Poll::Events(_)));
        assert!(matches!(pool.poll(1), Poll::Finished(_)));
        assert_eq!(pool.snapshot()["worker_buffered_events"], 0);
        assert_eq!(pool.snapshot()["attempted_native_operations"], 2);
    }
}

#[test]
fn completed_escrow_stop_does_not_reclaim_or_lose_uncommitted_results() {
    let pool = Pool::<1>::new(2);
    pool.dispatch(0, domain(0));
    pool.dispatch(1, domain(1));
    complete(&pool, 1, 1, 3);
    pool.reclaim_finished(0);
    pool.dispatch(2, domain(2));
    complete(&pool, 1, 2, 4);
    pool.fail(Failure {
        id: Some(0),
        phase: Some(Phase::Apply),
        kind: "cancelled",
        detail: "cancelled".into(),
    });
    pool.reclaim_finished(0);
    assert_eq!(pool.snapshot()["completed_slots_reclaimed"], 1);
    assert!(!pool.dispatch(3, domain(3)));
    pool.clear_buffers();
    assert_eq!(pool.snapshot()["worker_buffered_events"], 0);
    assert_eq!(pool.snapshot()["worker_buffered_logical_bytes"], 0);
    assert_eq!(pool.snapshot()["completed_escrow_events"], 0);
    let leftovers = pool.uncommitted();
    assert_eq!(leftovers.len(), 2);
    assert_eq!(
        leftovers
            .iter()
            .map(|(_, f)| f.native_operations())
            .sum::<usize>(),
        5
    );
    assert_eq!(pool.snapshot()["attempted_native_operations"], 5);
    assert_eq!(pool.snapshot()["completed_escrow_accounted_bytes"], 0);
}

#[test]
fn completed_escrow_threaded_refill_keeps_canonical_event_order() {
    if !licensed() {
        return;
    }
    let release = AtomicBool::new(false);
    let (counts, snapshot, leftovers) = with_pool_limits(
        2,
        None,
        EscrowLimits {
            entries: 8,
            bytes: 1 << 20,
        },
        |d, stop, emit| {
            if d.lower[0] == 0 {
                while !release.load(Ordering::Acquire) && !stop.load(Ordering::Acquire) {
                    std::thread::yield_now();
                }
            }
            if emit(Event {
                count: d.lower[0] as usize + 1,
                effect: Effect::Count,
            })
            .is_break()
            {
                return finished(Some("cancelled"));
            }
            finished(None)
        },
        |pool| {
            pool.dispatch(0, domain(0));
            for id in 1..=5 {
                assert!(pool.dispatch(id, domain(id as u64)));
                wait_until(pool, |s| s["returned_inspections"] == id);
                pool.reclaim_finished(0);
            }
            assert_eq!(pool.snapshot()["completed_escrow_entries"], 5);
            assert!(matches!(pool.poll(0), Poll::Waiting));
            release.store(true, Ordering::Release);
            let mut counts = [0; 6];
            for id in 0..6 {
                loop {
                    match pool.poll(id) {
                        Poll::Events(events) => {
                            counts[id] += events.iter().map(|e| e.count).sum::<usize>()
                        }
                        Poll::Finished(_) => break,
                        Poll::Waiting => pool.wait(id),
                    }
                }
            }
            counts
        },
    );
    assert_eq!(counts, [1, 2, 3, 4, 5, 6]);
    assert_eq!(snapshot["returned_inspections"], 6);
    assert_eq!(snapshot["attempted_events"], 21);
    assert_eq!(snapshot["completed_escrow_entries"], 0);
    assert_eq!(snapshot["active_workers"], 0);
    assert!(leftovers.is_empty());
}

#[test]
fn completed_escrow_later_native_failure_stays_immediate_and_returns_all_attempts() {
    if !licensed() {
        return;
    }
    let (_, snapshot, mut leftovers) = with_pool(
        2,
        |d, stop, emit| match d.lower[0] {
            0 => {
                while !stop.load(Ordering::Acquire) {
                    std::thread::yield_now();
                }
                let mut f = finished(Some("peer cancelled"));
                f.error_kind = "cancelled";
                f
            }
            2 => finished(Some("later actual refusal")),
            _ => {
                let _ = emit(count());
                finished(None)
            }
        },
        |pool| {
            pool.dispatch(0, domain(0));
            pool.dispatch(1, domain(1));
            wait_until(pool, |s| {
                s["returned_inspections"] == 1 && s["active_workers"] == 1
            });
            pool.reclaim_finished(0);
            assert!(pool.dispatch(2, domain(2)));
            wait_until(pool, |s| !s["first_failure"].is_null());
            assert_eq!(pool.failure().unwrap().id, Some(2));
            while !pool.wait_drained() {}
        },
    );
    leftovers.sort_by_key(|(id, _)| *id);
    assert_eq!(
        leftovers.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(snapshot["first_failure"]["detail"], "later actual refusal");
    assert_eq!(snapshot["returned_inspections"], 3);
    assert_eq!(snapshot["finished_uncommitted_domains"], 3);
    assert_eq!(snapshot["worker_buffered_events"], 0);
    assert_eq!(snapshot["completed_escrow_events"], 0);
}

#[test]
fn completed_escrow_observer_panic_drains_running_and_blocked_workers() {
    if !licensed() {
        return;
    }
    let living = AtomicUsize::new(0);
    struct Guard<'a>(&'a AtomicUsize);
    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::Relaxed);
        }
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_pool(
            2,
            |d, stop, emit| {
                living.fetch_add(1, Ordering::Relaxed);
                let _guard = Guard(&living);
                if d.lower[0] == 0 {
                    while !stop.load(Ordering::Acquire) {
                        std::thread::yield_now();
                    }
                } else if d.lower[0] == 2 {
                    for count in [CHUNK_EVENTS, CHUNK_EVENTS, 1] {
                        if emit(Event {
                            count,
                            effect: Effect::Count,
                        })
                        .is_break()
                        {
                            break;
                        }
                    }
                } else {
                    let _ = emit(count());
                }
                finished(None)
            },
            |pool| {
                pool.dispatch(0, domain(0));
                pool.dispatch(1, domain(1));
                wait_until(pool, |s| {
                    s["returned_inspections"] == 1 && s["active_workers"] == 1
                });
                pool.reclaim_finished(0);
                pool.dispatch(2, domain(2));
                wait_until(pool, |s| s["backpressured_workers"] == 1);
                assert_eq!(pool.snapshot()["completed_escrow_entries"], 1);
                panic!("intentional escrow observer panic");
            },
        )
    }));
    assert!(result.is_err());
    assert_eq!(living.load(Ordering::Relaxed), 0);
}
