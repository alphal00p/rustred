//! Indexed search for G2' anchors: per (Apply, owner) bucket, every merged
//! anchor-eligible record with its merge stamp and the exact scope it lends.
//!
//! A query returns every entry with `stamp < snapshot` whose interval shape
//! meets the query's (a necessary condition for a common lattice point; the
//! planner decides membership exactly). There is no linear scan of the
//! bucket: entries live in immutable sorted runs (Morton order of the lower
//! corner, leaves of `LEAF` entries, a hull tree of fan-out `FAN` over them)
//! plus a short append-only tail of at most `CHUNK` entries per chunk. Full
//! tail chunks are sealed into runs and runs are merged geometrically by
//! whichever planning worker first finds work pending (`try_lock`), so the
//! coordinator only appends. Readers take an `Arc` snapshot of the view; a
//! view always holds every appended entry exactly once (in a run or a chunk),
//! so a query sees every entry appended before its snapshot was published.
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use rustred::solver::DomainPowerBounds;

use super::super::queue::CompactDomain;

/// Tail chunk capacity (entries scanned linearly by a query).
pub(super) const CHUNK: usize = 1024;
/// Entries per leaf hull.
const LEAF: usize = 16;
/// Children per inner hull.
const FAN: usize = 16;

/// Interval-tight aggregate bounds of a domain. The box, `A <= a.1`,
/// `R <= r.1` and `d.0 <= D <= d.1` define exactly its lattice points (the
/// lower aggregate bounds are implied by the box). Values are saturated to
/// i32; every enumerable point's aggregates are far inside that range, so
/// saturation only ever stands for an infinite bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct Shape {
    pub a: (i32, i32),
    pub r: (i32, i32),
    pub d: (i32, i32),
}

fn sat(value: i128) -> i32 {
    value.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
}

impl Shape {
    /// None: the domain has no lattice point.
    pub fn of<const N: usize>(
        owner: &[bool; N],
        lower: &[u16; N],
        upper: &[u16; N],
        rank: Option<u32>,
        powers: DomainPowerBounds,
    ) -> Option<Self> {
        let (mut a_lo, mut r_lo) = (0i128, 0i128);
        let (mut a_box, mut r_box) = (Some(0i128), Some(0i128));
        for axis in 0..N {
            if upper[axis] < lower[axis] {
                return None;
            }
            let high = (upper[axis] != u16::MAX).then_some(i128::from(upper[axis]));
            if owner[axis] {
                a_lo += i128::from(lower[axis]) + 1;
                a_box = a_box.zip(high).map(|(s, h)| s + h + 1);
            } else {
                r_lo += i128::from(lower[axis]);
                r_box = r_box.zip(high).map(|(s, h)| s + h);
            }
        }
        let min = |x: Option<i128>, y: Option<i128>| match (x, y) {
            (Some(x), Some(y)) => Some(x.min(y)),
            (x, y) => x.or(y),
        };
        let a_hi = min(a_box, powers.max_positive_power.map(i128::from));
        let r_hi = min(r_box, rank.map(i128::from));
        if a_hi.is_some_and(|h| h < a_lo) || r_hi.is_some_and(|h| h < r_lo) {
            return None;
        }
        let d_lo = match (r_hi.map(|r| a_lo - r), powers.min_power_difference) {
            (Some(x), Some(y)) => Some(x.max(i128::from(y))),
            (x, y) => x.or(y.map(i128::from)),
        };
        let d_hi = min(
            a_hi.map(|a| a - r_lo),
            powers.max_power_difference.map(i128::from),
        );
        if let (Some(lo), Some(hi)) = (d_lo, d_hi)
            && lo > hi
        {
            return None;
        }
        Some(Self {
            a: (sat(a_lo), a_hi.map_or(i32::MAX, sat)),
            r: (sat(r_lo), r_hi.map_or(i32::MAX, sat)),
            d: (d_lo.map_or(i32::MIN, sat), d_hi.map_or(i32::MAX, sat)),
        })
    }

    #[inline]
    fn meets_aggregates(&self, other: &Self) -> bool {
        self.a.0 <= other.a.1
            && other.a.0 <= self.a.1
            && self.r.0 <= other.r.1
            && other.r.0 <= self.r.1
            && self.d.0 <= other.d.1
            && other.d.0 <= self.d.1
    }
}

/// One lattice point of a query with its aggregates.
#[derive(Clone, Copy, Debug)]
pub(in super::super) struct Point<const N: usize> {
    pub x: [u16; N],
    pub a: i32,
    pub r: i32,
    pub d: i32,
}

/// The box-and-shape image of a query domain.
#[derive(Clone, Copy, Debug)]
pub(in super::super) struct Query<const N: usize> {
    pub lower: [u16; N],
    pub upper: [u16; N],
    pub shape: Shape,
}

#[inline]
fn boxes_meet<const N: usize>(
    lower: &[u16; N],
    upper: &[u16; N],
    q_lower: &[u16; N],
    q_upper: &[u16; N],
) -> bool {
    (0..N).all(|axis| lower[axis] <= q_upper[axis] && q_lower[axis] <= upper[axis])
}

/// One merged anchor-eligible record: its lent scope (exact compact image),
/// the scope's shape, merge stamp, ID and record kind.
#[derive(Clone, Copy, Debug)]
pub(in super::super) struct Entry<const N: usize> {
    pub scope: CompactDomain<N>,
    pub shape: Shape,
    pub stamp: u64,
    pub id: u32,
    pub kind: u8,
}

impl<const N: usize> Entry<N> {
    #[inline]
    pub fn meets(&self, q: &Query<N>) -> bool {
        let (lower, upper) = self.scope.raw_bounds();
        self.shape.meets_aggregates(&q.shape) && boxes_meet(lower, upper, &q.lower, &q.upper)
    }
    /// Exact membership of a lattice point in the lent scope.
    #[inline]
    pub fn contains(&self, p: &Point<N>) -> bool {
        let (lower, upper) = self.scope.raw_bounds();
        p.a <= self.shape.a.1
            && p.r <= self.shape.r.1
            && self.shape.d.0 <= p.d
            && p.d <= self.shape.d.1
            && (0..N).all(|axis| lower[axis] <= p.x[axis] && p.x[axis] <= upper[axis])
    }
    fn key(&self) -> u128 {
        let (lower, _) = self.scope.raw_bounds();
        let mut key = 0u128;
        for bit in (0..(128 / N.max(1)).min(8)).rev() {
            for &value in lower.iter() {
                let value = value.min(255);
                key = key << 1 | u128::from((value >> bit) & 1);
            }
        }
        key
    }
}

#[derive(Clone, Copy, Debug)]
struct Hull<const N: usize> {
    lower: [u16; N],
    upper: [u16; N],
    shape: Shape,
    stamp_min: u64,
}

impl<const N: usize> Hull<N> {
    fn empty() -> Self {
        Self {
            lower: [u16::MAX; N],
            upper: [0; N],
            shape: Shape {
                a: (i32::MAX, i32::MIN),
                r: (i32::MAX, i32::MIN),
                d: (i32::MAX, i32::MIN),
            },
            stamp_min: u64::MAX,
        }
    }
    fn add_box(&mut self, lower: &[u16; N], upper: &[u16; N], shape: &Shape, stamp: u64) {
        for axis in 0..N {
            self.lower[axis] = self.lower[axis].min(lower[axis]);
            self.upper[axis] = self.upper[axis].max(upper[axis]);
        }
        let s = &mut self.shape;
        s.a = (s.a.0.min(shape.a.0), s.a.1.max(shape.a.1));
        s.r = (s.r.0.min(shape.r.0), s.r.1.max(shape.r.1));
        s.d = (s.d.0.min(shape.d.0), s.d.1.max(shape.d.1));
        self.stamp_min = self.stamp_min.min(stamp);
    }
    fn add_entry(&mut self, entry: &Entry<N>) {
        let (lower, upper) = entry.scope.raw_bounds();
        self.add_box(lower, upper, &entry.shape, entry.stamp);
    }
    fn add_hull(&mut self, other: &Self) {
        self.add_box(&other.lower, &other.upper, &other.shape, other.stamp_min);
    }
    #[inline]
    fn meets(&self, q: &Query<N>, snapshot: u64) -> bool {
        self.stamp_min < snapshot
            && self.shape.meets_aggregates(&q.shape)
            && boxes_meet(&self.lower, &self.upper, &q.lower, &q.upper)
    }
}

/// Visit counters of one query (telemetry only).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Visits {
    pub hulls: u64,
    pub entries: u64,
    pub tail: u64,
}

/// Immutable sorted run with its hull tree.
pub(super) struct Run<const N: usize> {
    entries: Box<[Entry<N>]>,
    /// levels[0]: one hull per LEAF entries; levels[k]: per FAN hulls of k-1.
    levels: Vec<Box<[Hull<N>]>>,
}

impl<const N: usize> Run<N> {
    fn build(mut entries: Vec<Entry<N>>) -> Self {
        entries.sort_by_cached_key(|entry| (entry.key(), entry.stamp));
        Self::from_sorted(entries)
    }
    fn from_sorted(entries: Vec<Entry<N>>) -> Self {
        let mut levels: Vec<Box<[Hull<N>]>> = Vec::new();
        let leaves: Box<[Hull<N>]> = entries
            .chunks(LEAF)
            .map(|chunk| {
                let mut hull = Hull::empty();
                chunk.iter().for_each(|entry| hull.add_entry(entry));
                hull
            })
            .collect();
        levels.push(leaves);
        while levels.last().is_some_and(|level| level.len() > FAN) {
            let next: Box<[Hull<N>]> = levels
                .last()
                .expect("level")
                .chunks(FAN)
                .map(|chunk| {
                    let mut hull = Hull::empty();
                    chunk.iter().for_each(|child| hull.add_hull(child));
                    hull
                })
                .collect();
            levels.push(next);
        }
        Self {
            entries: entries.into_boxed_slice(),
            levels,
        }
    }
    fn merge(older: &Self, newer: &Self) -> Self {
        let mut out = Vec::with_capacity(older.entries.len() + newer.entries.len());
        let (mut i, mut j) = (0, 0);
        let (a, b) = (&older.entries, &newer.entries);
        let (mut ka, mut kb) = (a.first().map(Entry::key), b.first().map(Entry::key));
        while let (Some(x), Some(y)) = (ka, kb) {
            if (x, a[i].stamp) <= (y, b[j].stamp) {
                out.push(a[i]);
                i += 1;
                ka = a.get(i).map(Entry::key);
            } else {
                out.push(b[j]);
                j += 1;
                kb = b.get(j).map(Entry::key);
            }
        }
        out.extend_from_slice(&a[i..]);
        out.extend_from_slice(&b[j..]);
        Self::from_sorted(out)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    fn query(&self, q: &Query<N>, snapshot: u64, out: &mut Vec<Entry<N>>, visits: &mut Visits) {
        let Some(top) = self.levels.last() else {
            return;
        };
        let depth = self.levels.len() - 1;
        for node in 0..top.len() {
            self.descend(depth, node, q, snapshot, out, visits);
        }
    }
    fn descend(
        &self,
        level: usize,
        node: usize,
        q: &Query<N>,
        snapshot: u64,
        out: &mut Vec<Entry<N>>,
        visits: &mut Visits,
    ) {
        visits.hulls += 1;
        if !self.levels[level][node].meets(q, snapshot) {
            return;
        }
        if level == 0 {
            let end = ((node + 1) * LEAF).min(self.entries.len());
            for entry in &self.entries[node * LEAF..end] {
                visits.entries += 1;
                if entry.stamp < snapshot && entry.meets(q) {
                    out.push(*entry);
                }
            }
            return;
        }
        let end = ((node + 1) * FAN).min(self.levels[level - 1].len());
        for child in node * FAN..end {
            self.descend(level - 1, child, q, snapshot, out, visits);
        }
    }
}

struct Chunk<const N: usize> {
    slots: Box<[OnceLock<Entry<N>>]>,
    len: AtomicUsize,
}

impl<const N: usize> Chunk<N> {
    fn new() -> Self {
        Self {
            slots: (0..CHUNK).map(|_| OnceLock::new()).collect(),
            len: AtomicUsize::new(0),
        }
    }
    fn full(&self) -> bool {
        self.len.load(Ordering::Acquire) == CHUNK
    }
    fn entries(&self) -> impl Iterator<Item = &Entry<N>> {
        let len = self.len.load(Ordering::Acquire);
        self.slots[..len].iter().filter_map(OnceLock::get)
    }
}

#[derive(Clone, Default)]
struct View<const N: usize> {
    runs: Vec<Arc<Run<N>>>,
    chunks: Vec<Arc<Chunk<N>>>,
}

/// One (Apply, owner) bucket.
pub(super) struct Bucket<const N: usize> {
    view: RwLock<Arc<View<N>>>,
    /// Serializes view replacements (coordinator appends, compaction).
    writer: Mutex<()>,
    /// One compaction at a time; planners only `try_lock` it.
    compaction: Mutex<()>,
    pending: AtomicBool,
    /// Coordinator-only append cursor (the last chunk of the view).
    current: Mutex<Option<Arc<Chunk<N>>>>,
    entries: AtomicUsize,
    pub merges: AtomicU64,
    pub merged_entries: AtomicU64,
    pub compaction_micros: AtomicU64,
}

impl<const N: usize> Default for Bucket<N> {
    fn default() -> Self {
        Self {
            view: RwLock::new(Arc::new(View::default())),
            writer: Mutex::new(()),
            compaction: Mutex::new(()),
            pending: AtomicBool::new(false),
            current: Mutex::new(None),
            entries: AtomicUsize::new(0),
            merges: AtomicU64::new(0),
            merged_entries: AtomicU64::new(0),
            compaction_micros: AtomicU64::new(0),
        }
    }
}

impl<const N: usize> Bucket<N> {
    fn replace_view(&self, edit: impl FnOnce(&mut View<N>)) {
        let _writer = self.writer.lock().expect("g2 bucket writer");
        let mut next = (**self.view.read().expect("g2 bucket view")).clone();
        edit(&mut next);
        *self.view.write().expect("g2 bucket view") = Arc::new(next);
    }

    pub fn len(&self) -> usize {
        self.entries.load(Ordering::Relaxed)
    }

    /// Coordinator only. Visible to every view taken after it returns.
    pub fn append(&self, entry: Entry<N>) {
        let mut current = self.current.lock().expect("g2 bucket cursor");
        if current.as_ref().is_none_or(|chunk| chunk.full()) {
            let chunk = Arc::new(Chunk::new());
            let sealed = current.is_some();
            self.replace_view(|view| view.chunks.push(chunk.clone()));
            if sealed {
                self.pending.store(true, Ordering::Release);
            }
            *current = Some(chunk);
        }
        let chunk = current.as_ref().expect("current chunk");
        let slot = chunk.len.load(Ordering::Relaxed);
        if chunk.slots[slot].set(entry).is_err() {
            unreachable!("G2' anchor slot written twice");
        }
        chunk.len.store(slot + 1, Ordering::Release);
        self.entries.fetch_add(1, Ordering::Relaxed);
    }

    /// Restore: one bulk run over entries in merge order.
    pub fn bulk(&self, entries: Vec<Entry<N>>) {
        if entries.is_empty() {
            return;
        }
        let count = entries.len();
        let run = Arc::new(Run::build(entries));
        self.replace_view(|view| view.runs.push(run));
        self.entries.fetch_add(count, Ordering::Relaxed);
    }

    /// Every visible entry with `stamp < snapshot` meeting the query.
    pub fn query(&self, q: &Query<N>, snapshot: u64, out: &mut Vec<Entry<N>>) -> Visits {
        let view = self.view.read().expect("g2 bucket view").clone();
        let mut visits = Visits::default();
        for run in &view.runs {
            run.query(q, snapshot, out, &mut visits);
        }
        for chunk in &view.chunks {
            for entry in chunk.entries() {
                visits.tail += 1;
                if entry.stamp < snapshot && entry.meets(q) {
                    out.push(*entry);
                }
            }
        }
        visits
    }

    /// Seal full chunks into runs and merge runs geometrically, if work is
    /// pending and no other planner is compacting. Returns whether it ran.
    pub fn compact_if_pending(&self) -> bool {
        if !self.pending.load(Ordering::Acquire) {
            return false;
        }
        let Ok(_guard) = self.compaction.try_lock() else {
            return false;
        };
        if !self.pending.swap(false, Ordering::AcqRel) {
            return false;
        }
        let started = std::time::Instant::now();
        let view = self.view.read().expect("g2 bucket view").clone();
        let full: Vec<Arc<Chunk<N>>> = view.chunks.iter().filter(|c| c.full()).cloned().collect();
        if !full.is_empty() {
            let mut entries = Vec::with_capacity(full.len() * CHUNK);
            for chunk in &full {
                entries.extend(chunk.entries().copied());
            }
            let run = Arc::new(Run::build(entries));
            self.replace_view(|view| {
                view.chunks
                    .retain(|c| !full.iter().any(|f| Arc::ptr_eq(c, f)));
                view.runs.push(run);
            });
        }
        // Only this compactor changes `runs`: merge the two newest while the
        // older one is at most twice the newer (geometric run sizes).
        loop {
            let view = self.view.read().expect("g2 bucket view").clone();
            let n = view.runs.len();
            if n < 2 || view.runs[n - 2].len() > 2 * view.runs[n - 1].len() {
                break;
            }
            let (older, newer) = (view.runs[n - 2].clone(), view.runs[n - 1].clone());
            let merged = Arc::new(Run::merge(&older, &newer));
            self.merges.fetch_add(1, Ordering::Relaxed);
            self.merged_entries
                .fetch_add(merged.len() as u64, Ordering::Relaxed);
            self.replace_view(|view| {
                let n = view.runs.len();
                debug_assert!(Arc::ptr_eq(&view.runs[n - 2], &older));
                view.runs.truncate(n - 2);
                view.runs.push(merged);
            });
        }
        self.compaction_micros
            .fetch_add(started.elapsed().as_micros() as u64, Ordering::Relaxed);
        true
    }

    pub fn runs_and_tail(&self) -> (usize, usize) {
        let view = self.view.read().expect("g2 bucket view").clone();
        (
            view.runs.len(),
            view.chunks
                .iter()
                .map(|c| c.len.load(Ordering::Acquire))
                .sum(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::routed_campaign::walking::queue::{Domain, Phase};

    /// None: the domain has no lattice point (never indexed).
    fn entry(
        owner: [bool; 3],
        lower: [u64; 3],
        upper: [u64; 3],
        rank: u32,
        stamp: u64,
    ) -> Option<Entry<3>> {
        let domain = Domain {
            phase: Phase::Apply,
            owner,
            lower: lower.to_vec(),
            upper: upper.iter().map(|&u| Some(u)).collect(),
            rank: Some(rank),
            powers: DomainPowerBounds::default(),
        };
        let scope = CompactDomain::try_from_domain(&domain).unwrap();
        let (l, u) = scope.raw_bounds();
        Some(Entry {
            shape: Shape::of(&owner, l, u, Some(rank), DomainPowerBounds::default())?,
            scope,
            stamp,
            id: stamp as u32,
            kind: 0,
        })
    }

    fn brute(entries: &[Entry<3>], q: &Query<3>, snapshot: u64) -> Vec<u32> {
        let mut ids: Vec<u32> = entries
            .iter()
            .filter(|e| e.stamp < snapshot && e.meets(q))
            .map(|e| e.id)
            .collect();
        ids.sort_unstable();
        ids
    }

    #[test]
    fn indexed_query_equals_the_linear_filter_through_seals_and_merges() {
        let owner = [true, false, true];
        let bucket = Bucket::<3>::default();
        let mut all = Vec::new();
        let mut seed = 7u64;
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            seed >> 33
        };
        for stamp in 0..(3 * CHUNK as u64 + 17) {
            let lower = [next() % 6, next() % 6, next() % 6];
            let upper = [
                lower[0] + next() % 3,
                lower[1] + next() % 3,
                lower[2] + next() % 3,
            ];
            let Some(e) = entry(owner, lower, upper, (next() % 8) as u32, stamp) else {
                continue;
            };
            bucket.append(e);
            all.push(e);
            if stamp % 500 == 0 {
                bucket.compact_if_pending();
            }
        }
        bucket.compact_if_pending();
        let (runs, tail) = bucket.runs_and_tail();
        assert!(runs >= 1 && tail < CHUNK, "runs {runs} tail {tail}");
        for trial in 0..200u64 {
            let lower = [next() % 7, next() % 7, next() % 7];
            let upper = [
                lower[0] + next() % 2,
                lower[1] + next() % 2,
                lower[2] + next() % 2,
            ];
            let Some(probe) = entry(owner, lower, upper, (next() % 9) as u32, 0) else {
                continue;
            };
            let (l, u) = probe.scope.raw_bounds();
            let q = Query {
                lower: *l,
                upper: *u,
                shape: probe.shape,
            };
            let snapshot = if trial % 3 == 0 {
                u64::MAX
            } else {
                next() % all.len() as u64
            };
            let mut out = Vec::new();
            bucket.query(&q, snapshot, &mut out);
            let mut got: Vec<u32> = out.iter().map(|e| e.id).collect();
            got.sort_unstable();
            assert_eq!(got, brute(&all, &q, snapshot), "trial {trial}");
        }
    }

    #[test]
    fn shape_membership_is_exact_on_small_boxes() {
        let owner = [true, false, true];
        for rank in 0..4u32 {
            for amax in 3..9u64 {
                for (dmin, dmax) in [
                    (None, None),
                    (Some(1), None),
                    (None, Some(3)),
                    (Some(2), Some(4)),
                ] {
                    let powers = DomainPowerBounds {
                        max_positive_power: Some(amax),
                        min_power_difference: dmin,
                        max_power_difference: dmax,
                    };
                    let lower = [0u16, 1, 1];
                    let upper = [3u16, 3, 2];
                    let summary = rustred::solver::DomainPowerSummary::try_new(
                        owner,
                        &[0, 1, 1],
                        &[Some(3), Some(3), Some(2)],
                        Some(rank),
                        powers,
                    )
                    .unwrap();
                    let shape = Shape::of(&owner, &lower, &upper, Some(rank), powers);
                    assert_eq!(shape.is_none(), summary.is_empty());
                    let Some(shape) = shape else { continue };
                    let e = Entry {
                        scope: CompactDomain::try_from_domain(&Domain {
                            phase: Phase::Apply,
                            owner,
                            lower: vec![0, 1, 1],
                            upper: vec![Some(3), Some(3), Some(2)],
                            rank: Some(rank),
                            powers,
                        })
                        .unwrap(),
                        shape,
                        stamp: 0,
                        id: 0,
                        kind: 0,
                    };
                    for x0 in 0..5u16 {
                        for x1 in 0..5u16 {
                            for x2 in 0..5u16 {
                                let a = i32::from(x0) + 1 + i32::from(x2) + 1;
                                let r = i32::from(x1);
                                let p = Point {
                                    x: [x0, x1, x2],
                                    a,
                                    r,
                                    d: a - r,
                                };
                                let one = rustred::solver::DomainPowerSummary::try_new(
                                    owner,
                                    &[u64::from(x0), u64::from(x1), u64::from(x2)],
                                    &[
                                        Some(u64::from(x0)),
                                        Some(u64::from(x1)),
                                        Some(u64::from(x2)),
                                    ],
                                    None,
                                    DomainPowerBounds::default(),
                                )
                                .unwrap();
                                assert_eq!(
                                    e.contains(&p),
                                    summary.contains(&one),
                                    "{p:?} rank {rank} amax {amax} {dmin:?} {dmax:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
