//! The serial merge of one cut (W2.0 protocol §6): P1 checks and classifies
//! every result, P2 plans (canonical in-merge resolution of every miss,
//! the cut's antichain, reverse retirement sets with transfer tokens), P3
//! preflights every reservation and then applies the plan without a
//! containment decision. S2 runs P2 serially on the coordinator and has no
//! P4 (the lookup index is maintained inside P3, IMP-13).
use super::super::queue::{CompactDomain, CompactSummary, Domain, Query};
use super::anchors::{AnchorKind, AnchorRecord, AnchorScope, AnchorView};
use super::job::{BreakReason, ErrorKind, JobResult, NativeKind, Writer, write_image};
use super::ledger6::{EPOCH_LIMIT, Entry6, MAX_ATTEMPTS, MAX_GUARD, Transition};
use super::state::{EpochState, NODE_ANCHORED, NODE_INSPECTED, NODE_RESIDUAL, NODE_SEALED};
use super::store::bucket_key;
use super::verify::{Container, QueryImage, Verified, verify};
use serde_json::Value;
use std::collections::HashMap;

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

pub(super) struct CheckedResult<const N: usize> {
    pub result: JobResult<N>,
    pub class: Class,
    pub cause: Option<Cause>,
    /// A recurring C3 merged as C2: no successors, no edges.
    pub recurring_panic: bool,
    /// A re-validated InitialDBand anchor (the anchor verified to contain the
    /// node's D >= cut slice).
    pub dband: Option<(u32, i64)>,
}

pub(super) struct Checked<const N: usize> {
    pub entries: Vec<CheckedResult<N>>,
    /// A merge-level stop decided in P1 (allowances): every result of the
    /// cut is then discarded without counter changes.
    pub stop: Option<StopReason>,
}

/// Merge-level settings.
#[derive(Clone, Copy)]
pub(super) struct MergeConfig {
    pub frontier_stop: bool,
    /// The merge applies a lockstep cut: every result was dispatched at the
    /// current k (depth 1).
    pub lockstep: bool,
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

/// P1: per result, the in-flight and ledger checks, the event parity and
/// the class; InitialDBand anchors re-validated (R1/R2 plus the geometry
/// through `verify`); aggregate allowances. Any protocol violation is C5.
pub(super) fn p1_check<const N: usize>(
    state: &mut EpochState<N>,
    cut: Vec<Vec<u8>>,
    config: MergeConfig,
) -> Result<Checked<N>, Fatal> {
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
        if result.v0 > state.k || (config.lockstep && result.v0 != state.k) {
            return Err(fatal(format!(
                "P1: result v0 {} beyond k {}",
                result.v0, state.k
            )));
        }
        let (class, cause) = classify(&result, counters.last_err)?;
        let recurring_panic = result.panic && class == Class::C2;
        // Kind and scope agree; InitialDBand re-validation (§6.2, §7 R2).
        let is_partial = result.kind == NativeKind::ApplyPartial;
        if is_partial != result.scope.is_some() {
            return Err(fatal(format!("P1: {parent} partial kind/scope mismatch")));
        }
        let mut dband = None;
        if let Some(scope) = result.scope {
            let anchor = scope.anchor;
            let domain = state.store.domains[parent as usize].expand();
            let record = AnchorRecord {
                node: parent,
                kind: AnchorKind::InitialDBand,
                dispatch_version: result.v0,
                scope: AnchorScope::DBandCut(scope.cut),
                anchors: vec![(anchor, None)],
            };
            let store = &state.store;
            let nodes = &state.nodes;
            let same_bucket = |a: u32, b: u32| {
                (a as usize) < store.len()
                    && (b as usize) < store.len()
                    && bucket_key(&store.domains[a as usize])
                        == bucket_key(&store.domains[b as usize])
            };
            let has_anchors = |a: u32| {
                nodes
                    .get(a as usize)
                    .is_some_and(|f| f & NODE_ANCHORED != 0)
            };
            let view = AnchorView {
                p0: state.p0,
                ledger: &state.ledger,
                same_bucket: &same_bucket,
                has_anchors: &has_anchors,
            };
            record
                .validate(&view, state.k + 1)
                .map_err(|v| fatal(format!("P1: {parent} anchor {anchor}: {v:?}")))?;
            if residual_powers(domain.powers, scope.cut) != Some(scope.residual) {
                return Err(fatal(format!(
                    "P1: {parent} residual is not the D < cut slice"
                )));
            }
            let high = CompactDomain::try_from_domain(&high_slice(&domain, scope.cut))
                .map_err(|e| fatal(format!("P1: {parent} high slice: {e}")))?;
            let q = QueryImage::new(high).map_err(|e| fatal(format!("P1: {e}")))?;
            let published_len = state.store.len();
            verify(
                Container::Stored {
                    id: anchor,
                    domains: &state.store.domains,
                    published_len,
                },
                &q,
                &mut state.verify,
            )
            .ok_or_else(|| {
                fatal(format!(
                    "P1: {parent} anchor {anchor} does not contain the D >= {} slice",
                    scope.cut
                ))
            })?;
            dband = Some((anchor, scope.cut));
        }
        entries.push(CheckedResult {
            result,
            class,
            cause,
            recurring_panic,
            dband,
        });
    }
    entries.sort_by_key(|entry| entry.result.parent);
    // Aggregate allowances over the merging results (§11.5: not bound).
    let merging = || entries.iter().filter(|entry| entry.class.merges());
    let events: u64 = merging().map(|entry| entry.result.emitted).sum();
    let frontiers: u64 = merging()
        .map(|entry| entry.result.frontiers.len() as u64)
        .sum();
    let stop = if state.counters.events.saturating_add(events) > state.max_events {
        Some(StopReason::EventAllowance)
    } else if state.counters.frontiers.saturating_add(frontiers) > state.max_frontiers {
        Some(StopReason::FrontierAllowance)
    } else {
        None
    };
    Ok(Checked { entries, stop })
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
                // The breaking event is emitted but not accepted.
                if r.accepted + 1 != r.emitted {
                    return Err(fatal(format!("P1: {parent}: break parity")));
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
        ErrorKind::NativeFailure | ErrorKind::Conversion => {
            if r.break_reason != BreakReason::None || r.emitted != r.accepted {
                return Err(fatal(format!("P1: {parent}: native failure parity")));
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
        use super::super::queue::{Coordinates, Entry, Signature};
        let mut index = super::super::queue::AggregateIndex::default();
        for (local, &slot) in members.iter().enumerate() {
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
        use super::super::queue::{Coordinates, Probe, Signature};
        let probe = Probe::new(
            Coordinates::of(&query.q.core),
            query.query.word,
            query.query.lanes,
            true,
        );
        self.index.find_from(
            Signature::of(&query.q.core),
            &probe,
            0,
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

/// P2 (serial in S2): canonical resolution of every miss against S_k, the
/// order-independent antichain of the remaining misses per bucket,
/// provisional survivor order (first parent position, ordinal), reverse sets
/// and transfer tokens. Reads the state only.
pub(super) fn p2_plan<const N: usize>(
    state: &mut EpochState<N>,
    checked: &Checked<N>,
) -> Result<MergePlan<N>, Fatal> {
    let published_len = state.store.len();
    // Per entry, in miss order: a resolved token, or a candidate of the cut.
    let mut slots_of: Vec<Vec<Result<Verified, u32>>> = Vec::with_capacity(checked.entries.len());
    let mut candidates: Vec<Candidate<N>> = Vec::new();
    let mut by_digest: HashMap<u64, Vec<u32>> = HashMap::new();
    for (position, entry) in checked.entries.iter().enumerate() {
        let mut entry_slots = Vec::new();
        if entry.class.merges() && !entry.recurring_panic {
            for miss in &entry.result.misses {
                state.counters.miss_requests += 1;
                // F1 (SND-7): the digest recomputed from the shipped image.
                if miss.image.digest().0 != miss.digest {
                    return Err(fatal(format!(
                        "P2: {} miss {}: shipped digest differs from its image",
                        entry.result.parent, miss.ordinal
                    )));
                }
                let q = QueryImage::new(miss.image).map_err(|e| fatal(format!("P2: {e}")))?;
                let query = Query::new(q.core.clone(), miss.image.phase());
                let found = state
                    .store
                    .lookup(
                        &q,
                        &query,
                        published_len,
                        &mut state.lookup,
                        &mut state.verify,
                    )
                    .map_err(|e| fatal(format!("P2: {e}")))?;
                if let Some((_, token, _hit)) = found {
                    entry_slots.push(Ok(token));
                    continue;
                }
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
                &mut state.verify,
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
    state.counters.antichain_folded += (candidates.len() - n_s) as u64;
    // Reverse sets (A4) and transfer tokens (F4 re-verify in P2).
    let mut survivors = Vec::with_capacity(n_s);
    let mut assigned: HashMap<u32, Verified> = HashMap::new();
    for (pos, &slot) in order.iter().enumerate() {
        let candidate = &candidates[slot as usize];
        let retire = state
            .store
            .contained_live(&candidate.q, &candidate.query, &mut state.lookup)
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
                &mut state.verify,
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
    fn push(&mut self, record: Value) -> Result<(), String>;
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
    ) -> Result<Value, String>;
    fn alias(
        &self,
        id: u32,
        image: &CompactDomain<N>,
        to: u32,
        merge_epoch: u64,
        exhausted: bool,
    ) -> Value;
}

/// P3 preflight (§6.4): capacities in every arena; the ID cap (F9) and the
/// u48 epoch (F13). A failure changes nothing logical; the caller discards
/// the cut without counter changes and stops.
pub(super) fn p3_preflight<const N: usize>(
    state: &mut EpochState<N>,
    checked: &Checked<N>,
    plan: &MergePlan<N>,
    records: &mut dyn RecordOut,
) -> Result<(), StopReason> {
    let n_s = plan.survivors.len();
    if state.store.len() + n_s > state.id_cap() {
        return Err(StopReason::DomainAllowance);
    }
    if state.k + 1 >= EPOCH_LIMIT {
        return Err(StopReason::Capacity);
    }
    let edge_words: usize =
        plan.targets.iter().map(|t| t.len() + 3).sum::<usize>() + 3 * plan.transfers.len();
    let digests: Vec<u64> = plan.survivors.iter().map(|s| s.digest).collect();
    let anchors = checked
        .entries
        .iter()
        .filter(|entry| entry.dband.is_some())
        .count();
    let reserved = state
        .reserve_ids(n_s)
        .and_then(|()| state.store.exact.try_reserve(&digests))
        .and_then(|()| state.edges.try_reserve(edge_words))
        .and_then(|()| state.anchors.try_reserve(anchors));
    if reserved.is_err() || records.reserve().is_err() {
        return Err(StopReason::RamGuard);
    }
    Ok(())
}

/// P3 apply (§6.4): assign IDs in provisional order, maintain the lookup
/// index and the live bits, transfers T3/T10 with their alias runs and
/// seals, then every merging result in cut order (final edge set sorted and
/// deduplicated, T4/T5/T6, seal iff T4, record), requeue bookkeeping and
/// advance k. Every containment fact comes from a P2 token; a violated
/// invariant is Err (C5, the caller never saves).
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
    let mut applied = Applied::default();
    let merge_epoch = state.k + 1;
    let first_new = state.watermark();
    // Step 1: IDs.
    for survivor in &plan.survivors {
        let id = state
            .store
            .push(survivor.image, survivor.summary, survivor.digest);
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
        state.tracker.edge(old as usize, to as usize);
        state.tracker.finish(old as usize, false, true);
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
        if let Some((anchor, _)) = entry.dband {
            targets.push(anchor);
        }
        targets.sort_unstable();
        targets.dedup();
        let sealed = entry.class == Class::C0;
        state
            .edges
            .append_run(parent, &targets, state.is_sealed(parent))
            .map_err(|e| fatal(format!("P3: {e}")))?;
        for &target in &targets {
            state.tracker.edge(parent as usize, target as usize);
        }
        let inspected = matches!(entry.class, Class::C0 | Class::C4);
        state.tracker.finish(parent as usize, inspected, sealed);
        let transition = match entry.class {
            Class::C0 => Transition::T4Native {
                epoch: merge_epoch,
                residual: false,
                dband: entry.dband.is_some(),
            },
            Class::C4 => Transition::T5Frontier { epoch: merge_epoch },
            _ => Transition::T6Error {
                epoch: merge_epoch,
                err: if entry.recurring_panic { 3 } else { 2 },
            },
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
        if let Some((anchor, cut)) = entry.dband {
            *node |= NODE_ANCHORED | NODE_RESIDUAL;
            state.anchors.push(AnchorRecord {
                node: parent,
                kind: AnchorKind::InitialDBand,
                dispatch_version: entry.result.v0,
                scope: AnchorScope::DBandCut(cut),
                anchors: vec![(anchor, None)],
            });
            state.counters.partials += 1;
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
        c.frontiers += r.frontiers.len() as u64;
        c.natives += 1;
        if entry.class == Class::C2 {
            c.native_errors += 1;
            any_error = true;
        } else {
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
        if entry.class == Class::C4 {
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
