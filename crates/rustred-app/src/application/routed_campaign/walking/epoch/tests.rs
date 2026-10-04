//! S2 unit tests (W2.0 protocol §17.2, S2 list). The merge tests drive P1-P3
//! with synthetic byte results on a two-axis owner; no native algebra runs.
use super::super::descendant_closure::Tracker;
use super::super::queue::{CompactDomain, Domain, Phase};
use super::anchors::{
    AnchorKind, AnchorMap, AnchorRecord, AnchorRef, AnchorScope, AnchorView, AnchorViolation, Lent,
    MergedView, Piece,
};
use super::dispatch::{Dispatch, Refill};
use super::edges::EdgeStore;
use super::job::{
    BreakReason, ErrorKind, G2Part, Job, JobResult, Miss, NativeKind, Reader, Scope, Writer,
    read_image, write_image,
};
use super::ledger6::{
    Counters, EPOCH_LIMIT, Entry6, Ledger6, LedgerError, MAX_ATTEMPTS, MAX_GUARD, Tag, Transition,
    err_class,
};
use super::merge::{self, Class, Fatal, MergeConfig, RecordOut, StopReason};
use super::state::{EpochState, NODE_ANCHORED, NODE_RESIDUAL};
use super::store::{ExactIndex, Store};
use super::verify::{Container, QueryImage, VerifyCounters, verify};
use super::{AdmissionError, admit_initial, anchor_self_check, certification, records};
use rustred::solver::DomainPowerBounds;
use serde_json::Value;
use std::sync::atomic::AtomicBool;

mod admission;
mod compact_images;
mod finite_replay;
mod rescue;
mod snapshot;
mod union_witness;

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
        g2: None,
        finite_replay: None,
        lookup: None,
        misses: misses
            .iter()
            .enumerate()
            .map(|(ordinal, d)| Miss {
                ordinal: ordinal as u32,
                digest: image(d).digest().0,
                image: image(d),
                target: None,
            })
            .collect(),
    }
}

struct Rows(Vec<Value>);
impl RecordOut for Rows {
    fn reserve(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn push(&mut self, record: records::typed::Record) -> Result<(), String> {
        self.0.push(record.project()?);
        Ok(())
    }
}

const CONFIG: MergeConfig = MergeConfig {
    frontier_stop: true,
    lockstep: true,
    g2: false,
    finite_replay: None,
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
    merge_cut_with(state, dispatch, rows, results, CONFIG)
}

fn merge_cut_with(
    state: &mut EpochState<2>,
    dispatch: &mut Dispatch,
    rows: &mut Rows,
    results: Vec<JobResult<2>>,
    config: MergeConfig,
) -> Result<merge::Applied, Fatal> {
    let bytes = results.iter().map(JobResult::encode).collect();
    let checked = merge::p1_check(state, bytes, config)?;
    assert!(checked.stop.is_none());
    let plan = merge::p2(state, &checked)?;
    merge::p3_preflight(state, &checked, &plan, rows).expect("preflight");
    merge::p3_apply(
        state,
        checked,
        plan,
        config,
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
    assert_eq!(Entry6::decode(7 << 61), Ok(Entry6::Abandoned { epoch: 0 }));
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
        Transition::T13Abandon { .. } => from == Reserved,
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
        Tag::Abandoned => &[Transition::T2Reserve, Transition::T13Abandon { epoch: 1 }],
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
    let plan = merge::p2(&mut state, &checked).unwrap();
    state.k = EPOCH_LIMIT - 1;
    let mut rows = Rows(Vec::new());
    assert_eq!(
        merge::p3_preflight(&mut state, &checked, &plan, &mut rows).err(),
        Some(merge::PreflightError::Stop(StopReason::Capacity))
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
        merge::p2(&mut state, &checked).is_err(),
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
        stop.stats_events = 1;
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
fn legacy_g2_result_is_protocol_fatal_not_coverage_or_retry() {
    use super::super::g2::G2Scope;
    use super::super::inspection::{Finished, NativeStats};
    use super::resolve::Resolver;

    let job = Job {
        seq: 1,
        parent: 0,
        v0: 0,
        attempts: 0,
        flags: 0,
        image: image(&boxed([0, 0], [1, 1])),
    };
    let result = Resolver::new().finish(
        &job,
        Finished {
            stats: NativeStats::ApplyG2(
                Default::default(),
                G2Scope {
                    snapshot: 0,
                    residual: None,
                    anchors: 1,
                },
            ),
            error: None,
            error_kind: "none",
            seconds: 0.0,
        },
    );
    // Check the actual inspector wire boundary too: it must retain C5.
    let decoded = JobResult::<2>::decode(&result.encode()).unwrap();
    assert_eq!(decoded.break_reason, BreakReason::Protocol);
    assert!(!decoded.panic);
    assert_eq!(decoded.kind, NativeKind::G2Residual);
    assert!(decoded.scope.is_none() && decoded.g2.is_none());
    for prior_error in [0, 4 /* C3's persisted last-error code */] {
        assert!(merge::classify(&decoded, prior_error).is_err());
    }
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
    error.stats_events += 1;
    error.error = Some("stopped".into());
    let mut cancelled = result(&batch[1], &[]);
    cancelled.error_kind = ErrorKind::Cancelled;
    let mut rows = Rows(Vec::new());
    let applied = merge_cut(&mut state, &mut dispatch, &mut rows, vec![error, cancelled]).unwrap();
    assert_eq!(applied.stop, Some(StopReason::ErrorStop));
    assert_eq!(
        entry(&state, 0),
        Entry6::NativeError {
            epoch: 1,
            err: err_class::RESOLVER_RANGE
        }
    );
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
    view.push(0, 5, 1, Lent::Full);
    view.push(0, 6, 3, Lent::Full);
    assert_eq!(view.visible(0, 2).collect::<Vec<_>>(), [5]);
    assert_eq!(view.visible(0, 0).count(), 0);
    assert!(view.contains(5, 1) && !view.contains(6, 2) && view.contains(6, 3));
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
    assert!(
        super::admit(&resume).is_ok(),
        "same-engine CP6 resume is supported"
    );
    let mut g2 = base();
    g2.g2_residual_anchors = crate::OwnerDomainWalkG2ResidualAnchors::Union;
    assert!(super::admit(&g2).is_ok());
    let mut activation = base();
    activation.g2_activate_on_resume = true;
    assert!(
        super::admit(&activation)
            .unwrap_err()
            .to_string()
            .contains("G2'")
    );
    let mut rescue = base();
    rescue.amendments.push(crate::OwnerDomainWalkAmendment {
        path: "unused-amendment.json".into(),
        text: "not parsed before policy rejection".into(),
    });
    assert!(
        super::admit(&rescue)
            .unwrap_err()
            .to_string()
            .contains("rescue")
    );
}

#[test]
fn epoch_input_roots_keep_exact_roles_and_default_required() {
    let mut document = serde_json::json!({"schema":"rustred.owner-domain-queries.json.v2",
    "queries":[
        {"id":"required-helper-anchor", "owner":"1", "lower":[0], "upper":[null], "max_numerator_rank":0},
        {"id":"ordinary-name", "owner":"1", "lower":[0], "upper":[null], "max_numerator_rank":0}
    ]});
    let parse = |document: &Value| {
        crate::application::routed_campaign::matching::input::parse(
            &document.to_string(),
            1,
            2,
            usize::MAX,
        )
        .unwrap()
    };
    for query in parse(&document) {
        let row = super::input_row(&query, Some(0));
        assert_eq!(row["role"], "required");
        assert_eq!(row["role_declared"], false);
    }
    document["query_roles"] = serde_json::json!({
        "required":["required-helper-anchor"],"auxiliary":["ordinary-name"]});
    let queries = parse(&document);
    assert_eq!(super::input_row(&queries[0], Some(0))["role"], "required");
    let auxiliary = super::input_row(&queries[1], None);
    assert_eq!(auxiliary["role"], "auxiliary");
    assert_eq!(auxiliary["role_declared"], true);
    assert!(auxiliary["domain"].is_null());
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

// ---- fix round: anchors, tokens, parity, liveness, E3, preflight ------------

const G2: MergeConfig = MergeConfig {
    frontier_stop: true,
    lockstep: true,
    g2: true,
    finite_replay: None,
};

#[test]
fn indexed_epoch_planner_uses_dispatch_cut_and_drops_no_partial_plan_state() {
    let (mut state, _, _, job) = d_band_fixture(G2);
    super::g2::enable(&mut state);
    let index = state.g2_store.as_ref().unwrap();
    let query = job.image.expand();
    let cancel = AtomicBool::new(false);
    assert!(matches!(
        index.plan_at(1, &query, &cancel),
        super::super::g2::Outcome::Whole
    ));
    let super::super::g2::Outcome::Planned(plan) = index.plan_at(job.v0 + 1, &query, &cancel)
    else {
        panic!("visible initial Native supplies a real residual opportunity");
    };
    assert!(plan.residual.is_some());
    assert!(plan.anchors.iter().all(|a| a.stamp <= job.v0));
    assert!(index.peek(job.parent as usize).is_none());
    assert!(index.pending_pins().is_empty());
    index.append(
        &query.owner,
        super::super::g2::Store::entry(&query, 99, job.v0 + 1, super::super::g2::kind::NATIVE),
    );
    assert_eq!(
        index.plan_at(job.v0 + 1, &query, &cancel),
        super::super::g2::Outcome::Planned(plan)
    );
}

#[test]
fn epoch_planner_preflight_matches_persisted_union_for_lent_low_slices() {
    use super::super::g2 as planner;
    let original = boxed([0, 0], [8, 4]);
    let mut low = original.clone();
    low.powers.max_power_difference = Some(2);
    let index = planner::Store::new(None, 0, 2);
    index.append(
        &original.owner,
        planner::Store::entry(&low, 0, 1, planner::kind::INITIAL_D_BAND),
    );
    for full_cover in [false, true] {
        let mut query = boxed([0, 0], [5, 3]);
        if full_cover {
            query.powers.max_power_difference = Some(2);
        }
        let node = image(&query);
        let planner::Outcome::Planned(plan) = index.plan_at(2, &query, &AtomicBool::new(false))
        else {
            panic!("a replayable low-slice union is available");
        };
        assert_eq!(plan.residual.is_none(), full_cover);
        let (lower, upper) = node.raw_bounds();
        let record = AnchorRecord {
            node: 1,
            kind: AnchorKind::G2Residual,
            dispatch_version: 1,
            scope: AnchorScope::Residual(
                plan.residual
                    .into_iter()
                    .map(|(lo, hi)| Piece {
                        d_lo: Some(lo),
                        d_hi: Some(hi),
                        lower: lower.to_vec(),
                        upper: upper.to_vec(),
                    })
                    .collect(),
            ),
            anchors: plan
                .anchors
                .iter()
                .map(|anchor| AnchorRef {
                    anchor: anchor.id,
                    stamp: Some(anchor.stamp),
                    lent: Lent::LowSlice,
                })
                .collect(),
        };
        assert_eq!(
            super::anchors::union_cover(&node, &[image(&original), node], &record, &|id| (id == 0)
                .then_some(3)),
            Some(true),
            "preflight must reproduce P1's residual-first exact raw cells"
        );
        // A different cut must not gain the uninspected high-D band.
        assert_eq!(
            super::anchors::union_cover(&node, &[image(&original), node], &record, &|_| Some(2)),
            Some(false)
        );
        assert_eq!(
            super::anchors::union_cover(&node, &[image(&original), node], &record, &|_| None),
            None,
            "a missing lent-scope cut never means the anchor's full domain"
        );
        let mut malformed = record.clone();
        malformed.scope = AnchorScope::Residual(vec![Piece {
            d_lo: None,
            d_hi: None,
            lower: vec![0],
            upper: vec![u16::MAX],
        }]);
        assert_eq!(
            super::anchors::union_cover(&node, &[image(&original), node], &malformed, &|_| Some(3)),
            None,
            "malformed residual dimensions stay fail-closed"
        );
    }
}

#[test]
fn merged_lender_eligibility_excludes_route_frontier_and_empty_residual() {
    let (mut state, _, _, _) = d_band_fixture(G2);
    assert!(super::g2::eligible(
        &state.store.domains,
        &state.ledger,
        &state.anchors,
        0
    ));
    assert!(!super::g2::eligible(
        &state.store.domains,
        &state.ledger,
        &state.anchors,
        3
    ));
    state
        .anchors
        .push(AnchorRecord {
            node: 0,
            kind: AnchorKind::G2Residual,
            dispatch_version: 0,
            scope: AnchorScope::Residual(Vec::new()),
            anchors: vec![AnchorRef {
                anchor: 1,
                stamp: Some(1),
                lent: Lent::Full,
            }],
        })
        .unwrap();
    assert!(!super::g2::eligible(
        &state.store.domains,
        &state.ledger,
        &state.anchors,
        0
    ));
    let mut route = boxed([0, 0], [1, 1]);
    route.phase = Phase::Route;
    let mut route_state = state_with(&[route]);
    let mut dispatch = Dispatch::new();
    let job = jobs(&mut route_state, &mut dispatch, 1).remove(0);
    let mut native = result(&job, &[]);
    native.kind = NativeKind::Route;
    merge_cut_with(
        &mut route_state,
        &mut dispatch,
        &mut Rows(Vec::new()),
        vec![native],
        G2,
    )
    .unwrap();
    assert_eq!(route_state.merged_view.len(), 0);
    let mut frontier = state_with(&[boxed([0, 0], [1, 1])]);
    frontier.ledger.apply(0, Transition::T2Reserve).unwrap();
    frontier
        .ledger
        .apply(0, Transition::T5Frontier { epoch: 1 })
        .unwrap();
    assert!(!super::g2::eligible(
        &frontier.store.domains,
        &frontier.ledger,
        &frontier.anchors,
        0
    ));
}

/// The edge run of `source` in the log.
fn run_of(state: &EpochState<2>, source: u32) -> Vec<u32> {
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
}

/// Owner APPLY: A = x0 + 1, R = x1, D = A - R.
fn d_banded(lower: [u64; 2], upper: [u64; 2], d_min: Option<i64>) -> Domain<2> {
    let mut d = boxed(lower, upper);
    d.powers.min_power_difference = d_min;
    d
}

/// Initial IDs 0 = A0 ([0,20]^2 with D >= 6), 1 = a point P, 2 = an OTHER
/// owner box; P0 = 3. Merge 1 (v0 = 0) merges them all; P emits the node
/// N = [0,12] x [0,3] (ID 3) and a far point Z (ID 4, left Pending). The
/// returned job is N's, dispatched at v0 = 1.
fn d_band_fixture(config: MergeConfig) -> (EpochState<2>, Dispatch, Rows, Job<2>) {
    let mut state = state_with(&[
        d_banded([0, 0], [20, 20], Some(6)),
        boxed([30, 30], [30, 30]),
        domain(OTHER, [0, 0], [Some(20), Some(20)]),
    ]);
    assert_eq!(state.p0, 3);
    let mut dispatch = Dispatch::new();
    let mut rows = Rows(Vec::new());
    let batch = jobs(&mut state, &mut dispatch, 64);
    let node = boxed([0, 0], [12, 3]);
    let far = boxed([40, 40], [40, 40]);
    let results = batch
        .iter()
        .map(|job| {
            if job.parent == 1 {
                result(job, &[node.clone(), far.clone()])
            } else {
                result(job, &[])
            }
        })
        .collect();
    merge_cut_with(&mut state, &mut dispatch, &mut rows, results, config).unwrap();
    assert_eq!(state.store.domains[3], image(&node));
    let job = jobs(&mut state, &mut dispatch, 1).remove(0);
    assert_eq!((job.parent, job.v0), (3, 1));
    assert_eq!(entry(&state, 4).tag(), Tag::Pending);
    (state, dispatch, rows, job)
}

fn partial(job: &Job<2>, anchor: u32, cut: i64) -> JobResult<2> {
    let mut r = result(job, &[]);
    r.kind = NativeKind::ApplyPartial;
    r.scope = Some(Scope {
        anchor,
        cut,
        residual: DomainPowerBounds {
            max_power_difference: Some(cut - 1),
            ..Default::default()
        },
    });
    r
}

fn g2_result(
    job: &Job<2>,
    kind: u8,
    anchors: &[(u32, u64, u8)],
    pieces: Vec<Piece>,
) -> JobResult<2> {
    let mut r = result(job, &[]);
    r.kind = NativeKind::G2Residual;
    r.g2 = Some(G2Part {
        kind,
        anchors: anchors.to_vec(),
        pieces,
    });
    r
}

#[test]
fn p1_initial_d_band_revalidation() {
    type Make = fn(&Job<2>) -> JobResult<2>;
    let forged: [(&str, Make); 5] = [
        ("anchor >= P0", |job| partial(job, 3, 6)),
        ("cross-bucket anchor", |job| partial(job, 2, 6)),
        ("residual is not the D < cut slice", |job| {
            let mut r = partial(job, 0, 6);
            r.scope.as_mut().unwrap().residual.max_power_difference = Some(4);
            r
        }),
        ("anchor does not contain the D >= cut slice", |job| {
            partial(job, 1, 6)
        }),
        ("a cut the anchor does not cover", |job| partial(job, 0, 5)),
    ];
    for (name, make) in forged {
        let (mut state, _, _, job) = d_band_fixture(CONFIG);
        let before = state.ledger.words().to_vec();
        assert!(
            merge::p1_check(&mut state, vec![make(&job).encode()], CONFIG).is_err(),
            "{name} must be C5"
        );
        assert_eq!(state.ledger.words(), before.as_slice(), "{name}");
        assert_eq!(state.anchors.len(), 0, "{name}");
    }
    // The honest scope merges; its anchor edge comes from P1's token.
    let (mut state, mut dispatch, mut rows, job) = d_band_fixture(CONFIG);
    merge_cut(
        &mut state,
        &mut dispatch,
        &mut rows,
        vec![partial(&job, 0, 6)],
    )
    .unwrap();
    assert_eq!(run_of(&state, 3), [0]);
    assert_eq!(
        entry(&state, 3),
        Entry6::Native {
            epoch: 2,
            residual: false,
            dband: true
        }
    );
    assert_eq!(
        state.nodes[3] & (NODE_ANCHORED | NODE_RESIDUAL),
        NODE_ANCHORED
    );
    assert_eq!(state.anchors.get(3).unwrap().d_band(), Some((0, 6)));
    anchor_self_check(&state).unwrap();
    let (resolutions, summary) = records::resolve(&state);
    assert_eq!(summary["partial_initial_inspections"], 1);
    assert_eq!(summary["partial_initial_blocked"], 0);
    assert_eq!(
        resolutions[3].status,
        super::super::delegation::ResolutionStatus::Discharged
    );
    let row = rows.0.iter().find(|r| r["id"] == 3).unwrap();
    assert_eq!(row["record_kind"], "partial_initial_overlap_inspection");
    assert_eq!(row["initial_overlap"]["anchor_id"], 0);
    let bytes = state.anchors.encode().unwrap();
    assert_eq!(
        AnchorMap::decode(&bytes, 2).unwrap(),
        state.anchors.records().to_vec()
    );
    // The G2' R3 rule on an InitialDBand native (the A0 -> B -> A0
    // construction of SND-2): B = 3 may lend only its low-D slice.
    let (mut state, mut dispatch, mut rows, job) = d_band_fixture(G2);
    merge_cut_with(
        &mut state,
        &mut dispatch,
        &mut rows,
        vec![partial(&job, 0, 6)],
        G2,
    )
    .unwrap();
    assert!(state.merged_view.contains(3, 2));
    let next = jobs(&mut state, &mut dispatch, 1).remove(0);
    assert_eq!((next.parent, next.v0), (4, 2));
    let full = g2_result(&next, 2, &[(3, 2, Lent::Full as u8)], vec![]);
    let error = merge::p1_check(&mut state, vec![full.encode()], G2)
        .err()
        .expect("full lending by an InitialDBand native is C5");
    assert!(error.0.contains("LentScopeMismatch"), "{}", error.0);
}

#[test]
fn p1_g2_results_need_the_flag_and_merge_through_cover_tokens() {
    let low = || vec![Piece::band(2, None, Some(5))];
    // Without a bound G2' flag (S2) every G2' result is C5.
    let (mut state, _, _, job) = d_band_fixture(CONFIG);
    let honest = g2_result(&job, 2, &[(0, 1, 0)], low());
    assert!(merge::p1_check(&mut state, vec![honest.encode()], CONFIG).is_err());
    // With the flag: A0 (merged at epoch 1, in MergedView at v0 = 1) plus the
    // residual D <= 5 cover N exactly.
    let (mut state, mut dispatch, mut rows, job) = d_band_fixture(G2);
    assert_eq!(state.merged_view.len(), 3);
    merge_cut_with(
        &mut state,
        &mut dispatch,
        &mut rows,
        vec![g2_result(&job, 2, &[(0, 1, 0)], low())],
        G2,
    )
    .unwrap();
    assert_eq!(run_of(&state, 3), [0]);
    assert_eq!(
        entry(&state, 3),
        Entry6::Native {
            epoch: 2,
            residual: true,
            dband: false
        }
    );
    assert_eq!(
        state.nodes[3] & (NODE_ANCHORED | NODE_RESIDUAL),
        NODE_ANCHORED | NODE_RESIDUAL
    );
    anchor_self_check(&state).unwrap();
    let row = rows.0.iter().find(|r| r["id"] == 3).unwrap();
    assert_eq!(row["record_kind"], "g2_residual_inspection");
    let (_, summary) = records::resolve(&state);
    assert_eq!(summary["g2_residual_records"], 1);
    assert_eq!(summary["g2_residual_blocked"], 0);
    // The §7 G2' mutations are C5 in P1 (fresh fixture each).
    let cases: [(&str, Vec<(u32, u64, u8)>, Vec<Piece>); 5] = [
        (
            "residual shrunk by one D layer",
            vec![(0, 1, 0)],
            vec![Piece::band(2, None, Some(4))],
        ),
        ("anchor stamp > dispatch version", vec![(0, 2, 0)], low()),
        (
            "anchor stamp is not its merge epoch",
            vec![(0, 0, 0)],
            low(),
        ),
        ("anchor not Native (Pending)", vec![(4, 1, 0)], low()),
        ("anchor in another bucket", vec![(2, 1, 0)], low()),
    ];
    for (name, anchors, pieces) in cases {
        let (mut state, _, _, job) = d_band_fixture(G2);
        let r = g2_result(&job, 2, &anchors, pieces);
        assert!(
            merge::p1_check(&mut state, vec![r.encode()], G2).is_err(),
            "{name} must be C5"
        );
    }
    // An anchor record on a node < P0 (R1): the initial parent 1's result.
    let mut state = state_with(&[
        d_banded([0, 0], [20, 20], Some(6)),
        boxed([30, 30], [30, 30]),
    ]);
    let batch = jobs(&mut state, &mut Dispatch::new(), 64);
    let initial = g2_result(&batch[1], 2, &[(0, 0, 0)], low());
    assert!(merge::p1_check(&mut state, vec![initial.encode()], G2).is_err());
}

#[test]
fn anchor_validate_every_violation() {
    use AnchorViolation as V;
    // Ledger: 0 plain Native (epoch 1), 1 InitialDBand native (epoch 1), 2 G2'
    // native (epoch 2), 3 NativeFrontier, 4 Pending, 5 both flags
    // (malformed), 6 plain Native missing from MergedView, 7 another bucket,
    // 11 plain Native whose anchor record says G2'. P0 = 8; node 9, epoch 3.
    let mut ledger = Ledger6::default();
    for id in 0..12 {
        ledger
            .apply(id, Transition::T1New { dispatch_class: 0 })
            .unwrap();
    }
    let mut native = |id: u32, epoch: u64, residual: bool, dband: bool| {
        ledger.apply(id, Transition::T2Reserve).unwrap();
        ledger
            .apply(
                id,
                Transition::T4Native {
                    epoch,
                    residual,
                    dband,
                },
            )
            .unwrap();
    };
    native(0, 1, false, false);
    native(1, 1, false, true);
    native(2, 2, true, false);
    native(5, 1, true, true);
    native(6, 1, false, false);
    native(7, 1, false, false);
    native(11, 1, false, false);
    ledger.apply(3, Transition::T2Reserve).unwrap();
    ledger
        .apply(3, Transition::T5Frontier { epoch: 1 })
        .unwrap();
    let same_bucket = |a: u32, _: u32| a != 7;
    let record_of = |id: u32| -> Option<(AnchorKind, Option<i64>)> {
        match id {
            1 => Some((AnchorKind::InitialDBand, Some(3))),
            2 | 11 => Some((AnchorKind::G2Residual, None)),
            _ => None,
        }
    };
    let visible = |id: u32, _: u64| id != 6;
    let cover_answer = std::cell::Cell::new(Some(true));
    let cover = |_: &AnchorRecord| -> Option<bool> { cover_answer.get() };
    let edges = |_: u32| -> Option<Vec<u32>> { Some(vec![1]) };
    let view = AnchorView {
        p0: 8,
        published_len: 12,
        arity: 2,
        ledger: &ledger,
        same_bucket: &same_bucket,
        record_of: &record_of,
        edges_of: None,
        merged_view: Some(&visible),
        cover: &cover,
    };
    let anchor = |anchor: u32, stamp: Option<u64>, lent: Lent| AnchorRef {
        anchor,
        stamp,
        lent,
    };
    let g2 = |anchors: Vec<AnchorRef>| AnchorRecord {
        node: 9,
        kind: AnchorKind::G2Residual,
        dispatch_version: 2,
        scope: AnchorScope::Residual(vec![Piece::band(2, None, Some(5))]),
        anchors,
    };
    let initial = |anchors: Vec<AnchorRef>| AnchorRecord {
        node: 9,
        kind: AnchorKind::InitialDBand,
        dispatch_version: 2,
        scope: AnchorScope::DBandCut(3),
        anchors,
    };
    let ok_g2 = g2(vec![anchor(0, Some(1), Lent::Full)]);
    assert_eq!(ok_g2.validate(&view, 3), Ok(()));
    assert_eq!(
        initial(vec![anchor(0, None, Lent::Full)]).validate(&view, 3),
        Ok(())
    );
    // An InitialDBand native lends its low-D slice: admissible.
    assert_eq!(
        g2(vec![anchor(1, Some(1), Lent::LowSlice)]).validate(&view, 3),
        Ok(())
    );
    let mut cases: Vec<(AnchorRecord, V)> = vec![
        (
            AnchorRecord {
                node: 7,
                ..ok_g2.clone()
            },
            V::NodeBelowP0,
        ),
        (g2(vec![]), V::NoAnchors),
        (
            g2(vec![
                anchor(0, Some(1), Lent::Full),
                anchor(0, Some(1), Lent::Full),
            ]),
            V::DuplicateAnchor,
        ),
        (
            g2(vec![anchor(12, Some(1), Lent::Full)]),
            V::AnchorOutOfRange,
        ),
        (
            initial(vec![
                anchor(0, None, Lent::Full),
                anchor(6, None, Lent::Full),
            ]),
            V::InitialDBandAnchorCount,
        ),
        (
            initial(vec![anchor(8, None, Lent::Full)]),
            V::InitialAnchorNotBelowP0,
        ),
        (g2(vec![anchor(7, Some(1), Lent::Full)]), V::CrossBucket),
        (
            initial(vec![anchor(1, None, Lent::Full)]),
            V::AnchorHasAnchors,
        ),
        (
            initial(vec![anchor(0, Some(1), Lent::Full)]),
            V::UnexpectedStamp,
        ),
        (
            initial(vec![anchor(0, None, Lent::LowSlice)]),
            V::LentScopeMismatch,
        ),
        (g2(vec![anchor(0, None, Lent::Full)]), V::MissingStamp),
        (
            g2(vec![anchor(0, Some(3), Lent::Full)]),
            V::StampAfterDispatch,
        ),
        (g2(vec![anchor(2, Some(1), Lent::Full)]), V::StampMismatch),
        (g2(vec![anchor(3, Some(1), Lent::Full)]), V::AnchorNotNative),
        (g2(vec![anchor(4, Some(1), Lent::Full)]), V::AnchorNotNative),
        (
            g2(vec![anchor(5, Some(1), Lent::Full)]),
            V::MalformedAnchorEntry,
        ),
        (
            AnchorRecord {
                dispatch_version: 3,
                ..ok_g2.clone()
            },
            V::NodeNotLaterThanAnchor,
        ),
        (g2(vec![anchor(6, Some(1), Lent::Full)]), V::NotInMergedView),
        // R3: an InitialDBand native never lends its full domain (SND-2).
        (
            g2(vec![anchor(1, Some(1), Lent::Full)]),
            V::LentScopeMismatch,
        ),
        (
            g2(vec![anchor(0, Some(1), Lent::LowSlice)]),
            V::LentScopeMismatch,
        ),
        (
            AnchorRecord {
                kind: AnchorKind::G2Native,
                ..g2(vec![anchor(2, Some(2), Lent::Full)])
            },
            V::AnchorKindInadmissible,
        ),
        (
            g2(vec![anchor(11, Some(1), Lent::Full)]),
            V::AnchorRecordMismatch,
        ),
        (
            AnchorRecord {
                scope: AnchorScope::DBandCut(3),
                ..ok_g2.clone()
            },
            V::ScopeKind,
        ),
        (
            AnchorRecord {
                scope: AnchorScope::Residual(vec![Piece::band(3, None, None)]),
                ..ok_g2.clone()
            },
            V::ScopeArity,
        ),
    ];
    // The A0 -> B -> A0 construction: a G2' record on the anchor A0 < P0.
    cases.push((
        AnchorRecord {
            node: 0,
            ..g2(vec![anchor(1, Some(1), Lent::LowSlice)])
        },
        V::NodeBelowP0,
    ));
    for (record, expected) in &cases {
        assert_eq!(record.validate(&view, 3), Err(*expected), "{record:?}");
    }
    cover_answer.set(Some(false));
    assert_eq!(ok_g2.validate(&view, 3), Err(V::UnionNotCovered));
    cover_answer.set(None);
    assert_eq!(ok_g2.validate(&view, 3), Err(V::UnionUndecided));
    cover_answer.set(Some(true));
    let with_edges = AnchorView {
        edges_of: Some(&edges),
        ..view
    };
    assert_eq!(ok_g2.validate(&with_edges, 3), Err(V::MissingAnchorEdge));
}

#[test]
fn anchors_section_roundtrip_and_checked_counts() {
    let mut map = AnchorMap::default();
    // 300 anchors (beyond a u8 count) and a residual of 3,000 pieces
    // (78,004 scope bytes, beyond a u16 length).
    let big = AnchorRecord {
        node: 40,
        kind: AnchorKind::G2Residual,
        dispatch_version: 7,
        scope: AnchorScope::Residual(
            (0..3000)
                .map(|i| Piece {
                    d_lo: Some(-(i as i64)),
                    d_hi: (i % 3 == 0).then_some(i as i64),
                    lower: vec![(i % 7) as u16, 0],
                    upper: vec![u16::MAX, 9],
                })
                .collect(),
        ),
        anchors: (0..300)
            .map(|a| AnchorRef {
                anchor: a,
                stamp: Some(u64::from(a) + 1),
                lent: if a % 2 == 0 {
                    Lent::Full
                } else {
                    Lent::LowSlice
                },
            })
            .collect(),
    };
    map.push(big.clone()).unwrap();
    map.push(AnchorRecord {
        node: 12,
        kind: AnchorKind::InitialDBand,
        dispatch_version: 3,
        scope: AnchorScope::DBandCut(-5),
        anchors: vec![AnchorRef {
            anchor: 2,
            stamp: None,
            lent: Lent::Full,
        }],
    })
    .unwrap();
    let bytes = map.encode().unwrap();
    let decoded = AnchorMap::decode(&bytes, 2).unwrap();
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0].node, 12, "sorted by node");
    assert_eq!(decoded[1], big);
    assert_eq!(map.get(40), Some(&big));
    // A second record for one node, a u64::MAX stamp, corrupt bytes: refused.
    assert!(map.push(big.clone()).is_err());
    let mut bad = AnchorMap::default();
    let mut forged = big.clone();
    forged.anchors[0].stamp = Some(u64::MAX);
    bad.push(forged).unwrap();
    assert!(bad.encode().is_err());
    assert!(AnchorMap::decode(&bytes[..bytes.len() - 1], 2).is_err());
    let mut reserved = bytes.clone();
    reserved[2 + 8 + 4 + 1] = 1;
    assert!(AnchorMap::decode(&reserved, 2).is_err());
}

#[test]
fn consumer_stop_parity_relations() {
    let job = Job {
        seq: 1,
        parent: 0,
        v0: 0,
        attempts: 0,
        flags: 0,
        image: image(&boxed([0, 0], [1, 1])),
    };
    let class = |r: &JobResult<2>| merge::classify(r, 0).map(|c| c.0);
    let with = |kind: ErrorKind, reason: BreakReason, counts: (u64, u64, u64)| {
        let mut r = result(&job, &[]);
        r.error_kind = kind;
        r.break_reason = reason;
        (r.emitted, r.accepted, r.stats_events) = counts;
        r
    };
    // A resolver break: the solver counts the breaking event before the
    // visitor refuses it (stats == emitted == accepted + 1).
    let stop = |counts| with(ErrorKind::ConsumerStop, BreakReason::ResolverRange, counts);
    assert_eq!(class(&stop((3, 2, 3))).unwrap(), Class::C2);
    assert!(class(&stop((3, 2, 2))).is_err());
    assert!(class(&stop((3, 3, 3))).is_err());
    // native_failure: emitted == accepted == stats over the prefix.
    let failure = |counts| with(ErrorKind::NativeFailure, BreakReason::None, counts);
    assert_eq!(class(&failure((2, 2, 2))).unwrap(), Class::C2);
    assert!(class(&failure((2, 2, 3))).is_err());
    assert!(class(&failure((2, 1, 2))).is_err());
    // conversion: the refused diagnostic is charged, never emitted.
    let conversion = |counts| with(ErrorKind::Conversion, BreakReason::None, counts);
    assert_eq!(class(&conversion((2, 2, 3))).unwrap(), Class::C2);
    assert!(class(&conversion((2, 2, 2))).is_err());
}

#[test]
fn error_class_codes_distinguish_c2_sources() {
    let job = Job {
        seq: 1,
        parent: 0,
        v0: 0,
        attempts: 0,
        flags: 0,
        image: image(&boxed([0, 0], [1, 1])),
    };
    let checked = |kind: ErrorKind, reason: BreakReason, panic: bool| merge::CheckedResult {
        result: JobResult {
            error_kind: kind,
            break_reason: reason,
            panic,
            ..result(&job, &[])
        },
        class: Class::C2,
        cause: None,
        recurring_panic: panic,
        anchors: None,
    };
    for (kind, reason, panic, code) in [
        (
            ErrorKind::NativeFailure,
            BreakReason::None,
            false,
            err_class::NATIVE_FAILURE,
        ),
        (
            ErrorKind::Conversion,
            BreakReason::None,
            false,
            err_class::CONVERSION,
        ),
        (
            ErrorKind::ConsumerStop,
            BreakReason::ResolverRange,
            false,
            err_class::RESOLVER_RANGE,
        ),
        (
            ErrorKind::ConsumerStop,
            BreakReason::ResolverSummary,
            false,
            err_class::RESOLVER_SUMMARY,
        ),
        (
            ErrorKind::ConsumerStop,
            BreakReason::ResolverDiagnostic,
            false,
            err_class::RESOLVER_DIAGNOSTIC,
        ),
        (
            ErrorKind::ConsumerStop,
            BreakReason::Allowance,
            false,
            err_class::ALLOWANCE,
        ),
        (
            ErrorKind::Other,
            BreakReason::None,
            true,
            err_class::RECURRING_PANIC,
        ),
        (
            ErrorKind::Other,
            BreakReason::None,
            false,
            err_class::RECURRING_UNKNOWN,
        ),
    ] {
        assert_eq!(merge::error_class(&checked(kind, reason, panic)), code);
        assert_ne!(err_class::name(code), "invalid");
    }
}

#[test]
fn ledger6_t8_requires_the_liveness_limits() {
    let exhaust = |d_attempts, d_guard| Transition::T8Exhaust {
        d_attempts,
        d_guard,
        last_err: 1,
    };
    let mut ledger = ledger_at(Tag::Reserved);
    let before = ledger.words().to_vec();
    assert!(ledger.apply(0, exhaust(1, 0)).is_err(), "attempts 1 < 8");
    assert!(ledger.apply(0, exhaust(0, 2)).is_err(), "guard 2 < 3");
    assert_eq!(ledger.words(), before.as_slice());
    assert!(ledger.apply(0, exhaust(MAX_ATTEMPTS, 0)).is_ok());
    let mut ledger = ledger_at(Tag::Reserved);
    // At restore the journal-derived guard arrives as d_guard.
    assert!(ledger.apply(0, exhaust(0, MAX_GUARD)).is_ok());
    assert_eq!(ledger.tag(0), Some(Tag::Exhausted));
}

#[test]
fn exact_index_forced_collision_and_duplicate_images_refused() {
    // Two distinct images under one forced digest: both found by image, a
    // third image not; the second insert lands in the overflow map.
    let a = image(&boxed([0, 0], [1, 1]));
    let b = image(&boxed([2, 2], [3, 3]));
    let c = image(&boxed([4, 4], [5, 5]));
    let domains = [a, b];
    let digest = 0xdead_beef_u64;
    let mut exact = ExactIndex::new();
    exact.try_reserve(&[digest, digest]).unwrap();
    exact.insert(digest, 0);
    exact.insert(digest, 1);
    assert_eq!(exact.get(digest, &a, &domains), Some(0));
    assert_eq!(exact.get(digest, &b, &domains), Some(1));
    assert_eq!(exact.get(digest, &c, &domains), None);
    assert_eq!(exact.get(digest ^ 1, &a, &domains), None);
    assert_eq!((exact.len(), exact.overflow_len()), (2, 1));
    let mut streamed = ExactIndex::new();
    for (id, domain) in domains.iter().enumerate() {
        streamed.try_reserve_one(digest).unwrap();
        streamed.insert(digest, id as u32);
        assert_eq!(streamed.get(digest, domain, &domains), Some(id as u32));
    }
    assert_eq!((streamed.len(), streamed.overflow_len()), (2, 1));
    // E3 and SND-7 in the store: a duplicate image or a wrong key is refused.
    let mut store = Store::<2>::new();
    let q = QueryImage::new(a).unwrap();
    let summary = super::super::queue::Query::new(q.core.clone(), a.phase()).compact;
    store.try_reserve(2).unwrap();
    store.exact.try_reserve(&[q.digest]).unwrap();
    assert_eq!(store.push(a, summary, q.digest), Ok(0));
    assert!(store.push(a, summary, q.digest).is_err(), "E3");
    assert!(store.push(b, summary, q.digest).is_err(), "SND-7");
    assert_eq!(store.len(), 1);
}

#[test]
fn survivor_geometry_single_source() {
    let mut state = state_with(&[boxed([0, 0], [1, 1])]);
    let mut dispatch = Dispatch::new();
    let job = jobs(&mut state, &mut dispatch, 64).remove(0);
    let shipped = [
        boxed([3, 3], [4, 5]),
        domain(OTHER, [1, 0], [None, Some(2)]),
    ];
    merge_cut(
        &mut state,
        &mut dispatch,
        &mut Rows(Vec::new()),
        vec![result(&job, &shipped)],
    )
    .unwrap();
    // Every stored fact of a new ID (image, summary, digest, bucket, exact
    // entry) derives from the one shipped image.
    for (offset, d) in shipped.iter().enumerate() {
        let id = 1 + offset as u32;
        let shipped = image(d);
        assert_eq!(state.store.domains[id as usize], shipped);
        let query = super::super::queue::Query::new(shipped.native_summary(), shipped.phase());
        assert!(state.store.summaries[id as usize] == query.compact);
        assert_eq!(
            state
                .store
                .exact
                .get(shipped.digest().0, &shipped, &state.store.domains),
            Some(id)
        );
        assert!(
            state
                .store
                .bucket_of
                .contains_key(&super::store::bucket_key(&shipped))
        );
    }
}

#[test]
fn p3_preflight_failure_is_noop_at_every_reserve() {
    for step in 0..merge::PREFLIGHT_STEPS {
        let (mut state, mut dispatch, _, job) = d_band_fixture(CONFIG);
        let bytes = vec![
            JobResult {
                ..partial(&job, 0, 6)
            }
            .encode(),
        ];
        let checked = merge::p1_check(&mut state, bytes, CONFIG).unwrap();
        let plan = merge::p2(&mut state, &checked).unwrap();
        let snapshot = |s: &EpochState<2>| {
            (
                s.ledger.words().to_vec(),
                s.store.domains.clone(),
                s.nodes.clone(),
                s.live.clone(),
                s.edges.log().to_vec(),
                s.anchors.len(),
                s.k,
                s.in_flight.clone(),
                s.counters.merges,
            )
        };
        let before = snapshot(&state);
        merge::FAIL_PREFLIGHT_STEP.with(|f| f.set(Some(step)));
        let failed = merge::p3_preflight(&mut state, &checked, &plan, &mut Rows(Vec::new()));
        merge::FAIL_PREFLIGHT_STEP.with(|f| f.set(None));
        assert!(failed.is_err(), "step {step}");
        assert!(snapshot(&state) == before, "step {step} changed the state");
        // The cut is discarded without counter changes and requeued.
        merge::discard_cut(&mut state, &checked, &mut |id, a| dispatch.requeue(id, a)).unwrap();
        match entry(&state, 3) {
            Entry6::Reserved(c) => assert_eq!((c.attempts, c.guard), (0, 0)),
            other => panic!("{other:?}"),
        }
        assert_eq!(dispatch.queued(), (1, 0));
    }
}

#[test]
fn termination_requires_monitor_available() {
    let mut state = state_with(&[boxed([0, 0], [1, 1])]);
    let mut dispatch = Dispatch::new();
    let job = jobs(&mut state, &mut dispatch, 64).remove(0);
    merge_cut(
        &mut state,
        &mut dispatch,
        &mut Rows(Vec::new()),
        vec![result(&job, &[])],
    )
    .unwrap();
    assert!(matches!(dispatch.refill(&mut state, 64), Refill::Drained));
    state
        .tracker
        .refresh(&std::sync::atomic::AtomicBool::new(false), true);
    assert_eq!(certification(&state, true, false, 0), (true, true));
    assert_eq!(
        certification(&state, false, false, 0).0,
        false,
        "not drained"
    );
    assert_eq!(
        certification(&state, true, true, 0).0,
        false,
        "admission error"
    );
    assert_eq!(
        certification(&state, true, false, 1).0,
        false,
        "input frontier"
    );
    state
        .tracker
        .disable("test: the monitor released its graph");
    assert_eq!(certification(&state, true, false, 0), (false, false));
}

#[test]
fn epoch_record_resolver_counters_preserve_c2_breaking_event_difference() {
    let mut state = state_with(&[boxed([0, 0], [1, 1])]);
    let mut dispatch = Dispatch::new();
    let job = jobs(&mut state, &mut dispatch, 1).remove(0);
    let mut native = result(&job, &[boxed([0, 0], [0, 0])]);
    native.emitted = 2;
    native.stats_events = 2;
    native.stats_json = br#"{"events":2,"successors":2}"#.to_vec();
    native.error_kind = ErrorKind::ConsumerStop;
    native.break_reason = BreakReason::ResolverRange;
    native.error = Some("second successor refused by resolver".into());
    let expected = records::ResolverCounters::from_result(&native);
    let mut rows = Rows(Vec::new());
    merge_cut(&mut state, &mut dispatch, &mut rows, vec![native]).unwrap();
    assert!(matches!(
        state.ledger.get(0).unwrap(),
        Entry6::NativeError { .. }
    ));
    let row = &rows.0[0];
    assert_eq!(row["epoch"]["class"], "C2");
    assert_eq!(row["stats"]["successors"], 2);
    assert_eq!(state.counters.successors, 1);
    let saved: records::ResolverCounters =
        serde_json::from_value(row["epoch"]["resolver_counters"].clone()).unwrap();
    assert_eq!(saved, expected);
    let mut aggregate = records::ResolverCounters::default();
    aggregate.add(&saved).unwrap();
    assert!(aggregate.agrees_with(&state.counters));
    let mut corrupted = row["epoch"]["resolver_counters"].clone();
    corrupted["successors"] = 2.into();
    let corrupted: records::ResolverCounters = serde_json::from_value(corrupted).unwrap();
    assert!(
        !corrupted.agrees_with(&state.counters),
        "native charged count cannot replace accepted count"
    );
}

#[test]
fn epoch_record_counter_schema_and_overflow_refuse_without_partial_sum() {
    let sample = records::ResolverCounters {
        version: 1,
        successors: 7,
        conditional: 3,
        optional: [11, 5, 6],
        route_masks: 13,
        route_joint_pruned: 2,
    };
    let mut total = records::ResolverCounters::default();
    total.add(&sample).unwrap();
    assert_eq!(total, sample);
    let mut bad = sample;
    bad.route_joint_pruned = u64::MAX;
    assert!(total.add(&bad).is_err());
    assert_eq!(total, sample);
    bad.version = 2;
    assert!(total.add(&bad).is_err());
    assert_eq!(total, sample);
    let mut value = serde_json::to_value(sample).unwrap();
    value["unknown"] = true.into();
    assert!(serde_json::from_value::<records::ResolverCounters>(value).is_err());
    let mut value = serde_json::to_value(sample).unwrap();
    value.as_object_mut().unwrap().remove("conditional");
    assert!(serde_json::from_value::<records::ResolverCounters>(value).is_err());
}

#[test]
fn initial_admission_errors_map_to_their_stop_reasons() {
    let mut state = EpochState::<2>::new(1, usize::MAX, usize::MAX);
    admit_initial(&mut state, &boxed([0, 0], [1, 1])).unwrap();
    // A duplicate query resolves to the stored ID; a new one hits the cap.
    assert_eq!(
        admit_initial(&mut state, &boxed([0, 0], [1, 1])).unwrap(),
        0
    );
    let cap = admit_initial(&mut state, &boxed([5, 5], [6, 6])).unwrap_err();
    assert!(matches!(cap, AdmissionError::DomainCap));
    assert_eq!(cap.stop().1, Ok(StopReason::DomainAllowance));
    // A coordinate the compact image cannot hold: a deterministic input
    // refusal (error_stop), not a domain allowance.
    let mut state = EpochState::<2>::new(10, usize::MAX, usize::MAX);
    let refused = admit_initial(&mut state, &boxed([70_000, 0], [70_001, 1])).unwrap_err();
    assert!(matches!(refused, AdmissionError::Refused(_)));
    assert_eq!(refused.stop().1, Ok(StopReason::ErrorStop));
    // Initial retirement holds verify tokens (the new ID contains the old).
    let mut state = EpochState::<2>::new(10, usize::MAX, usize::MAX);
    admit_initial(&mut state, &boxed([1, 1], [2, 2])).unwrap();
    let calls = state.verify.calls;
    admit_initial(&mut state, &boxed([0, 0], [5, 5])).unwrap();
    assert!(!state.is_live(0) && state.is_live(1));
    assert!(state.verify.calls > calls && state.verify.accepted >= 1);
}

#[test]
fn inspector_pool_survives_job_panics() {
    // A job function that panics for one job: every job still yields exactly
    // one result (empty for the panic; P1 refuses it as C5), so the batch
    // never waits for a result that cannot come.
    let job = |bytes: &[u8]| -> Vec<u8> {
        if bytes == [1] {
            panic!("injected job panic");
        }
        bytes.to_vec()
    };
    for threads in [0, 3] {
        let results =
            super::inspector::with_pool(threads, &job, |run| run(vec![vec![0], vec![1], vec![2]]))
                .unwrap()
                .unwrap();
        let mut sorted = results.clone();
        sorted.sort();
        assert_eq!(sorted, vec![vec![], vec![0], vec![2]], "threads {threads}");
    }
}

#[test]
fn records_resolve_requires_every_anchor_transitively() {
    // IDs 0..5: 0 Native (epoch 1), 1 NativeFrontier (epoch 1), 2 Native
    // (epoch 2) anchored on 1, 3 Native (epoch 3) anchored on 0 and 2,
    // 4 Native (epoch 3) anchored on 0 only.
    let mut state = state_with(&[
        boxed([0, 0], [0, 0]),
        boxed([1, 1], [1, 1]),
        boxed([2, 2], [2, 2]),
        boxed([3, 3], [3, 3]),
        boxed([4, 4], [4, 4]),
    ]);
    for (id, epoch) in [(0u32, 1u64), (2, 2), (3, 3), (4, 3)] {
        state.ledger.apply(id, Transition::T2Reserve).unwrap();
        state
            .ledger
            .apply(
                id,
                Transition::T4Native {
                    epoch,
                    residual: id >= 2,
                    dband: false,
                },
            )
            .unwrap();
    }
    state.ledger.apply(1, Transition::T2Reserve).unwrap();
    state
        .ledger
        .apply(1, Transition::T5Frontier { epoch: 1 })
        .unwrap();
    state.frontier_counts.insert(1, 1);
    let record = |node: u32, anchors: &[u32]| AnchorRecord {
        node,
        kind: AnchorKind::G2Residual,
        dispatch_version: 0,
        scope: AnchorScope::Residual(vec![]),
        anchors: anchors
            .iter()
            .map(|&anchor| AnchorRef {
                anchor,
                stamp: Some(1),
                lent: Lent::Full,
            })
            .collect(),
    };
    // Pushed in an order unlike merge order: resolution follows epochs.
    state.anchors.push(record(3, &[0, 2])).unwrap();
    state.anchors.push(record(4, &[0])).unwrap();
    state.anchors.push(record(2, &[1])).unwrap();
    let (resolutions, summary) = records::resolve(&state);
    use super::super::delegation::ResolutionStatus as S;
    assert_eq!(resolutions[4].status, S::Discharged);
    assert!(matches!(
        resolutions[2].status,
        S::UnresolvedFrontiers { .. }
    ));
    assert!(
        matches!(resolutions[3].status, S::UnresolvedFrontiers { .. }),
        "a later anchor blocks, transitively"
    );
    assert_eq!(summary["g2_residual_records"], 3);
    assert_eq!(summary["g2_residual_blocked"], 2);
    assert_eq!(summary["all_ledger_obligations_discharged"], false);
}

#[test]
fn s2_closure_forced_refresh_only() {
    // S2 closure (§10.2): the legacy Tracker is refreshed only at the
    // deterministic drain/final point, never during merges.
    let mut state = state_with(&[boxed([0, 0], [1, 1]), boxed([4, 4], [5, 5])]);
    let mut dispatch = Dispatch::new();
    for _ in 0..3 {
        let Refill::Jobs(batch) = dispatch.refill(&mut state, 1) else {
            break;
        };
        let results = batch
            .iter()
            .map(|job| {
                result(
                    job,
                    &[boxed([7, job.parent as u64], [7, job.parent as u64])],
                )
            })
            .collect();
        merge_cut(&mut state, &mut dispatch, &mut Rows(Vec::new()), results).unwrap();
        assert_eq!(
            state.tracker.counters().refresh_count,
            0,
            "no refresh in a merge"
        );
    }
    state
        .tracker
        .refresh(&std::sync::atomic::AtomicBool::new(false), true);
    assert_eq!(state.tracker.counters().refresh_count, 1);
}
