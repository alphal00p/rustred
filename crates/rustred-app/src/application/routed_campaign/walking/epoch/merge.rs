//! Controlled merge of one cut (W2.0 protocol §6): P1 checks and classifies
//! every result, P2 plans (canonical in-merge resolution of every miss,
//! the cut's antichain, reverse retirement sets with transfer tokens), P3
//! preflights every reservation and then applies the plan without a
//! containment decision. S5 prepares P2 against immutable state on a bounded
//! private pool, preserves canonical folding, and publishes typed records and
//! bulk dependency edges. `p2_plan` remains the scalar differential reference.
use super::super::queue::{CompactDomain, CompactSummary, Domain, Query};
use super::anchors::{
    AnchorKind, AnchorRecord, AnchorRef, AnchorScope, AnchorView, Lent, union_cover,
};
use super::job::{BreakReason, ErrorKind, JobResult, NativeKind, Writer, write_image};
use super::ledger6::{EPOCH_LIMIT, Entry6, MAX_ATTEMPTS, MAX_GUARD, Transition, err_class};
use super::records::typed::Record;
use super::state::{EpochState, NODE_ANCHORED, NODE_INSPECTED, NODE_RESIDUAL, NODE_SEALED};
use super::store::{LookupCounters, bucket_key};
use super::verify::{
    Container, QueryImage, UnionCover, Verified, VerifyCounters, verify, verify_cover,
};
use std::collections::HashMap;

mod finite_replay;
pub(super) mod preparation;

/// Result classes (§9.1). `C4` is a C0 result with frontiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Class {
    C0,
    C1,
    C2,
    C3,
    C4,
}

impl Class {
    pub fn name(self) -> &'static str {
        match self {
            Class::C0 => "C0",
            Class::C1 => "C1",
            Class::C2 => "C2",
            Class::C3 => "C3",
            Class::C4 => "C4",
        }
    }
    /// The result merges (record, edges, T4/T5/T6).
    pub fn merges(self) -> bool {
        matches!(self, Class::C0 | Class::C2 | Class::C4)
    }
    /// `last_err` code in ledger6 counters.
    fn code(self) -> u8 {
        self as u8 + 1
    }
}

/// Why a C1/C3 result was discarded (§8.4 cause table).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Cause {
    /// Cooperative, operator or policy stop: never a penalty.
    Stop,
    /// Resolver `try_reserve` failure: attempts + 1.
    Alloc,
    /// Spill I/O (no spill in S2): no penalty.
    SpillIo,
    /// First caught panic: attempts + 1.
    Panic,
    /// Unknown class: attempts + 1.
    Unknown,
}

impl Cause {
    fn d_attempts(self) -> u8 {
        match self {
            Cause::Stop | Cause::SpillIo => 0,
            Cause::Alloc | Cause::Panic | Cause::Unknown => 1,
        }
    }
}

/// Engine-fatal (C5): no new generation, poison, exit 70.
#[derive(Debug)]
pub(super) struct Fatal(pub String);

/// Resumable stops decided by a merge (§9.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StopReason {
    FrontierStop,
    ErrorStop,
    ExhaustedStop,
    DomainAllowance,
    EventAllowance,
    FrontierAllowance,
    Capacity,
    RamGuard,
    Paused,
    DrainedUncertified,
}

impl StopReason {
    pub fn name(self) -> &'static str {
        match self {
            StopReason::FrontierStop => "frontier_stop",
            StopReason::ErrorStop => "error_stop",
            StopReason::ExhaustedStop => "exhausted_stop",
            StopReason::DomainAllowance => "domain_allowance",
            StopReason::EventAllowance => "event_allowance",
            StopReason::FrontierAllowance => "frontier_allowance",
            StopReason::Capacity => "capacity",
            StopReason::RamGuard => "ram_guard",
            StopReason::Paused => "paused",
            StopReason::DrainedUncertified => "drained_uncertified",
        }
    }
}

/// The anchors of one result, validated in P1 (§6.2, §7).
pub(super) struct CheckedAnchors {
    /// The validated record; P3 pushes it into the anchor map.
    pub record: AnchorRecord,
    /// One `Verified` token per anchor, in record order: InitialDBand, the
    /// anchor contains the node's D >= cut slice (`verify`); G2', the exact
    /// cover of the node by its residual and the lent scopes
    /// (`verify_cover`). P3 turns them into the anchor edges.
    pub tokens: Vec<Verified>,
    /// The query digest every token names (checked in P3).
    pub q_digest: u64,
}

pub(super) struct CheckedResult<const N: usize> {
    pub result: JobResult<N>,
    pub class: Class,
    pub cause: Option<Cause>,
    /// A recurring C3 merged as C2: no successors, no edges.
    pub recurring_panic: bool,
    pub anchors: Option<CheckedAnchors>,
}

impl<const N: usize> CheckedResult<N> {
    /// `(anchor, cut)` of an InitialDBand result.
    pub fn d_band(&self) -> Option<(u32, i64)> {
        self.anchors.as_ref().and_then(|a| a.record.d_band())
    }
    /// The anchor record kind, if the result is anchored.
    pub fn anchor_kind(&self) -> Option<AnchorKind> {
        self.anchors.as_ref().map(|a| a.record.kind)
    }
}

pub(super) struct Checked<const N: usize> {
    pub entries: Vec<CheckedResult<N>>,
    /// A merge-level stop decided in P1 (allowances): every result of the
    /// cut is then discarded without counter changes.
    pub stop: Option<StopReason>,
    /// Private invocation context only; never part of a result/checkpoint.
    native_session: Option<super::snapshot::NativeSession>,
}

impl<const N: usize> Checked<N> {
    /// Existing P1 checks have already bound this result's version and
    /// watermark to its current reservation. A native-session mismatch or
    /// absent lookup report simply retains the canonical full scan.
    fn negative_prefix(&self, result: &JobResult<N>) -> Option<usize> {
        self.native_session
            .filter(|session| session.matches(result.seq))?;
        result
            .lookup
            .as_ref()
            .map(|work| work.published_len as usize)
    }
}

/// Merge-level settings.
#[derive(Clone, Copy)]
pub(super) struct MergeConfig {
    pub frontier_stop: bool,
    /// The merge applies a lockstep cut: every result was dispatched at the
    /// current k (depth 1).
    pub lockstep: bool,
    /// A bound G2' flag (W4). Never set in S2: a G2' result is then C5.
    /// Tests set it to exercise the G2' P1/P3 path.
    pub g2: bool,
    pub finite_replay: Option<super::super::OwnerDomainWalkFiniteReplayLimits>,
}

fn fatal(message: impl Into<String>) -> Fatal {
    Fatal(message.into())
}

/// The D >= cut slice of a domain (the part an InitialDBand anchor covers).
fn high_slice<const N: usize>(domain: &Domain<N>, cut: i64) -> Domain<N> {
    let mut high = domain.clone();
    high.powers.min_power_difference = Some(
        domain
            .powers
            .min_power_difference
            .map_or(cut, |v| v.max(cut)),
    );
    high
}

/// The D < cut residual power bounds the inspector must have inspected.
fn residual_powers(
    mut powers: rustred::solver::DomainPowerBounds,
    cut: i64,
) -> Option<rustred::solver::DomainPowerBounds> {
    let below = cut.checked_sub(1)?;
    powers.max_power_difference = Some(powers.max_power_difference.map_or(below, |v| v.min(below)));
    Some(powers)
}

/// P1 anchor checks of one result (§6.2, §7): the kind and its anchor part
/// agree; the record passes R1-R3, the lent-scope rule and the exact cover
/// (`AnchorRecord::validate`); InitialDBand also re-checks the shipped
/// residual (the D < cut slice) and verifies the anchor contains the D >=
/// cut slice; G2' requires the bound flag and `MergedView` visibility at v0.
/// The tokens carry the anchor edges to P3. Any failure is C5.
pub(super) fn p1_anchors<const N: usize>(
    state: &mut EpochState<N>,
    result: &JobResult<N>,
    config: MergeConfig,
) -> Result<Option<CheckedAnchors>, Fatal> {
    let parent = result.parent;
    let partial = result.kind == NativeKind::ApplyPartial;
    let g2 = result.kind == NativeKind::G2Residual;
    if partial != result.scope.is_some() || g2 != result.g2.is_some() {
        return Err(fatal(format!(
            "P1: {parent}: native kind and anchor parts disagree"
        )));
    }
    if g2 && !config.g2 {
        return Err(fatal(format!(
            "P1: {parent}: a G2' result without a bound G2' flag"
        )));
    }
    let record = if let Some(scope) = result.scope {
        AnchorRecord {
            node: parent,
            kind: AnchorKind::InitialDBand,
            dispatch_version: result.v0,
            scope: AnchorScope::DBandCut(scope.cut),
            anchors: vec![AnchorRef {
                anchor: scope.anchor,
                stamp: None,
                lent: Lent::Full,
            }],
        }
    } else if let Some(part) = &result.g2 {
        let kind = AnchorKind::from_code(part.kind)
            .filter(|kind| kind.is_g2())
            .ok_or_else(|| fatal(format!("P1: {parent}: G2' record kind {}", part.kind)))?;
        let anchors = part
            .anchors
            .iter()
            .map(|&(anchor, stamp, lent)| {
                Ok(AnchorRef {
                    anchor,
                    stamp: Some(stamp),
                    lent: Lent::from_code(lent)
                        .ok_or_else(|| fatal(format!("P1: {parent}: lent scope code {lent}")))?,
                })
            })
            .collect::<Result<Vec<_>, Fatal>>()?;
        AnchorRecord {
            node: parent,
            kind,
            dispatch_version: result.v0,
            scope: AnchorScope::Residual(part.pieces.clone()),
            anchors,
        }
    } else {
        return Ok(None);
    };
    let domains = &state.store.domains;
    if record
        .anchors
        .iter()
        .any(|anchor| state.store.is_quarantined(anchor.anchor))
    {
        return Err(fatal("P1: quarantined anchor"));
    }
    let published_len = state.store.len();
    let node = domains[parent as usize];
    let anchor_map = &state.anchors;
    let merged_view = &state.merged_view;
    let same_bucket = |a: u32, b: u32| {
        (a as usize) < domains.len()
            && (b as usize) < domains.len()
            && bucket_key(&domains[a as usize]) == bucket_key(&domains[b as usize])
    };
    let record_of = |id: u32| {
        anchor_map
            .get(id)
            .map(|r| (r.kind, r.d_band().map(|(_, cut)| cut)))
    };
    let cut_of = |id: u32| {
        anchor_map
            .get(id)
            .and_then(AnchorRecord::d_band)
            .map(|(_, cut)| cut)
    };
    let visible = |id: u32, v0: u64| {
        merged_view.contains(id, v0)
            && !state.store.is_quarantined(id)
            && super::g2::eligible(domains, &state.ledger, anchor_map, id)
    };
    let union = {
        let evaluated = std::cell::RefCell::new(None);
        let cover = |r: &AnchorRecord| {
            if !r.kind.is_g2() {
                return union_cover(&node, domains, r, &cut_of);
            }
            // The view is local to this record. Bind the one-use evaluation to
            // its captured borrow rather than extending the callback argument's
            // lifetime. Structural checks still precede this exact oracle call.
            if !std::ptr::eq(r, &record) {
                return None;
            }
            let union = UnionCover::evaluate(&record, &node, domains, published_len, &cut_of);
            let verdict = union.verdict();
            evaluated.replace(Some(union));
            verdict
        };
        let view = AnchorView {
            p0: state.p0,
            published_len,
            arity: N,
            ledger: &state.ledger,
            same_bucket: &same_bucket,
            record_of: &record_of,
            edges_of: None,
            merged_view: if config.g2 { Some(&visible) } else { None },
            cover: &cover,
        };
        record
            .validate(&view, state.k + 1)
            .map_err(|v| fatal(format!("P1: {parent} anchors: {v:?}")))?;
        evaluated.into_inner()
    };
    let (tokens, q_digest) = match (record.d_band(), result.scope) {
        (Some((anchor, cut)), Some(scope)) => {
            let domain = node.expand();
            if residual_powers(domain.powers, cut) != Some(scope.residual) {
                return Err(fatal(format!(
                    "P1: {parent} residual is not the D < cut slice"
                )));
            }
            let high = CompactDomain::try_from_domain(&high_slice(&domain, cut))
                .map_err(|e| fatal(format!("P1: {parent} high slice: {e}")))?;
            let q = QueryImage::new(high).map_err(|e| fatal(format!("P1: {e}")))?;
            let token = verify(
                Container::Stored {
                    id: anchor,
                    domains,
                    published_len,
                },
                &q,
                &mut state.verify,
            )
            .ok_or_else(|| {
                fatal(format!(
                    "P1: {parent} anchor {anchor} does not contain the D >= {cut} slice"
                ))
            })?;
            (vec![token], q.digest)
        }
        _ => {
            let q = QueryImage::new(node).map_err(|e| fatal(format!("P1: {e}")))?;
            let tokens = union
                .and_then(|union| verify_cover(&record, &q, union, &mut state.verify))
                .ok_or_else(|| fatal(format!("P1: {parent}: G2' anchors do not cover the node")))?;
            (tokens, q.digest)
        }
    };
    Ok(Some(CheckedAnchors {
        record,
        tokens,
        q_digest,
    }))
}

/// P1: per result, the in-flight and ledger checks, the event parity and
/// the class; anchors re-validated (`p1_anchors`); aggregate allowances.
/// Any protocol violation is C5.
pub(super) fn p1_check<const N: usize>(
    state: &mut EpochState<N>,
    cut: Vec<Vec<u8>>,
    config: MergeConfig,
) -> Result<Checked<N>, Fatal> {
    p1_check_bound(state, cut, config, None)
}

/// Native controllers supply an invocation-local binding. Generic/synthetic
/// callers keep p1_check; merely shipping snapshot metadata is not opt-in.
pub(super) fn p1_check_bound<const N: usize>(
    state: &mut EpochState<N>,
    cut: Vec<Vec<u8>>,
    config: MergeConfig,
    native_session: Option<super::snapshot::NativeSession>,
) -> Result<Checked<N>, Fatal> {
    state.store.ensure_unique().map_err(fatal)?;
    let mut entries = Vec::with_capacity(cut.len());
    let mut seen = std::collections::BTreeSet::new();
    for bytes in cut {
        let result = JobResult::<N>::decode(&bytes).map_err(|e| fatal(format!("P1: {e}")))?;
        let parent = result.parent;
        if !seen.insert(parent) {
            return Err(fatal(format!("P1: two results for {parent} in one cut")));
        }
        let meta = state
            .in_flight
            .get(&parent)
            .ok_or_else(|| fatal(format!("P1: result for {parent} not in flight")))?;
        if meta.seq != result.seq || meta.v0 != result.v0 {
            return Err(fatal(format!("P1: stale result for {parent}")));
        }
        let entry = state
            .ledger
            .get(parent)
            .map_err(|e| fatal(format!("P1: {e}")))?;
        let Entry6::Reserved(counters) = entry else {
            return Err(fatal(format!("P1: {parent} is not Reserved")));
        };
        finite_replay::validate(state, &result, config, counters.attempts)?;
        if result.v0 > state.k || (config.lockstep && result.v0 != state.k) {
            return Err(fatal(format!(
                "P1: result v0 {} beyond k {}",
                result.v0, state.k
            )));
        }
        if let Some(work) = &result.lookup {
            if work.version != result.v0
                || work.published_len != meta.published_len
                || work.published_len as usize > state.store.len()
                || parent >= work.published_len
            {
                return Err(fatal(format!("P1: {parent}: lookup snapshot mismatch")));
            }
        } else if result.misses.iter().any(|miss| miss.target.is_some()) {
            return Err(fatal(format!(
                "P1: {parent}: stored target without snapshot"
            )));
        }
        let (class, cause) = classify(&result, counters.last_err)?;
        let abandoned = result.kind == NativeKind::Abandoned;
        if abandoned != (super::rescue::job_flags(state, parent) != 0)
            || abandoned
                && (class != Class::C4
                    || result.emitted != 1
                    || result.accepted != 1
                    || result.successors != 0
                    || result.conditional != 0
                    || result.known_reuse != 0
                    || result.job_duplicates != 0
                    || result.optional != [0; 3]
                    || result.route_masks != 0
                    || result.route_joint_pruned != 0
                    || !result.refusals.is_empty()
                    || result.refusals_truncated
                    || result.frontiers.len() != 1
                    || !result.misses.is_empty()
                    || result.scope.is_some()
                    || result.g2.is_some()
                    || result.error.is_some()
                    || result.panic
                    || serde_json::from_slice::<serde_json::Value>(&result.frontiers[0])
                        .ok()
                        .is_none_or(|value| {
                            value["kind"] != super::super::inspection::RESCUE_ABANDONED_KIND
                        }))
        {
            return Err(fatal("P1: unauthorized or malformed rescue abandonment"));
        }
        let recurring_panic = result.panic && class == Class::C2;
        let anchors = p1_anchors(state, &result, config)?;
        entries.push(CheckedResult {
            result,
            class,
            cause,
            recurring_panic,
            anchors,
        });
    }
    entries.sort_by_key(|entry| entry.result.parent);
    // Aggregate allowances over the merging results (§11.5: not bound).
    let merging = || entries.iter().filter(|entry| entry.class.merges());
    let events: u64 = merging().map(|entry| entry.result.emitted).sum();
    let frontiers: u64 = merging()
        .filter(|entry| entry.result.kind != NativeKind::Abandoned)
        .map(|entry| entry.result.frontiers.len() as u64)
        .sum();
    let stop = if state.counters.events.saturating_add(events) > state.max_events {
        Some(StopReason::EventAllowance)
    } else if state.counters.frontiers.saturating_add(frontiers) > state.max_frontiers {
        Some(StopReason::FrontierAllowance)
    } else {
        None
    };
    Ok(Checked {
        entries,
        stop,
        native_session,
    })
}

/// §9.1: the class of one result from its error kind, break reason, panic
/// flag and the ID's last discarded class; event parity per class.
pub(super) fn classify<const N: usize>(
    r: &JobResult<N>,
    last_err: u8,
) -> Result<(Class, Option<Cause>), Fatal> {
    let parent = r.parent;
    if r.panic {
        return Ok(if last_err == Class::C3.code() {
            (Class::C2, None)
        } else {
            (Class::C3, Some(Cause::Panic))
        });
    }
    if r.break_reason == BreakReason::Protocol {
        return Err(fatal(format!(
            "P1: {parent}: resolver received an impossible event"
        )));
    }
    // Event parity per class (§9.1, F7 as amended). The solver charges an
    // event to its stats before the visitor sees it ([src] rustred-core
    // applied/engine.rs `emit`, routed/domain_overcover/visit.rs; pinned by
    // the core test at applied/tests.rs:492 and by `consumer_stop_parity`).
    let full_parity = r.emitted == r.accepted && r.accepted == r.stats_events;
    match r.error_kind {
        ErrorKind::None => {
            if r.break_reason != BreakReason::None {
                return Err(fatal(format!("P1: {parent}: break without consumer stop")));
            }
            if !full_parity {
                return Err(fatal(format!(
                    "P1: {parent}: event parity (emitted {}, accepted {}, stats {})",
                    r.emitted, r.accepted, r.stats_events
                )));
            }
            Ok((
                if r.frontiers.is_empty() {
                    Class::C0
                } else {
                    Class::C4
                },
                None,
            ))
        }
        ErrorKind::Cancelled => Ok((Class::C1, Some(Cause::Stop))),
        ErrorKind::ConsumerStop => match r.break_reason {
            BreakReason::ResolverRange
            | BreakReason::ResolverSummary
            | BreakReason::ResolverDiagnostic
            | BreakReason::Allowance => {
                // The breaking event is emitted but not accepted, and the
                // solver counted it: stats == emitted == accepted + 1.
                if r.accepted.checked_add(1) != Some(r.emitted) || r.stats_events != r.emitted {
                    return Err(fatal(format!(
                        "P1: {parent}: break parity (emitted {}, accepted {}, stats {})",
                        r.emitted, r.accepted, r.stats_events
                    )));
                }
                Ok((Class::C2, None))
            }
            BreakReason::Alloc => Ok((Class::C1, Some(Cause::Alloc))),
            BreakReason::SpillIo => Ok((Class::C1, Some(Cause::SpillIo))),
            BreakReason::Cancel => Ok((Class::C1, Some(Cause::Stop))),
            BreakReason::None | BreakReason::Protocol => Ok(if last_err == Class::C3.code() {
                (Class::C2, None)
            } else {
                (Class::C3, Some(Cause::Unknown))
            }),
        },
        ErrorKind::NativeFailure => {
            // Over the prefix: emitted == accepted == stats (a failing charge
            // is not counted and the event never reaches the visitor).
            if r.break_reason != BreakReason::None || !full_parity {
                return Err(fatal(format!(
                    "P1: {parent}: native failure parity (emitted {}, accepted {}, stats {})",
                    r.emitted, r.accepted, r.stats_events
                )));
            }
            Ok((Class::C2, None))
        }
        ErrorKind::Conversion => {
            // The refused diagnostic was charged by the solver and then
            // refused inside the visitor before the resolver saw it
            // (`inspection.rs`, conversion branch): stats == emitted + 1.
            if r.break_reason != BreakReason::None
                || r.emitted != r.accepted
                || r.emitted.checked_add(1) != Some(r.stats_events)
            {
                return Err(fatal(format!(
                    "P1: {parent}: conversion parity (emitted {}, accepted {}, stats {})",
                    r.emitted, r.accepted, r.stats_events
                )));
            }
            Ok((Class::C2, None))
        }
        ErrorKind::Other => Ok(if last_err == Class::C3.code() {
            (Class::C2, None)
        } else {
            (Class::C3, Some(Cause::Unknown))
        }),
    }
}

/// The ledger6 NativeError class of a merged C2 result (§4.1 err class; the
/// code table is `ledger6::err_class`).
pub(super) fn error_class<const N: usize>(entry: &CheckedResult<N>) -> u8 {
    let r = &entry.result;
    if entry.recurring_panic {
        return err_class::RECURRING_PANIC;
    }
    match (r.error_kind, r.break_reason) {
        (ErrorKind::NativeFailure, _) => err_class::NATIVE_FAILURE,
        (ErrorKind::Conversion, _) => err_class::CONVERSION,
        (ErrorKind::ConsumerStop, BreakReason::ResolverRange) => err_class::RESOLVER_RANGE,
        (ErrorKind::ConsumerStop, BreakReason::ResolverSummary) => err_class::RESOLVER_SUMMARY,
        (ErrorKind::ConsumerStop, BreakReason::ResolverDiagnostic) => {
            err_class::RESOLVER_DIAGNOSTIC
        }
        (ErrorKind::ConsumerStop, BreakReason::Allowance) => err_class::ALLOWANCE,
        _ => err_class::RECURRING_UNKNOWN,
    }
}

/// One survivor of the cut (a new ID after P3), in provisional order.
pub(super) struct Survivor<const N: usize> {
    pub image: CompactDomain<N>,
    pub summary: CompactSummary<N>,
    pub query: Query<N>,
    pub digest: u64,
    /// Every live ID of S_k contained in this survivor (ascending).
    pub retire: Vec<u32>,
}

pub(super) struct MergePlan<const N: usize> {
    pub survivors: Vec<Survivor<N>>,
    /// Per checked entry (cut order): the tokens of its edge targets.
    pub targets: Vec<Vec<Verified>>,
    /// (old ID, token for survivor ⊇ old), ascending old ID; the smallest
    /// containing survivor position wins.
    pub transfers: Vec<(u32, Verified)>,
    /// P2's own counters (P2 reads the state only; `p2` adds them).
    pub counters: P2Counters,
}

/// Counters of one P2 plan, added to the state after planning.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct P2Counters {
    pub lookup: LookupCounters,
    pub verify: VerifyCounters,
    pub miss_requests: u64,
    pub antichain_folded: u64,
    pub inspector: super::state::InspectorLookup,
}

impl P2Counters {
    pub fn apply_to<const N: usize>(&self, state: &mut EpochState<N>) {
        state.lookup.add(&self.lookup);
        state.verify.add(&self.verify);
        state.counters.miss_requests += self.miss_requests;
        state.counters.antichain_folded += self.antichain_folded;
        state.inspector_lookup.queries += self.inspector.queries;
        state.inspector_lookup.stored_hits += self.inspector.stored_hits;
        state.inspector_lookup.coordinator_miss_rechecks_skipped +=
            self.inspector.coordinator_miss_rechecks_skipped;
        state.inspector_lookup.seconds += self.inspector.seconds;
    }
}

/// P2 and its counters (the serial S2 driver): `p2_plan` on a shared borrow,
/// then the plan's counters added to the state.
pub(super) fn p2<const N: usize>(
    state: &mut EpochState<N>,
    checked: &Checked<N>,
) -> Result<MergePlan<N>, Fatal> {
    let plan = p2_plan(state, checked)?;
    plan.counters.apply_to(state);
    Ok(plan)
}

/// A candidate miss of the cut (distinct image).
struct Candidate<const N: usize> {
    q: QueryImage<N>,
    query: Query<N>,
    key: (u64, Vec<u8>),
    first: (u32, u32),
    bucket: (u8, u32),
}

fn canonical_bytes<const N: usize>(image: &CompactDomain<N>) -> Vec<u8> {
    let mut w = Writer::default();
    write_image(&mut w, image);
    w.0
}

/// Native summary inclusion between two candidates (the prefilter of the
/// antichain; `verify` is the authority for every token).
fn contains<const N: usize>(outer: &Candidate<N>, inner: &Candidate<N>) -> bool {
    outer.bucket == inner.bucket
        && outer
            .query
            .compact
            .contains(&inner.query.compact)
            .unwrap_or_else(|| outer.q.core.contains(&inner.q.core))
}

/// Buckets with more candidates than this use a temporary kernel index for
/// the antichain (performance only; identical result, tested).
pub(super) const INDEXED_ANTICHAIN: usize = 64;

/// The antichain rule on one bucket, pairwise: `j` survives iff no other
/// candidate contains it strictly, or equivalently with a smaller key.
fn antichain_pairwise<const N: usize>(members: &[u32], candidates: &[Candidate<N>]) -> Vec<bool> {
    members
        .iter()
        .map(|&j| {
            let cj = &candidates[j as usize];
            !members.iter().any(|&i| {
                i != j && {
                    let ci = &candidates[i as usize];
                    contains(ci, cj) && (!contains(cj, ci) || ci.key < cj.key)
                }
            })
        })
        .collect()
}

/// A temporary kernel index over some candidates (local IDs = positions in
/// `members`), queried with an arbitrary exact predicate: the index's
/// signature, block and lane filters are necessary conditions of summary
/// inclusion, so no candidate satisfying `predicate` (which implies
/// inclusion) is skipped, and `first` returns the minimum local ID.
struct TempIndex<const N: usize> {
    index: super::super::queue::AggregateIndex<N>,
}

struct Predicate<F>(F);
impl<F: FnMut(usize) -> bool> super::super::queue::Visit for Predicate<F> {
    fn rejected(&mut self, _: &[u32], _: u32) -> Result<(), &'static str> {
        Ok(())
    }
    fn test(&mut self, id: usize) -> Result<bool, &'static str> {
        Ok((self.0)(id))
    }
}

impl<const N: usize> TempIndex<N> {
    fn build(members: &[u32], candidates: &[Candidate<N>]) -> Result<Self, &'static str> {
        Self::build_controlled(members, candidates, || Ok(()))
    }
    fn build_controlled(
        members: &[u32],
        candidates: &[Candidate<N>],
        mut checkpoint: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Self, &'static str> {
        use super::super::queue::{Coordinates, Entry, Signature};
        let mut index = super::super::queue::AggregateIndex::default();
        for (local, &slot) in members.iter().enumerate() {
            checkpoint()?;
            let query = &candidates[slot as usize].query;
            let coordinates = Coordinates::of(&query.core);
            let insertion = index.prepare(Signature::of(&query.core), coordinates)?;
            let (word, lanes) = query.image();
            index.insert(
                insertion,
                Entry {
                    id: local,
                    coordinates,
                    word,
                    lanes,
                },
            );
        }
        Ok(Self { index })
    }
    fn first(
        &self,
        query: &Candidate<N>,
        predicate: impl FnMut(usize) -> bool,
    ) -> Result<Option<usize>, &'static str> {
        self.first_controlled(query, predicate, || Ok(()))
    }
    fn first_controlled(
        &self,
        query: &Candidate<N>,
        predicate: impl FnMut(usize) -> bool,
        checkpoint: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Option<usize>, &'static str> {
        use super::super::queue::{Coordinates, Probe, Signature};
        let probe = Probe::new(
            Coordinates::of(&query.q.core),
            query.query.word,
            query.query.lanes,
            true,
        );
        self.index.find_controlled(
            Signature::of(&query.q.core),
            &probe,
            0,
            checkpoint,
            &mut Predicate(predicate),
        )
    }
}

/// `antichain_pairwise` through a temporary index (large buckets).
fn antichain_indexed<const N: usize>(
    members: &[u32],
    candidates: &[Candidate<N>],
) -> Result<Vec<bool>, &'static str> {
    let index = TempIndex::build(members, candidates)?;
    members
        .iter()
        .enumerate()
        .map(|(local_j, &j)| {
            let cj = &candidates[j as usize];
            let dominated = index.first(cj, |local_i| {
                local_i != local_j && {
                    let ci = &candidates[members[local_i] as usize];
                    contains(ci, cj) && (!contains(cj, ci) || ci.key < cj.key)
                }
            })?;
            Ok(dominated.is_none())
        })
        .collect()
}

/// Test seam: the survivor flags of one bucket both ways.
#[cfg(test)]
pub(super) fn antichain_both_ways<const N: usize>(
    images: &[CompactDomain<N>],
) -> (Vec<bool>, Vec<bool>) {
    let candidates: Vec<Candidate<N>> = images
        .iter()
        .enumerate()
        .map(|(position, image)| {
            let q = QueryImage::new(*image).expect("summary");
            Candidate {
                query: Query::new(q.core.clone(), image.phase()),
                key: (q.digest, canonical_bytes(image)),
                first: (position as u32, 0),
                bucket: bucket_key(image),
                q,
            }
        })
        .collect();
    let members: Vec<u32> = (0..candidates.len() as u32).collect();
    (
        antichain_pairwise(&members, &candidates),
        antichain_indexed(&members, &candidates).expect("indexed antichain"),
    )
}

struct CandidateBatch<const N: usize> {
    slots_of: Vec<Vec<Result<Verified, u32>>>,
    candidates: Vec<Candidate<N>>,
    counters: P2Counters,
}

/// Initial resolution has no mutation or publication authority. Both the
/// serial reference and prepared merge use this exact validation/lookup path.
fn lookup_accounting<const N: usize>(
    result: &JobResult<N>,
    published_len: usize,
    counters: &mut P2Counters,
    mut checkpoint: impl FnMut() -> Result<(), &'static str>,
) -> Result<(), Fatal> {
    if let Some(work) = &result.lookup {
        let mut stored_hits = 0u64;
        for chunk in result.misses.chunks(256) {
            checkpoint().map_err(|e| fatal(format!("P2: {e}")))?;
            stored_hits += chunk.iter().filter(|miss| miss.target.is_some()).count() as u64;
        }
        let hits = work
            .lookup
            .exact_hits
            .checked_add(work.lookup.orthant_hits)
            .and_then(|n| n.checked_add(work.lookup.contained_hits))
            .ok_or_else(|| fatal("P2: inspector lookup count overflow"))?;
        if hits.checked_add(work.lookup.misses) != Some(result.misses.len() as u64)
            || hits != stored_hits
            || work.lookup.reverse_candidates != 0
            || work.lookup.reverse_tests != 0
            || work.verify.calls != hits
            || work.verify.accepted != hits
            || work.verify.refused_range != 0
            || work.verify.refused_bucket != 0
            || work.verify.union_covers != 0
            || work
                .verify
                .raw_inclusions
                .checked_add(work.verify.recomputes)
                != Some(hits)
            || work.lookup.forward_tests > work.lookup.forward_candidates
            || u128::from(work.lookup.forward_candidates)
                > (published_len as u128) * (result.misses.len() as u128)
        {
            return Err(fatal("P2: inspector lookup accounting mismatch"));
        }
        counters.lookup.add(&work.lookup);
        counters.verify.add(&work.verify);
        counters.inspector.queries += result.misses.len() as u64;
        counters.inspector.stored_hits += hits;
        counters.inspector.seconds += work.seconds;
    }
    Ok(())
}

enum MissResolution<const N: usize> {
    Stored(Verified),
    Candidate { q: QueryImage<N>, query: Query<N> },
}

/// No authority is gained from a negative. Stored positives are independently
/// reverified; current-view misses still check exact uniqueness. Bound native
/// stale negatives can skip their previously searched aggregate prefix only;
/// exact/orthant priorities stay global. Unbound stale/AllMiss rows scan all.
/// The controlled and reference paths share these checks.
fn resolve_miss<const N: usize>(
    store: &super::store::Store<N>,
    version: u64,
    result: &JobResult<N>,
    miss: &super::job::Miss<N>,
    negative_prefix: Option<usize>,
    counters: &mut P2Counters,
    mut checkpoint: impl FnMut() -> Result<(), &'static str>,
) -> Result<MissResolution<N>, Fatal> {
    checkpoint().map_err(|e| fatal(format!("P2: {e}")))?;
    let published_len = store.len();
    if miss.image.digest().0 != miss.digest {
        return Err(fatal(format!(
            "P2: {} miss {}: shipped digest differs from its image",
            result.parent, miss.ordinal
        )));
    }
    let q = QueryImage::new(miss.image).map_err(|e| fatal(format!("P2: {e}")))?;
    if let Some(id) = miss.target {
        if store.is_quarantined(id) {
            return Err(fatal("P2: stored target is quarantined"));
        }
        let token = verify(
            Container::Stored {
                id,
                domains: &store.domains,
                published_len: result
                    .lookup
                    .as_ref()
                    .map_or(published_len, |work| work.published_len as usize),
            },
            &q,
            &mut counters.verify,
        )
        .ok_or_else(|| fatal(format!("P2: stored target {id} failed verify")))?;
        return Ok(MissResolution::Stored(token));
    }
    counters.miss_requests += 1;
    let query = Query::new(q.core.clone(), miss.image.phase());
    if result
        .lookup
        .as_ref()
        .is_some_and(|work| work.version == version && work.published_len as usize == published_len)
    {
        if store
            .lookup_exact(
                &q,
                published_len,
                &mut counters.lookup,
                &mut counters.verify,
            )
            .map_err(|e| fatal(format!("P2: {e}")))?
            .is_some()
        {
            return Err(fatal("P2: inspector miss contradicts exact image"));
        }
        counters.inspector.coordinator_miss_rechecks_skipped += 1;
    } else if let Some((_, token, _)) = store
        .lookup_suffix_controlled(
            &q,
            &query,
            published_len,
            negative_prefix.unwrap_or(0),
            &mut counters.lookup,
            &mut counters.verify,
            checkpoint,
        )
        .map_err(|e| fatal(format!("P2: {e}")))?
    {
        return Ok(MissResolution::Stored(token));
    }
    Ok(MissResolution::Candidate { q, query })
}

fn resolve_misses<const N: usize>(
    state: &EpochState<N>,
    checked: &Checked<N>,
) -> Result<CandidateBatch<N>, Fatal> {
    let mut counters = P2Counters::default();
    let mut slots_of = Vec::with_capacity(checked.entries.len());
    let mut candidates: Vec<Candidate<N>> = Vec::new();
    let mut by_digest: HashMap<u64, Vec<u32>> = HashMap::new();
    for (position, entry) in checked.entries.iter().enumerate() {
        let mut entry_slots = Vec::new();
        if entry.class.merges() && !entry.recurring_panic {
            lookup_accounting(&entry.result, state.store.len(), &mut counters, || Ok(()))?;
            let negative_prefix = checked.negative_prefix(&entry.result);
            for miss in &entry.result.misses {
                let (q, query) = match resolve_miss(
                    &state.store,
                    state.k,
                    &entry.result,
                    miss,
                    negative_prefix,
                    &mut counters,
                    || Ok(()),
                )? {
                    MissResolution::Stored(token) => {
                        entry_slots.push(Ok(token));
                        continue;
                    }
                    MissResolution::Candidate { q, query } => (q, query),
                };
                let slots = by_digest.entry(miss.digest).or_default();
                let known = slots
                    .iter()
                    .copied()
                    .find(|&slot| candidates[slot as usize].q.image == miss.image);
                let slot = match known {
                    Some(slot) => {
                        let candidate = &mut candidates[slot as usize];
                        candidate.first = candidate.first.min((position as u32, miss.ordinal));
                        slot
                    }
                    None => {
                        let slot = candidates.len() as u32;
                        slots.push(slot);
                        candidates.push(Candidate {
                            key: (miss.digest, canonical_bytes(&miss.image)),
                            bucket: bucket_key(&miss.image),
                            first: (position as u32, miss.ordinal),
                            q,
                            query,
                        });
                        slot
                    }
                };
                entry_slots.push(Err(slot));
            }
        }
        slots_of.push(entry_slots);
    }
    Ok(CandidateBatch {
        slots_of,
        candidates,
        counters,
    })
}

/// Serial P2 reference: canonical resolution, the per-bucket antichain,
/// first-occurrence survivor order and minimum-position reverse transfers.
/// Reads the state only; counters travel in the returned plan.
pub(super) fn p2_plan<const N: usize>(
    state: &EpochState<N>,
    checked: &Checked<N>,
) -> Result<MergePlan<N>, Fatal> {
    let CandidateBatch {
        slots_of,
        candidates,
        mut counters,
    } = resolve_misses(state, checked)?;
    // Antichain per bucket (§6.3 step 4): a candidate survives iff no other
    // candidate contains it strictly or equivalently with a smaller key.
    let mut buckets: HashMap<(u8, u32), Vec<u32>> = HashMap::new();
    for (slot, candidate) in candidates.iter().enumerate() {
        buckets
            .entry(candidate.bucket)
            .or_default()
            .push(slot as u32);
    }
    let mut survivor = vec![false; candidates.len()];
    for members in buckets.values() {
        let flags = if members.len() > INDEXED_ANTICHAIN {
            antichain_indexed(members, &candidates).map_err(|e| fatal(format!("P2: {e}")))?
        } else {
            antichain_pairwise(members, &candidates)
        };
        for (&j, flag) in members.iter().zip(flags) {
            survivor[j as usize] = flag;
        }
    }
    // Provisional order: (first parent position, ordinal).
    let mut order: Vec<u32> = (0..candidates.len() as u32)
        .filter(|&slot| survivor[slot as usize])
        .collect();
    order.sort_by_key(|&slot| candidates[slot as usize].first);
    let mut position_of = vec![u32::MAX; candidates.len()];
    for (pos, &slot) in order.iter().enumerate() {
        position_of[slot as usize] = pos as u32;
    }
    let n_s = order.len();
    // Resolution of every candidate: itself (a survivor) or the containing
    // survivor with the smallest position; always a `Planned` token.
    let mut resolved: Vec<Option<Verified>> = vec![None; candidates.len()];
    for members in buckets.values() {
        let mut bucket_survivors: Vec<u32> = members
            .iter()
            .copied()
            .filter(|&slot| survivor[slot as usize])
            .collect();
        bucket_survivors.sort_by_key(|&slot| position_of[slot as usize]);
        let index = (bucket_survivors.len() > INDEXED_ANTICHAIN)
            .then(|| TempIndex::build(&bucket_survivors, &candidates))
            .transpose()
            .map_err(|e| fatal(format!("P2: {e}")))?;
        for &j in members {
            let cj = &candidates[j as usize];
            let s = if survivor[j as usize] {
                j
            } else {
                let first = match &index {
                    Some(index) => index
                        .first(cj, |local| {
                            contains(&candidates[bucket_survivors[local] as usize], cj)
                        })
                        .map_err(|e| fatal(format!("P2: {e}")))?
                        .map(|local| bucket_survivors[local]),
                    None => bucket_survivors
                        .iter()
                        .copied()
                        .find(|&s| contains(&candidates[s as usize], cj)),
                };
                first.ok_or_else(|| fatal("P2: a non-survivor has no containing survivor"))?
            };
            let token = verify(
                Container::Planned {
                    pos: position_of[s as usize],
                    survivors: n_s,
                    image: &candidates[s as usize].q.image,
                },
                &cj.q,
                &mut counters.verify,
            )
            .ok_or_else(|| fatal("P2: antichain token failed verify"))?;
            resolved[j as usize] = Some(token);
        }
    }
    let targets: Vec<Vec<Verified>> = slots_of
        .into_iter()
        .map(|slots| {
            slots
                .into_iter()
                .map(|slot| {
                    slot.unwrap_or_else(|c| resolved[c as usize].expect("resolved candidate"))
                })
                .collect()
        })
        .collect();
    counters.antichain_folded += (candidates.len() - n_s) as u64;
    // Reverse sets (A4) and transfer tokens (F4 re-verify in P2).
    let mut survivors = Vec::with_capacity(n_s);
    let mut assigned: HashMap<u32, Verified> = HashMap::new();
    for (pos, &slot) in order.iter().enumerate() {
        let candidate = &candidates[slot as usize];
        let retire = state
            .store
            .contained_live(&candidate.q, &candidate.query, &mut counters.lookup)
            .map_err(|e| fatal(format!("P2: {e}")))?;
        for &old in &retire {
            if assigned.contains_key(&old) {
                continue;
            }
            let old_q = QueryImage::new(state.store.domains[old as usize])
                .map_err(|e| fatal(format!("P2: {e}")))?;
            let token = verify(
                Container::Planned {
                    pos: pos as u32,
                    survivors: n_s,
                    image: &candidate.q.image,
                },
                &old_q,
                &mut counters.verify,
            )
            .ok_or_else(|| fatal(format!("P2: reverse candidate {old} failed verify")))?;
            assigned.insert(old, token);
        }
        survivors.push(Survivor {
            image: candidate.q.image,
            summary: candidate.query.compact,
            query: candidate.query.clone(),
            digest: candidate.key.0,
            retire,
        });
    }
    let mut transfers: Vec<(u32, Verified)> = assigned.into_iter().collect();
    transfers.sort_by_key(|&(old, _)| old);
    Ok(MergePlan {
        survivors,
        targets,
        transfers,
        counters,
    })
}

/// What P3 decided beyond the state change.
#[derive(Default)]
pub(super) struct Applied {
    pub stop: Option<StopReason>,
    /// Discarded (requeued) IDs, ascending, with whether they became
    /// Exhausted.
    pub requeued: Vec<(u32, bool)>,
    pub merged: u64,
    pub new_ids: u64,
}

/// Records produced by P3 (merge order).
pub(super) trait RecordOut {
    fn reserve(&mut self) -> Result<(), String>;
    fn push(&mut self, record: Record) -> Result<(), String>;
}

/// The record builder P3 calls (records.rs), kept behind a trait object so
/// that merge logic is testable without owners.
pub(super) trait RecordBuilder<const N: usize> {
    fn native(
        &self,
        id: u32,
        image: &CompactDomain<N>,
        entry: &CheckedResult<N>,
        merge_epoch: u64,
        distinct_edges: u32,
        self_edge: bool,
    ) -> Result<Record, String>;
    fn alias(
        &self,
        id: u32,
        image: &CompactDomain<N>,
        to: u32,
        merge_epoch: u64,
        exhausted: bool,
    ) -> Record;
}

/// The preflight's reservation steps, in order (the test seam fails one).
#[cfg(test)]
pub(super) const PREFLIGHT_STEPS: usize = 7;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum PreflightError {
    Stop(StopReason),
    Engine(&'static str),
}

#[cfg(test)]
thread_local! {
    /// Test seam: the preflight step that fails (`p3_preflight_failure_is_noop`).
    pub(super) static FAIL_PREFLIGHT_STEP: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(None) };
}

fn injected(step: usize) -> bool {
    #[cfg(test)]
    {
        FAIL_PREFLIGHT_STEP.with(|f| f.get() == Some(step))
    }
    #[cfg(not(test))]
    {
        let _ = step;
        false
    }
}

/// P3 preflight (§6.4): the ID cap (F9), the u48 epoch (F13) and capacity
/// in every arena P3 appends to (per-ID arrays, exact shards, edge log,
/// anchor map, record sink). A failure changes nothing logical (capacity
/// only); the caller discards the cut without counter changes and stops.
pub(super) fn p3_preflight<const N: usize>(
    state: &mut EpochState<N>,
    checked: &Checked<N>,
    plan: &MergePlan<N>,
    records: &mut dyn RecordOut,
) -> Result<(), PreflightError> {
    state
        .store
        .ensure_unique()
        .map_err(PreflightError::Engine)?;
    let n_s = plan.survivors.len();
    if injected(0) || state.store.len() + n_s > state.id_cap() {
        return Err(PreflightError::Stop(StopReason::DomainAllowance));
    }
    if injected(1) || state.k + 1 >= EPOCH_LIMIT {
        return Err(PreflightError::Stop(StopReason::Capacity));
    }
    let anchor_edges: usize = checked
        .entries
        .iter()
        .filter_map(|entry| entry.anchors.as_ref())
        .map(|a| a.tokens.len())
        .sum();
    let edge_words: usize = plan.targets.iter().map(|t| t.len() + 3).sum::<usize>()
        + anchor_edges
        + 3 * plan.transfers.len();
    let digests: Vec<u64> = plan.survivors.iter().map(|s| s.digest).collect();
    let anchors = checked
        .entries
        .iter()
        .filter(|entry| entry.anchors.is_some())
        .count();
    // Steps 2..=6 in order; the first failure stops (short-circuit).
    let reserved = !injected(2)
        && state.reserve_ids(n_s).is_ok()
        && !injected(3)
        && state.store.reserve_exact(&digests).is_ok()
        && !injected(4)
        && state.edges.try_reserve(edge_words).is_ok()
        && !injected(5)
        && state.anchors.try_reserve(anchors).is_ok()
        && !injected(6)
        && records.reserve().is_ok()
        && state
            .store
            .prepare_snapshot_updates(
                state.watermark(),
                &digests,
                plan.survivors
                    .iter()
                    .map(|survivor| survivor.retire.as_slice()),
            )
            .is_ok();
    if !reserved {
        return Err(PreflightError::Stop(StopReason::RamGuard));
    }
    Ok(())
}

/// P3 apply (§6.4): assign IDs in provisional order, maintain the lookup
/// index and the live bits, transfers T3/T10 with their alias runs and
/// seals, then every merging result in cut order (final edge set = P2 tokens
/// plus P1's anchor tokens, sorted and deduplicated; T4/T5/T6, seal iff T4,
/// record), requeue bookkeeping and advance k. Every containment fact comes
/// from a token; a violated invariant is Err (C5, the caller never saves).
/// `state.poisoned` is set for the duration: an Err or a panic leaves it set.
#[allow(clippy::too_many_arguments)]
pub(super) fn p3_apply<const N: usize>(
    state: &mut EpochState<N>,
    checked: Checked<N>,
    plan: MergePlan<N>,
    config: MergeConfig,
    builder: &dyn RecordBuilder<N>,
    records: &mut dyn RecordOut,
    requeue: &mut dyn FnMut(u32, u8),
) -> Result<Applied, Fatal> {
    state.store.ensure_unique().map_err(fatal)?;
    state.poisoned = true;
    let mut applied = Applied::default();
    let merge_epoch = state.k + 1;
    let first_new = state.watermark();
    // Step 1: IDs.
    for survivor in &plan.survivors {
        let id = state
            .store
            .push(survivor.image, survivor.summary, survivor.digest)
            .map_err(|e| fatal(format!("P3: {e}")))?;
        state.admit_id(id);
    }
    let watermark = state.watermark();
    applied.new_ids = plan.survivors.len() as u64;
    state.counters.survivors += plan.survivors.len() as u64;
    // Lookup index and live bits: retire the P2 sets, index the survivor.
    for (pos, survivor) in plan.survivors.iter().enumerate() {
        let mut expected = 0;
        for &old in &survivor.retire {
            if old >= first_new {
                return Err(fatal("P3: retirement of a new ID"));
            }
            if state.is_live(old) {
                expected += 1;
                state.set_live(old, false);
            }
        }
        let removed = state
            .store
            .index_survivor(first_new + pos as u32, &survivor.query, &survivor.retire)
            .map_err(|e| fatal(format!("P3: index maintenance: {e}")))?;
        if removed != expected {
            return Err(fatal(format!(
                "P3: index retired {removed} entries, the live bits expected {expected}"
            )));
        }
    }
    state.tracker.discovered(watermark as usize);
    // Optional observation graph: reserve the cut's upper-bound edge payload
    // once. Allocation failure disables monitoring, never discharges work.
    let tracker_edges = plan.targets.iter().map(Vec::len).sum::<usize>()
        + checked
            .entries
            .iter()
            .filter_map(|e| e.anchors.as_ref())
            .map(|a| a.tokens.len())
            .sum::<usize>()
        + plan.transfers.len();
    state.tracker.reserve_edge_batch(tracker_edges);
    // Step 2: transfers (ascending old ID).
    for (old, token) in &plan.transfers {
        let old = *old;
        let id_token = token
            .into_id(first_new)
            .ok_or_else(|| fatal("P3: transfer token without an ID"))?;
        let to = id_token.id();
        if !(to >= first_new && to < watermark && old < first_new) {
            return Err(fatal(format!("P3: transfer {old} -> {to} out of range")));
        }
        if id_token.q_digest() != state.store.domains[old as usize].digest().0 {
            return Err(fatal(format!(
                "P3: transfer token of {old} names another image"
            )));
        }
        if bucket_key(&state.store.domains[old as usize])
            != bucket_key(&state.store.domains[to as usize])
        {
            return Err(fatal(format!("P3: transfer {old} -> {to} across buckets")));
        }
        let transition = match state.ledger.get(old).map_err(|e| fatal(e.to_string()))? {
            Entry6::Pending(_) if old >= state.p0 => Transition::T3Alias { to },
            Entry6::Exhausted(_) if old >= state.p0 => Transition::T10ExhaustedAlias { to },
            _ => {
                // Reserved, terminal or protected: only the lookup entry retired.
                state.counters.retired_lookup_only += 1;
                continue;
            }
        };
        let exhausted = matches!(transition, Transition::T10ExhaustedAlias { .. });
        state
            .edges
            .append_run(old, &[to], state.is_sealed(old))
            .map_err(|e| fatal(format!("P3: {e}")))?;
        state
            .ledger
            .apply(old, transition)
            .map_err(|e| fatal(format!("P3: {e}")))?;
        state
            .tracker
            .finish_with_sorted_targets(old as usize, &[to], false, true);
        state.nodes[old as usize] |= NODE_SEALED;
        let image = state.store.domains[old as usize];
        records
            .push(builder.alias(old, &image, to, merge_epoch, exhausted))
            .map_err(|e| fatal(format!("P3: record: {e}")))?;
        state.counters.aliases += 1;
        state.counters.transfers += 1;
    }
    // Step 3: results in cut order.
    let mut any_frontier = false;
    let mut any_error = false;
    let mut any_exhausted = false;
    for (entry, tokens) in checked.entries.iter().zip(plan.targets.iter()) {
        let parent = entry.result.parent;
        if !entry.class.merges() {
            continue;
        }
        let misses = &entry.result.misses;
        if !entry.recurring_panic && tokens.len() != misses.len() {
            return Err(fatal(format!(
                "P3: {parent}: {} tokens for {} misses",
                tokens.len(),
                misses.len()
            )));
        }
        let mut targets = Vec::with_capacity(tokens.len() + 1);
        for (token, miss) in tokens.iter().zip(misses) {
            let id = token
                .into_id(first_new)
                .ok_or_else(|| fatal("P3: result token without an ID"))?;
            if id.q_digest() != miss.digest || id.id() >= watermark {
                return Err(fatal(format!("P3: {parent}: token does not name its miss")));
            }
            targets.push(id.id());
        }
        // Anchor edges from P1's tokens (never from a plain ID).
        if let Some(anchors) = &entry.anchors {
            if anchors.tokens.len() != anchors.record.anchors.len() {
                return Err(fatal(format!(
                    "P3: {parent}: anchor tokens and anchors differ"
                )));
            }
            for (token, a) in anchors.tokens.iter().zip(&anchors.record.anchors) {
                let id = token
                    .into_id(first_new)
                    .ok_or_else(|| fatal("P3: anchor token without an ID"))?;
                if id.id() != a.anchor || id.q_digest() != anchors.q_digest || id.id() >= first_new
                {
                    return Err(fatal(format!(
                        "P3: {parent}: anchor token does not name anchor {}",
                        a.anchor
                    )));
                }
                targets.push(id.id());
            }
        }
        targets.sort_unstable();
        targets.dedup();
        let abandoned = entry.result.kind == NativeKind::Abandoned;
        let sealed = entry.class == Class::C0 && !abandoned;
        state
            .edges
            .append_run(parent, &targets, state.is_sealed(parent))
            .map_err(|e| fatal(format!("P3: {e}")))?;
        let inspected = matches!(entry.class, Class::C0 | Class::C4) && !abandoned;
        state
            .tracker
            .finish_with_sorted_targets(parent as usize, &targets, inspected, sealed);
        let anchor_kind = entry.anchor_kind();
        let transition = if abandoned {
            Transition::T13Abandon { epoch: merge_epoch }
        } else {
            match entry.class {
                Class::C0 => Transition::T4Native {
                    epoch: merge_epoch,
                    residual: anchor_kind.is_some_and(AnchorKind::is_g2),
                    dband: anchor_kind == Some(AnchorKind::InitialDBand),
                },
                Class::C4 => Transition::T5Frontier { epoch: merge_epoch },
                _ => Transition::T6Error {
                    epoch: merge_epoch,
                    err: error_class(entry),
                },
            }
        };
        state
            .ledger
            .apply(parent, transition)
            .map_err(|e| fatal(format!("P3: {e}")))?;
        let node = &mut state.nodes[parent as usize];
        if inspected {
            *node |= NODE_INSPECTED;
        }
        if sealed {
            *node |= NODE_SEALED;
        }
        if let Some(anchors) = &entry.anchors {
            *node |= NODE_ANCHORED;
            if anchors.record.kind.is_g2() {
                *node |= NODE_RESIDUAL;
                state.counters.g2_records += 1;
            } else {
                state.counters.partials += 1;
            }
            state
                .anchors
                .push(anchors.record.clone())
                .map_err(|e| fatal(format!("P3: {e}")))?;
        }
        if config.g2 && entry.class == Class::C0 {
            super::g2::publish(state, parent);
        }
        let tag = state.ledger.tag(parent).expect("merged tag") as u8;
        state.edges.fold_record(parent, tag, targets.len() as u32);
        let image = state.store.domains[parent as usize];
        let self_edge = targets.binary_search(&parent).is_ok();
        let record = builder
            .native(
                parent,
                &image,
                entry,
                merge_epoch,
                targets.len() as u32,
                self_edge,
            )
            .map_err(|e| fatal(format!("P3: record: {e}")))?;
        records
            .push(record)
            .map_err(|e| fatal(format!("P3: record: {e}")))?;
        let r = &entry.result;
        let c = &mut state.counters;
        c.events += r.emitted;
        c.successors += r.successors;
        c.conditional += r.conditional;
        c.known_reuse += r.known_reuse;
        c.job_duplicates += r.job_duplicates;
        c.frontiers += if abandoned {
            0
        } else {
            r.frontiers.len() as u64
        };
        c.natives += u64::from(!abandoned);
        if entry.class == Class::C2 {
            c.native_errors += 1;
            any_error = true;
        } else if !abandoned {
            c.completed += 1;
            c.initial_inspected += u64::from(parent < state.p0);
        }
        if r.kind == NativeKind::Route {
            c.routed += 1;
            c.route_masks += r.route_masks;
            c.route_joint_pruned += r.route_joint_pruned;
        } else {
            c.optional_total += r.optional[0];
            c.optional_original += r.optional[1];
            c.optional_coalesced += r.optional[2];
        }
        if entry.class == Class::C4 && !abandoned {
            any_frontier = true;
            state
                .frontier_counts
                .insert(parent, r.frontiers.len() as u32);
        }
        state.in_flight.remove(&parent);
        applied.merged += 1;
    }
    // Step 4: requeue bookkeeping for discarded C1/C3 results.
    for entry in &checked.entries {
        if entry.class.merges() {
            continue;
        }
        let parent = entry.result.parent;
        let cause = entry.cause.unwrap_or(Cause::Unknown);
        let counters = state
            .ledger
            .get(parent)
            .map_err(|e| fatal(e.to_string()))?
            .counters()
            .ok_or_else(|| fatal(format!("P3: discarded {parent} has no counters")))?;
        let d_attempts = cause.d_attempts();
        let exhaust = counters.attempts.saturating_add(d_attempts) >= MAX_ATTEMPTS
            || counters.guard >= MAX_GUARD;
        let last_err = entry.class.code();
        let transition = if exhaust {
            Transition::T8Exhaust {
                d_attempts,
                d_guard: 0,
                last_err,
            }
        } else {
            Transition::T7Requeue {
                d_attempts,
                d_guard: 0,
                last_err,
            }
        };
        state
            .ledger
            .apply(parent, transition)
            .map_err(|e| fatal(format!("P3: {e}")))?;
        state.in_flight.remove(&parent);
        state.counters.discarded += 1;
        if exhaust {
            any_exhausted = true;
        } else {
            state.counters.requeued += 1;
            requeue(parent, counters.attempts.saturating_add(d_attempts));
        }
        applied.requeued.push((parent, exhaust));
    }
    // Step 5: advance.
    state.k = merge_epoch;
    state.counters.merges += 1;
    state.poisoned = false;
    applied.stop = if any_error {
        Some(StopReason::ErrorStop)
    } else if any_exhausted {
        Some(StopReason::ExhaustedStop)
    } else if any_frontier && config.frontier_stop {
        Some(StopReason::FrontierStop)
    } else {
        None
    };
    Ok(applied)
}

/// A merge-level discard (P1 allowance stop or a failed preflight): every
/// result of the cut goes back to the requeue with no counter change
/// (§8.4: allowance, preflight and stop causes never penalize a job).
pub(super) fn discard_cut<const N: usize>(
    state: &mut EpochState<N>,
    checked: &Checked<N>,
    requeue: &mut dyn FnMut(u32, u8),
) -> Result<(), Fatal> {
    for entry in &checked.entries {
        let parent = entry.result.parent;
        let counters = state
            .ledger
            .get(parent)
            .map_err(|e| fatal(e.to_string()))?
            .counters()
            .ok_or_else(|| fatal("discarded result without counters"))?;
        state
            .ledger
            .apply(
                parent,
                Transition::T7Requeue {
                    d_attempts: 0,
                    d_guard: 0,
                    last_err: counters.last_err,
                },
            )
            .map_err(|e| fatal(format!("discard: {e}")))?;
        state.in_flight.remove(&parent);
        state.counters.discarded += 1;
        requeue(parent, counters.attempts);
    }
    Ok(())
}
