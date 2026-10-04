use super::*;
use crate::application::routed_campaign::walking::descendant_closure::Tracker;
use crate::application::routed_campaign::walking::epoch::{
    admit_initial,
    dispatch::{Dispatch, Refill},
    job::{Job, LookupReport, Miss},
    records,
};
use crate::application::routed_campaign::walking::queue::Phase;
use rustred::solver::DomainPowerBounds;
use serde_json::Value;
use std::sync::atomic::AtomicUsize;

const CONFIG: MergeConfig = MergeConfig {
    frontier_stop: true,
    lockstep: true,
    g2: false,
    finite_replay: None,
};
const LIMITS: Limits = Limits {
    obligations: 10_000,
    retirements: 10_000,
};

fn image(phase: Phase, owner: [bool; 2], lower: [u64; 2], upper: [u64; 2]) -> CompactDomain<2> {
    CompactDomain::try_from_domain(&Domain {
        phase,
        owner,
        lower: lower.to_vec(),
        upper: upper.into_iter().map(Some).collect(),
        rank: None,
        powers: DomainPowerBounds::default(),
    })
    .unwrap()
}

fn apply(lower: [u64; 2], upper: [u64; 2]) -> CompactDomain<2> {
    image(Phase::Apply, [true, false], lower, upper)
}

fn result(job: &Job<2>, images: Vec<CompactDomain<2>>) -> JobResult<2> {
    let n = images.len() as u64;
    JobResult {
        seq: job.seq,
        parent: job.parent,
        v0: job.v0,
        kind: NativeKind::Apply,
        error_kind: ErrorKind::None,
        break_reason: BreakReason::None,
        panic: false,
        emitted: n,
        accepted: n,
        stats_events: n,
        successors: n,
        conditional: 0,
        known_reuse: 0,
        job_duplicates: 0,
        optional: [0; 3],
        route_masks: 0,
        route_joint_pruned: 0,
        seconds: 0.0,
        stats_json: format!("{{\"events\":{n},\"successors\":{n}}}").into_bytes(),
        error: None,
        frontiers: Vec::new(),
        refusals: Vec::new(),
        refusals_truncated: false,
        scope: None,
        g2: None,
        finite_replay: None,
        lookup: None,
        misses: images
            .into_iter()
            .enumerate()
            .map(|(ordinal, image)| Miss {
                ordinal: ordinal as u32,
                digest: image.digest().0,
                image,
                target: None,
            })
            .collect(),
    }
}

fn fixture(large: bool) -> (EpochState<2>, Dispatch, Checked<2>) {
    let mut state = EpochState::new(10_000, usize::MAX, usize::MAX);
    for initial in [apply([0, 0], [0, 0]), apply([10, 0], [10, 0])] {
        admit_initial(&mut state, &initial.expand()).unwrap();
    }
    state.p0 = state.watermark();
    // An unprotected pending point contained in both incomparable new boxes:
    // minimum provisional position must win even across helper completion order.
    admit_initial(&mut state, &apply([4, 4], [4, 4]).expand()).unwrap();
    state.tracker = Tracker::new(state.store.len());
    let mut dispatch = Dispatch::new();
    let jobs = match dispatch.refill(&mut state, 2) {
        Refill::Jobs(jobs) => jobs,
        _ => panic!("two inspection jobs"),
    };
    assert_eq!(jobs.len(), 2);
    let point = apply([3, 3], [3, 3]);
    let mut first = vec![
        point,
        apply([2, 2], [5, 5]),
        apply([3, 3], [6, 6]),
        image(Phase::Route, [true, false], [2, 2], [5, 5]),
        image(Phase::Apply, [false, true], [2, 2], [5, 5]),
    ];
    if large {
        first.extend((0..600).map(|i| apply([100 + 2 * i, 0], [100 + 2 * i, 0])));
    }
    let second = vec![point, apply([3, 3], [6, 6]), apply([300, 0], [301, 0])];
    let bytes = vec![
        result(&jobs[1], second).encode(),
        result(&jobs[0], first).encode(),
    ];
    let checked = p1_check(&mut state, bytes, CONFIG).unwrap();
    assert!(checked.stop.is_none());
    (state, dispatch, checked)
}

fn assert_plan_eq(a: &MergePlan<2>, b: &MergePlan<2>) {
    assert_eq!(a.targets, b.targets);
    assert_eq!(a.transfers, b.transfers);
    assert_eq!(a.survivors.len(), b.survivors.len());
    for (a, b) in a.survivors.iter().zip(&b.survivors) {
        assert_eq!(a.image, b.image);
        assert_eq!(a.summary, b.summary);
        assert_eq!(a.query.core, b.query.core);
        assert_eq!(a.digest, b.digest);
        assert_eq!(a.retire, b.retire);
    }
    assert_eq!(a.counters.lookup, b.counters.lookup);
    assert_eq!(a.counters.verify, b.counters.verify);
    assert_eq!(a.counters.miss_requests, b.counters.miss_requests);
    assert_eq!(a.counters.antichain_folded, b.counters.antichain_folded);
    assert_eq!(a.counters.inspector.queries, b.counters.inspector.queries);
    assert_eq!(
        a.counters.inspector.stored_hits,
        b.counters.inspector.stored_hits
    );
    assert_eq!(
        a.counters.inspector.coordinator_miss_rechecks_skipped,
        b.counters.inspector.coordinator_miss_rechecks_skipped
    );
    assert_eq!(a.counters.inspector.seconds, b.counters.inspector.seconds);
}

#[test]
fn serial_parallel_plan_identity_across_buckets_and_index_threshold() {
    let cancelled = AtomicBool::new(false);
    for large in [false, true] {
        let (state, _, checked) = fixture(large);
        let reference = p2_plan(&state, &checked).unwrap();
        for helpers in [0, 1, 2, 7] {
            let engine = Engine::new(helpers, LIMITS).unwrap();
            let prepared = engine.prepare(&state, &checked, &cancelled).unwrap();
            assert_plan_eq(&reference, &prepared.plan);
            assert_eq!(prepared.metrics.helper_limit, helpers);
            assert_eq!(prepared.metrics.buckets, 3);
            if large {
                assert!(prepared.metrics.largest_bucket > INDEXED_ANTICHAIN);
                assert!(prepared.metrics.largest_bucket * 10 > prepared.metrics.candidates * 9);
                assert!(prepared.metrics.source_tasks > checked.entries.len() * 2);
            }
        }
    }
}

#[test]
fn cross_entry_observation_preserves_plan_counters_and_serialization_off() {
    for large in [false, true] {
        let (state, _, checked) = fixture(large);
        let reference = p2_plan(&state, &checked).unwrap();
        for helpers in [0, 2] {
            for enabled in [false, true] {
                let mut engine = Engine::new(helpers, LIMITS).unwrap();
                engine.observe_cross_entry = enabled;
                let prepared = engine
                    .prepare(&state, &checked, &AtomicBool::new(false))
                    .unwrap();
                assert_plan_eq(&reference, &prepared.plan);
                let metrics = serde_json::to_value(&prepared.metrics).unwrap();
                let mut totals = Totals::default();
                totals.add(&prepared.metrics);
                let totals = serde_json::to_value(totals).unwrap();
                if enabled {
                    let sample = &metrics["cross_entry_observation"];
                    let extra = if large { 600 } else { 0 };
                    assert_eq!(sample["observed_rows"], 8 + extra);
                    assert_eq!(sample["unique_images"], 6 + extra);
                    assert_eq!(sample["cross_entry_geometry_repeats"], 2);
                    assert_eq!(sample["cross_entry_same_context_repeats"], 2);
                    assert_eq!(sample["truncated"], false);
                    assert_eq!(totals["cross_entry_observation"]["sampled_cuts"], 1);
                } else {
                    assert!(metrics.get("cross_entry_observation").is_none());
                    assert!(totals.get("cross_entry_observation").is_none());
                }
            }
        }
    }
}

#[test]
fn cross_entry_observation_preserves_failures_and_cancellation() {
    for failure in 0..4 {
        let (mut state, _, mut checked) = fixture(false);
        match failure {
            0 => checked.entries[0].result.misses[0].digest ^= 1,
            1 => checked.entries[0].result.misses[0].target = Some(u32::MAX),
            2 => {
                checked.entries[0].result.lookup = Some(LookupReport {
                    version: state.k,
                    published_len: state.watermark(),
                    lookup: LookupCounters::default(),
                    verify: VerifyCounters::default(),
                    seconds: 0.0,
                });
            }
            _ => {
                checked.entries[0].result.misses[0].target = Some(0);
                state.store.enable_rescue_duplicates().unwrap();
                state.store.install_quarantine(vec![1]).unwrap();
            }
        }
        let reference = p2_plan(&state, &checked).err().unwrap().0;
        for helpers in [0, 2] {
            for enabled in [false, true] {
                let mut engine = Engine::new(helpers, LIMITS).unwrap();
                engine.observe_cross_entry = enabled;
                let error = engine
                    .prepare(&state, &checked, &AtomicBool::new(false))
                    .err()
                    .unwrap();
                assert!(matches!(error, Error::Fatal(Fatal(ref message)) if message == &reference));
                assert!(matches!(
                    engine.prepare(&state, &checked, &AtomicBool::new(true)),
                    Err(Error::Stopped)
                ));
            }
        }
    }
}

struct Rows(Vec<Value>);
impl RecordOut for Rows {
    fn reserve(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn push(&mut self, row: Record) -> Result<(), String> {
        self.0.push(row.project()?);
        Ok(())
    }
}

fn applied(helpers: Option<usize>) -> (EpochState<2>, Vec<Value>) {
    let (mut state, mut dispatch, checked) = fixture(true);
    let plan = match helpers {
        None => p2_plan(&state, &checked).unwrap(),
        Some(helpers) => {
            Engine::new(helpers, LIMITS)
                .unwrap()
                .prepare(&state, &checked, &AtomicBool::new(false))
                .unwrap()
                .plan
        }
    };
    plan.counters.apply_to(&mut state);
    let mut rows = Rows(Vec::new());
    p3_preflight(&mut state, &checked, &plan, &mut rows).unwrap();
    p3_apply(
        &mut state,
        checked,
        plan,
        CONFIG,
        &records::Builder,
        &mut rows,
        &mut |id, attempts| dispatch.requeue(id, attempts),
    )
    .unwrap();
    (state, rows.0)
}

#[test]
fn serial_parallel_publication_identity() {
    let (reference, rows) = applied(None);
    for helpers in [0, 1, 2, 7] {
        let (state, actual_rows) = applied(Some(helpers));
        assert_eq!(rows, actual_rows);
        assert_eq!(reference.store.domains, state.store.domains);
        assert_eq!(reference.edges.log(), state.edges.log());
        assert_eq!(reference.nodes, state.nodes);
        assert_eq!(reference.live, state.live);
        assert_eq!(reference.k, state.k);
        for id in 0..state.watermark() {
            assert_eq!(
                reference.ledger.get(id).unwrap(),
                state.ledger.get(id).unwrap()
            );
        }
        assert!(state.seal_rule_holds());
    }
}

#[test]
fn bounds_and_cancellation_leave_authority_unchanged() {
    let (state, _, checked) = fixture(false);
    let domains = state.store.domains.clone();
    let nodes = state.nodes.clone();
    for limits in [
        Limits {
            obligations: 1,
            ..LIMITS
        },
        Limits {
            retirements: 0,
            ..LIMITS
        },
    ] {
        let result =
            Engine::new(2, limits)
                .unwrap()
                .prepare(&state, &checked, &AtomicBool::new(false));
        assert!(matches!(result, Err(Error::RamGuard(_))));
    }
    let result = Engine::new(2, LIMITS)
        .unwrap()
        .prepare(&state, &checked, &AtomicBool::new(true));
    assert!(matches!(result, Err(Error::Stopped)));
    assert_eq!(state.store.domains, domains);
    assert_eq!(state.nodes, nodes);
    assert_eq!(state.k, 0);
    assert!(state.edges.log().is_empty());
    assert!(!state.poisoned);
}

#[test]
fn observed_preparation_matches_reference_and_joins_on_cancellation() {
    let (state, _, checked) = fixture(true);
    let reference = p2_plan(&state, &checked).unwrap();
    for helpers in [0, 1, 3] {
        let engine = Engine::new(helpers, LIMITS).unwrap();
        let mut ticks = 0;
        let prepared = engine
            .prepare_observed(&state, &checked, || {
                ticks += 1;
                false
            })
            .unwrap();
        assert_plan_eq(&reference, &prepared.plan);
        assert!(ticks >= 2);
        assert!(matches!(
            engine.prepare_observed(&state, &checked, || true),
            Err(Error::Stopped)
        ));
        // A subsequent successful call proves there are no orphaned pool jobs
        // or a sticky cancellation bit inherited from the prior invocation.
        let next = engine.prepare_observed(&state, &checked, || false).unwrap();
        assert_plan_eq(&reference, &next.plan);
        assert_eq!(state.k, 0);
        assert!(state.edges.log().is_empty());
    }
}

#[test]
fn inline_preparation_polls_during_work_without_a_helper_thread() {
    let (state, _, checked) = fixture(true);
    let engine = Engine::new(0, LIMITS).unwrap();
    assert!(engine.pool.is_none());
    let mut polls = 0;
    // A zero interval is a deterministic test clock: the third observation
    // happens inside the work after 256 controlled scan steps, not at return.
    let result = engine.prepare_observed_periodic(
        &state,
        &checked,
        || {
            polls += 1;
            polls >= 3
        },
        std::time::Duration::ZERO,
    );
    assert!(matches!(result, Err(Error::Stopped)));
    assert_eq!(polls, 4, "three in-work polls plus the final race check");
    assert_eq!(state.k, 0);
    assert!(state.edges.log().is_empty());
    assert!(!state.poisoned);
    let reference = p2_plan(&state, &checked).unwrap();
    let prepared = engine.prepare_observed(&state, &checked, || false).unwrap();
    assert_plan_eq(&reference, &prepared.plan);
}

#[test]
fn task_failure_selection_is_ordered_and_all_owned_work_joins() {
    for helpers in [0, 1, 2, 7] {
        let engine = Engine::new(helpers, LIMITS).unwrap();
        let completed = AtomicUsize::new(0);
        let error = engine
            .map(
                &[0, 1, 2, 3],
                20,
                &AtomicBool::new(false),
                &mut || Ok(()),
                |ordinal, _, _| {
                    completed.fetch_add(1, Ordering::Relaxed);
                    if ordinal == 21 {
                        return Err(Error::Fatal(fatal("first failure")));
                    }
                    if ordinal == 22 {
                        panic!("later failure");
                    }
                    Ok(ordinal)
                },
            )
            .unwrap_err();
        assert!(matches!(error, Error::Fatal(Fatal(ref text)) if text == "first failure"));
        assert_eq!(completed.load(Ordering::Relaxed), 4);
    }
}

#[test]
fn bounded_store_query_never_returns_a_partial_retirement_set() {
    let (state, _, _) = fixture(false);
    let q = QueryImage::new(apply([0, 0], [20, 20])).unwrap();
    let query = Query::new(q.core.clone(), Phase::Apply);
    let reference = state
        .store
        .contained_live(&q, &query, &mut LookupCounters::default())
        .unwrap();
    assert!(reference.len() > 1);
    let mut checkpoints = 0;
    let actual = state
        .store
        .contained_live_bounded(
            &q,
            &query,
            &mut LookupCounters::default(),
            reference.len(),
            || {
                checkpoints += 1;
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(reference, actual);
    assert!(checkpoints > 0);
    assert_eq!(
        state
            .store
            .contained_live_bounded(
                &q,
                &query,
                &mut LookupCounters::default(),
                reference.len() - 1,
                || Ok(())
            )
            .unwrap_err(),
        "prepared retirement set limit"
    );
    assert_eq!(
        state
            .store
            .contained_live_bounded(
                &q,
                &query,
                &mut LookupCounters::default(),
                reference.len(),
                || Err(CANCELLED)
            )
            .unwrap_err(),
        CANCELLED
    );
}

#[test]
fn source_blocks_preserve_current_and_stale_snapshot_lookup_counters() {
    for stale in [false, true] {
        let (mut state, _, mut checked) = fixture(true);
        let result = &mut checked.entries[0].result;
        let miss = &mut result.misses[0];
        miss.image = state.store.domains[0];
        miss.digest = miss.image.digest().0;
        miss.target = Some(0);
        result.lookup = Some(LookupReport {
            version: state.k,
            published_len: state.watermark(),
            lookup: LookupCounters {
                exact_hits: 1,
                misses: result.misses.len() as u64 - 1,
                ..LookupCounters::default()
            },
            verify: VerifyCounters {
                calls: 1,
                accepted: 1,
                raw_inclusions: 1,
                ..VerifyCounters::default()
            },
            seconds: 0.125,
        });
        if stale {
            // Synthetic P2 state: the same admitted report is now older than
            // the publication version, so every unstored miss must recheck.
            state.k += 1;
        }
        let reference = p2_plan(&state, &checked).unwrap();
        for helpers in [0, 1, 2, 7] {
            let prepared = Engine::new(helpers, LIMITS)
                .unwrap()
                .prepare(&state, &checked, &AtomicBool::new(false))
                .unwrap();
            assert_plan_eq(&reference, &prepared.plan);
            assert_eq!(prepared.plan.counters.inspector.stored_hits, 1);
            assert_eq!(prepared.plan.counters.inspector.seconds, 0.125);
        }
        state.store.enable_rescue_duplicates().unwrap();
        state.store.install_quarantine(vec![1]).unwrap();
        let reference = p2_plan(&state, &checked).err().unwrap().0;
        for helpers in [0, 2, 7] {
            let error = Engine::new(helpers, LIMITS)
                .unwrap()
                .prepare(&state, &checked, &AtomicBool::new(false))
                .err()
                .unwrap();
            assert!(matches!(error, Error::Fatal(Fatal(ref message)) if message == &reference));
        }
    }
}

#[test]
fn source_failure_precedes_later_header_failure_independent_of_helpers() {
    let (state, _, mut checked) = fixture(true);
    checked.entries[0].result.misses[400].digest ^= 1;
    checked.entries[1].result.lookup = Some(LookupReport {
        version: state.k,
        published_len: state.watermark(),
        lookup: LookupCounters::default(),
        verify: VerifyCounters::default(),
        seconds: 0.0,
    });
    let reference = p2_plan(&state, &checked).err().unwrap().0;
    assert!(reference.contains("miss 400: shipped digest differs"));
    for helpers in [0, 1, 2, 7] {
        let error = Engine::new(helpers, LIMITS)
            .unwrap()
            .prepare(&state, &checked, &AtomicBool::new(false))
            .err()
            .unwrap();
        assert!(matches!(error, Error::Fatal(Fatal(ref message)) if message == &reference));
    }
}

#[test]
fn controlled_lookup_preserves_exact_winner_and_stops_inside_index_scan() {
    let (state, _, _) = fixture(false);
    for probe in [apply([4, 4], [4, 4]), apply([15, 15], [15, 15])] {
        let q = QueryImage::new(probe).unwrap();
        let query = Query::new(q.core.clone(), Phase::Apply);
        let (mut lookup, mut verify) = (LookupCounters::default(), VerifyCounters::default());
        let reference = state
            .store
            .lookup(&q, &query, state.store.len(), &mut lookup, &mut verify)
            .unwrap();
        let (mut actual_lookup, mut actual_verify) =
            (LookupCounters::default(), VerifyCounters::default());
        let actual = state
            .store
            .lookup_controlled(
                &q,
                &query,
                state.store.len(),
                &mut actual_lookup,
                &mut actual_verify,
                || Ok(()),
            )
            .unwrap();
        assert_eq!(reference, actual);
        assert_eq!(lookup, actual_lookup);
        assert_eq!(verify, actual_verify);
    }
    let q = QueryImage::new(apply([15, 15], [15, 15])).unwrap();
    let query = Query::new(q.core.clone(), Phase::Apply);
    let mut calls = 0;
    let error = state
        .store
        .lookup_controlled(
            &q,
            &query,
            state.store.len(),
            &mut LookupCounters::default(),
            &mut VerifyCounters::default(),
            || {
                calls += 1;
                if calls == 2 { Err(CANCELLED) } else { Ok(()) }
            },
        )
        .unwrap_err();
    assert_eq!(error, CANCELLED);
    assert_eq!(calls, 2);
}

#[test]
fn source_row_layout_diagnostic() {
    // Prospective transport shapes only: no resolver/production layout change.
    #[allow(dead_code)]
    enum PackedTag {
        Stored(Verified),
        Candidate(u16),
    }
    type Payload<const N: usize> = (QueryImage<N>, Query<N>);

    fn layout<T>() -> Value {
        serde_json::json!({"size": std::mem::size_of::<T>(), "align": std::mem::align_of::<T>()})
    }
    fn reserved<T>(slots: usize) -> (usize, usize) {
        let mut values = Vec::<T>::new();
        values.try_reserve_exact(slots).unwrap();
        // No values are initialized: report allocator-provided Vec capacity,
        // not resident/touched bytes, copy traffic, allocation latency or speed.
        (
            values.capacity(),
            values.capacity() * std::mem::size_of::<T>(),
        )
    }
    fn report<const N: usize>() {
        let current = reserved::<MissResolution<N>>(256);
        let tags = reserved::<PackedTag>(256);
        let no_payload = reserved::<Payload<N>>(0);
        let full_payload = reserved::<Payload<N>>(256);
        let break_even = (std::mem::size_of::<MissResolution<N>>() as f64
            - std::mem::size_of::<PackedTag>() as f64)
            / std::mem::size_of::<Payload<N>>() as f64;
        println!(
            "source_row_layout {}",
            serde_json::json!({
                "arity": N,
                "scope": "generic type layout and reserve-only backing capacity; Vec lengths zero; excludes allocator overhead/Vec headers; not bytes touched, copy cost, RSS or speed",
                "types": {
                    "miss_resolution": layout::<MissResolution<N>>(),
                    "verified": layout::<Verified>(),
                    "query_image": layout::<QueryImage<N>>(),
                    "query": layout::<Query<N>>(),
                    "prospective_packed_tag": layout::<PackedTag>(),
                    "prospective_candidate_payload": layout::<Payload<N>>()
                },
                "requested_rows": 256,
                "reserved_capacity_slots_and_bytes": {
                    "current_either_mix": current,
                    "packed_tags_either_mix": tags,
                    "all_stored_payload": no_payload,
                    "all_candidate_payload": full_payload
                },
                "total_backing_bytes": {
                    "current_either_mix": current.1,
                    "packed_all_stored": tags.1 + no_payload.1,
                    "packed_all_candidate": tags.1 + full_payload.1
                },
                "ideal_break_even_candidate_fraction": break_even,
                "break_even_scope": "(enum stride - tag stride) / payload stride; ideal exact capacities, not a measured workload mix"
            })
        );
    }
    report::<2>();
    report::<10>();
    report::<15>();
    report::<32>();
}
