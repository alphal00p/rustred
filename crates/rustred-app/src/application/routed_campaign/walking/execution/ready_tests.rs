//! Actual shared-pool tests, not a second scheduling model.
use super::super::{
    checkpoint::codec,
    delegation::{Ledger, SchedulingPolicy},
    queue::Domain,
};
use super::*;
use std::num::NonZeroUsize;

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
fn run_pause() -> (State<1>, Vec<u8>) {
    let request = request();
    let mut state = seed();
    let cancelled = AtomicBool::new(false);
    let start = Instant::now();
    let mut bytes = Vec::new();
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
                bytes.clear();
                codec::write(&mut bytes, s, &[], &[])?;
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
    assert!(!bytes.is_empty());
    (state, bytes)
}

#[test]
fn ready_shared_pool_replenishes_beyond_h_same_owner_and_replays_multiple_prefixes() {
    if !symbolica::license::LicenseManager::is_licensed() {
        return;
    }
    let (paused, bytes) = run_pause();
    let mut resumed: State<1> = codec::read::<1>(bytes.as_slice()).unwrap().state;
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
    let (_, bytes) = run_pause();
    let mut s = codec::read::<1>(bytes.as_slice()).unwrap().state;
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
