//! Real-data validation of the exact multi-target cover predicate
//! `lattice::Cell::covered_by_union` against point enumeration
//! (`Cell::brute_force_covered_by_union`: direct evaluation of the defining
//! inequalities at every point), on sampled saved cells with all N axes and
//! both owner groups (`--union-sample COUNT[:SEED]`).
//!
//! For each sampled cell Q whose A/R-capped box holds at most `MAX_POINTS`
//! lattice points, two families of covers are decided both ways:
//! - real: earlier Native records of the same (phase, owner) bucket, taken
//!   greedily (scanning back from Q) while each covers a point of Q the
//!   previous ones leave uncovered, up to `MAX_TARGETS` records: genuine
//!   multi-record covers and near misses, the G2' question "is Q contained
//!   in the union of merged natives";
//! - synthetic: Q split strictly inside its actual point ranges, along a
//!   coordinate axis or D, into 2 or 3 pieces (exact partitions, with the
//!   real records appended as distractors), and the same splits with a
//!   one-layer gap (no distractors).
//!
//! Every decided exact answer must equal enumeration, every partition must
//! be covered by enumeration, and the greedy uncovered-point bookkeeping
//! must agree with enumeration; anything else is a `union_cross_check`
//! violation. Undecided answers (region budget) are counted, never wrong.
use super::super::queue::{CompactDomain, Phase};
use super::lattice::Cell;
use super::{Kind, Node, ccell, sample};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const MAX_POINTS: u64 = 1 << 16;
const GREEDY_POINTS: u64 = 1 << 14;
const MAX_TARGETS: usize = 8;
const MAX_SCAN: usize = 100_000;
const REGION_BUDGET: u64 = 1 << 20;

#[derive(Default, Clone, Copy)]
struct Family {
    checks: u64,
    covered: u64,
    not_covered: u64,
    undecided: u64,
    disagreements: u64,
}
impl Family {
    fn record(&mut self, exact: Option<bool>, brute: bool) {
        self.checks += 1;
        match exact {
            None => self.undecided += 1,
            Some(exact) => {
                if exact {
                    self.covered += 1;
                } else {
                    self.not_covered += 1;
                }
                self.disagreements += u64::from(exact != brute);
            }
        }
    }
    fn add(&mut self, other: &Family) {
        self.checks += other.checks;
        self.covered += other.covered;
        self.not_covered += other.not_covered;
        self.undecided += other.undecided;
        self.disagreements += other.disagreements;
    }
    fn json(&self) -> Value {
        json!({"checks": self.checks, "covered": self.covered, "not_covered": self.not_covered,
            "undecided": self.undecided, "disagreements_with_enumeration": self.disagreements})
    }
}

#[derive(Default)]
struct Stats {
    sampled: u64,
    empty: u64,
    not_enumerable: u64,
    enumerable: u64,
    points: u64,
    real: Family,
    real_single_record: u64,
    real_union_only: u64,
    real_union_only_max_records: u64,
    real_targets: u64,
    real_no_target: u64,
    real_too_many_points: u64,
    greedy_mismatches: u64,
    partition2: Family,
    partition3: Family,
    gap2: Family,
    gap3: Family,
    partitions_not_covered: u64,
}
impl Stats {
    fn add(&mut self, other: &Stats) {
        self.sampled += other.sampled;
        self.empty += other.empty;
        self.not_enumerable += other.not_enumerable;
        self.enumerable += other.enumerable;
        self.points += other.points;
        self.real.add(&other.real);
        self.real_single_record += other.real_single_record;
        self.real_union_only += other.real_union_only;
        self.real_union_only_max_records = self
            .real_union_only_max_records
            .max(other.real_union_only_max_records);
        self.real_targets += other.real_targets;
        self.real_no_target += other.real_no_target;
        self.real_too_many_points += other.real_too_many_points;
        self.greedy_mismatches += other.greedy_mismatches;
        self.partition2.add(&other.partition2);
        self.partition3.add(&other.partition3);
        self.gap2.add(&other.gap2);
        self.gap3.add(&other.gap3);
        self.partitions_not_covered += other.partitions_not_covered;
    }
    fn failures(&self) -> u64 {
        [
            self.real,
            self.partition2,
            self.partition3,
            self.gap2,
            self.gap3,
        ]
        .iter()
        .map(|f| f.disagreements)
        .sum::<u64>()
            + self.greedy_mismatches
            + self.partitions_not_covered
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

/// Split `cell` along `dim` (an axis, or D when `dim == owner.len()`) into
/// `x <= cut` and `x >= cut + 1 + gap`.
fn split(cell: &Cell, dim: usize, cut: i64, gap: bool) -> (Cell, Cell) {
    let (mut low, mut high) = (cell.clone(), cell.clone());
    let next = cut + 1 + i64::from(gap);
    if dim < cell.owner.len() {
        let (cut, next) = (cut as u64, next as u64);
        low.upper[dim] = Some(low.upper[dim].map_or(cut, |u| u.min(cut)));
        high.lower[dim] = high.lower[dim].max(next);
    } else {
        low.powers.max_power_difference =
            Some(low.powers.max_power_difference.map_or(cut, |d| d.min(cut)));
        high.powers.min_power_difference = Some(
            high.powers
                .min_power_difference
                .map_or(next, |d| d.max(next)),
        );
    }
    (low, high)
}

fn one<const N: usize>(
    q: usize,
    domains: &[CompactDomain<N>],
    buckets: &BTreeMap<(bool, [bool; N]), Vec<u32>>,
    seed: u64,
    stats: &mut Stats,
) {
    stats.sampled += 1;
    let cell = ccell(&domains[q]);
    // Actual per-axis and D ranges of Q's points; the points themselves for
    // the greedy real cover.
    let mut low = [u64::MAX; N];
    let mut high = [0u64; N];
    let (mut d_low, mut d_high) = (i64::MAX, i64::MIN);
    let mut points: Vec<u64> = Vec::new();
    let visited = cell.for_each_point(MAX_POINTS, |point| {
        let (mut a, mut r) = (0i64, 0i64);
        for axis in 0..N {
            low[axis] = low[axis].min(point[axis]);
            high[axis] = high[axis].max(point[axis]);
            if cell.owner[axis] {
                a += point[axis] as i64 + 1;
            } else {
                r += point[axis] as i64;
            }
        }
        d_low = d_low.min(a - r);
        d_high = d_high.max(a - r);
        if (points.len() as u64) < GREEDY_POINTS * N as u64 {
            points.extend_from_slice(point);
        }
        true
    });
    let count = match visited {
        None => {
            stats.not_enumerable += 1;
            return;
        }
        Some(0) => {
            stats.empty += 1;
            return;
        }
        Some(count) => count,
    };
    stats.enumerable += 1;
    stats.points += count;
    let brute = |targets: &[&Cell]| {
        cell.brute_force_covered_by_union(targets, MAX_POINTS)
            .expect("enumerable")
    };
    // Real earlier Native records, greedily.
    let mut real: Vec<Cell> = Vec::new();
    if count <= GREEDY_POINTS {
        let key = (domains[q].phase() == Phase::Route, domains[q].owner());
        let mut uncovered: Vec<usize> = (0..count as usize).collect();
        if let Some(bucket) = buckets.get(&key) {
            let start = bucket.partition_point(|&t| (t as usize) < q);
            for &t in bucket[..start].iter().rev().take(MAX_SCAN) {
                let target = ccell(&domains[t as usize]);
                if !target.meets(&cell) {
                    continue;
                }
                let before = uncovered.len();
                uncovered.retain(|&i| !target.member(&points[i * N..(i + 1) * N]));
                if uncovered.len() < before {
                    real.push(target);
                    if uncovered.is_empty() || real.len() == MAX_TARGETS {
                        break;
                    }
                }
            }
        }
        if real.is_empty() {
            stats.real_no_target += 1;
        } else {
            let refs: Vec<&Cell> = real.iter().collect();
            let exact = cell.covered_by_union(&refs, REGION_BUDGET);
            let truth = brute(&refs);
            stats.real.record(exact, truth);
            stats.real_targets += refs.len() as u64;
            stats.greedy_mismatches += u64::from(truth != uncovered.is_empty());
            if exact == Some(true) {
                if refs.iter().any(|t| t.contains(&cell)) {
                    stats.real_single_record += 1;
                } else {
                    stats.real_union_only += 1;
                    stats.real_union_only_max_records =
                        stats.real_union_only_max_records.max(refs.len() as u64);
                }
            }
        }
    } else {
        stats.real_too_many_points += 1;
    }
    // Synthetic splits strictly inside the actual ranges.
    let mut dims: Vec<(usize, i64, i64)> = (0..N)
        .filter(|&axis| high[axis] > low[axis])
        .map(|axis| (axis, low[axis] as i64, high[axis] as i64))
        .collect();
    if d_high > d_low {
        dims.push((N, d_low, d_high));
    }
    if dims.is_empty() {
        return;
    }
    let mut rng = Rng(seed ^ (q as u64).wrapping_mul(0xA24B_AED4_963E_E407));
    let pick = |rng: &mut Rng, choices: &[(usize, i64, i64)]| {
        let (dim, lo, hi) = choices[rng.below(choices.len() as u64) as usize];
        (dim, lo + rng.below((hi - lo) as u64) as i64)
    };
    let (first, cut) = pick(&mut rng, &dims);
    // The second split on another dimension when one exists.
    let others: Vec<(usize, i64, i64)> = dims.iter().copied().filter(|d| d.0 != first).collect();
    let (second, second_cut) = pick(&mut rng, if others.is_empty() { &dims } else { &others });
    let distractors: Vec<&Cell> = real.iter().collect();
    let (low_piece, high_piece) = split(&cell, first, cut, false);
    for gap in [false, true] {
        let (a, b) = split(&cell, first, cut, gap);
        let mut targets = vec![&a, &b];
        if !gap {
            targets.extend(distractors.iter().copied());
        }
        let exact = cell.covered_by_union(&targets, REGION_BUDGET);
        let truth = brute(&targets);
        if gap {
            stats.gap2.record(exact, truth);
        } else {
            stats.partition2.record(exact, truth);
            stats.partitions_not_covered += u64::from(!truth);
        }
        // Three pieces: the high part split again, the gap (if any) in the
        // second split only.
        let (b1, b2) = split(&high_piece, second, second_cut, gap);
        let mut targets = vec![&low_piece, &b1, &b2];
        if !gap {
            targets.extend(distractors.iter().copied());
        }
        let exact = cell.covered_by_union(&targets, REGION_BUDGET);
        let truth = brute(&targets);
        if gap {
            stats.gap3.record(exact, truth);
        } else {
            stats.partition3.record(exact, truth);
            stats.partitions_not_covered += u64::from(!truth);
        }
    }
}

/// Runs the validation on `count` sampled records; returns the report block
/// and the number of failures (each a `union_cross_check` violation).
pub(super) fn validate<const N: usize>(
    domains: &[CompactDomain<N>],
    nodes: &[Node],
    count: usize,
    seed: u64,
    threads: usize,
    cancellation: &AtomicBool,
) -> (Value, u64) {
    let started = std::time::Instant::now();
    let mut buckets: BTreeMap<(bool, [bool; N]), Vec<u32>> = BTreeMap::new();
    for (id, node) in nodes.iter().enumerate() {
        if node.kind == Kind::Native {
            let domain = &domains[id];
            buckets
                .entry((domain.phase() == Phase::Route, domain.owner()))
                .or_default()
                .push(id as u32);
        }
    }
    let candidates: Vec<usize> = (0..nodes.len())
        .filter(|&id| nodes[id].kind != Kind::Missing)
        .collect();
    let selected = sample(&candidates, count, seed);
    drop(candidates);
    let next = AtomicUsize::new(0);
    let total = Mutex::new(Stats::default());
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                let mut stats = Stats::default();
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= selected.len() || cancellation.load(Ordering::Relaxed) {
                        break;
                    }
                    one(selected[index], domains, &buckets, seed, &mut stats);
                }
                total.lock().expect("union sample stats").add(&stats);
            });
        }
    });
    let stats = total.into_inner().expect("union sample stats");
    let failures = stats.failures();
    let report = json!({
        "requested": count, "seed": seed, "sampled": stats.sampled, "axes": N,
        "enumerable": stats.enumerable, "empty": stats.empty,
        "not_enumerable": stats.not_enumerable, "enumerated_points": stats.points,
        "max_points": MAX_POINTS, "greedy_max_points": GREEDY_POINTS,
        "max_records": MAX_TARGETS, "max_scan": MAX_SCAN, "region_budget": REGION_BUDGET,
        "real_earlier_natives": {
            "decisions": stats.real.json(),
            "covered_by_one_record": stats.real_single_record,
            "covered_only_by_a_union": stats.real_union_only,
            "covered_only_by_a_union_max_records": stats.real_union_only_max_records,
            "records_used": stats.real_targets,
            "no_intersecting_earlier_native": stats.real_no_target,
            "skipped_above_greedy_max_points": stats.real_too_many_points,
            "greedy_bookkeeping_mismatches": stats.greedy_mismatches},
        "synthetic": {"partition2_with_real_distractors": stats.partition2.json(),
            "partition3_with_real_distractors": stats.partition3.json(),
            "one_layer_gap2": stats.gap2.json(), "one_layer_gap3": stats.gap3.json(),
            "partitions_not_covered_by_enumeration": stats.partitions_not_covered},
        "failures": failures,
        "seconds": started.elapsed().as_secs_f64(),
        "definitions": "every decided covered_by_union answer is compared with point enumeration of Q; real = earlier same-bucket Native records chosen greedily while each covers a new point of Q (covered_only_by_a_union: no single record contains Q); synthetic = Q split strictly inside its actual point ranges along an axis or D (2 or 3 pieces; gap variants leave one layer out)"});
    (report, failures)
}
