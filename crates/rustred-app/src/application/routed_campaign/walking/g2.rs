//! G2' residual anchors (request `--g2-residual-anchors union`; off by
//! default, and then the engine is byte-identical to a build without it).
//!
//! A newly dispatched Apply domain Q (ID >= the initial prefix, whole
//! inspection, no initial-D-band plan) is inspected only on a D band: the D
//! levels of Q at the top and at the bottom that are covered, point by point,
//! by the union of ANCHORS are cut off, and the job inspects the band between
//! them (one piece; nothing when every level is covered). An anchor is a
//! merged record of Q's (Apply, owner) bucket whose lent scope is fully
//! discharged by its own inspection history:
//! - a Native Apply record (0 frontiers) lends its whole domain;
//! - a G2' residual record lends its whole domain: its residual was
//!   inspected and every other point of it was covered by its own anchors,
//!   which were merged before its snapshot (owner decision 2026-09-28: union
//!   form, well-founded in merge order);
//! - an initial-D-band partial lends only its inspected low-D slice (its high
//!   slice is delegated to an initial anchor that may merge later; W2 protocol
//!   §7 R3). G2' full covers lend nothing new (their points are covered by
//!   their own, older anchors) and are not indexed.
//!
//! Merge stamps: a record's stamp is its position in the publication stream
//! (the ledger's publication count when it is published). A plan reads a
//! SNAPSHOT stamp and may only use anchors with `stamp < snapshot`; the job
//! itself publishes later, so `snapshot <= own stamp` and anchor links
//! strictly decrease the stamp (no responsibility cycle). Ordered walks use
//! the deterministic snapshot `id + 1 - lookahead` (every ID up to
//! `id - lookahead` is published before `id` can be dispatched), so an
//! Ordered G2' walk is reproducible across widths and pause/resume; Ready
//! walks read the current publication count.
//!
//! Exactness: the planner enumerates Q's lattice points (at most
//! `POINT_CAP`, else Q is inspected whole), assigns each covered point to one
//! anchor containing it (exact interval membership), and confirms every
//! assignment with the native `DomainPowerSummary::contains` of the one-point
//! domain (the authority). The residual record carries its anchors, stamps
//! and residual band; restore validators and the offline verifier re-check
//! `Q <= residual u anchors` with `lattice::Cell::covered_by_union`.
mod index;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use rustred::solver::{DomainPowerBounds, DomainPowerSummary};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(super) use index::Entry;
use index::{Bucket, Point, Query, Shape};

use super::queue::{CompactDomain, Domain, Phase};
use super::verify_closure::lattice::Cell;

/// Largest query (lattice points) a plan enumerates; larger Q is inspected whole.
pub(super) const POINT_CAP: usize = 1 << 18;
/// Epoch admission and persisted-anchor validation must use the same exact
/// union predicate and region budget. Pointwise discovery alone is not enough
/// to guarantee that the independent persisted proof can be replayed.
pub(super) const EPOCH_COVER_REGIONS: u64 = 1 << 16;
/// The independent union predicate recurses per target. Limit optimization
/// proposals before running it on an inspector's stack; larger unions are
/// inspected whole, so this is not a bound on supported mathematical scope.
const EPOCH_COVER_ANCHORS: usize = 256;
/// Membership tests a plan may spend beyond 64 per point.
const TEST_BUDGET: u64 = 1 << 22;

/// Request policy for G2' residual anchors (bound into the checkpoint).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum G2ResidualAnchors {
    /// No G2' planning (the historical engine).
    #[default]
    Off,
    /// D-band residual inspection against the union of merged anchors.
    Union,
}
impl G2ResidualAnchors {
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Union => "union",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "union" => Some(Self::Union),
            _ => None,
        }
    }
}

/// Record kinds of merged Apply native records (ledger G2' log and anchors).
pub(super) mod kind {
    pub const NATIVE: u8 = 0;
    pub const INITIAL_D_BAND: u8 = 1;
    pub const G2_RESIDUAL: u8 = 2;
    pub const G2_FULL_COVER: u8 = 3;
    pub fn name(kind: u8) -> &'static str {
        match kind {
            NATIVE => "native",
            INITIAL_D_BAND => "initial_d_band",
            G2_RESIDUAL => "g2_residual",
            G2_FULL_COVER => "g2_full_cover",
            _ => "invalid",
        }
    }
    /// The scope an anchor of this kind lends.
    pub fn scope(kind: u8) -> &'static str {
        match kind {
            INITIAL_D_BAND => "inspected_low_D_slice",
            _ => "domain",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct AnchorRef {
    pub id: u32,
    pub stamp: u64,
    pub kind: u8,
}

/// What a G2'-planned job inspects and what it relies on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Plan {
    pub snapshot: u64,
    /// Inspected D band `[lo, hi]` of Q; None: every point is covered.
    pub residual: Option<(i64, i64)>,
    /// Sorted by stamp, then ID; distinct.
    pub anchors: Vec<AnchorRef>,
    pub points: u32,
    pub covered_points: u32,
    pub candidates: u32,
    pub membership_tests: u64,
}

impl Plan {
    /// The residual domain (Q's coordinates and rank; D restricted).
    pub fn residual_domain<const N: usize>(&self, q: &Domain<N>) -> Option<Domain<N>> {
        let (lo, hi) = self.residual?;
        Some(Domain {
            powers: residual_powers(q.powers, lo, hi),
            ..q.clone()
        })
    }
    pub fn scope(&self) -> G2Scope {
        G2Scope {
            snapshot: self.snapshot,
            residual: self.residual,
            anchors: self.anchors.len() as u32,
        }
    }
}

pub(super) fn residual_powers(q: DomainPowerBounds, lo: i64, hi: i64) -> DomainPowerBounds {
    DomainPowerBounds {
        max_positive_power: q.max_positive_power,
        min_power_difference: Some(q.min_power_difference.map_or(lo, |d| d.max(lo))),
        max_power_difference: Some(q.max_power_difference.map_or(hi, |d| d.min(hi))),
    }
}

/// The planner's decision for one eligible job (pinned across a checkpoint
/// while the job's stream holds accepted events).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Outcome {
    Whole,
    Planned(Plan),
}

/// Copy summary carried by `Finished`; the full plan stays in the store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct G2Scope {
    pub snapshot: u64,
    pub residual: Option<(i64, i64)>,
    pub anchors: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Whole {
    Unrepresentable,
    Empty,
    Unbounded,
    TooLarge,
    NoCandidates,
    NoCoveredLevel,
    OverBudget,
    AuthorityRefused,
    UnionUndecided,
    UnionLenderBudget,
}

#[derive(Default)]
struct Stats {
    plans: AtomicU64,
    pinned: AtomicU64,
    full_cover: AtomicU64,
    residual: AtomicU64,
    whole: [AtomicU64; 10],
    points: AtomicU64,
    covered_points: AtomicU64,
    candidates: AtomicU64,
    membership_tests: AtomicU64,
    anchor_edges: AtomicU64,
    original_levels: AtomicU64,
    residual_levels: AtomicU64,
    hull_visits: AtomicU64,
    entry_visits: AtomicU64,
    tail_visits: AtomicU64,
    plan_micros: AtomicU64,
    appended: AtomicU64,
    unindexed: AtomicU64,
}

const WHOLE_NAMES: [&str; 10] = [
    "unrepresentable",
    "empty",
    "unbounded",
    "over_point_cap",
    "no_candidates",
    "no_covered_level",
    "over_test_budget",
    "authority_refused",
    "union_cover_undecided",
    "union_cover_lender_budget",
];

/// Shared G2' state of one walk session: the anchor index, the plans that
/// workers hand to the coordinator, and plans pinned by a restore.
pub(super) struct Store<const N: usize> {
    ordered_lag: Option<u64>,
    initial_count: usize,
    buckets: RwLock<HashMap<[bool; N], Arc<Bucket<N>>>>,
    /// Ready: every anchor with a smaller stamp is in the index.
    published: AtomicU64,
    plans: Mutex<HashMap<usize, Arc<Outcome>>>,
    pinned: Mutex<HashMap<usize, Arc<Outcome>>>,
    stats: Stats,
}

impl<const N: usize> Store<N> {
    pub fn new(ordered_lag: Option<u64>, initial_count: usize, published: u64) -> Self {
        Self {
            ordered_lag,
            initial_count,
            buckets: RwLock::new(HashMap::new()),
            published: AtomicU64::new(published),
            plans: Mutex::new(HashMap::new()),
            pinned: Mutex::new(HashMap::new()),
            stats: Stats::default(),
        }
    }

    /// Whether job `id` on `domain` is G2'-eligible (R1: never an initial ID).
    pub fn eligible(&self, id: usize, domain: &Domain<N>) -> bool {
        domain.phase == Phase::Apply && id >= self.initial_count
    }

    fn bucket(&self, owner: &[bool; N]) -> Option<Arc<Bucket<N>>> {
        self.buckets.read().expect("g2 buckets").get(owner).cloned()
    }
    fn bucket_or_insert(&self, owner: &[bool; N]) -> Arc<Bucket<N>> {
        if let Some(bucket) = self.bucket(owner) {
            return bucket;
        }
        self.buckets
            .write()
            .expect("g2 buckets")
            .entry(*owner)
            .or_default()
            .clone()
    }

    /// The anchor entry of a merged record's lent scope (None: empty scope).
    pub fn entry(scope: &Domain<N>, id: usize, stamp: u64, kind: u8) -> Option<Entry<N>> {
        let compact = CompactDomain::try_from_domain(scope).ok()?;
        let (lower, upper) = compact.raw_bounds();
        let shape = Shape::of(&scope.owner, lower, upper, scope.rank, scope.powers)?;
        Some(Entry {
            scope: compact,
            shape,
            stamp,
            id: u32::try_from(id).ok()?,
            kind,
        })
    }

    /// Coordinator: index one merged anchor-eligible record. Call BEFORE
    /// `published` advances past its stamp.
    pub fn append(&self, owner: &[bool; N], entry: Option<Entry<N>>) {
        match entry {
            Some(entry) => {
                self.bucket_or_insert(owner).append(entry);
                self.stats.appended.fetch_add(1, Ordering::Relaxed);
            }
            None => {
                self.stats.unindexed.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Restore: every anchor-eligible record, in merge order.
    pub fn bulk(&self, entries: Vec<([bool; N], Entry<N>)>) {
        let mut by_owner: HashMap<[bool; N], Vec<Entry<N>>> = HashMap::new();
        let count = entries.len() as u64;
        for (owner, entry) in entries {
            by_owner.entry(owner).or_default().push(entry);
        }
        for (owner, entries) in by_owner {
            self.bucket_or_insert(&owner).bulk(entries);
        }
        self.stats.appended.fetch_add(count, Ordering::Relaxed);
    }

    /// Coordinator: every record with a stamp below `count` is indexed.
    pub fn published(&self, count: u64) {
        self.published.fetch_max(count, Ordering::Release);
    }

    fn snapshot(&self, id: usize) -> u64 {
        match self.ordered_lag {
            Some(lag) => (id as u64 + 1).saturating_sub(lag),
            None => self.published.load(Ordering::Acquire),
        }
    }

    /// Worker: the decision for job `id` (a pinned one first), handed to the
    /// coordinator through the plan table before any event is emitted.
    pub fn decide(&self, id: usize, q: &Domain<N>, cancellation: &AtomicBool) -> Arc<Outcome> {
        let pinned = self.pinned.lock().expect("g2 pins").remove(&id);
        let outcome = match pinned {
            Some(outcome) => {
                self.stats.pinned.fetch_add(1, Ordering::Relaxed);
                outcome
            }
            None => Arc::new(self.plan(id, q, cancellation)),
        };
        self.plans
            .lock()
            .expect("g2 plans")
            .insert(id, outcome.clone());
        outcome
    }

    /// Coordinator at commit (or retention): the decision of job `id`.
    pub fn take(&self, id: usize) -> Option<Arc<Outcome>> {
        self.plans.lock().expect("g2 plans").remove(&id)
    }

    /// Checkpoint save: the decision of an unfinished job, if made.
    pub fn peek(&self, id: usize) -> Option<Arc<Outcome>> {
        self.plans.lock().expect("g2 plans").get(&id).cloned()
    }

    /// Checkpoint save: pins restored but not yet consumed by a dispatch.
    pub fn pending_pins(&self) -> Vec<(usize, Arc<Outcome>)> {
        self.pinned
            .lock()
            .expect("g2 pins")
            .iter()
            .map(|(id, outcome)| (*id, outcome.clone()))
            .collect()
    }

    /// Restore: a job whose stream holds accepted events re-runs this decision.
    pub fn pin(&self, id: usize, outcome: Outcome) {
        self.pinned
            .lock()
            .expect("g2 pins")
            .insert(id, Arc::new(outcome));
    }

    fn plan(&self, id: usize, q: &Domain<N>, cancellation: &AtomicBool) -> Outcome {
        self.plan_bound(None, id, q, cancellation, None)
    }

    /// Epoch adapter: an explicit exclusive merge-stamp bound. This does not
    /// consult publication timing or retain a callback plan in the CP5 tables.
    /// Before emitting residual events, require the same bounded union proof
    /// as P1/restore. An undecidable optimization becomes a whole inspection,
    /// not an engine invariant failure after partial inspection has begun.
    pub fn plan_at(&self, snapshot: u64, q: &Domain<N>, cancellation: &AtomicBool) -> Outcome {
        self.plan_bound(
            Some(snapshot),
            0,
            q,
            cancellation,
            Some(EPOCH_COVER_REGIONS),
        )
    }

    fn plan_bound(
        &self,
        snapshot: Option<u64>,
        id: usize,
        q: &Domain<N>,
        cancellation: &AtomicBool,
        cover_regions: Option<u64>,
    ) -> Outcome {
        let started = Instant::now();
        self.stats.plans.fetch_add(1, Ordering::Relaxed);
        let result = self.plan_inner(snapshot, id, q, cancellation, cover_regions);
        self.stats
            .plan_micros
            .fetch_add(started.elapsed().as_micros() as u64, Ordering::Relaxed);
        match result {
            Ok(plan) => {
                let s = &self.stats;
                if plan.residual.is_some() {
                    s.residual.fetch_add(1, Ordering::Relaxed);
                } else {
                    s.full_cover.fetch_add(1, Ordering::Relaxed);
                }
                s.anchor_edges
                    .fetch_add(plan.anchors.len() as u64, Ordering::Relaxed);
                s.covered_points
                    .fetch_add(u64::from(plan.covered_points), Ordering::Relaxed);
                Outcome::Planned(plan)
            }
            Err(reason) => {
                self.stats.whole[reason as usize].fetch_add(1, Ordering::Relaxed);
                Outcome::Whole
            }
        }
    }

    fn plan_inner(
        &self,
        snapshot: Option<u64>,
        id: usize,
        q: &Domain<N>,
        cancellation: &AtomicBool,
        cover_regions: Option<u64>,
    ) -> Result<Plan, Whole> {
        let compact = CompactDomain::try_from_domain(q).map_err(|_| Whole::Unrepresentable)?;
        let (lower, upper) = compact.raw_bounds();
        let shape = Shape::of(&q.owner, lower, upper, q.rank, q.powers).ok_or(Whole::Empty)?;
        let query = Query {
            lower: *lower,
            upper: *upper,
            shape,
        };
        let mut points = Vec::new();
        enumerate(&q.owner, &query, POINT_CAP, &mut points)?;
        if points.is_empty() {
            return Err(Whole::Empty);
        }
        let s = &self.stats;
        s.points.fetch_add(points.len() as u64, Ordering::Relaxed);
        // Preserve the historical CP5 snapshot point after enumeration.
        let snapshot = snapshot.unwrap_or_else(|| self.snapshot(id));
        // A bucket may exist only through anchors merged after the snapshot:
        // no bucket and no visible candidate are one outcome (deterministic).
        let bucket = self.bucket(&q.owner).ok_or(Whole::NoCandidates)?;
        bucket.compact_if_pending();
        let mut candidates = Vec::new();
        let visits = bucket.query(&query, snapshot, &mut candidates);
        s.hull_visits.fetch_add(visits.hulls, Ordering::Relaxed);
        s.entry_visits.fetch_add(visits.entries, Ordering::Relaxed);
        s.tail_visits.fetch_add(visits.tail, Ordering::Relaxed);
        s.candidates
            .fetch_add(candidates.len() as u64, Ordering::Relaxed);
        if candidates.is_empty() {
            return Err(Whole::NoCandidates);
        }
        // Deterministic order: oldest anchor first.
        candidates.sort_unstable_by_key(|entry| (entry.stamp, entry.id));
        // D levels from the top down (points in a fixed order within a level).
        points.sort_unstable_by(|a, b| b.d.cmp(&a.d).then(a.x.cmp(&b.x)));
        let mut levels: Vec<(i32, usize, usize)> = Vec::new();
        let mut start = 0;
        while start < points.len() {
            let d = points[start].d;
            let end = start + points[start..].iter().take_while(|p| p.d == d).count();
            levels.push((d, start, end));
            start = end;
        }
        let budget = TEST_BUDGET + 64 * points.len() as u64;
        let mut tests = 0u64;
        let mut used: Vec<usize> = Vec::new();
        let mut is_used = vec![false; candidates.len()];
        let mut assigned: Vec<(usize, usize)> = Vec::new();
        let mut cover = |level: (i32, usize, usize),
                         used: &mut Vec<usize>,
                         is_used: &mut Vec<bool>,
                         assigned: &mut Vec<(usize, usize)>|
         -> Result<bool, Whole> {
            let mut local = Vec::with_capacity(level.2 - level.1);
            let mut fresh: Vec<usize> = Vec::new();
            for index in level.1..level.2 {
                let point = &points[index];
                // Anchors already used first, then every candidate by age.
                let mut hit = None;
                for &c in used.iter().chain(fresh.iter()) {
                    tests += 1;
                    if candidates[c].contains(point) {
                        hit = Some(c);
                        break;
                    }
                }
                if hit.is_none() {
                    for (c, candidate) in candidates.iter().enumerate() {
                        if is_used[c] || fresh.contains(&c) {
                            continue;
                        }
                        tests += 1;
                        if candidate.contains(point) {
                            hit = Some(c);
                            break;
                        }
                    }
                }
                if tests > budget {
                    return Err(Whole::OverBudget);
                }
                let Some(c) = hit else {
                    return Ok(false);
                };
                if !is_used[c] && !fresh.contains(&c) {
                    fresh.push(c);
                }
                local.push((index, c));
            }
            for c in fresh {
                is_used[c] = true;
                used.push(c);
            }
            assigned.extend(local);
            Ok(true)
        };
        let mut top = 0;
        while top < levels.len() && cover(levels[top], &mut used, &mut is_used, &mut assigned)? {
            top += 1;
        }
        let mut bottom = levels.len();
        if top < levels.len() {
            while bottom - 1 > top
                && cover(levels[bottom - 1], &mut used, &mut is_used, &mut assigned)?
            {
                bottom -= 1;
            }
        }
        s.membership_tests.fetch_add(tests, Ordering::Relaxed);
        if assigned.is_empty() {
            return Err(Whole::NoCoveredLevel);
        }
        if cancellation.load(Ordering::Relaxed) {
            // The job is being cancelled: keep it whole (never published).
            return Err(Whole::NoCoveredLevel);
        }
        // Authority: native inclusion of every covered point in its anchor.
        let mut summaries: HashMap<usize, DomainPowerSummary<N>> = HashMap::new();
        for &(index, c) in &assigned {
            let summary = match summaries.get(&c) {
                Some(summary) => summary,
                None => {
                    let scope = candidates[c].scope.expand();
                    let summary = DomainPowerSummary::try_new(
                        scope.owner,
                        &scope.lower,
                        &scope.upper,
                        scope.rank,
                        scope.powers,
                    )
                    .map_err(|_| Whole::AuthorityRefused)?;
                    summaries.entry(c).or_insert(summary)
                }
            };
            let x: [u64; N] = std::array::from_fn(|axis| u64::from(points[index].x[axis]));
            let upper: [Option<u64>; N] = x.map(Some);
            let one = DomainPowerSummary::try_new(
                q.owner,
                &x,
                &upper,
                None,
                DomainPowerBounds::default(),
            )
            .map_err(|_| Whole::AuthorityRefused)?;
            if one.is_empty() || !summary.contains(&one) {
                return Err(Whole::AuthorityRefused);
            }
        }
        let residual = (top < bottom).then(|| {
            // levels run from the highest D down: [bottom-1] is the lowest
            // uncovered level, [top] the highest.
            (i64::from(levels[bottom - 1].0), i64::from(levels[top].0))
        });
        if let Some(max_regions) = cover_regions {
            if used.len() > EPOCH_COVER_ANCHORS {
                return Err(Whole::UnionLenderBudget);
            }
            // P1 records the residual first and then lenders in (stamp, ID)
            // order. Region-budget exhaustion is order-sensitive: preserve
            // that order rather than the discovery order in `used`. Legacy
            // planning needs no additional sort or union preflight.
            used.sort_unstable_by_key(|&c| (candidates[c].stamp, candidates[c].id));
            let proof = replayable_union_cover(
                q,
                residual,
                used.iter().map(|&c| &candidates[c].scope),
                max_regions,
            );
            match proof {
                Some(true) => {}
                Some(false) => return Err(Whole::AuthorityRefused),
                None => return Err(Whole::UnionUndecided),
            }
            if cancellation.load(Ordering::Relaxed) {
                return Err(Whole::NoCoveredLevel);
            }
        }
        s.original_levels
            .fetch_add(levels.len() as u64, Ordering::Relaxed);
        s.residual_levels
            .fetch_add((bottom - top) as u64, Ordering::Relaxed);
        let mut anchors: Vec<AnchorRef> = used
            .iter()
            .map(|&c| AnchorRef {
                id: candidates[c].id,
                stamp: candidates[c].stamp,
                kind: candidates[c].kind,
            })
            .collect();
        anchors.sort_unstable_by_key(|a| (a.stamp, a.id));
        Ok(Plan {
            snapshot,
            residual,
            anchors,
            points: points.len() as u32,
            covered_points: assigned.len() as u32,
            candidates: candidates.len() as u32,
            membership_tests: tests,
        })
    }

    /// Deterministic counters (Ordered: equal across widths and resumes,
    /// except `pinned_plans`); the telemetry below is not.
    pub fn report(&self) -> Value {
        let l = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let s = &self.stats;
        let mut whole = serde_json::Map::new();
        for (name, counter) in WHOLE_NAMES.iter().zip(&s.whole) {
            whole.insert((*name).to_owned(), json!(l(counter)));
        }
        json!({"mode":"union","session_scope":"this process session",
            "planned_jobs":l(&s.plans),"pinned_plans":l(&s.pinned),
            "residual_plans":l(&s.residual),"full_cover_plans":l(&s.full_cover),
            "whole_inspections":whole,"query_points":l(&s.points),
            "covered_points":l(&s.covered_points),"candidates":l(&s.candidates),
            "membership_tests":l(&s.membership_tests),"anchor_edges":l(&s.anchor_edges),
            "original_d_levels_of_planned":l(&s.original_levels),
            "residual_d_levels_of_planned":l(&s.residual_levels),
            "anchors_indexed":l(&s.appended),"merged_records_not_indexed":l(&s.unindexed),
            "point_cap":POINT_CAP,"test_budget":TEST_BUDGET,
            "snapshot_rule":if self.ordered_lag.is_some() {"ordered: id + 1 - lookahead"} else {"ready: published merge count"},
            "authority":"exact interval membership, confirmed per covered point by the native one-point DomainPowerSummary inclusion"})
    }

    pub fn telemetry(&self) -> Value {
        let l = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let s = &self.stats;
        let buckets = self.buckets.read().expect("g2 buckets");
        let (mut runs, mut tail, mut entries, mut merges, mut merged, mut micros) =
            (0usize, 0usize, 0usize, 0u64, 0u64, 0u64);
        for bucket in buckets.values() {
            let (r, t) = bucket.runs_and_tail();
            runs += r;
            tail += t;
            entries += bucket.len();
            merges += l(&bucket.merges);
            merged += l(&bucket.merged_entries);
            micros += l(&bucket.compaction_micros);
        }
        json!({"buckets":buckets.len(),"indexed_entries":entries,"runs":runs,"tail_entries":tail,
            "run_merges":merges,"run_merged_entries":merged,"compaction_seconds":micros as f64 * 1e-6,
            "hull_visits":l(&s.hull_visits),"entry_visits":l(&s.entry_visits),"tail_visits":l(&s.tail_visits),
            "plan_seconds":l(&s.plan_micros) as f64 * 1e-6,
            "entry_bytes":std::mem::size_of::<Entry<N>>(),
            "index":"per (Apply, owner) bucket: Morton-sorted runs with a hull tree (leaf 16, fan-out 16), geometric merges, append-only tail chunks of 1024"})
    }
}

/// Reuse the independent lattice verifier; this is not another cover solver.
/// Indexed scopes already contain exactly the low-D slice lent by partial
/// anchors. Compact encoding preserves their coordinates/rank/power bounds.
/// Missing or undecidable proof is handled before any residual event exists.
fn replayable_union_cover<'a, const N: usize>(
    q: &Domain<N>,
    residual: Option<(i64, i64)>,
    scopes: impl Iterator<Item = &'a CompactDomain<N>>,
    max_regions: u64,
) -> Option<bool> {
    let cell = |domain: &Domain<N>| Cell {
        owner: domain.owner.to_vec(),
        lower: domain.lower.clone(),
        upper: domain.upper.clone(),
        rank: domain.rank,
        powers: domain.powers,
    };
    let query = cell(q);
    let mut targets = Vec::new();
    if let Some((lo, hi)) = residual {
        let mut remainder = query.clone();
        remainder.powers = residual_powers(q.powers, lo, hi);
        targets.push(remainder);
    }
    targets.extend(scopes.map(|scope| cell(&scope.expand())));
    query.covered_by_union(&targets.iter().collect::<Vec<_>>(), max_regions)
}

/// Every lattice point of the query (at most `cap`), with its aggregates.
fn enumerate<const N: usize>(
    owner: &[bool; N],
    q: &Query<N>,
    cap: usize,
    out: &mut Vec<Point<N>>,
) -> Result<(), Whole> {
    let shape = q.shape;
    let finite = |v: i32| v != i32::MAX && v != i32::MIN;
    // Effective aggregate caps (D bounds may bound A or R through the other).
    let mut a_hi = i64::from(shape.a.1);
    let mut r_hi = i64::from(shape.r.1);
    if !finite(shape.a.1) && finite(shape.d.1) && finite(shape.r.1) {
        a_hi = i64::from(shape.d.1) + i64::from(shape.r.1);
    }
    if !finite(shape.r.1) && finite(shape.a.1) && finite(shape.d.0) {
        r_hi = i64::from(shape.a.1) - i64::from(shape.d.0);
    }
    let a_open = !finite(shape.a.1) && a_hi == i64::from(i32::MAX);
    let r_open = !finite(shape.r.1) && r_hi == i64::from(i32::MAX);
    let a_lo: i64 = (0..N)
        .filter(|&i| owner[i])
        .map(|i| i64::from(q.lower[i]) + 1)
        .sum();
    let r_lo: i64 = (0..N)
        .filter(|&i| !owner[i])
        .map(|i| i64::from(q.lower[i]))
        .sum();
    let mut caps = [0u16; N];
    for axis in 0..N {
        let mut high = if q.upper[axis] == u16::MAX {
            i64::MAX
        } else {
            i64::from(q.upper[axis])
        };
        let low = i64::from(q.lower[axis]);
        if owner[axis] && !a_open {
            high = high.min(a_hi - (a_lo - (low + 1)) - 1);
        }
        if !owner[axis] && !r_open {
            high = high.min(r_hi - (r_lo - low));
        }
        if high == i64::MAX || high >= i64::from(u16::MAX) {
            return Err(Whole::Unbounded);
        }
        if high < low {
            return Ok(());
        }
        caps[axis] = high as u16;
    }
    struct Walk<'a, const N: usize> {
        owner: &'a [bool; N],
        lower: [u16; N],
        caps: [u16; N],
        rest_a: [i64; N],
        rest_r: [i64; N],
        a_hi: i64,
        r_hi: i64,
        a_open: bool,
        r_open: bool,
        shape: Shape,
        x: [u16; N],
        cap: usize,
        out: &'a mut Vec<Point<N>>,
        overflow: bool,
    }
    impl<const N: usize> Walk<'_, N> {
        fn go(&mut self, axis: usize, a: i64, r: i64) {
            if axis == N {
                let d = a - r;
                if (!self.a_open && a > self.a_hi)
                    || (!self.r_open && r > self.r_hi)
                    || d < i64::from(self.shape.d.0)
                    || d > i64::from(self.shape.d.1)
                    || a > i64::from(self.shape.a.1)
                    || r > i64::from(self.shape.r.1)
                {
                    return;
                }
                if self.out.len() >= self.cap {
                    self.overflow = true;
                    return;
                }
                self.out.push(Point {
                    x: self.x,
                    a: a as i32,
                    r: r as i32,
                    d: d as i32,
                });
                return;
            }
            for v in self.lower[axis]..=self.caps[axis] {
                let (na, nr) = if self.owner[axis] {
                    (a + i64::from(v) + 1, r)
                } else {
                    (a, r + i64::from(v))
                };
                if self.owner[axis] && !self.a_open && na + self.rest_a[axis] > self.a_hi {
                    break;
                }
                if !self.owner[axis] && !self.r_open && nr + self.rest_r[axis] > self.r_hi {
                    break;
                }
                self.x[axis] = v;
                self.go(axis + 1, na, nr);
                if self.overflow {
                    return;
                }
            }
        }
    }
    // Minimal contributions of the axes after each position.
    let mut rest_a = [0i64; N];
    let mut rest_r = [0i64; N];
    let (mut sa, mut sr) = (0i64, 0i64);
    for axis in (0..N).rev() {
        rest_a[axis] = sa;
        rest_r[axis] = sr;
        if owner[axis] {
            sa += i64::from(q.lower[axis]) + 1;
        } else {
            sr += i64::from(q.lower[axis]);
        }
    }
    let mut walk = Walk {
        owner,
        lower: q.lower,
        caps,
        rest_a,
        rest_r,
        a_hi,
        r_hi,
        a_open,
        r_open,
        shape,
        x: [0; N],
        cap,
        out,
        overflow: false,
    };
    walk.go(0, 0, 0);
    if walk.overflow {
        return Err(Whole::TooLarge);
    }
    Ok(())
}
