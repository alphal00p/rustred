//! S2 unit tests (W2.0 protocol §17.2, S2 list). The merge tests drive P1-P3
//! with synthetic byte results on a two-axis owner; no native algebra runs.
use super::super::descendant_closure::Tracker;
use super::super::queue::{CompactDomain, Domain, Phase};
use super::anchors::{AnchorKind, MergedView};
use super::dispatch::{Dispatch, Refill};
use super::edges::EdgeStore;
use super::job::{
    BreakReason, ErrorKind, Job, JobResult, Miss, NativeKind, Reader, Writer, read_image,
    write_image,
};
use super::ledger6::{Counters, EPOCH_LIMIT, Entry6, Ledger6, LedgerError, Tag, Transition};
use super::merge::{self, Class, Fatal, MergeConfig, RecordOut, StopReason};
use super::state::EpochState;
use super::verify::{Container, QueryImage, VerifyCounters, verify};
use super::{admit_initial, records};
use rustred::solver::DomainPowerBounds;
use serde_json::Value;

const APPLY: [bool; 2] = [true, false];
const OTHER: [bool; 2] = [false, true];

fn domain(owner: [bool; 2], lower: [u64; 2], upper: [Option<u64>; 2]) -> Domain<2> {
    Domain {
        phase: Phase::Apply,
        owner,
        lower: lower.to_vec(),
        upper: upper.to_vec(),
        rank: Some(100),
        powers: DomainPowerBounds::default(),
    }
}

fn boxed(lower: [u64; 2], upper: [u64; 2]) -> Domain<2> {
    domain(APPLY, lower, [Some(upper[0]), Some(upper[1])])
}

fn image(d: &Domain<2>) -> CompactDomain<2> {
    CompactDomain::try_from_domain(d).unwrap()
}

fn state_with(initial: &[Domain<2>]) -> EpochState<2> {
    let mut state = EpochState::new(10_000, usize::MAX, usize::MAX);
    for d in initial {
        admit_initial(&mut state, d).unwrap();
    }
    state.p0 = state.watermark();
    state.tracker = Tracker::new(state.p0 as usize);
    state
}

fn result(job: &Job<2>, misses: &[Domain<2>]) -> JobResult<2> {
    let n = misses.len() as u64;
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
        misses: misses
            .iter()
            .enumerate()
            .map(|(ordinal, d)| Miss {
                ordinal: ordinal as u32,
                digest: image(d).digest().0,
                image: image(d),
            })
            .collect(),
    }
}

struct Rows(Vec<Value>);
impl RecordOut for Rows {
    fn reserve(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn push(&mut self, record: Value) -> Result<(), String> {
        self.0.push(record);
        Ok(())
    }
}

const CONFIG: MergeConfig = MergeConfig {
    frontier_stop: true,
    lockstep: true,
};

fn jobs(state: &mut EpochState<2>, dispatch: &mut Dispatch, want: usize) -> Vec<Job<2>> {
    match dispatch.refill(state, want) {
        Refill::Jobs(jobs) => jobs,
        _ => panic!("expected jobs"),
    }
}

fn merge_cut(
    state: &mut EpochState<2>,
    dispatch: &mut Dispatch,
    rows: &mut Rows,
    results: Vec<JobResult<2>>,
) -> Result<merge::Applied, Fatal> {
    let bytes = results.iter().map(JobResult::encode).collect();
    let checked = merge::p1_check(state, bytes, CONFIG)?;
    assert!(checked.stop.is_none());
    let plan = merge::p2_plan(state, &checked)?;
    merge::p3_preflight(state, &checked, &plan, rows).expect("preflight");
    merge::p3_apply(
        state,
        checked,
        plan,
        CONFIG,
        &records::Builder,
        rows,
        &mut |id, attempts| dispatch.requeue(id, attempts),
    )
}

fn entry(state: &EpochState<2>, id: u32) -> Entry6 {
    state.ledger.get(id).unwrap()
}

// ---- ledger6 ------------------------------------------------------------------

#[test]
fn ledger6_roundtrip_boundaries() {
    let counters = Counters {
        attempts: 255,
        guard: 15,
        last_err: 255,
        dispatch_class: 255,
    };
    for entry in [
        Entry6::Pending(Counters::default()),
        Entry6::Pending(counters),
        Entry6::Reserved(counters),
        Entry6::Exhausted(counters),
        Entry6::Native {
            epoch: 0,
            residual: false,
            dband: false,
        },
        Entry6::Native {
            epoch: EPOCH_LIMIT - 1,
            residual: true,
            dband: true,
        },
        Entry6::NativeFrontier {
            epoch: EPOCH_LIMIT - 1,
        },
        Entry6::NativeError {
            epoch: EPOCH_LIMIT - 1,
            err: 255,
        },
        Entry6::Alias { to: u32::MAX },
    ] {
        assert_eq!(Entry6::decode(entry.encode()), Ok(entry), "{entry:?}");
    }
    assert_eq!(Entry6::decode(7 << 61), Err(LedgerError::InvalidTag));
    // Payload bits outside a tag's layout are refused.
    assert_eq!(
        Entry6::decode((5 << 61) | (1 << 40)),
        Err(LedgerError::Malformed)
    );
    assert_eq!(
        Entry6::decode((2 << 61) | (1 << 55)),
        Err(LedgerError::Malformed)
    );
    assert_eq!(Entry6::decode(1 << 30), Err(LedgerError::Malformed));
}

/// The §4.3 table, written out independently of `allowed_from`.
fn table_allows(from: Tag, t: Transition) -> bool {
    use Tag::*;
    match t {
        Transition::T1New { .. } => false,
        Transition::T2Reserve | Transition::T3Alias { .. } => from == Pending,
        Transition::T4Native { .. }
        | Transition::T5Frontier { .. }
        | Transition::T6Error { .. }
        | Transition::T7Requeue { .. }
        | Transition::T8Exhaust { .. } => from == Reserved,
        Transition::T9Retry | Transition::T10ExhaustedAlias { .. } => from == Exhausted,
        Transition::T12ExhaustId => matches!(from, Pending | Reserved),
    }
}

/// A ledger whose ID 0 has tag `tag` (through legal transitions only).
fn ledger_at(tag: Tag) -> Ledger6 {
    let mut ledger = Ledger6::default();
    ledger
        .apply(0, Transition::T1New { dispatch_class: 0 })
        .unwrap();
    ledger
        .apply(1, Transition::T1New { dispatch_class: 0 })
        .unwrap();
    ledger
        .apply(2, Transition::T1New { dispatch_class: 0 })
        .unwrap();
    let path: &[Transition] = match tag {
        Tag::Pending => &[],
        Tag::Reserved => &[Transition::T2Reserve],
        Tag::Native => &[
            Transition::T2Reserve,
            Transition::T4Native {
                epoch: 1,
                residual: false,
                dband: false,
            },
        ],
        Tag::NativeFrontier => &[Transition::T2Reserve, Transition::T5Frontier { epoch: 1 }],
        Tag::NativeError => &[
            Transition::T2Reserve,
            Transition::T6Error { epoch: 1, err: 2 },
        ],
        Tag::Alias => &[Transition::T3Alias { to: 2 }],
        Tag::Exhausted => &[Transition::T12ExhaustId],
    };
    for &t in path {
        ledger.apply(0, t).unwrap();
    }
    assert_eq!(ledger.tag(0), Some(tag));
    ledger
}

#[test]
fn ledger6_transition_table_exhaustive() {
    for from in Tag::ALL {
        for t in Transition::KINDS {
            if matches!(t, Transition::T1New { .. }) {
                continue;
            }
            let mut ledger = ledger_at(from);
            let before = ledger.words().to_vec();
            let counts = ledger.counts();
            let applied = ledger.apply(0, t);
            assert_eq!(
                applied.is_ok(),
                table_allows(from, t),
                "{from:?} x {t:?}: {applied:?}"
            );
            if applied.is_err() {
                assert_eq!(ledger.words(), before.as_slice(), "refusal must not mutate");
                assert_eq!(ledger.counts(), counts);
            }
            assert_eq!(ledger.recount().unwrap(), ledger.counts());
        }
    }
    // T1 only at the end; aliases only forward.
    let mut ledger = Ledger6::default();
    assert_eq!(
        ledger.apply(1, Transition::T1New { dispatch_class: 0 }),
        Err(LedgerError::InvalidId)
    );
    ledger
        .apply(0, Transition::T1New { dispatch_class: 0 })
        .unwrap();
    assert_eq!(
        ledger.apply(0, Transition::T3Alias { to: 0 }),
        Err(LedgerError::BackwardAlias)
    );
    // T7 that would reach the limits must be T8.
    let mut ledger = ledger_at(Tag::Reserved);
    let requeue = Transition::T7Requeue {
        d_attempts: 8,
        d_guard: 0,
        last_err: 1,
    };
    assert!(ledger.apply(0, requeue).is_err());
}

#[test]
fn epoch_u48_preflight() {
    let mut ledger = ledger_at(Tag::Reserved);
    assert_eq!(
        ledger.apply(0, Transition::T5Frontier { epoch: EPOCH_LIMIT }),
        Err(LedgerError::Overflow)
    );
    let mut state = state_with(&[boxed([0, 0], [1, 1])]);
    let mut dispatch = Dispatch::new();
    let job = jobs(&mut state, &mut dispatch, 64).remove(0);
    let bytes = vec![result(&job, &[boxed([4, 4], [5, 5])]).encode()];
    let checked = merge::p1_check(&mut state, bytes, CONFIG).unwrap();
    let plan = merge::p2_plan(&mut state, &checked).unwrap();
    state.k = EPOCH_LIMIT - 1;
    let mut rows = Rows(Vec::new());
    assert_eq!(
        merge::p3_preflight(&mut state, &checked, &plan, &mut rows).err(),
        Some(StopReason::Capacity)
    );
    // A failed preflight changes nothing logical.
    assert_eq!(state.watermark(), 1);
    assert_eq!(entry(&state, 0).tag(), Tag::Reserved);
}

// ---- job codecs ----------------------------------------------------------------

#[test]
fn job_and_result_bytes_roundtrip_and_image_is_canonical() {
    let d = domain(OTHER, [3, 0], [None, Some(7)]);
    let job = Job {
        seq: (1 << 40) | 5,
        parent: 9,
        v0: 3,
        attempts: 1,
        flags: 0,
        image: image(&d),
    };
    assert_eq!(Job::<2>::decode(&job.encode()), Ok(job.clone()));
    let mut r = result(&job, &[boxed([1, 2], [3, 4]), d.clone()]);
    r.frontiers = vec![br#"{"kind":"x"}"#.to_vec()];
    r.error = Some("text".into());
    r.scope = Some(super::job::Scope {
        anchor: 2,
        cut: -3,
        residual: DomainPowerBounds {
            max_positive_power: Some(4),
            min_power_difference: None,
            max_power_difference: Some(-4),
        },
    });
    assert_eq!(JobResult::<2>::decode(&r.encode()), Ok(r.clone()));
    let mut bytes = r.encode();
    bytes.push(0);
    assert!(JobResult::<2>::decode(&bytes).is_err(), "trailing bytes");
    // A non-canonical absent value in an image is refused.
    let mut w = Writer::default();
    write_image(&mut w, &image(&d));
    let mut raw = w.0;
    let rank_value_at = 1 + 4 + 1;
    raw[rank_value_at - 1] = 0; // rank absent, value still nonzero
    assert!(read_image::<2>(&mut Reader::new(&raw)).is_err());
}

// ---- verify --------------------------------------------------------------------

#[test]
fn container_verify_matrix() {
    let outer = image(&boxed([0, 0], [5, 5]));
    let domains = [outer];
    let mut counters = VerifyCounters::default();
    let q = |d: &Domain<2>| QueryImage::new(image(d)).unwrap();
    let inner = q(&boxed([1, 1], [2, 2]));
    let stored = |id, len| Container::Stored {
        id,
        domains: &domains,
        published_len: len,
    };
    assert!(verify(stored(0, 1), &inner, &mut counters).is_some());
    assert!(
        verify(stored(0, 0), &inner, &mut counters).is_none(),
        "range"
    );
    assert!(
        verify(stored(1, 1), &inner, &mut counters).is_none(),
        "range"
    );
    // Another owner, and another phase, with the same box.
    let other_owner = q(&domain(OTHER, [1, 1], [Some(2), Some(2)]));
    assert!(verify(stored(0, 1), &other_owner, &mut counters).is_none());
    let mut route = boxed([1, 1], [2, 2]);
    route.phase = Phase::Route;
    assert!(verify(stored(0, 1), &q(&route), &mut counters).is_none());
    // An EMPTY query of another owner (A capped below its minimum): the
    // native predicate would accept it, the explicit owner check refuses.
    let mut empty = domain(OTHER, [0, 0], [None, None]);
    empty.powers.max_positive_power = Some(0);
    let empty = q(&empty);
    assert!(empty.core.extrema().is_none(), "the fixture is EMPTY");
    assert!(verify(stored(0, 1), &empty, &mut counters).is_none());
    // JobMiss and Planned ranges.
    let miss = |ordinal, current| Container::JobMiss {
        ordinal,
        current,
        image: &outer,
    };
    assert!(verify(miss(0, 1), &inner, &mut counters).is_some());
    assert!(verify(miss(1, 1), &inner, &mut counters).is_none());
    let planned = |pos, survivors| Container::Planned {
        pos,
        survivors,
        image: &outer,
    };
    assert!(verify(planned(0, 1), &inner, &mut counters).is_some());
    assert!(verify(planned(1, 1), &inner, &mut counters).is_none());
    // Not contained.
    assert!(verify(stored(0, 1), &q(&boxed([4, 4], [6, 6])), &mut counters).is_none());
    assert!(counters.refused_bucket >= 3 && counters.refused_range >= 4);
}

// ---- merge ----------------------------------------------------------------------

#[test]
fn id_assignment_first_parent_ordinal_and_antichain() {
    let mut state = state_with(&[boxed([0, 0], [1, 1]), boxed([0, 10], [1, 11])]);
    let mut dispatch = Dispatch::new();
    let mut rows = Rows(Vec::new());
    let batch = jobs(&mut state, &mut dispatch, 64);
    assert_eq!(batch.iter().map(|j| j.parent).collect::<Vec<_>>(), [0, 1]);
    // Parent 0: a point, then a box containing it; parent 1: another box
    // first, then the point again. Survivors in (first parent, ordinal).
    let point = boxed([3, 0], [3, 0]);
    let wide = boxed([2, 0], [4, 0]);
    let other = boxed([8, 8], [9, 9]);
    let results = vec![
        result(&batch[1], &[other.clone(), point.clone()]),
        result(&batch[0], &[point.clone(), wide.clone()]),
    ];
    merge_cut(&mut state, &mut dispatch, &mut rows, results).unwrap();
    assert_eq!(state.watermark(), 4);
    // `point` is folded into `wide` (antichain); wide came from parent 0
    // (position 0, ordinal 1), other from parent 1 (position 1, ordinal 0).
    assert_eq!(state.store.domains[2], image(&wide));
    assert_eq!(state.store.domains[3], image(&other));
    assert_eq!(state.counters.antichain_folded, 1);
    // Parent 0's edges: point and wide both resolve to 2; parent 1: 2 and 3.
    let run = |source: u32| {
        let log = state.edges.log();
        let mut at = 0;
        while at < log.len() {
            let (s, n) = (log[at], log[at + 1] as usize);
            if s == source {
                return log[at + 2..at + 2 + n].to_vec();
            }
            at += 2 + n;
        }
        panic!("no run for {source}");
    };
    assert_eq!(run(0), [2]);
    assert_eq!(run(1), [2, 3]);
    assert!(state.seal_rule_holds());
    assert_eq!(state.k, 1);
    for id in [0, 1] {
        assert_eq!(
            entry(&state, id),
            Entry6::Native {
                epoch: 1,
                residual: false,
                dband: false
            }
        );
    }
    // Order independence: the same misses in the other order within parents.
    let mut again = state_with(&[boxed([0, 0], [1, 1]), boxed([0, 10], [1, 11])]);
    let mut dispatch = Dispatch::new();
    let batch = jobs(&mut again, &mut dispatch, 64);
    let results = vec![
        result(&batch[0], &[point.clone(), wide.clone()]),
        result(&batch[1], &[other.clone(), point.clone()]),
    ];
    merge_cut(&mut again, &mut dispatch, &mut Rows(Vec::new()), results).unwrap();
    assert_eq!(again.store.domains, state.store.domains);
    assert_eq!(again.ledger.words(), state.ledger.words());
    assert_eq!(again.edges.log(), state.edges.log());
}

#[test]
fn transfers_protected_prefix_reserved_and_forward_aliases() {
    // P0 = 2; dispatch one ID at a time so Pending IDs exist beside a merge.
    let mut state = state_with(&[boxed([0, 0], [1, 1]), boxed([2, 2], [2, 2])]);
    let mut dispatch = Dispatch::new();
    let mut rows = Rows(Vec::new());
    let first = jobs(&mut state, &mut dispatch, 1).remove(0);
    assert_eq!(first.parent, 0);
    // A survivor containing the protected Pending ID 1 and the Reserved
    // parent 0 itself: both retire from the lookup only.
    let big = boxed([0, 0], [3, 3]);
    let small = boxed([6, 6], [6, 6]);
    merge_cut(
        &mut state,
        &mut dispatch,
        &mut rows,
        vec![result(&first, &[big.clone(), small.clone()])],
    )
    .unwrap();
    assert_eq!(
        entry(&state, 1).tag(),
        Tag::Pending,
        "protected: never transfers"
    );
    assert!(!state.is_live(0) && !state.is_live(1));
    assert_eq!(state.counters.transfers, 0);
    assert_eq!(state.counters.retired_lookup_only, 2);
    // Now ID 3 (small) is Pending and >= P0: a merge whose survivor contains
    // it transfers it (T3) to the survivor, with its one-edge alias run.
    let next = jobs(&mut state, &mut dispatch, 1).remove(0);
    assert_eq!(next.parent, 1);
    let cover = boxed([5, 5], [7, 7]);
    merge_cut(
        &mut state,
        &mut dispatch,
        &mut rows,
        vec![result(&next, &[cover.clone()])],
    )
    .unwrap();
    let to = state.watermark() - 1;
    assert_eq!(entry(&state, 3), Entry6::Alias { to });
    assert!(to > 3);
    assert_eq!(
        super::store::bucket_key(&state.store.domains[3]),
        super::store::bucket_key(&state.store.domains[to as usize])
    );
    assert!(state.is_sealed(3));
    assert!(state.seal_rule_holds());
    let alias = rows
        .0
        .iter()
        .find(|r| r["record_kind"] == "delegated_not_inspected")
        .unwrap();
    assert_eq!(alias["id"], 3);
    assert_eq!(alias["representative_id"], to);
}

#[test]
fn p1_stale_seq_event_parity_and_shipped_digest_are_c5() {
    let mut state = state_with(&[boxed([0, 0], [1, 1])]);
    let mut dispatch = Dispatch::new();
    let job = jobs(&mut state, &mut dispatch, 64).remove(0);
    let mut stale = result(&job, &[]);
    stale.seq += 1;
    assert!(merge::p1_check(&mut state, vec![stale.encode()], CONFIG).is_err());
    let mut parity = result(&job, &[boxed([3, 3], [3, 3])]);
    parity.stats_events += 1;
    assert!(merge::p1_check(&mut state, vec![parity.encode()], CONFIG).is_err());
    let mut forged = result(&job, &[boxed([3, 3], [3, 3])]);
    forged.misses[0].digest ^= 1;
    let checked = merge::p1_check(&mut state, vec![forged.encode()], CONFIG).unwrap();
    assert!(
        merge::p2_plan(&mut state, &checked).is_err(),
        "F1: shipped digest"
    );
    // Nothing merged.
    assert_eq!(state.k, 0);
    assert_eq!(entry(&state, 0).tag(), Tag::Reserved);
}

#[test]
fn result_class_matrix() {
    let job = Job {
        seq: 1,
        parent: 0,
        v0: 0,
        attempts: 0,
        flags: 0,
        image: image(&boxed([0, 0], [1, 1])),
    };
    let base = result(&job, &[]);
    let classify = |r: &JobResult<2>, last: u8| merge::classify(r, last).map(|c| c.0);
    assert_eq!(classify(&base, 0).unwrap(), Class::C0);
    let mut frontier = base.clone();
    frontier.frontiers = vec![b"{}".to_vec()];
    assert_eq!(classify(&frontier, 0).unwrap(), Class::C4);
    let mut cancelled = base.clone();
    cancelled.error_kind = ErrorKind::Cancelled;
    assert_eq!(classify(&cancelled, 0).unwrap(), Class::C1);
    for (reason, class) in [
        (BreakReason::ResolverRange, Class::C2),
        (BreakReason::ResolverSummary, Class::C2),
        (BreakReason::ResolverDiagnostic, Class::C2),
        (BreakReason::Allowance, Class::C2),
        (BreakReason::Alloc, Class::C1),
        (BreakReason::SpillIo, Class::C1),
        (BreakReason::Cancel, Class::C1),
    ] {
        let mut stop = base.clone();
        stop.error_kind = ErrorKind::ConsumerStop;
        stop.break_reason = reason;
        stop.emitted = 1;
        assert_eq!(classify(&stop, 0).unwrap(), class, "{reason:?}");
    }
    let mut native = base.clone();
    native.error_kind = ErrorKind::NativeFailure;
    assert_eq!(classify(&native, 0).unwrap(), Class::C2);
    let mut panic = base.clone();
    panic.panic = true;
    assert_eq!(classify(&panic, 0).unwrap(), Class::C3);
    assert_eq!(
        classify(&panic, 4).unwrap(),
        Class::C2,
        "a recurring C3 is C2"
    );
    let mut other = base.clone();
    other.error_kind = ErrorKind::Other;
    assert_eq!(classify(&other, 0).unwrap(), Class::C3);
    let mut protocol = base.clone();
    protocol.break_reason = BreakReason::Protocol;
    assert!(classify(&protocol, 0).is_err(), "C5");
    let mut bad = base.clone();
    bad.break_reason = BreakReason::ResolverRange;
    assert!(
        classify(&bad, 0).is_err(),
        "break without consumer stop is C5"
    );
}

#[test]
fn c2_error_stops_and_c1_requeues_without_penalty() {
    let mut state = state_with(&[boxed([0, 0], [1, 1]), boxed([4, 4], [5, 5])]);
    let mut dispatch = Dispatch::new();
    let batch = jobs(&mut state, &mut dispatch, 64);
    let mut error = result(&batch[0], &[boxed([2, 2], [2, 2])]);
    error.error_kind = ErrorKind::ConsumerStop;
    error.break_reason = BreakReason::ResolverRange;
    error.emitted += 1;
    error.error = Some("stopped".into());
    let mut cancelled = result(&batch[1], &[]);
    cancelled.error_kind = ErrorKind::Cancelled;
    let mut rows = Rows(Vec::new());
    let applied = merge_cut(&mut state, &mut dispatch, &mut rows, vec![error, cancelled]).unwrap();
    assert_eq!(applied.stop, Some(StopReason::ErrorStop));
    assert!(matches!(
        entry(&state, 0),
        Entry6::NativeError { epoch: 1, .. }
    ));
    assert!(!state.is_sealed(0), "NativeError never seals");
    // The emitted prefix's successor is merged (an obligation).
    assert_eq!(state.watermark(), 3);
    match entry(&state, 1) {
        Entry6::Reserved(c) => assert_eq!(c.attempts, 0, "stop causes never penalize"),
        other => panic!("{other:?}"),
    }
    assert_eq!(dispatch.queued(), (1, 0));
    assert!(state.seal_rule_holds());
}

#[test]
fn cross_phase_same_owner_misses_stay_apart() {
    let mut state = state_with(&[boxed([0, 0], [1, 1])]);
    let mut dispatch = Dispatch::new();
    let job = jobs(&mut state, &mut dispatch, 64).remove(0);
    let apply = boxed([3, 3], [4, 4]);
    let mut route = apply.clone();
    route.phase = Phase::Route;
    let mut rows = Rows(Vec::new());
    merge_cut(
        &mut state,
        &mut dispatch,
        &mut rows,
        vec![result(&job, &[route, apply])],
    )
    .unwrap();
    // Two survivors in distinct buckets (never contained across phase).
    assert_eq!(state.watermark(), 3);
    assert_ne!(
        state.store.domains[1].phase(),
        state.store.domains[2].phase()
    );
    assert_eq!(state.counters.antichain_folded, 0);
}

#[test]
fn termination_and_refill_progress_assertion() {
    let mut state = state_with(&[boxed([0, 0], [1, 1])]);
    let mut dispatch = Dispatch::new();
    let job = jobs(&mut state, &mut dispatch, 64).remove(0);
    let mut rows = Rows(Vec::new());
    merge_cut(
        &mut state,
        &mut dispatch,
        &mut rows,
        vec![result(&job, &[])],
    )
    .unwrap();
    assert!(matches!(dispatch.refill(&mut state, 64), Refill::Drained));
    // A Reserved ID that is neither in flight nor queued: a stall (C5).
    let mut stalled = state_with(&[boxed([0, 0], [1, 1])]);
    stalled.ledger.apply(0, Transition::T2Reserve).unwrap();
    assert!(matches!(
        Dispatch::new().refill(&mut stalled, 64),
        Refill::Stalled
    ));
}

#[test]
fn append_run_strictly_increasing_and_unsealed_source() {
    let mut edges = EdgeStore::new();
    assert!(edges.append_run(1, &[2, 3], false).is_ok());
    assert!(edges.append_run(1, &[3, 2], false).is_err());
    assert!(edges.append_run(1, &[2, 2], false).is_err());
    assert!(edges.append_run(1, &[2], true).is_err());
    assert_eq!(edges.runs(), 1);
}

#[test]
fn merge_numbering_visibility_and_snapshot_epochs() {
    let mut state = state_with(&[boxed([0, 0], [1, 1])]);
    let mut dispatch = Dispatch::new();
    let job = jobs(&mut state, &mut dispatch, 64).remove(0);
    assert_eq!(job.v0, 0);
    let mut rows = Rows(Vec::new());
    merge_cut(
        &mut state,
        &mut dispatch,
        &mut rows,
        vec![result(&job, &[boxed([3, 3], [3, 3])])],
    )
    .unwrap();
    assert_eq!(state.k, 1, "merge k+1 turns S_k into S_k+1");
    assert_eq!(entry(&state, 0).merge_epoch(), Some(1));
    let next = jobs(&mut state, &mut dispatch, 64).remove(0);
    assert_eq!(next.v0, 1);
    // G2' planning reads the snapshot's MergedView: a native merged after v0
    // is never visible (test-only planner seam).
    let mut view = MergedView::default();
    view.push(0, 5, 1, AnchorKind::G2Native);
    view.push(0, 6, 3, AnchorKind::G2Native);
    assert_eq!(view.visible(0, 2).collect::<Vec<_>>(), [5]);
    assert_eq!(view.visible(0, 0).count(), 0);
}

#[test]
fn f20_refused_lanes() {
    use crate::{OwnerDomainMatchRequest, OwnerDomainWalkRequest, OwnerDomainWalkSchedulingPolicy};
    let base = || {
        let mut request =
            OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(String::new(), String::new()));
        request.publication_policy = crate::OwnerDomainWalkPublicationPolicy::Epoch;
        request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
            lookahead: std::num::NonZeroUsize::new(4).unwrap(),
        };
        request
    };
    assert!(super::admit(&base()).is_ok());
    let mut capped = base();
    capped.max_containment_checks = Some(10);
    assert!(super::admit(&capped).is_err());
    let mut split = base();
    split.apply_subdivision = Some(crate::OwnerDomainWalkApplySubdivision { axis: 0, cut: 1 });
    assert!(super::admit(&split).is_err());
    let mut inspect_all = base();
    inspect_all.scheduling_policy = OwnerDomainWalkSchedulingPolicy::InspectAll;
    assert!(super::admit(&inspect_all).is_err());
    let mut resume = base();
    resume.checkpoint = Some(crate::OwnerDomainWalkCheckpointOptions {
        resume: true,
        ..crate::OwnerDomainWalkCheckpointOptions::new("unused")
    });
    assert!(super::admit(&resume).is_err());
}

#[test]
fn a3_bucket_interning_rebuilds_in_id_order() {
    let state = state_with(&[
        boxed([0, 0], [1, 1]),
        domain(OTHER, [0, 0], [Some(1), Some(1)]),
        boxed([4, 4], [5, 5]),
    ]);
    let rebuilt = state_with(
        &state
            .store
            .domains
            .iter()
            .map(CompactDomain::expand)
            .collect::<Vec<_>>(),
    );
    assert_eq!(rebuilt.store.bucket_of, state.store.bucket_of);
}

#[test]
fn antichain_indexed_equals_pairwise() {
    // Deterministic pseudo-random boxes of one bucket, with small coordinates
    // (many inclusions) and rank caps that make distinct images summary-
    // equivalent (mutual inclusion, decided by the canonical key).
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = |bound: u64| {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) % bound
    };
    for round in 0..4 {
        let mut images = Vec::new();
        for _ in 0..(90 + 40 * round) {
            let (a, b) = (next(6), next(6));
            let (c, d) = (a + next(4), b + next(4));
            let mut d0 = boxed([a, b], [c, d]);
            d0.rank = if next(3) == 0 {
                Some(b as u32 + next(3) as u32)
            } else {
                Some(100)
            };
            if !images.contains(&image(&d0)) {
                images.push(image(&d0));
            }
        }
        let (pairwise, indexed) = merge::antichain_both_ways(&images);
        assert_eq!(pairwise, indexed, "round {round}");
        assert!(pairwise.iter().any(|&s| s) && pairwise.iter().any(|&s| !s));
    }
}
