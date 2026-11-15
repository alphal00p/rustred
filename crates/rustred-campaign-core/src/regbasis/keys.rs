//! Keys of the regular-basis construction: the band of a box, ranks and dots, the candidate family, the dot-free keys of
//! the tower test, and the preference order. Everything is driven by the chart (which slots are lines) and data; no
//! arity, line count or bound is a constant here (L8).

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::boxdef::BoxDef;
use crate::key::{Key, compositions};

use super::spec::{CandidateOrder, RegBasisSpec};

/// Dots D = sum over lines of (n - 1) of a TOP-SECTOR chart key (every line >= 1, every other slot <= 0); `None`
/// otherwise (or on a wrong length).
pub fn dots_of(chart: &[bool], key: &[i32]) -> Option<i64> {
    if !is_top_chart(chart, key) {
        return None;
    }
    Some(
        key.iter()
            .zip(chart)
            .filter(|(_, l)| **l)
            .map(|(&x, _)| i64::from(x) - 1)
            .sum(),
    )
}

/// Rank r = sum over the non-line slots of |n| of a top-sector chart key; `None` otherwise.
pub fn rank_of(chart: &[bool], key: &[i32]) -> Option<i64> {
    if !is_top_chart(chart, key) {
        return None;
    }
    Some(
        key.iter()
            .zip(chart)
            .filter(|(_, l)| !**l)
            .map(|(&x, _)| -i64::from(x))
            .sum(),
    )
}

fn is_top_chart(chart: &[bool], key: &[i32]) -> bool {
    key.len() == chart.len()
        && key
            .iter()
            .zip(chart)
            .all(|(&x, &l)| if l { x >= 1 } else { x <= 0 })
}

/// The TOP-SECTOR keys of a box (the band of the construction), in the box's streaming order. Subsector keys are
/// not part of the construction (everything is modulo subsectors).
pub fn band_keys(bx: &BoxDef) -> Vec<Key> {
    let mut out = vec![];
    bx.for_each_key(|k| {
        if is_top_chart(&bx.chart, k) {
            out.push(k.to_vec());
        }
    });
    out
}

/// The dot-free top-sector keys (every line at power 1) of rank exactly `rank`, lexicographically sorted.
pub fn dot_free_keys(chart: &[bool], rank: u32) -> Vec<Key> {
    let nums: Vec<usize> = (0..chart.len()).filter(|&i| !chart[i]).collect();
    let mut out: Vec<Key> = compositions(rank, nums.len(), rank)
        .into_iter()
        .map(|c| {
            let mut k: Key = chart.iter().map(|&l| i32::from(l)).collect();
            for (j, &s) in nums.iter().enumerate() {
                k[s] = -(c[j] as i32);
            }
            k
        })
        .collect();
    out.sort();
    out
}

/// The candidate family of the spec: the top-sector keys with dots <= max_dots, rank <= max_rank, dots + rank <=
/// max_weight, line powers bounded by the dots, restricted to `allowed` (normally: in the band). Unsorted.
pub fn candidate_family(chart: &[bool], spec: &RegBasisSpec, allowed: &dyn Fn(&[i32]) -> bool) -> Vec<Key> {
    let lines: Vec<usize> = (0..chart.len()).filter(|&i| chart[i]).collect();
    let nums: Vec<usize> = (0..chart.len()).filter(|&i| !chart[i]).collect();
    let mut out = vec![];
    for d in 0..=spec.max_dots.min(spec.max_weight) {
        let rmax = spec.max_rank.min(spec.max_weight - d);
        let dl = compositions(d, lines.len(), d);
        for r in 0..=rmax {
            let nl = compositions(r, nums.len(), r);
            for a in &dl {
                for b in &nl {
                    let mut k = vec![0i32; chart.len()];
                    for (j, &s) in lines.iter().enumerate() {
                        k[s] = 1 + a[j] as i32;
                    }
                    for (j, &s) in nums.iter().enumerate() {
                        k[s] = -(b[j] as i32);
                    }
                    if allowed(&k) {
                        out.push(k);
                    }
                }
            }
        }
    }
    out
}

/// The preference order of the greedy selection.
pub struct Preference<'a> {
    pub chart: &'a [bool],
    pub order: CandidateOrder,
    /// The closure's terminals; with `terminals_first` they head the order in their closure order.
    pub terminals: &'a [Key],
    pub terminals_first: bool,
    /// The node comparator (`NodeComparator::cmp` of the closure's order).
    pub cmp: &'a dyn Fn(&[i32], &[i32]) -> Ordering,
}

/// Sort keys by the preference (stable; duplicates removed).
pub fn sort_by_preference(keys: &mut Vec<Key>, p: &Preference<'_>) {
    let tpos: HashMap<&[i32], usize> = p
        .terminals
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_slice(), i))
        .collect();
    let weight = |k: &[i32]| -> i64 { dots_of(p.chart, k).unwrap_or(i64::MAX / 4) + rank_of(p.chart, k).unwrap_or(i64::MAX / 4) };
    keys.sort_by(|a, b| {
        if p.terminals_first {
            match (tpos.get(a.as_slice()), tpos.get(b.as_slice())) {
                (Some(x), Some(y)) => return x.cmp(y),
                (Some(_), None) => return Ordering::Less,
                (None, Some(_)) => return Ordering::Greater,
                (None, None) => {}
            }
        }
        let w = match p.order {
            CandidateOrder::WeightThenNode => weight(a).cmp(&weight(b)),
            CandidateOrder::Node => Ordering::Equal,
        };
        w.then_with(|| (p.cmp)(a, b)).then_with(|| a.cmp(b))
    });
    keys.dedup();
}
