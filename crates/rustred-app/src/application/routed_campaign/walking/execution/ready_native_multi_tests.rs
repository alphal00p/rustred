//! Real native multi-inspector Ready: a disk checkpoint holding two positive
//! unfinished accepted prefixes plus a finished hole resumes to exhaustion.
//!
//! Ready admission order depends on readiness, so exact equality needs a
//! fixed schedule. Scripted inspectors wrap the unchanged native visitor and
//! gate only *when* they emit, from state the coordinator exposes through its
//! checkpoint callback: ticket 0 flushes its whole native stream as one
//! accepted prefix and stays unfinished; ticket 2 then does the same; ticket
//! 1 finishes (the hole). Every later ticket emits only once all lower IDs
//! are published, so commits follow ID order. The paused run, its resume in
//! a fresh `State` from the on-disk CP5 store and the uninterrupted gated
//! baseline therefore commit the same events in the same order.
use super::super::{
    DiagnosticPause,
    checkpoint::{
        SaveKind,
        test_support::{Fixture, OWNER},
    },
    delegation::{Ledger, SchedulingPolicy},
    diagnostic_checkpoint,
    queue::Domain,
};
use super::ready_native_tests::without_timing;
use super::ready_tests::has_two_prefixes;
use super::*;
use std::num::NonZeroUsize;
use std::sync::atomic::AtomicUsize;

const FIRST: usize = 0;
const HOLE: usize = 1;
const SECOND: usize = 2;
const SEEDS: u64 = 5;

fn request() -> OwnerDomainWalkRequest {
    let mut request = OwnerDomainWalkRequest::new(super::super::OwnerDomainMatchRequest::new(
        String::new(),
        String::new(),
    ));
    request.workers = 4;
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Ready;
    request.scheduling_policy = SchedulingPolicy::TransferUnreserved {
        lookahead: NonZeroUsize::new(3).unwrap(),
    };
    // The flush marker alone is one full chunk of logical events.
    request.max_events = usize::MAX;
    request.max_domains = 10_000;
    request
}

fn seed() -> State<1> {
    let mut queue = Queue::new(10_000, None);
    queue.delegation = Some(Ledger::new_ready(NonZeroUsize::new(3).unwrap(), 10_000).unwrap());
    let ledger = queue.delegation.as_mut().unwrap();
    ledger.begin_initial_admission().unwrap();
    for n in 1..=SEEDS {
        queue
            .admit(Domain {
                phase: Phase::Apply,
                owner: [true],
                lower: vec![n],
                upper: vec![Some(n)],
                rank: Some(0),
                powers: Default::default(),
            })
            .unwrap();
    }
    queue
        .delegation
        .as_mut()
        .unwrap()
        .finish_initial_admission()
        .unwrap();
    State::new(queue, 0, None)
}

/// Sticky gates, advanced only from committed state seen by `maybe_save`.
#[derive(Default)]
struct Script {
    first_prefix: AtomicBool,
    both_prefixes: AtomicBool,
    hole: AtomicBool,
    watermark: AtomicUsize,
}

impl Script {
    fn observe(&self, state: &State<1>) {
        let prefixes = state.ready_accepted_source_prefixes();
        if prefixes >= 1 {
            self.first_prefix.store(true, Ordering::Release);
        }
        if prefixes >= 2 {
            self.both_prefixes.store(true, Ordering::Release);
        }
        let ledger = state.queue.delegation.as_ref().expect("ready ledger");
        if ledger.is_published(HOLE) {
            self.hole.store(true, Ordering::Release);
        }
        self.watermark.store(state.queue.next, Ordering::Release);
    }

    fn after(&self, id: usize) -> bool {
        self.watermark.load(Ordering::Acquire) >= id
    }

    /// The native visitor, delayed by the schedule. A stop while parked is an
    /// ordinary cooperative cancellation of an unfinished source.
    fn inspect(
        &self,
        reducer: &RoutedCandidateReducer<1>,
        request: &OwnerDomainWalkRequest,
        id: usize,
        domain: &Domain<1>,
        stop: &AtomicBool,
        emit: &mut dyn FnMut(Event<1>) -> ControlFlow<()>,
    ) -> Finished {
        let mut run = || -> Result<Finished, Finished> {
            match id {
                FIRST => {}
                SECOND => park(stop, || self.first_prefix.load(Ordering::Acquire))?,
                HOLE => park(stop, || self.both_prefixes.load(Ordering::Acquire))?,
                _ => park(stop, || self.after(id))?,
            }
            let finished = inspection::inspect(
                reducer,
                domain,
                request,
                stop,
                &InitialOrthants::empty(),
                &InitialOverlapIndex::empty(),
                emit,
            );
            if id == FIRST || id == SECOND {
                // Flush everything emitted so far as an accepted prefix.
                let flush = Event {
                    count: parallel::CHUNK_EVENTS,
                    effect: Effect::Count,
                };
                if emit(flush).is_break() {
                    return Err(stopped("cancelled", "cancelled"));
                }
                if id == FIRST {
                    park(stop, || self.hole.load(Ordering::Acquire))?;
                } else {
                    park(stop, || self.after(SECOND))?;
                }
            }
            Ok(finished)
        };
        run().unwrap_or_else(|finished| finished)
    }
}

fn stopped(error: &str, kind: &'static str) -> Finished {
    Finished {
        stats: NativeStats::Apply(Default::default()),
        error: Some(error.into()),
        error_kind: kind,
        seconds: 0.0,
    }
}

fn park(stop: &AtomicBool, open: impl Fn() -> bool) -> Result<(), Finished> {
    let started = Instant::now();
    while !open() {
        if stop.load(Ordering::Acquire) {
            return Err(stopped("cancelled", "cancelled"));
        }
        if started.elapsed() > Duration::from_secs(60) {
            return Err(stopped("scripted schedule stalled", "native_failure"));
        }
        std::thread::yield_now();
    }
    Ok(())
}

/// One scripted walk; `pause` sees each checkpoint opportunity first.
fn walk(
    state: &mut State<1>,
    reducer: &RoutedCandidateReducer<1>,
    cancellation: &AtomicBool,
    pause: &mut dyn FnMut(&State<1>),
) {
    let request = request();
    let script = Script::default();
    script.observe(state);
    run_pool(
        state,
        &request,
        cancellation,
        &|_| {},
        true,
        &mut |state| {
            pause(state);
            script.observe(state);
            Ok(())
        },
        |id, domain, stop, emit| script.inspect(reducer, &request, id, domain, stop, emit),
    );
}

fn assert_exhausted(state: &State<1>) {
    assert_eq!(state.error, None);
    assert!(!state.checkpoint_paused);
    assert_eq!(state.frontiers, 0);
    assert_eq!(state.published_count(), state.queue.domains.len());
    assert_eq!(state.queue.next, state.queue.domains.len());
    assert_eq!(state.initial_published(), SEEDS as usize);
    assert_eq!(state.pending_descendants(), 0);
    assert!(state.streams.active.is_none() && state.streams.parked.is_empty());
}

/// The uninterrupted gated walk; the records are taken before finalization
/// (unannotated: finalization returns the ledger resolutions instead of
/// annotating records in place) and the ledger summary from it.
struct Baseline {
    state: State<1>,
    records: Value,
    ledger: Value,
}

/// Resume one on-disk generation in a fresh `State`, walk it to exhaustion
/// and require exact equality with the uninterrupted gated baseline.
fn resume_matches_baseline(
    fixture: &Fixture,
    reducer: &RoutedCandidateReducer<1>,
    baseline: &Baseline,
    paused: &State<1>,
) {
    let Baseline {
        state: baseline,
        records,
        ledger,
    } = baseline;
    let mut resumed: State<1> = fixture.resume().unwrap();
    assert!(has_two_prefixes(&resumed));
    assert!(resumed.streams.active.is_none());
    assert_eq!(resumed.streams.parked.len(), 2);
    let parked: Vec<usize> = resumed
        .streams
        .parked
        .iter()
        .map(|(t, _)| t.parent)
        .collect();
    assert!(
        parked.contains(&FIRST) && parked.contains(&SECOND),
        "{parked:?}"
    );
    assert!(
        resumed
            .queue
            .delegation
            .as_ref()
            .unwrap()
            .is_published(HOLE)
    );
    assert!(resumed.published_count() > resumed.queue.next);
    assert!(DiagnosticPause::ReadyMultiPrefix.fires(&resumed));
    // Neither save's closure scan was cut, although the post-cancellation
    // one ran with the run's cancellation set.
    let closure = resumed.closure_json();
    assert_eq!(closure["available"], true);
    assert_eq!(closure["snapshot_stale"], false, "{closure}");
    walk(&mut resumed, reducer, &AtomicBool::new(false), &mut |_| {});
    assert_exhausted(&resumed);
    assert!(resumed.completed > paused.completed);

    assert_eq!(resumed.queue.domains, baseline.queue.domains);
    assert_eq!(
        &without_timing(json!(resumed.records.borrow().snapshot())),
        records
    );
    assert_eq!(
        (
            resumed.events,
            resumed.successors,
            resumed.conditional,
            resumed.completed,
            resumed.queue.deduplicated
        ),
        (
            baseline.events,
            baseline.successors,
            baseline.conditional,
            baseline.completed,
            baseline.queue.deduplicated
        )
    );
    assert_eq!(&resumed.finalize_delegation().0.unwrap(), ledger);
}

#[test]
fn ready_multi_inspector_multi_prefix_disk_resume_matches_gated_baseline() {
    if !crate::test_gates::licensed_or_skip(
        "ready_multi_inspector_multi_prefix_disk_resume_matches_gated_baseline",
    ) {
        return;
    }
    if rustred::campaign::ParallelExecution::preflight_requested_core_budget(4).is_err() {
        crate::test_gates::skip(
            "ready_multi_inspector_multi_prefix_disk_resume_matches_gated_baseline",
            "W=4 needs four CPUs in this process's affinity mask",
        );
        return;
    }
    let reducer = super::initial_orthants_tests::native_fixture();
    let budget = super::super::worker_budget::WorkerBudget::for_request(&request());
    assert_eq!((budget.inspection, budget.helpers), (3, 0));

    let mut baseline = seed();
    walk(
        &mut baseline,
        &reducer,
        &AtomicBool::new(false),
        &mut |_| {},
    );
    assert_exhausted(&baseline);
    let records = without_timing(json!(baseline.records.borrow().snapshot()));
    let ledger = baseline.finalize_delegation().0.unwrap();
    let baseline = Baseline {
        state: baseline,
        records,
        ledger,
    };

    // The production branch decides when to save, label, journal and cancel,
    // against a real store set up as `walking::run` sets up a fresh
    // checkpointed walk: bootstrap, owner binding, the record sidecar
    // attached before the first commit (every save seals a segment) and the
    // initial forced save. The triggering generation is also copied to a
    // second directory so both generations a paused process leaves can be
    // resumed.
    let production = Fixture::fresh(&seed());
    let mut store = production.open(false).unwrap();
    store.bootstrap().unwrap();
    store.bind_owners(vec![OWNER.into()]).unwrap();
    let journal = RefCell::new(Vec::new());
    let observer = |event: Value| journal.borrow_mut().push(event);
    let cancellation = AtomicBool::new(false);
    let mut pause = Some(DiagnosticPause::ReadyMultiPrefix);
    let mut trigger = None;
    let mut paused = seed();
    store.attach_records(&paused).unwrap();
    assert!(matches!(
        &*paused.records.borrow(),
        records::RecordSink::Sidecar(_)
    ));
    observer(
        store
            .save_cancellable(
                &paused,
                &[],
                &[],
                SaveKind::Forced,
                &cancellation,
                &observer,
            )
            .unwrap()
            .expect("the initial generation"),
    );
    walk(&mut paused, &reducer, &cancellation, &mut |state| {
        let fired = diagnostic_checkpoint(
            &mut pause,
            &mut store,
            state,
            &[],
            &[],
            &cancellation,
            &observer,
        )
        .unwrap();
        if fired {
            assert!(trigger.is_none(), "the pause fires once");
            assert_eq!(state.queue.next, FIRST);
            trigger = Some(production.copy_latest());
        }
    });
    assert_eq!(paused.error, None);
    assert!(paused.checkpoint_paused);
    assert!(paused.ready_multi_prefix_hole());
    assert!(pause.is_none() && cancellation.load(Ordering::Acquire));
    assert!(
        !diagnostic_checkpoint(
            &mut pause,
            &mut store,
            &paused,
            &[],
            &[],
            &cancellation,
            &observer
        )
        .unwrap()
    );
    let triggers: Vec<Value> = journal
        .borrow()
        .iter()
        .filter(|event| event["event"] == "diagnostic_pause")
        .cloned()
        .collect();
    assert_eq!(triggers.len(), 1);
    assert_eq!(triggers[0]["diagnostic_pause"], "ready-multi-prefix");
    assert!(triggers[0]["ready_accepted_source_prefixes"].as_u64() >= Some(2));
    assert!(triggers[0]["ready_published_holes"].as_u64() > Some(0));
    let labelled = production.manifest()["metadata"].clone();
    assert_eq!(labelled["diagnostic_pause"], "ready-multi-prefix");
    assert_eq!(labelled["paused"], false);
    // The walk's own final save after cancellation, made as `walking::run`
    // makes it (the run's flag is set; the save neither folds the edge log
    // nor cuts its closure scan): the generation a production resume
    // restores, still labelled by this session.
    store
        .save_cancellable(&paused, &[], &[], SaveKind::Final, &cancellation, &observer)
        .unwrap()
        .expect("the cancelled state differs from the triggering one");
    drop(store);
    let latest = production.manifest()["metadata"].clone();
    assert_eq!(latest["diagnostic_pause"], "ready-multi-prefix");
    assert_eq!(latest["paused"], true);
    assert!(latest["generation"].as_u64() > labelled["generation"].as_u64());

    let trigger = trigger.expect("multi-prefix hole checkpoint");
    // Every committed record of both generations sits in a sealed sidecar
    // segment; the manifest tiles them from record 0.
    assert!(matches!(
        &*paused.records.borrow(),
        records::RecordSink::Sidecar(_)
    ));
    for (fixture, published) in [
        (&trigger, triggers[0]["committed_domains"].as_u64().unwrap()),
        (&production, paused.published_count() as u64),
    ] {
        let records = &fixture.manifest()["sections"]["records"];
        assert!(published > 0);
        assert_eq!(records["total"].as_u64(), Some(published), "{records}");
        let mut next = 0;
        for segment in records["segments"].as_array().unwrap() {
            assert_eq!(segment["first"].as_u64(), Some(next), "{records}");
            next += segment["count"].as_u64().unwrap();
            assert!(
                fixture
                    .dir
                    .join(segment["file"].as_str().unwrap())
                    .is_file()
            );
        }
        assert_eq!(next, published, "{records}");
    }
    resume_matches_baseline(&trigger, &reducer, &baseline, &paused);
    resume_matches_baseline(&production, &reducer, &baseline, &paused);
    println!(
        "ready_multi_prefix_gate domains={} events={} completed={} paused_published={} paused_watermark={}",
        baseline.state.queue.domains.len(),
        baseline.state.events,
        baseline.state.completed,
        paused.published_count(),
        paused.queue.next
    );
}

#[test]
fn multi_prefix_threshold_needs_two_prefixes_and_a_published_hole() {
    use super::streams::multi_prefix_hole;
    // (accepted prefixes, published records, contiguous watermark)
    assert!(!multi_prefix_hole(1, 5, 0));
    assert!(!multi_prefix_hole(2, 3, 3));
    assert!(multi_prefix_hole(2, 4, 3));
    assert!(multi_prefix_hole(17, 1, 0));
    assert!(!multi_prefix_hole(0, 0, 0));
}
