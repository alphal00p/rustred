//! Controlled streams exercise the production coordinator, not a mock scheduler.
use super::*;
use crate::application::routed_campaign::walking::inspection::NativeStats;
use std::num::NonZeroUsize;
use std::sync::{Condvar, Mutex};

mod concurrent;

const SLOW: [bool; 2] = [false, true];
const FAST: [bool; 2] = [true, false];

fn point(owner: [bool; 2], n: u64) -> Domain<2> {
    Domain {
        phase: Phase::Apply,
        owner,
        lower: vec![n, 0],
        upper: vec![Some(n), Some(0)],
        rank: Some(0),
        powers: Default::default(),
    }
}

fn setup(workers: usize) -> (OwnerDomainWalkRequest, Walk<2>) {
    assert!(symbolica::license::LicenseManager::is_licensed());
    let mut request = OwnerDomainWalkRequest::new(
        crate::application::routed_campaign::OwnerDomainMatchRequest::new(
            String::new(),
            String::new(),
        ),
    );
    request.workers = workers;
    request.max_events = 2 * parallel::CHUNK_EVENTS;
    request.max_containment_checks = None;
    request.scheduling_policy =
        super::super::super::super::OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
            lookahead: NonZeroUsize::new(16).unwrap(),
        };
    let initial = [Arc::new(point(SLOW, 0)), Arc::new(point(FAST, 0))];
    let (walk, _) = initialize(&initial, 0, 0, &request, &AtomicBool::new(false)).unwrap();
    (request, walk)
}

fn finished(events: usize, error: Option<&str>) -> Finished {
    let mut stats = rustred::solver::OwnerAppliedStats::default();
    stats.events = events;
    Finished {
        stats: NativeStats::Apply(stats),
        error: error.map(str::to_owned),
        error_kind: if error.is_some() {
            "native_failure"
        } else {
            "none"
        },
        seconds: 0.0,
    }
}

#[derive(Default)]
struct Gates {
    // (slow producer entered, fast successor actually began inspection)
    state: Mutex<(bool, bool)>,
    changed: Condvar,
}
impl Gates {
    fn set(&self, update: impl FnOnce(&mut (bool, bool))) {
        update(&mut self.state.lock().unwrap());
        self.changed.notify_all();
    }
    fn wait(&self, stop: &AtomicBool, ready: impl Fn(&(bool, bool)) -> bool) -> bool {
        // A broken scheduler must fail the test rather than deadlock its join.
        // Ordering is imposed by handshakes, not a relative timing assertion.
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut state = self.state.lock().unwrap();
        while !ready(&state) {
            if stop.load(Ordering::Acquire) || Instant::now() >= deadline {
                return false;
            }
            state = self
                .changed
                .wait_timeout(state, Duration::from_millis(20))
                .unwrap()
                .0;
        }
        true
    }
}

#[test]
fn owner_ready_peer_delivers_all_chunks_publishes_and_refills_before_slow_first_chunk() {
    for workers in [3, 6] {
        let (mut request, _) = setup(workers);
        // H1 deliberately retains this test's refill-after-publication contract.
        // Larger horizons separately test same-owner speculative inspection.
        request.scheduling_policy =
            super::super::super::super::OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
                lookahead: NonZeroUsize::new(1).unwrap(),
            };
        let (mut walk, _) = initialize(
            &[Arc::new(point(SLOW, 0)), Arc::new(point(FAST, 0))],
            0,
            0,
            &request,
            &AtomicBool::new(false),
        )
        .unwrap();
        let gates = Gates::default();
        let published_before_refill = AtomicBool::new(false);
        let snapshot = run_parallel(
            &mut walk,
            &request,
            &AtomicBool::new(false),
            &|event| {
                if event["event"] == "domain_started"
                    && event["bucket"] == "apply:10"
                    && event["id"] == 1
                {
                    // Both chunks must have been delivered before the first
                    // fast source is discharged and this key can be reused.
                    assert_eq!(event["native_processed_nodes"], 1);
                    assert_eq!(event["committed_events"], parallel::CHUNK_EVENTS + 1);
                    published_before_refill.store(true, Ordering::Release);
                }
            },
            |domain, stop, emit| {
                if domain.owner == SLOW {
                    gates.set(|s| s.0 = true);
                    return if gates.wait(stop, |s| s.1) {
                        finished(0, None)
                    } else {
                        finished(0, Some("slow producer released only by failure/deadline"))
                    };
                }
                if domain.lower[0] == 0 {
                    if !gates.wait(stop, |s| s.0) {
                        return finished(0, Some("slow producer did not enter"));
                    }
                    if emit(Event::one(Effect::Admit {
                        domain: point(FAST, 1),
                        successor: true,
                        conditional: false,
                    }))
                    .is_break()
                        || emit(Event {
                            count: parallel::CHUNK_EVENTS,
                            effect: Effect::Count,
                        })
                        .is_break()
                    {
                        return finished(0, Some("unexpected stopped stream"));
                    }
                    return finished(parallel::CHUNK_EVENTS + 1, None);
                }
                assert!(published_before_refill.load(Ordering::Acquire));
                // Release the slow producer only when a real worker is running
                // the replacement job, not merely on a dispatch notification.
                gates.set(|s| {
                    assert!(s.0 && !s.1);
                    s.1 = true;
                });
                finished(0, None)
            },
        );
        assert!(walk.error.is_none(), "{:?}", walk.error);
        assert!(gates.state.lock().unwrap().1);
        assert!(published_before_refill.load(Ordering::Acquire));
        assert_eq!(walk.budget.events, parallel::CHUNK_EVENTS + 1);
        assert_eq!(walk.buckets[&(Phase::Apply, FAST)].state.native_records, 2);
        let report = report::finish(walk, &request, 0.0, snapshot);
        assert_eq!(report["all_scheduled_domains_resolved"], true);
        assert_eq!(report["native_processed_nodes"], 3);
        assert_eq!(report["queued_nodes"], 0);
        assert_eq!(report["delegation"]["pending_native_publications"], 0);
    }
}

#[test]
fn owner_ready_wait_observes_external_cancellation_without_publishing_success() {
    let (request, mut walk) = setup(3);
    let cancel = AtomicBool::new(false);
    let gates = Gates::default();
    let snapshot = run_parallel(
        &mut walk,
        &request,
        &cancel,
        &|event| {
            if event["event"] == "domain_progress" {
                cancel.store(true, Ordering::Release);
            }
        },
        |_, stop, _| {
            assert!(!gates.wait(stop, |_| false));
            finished(0, Some("producer stopped while waiting"))
        },
    );
    assert!(cancel.load(Ordering::Acquire));
    assert_eq!(walk.error.as_deref(), Some("cancelled"));
    assert!(walk.metrics.idle_stream_wait_seconds > 0.0);
    let report = report::finish(walk, &request, 0.0, snapshot);
    assert_eq!(report["all_scheduled_domains_resolved"], false);
    assert_eq!(report["completed_nodes"], 0);
    assert_eq!(
        report["delegation"]["all_ledger_obligations_discharged"],
        false
    );
}

#[test]
fn owner_ready_peer_failure_stops_quiet_producer_and_retains_failed_responsibilities() {
    let (request, mut walk) = setup(3);
    let gates = Gates::default();
    let snapshot = run_parallel(
        &mut walk,
        &request,
        &AtomicBool::new(false),
        &|_| {},
        |domain, stop, _| {
            if domain.owner == SLOW {
                gates.set(|s| s.0 = true);
                assert!(!gates.wait(stop, |_| false));
                finished(0, Some("quiet peer stopped"))
            } else {
                assert!(gates.wait(stop, |s| s.0));
                finished(0, Some("injected ready-peer failure"))
            }
        },
    );
    assert_eq!(walk.error.as_deref(), Some("injected ready-peer failure"));
    let report = report::finish(walk, &request, 0.0, snapshot);
    assert_eq!(report["all_scheduled_domains_resolved"], false);
    assert_eq!(report["completed_nodes"], 0);
    assert_eq!(
        report["delegation"]["all_ledger_obligations_discharged"],
        false
    );
    assert_eq!(report["native_processed_nodes"], 2);
}

#[test]
fn owner_ready_streams_keep_global_event_cap_and_unpublished_failures() {
    let (mut request, mut walk) = setup(6);
    request.max_events = 1;
    let snapshot = run_parallel(
        &mut walk,
        &request,
        &AtomicBool::new(false),
        &|_| {},
        |_, _, emit| {
            let _ = emit(Event {
                count: 2,
                effect: Effect::Count,
            });
            finished(2, None)
        },
    );
    assert!(walk.error.is_some());
    assert_eq!(walk.budget.events, 1);
    let report = report::finish(walk, &request, 0.0, snapshot);
    assert_eq!(report["all_scheduled_domains_resolved"], false);
    assert_eq!(report["completed_nodes"], 0);
    assert_eq!(
        report["delegation"]["all_ledger_obligations_discharged"],
        false
    );
}
