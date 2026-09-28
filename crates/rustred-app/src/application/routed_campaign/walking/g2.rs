//! W0 G2' falsifier (THROWAWAY, branch `fable_5_1-v3-g2falsify`, never merged
//! into the production engine): D-only residual inspection of a newly
//! dispatched Apply domain Q against Native Apply anchors of the same
//! (phase, owner) bucket that were committed strictly before Q's dispatch.
//!
//! Selected by `RUSTRED_WALK_G2_DONLY=1` (one anchor), `=2` (up to two
//! anchors), `=u` (pointwise union of anchors per D level, the plan's G2'
//! restricted to a one-piece D-only residual) or `=n` (mode u with anchors
//! restricted to full native inspections: G2' residual/full-cover records and
//! initial-overlap partials get a commit stamp but never become anchors, the
//! literal reading of S7 / master plan 3.11 "each anchor is Native");
//! unset/0/off leaves the engine byte-identical (no store, no dispatch
//! stamps, no record or report fields).
//!
//! Mode u: for a finite Q with at most `UNION_POINT_CAP` lattice points, the
//! worker enumerates Q's points (from Q's tight extrema), assigns each point
//! of the D levels from the top down to a committed anchor that contains it
//! (the native `DomainPowerSummary::contains` of the one-point domain is the
//! authority for every assignment), and cuts at the lowest D of the fully
//! covered top run: the job inspects Q restricted to D <= c-1 (nothing if all
//! levels are covered), with an edge to every anchor used. Infinite or larger
//! Q, or a search over budget, fall back to the mode-2 plan.
//!
//! Plan: for the smallest cut c1 such that an anchor A1 contains Q restricted
//! to D >= c1 (checked with the native `DomainPowerSummary::contains`; the
//! compact scan record is only a necessary prefilter), the job inspects only
//! Q restricted to D <= c1-1. In mode 2 the same search then runs on that
//! residual: A2 contains Q restricted to c2 <= D <= c1-1 and the job inspects
//! Q restricted to D <= c2-1. An empty residual is an alias-like cover with
//! zero native work. The record keeps Q's identity and coordinates, reuses the
//! partial-record semantics of `initial_overlap.rs`
//! (`partial_initial_overlap_inspection`, anchor edges Q -> A1 [-> A2] in the
//! ledger and the descendant closure), and carries a `g2_residual_anchor`
//! block with the commit stamps for the audit.
//!
//! Soundness: anchors are committed Native records (full or partial; full
//! only in mode n) whose
//! commit stamp is below Q's dispatch stamp; they are never pending, aliased
//! or in flight, so anchor links are strictly ordered in commit time (no
//! mutual subtraction). Frontiers/errors of an anchor block Q through the edge.
//! Scope: pool runs (workers > 1) without Apply subdivision; initial-prefix
//! domains are never planned; checkpoints written with the flag on are not
//! resumable (the relaxed ledger rules are not persisted).
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Instant;

use rustred::solver::{DomainPowerBounds, DomainPowerExtrema, DomainPowerSummary};
use serde_json::{Value, json};

use super::initial_overlap::InitialOverlapScope;
use super::queue::{Domain, Phase};

/// Owner classes reported separately (the two hot owners of the five-loop
/// controls and gen-7), then everything else.
const HOT_OWNERS: [&str; 2] = ["000011001001011", "011101110111000"];
const CLASS_NAMES: [&str; 3] = ["000011001001011", "011101110111000", "other"];
const CLASSES: usize = 3;
const CHUNK: usize = 1024;
/// Cut values memoized per plan (D ranges of admitted domains are small).
const MEMO_LIMIT: i128 = 4096;

/// Mode u: largest Q (lattice points) enumerated for the union plan.
const UNION_POINT_CAP: usize = 1 << 18;
/// Mode u: point-anchor membership tests allowed per plan beyond 64 per point.
const UNION_TEST_BUDGET: u64 = 1 << 22;

/// 0 off, 1 one anchor, 2 up to two anchors, 3 union (env value `u`),
/// 4 union with full-native anchors only (env value `n`).
pub(super) fn mode() -> u8 {
    static MODE: OnceLock<u8> = OnceLock::new();
    *MODE.get_or_init(|| {
        let mode = match std::env::var("RUSTRED_WALK_G2_DONLY").as_deref() {
            Err(_) | Ok("") | Ok("0") | Ok("off") => 0,
            Ok("1") => 1,
            Ok("2") => 2,
            Ok("u") => 3,
            Ok("n") => 4,
            Ok(other) => panic!("RUSTRED_WALK_G2_DONLY={other}: expected 1, 2, u, n or unset"),
        };
        if mode == 4 {
            eprintln!(
                "W0 G2' D-only residual-anchor falsifier ACTIVE (union of committed full-native anchors per D level)"
            );
        } else if mode == 3 {
            eprintln!(
                "W0 G2' D-only residual-anchor falsifier ACTIVE (union of committed Native anchors per D level)"
            );
        } else if mode != 0 {
            eprintln!(
                "W0 G2' D-only residual-anchor falsifier ACTIVE (up to {mode} committed Native anchor(s))"
            );
        }
        mode
    })
}

pub(super) fn enabled() -> bool {
    mode() != 0
}

/// Modes u and n plan unions.
fn union_mode() -> bool {
    mode() >= 3
}

/// Mode n: only full native inspections become anchors.
pub(super) fn native_anchors_only() -> bool {
    mode() == 4
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SecondAnchor {
    pub id: usize,
    pub seq: u64,
}

/// Commit/dispatch stamps and the plan outcome, copied into the record.
/// The scope's anchor covers D >= first_cut; the optional second anchor
/// covers scope.cut <= D <= first_cut-1; the residual is D <= scope.cut-1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct G2Info {
    pub anchor_commit_seq: u64,
    pub dispatch_snapshot: u64,
    pub full_cover: bool,
    pub anchors_scanned: u64,
    pub plan_micros: u64,
    pub original_d_levels: u64,
    pub residual_d_levels: u64,
    pub first_cut: i64,
    pub second: Option<SecondAnchor>,
    /// Mode u: number of anchors in the union list (first = scope anchor);
    /// the list itself is kept in the store until the commit takes it.
    pub union_count: u32,
    /// Mode u: points of Q's covered D levels checked natively.
    pub union_points: u64,
}

fn mode_name(union: bool) -> &'static str {
    if union && native_anchors_only() {
        "d_only_union_of_committed_full_native_anchors"
    } else if native_anchors_only() {
        "d_only_band_fallback_up_to_two_committed_full_native_anchors"
    } else if union {
        "d_only_union_of_committed_native_anchors"
    } else if mode() >= 2 {
        "d_only_up_to_two_committed_native_anchors"
    } else {
        "d_only_single_committed_native_anchor"
    }
}

impl G2Info {
    pub fn json(&self) -> Value {
        json!({"mode":mode_name(self.union_count > 0),
            "union_anchor_count":self.union_count,
            "union_points_checked":self.union_points,
            "anchor_commit_seq":self.anchor_commit_seq,
            "dispatch_snapshot":self.dispatch_snapshot,
            "first_cut":self.first_cut,
            "second_anchor":self.second.map(|s| json!({"anchor_id":s.id,"anchor_commit_seq":s.seq,
                "covered_slice":"original_intersect_cut_le_D_le_first_cut_minus_1"})),
            "full_cover":self.full_cover,
            "residual_pieces":u8::from(!self.full_cover),
            "original_d_levels":self.original_d_levels,
            "residual_d_levels":self.residual_d_levels,
            "anchors_scanned":self.anchors_scanned,
            "plan_micros":self.plan_micros,
            "authority":if self.union_count > 0 {"native_summary_contains_of_each_point_of_the_covered_D_levels_by_one_listed_anchor; anchors committed before dispatch"} else {"native_summary_contains_of_each_D_slice; anchors committed before dispatch"}})
    }
}

pub(super) struct Plan<const N: usize> {
    pub scope: InitialOverlapScope,
    /// None: the anchors contain Q (empty residual, no native call).
    pub residual: Option<Domain<N>>,
}

/// Monotone saturating image of the tight extrema: a necessary prefilter for
/// `DomainPowerSummary::contains` (monotone maps preserve every comparison).
#[derive(Clone, Copy)]
struct Compact<const N: usize> {
    lower: [u16; N],
    upper: [u16; N],
    positive: (u32, u32),
    numerator: (u32, u32),
    difference: (i32, i32),
}

fn sat16(x: u64) -> u16 {
    x.min(u64::from(u16::MAX)) as u16
}
fn sat32(x: u128) -> u32 {
    x.min(u128::from(u32::MAX)) as u32
}
fn sati32(x: i128) -> i32 {
    x.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
}

impl<const N: usize> Compact<N> {
    fn of(e: &DomainPowerExtrema<N>) -> Self {
        let (pl, pu) = e.positive_power();
        let (nl, nu) = e.numerator_rank();
        let (dl, du) = e.power_difference();
        Self {
            lower: std::array::from_fn(|i| sat16(e.lower()[i])),
            upper: std::array::from_fn(|i| e.upper()[i].map_or(u16::MAX, sat16)),
            positive: (sat32(pl), pu.map_or(u32::MAX, sat32)),
            numerator: (sat32(nl), nu.map_or(u32::MAX, sat32)),
            difference: (dl.map_or(i32::MIN, sati32), du.map_or(i32::MAX, sati32)),
        }
    }
    /// Necessary for a nonempty intersection (monotone images).
    #[inline]
    fn may_intersect(&self, o: &Self) -> bool {
        self.positive.0 <= o.positive.1
            && o.positive.0 <= self.positive.1
            && self.numerator.0 <= o.numerator.1
            && o.numerator.0 <= self.numerator.1
            && self.difference.0 <= o.difference.1
            && o.difference.0 <= self.difference.1
            && self.lower.iter().zip(&o.upper).all(|(a, b)| a <= b)
            && o.lower.iter().zip(&self.upper).all(|(a, b)| a <= b)
    }
    #[inline]
    fn may_contain(&self, o: &Self) -> bool {
        self.positive.0 <= o.positive.0
            && self.positive.1 >= o.positive.1
            && self.numerator.0 <= o.numerator.0
            && self.numerator.1 >= o.numerator.1
            && self.difference.0 <= o.difference.0
            && self.difference.1 >= o.difference.1
            && self.lower.iter().zip(&o.lower).all(|(a, b)| a <= b)
            && self.upper.iter().zip(&o.upper).all(|(a, b)| a >= b)
    }
}

struct Anchor<const N: usize> {
    id: usize,
    seq: u64,
    summary: DomainPowerSummary<N>,
    dlo: Option<i128>,
    dhi_unbounded: bool,
}

struct Chunk<const N: usize> {
    /// Scan records (commit stamp + compact extrema), contiguous for the scan.
    scan: Box<[OnceLock<(u64, Compact<N>)>]>,
    full: Box<[OnceLock<Anchor<N>>]>,
    len: AtomicUsize,
}

impl<const N: usize> Chunk<N> {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            scan: (0..CHUNK).map(|_| OnceLock::new()).collect(),
            full: (0..CHUNK).map(|_| OnceLock::new()).collect(),
            len: AtomicUsize::new(0),
        })
    }
}

/// Append-only per-bucket anchor list: one writer (the coordinator at
/// commit), lock-free readers apart from a brief chunk-list clone.
#[derive(Default)]
struct Bucket<const N: usize> {
    chunks: RwLock<Vec<Arc<Chunk<N>>>>,
}

#[derive(Default)]
struct ClassStats {
    dispatch_stamps: AtomicU64,
    plans: AtomicU64,
    no_snapshot: AtomicU64,
    no_anchor: AtomicU64,
    partial: AtomicU64,
    full_cover: AtomicU64,
    second_anchor: AtomicU64,
    union_plans: AtomicU64,
    union_full: AtomicU64,
    union_fallback: AtomicU64,
    union_over_budget: AtomicU64,
    union_candidates: AtomicU64,
    union_points: AtomicU64,
    union_tests: AtomicU64,
    union_anchor_edges: AtomicU64,
    anchors_scanned: AtomicU64,
    prefilter_pass: AtomicU64,
    top_filter_pass: AtomicU64,
    slab_summaries: AtomicU64,
    plan_micros: AtomicU64,
    residual_d_levels: AtomicU64,
    original_d_levels: AtomicU64,
    commits: AtomicU64,
}

impl ClassStats {
    fn json(&self) -> Value {
        let l = |a: &AtomicU64| a.load(Ordering::Relaxed);
        json!({"dispatch_stamps":l(&self.dispatch_stamps),"plans":l(&self.plans),
            "no_dispatch_snapshot":l(&self.no_snapshot),"no_anchor":l(&self.no_anchor),
            "partial_residual":l(&self.partial),"full_cover":l(&self.full_cover),
            "plans_with_second_anchor":l(&self.second_anchor),
            "union_cut_plans":l(&self.union_plans),"union_full_cover":l(&self.union_full),
            "union_fallback_to_band_search":l(&self.union_fallback),
            "union_over_budget":l(&self.union_over_budget),
            "union_candidates":l(&self.union_candidates),"union_points_checked":l(&self.union_points),
            "union_membership_tests":l(&self.union_tests),"union_anchor_edges":l(&self.union_anchor_edges),
            "anchors_scanned":l(&self.anchors_scanned),"compact_prefilter_pass":l(&self.prefilter_pass),
            "top_slice_filter_pass":l(&self.top_filter_pass),
            "slab_summaries":l(&self.slab_summaries),"plan_seconds":l(&self.plan_micros) as f64 * 1e-6,
            "original_d_levels_of_planned":l(&self.original_d_levels),
            "residual_d_levels_of_planned":l(&self.residual_d_levels),
            "committed_apply_records":l(&self.commits)})
    }
}

pub(super) struct Store<const N: usize> {
    buckets: RwLock<HashMap<[bool; N], Arc<Bucket<N>>>>,
    /// Commit stamps: the number of Apply records committed without error so
    /// far (coordinator only). An anchor's seq is the stamp before its commit.
    committed: AtomicU64,
    dispatch: Mutex<HashMap<usize, u64>>,
    /// Mode u: the union anchor list of each planned job (first = scope anchor).
    unions: Mutex<HashMap<usize, Vec<SecondAnchor>>>,
    appended: AtomicU64,
    unusable: AtomicU64,
    /// Mode n: partial records stamped but not appended as anchors.
    stamped_only: AtomicU64,
    classes: [ClassStats; CLASSES],
}

fn class<const N: usize>(owner: &[bool; N]) -> usize {
    let mask = super::mask(owner);
    HOT_OWNERS.iter().position(|h| *h == mask).unwrap_or(2)
}

fn summary<const N: usize>(
    domain: &Domain<N>,
    powers: DomainPowerBounds,
) -> Option<DomainPowerSummary<N>> {
    if let (Some(a), Some(b)) = (powers.min_power_difference, powers.max_power_difference)
        && a > b
    {
        return None;
    }
    DomainPowerSummary::try_new(
        domain.owner,
        &domain.lower,
        &domain.upper,
        domain.rank,
        powers,
    )
    .ok()
}

/// Q restricted to D >= cut (raw bounds; the summary makes them tight).
fn high_powers(powers: DomainPowerBounds, cut: i64) -> DomainPowerBounds {
    let mut high = powers;
    high.min_power_difference = Some(high.min_power_difference.map_or(cut, |v| v.max(cut)));
    high
}

/// Q restricted to D <= cut - 1.
fn low_powers(powers: DomainPowerBounds, cut: i64) -> Option<DomainPowerBounds> {
    let below = cut.checked_sub(1)?;
    let mut low = powers;
    low.max_power_difference = Some(low.max_power_difference.map_or(below, |v| v.min(below)));
    Some(low)
}

fn with_powers<const N: usize>(q: &Domain<N>, powers: DomainPowerBounds) -> Option<Domain<N>> {
    let mut lower = Vec::new();
    let mut upper = Vec::new();
    if lower.try_reserve_exact(q.lower.len()).is_err()
        || upper.try_reserve_exact(q.upper.len()).is_err()
    {
        return None;
    }
    lower.extend_from_slice(&q.lower);
    upper.extend_from_slice(&q.upper);
    Some(Domain {
        phase: q.phase,
        owner: q.owner,
        lower,
        upper,
        rank: q.rank,
        powers,
    })
}


/// (A, R) of a point: A = t + sum of active x, R = sum of inactive x.
fn aggregates<const N: usize>(owner: &[bool; N], x: &[u64; N]) -> (u128, u128) {
    let mut a = 0u128;
    let mut r = 0u128;
    for (active, &v) in owner.iter().zip(x) {
        if *active {
            a += u128::from(v) + 1;
        } else {
            r += u128::from(v);
        }
    }
    (a, r)
}

/// Point membership in a domain given its tight extrema (the domain equals
/// their conjunction); a fast search predicate, the native `contains` of the
/// one-point domain is checked separately for every assignment.
fn member<const N: usize>(
    e: &DomainPowerExtrema<N>,
    x: &[u64; N],
    a: u128,
    r: u128,
    d: i128,
) -> bool {
    let (pl, pu) = e.positive_power();
    let (nl, nu) = e.numerator_rank();
    let (dl, du) = e.power_difference();
    pl <= a
        && pu.is_none_or(|u| a <= u)
        && nl <= r
        && nu.is_none_or(|u| r <= u)
        && dl.is_none_or(|l| l <= d)
        && du.is_none_or(|u| d <= u)
        && e.lower().iter().zip(x).all(|(l, v)| l <= v)
        && e.upper().iter().zip(x).all(|(u, v)| u.is_none_or(|u| *v <= u))
}

/// Every lattice point of a finite domain from its tight extrema: coordinates
/// (N u16 per point) and (D, point index). False if a coordinate is
/// unbounded or above u16::MAX, or if there are more than `cap` points.
fn enumerate_points<const N: usize>(
    owner: &[bool; N],
    e: &DomainPowerExtrema<N>,
    cap: usize,
    coords: &mut Vec<u16>,
    levels: &mut Vec<(i128, u32)>,
) -> bool {
    struct Walk<'a, const N: usize> {
        owner: &'a [bool; N],
        lower: [u64; N],
        upper: [u64; N],
        rest_a: Vec<u128>,
        rest_r: Vec<u128>,
        a_bounds: (u128, Option<u128>),
        r_bounds: (u128, Option<u128>),
        d_bounds: (Option<i128>, Option<i128>),
        x: [u64; N],
        cap: usize,
        coords: &'a mut Vec<u16>,
        levels: &'a mut Vec<(i128, u32)>,
        overflow: bool,
    }
    impl<const N: usize> Walk<'_, N> {
        fn go(&mut self, axis: usize, a: u128, r: u128) {
            if axis == N {
                let (pl, pu) = self.a_bounds;
                let (nl, nu) = self.r_bounds;
                let (dl, du) = self.d_bounds;
                let d = a as i128 - r as i128;
                if a < pl
                    || pu.is_some_and(|u| a > u)
                    || r < nl
                    || nu.is_some_and(|u| r > u)
                    || dl.is_some_and(|l| d < l)
                    || du.is_some_and(|u| d > u)
                {
                    return;
                }
                if self.levels.len() >= self.cap {
                    self.overflow = true;
                    return;
                }
                let index = self.levels.len() as u32;
                self.coords.extend(self.x.iter().map(|&v| v as u16));
                self.levels.push((d, index));
                return;
            }
            for v in self.lower[axis]..=self.upper[axis] {
                let (na, nr) = if self.owner[axis] {
                    (a + u128::from(v) + 1, r)
                } else {
                    (a, r + u128::from(v))
                };
                if self.owner[axis] && self.a_bounds.1.is_some_and(|u| na + self.rest_a[axis + 1] > u) {
                    break;
                }
                if !self.owner[axis] && self.r_bounds.1.is_some_and(|u| nr + self.rest_r[axis + 1] > u) {
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
    let mut upper = [0u64; N];
    for (axis, bound) in e.upper().iter().enumerate() {
        match bound {
            Some(u) if *u <= u64::from(u16::MAX) => upper[axis] = *u,
            _ => return false,
        }
    }
    let lower = *e.lower();
    // Minimal contributions of the axes after each position.
    let mut rest_a = vec![0u128; N + 1];
    let mut rest_r = vec![0u128; N + 1];
    for axis in (0..N).rev() {
        let (da, dr) = if owner[axis] {
            (u128::from(lower[axis]) + 1, 0)
        } else {
            (0, u128::from(lower[axis]))
        };
        rest_a[axis] = rest_a[axis + 1] + da;
        rest_r[axis] = rest_r[axis + 1] + dr;
    }
    let mut walk = Walk {
        owner,
        lower,
        upper,
        rest_a,
        rest_r,
        a_bounds: e.positive_power(),
        r_bounds: e.numerator_rank(),
        d_bounds: e.power_difference(),
        x: [0; N],
        cap,
        coords,
        levels,
        overflow: false,
    };
    walk.go(0, 0, 0);
    !walk.overflow
}

/// One search outcome: the anchor contains the whole query, or its
/// D >= cut slice for the smallest such cut.
enum Found<'a, const N: usize> {
    Full(&'a Anchor<N>),
    Cut(&'a Anchor<N>, i128),
}

impl<const N: usize> Store<N> {
    pub fn from_env() -> Option<Arc<Self>> {
        enabled().then(|| {
            Arc::new(Self {
                buckets: RwLock::new(HashMap::new()),
                committed: AtomicU64::new(0),
                dispatch: Mutex::new(HashMap::new()),
                unions: Mutex::new(HashMap::new()),
                appended: AtomicU64::new(0),
                unusable: AtomicU64::new(0),
                stamped_only: AtomicU64::new(0),
                classes: Default::default(),
            })
        })
    }

    /// Coordinator, immediately before the pool dispatch of an eligible
    /// Apply domain: stamp the committed-anchor count visible to its job.
    pub fn note_dispatch(&self, id: usize, owner: &[bool; N]) {
        let stamp = self.committed.load(Ordering::Acquire);
        self.dispatch
            .lock()
            .expect("g2 dispatch table")
            .insert(id, stamp);
        self.classes[class(owner)]
            .dispatch_stamps
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Coordinator at commit (or retention): the union anchor list of job
    /// `id` (empty unless its plan was a mode-u union plan).
    pub fn take_union(&self, id: usize) -> Vec<SecondAnchor> {
        self.unions
            .lock()
            .expect("g2 union table")
            .remove(&id)
            .unwrap_or_default()
    }

    /// Coordinator: the dispatch did not happen after all.
    pub fn forget_dispatch(&self, id: usize) {
        self.dispatch.lock().expect("g2 dispatch table").remove(&id);
    }

    /// Coordinator, at the end of a committed (published) Apply record without
    /// error. Returns this record's commit stamp. The record becomes visible
    /// as an anchor to jobs dispatched from now on, unless it is a partial
    /// record (`partial` true) in mode n, which is stamped only.
    pub fn commit(&self, id: usize, domain: &Domain<N>, partial: bool) -> u64 {
        let seq = self.committed.load(Ordering::Relaxed);
        self.classes[class(&domain.owner)]
            .commits
            .fetch_add(1, Ordering::Relaxed);
        if partial && native_anchors_only() {
            self.stamped_only.fetch_add(1, Ordering::Relaxed);
            self.committed.store(seq + 1, Ordering::Release);
            return seq;
        }
        let usable = summary(domain, domain.powers).and_then(|s| {
            let extrema = s.extrema()?;
            let (dlo, dhi) = extrema.power_difference();
            let compact = Compact::of(extrema);
            Some((
                compact,
                Anchor {
                    id,
                    seq,
                    dlo,
                    dhi_unbounded: dhi.is_none(),
                    summary: s,
                },
            ))
        });
        if let Some((compact, anchor)) = usable {
            self.append(domain.owner, compact, anchor);
            self.appended.fetch_add(1, Ordering::Relaxed);
        } else {
            self.unusable.fetch_add(1, Ordering::Relaxed);
        }
        // Publish the stamp only after the anchor is readable.
        self.committed.store(seq + 1, Ordering::Release);
        seq
    }

    fn append(&self, owner: [bool; N], compact: Compact<N>, anchor: Anchor<N>) {
        let bucket = {
            let existing = self
                .buckets
                .read()
                .expect("g2 buckets")
                .get(&owner)
                .cloned();
            match existing {
                Some(bucket) => bucket,
                None => self
                    .buckets
                    .write()
                    .expect("g2 buckets")
                    .entry(owner)
                    .or_default()
                    .clone(),
            }
        };
        let tail = bucket
            .chunks
            .read()
            .expect("g2 chunks")
            .last()
            .cloned()
            .filter(|c| c.len.load(Ordering::Relaxed) < CHUNK);
        let tail = match tail {
            Some(tail) => tail,
            None => {
                let chunk = Chunk::new();
                bucket
                    .chunks
                    .write()
                    .expect("g2 chunks")
                    .push(chunk.clone());
                chunk
            }
        };
        // Single writer: the slot at len is unset. Full record first.
        let slot = tail.len.load(Ordering::Relaxed);
        let seq = anchor.seq;
        if tail.full[slot].set(anchor).is_err() || tail.scan[slot].set((seq, compact)).is_err() {
            panic!("W0 G2': anchor slot written twice");
        }
        tail.len.store(slot + 1, Ordering::Release);
    }

    /// Worker, before the native call of job `id` on Q. None: inspect Q whole.
    pub fn plan(&self, id: usize, q: &Domain<N>, cancellation: &AtomicBool) -> Option<Plan<N>> {
        if q.phase != Phase::Apply {
            return None;
        }
        let started = Instant::now();
        let stats = &self.classes[class(&q.owner)];
        let Some(snapshot) = self
            .dispatch
            .lock()
            .expect("g2 dispatch table")
            .remove(&id)
        else {
            stats.no_snapshot.fetch_add(1, Ordering::Relaxed);
            return None;
        };
        stats.plans.fetch_add(1, Ordering::Relaxed);
        let result = if union_mode() {
            match self.plan_union(id, q, snapshot, cancellation, stats) {
                Ok(plan) => plan,
                Err(()) => {
                    stats.union_fallback.fetch_add(1, Ordering::Relaxed);
                    self.plan_at(q, snapshot, cancellation, stats)
                }
            }
        } else {
            self.plan_at(q, snapshot, cancellation, stats)
        };
        let micros = started.elapsed().as_micros() as u64;
        stats.plan_micros.fetch_add(micros, Ordering::Relaxed);
        let Some(mut plan) = result else {
            stats.no_anchor.fetch_add(1, Ordering::Relaxed);
            return None;
        };
        if let Some(info) = plan.scope.g2.as_mut() {
            info.plan_micros = micros;
            if info.full_cover {
                stats.full_cover.fetch_add(1, Ordering::Relaxed);
            } else {
                stats.partial.fetch_add(1, Ordering::Relaxed);
            }
            if info.second.is_some() {
                stats.second_anchor.fetch_add(1, Ordering::Relaxed);
            }
            if info.union_count > 0 {
                stats.union_plans.fetch_add(1, Ordering::Relaxed);
                stats
                    .union_full
                    .fetch_add(u64::from(info.full_cover), Ordering::Relaxed);
                stats
                    .union_anchor_edges
                    .fetch_add(u64::from(info.union_count), Ordering::Relaxed);
            }
            stats
                .original_d_levels
                .fetch_add(info.original_d_levels, Ordering::Relaxed);
            stats
                .residual_d_levels
                .fetch_add(info.residual_d_levels, Ordering::Relaxed);
        }
        Some(plan)
    }

    /// Mode u (module doc). Err(()): not applicable (no finite extrema, more
    /// than `UNION_POINT_CAP` points, over the test budget); the caller falls
    /// back to the band search. Ok(None): no covered top D level.
    fn plan_union(
        &self,
        id: usize,
        q: &Domain<N>,
        snapshot: u64,
        cancellation: &AtomicBool,
        stats: &ClassStats,
    ) -> Result<Option<Plan<N>>, ()> {
        let whole = summary(q, q.powers).ok_or(())?;
        let qe = whole.extrema().ok_or(())?;
        let (Some(qdlo), Some(qdhi)) = qe.power_difference() else {
            return Err(());
        };
        let qcut = i64::try_from(qdlo).map_err(|_| ())?;
        let mut coords: Vec<u16> = Vec::new();
        let mut levels: Vec<(i128, u32)> = Vec::new();
        if !enumerate_points(&q.owner, qe, UNION_POINT_CAP, &mut coords, &mut levels) {
            return Err(());
        }
        let Some(bucket) = self
            .buckets
            .read()
            .expect("g2 buckets")
            .get(&q.owner)
            .cloned()
        else {
            return Ok(None);
        };
        let chunks = bucket.chunks.read().expect("g2 chunks").clone();
        let target = Compact::of(qe);
        let mut candidates: Vec<&Anchor<N>> = Vec::new();
        let mut scanned = 0u64;
        'outer: for chunk in &chunks {
            let len = chunk.len.load(Ordering::Acquire);
            for (index, slot) in chunk.scan[..len].iter().enumerate() {
                let Some((seq, compact)) = slot.get() else {
                    break 'outer;
                };
                // Append order is commit order: nothing later is visible.
                if *seq >= snapshot {
                    break 'outer;
                }
                scanned += 1;
                if scanned % 4096 == 0 && cancellation.load(Ordering::Relaxed) {
                    break 'outer;
                }
                if !compact.may_intersect(&target) {
                    continue;
                }
                let Some(anchor) = chunk.full[index].get() else {
                    break 'outer;
                };
                candidates.push(anchor);
            }
        }
        stats.anchors_scanned.fetch_add(scanned, Ordering::Relaxed);
        stats
            .prefilter_pass
            .fetch_add(candidates.len() as u64, Ordering::Relaxed);
        stats
            .union_candidates
            .fetch_add(candidates.len() as u64, Ordering::Relaxed);
        if candidates.is_empty() {
            return Ok(None);
        }
        // D levels from the top down; each level is kept only if every one of
        // its points is assigned to an anchor.
        levels.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let point = |index: u32| -> [u64; N] {
            std::array::from_fn(|axis| u64::from(coords[index as usize * N + axis]))
        };
        let mut order: Vec<usize> = (0..candidates.len()).collect();
        let mut used: Vec<usize> = Vec::new();
        let mut used_flag = vec![false; candidates.len()];
        let budget = UNION_TEST_BUDGET + 64 * levels.len() as u64;
        let (mut tests, mut checked) = (0u64, 0u64);
        let mut covered_cut: Option<i128> = None;
        let mut start = 0usize;
        let mut assigned: Vec<(u32, usize)> = Vec::new();
        let outcome = 'levels: loop {
            if start >= levels.len() {
                break 'levels Ok(());
            }
            let d = levels[start].0;
            let end = start + levels[start..].iter().take_while(|l| l.0 == d).count();
            assigned.clear();
            for &(_, index) in &levels[start..end] {
                let x = point(index);
                let (a, r) = aggregates(&q.owner, &x);
                let mut hit = None;
                for (position, &candidate) in order.iter().enumerate() {
                    tests += 1;
                    if candidates[candidate]
                        .summary
                        .extrema()
                        .is_some_and(|e| member(e, &x, a, r, d))
                    {
                        hit = Some(position);
                        break;
                    }
                }
                if tests > budget {
                    stats.union_over_budget.fetch_add(1, Ordering::Relaxed);
                    break 'levels Err(());
                }
                let Some(position) = hit else {
                    // This level has an uncovered point: the cut stays above.
                    break 'levels Ok(());
                };
                let candidate = order[position];
                order[..=position].rotate_right(1);
                assigned.push((index, candidate));
            }
            // Authority: the native inclusion of every one-point domain in its
            // assigned anchor.
            for &(index, candidate) in &assigned {
                let x = point(index);
                let upper: [Option<u64>; N] = x.map(Some);
                let Ok(one) = DomainPowerSummary::try_new(
                    q.owner,
                    &x,
                    &upper,
                    None,
                    DomainPowerBounds::default(),
                ) else {
                    break 'levels Err(());
                };
                if one.is_empty() || !candidates[candidate].summary.contains(&one) {
                    break 'levels Err(());
                }
                if !used_flag[candidate] {
                    used_flag[candidate] = true;
                    used.push(candidate);
                }
                checked += 1;
            }
            covered_cut = Some(d);
            start = end;
        };
        stats.union_tests.fetch_add(tests, Ordering::Relaxed);
        stats.union_points.fetch_add(checked, Ordering::Relaxed);
        outcome?;
        let Some(cut) = covered_cut else {
            return Ok(None);
        };
        let full = start >= levels.len();
        let first = candidates[used[0]];
        let list: Vec<SecondAnchor> = used
            .iter()
            .map(|&c| SecondAnchor {
                id: candidates[c].id,
                seq: candidates[c].seq,
            })
            .collect();
        let count = u32::try_from(list.len()).map_err(|_| ())?;
        let original = (qdhi - qdlo + 1).max(0) as u64;
        let (scope_cut, residual) = if full {
            (qcut, None)
        } else {
            let c = i64::try_from(cut).map_err(|_| ())?;
            let low = low_powers(q.powers, c).ok_or(())?;
            (c, Some(with_powers(q, low).ok_or(())?))
        };
        let info = G2Info {
            anchor_commit_seq: first.seq,
            dispatch_snapshot: snapshot,
            full_cover: full,
            anchors_scanned: scanned,
            plan_micros: 0,
            original_d_levels: original,
            residual_d_levels: if full { 0 } else { (cut - qdlo).max(0) as u64 },
            first_cut: scope_cut,
            second: None,
            union_count: count,
            union_points: checked,
        };
        self.unions
            .lock()
            .expect("g2 union table")
            .insert(id, list);
        Ok(Some(Plan {
            scope: InitialOverlapScope {
                anchor_id: first.id,
                cut: scope_cut,
                residual_powers: low_powers(q.powers, scope_cut).ok_or(())?,
                g2: Some(info),
            },
            residual,
        }))
    }

    fn plan_at(
        &self,
        q: &Domain<N>,
        snapshot: u64,
        cancellation: &AtomicBool,
        stats: &ClassStats,
    ) -> Option<Plan<N>> {
        let whole = summary(q, q.powers)?;
        let (qdlo, qdhi) = whole.extrema()?.power_difference();
        // A finite lower D is needed to express the residual cut.
        let qdlo = qdlo?;
        let bucket = self
            .buckets
            .read()
            .expect("g2 buckets")
            .get(&q.owner)
            .cloned()?;
        let chunks = bucket.chunks.read().expect("g2 chunks").clone();
        let mut scanned = 0u64;
        let first = self.search(q, &chunks, snapshot, None, cancellation, stats, &mut scanned);
        stats.anchors_scanned.fetch_add(scanned, Ordering::Relaxed);
        let levels = |lo: i128, hi: Option<i128>| hi.map_or(0, |h| (h - lo + 1).max(0) as u64);
        let info = |anchor: &Anchor<N>,
                    first_cut: i64,
                    second: Option<SecondAnchor>,
                    full: bool,
                    residual_levels: u64| G2Info {
            anchor_commit_seq: anchor.seq,
            dispatch_snapshot: snapshot,
            full_cover: full,
            anchors_scanned: 0,
            plan_micros: 0,
            original_d_levels: levels(qdlo, qdhi),
            residual_d_levels: residual_levels,
            first_cut,
            second,
            union_count: 0,
            union_points: 0,
        };
        let qcut = i64::try_from(qdlo).ok()?;
        let (a1, c1) = match first? {
            Found::Full(anchor) => {
                // Authority: exact native inclusion of the whole of Q.
                if !anchor.summary.contains(&whole) {
                    return None;
                }
                let mut g2 = info(anchor, qcut, None, true, 0);
                g2.anchors_scanned = scanned;
                return Some(Plan {
                    scope: InitialOverlapScope {
                        anchor_id: anchor.id,
                        cut: qcut,
                        residual_powers: low_powers(q.powers, qcut)?,
                        g2: Some(g2),
                    },
                    residual: None,
                });
            }
            Found::Cut(anchor, cut) => (anchor, i64::try_from(cut).ok()?),
        };
        // Authority: exact native inclusion of the D >= c1 slice; the residual
        // D <= c1-1 must be nonempty (else Q itself is contained).
        let high = summary(q, high_powers(q.powers, c1))?;
        let low1 = low_powers(q.powers, c1)?;
        let low1_summary = summary(q, low1)?;
        if high.is_empty() || low1_summary.is_empty() || !a1.summary.contains(&high) {
            return None;
        }
        let residual1 = with_powers(q, low1)?;
        if mode() >= 2 {
            let mut scanned2 = 0u64;
            let second = self.search(
                &residual1,
                &chunks,
                snapshot,
                Some(a1.id),
                cancellation,
                stats,
                &mut scanned2,
            );
            stats.anchors_scanned.fetch_add(scanned2, Ordering::Relaxed);
            scanned += scanned2;
            match second {
                Some(Found::Full(a2)) if a2.summary.contains(&low1_summary) => {
                    let mut g2 = info(a1, c1, Some(SecondAnchor { id: a2.id, seq: a2.seq }), true, 0);
                    g2.anchors_scanned = scanned;
                    return Some(Plan {
                        scope: InitialOverlapScope {
                            anchor_id: a1.id,
                            cut: qcut,
                            residual_powers: low_powers(q.powers, qcut)?,
                            g2: Some(g2),
                        },
                        residual: None,
                    });
                }
                Some(Found::Cut(a2, cut)) => {
                    let c2 = i64::try_from(cut).ok()?;
                    let band = summary(&residual1, high_powers(low1, c2))?;
                    let low2 = low_powers(q.powers, c2)?;
                    let low2_summary = summary(q, low2)?;
                    if !band.is_empty() && !low2_summary.is_empty() && a2.summary.contains(&band) {
                        let rdhi = low2_summary.extrema().and_then(|e| e.power_difference().1);
                        let mut g2 = info(
                            a1,
                            c1,
                            Some(SecondAnchor { id: a2.id, seq: a2.seq }),
                            false,
                            levels(qdlo, rdhi),
                        );
                        g2.anchors_scanned = scanned;
                        return Some(Plan {
                            scope: InitialOverlapScope {
                                anchor_id: a1.id,
                                cut: c2,
                                residual_powers: low2,
                                g2: Some(g2),
                            },
                            residual: Some(with_powers(q, low2)?),
                        });
                    }
                }
                _ => {}
            }
        }
        let rdhi = low1_summary.extrema().and_then(|e| e.power_difference().1);
        let mut g2 = info(a1, c1, None, false, levels(qdlo, rdhi));
        g2.anchors_scanned = scanned;
        Some(Plan {
            scope: InitialOverlapScope {
                anchor_id: a1.id,
                cut: c1,
                residual_powers: low1,
                g2: Some(g2),
            },
            residual: Some(residual1),
        })
    }

    /// Best anchor for query q among the committed anchors below `snapshot`:
    /// the oldest one containing q, else the smallest D cut (oldest on ties).
    #[allow(clippy::too_many_arguments)]
    fn search<'a>(
        &self,
        q: &Domain<N>,
        chunks: &'a [Arc<Chunk<N>>],
        snapshot: u64,
        exclude: Option<usize>,
        cancellation: &AtomicBool,
        stats: &ClassStats,
        scanned: &mut u64,
    ) -> Option<Found<'a, N>> {
        let whole = summary(q, q.powers)?;
        let (qdlo, qdhi) = whole.extrema()?.power_difference();
        let qdlo = qdlo?;
        let mut memo: Vec<Option<Option<DomainPowerSummary<N>>>> = match qdhi {
            Some(h) if h - qdlo < MEMO_LIMIT => vec![None; (h - qdlo + 1) as usize],
            _ => Vec::new(),
        };
        let mut slab = |cut: i128| -> Option<DomainPowerSummary<N>> {
            let index = cut - qdlo;
            if index >= 0
                && let Some(entry) = memo.get(index as usize)
                && let Some(done) = entry
            {
                return done.clone();
            }
            stats.slab_summaries.fetch_add(1, Ordering::Relaxed);
            let value = i64::try_from(cut)
                .ok()
                .and_then(|c| summary(q, high_powers(q.powers, c)));
            if index >= 0
                && let Some(entry) = memo.get_mut(index as usize)
            {
                *entry = Some(value.clone());
            }
            value
        };
        // Necessary for every slab: the top D level of q lies in each slab.
        let top = match qdhi {
            Some(h) => Some(slab(h)?),
            None => None,
        };
        // Prefilter target: the top slice (contained in every slab and in q).
        // Unbounded D above: no common slice, no prefilter.
        let target = match &top {
            Some(top) => Some(Compact::of(top.extrema()?)),
            None => None,
        };
        let mut best: Option<(&Anchor<N>, i128)> = None;
        'outer: for chunk in chunks {
            let len = chunk.len.load(Ordering::Acquire);
            for (index, slot) in chunk.scan[..len].iter().enumerate() {
                let Some((seq, compact)) = slot.get() else {
                    break 'outer;
                };
                // Append order is commit order: nothing later is visible.
                if *seq >= snapshot {
                    break 'outer;
                }
                *scanned += 1;
                if *scanned % 4096 == 0 && cancellation.load(Ordering::Relaxed) {
                    break 'outer;
                }
                if target.as_ref().is_some_and(|t| !compact.may_contain(t)) {
                    continue;
                }
                stats.prefilter_pass.fetch_add(1, Ordering::Relaxed);
                let Some(anchor) = chunk.full[index].get() else {
                    break 'outer;
                };
                if exclude == Some(anchor.id) {
                    continue;
                }
                if anchor.summary.contains(&whole) {
                    return Some(Found::Full(anchor));
                }
                match &top {
                    Some(top) => {
                        if !anchor.summary.contains(top) {
                            continue;
                        }
                    }
                    None => {
                        if !anchor.dhi_unbounded {
                            continue;
                        }
                    }
                }
                stats.top_filter_pass.fetch_add(1, Ordering::Relaxed);
                // q itself is not contained, so a useful cut is above qdlo.
                let lo = anchor.dlo.map_or(qdlo + 1, |d| d.max(qdlo + 1));
                let mut hi = match qdhi {
                    Some(h) => h,
                    None => lo,
                };
                if let Some((_, cut)) = best {
                    hi = hi.min(cut - 1);
                }
                if lo > hi {
                    continue;
                }
                let contains =
                    |s: Option<DomainPowerSummary<N>>| s.is_some_and(|s| anchor.summary.contains(&s));
                if !contains(slab(hi)) {
                    continue;
                }
                // Smallest cut in [lo, hi]; containment is monotone in the cut.
                let (mut a, mut b) = (lo, hi);
                while a < b {
                    let mid = a + (b - a) / 2;
                    if contains(slab(mid)) {
                        b = mid;
                    } else {
                        a = mid + 1;
                    }
                }
                best = Some((anchor, b));
            }
        }
        best.map(|(anchor, cut)| Found::Cut(anchor, cut))
    }

    pub fn report(&self) -> Value {
        let mut classes = serde_json::Map::new();
        for (name, stats) in CLASS_NAMES.iter().zip(&self.classes) {
            classes.insert((*name).to_owned(), stats.json());
        }
        json!({"mode":match mode() {
                4 => "d_only_union_of_committed_full_native_anchors_with_band_fallback",
                3 => "d_only_union_of_committed_native_anchors_with_band_fallback",
                _ => mode_name(false)},
            "env":format!("RUSTRED_WALK_G2_DONLY={}", match mode() {
                4 => "n".to_owned(), 3 => "u".to_owned(), m => m.to_string()}),
            "anchor_eligibility":if native_anchors_only() {"full native inspections only (no G2' or initial-overlap partial records)"} else {"every Apply record committed without error"},
            "stamped_not_anchored":self.stamped_only.load(Ordering::Relaxed),
            "union_point_cap":UNION_POINT_CAP,"union_test_budget":UNION_TEST_BUDGET,
            "scan":"per-bucket append-only commit-ordered list; compact saturating extrema prefilter, then native contains",
            "committed_apply_records":self.committed.load(Ordering::Relaxed),
            "anchors_appended":self.appended.load(Ordering::Relaxed),
            "anchors_unusable_summary":self.unusable.load(Ordering::Relaxed),
            "dispatch_stamps_outstanding":self.dispatch.lock().map_or(0, |d| d.len()),
            "by_owner_class":classes,
            "scope":"process counters of the throwaway W0 G2' falsifier; plans of cancelled jobs included"})
    }
}
