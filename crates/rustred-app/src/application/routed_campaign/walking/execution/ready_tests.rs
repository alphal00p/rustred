//! Actual shared-pool tests, not a second scheduling model.
use super::super::{
    checkpoint::test_support::Fixture,
    delegation::{Ledger, SchedulingPolicy},
    queue::Domain,
};
use super::*;
use std::num::NonZeroUsize;
use std::sync::atomic::AtomicUsize;

fn request() -> OwnerDomainWalkRequest {
    let mut r = OwnerDomainWalkRequest::new(super::super::OwnerDomainMatchRequest::new(
        String::new(),
        String::new(),
    ));
    r.workers = 4;
    r.publication_policy = OwnerDomainWalkPublicationPolicy::Ready;
    r.scheduling_policy = SchedulingPolicy::TransferUnreserved {
        lookahead: NonZeroUsize::new(3).unwrap(),
    };
    r.max_containment_checks = None;
    r.disable_work_limits();
    r
}
fn point(n: u64) -> Domain<1> {
    Domain {
        phase: Phase::Apply,
        owner: [true],
        lower: vec![n],
        upper: vec![Some(n)],
        rank: None,
        powers: Default::default(),
    }
}
fn seed() -> State<1> {
    let mut q = Queue::new(usize::MAX, None);
    q.delegation = Some(Ledger::new_ready(NonZeroUsize::new(3).unwrap(), usize::MAX).unwrap());
    for n in 0..16 {
        q.admit(point(n)).unwrap();
    }
    State::new(q, 0, None)
}
fn finished(error: Option<(&str, &'static str)>) -> Finished {
    Finished {
        stats: NativeStats::Apply(Default::default()),
        error: error.map(|e| e.0.into()),
        error_kind: error.map_or("none", |e| e.1),
        seconds: 0.0,
    }
}
fn prefix(id: usize, changed: bool, emit: &mut dyn FnMut(Event<1>) -> ControlFlow<()>) -> bool {
    if id < 2 {
        if emit(Event::one(Effect::Admit {
            domain: point(100 + id as u64 + u64::from(changed) * 100),
            successor: true,
            conditional: true,
        }))
        .is_break()
        {
            return false;
        }
        if emit(Event {
            count: parallel::CHUNK_EVENTS,
            effect: Effect::Count,
        })
        .is_break()
        {
            return false;
        }
    }
    true
}
fn has_two_prefixes(s: &State<1>) -> bool {
    let parked = serde_json::to_value(&s.streams).unwrap();
    usize::from(
        s.replay
            .as_ref()
            .is_some_and(|r| r.snapshot()["events"].as_u64().unwrap() > 0),
    ) + parked["parked"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p[1]["replay"]["events"].as_u64().unwrap() > 0)
        .count()
        == 2
}
fn run_pause() -> (State<1>, Fixture) {
    let request = request();
    let mut state = seed();
    let cancelled = AtomicBool::new(false);
    let start = Instant::now();
    let mut fixture = None;
    run_pool(
        &mut state,
        &request,
        &cancelled,
        &|_| {},
        true,
        &mut |s| {
            if s.completed >= 8 && has_two_prefixes(s) {
                assert_eq!(s.queue.next, 0);
                assert!(s.published_count() > 3);
                fixture = Some(Fixture::save(s));
                cancelled.store(true, Ordering::Release);
            }
            Ok(())
        },
        |id, _, stop, emit| {
            if !prefix(id, false, emit) {
                return finished(Some(("cancelled", "cancelled")));
            }
            if id < 2 {
                while !stop.load(Ordering::Acquire) && start.elapsed() < Duration::from_secs(10) {
                    std::thread::yield_now();
                }
                return finished(Some(if stop.load(Ordering::Acquire) {
                    ("cancelled", "cancelled")
                } else {
                    ("test stalled", "native_failure")
                }));
            }
            let _ = emit(Event::one(Effect::Count));
            finished(None)
        },
    );
    assert!(state.error.is_none(), "{:?}", state.error);
    assert!(state.checkpoint_paused);
    (state, fixture.expect("paused checkpoint fixture"))
}

#[test]
fn ready_shared_pool_replenishes_beyond_h_same_owner_and_replays_multiple_prefixes() {
    if !symbolica::license::LicenseManager::is_licensed() {
        return;
    }
    let (paused, fixture) = run_pause();
    let mut resumed: State<1> = fixture.resume().unwrap();
    assert!(has_two_prefixes(&resumed));
    assert!(resumed.published_count() > 3);
    assert_eq!(resumed.queue.next, 0);
    let mut baseline = seed();
    for s in [&mut resumed, &mut baseline] {
        run_pool(
            s,
            &request(),
            &AtomicBool::new(false),
            &|_| {},
            true,
            &mut |_| Ok(()),
            |id, _, _, emit| {
                assert!(prefix(id, false, emit));
                let _ = emit(Event::one(Effect::Count));
                finished(None)
            },
        );
        assert!(s.error.is_none(), "{:?}", s.error);
        assert_eq!(s.published_count(), s.queue.domains.len());
        assert_eq!(s.queue.next, s.queue.domains.len());
        assert!(s.streams.active.is_none() && s.streams.parked.is_empty());
    }
    assert_eq!(
        (
            resumed.events,
            resumed.successors,
            resumed.conditional,
            resumed.completed
        ),
        (
            baseline.events,
            baseline.successors,
            baseline.conditional,
            baseline.completed
        )
    );
    assert_eq!(resumed.queue.domains.len(), baseline.queue.domains.len());
    assert_eq!(resumed.initial_published(), 16);
    assert_eq!(resumed.pending_descendants(), 0);
    assert_eq!(
        resumed
            .queue
            .delegation
            .as_ref()
            .unwrap()
            .resolve()
            .unwrap()
            .summary,
        baseline
            .queue
            .delegation
            .as_ref()
            .unwrap()
            .resolve()
            .unwrap()
            .summary
    );
    assert!(paused.completed >= 8);
}

#[test]
fn ready_changed_parked_prefix_fails_before_any_suffix_admission() {
    if !symbolica::license::LicenseManager::is_licensed() {
        return;
    }
    let (_, fixture) = run_pause();
    let mut s = fixture.resume::<1>().unwrap();
    let parked = serde_json::to_value(&s.streams).unwrap();
    let bad = parked["parked"][0][0]["parent"].as_u64().unwrap() as usize;
    let admitted = s.queue.domains.len();
    run_pool(
        &mut s,
        &request(),
        &AtomicBool::new(false),
        &|_| {},
        true,
        &mut |_| Ok(()),
        |id, _, _, emit| {
            let ok = prefix(id, id == bad, emit);
            if ok {
                finished(None)
            } else {
                finished(Some(("stopped", "consumer_stop")))
            }
        },
    );
    assert!(
        s.error
            .as_deref()
            .is_some_and(|e| e.contains("checkpoint") || e.contains("replay")),
        "{:?}",
        s.error
    );
    assert!(!s.checkpoint_paused);
    assert_eq!(s.queue.domains.len(), admitted);
    assert!(!s.queue.domains.iter().any(|d| d.lower[0] >= 200));
}

fn ready_request(lookahead: usize) -> OwnerDomainWalkRequest {
    let mut r = request();
    r.scheduling_policy = SchedulingPolicy::TransferUnreserved {
        lookahead: NonZeroUsize::new(lookahead).unwrap(),
    };
    r
}
fn ready_seed(points: u64, lookahead: usize) -> State<1> {
    let mut q = Queue::new(usize::MAX, None);
    q.delegation =
        Some(Ledger::new_ready(NonZeroUsize::new(lookahead).unwrap(), usize::MAX).unwrap());
    for n in 0..points {
        q.admit(point(n)).unwrap();
    }
    State::new(q, 0, None)
}
fn admissions(first: u64, count: u64) -> Vec<Event<1>> {
    (0..count)
        .map(|i| {
            Event::one(Effect::Admit {
                domain: point(first + i),
                successor: true,
                conditional: false,
            })
        })
        .collect()
}
fn spin_until(deadline: Duration, mut done: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while !done() {
        if started.elapsed() >= deadline {
            return false;
        }
        std::thread::yield_now();
    }
    true
}

/// One long chunk-heavy ticket and many tiny tickets on three inspector
/// slots. Before this change a finished tiny ticket kept its slot until the
/// coordinator polled it, which under Ready never happens inside a chunk
/// commit. The service step between commit batches now reclaims those slots
/// into the bounded escrow and dispatches reserved work onto them.
#[test]
fn ready_finished_slots_are_recycled_before_the_current_chunk_commit_ends() {
    if !symbolica::license::LicenseManager::is_licensed() {
        eprintln!("skipped: parallel Symbolica workers require a license");
        return;
    }
    // Deterministic mechanism: the heavy stream holds slot 0, both tiny
    // tickets have finished in their slots, and only service steps run.
    let request = ready_request(64);
    let mut state = ready_seed(40, 64);
    let budget = super::super::worker_budget::WorkerBudget::for_request(&request);
    assert_eq!((budget.inspection, budget.helpers), (3, 0));
    state.admission = admission::Metrics::new(budget);
    let engine = admission::Engine::new(budget).unwrap();
    let cancel = AtomicBool::new(false);
    let release = AtomicBool::new(false);
    let started = AtomicUsize::new(0);
    let (_, snapshot, _) = parallel::with_ticket_pool_escrow::<1, _>(
        3,
        ready_escrow_limits(3),
        |id, _, stop, emit| {
            started.fetch_add(1, Ordering::Relaxed);
            if id == 0 {
                spin_until(Duration::from_secs(20), || {
                    release.load(Ordering::Acquire) || stop.load(Ordering::Acquire)
                });
                return finished(None);
            }
            let _ = emit(Event::one(Effect::Count));
            finished(None)
        },
        |pool| {
            let mut dispatcher = Dispatcher::new(0, 0);
            let mut streams = publication::ReadyStreams::default();
            assert_eq!(
                dispatcher
                    .run(&mut state, pool, &request, &cancel, &mut streams, None)
                    .0,
                3
            );
            assert!(
                spin_until(
                    Duration::from_secs(10),
                    || pool.snapshot()["finished_awaiting_poll"] == 2
                ),
                "tiny tickets did not finish"
            );
            assert_eq!(pool.snapshot()["occupied_native_slots"], 3);
            let mut service_steps = 0;
            engine
                .commit_chunk(
                    &mut state,
                    &request,
                    admissions(1000, 1000),
                    &cancel,
                    &pool.stop,
                    &mut |state| {
                        service_steps += 1;
                        ready_service(
                            state,
                            pool,
                            &mut dispatcher,
                            &mut streams,
                            &request,
                            &cancel,
                        );
                    },
                )
                .unwrap();
            assert_eq!(service_steps, 4, "1000 records commit in four batches");
            let duty = state.admission.duty;
            assert!(duty.ready_service_reclaims >= 2, "{duty:?}");
            assert!(duty.ready_service_dispatches >= 2, "{duty:?}");
            assert!(dispatcher.next > 3 && started.load(Ordering::Relaxed) > 3);
            let snapshot = pool.snapshot();
            assert!(snapshot["completed_slots_reclaimed"].as_u64().unwrap() >= 2);
            assert_eq!(snapshot["completed_escrow_max_entries"], 6);
            assert_eq!(
                snapshot["completed_escrow_max_accounted_bytes"],
                json!(2 * parallel::CHUNK_BYTES * 3)
            );
            // Every mid-commit dispatch moved a reserved obligation to Started
            // and joined the pending set; nothing was published.
            let ledger = state.queue.delegation.as_ref().unwrap();
            assert_eq!(ledger.published_count(), 0);
            assert!((0..dispatcher.next).all(|id| !ledger.can_dispatch(id)));
            assert!(!streams.is_empty());
            // Escrowed tickets are still polled in stream order: chunk, then
            // Finished; the slot they held is free for later dispatch.
            assert!(matches!(pool.poll(1), Poll::Events(chunk) if chunk.len() == 1));
            assert!(matches!(pool.poll(1), Poll::Finished(_)));
            assert!(matches!(pool.poll(1), Poll::Waiting));
            release.store(true, Ordering::Release);
            println!("ready_recycling_mechanism duty={duty:?} snapshot={snapshot}");
        },
    );
    assert!(snapshot["first_failure"].is_null(), "{snapshot}");

    // The production loop end to end: the heavy stream waits until the
    // coordinator has recycled at least one finished slot, then publishes a
    // four-batch chunk; every obligation is still published exactly once.
    let request = ready_request(64);
    let mut state = ready_seed(40, 64);
    let release = AtomicBool::new(false);
    run_pool(
        &mut state,
        &request,
        &AtomicBool::new(false),
        &|_| {},
        true,
        &mut |s| {
            if s.parallel["completed_slots_reclaimed"]
                .as_u64()
                .is_some_and(|n| n > 0)
            {
                release.store(true, Ordering::Release);
            }
            Ok(())
        },
        |id, _, stop, emit| {
            if id == 0 {
                if !spin_until(Duration::from_secs(20), || {
                    release.load(Ordering::Acquire) || stop.load(Ordering::Acquire)
                }) || stop.load(Ordering::Acquire)
                {
                    return finished(Some(("no finished slot was recycled", "native_failure")));
                }
                for event in admissions(2000, 1000) {
                    if emit(event).is_break() {
                        return finished(Some(("cancelled", "cancelled")));
                    }
                }
                return finished(None);
            }
            let _ = emit(Event::one(Effect::Count));
            finished(None)
        },
    );
    assert!(state.error.is_none(), "{:?}", state.error);
    assert_eq!(state.queue.domains.len(), 1040);
    assert_eq!(state.published_count(), 1040);
    assert_eq!(state.records.len(), 1040);
    assert_eq!(state.queue.next, 1040);
    assert!(state.streams.active.is_none() && state.streams.parked.is_empty());
    let parallel = &state.parallel;
    assert!(parallel["completed_slots_reclaimed"].as_u64().unwrap() > 0);
    assert_eq!(parallel["completed_escrow_max_entries"], 6);
    assert_eq!(parallel["completed_escrow_entries"], 0);
    let duty = &parallel["coordinator_duty"];
    assert!(duty["coordinator_elapsed_seconds"].as_f64().unwrap() > 0.0);
    assert!(duty["dispatch_seconds"].as_f64().unwrap() >= 0.0);
    println!(
        "ready_recycling_end_to_end duty={duty} reclaimed={}",
        parallel["completed_slots_reclaimed"]
    );
}

/// Under Ready, `transfer_retired` turns Unreserved IDs into Delegates and the
/// reservation sweep passes over them, so unpublished Delegates routinely sit
/// between the cursor and the next Reserved IDs. A service step must step
/// over them (publication is the main loop's job) instead of stopping, or a
/// long chunk commit reclaims slots without ever refilling them.
#[test]
fn ready_service_defers_unpublished_delegates_and_keeps_dispatching() {
    if !symbolica::license::LicenseManager::is_licensed() {
        eprintln!("skipped: parallel Symbolica workers require a license");
        return;
    }
    let request = ready_request(3);
    let mut state = ready_seed(40, 3);
    let band = Domain {
        lower: vec![3],
        upper: vec![Some(4)],
        ..point(3)
    };
    assert_eq!(state.queue.admit(band), Ok((40, true)));
    {
        let ledger = state.queue.delegation.as_ref().unwrap();
        assert_eq!(ledger.delegated_to(3), Some(40));
        assert_eq!(ledger.delegated_to(4), Some(40));
        assert_eq!(ledger.dispatch_fence(), 3, "IDs 0..3 reserved by H = 3");
    }
    let budget = super::super::worker_budget::WorkerBudget::for_request(&request);
    assert_eq!((budget.inspection, budget.helpers), (3, 0));
    state.admission = admission::Metrics::new(budget);
    let cancel = AtomicBool::new(false);
    let release = AtomicBool::new(false);
    let (_, snapshot, _) = parallel::with_ticket_pool_escrow::<1, _>(
        3,
        ready_escrow_limits(3),
        |id, _, stop, _| {
            if id == 0 {
                spin_until(Duration::from_secs(20), || {
                    release.load(Ordering::Acquire) || stop.load(Ordering::Acquire)
                });
            }
            finished(None)
        },
        |pool| {
            let mut dispatcher = Dispatcher::new(0, 0);
            let mut streams = publication::ReadyStreams::default();
            assert_eq!(
                dispatcher
                    .run(&mut state, pool, &request, &cancel, &mut streams, None)
                    .0,
                3
            );
            assert!(spin_until(Duration::from_secs(10), || {
                pool.snapshot()["finished_awaiting_poll"] == 2
            }));
            // Publish tickets 1 and 2 the way the main loop does; each native
            // publication releases a credit and the sweep reserves 5 and 6,
            // stepping over the Delegates 3 and 4.
            for raw in [1_usize, 2] {
                let Poll::Finished(finished) = pool.poll(raw) else {
                    panic!("ticket {raw} finished without a chunk");
                };
                let ticket = Ticket {
                    parent: raw,
                    part: None,
                };
                state.activate_stream(ticket, false).unwrap();
                state.commit_physical(ticket, finished, &request);
                streams.finished(raw);
            }
            assert!(state.error.is_none(), "{:?}", state.error);
            {
                let ledger = state.queue.delegation.as_ref().unwrap();
                assert_eq!(ledger.dispatch_fence(), 7);
                assert!(ledger.can_dispatch(5) && ledger.can_dispatch(6));
                assert!(!ledger.is_published(3) && !ledger.is_published(4));
            }
            let (reclaimed, dispatched) = ready_service(
                &mut state,
                pool,
                &mut dispatcher,
                &mut streams,
                &request,
                &cancel,
            );
            assert_eq!(reclaimed, 0, "both finished slots were polled");
            assert_eq!(dispatched, 2, "IDs 5 and 6 reach the two free slots");
            assert_eq!(dispatcher.deferred_delegates, [3, 4]);
            assert_eq!(dispatcher.next, 7);
            let duty = state.admission.duty;
            assert_eq!(duty.ready_service_deferred_delegates, 2);
            assert_eq!(duty.ready_service_dispatches, 2);
            {
                let ledger = state.queue.delegation.as_ref().unwrap();
                assert!(!ledger.is_published(3) && !ledger.is_published(4));
                assert_eq!(ledger.published_count(), 2);
            }
            // The main loop publishes the deferred Delegates first, in order.
            let events = RefCell::new(Vec::new());
            let observer = |event: Value| events.borrow_mut().push(event["id"].as_u64().unwrap());
            let mut saves = 0;
            let mut maybe_save = |_: &State<1>| {
                saves += 1;
                Ok(())
            };
            let (dispatched, nested) = dispatcher.run(
                &mut state,
                pool,
                &request,
                &cancel,
                &mut streams,
                Some((&observer, &mut maybe_save)),
            );
            assert_eq!(dispatched, 0, "no free slot: 0, 5 and 6 are running");
            assert!(
                nested > 0.0,
                "deferred publication is reported to the caller"
            );
            assert!(dispatcher.deferred_delegates.is_empty());
            assert_eq!(*events.borrow(), [3, 4]);
            assert_eq!(saves, 2);
            let ledger = state.queue.delegation.as_ref().unwrap();
            assert!(ledger.is_published(3) && ledger.is_published(4));
            assert_eq!(ledger.published_count(), 4);
            assert!(state.admission.duty.publication > 0.0);
            let delegated: Vec<u64> = state
                .records
                .iter()
                .filter(|record| record["record_kind"] == "delegated_not_inspected")
                .map(|record| record["id"].as_u64().unwrap())
                .collect();
            assert_eq!(delegated, [3, 4]);
            release.store(true, Ordering::Release);
        },
    );
    assert!(snapshot["first_failure"].is_null(), "{snapshot}");
    assert!(state.error.is_none(), "{:?}", state.error);
}

#[test]
fn ready_late_native_fault_after_cancellation_disallows_pause() {
    if !symbolica::license::LicenseManager::is_licensed() {
        return;
    }
    let mut s = seed();
    let cancelled = AtomicBool::new(false);
    run_pool(
        &mut s,
        &request(),
        &cancelled,
        &|_| {},
        true,
        &mut |s| {
            if s.completed >= 3 {
                cancelled.store(true, Ordering::Release);
            }
            Ok(())
        },
        |id, _, stop, _| {
            if id == 0 {
                let start = Instant::now();
                while !stop.load(Ordering::Acquire) && start.elapsed() < Duration::from_secs(10) {
                    std::thread::yield_now();
                }
                finished(Some(("genuine late failure", "native_failure")))
            } else {
                finished(None)
            }
        },
    );
    assert_eq!(s.error.as_deref(), Some("genuine late failure"));
    assert!(cancelled.load(Ordering::Acquire));
    assert_eq!(s.parallel["first_failure"]["kind"], "cancelled");
    assert!(!s.checkpoint_paused);
    assert_eq!(
        s.parallel["non_cancellation_failure"]["kind"],
        "native_failure"
    );
}
