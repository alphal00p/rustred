use super::*;
use crate::application::routed_campaign::walking::inspection::NativeStats;

fn domain(n: u64) -> Arc<Domain<1>> {
    Arc::new(Domain {
        phase: Phase::Apply,
        owner: [true],
        lower: vec![n],
        upper: vec![None],
        rank: Some(11),
    })
}
fn finished(error: Option<&str>) -> Finished {
    Finished {
        stats: NativeStats::Apply(Default::default()),
        error: error.map(str::to_owned),
        error_kind: if error.is_some() {
            "native_failure"
        } else {
            "none"
        },
        seconds: 0.0,
    }
}
fn licensed() -> bool {
    symbolica::license::LicenseManager::is_licensed()
}
fn count() -> Event<1> {
    Event::one(Effect::Count)
}

#[test]
fn initial_orthants_compact_only_same_family_and_current_flags() {
    let pool = Pool::<1>::new(1);
    assert!(pool.dispatch(0, domain(0)));
    let mut emitter = Emitter {
        pool: &pool,
        slot: 0,
        id: 0,
        phase: Phase::Apply,
        chunk: Vec::new(),
        bytes: 0,
        events: 0,
    };
    for _ in 0..1000 {
        assert!(
            emitter
                .emit(Event::one(Effect::PreAdmittedOrthantReuse {
                    successor: true,
                    conditional: true,
                }))
                .is_continue()
        );
    }
    assert_eq!(emitter.chunk.len(), 1);
    assert_eq!(emitter.chunk[0].count, 1000);
    assert!(
        emitter
            .emit(Event::one(Effect::KnownReuse {
                successor: true,
                conditional: true
            }))
            .is_continue()
    );
    assert!(
        emitter
            .emit(Event::one(Effect::PreAdmittedOrthantReuse {
                successor: true,
                conditional: false
            }))
            .is_continue()
    );
    assert!(
        emitter
            .emit(Event::one(Effect::PreAdmittedOrthantReuse {
                successor: false,
                conditional: false
            }))
            .is_continue()
    );
    assert_eq!(emitter.chunk.len(), 4);
    assert_eq!(emitter.events, 1003);
    assert!(emitter.flush());
    let Poll::Events(chunk) = pool.poll(0) else {
        panic!("flushed marker chunk");
    };
    assert_eq!(chunk.iter().map(|event| event.count).sum::<usize>(), 1003);
    assert_eq!(chunk.len(), 4);
    assert_eq!(pool.snapshot()["worker_buffered_events"], 0);
    // Larger chunks must not defer cancellation to a flush boundary.
    pool.stop.store(true, Ordering::Release);
    assert!(
        emitter
            .emit(Event::one(Effect::PreAdmittedOrthantReuse {
                successor: true,
                conditional: true,
            }))
            .is_break()
    );
    assert_eq!(pool.snapshot()["worker_buffered_events"], 0);
}

#[test]
fn job_local_reuse_compaction_uses_physical_and_logical_limits_separately() {
    let pool = Pool::<1>::new(1);
    assert!(pool.dispatch(0, domain(0)));
    let mut emitter = Emitter {
        pool: &pool,
        slot: 0,
        id: 0,
        phase: Phase::Apply,
        chunk: Vec::new(),
        bytes: 0,
        events: 0,
    };
    let reused = || {
        Event::one(Effect::KnownReuse {
            successor: true,
            conditional: true,
        })
    };
    for _ in 0..1000 {
        assert!(emitter.emit(reused()).is_continue());
    }
    assert_eq!(emitter.chunk.len(), 1);
    assert_eq!(emitter.events, 1000);
    assert!(matches!(pool.poll(0), Poll::Waiting)); // no flush at64 logical callbacks
    for _ in 1000..CHUNK_EVENTS {
        assert!(emitter.emit(reused()).is_continue());
    }
    let Poll::Events(chunk) = pool.poll(0) else {
        panic!("logical bound must flush");
    };
    assert_eq!(chunk.len(), 1);
    assert_eq!(chunk[0].count, CHUNK_EVENTS);
    assert_eq!(pool.snapshot()["worker_buffered_events"], 0);
    // Alternating charge vectors cannot compact and hit the physical cap.
    assert!(CHUNK_RECORDS * size_of::<Event<1>>() < CHUNK_BYTES);
    for i in 0..=CHUNK_RECORDS {
        assert!(
            emitter
                .emit(Event::one(Effect::KnownReuse {
                    successor: true,
                    conditional: i % 2 == 0
                }))
                .is_continue()
        );
    }
    let Poll::Events(chunk) = pool.poll(0) else {
        panic!("physical bound must flush");
    };
    assert_eq!(chunk.len(), CHUNK_RECORDS);
    assert_eq!(emitter.chunk.len(), 1);
    assert!(emitter.flush());
    assert!(matches!(pool.poll(0), Poll::Events(_)));
}

#[test]
fn job_local_reuse_byte_bound_still_flushes_noncompact_records() {
    let pool = Pool::<1>::new(1);
    assert!(pool.dispatch(0, domain(0)));
    let mut emitter = Emitter {
        pool: &pool,
        slot: 0,
        id: 0,
        phase: Phase::Apply,
        chunk: Vec::new(),
        bytes: 0,
        events: 0,
    };
    let large = || {
        Event::one(Effect::Frontier {
            value: json!("x".repeat(CHUNK_BYTES * 3 / 4)),
            successor: false,
            conditional: false,
        })
    };
    assert!(emitter.emit(large()).is_continue());
    assert!(emitter.emit(large()).is_continue());
    let Poll::Events(chunk) = pool.poll(0) else {
        panic!("byte bound must flush");
    };
    assert_eq!(chunk.len(), 1);
    assert!(emitter.bytes <= CHUNK_BYTES);
    assert!(emitter.flush());
    assert!(matches!(pool.poll(0), Poll::Events(_)));
    assert_eq!(pool.snapshot()["worker_buffered_events"], 0);
}
fn wait_until(pool: &Pool<1>, predicate: impl Fn(&Value) -> bool) {
    let started = Instant::now();
    while !predicate(&pool.snapshot()) {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{}",
            pool.snapshot()
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn symbolic_parallel_stream_is_bounded_and_commits_in_requested_order() {
    if !licensed() {
        return;
    }
    let release = AtomicBool::new(false);
    let (counts, snapshot, leftovers) = with_pool(
        2,
        |d, stop, emit| {
            if d.lower[0] == 0 {
                while !release.load(Ordering::Acquire) && !stop.load(Ordering::Acquire) {
                    std::thread::yield_now();
                }
            }
            // Genuine full published/private chunks without redundant millions
            // of producer calls; the compaction test exercises count-one input.
            for count in [CHUNK_EVENTS, CHUNK_EVENTS, 1] {
                if emit(Event {
                    count,
                    effect: Effect::Count,
                })
                .is_break()
                {
                    return finished(Some("stopped"));
                }
            }
            finished(None)
        },
        |pool| {
            assert!(pool.dispatch(0, domain(0)));
            assert!(pool.dispatch(1, domain(1)));
            wait_until(pool, |s| s["backpressured_workers"] == 1);
            let s = pool.snapshot();
            assert!(s["worker_buffered_events"].as_u64().unwrap() <= (4 * CHUNK_EVENTS) as u64);
            assert!(
                s["worker_buffered_logical_bytes"].as_u64().unwrap() <= (4 * CHUNK_BYTES) as u64
            );
            release.store(true, Ordering::Release);
            let mut counts = [0; 2];
            for id in 0..2 {
                loop {
                    match pool.poll(id) {
                        Poll::Events(events) => {
                            counts[id] += events.iter().map(|e| e.count).sum::<usize>()
                        }
                        Poll::Finished(done) => {
                            assert!(done.error.is_none());
                            break;
                        }
                        Poll::Waiting => pool.wait(id),
                    }
                }
            }
            counts
        },
    );
    assert_eq!(counts, [2 * CHUNK_EVENTS + 1, 2 * CHUNK_EVENTS + 1]);
    assert!(leftovers.is_empty());
    assert_eq!(snapshot["attempted_events"], 4 * CHUNK_EVENTS + 2);
    assert_eq!(snapshot["returned_inspections"], 2);
    assert_eq!(snapshot["worker_buffered_events"], 0);
    assert_eq!(snapshot["active_workers"], 0);
}

#[test]
fn symbolic_parallel_later_failure_stops_and_preserves_first_cause() {
    if !licensed() {
        return;
    }
    let ready = std::sync::Barrier::new(2);
    let (_, snapshot, leftovers) = with_pool(
        2,
        |d, stop, emit| {
            ready.wait();
            if d.lower[0] == 1 {
                return finished(Some("actual native refusal"));
            }
            while !stop.load(Ordering::Acquire) {
                if emit(count()).is_break() {
                    break;
                }
            }
            let mut done = finished(Some("Cancelled"));
            done.error_kind = "cancelled";
            done
        },
        |pool| {
            assert!(pool.dispatch(0, domain(0)));
            assert!(pool.dispatch(1, domain(1)));
            wait_until(pool, |s| !s["first_failure"].is_null());
            while !pool.wait_drained() {}
        },
    );
    assert_eq!(snapshot["first_failure"]["domain"], 1);
    assert_eq!(snapshot["first_failure"]["detail"], "actual native refusal");
    assert!(leftovers.len() <= 2);
    assert_eq!(snapshot["active_workers"], 0);
}

#[test]
fn symbolic_parallel_observer_panic_unblocks_full_mailboxes_and_joins() {
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
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_pool(
            2,
            |_, _, emit| {
                living.fetch_add(1, Ordering::Relaxed);
                let _guard = Guard(&living);
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
                finished(None)
            },
            |pool| {
                pool.dispatch(0, domain(0));
                pool.dispatch(1, domain(1));
                wait_until(pool, |s| s["backpressured_workers"] == 2);
                panic!("intentional observer fixture");
            },
        )
    }));
    assert!(panic.is_err());
    assert_eq!(living.load(Ordering::Relaxed), 0);
}

#[test]
fn symbolic_parallel_worker_panic_and_spawn_failure_are_terminal() {
    if !licensed() {
        return;
    }
    let (_, snapshot, _) = with_pool(
        2,
        |_, _, _| panic!("intentional worker fixture"),
        |pool| {
            pool.dispatch(0, domain(0));
            wait_until(pool, |s| !s["first_failure"].is_null());
        },
    );
    assert_eq!(snapshot["first_failure"]["kind"], "worker_panic");
    let (_, snapshot, _) = with_pool_inner(
        2,
        Some(1),
        |_, _, _| finished(None),
        |pool| {
            assert!(!pool.dispatch(0, domain(0)));
        },
    );
    assert_eq!(snapshot["first_failure"]["kind"], "worker_spawn");
    assert_eq!(snapshot["active_workers"], 0);
}

#[test]
fn symbolic_parallel_buffer_refusal_does_not_publish_oversized_descriptor() {
    if !licensed() {
        return;
    }
    let (_, snapshot, _) = with_pool(
        2,
        |_, _, emit| {
            assert!(
                emit(Event::one(Effect::Frontier {
                    value: json!("x".repeat(CHUNK_BYTES)),
                    successor: false,
                    conditional: false
                }))
                .is_break()
            );
            finished(Some("StoppedByConsumer"))
        },
        |pool| {
            pool.dispatch(0, domain(0));
            wait_until(pool, |s| !s["first_failure"].is_null());
        },
    );
    assert_eq!(snapshot["first_failure"]["kind"], "buffer_limit");
    assert_eq!(snapshot["worker_buffered_logical_bytes"], 0);
}

#[test]
fn symbolic_parallel_returned_counter_overflow_is_atomic() {
    let pool = Pool::<1>::new(1);
    pool.dispatch(0, domain(0));
    {
        let mut state = pool.lock();
        state.totals.native = 7;
        state.totals.predicates = usize::MAX;
    }
    let mut done = finished(None);
    if let NativeStats::Apply(s) = &mut done.stats {
        s.native_operations = 2;
        s.matching.predicates = 1;
    }
    pool.finish(0, done);
    let state = pool.lock();
    assert_eq!(state.totals.returned, 0);
    assert_eq!(state.totals.native, 7);
    assert_eq!(state.totals.predicates, usize::MAX);
    assert_eq!(state.failure.as_ref().unwrap().kind, "counter_overflow");
}
