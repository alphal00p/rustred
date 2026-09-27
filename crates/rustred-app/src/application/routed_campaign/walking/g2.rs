//! W0 G2' falsifier (THROWAWAY, branch `fable_5_1-v3-g2falsify`, never merged
//! into the production engine): D-only residual inspection of a newly
//! dispatched Apply domain Q against ONE Native Apply anchor A of the same
//! (phase, owner) bucket that was committed strictly before Q's dispatch.
//!
//! Selected by `RUSTRED_WALK_G2_DONLY=1`; any other value (or unset) is off and
//! leaves the engine byte-identical (no store, no dispatch stamps, no record
//! or report fields).
//!
//! Plan: for the smallest cut c such that A contains Q restricted to D >= c
//! (checked with the native `DomainPowerSummary::contains`, never a prefilter
//! alone), the job inspects only the residual Q restricted to D <= c-1. An
//! empty residual (A contains Q) is an alias-like cover with zero native work.
//! The record keeps Q's identity and coordinates, reuses the partial-record
//! semantics of `initial_overlap.rs` (`partial_initial_overlap_inspection`,
//! anchor edge Q -> A in the ledger and the descendant closure), and carries a
//! `g2_residual_anchor` block with the commit stamps for the audit.
//!
//! Soundness: anchors are committed Native records (full or partial) whose
//! commit stamp is below Q's dispatch stamp; they are never pending, aliased
//! or in flight, so anchor chains are strictly ordered in commit time (no
//! mutual subtraction). Frontiers/errors of A block Q through the edge.
//! Scope: pool runs (workers > 1) without Apply subdivision; initial-prefix
//! domains are never planned; checkpoints written with the flag on are not
//! resumable (the relaxed ledger rules are not persisted).
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Instant;

use rustred::solver::{DomainPowerBounds, DomainPowerSummary};
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

pub(super) fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        let on = match std::env::var("RUSTRED_WALK_G2_DONLY").as_deref() {
            Err(_) | Ok("") | Ok("0") | Ok("off") => false,
            Ok("1") => true,
            Ok(other) => panic!("RUSTRED_WALK_G2_DONLY={other}: expected 1 or unset"),
        };
        if on {
            eprintln!("W0 G2' D-only residual-anchor falsifier ACTIVE (one committed Native anchor)");
        }
        on
    })
}

/// Commit/dispatch stamps and the plan outcome, copied into the record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct G2Info {
    pub anchor_commit_seq: u64,
    pub dispatch_snapshot: u64,
    pub full_cover: bool,
    pub anchors_scanned: u64,
    pub plan_micros: u64,
    pub original_d_levels: u64,
    pub residual_d_levels: u64,
}

impl G2Info {
    pub fn json(&self) -> Value {
        json!({"mode":"d_only_single_committed_native_anchor",
            "anchor_commit_seq":self.anchor_commit_seq,
            "dispatch_snapshot":self.dispatch_snapshot,
            "full_cover":self.full_cover,
            "residual_pieces":u8::from(!self.full_cover),
            "original_d_levels":self.original_d_levels,
            "residual_d_levels":self.residual_d_levels,
            "anchors_scanned":self.anchors_scanned,
            "plan_micros":self.plan_micros,
            "authority":"native_summary_contains_of_D_ge_cut_slice; anchor committed before dispatch"})
    }
}

pub(super) struct Plan<const N: usize> {
    pub scope: InitialOverlapScope,
    /// None: the anchor contains Q (empty residual, no native call).
    pub residual: Option<Domain<N>>,
}

struct Anchor<const N: usize> {
    id: usize,
    seq: u64,
    summary: DomainPowerSummary<N>,
    dlo: Option<i128>,
    dhi_unbounded: bool,
}

struct Chunk<const N: usize> {
    slots: Box<[OnceLock<Anchor<N>>]>,
    len: AtomicUsize,
}

impl<const N: usize> Chunk<N> {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            slots: (0..CHUNK).map(|_| OnceLock::new()).collect(),
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
    anchors_scanned: AtomicU64,
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
            "anchors_scanned":l(&self.anchors_scanned),"top_slice_filter_pass":l(&self.top_filter_pass),
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
    appended: AtomicU64,
    unusable: AtomicU64,
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

impl<const N: usize> Store<N> {
    pub fn from_env() -> Option<Arc<Self>> {
        enabled().then(|| {
            Arc::new(Self {
                buckets: RwLock::new(HashMap::new()),
                committed: AtomicU64::new(0),
                dispatch: Mutex::new(HashMap::new()),
                appended: AtomicU64::new(0),
                unusable: AtomicU64::new(0),
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

    /// Coordinator: the dispatch did not happen after all.
    pub fn forget_dispatch(&self, id: usize) {
        self.dispatch.lock().expect("g2 dispatch table").remove(&id);
    }

    /// Coordinator, at the end of a committed (published) Apply record without
    /// error. Returns this record's commit stamp. The record becomes visible
    /// as an anchor to jobs dispatched from now on.
    pub fn commit(&self, id: usize, domain: &Domain<N>) -> u64 {
        let seq = self.committed.load(Ordering::Relaxed);
        self.classes[class(&domain.owner)]
            .commits
            .fetch_add(1, Ordering::Relaxed);
        let usable = summary(domain, domain.powers).and_then(|s| {
            let (dlo, dhi) = s.extrema()?.power_difference();
            Some(Anchor {
                id,
                seq,
                dlo,
                dhi_unbounded: dhi.is_none(),
                summary: s,
            })
        });
        if let Some(anchor) = usable {
            self.append(domain.owner, anchor);
            self.appended.fetch_add(1, Ordering::Relaxed);
        } else {
            self.unusable.fetch_add(1, Ordering::Relaxed);
        }
        // Publish the stamp only after the anchor is readable.
        self.committed.store(seq + 1, Ordering::Release);
        seq
    }

    fn append(&self, owner: [bool; N], anchor: Anchor<N>) {
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
        // Single writer: the slot at len is unset.
        let slot = tail.len.load(Ordering::Relaxed);
        if tail.slots[slot].set(anchor).is_err() {
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
        let result = self.search(q, snapshot, cancellation, stats);
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
            stats
                .original_d_levels
                .fetch_add(info.original_d_levels, Ordering::Relaxed);
            stats
                .residual_d_levels
                .fetch_add(info.residual_d_levels, Ordering::Relaxed);
        }
        Some(plan)
    }

    fn search(
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
        let bucket = self.buckets.read().expect("g2 buckets").get(&q.owner).cloned()?;
        let chunks = bucket.chunks.read().expect("g2 chunks").clone();
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
        // Necessary for every slab: the top D level of Q lies in each slab.
        let top = match qdhi {
            Some(h) => Some(slab(h)?),
            None => None,
        };
        let mut scanned = 0u64;
        let mut best: Option<(&Anchor<N>, i128)> = None;
        let mut full: Option<&Anchor<N>> = None;
        'outer: for chunk in &chunks {
            let len = chunk.len.load(Ordering::Acquire);
            for slot in &chunk.slots[..len] {
                let Some(anchor) = slot.get() else {
                    break 'outer;
                };
                // Append order is commit order: nothing later is visible.
                if anchor.seq >= snapshot {
                    break 'outer;
                }
                scanned += 1;
                if scanned % 4096 == 0 && cancellation.load(Ordering::Relaxed) {
                    break 'outer;
                }
                if anchor.summary.contains(&whole) {
                    full = Some(anchor);
                    break 'outer;
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
                // Q itself is not contained, so a useful cut is above qdlo.
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
                let contains = |s: Option<DomainPowerSummary<N>>| {
                    s.is_some_and(|s| anchor.summary.contains(&s))
                };
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
        stats.anchors_scanned.fetch_add(scanned, Ordering::Relaxed);
        let levels = |lo: i128, hi: Option<i128>| hi.map_or(0, |h| (h - lo + 1).max(0) as u64);
        if let Some(anchor) = full {
            // Authority: exact native inclusion of the whole of Q.
            if !anchor.summary.contains(&whole) {
                return None;
            }
            let cut = i64::try_from(qdlo).ok()?;
            return Some(Plan {
                scope: InitialOverlapScope {
                    anchor_id: anchor.id,
                    cut,
                    residual_powers: low_powers(q.powers, cut)?,
                    g2: Some(G2Info {
                        anchor_commit_seq: anchor.seq,
                        dispatch_snapshot: snapshot,
                        full_cover: true,
                        anchors_scanned: scanned,
                        plan_micros: 0,
                        original_d_levels: levels(qdlo, qdhi),
                        residual_d_levels: 0,
                    }),
                },
                residual: None,
            });
        }
        let (anchor, cut) = best?;
        let cut64 = i64::try_from(cut).ok()?;
        let high = summary(q, high_powers(q.powers, cut64))?;
        let low = low_powers(q.powers, cut64)?;
        let low_summary = summary(q, low)?;
        // Authority: exact native inclusion of the D >= cut slice; the
        // residual D <= cut-1 must be nonempty (else Q itself is contained).
        if high.is_empty() || low_summary.is_empty() || !anchor.summary.contains(&high) {
            return None;
        }
        let mut lower = Vec::new();
        let mut upper = Vec::new();
        if lower.try_reserve_exact(q.lower.len()).is_err()
            || upper.try_reserve_exact(q.upper.len()).is_err()
        {
            return None;
        }
        lower.extend_from_slice(&q.lower);
        upper.extend_from_slice(&q.upper);
        let rdhi = low_summary.extrema().and_then(|e| e.power_difference().1);
        Some(Plan {
            scope: InitialOverlapScope {
                anchor_id: anchor.id,
                cut: cut64,
                residual_powers: low,
                g2: Some(G2Info {
                    anchor_commit_seq: anchor.seq,
                    dispatch_snapshot: snapshot,
                    full_cover: false,
                    anchors_scanned: scanned,
                    plan_micros: 0,
                    original_d_levels: levels(qdlo, qdhi),
                    residual_d_levels: levels(qdlo, rdhi),
                }),
            },
            residual: Some(Domain {
                phase: q.phase,
                owner: q.owner,
                lower,
                upper,
                rank: q.rank,
                powers: low,
            }),
        })
    }

    pub fn report(&self) -> Value {
        let mut classes = serde_json::Map::new();
        for (name, stats) in CLASS_NAMES.iter().zip(&self.classes) {
            classes.insert((*name).to_owned(), stats.json());
        }
        json!({"mode":"d_only_single_committed_native_anchor",
            "env":"RUSTRED_WALK_G2_DONLY=1",
            "committed_apply_records":self.committed.load(Ordering::Relaxed),
            "anchors_appended":self.appended.load(Ordering::Relaxed),
            "anchors_unusable_summary":self.unusable.load(Ordering::Relaxed),
            "dispatch_stamps_outstanding":self.dispatch.lock().map_or(0, |d| d.len()),
            "by_owner_class":classes,
            "scope":"process counters of the throwaway W0 G2' falsifier; plans of cancelled jobs included"})
    }
}
