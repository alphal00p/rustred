//! Shared read-only helpers for the W0.9 / W0 G2' falsifier tools (copied from
//! branch fable_5_1-v3-widen, tools/research/w0_falsify).
//!
//! Records come from a walk `result.json` (pretty-printed; only the top-level
//! `"domains"` array is read) or from CP5 `records-*.jsonl` sidecars.
//! Lattice points are exact enumerations of box ∩ {A <= Amax, R <= rank,
//! Dmin <= D <= Dmax}, the set `DomainPowerSummary::contains` decides
//! (same convention as the session `rtool`: active n = x + 1, inactive n = -x).
use serde::Deserialize;
use std::io::{BufRead, BufReader};

#[derive(Deserialize, Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PB {
    pub max_positive_power: Option<u64>,
    pub min_power_difference: Option<i64>,
    pub max_power_difference: Option<i64>,
}

#[derive(Deserialize, Debug)]
pub struct Rec {
    pub id: serde_json::Value,
    pub owner: String,
    pub phase: Option<String>,
    pub lower: Vec<u64>,
    pub upper: Vec<Option<u64>>,
    pub rank: Option<u32>,
    pub power_bounds: Option<PB>,
    pub record_kind: String,
    pub seconds: Option<f64>,
    pub stats: Option<serde_json::Value>,
    pub frontiers: Option<Vec<serde_json::Value>>,
    pub error: Option<serde_json::Value>,
    pub representative_id: Option<serde_json::Value>,
    pub descendant_closed: Option<bool>,
    #[serde(default)]
    pub initial_overlap: Option<serde_json::Value>,
    #[serde(default)]
    pub g2_residual_anchor: Option<serde_json::Value>,
    #[serde(default)]
    pub g2_commit_seq: Option<u64>,
}

impl Rec {
    pub fn id_u64(&self) -> Option<u64> {
        self.id.as_u64()
    }
    pub fn owner_bits(&self) -> Vec<bool> {
        self.owner.bytes().map(|b| b == b'1').collect()
    }
    pub fn pb(&self) -> PB {
        self.power_bounds.unwrap_or_default()
    }
    pub fn phase_str(&self) -> &str {
        self.phase.as_deref().unwrap_or("")
    }
    pub fn stat(&self, key: &str) -> f64 {
        self.stats
            .as_ref()
            .and_then(|s| s.get(key))
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0)
    }
}

/// Stream every record of the top-level `domains` array (or every JSONL line).
pub fn for_each_record(path: &str, mut f: impl FnMut(Rec)) -> usize {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
    let reader = BufReader::with_capacity(1 << 24, file);
    let jsonl = path.ends_with(".jsonl");
    let mut n = 0usize;
    let mut buf = String::new();
    let mut in_domains = false;
    let mut in_rec = false;
    let mut parse = |text: &str, n: &mut usize| match serde_json::from_str::<Rec>(text) {
        Ok(r) => {
            *n += 1;
            f(r)
        }
        Err(e) => eprintln!("parse error {e}"),
    };
    for line in reader.lines() {
        let line = line.unwrap();
        if jsonl {
            if !line.trim().is_empty() {
                parse(&line, &mut n);
            }
            continue;
        }
        if !in_domains {
            if line.starts_with("  \"domains\": [") {
                in_domains = !line.ends_with("[]") && !line.ends_with("[],");
            }
            continue;
        }
        if !in_rec {
            if line == "    {" {
                in_rec = true;
                buf.clear();
                buf.push('{');
            } else if line.starts_with("  ]") {
                in_domains = false;
            }
        } else if line == "    }," || line == "    }" {
            buf.push('}');
            parse(&buf, &mut n);
            in_rec = false;
        } else {
            buf.push_str(line.trim());
        }
    }
    n
}

/// (A cap on local active sum, R cap), with the same implied caps as rtool.
pub fn limits(owner: &[bool], rank: Option<u32>, pb: PB) -> (Option<i64>, Option<i64>) {
    let t = owner.iter().filter(|&&b| b).count() as i64;
    let mut acap = pb.max_positive_power.map(|a| a as i64 - t);
    let mut rcap = rank.map(|r| r as i64);
    if acap.is_none() {
        if let (Some(d), Some(r)) = (pb.max_power_difference, rcap) {
            acap = Some(r + d - t);
        }
    }
    if rcap.is_none() {
        if let (Some(d), Some(a)) = (pb.min_power_difference, acap) {
            rcap = Some(a + t - d);
        }
    }
    (acap, rcap)
}

fn dist(bounds: &[(u64, Option<u64>)], cap: Option<i64>) -> Option<Vec<u128>> {
    let cap = match cap {
        Some(c) => c,
        None => {
            if bounds.iter().any(|b| b.1.is_none()) {
                return None;
            }
            bounds.iter().map(|b| b.1.unwrap() as i64).sum()
        }
    };
    if cap < 0 {
        return Some(vec![]);
    }
    let cap = cap as usize;
    let mut counts = vec![0u128; cap + 1];
    counts[0] = 1;
    for &(lo, hi) in bounds {
        let lo = lo as usize;
        let hi2 = hi.map_or(cap, |h| (h as usize).min(cap));
        let mut new = vec![0u128; cap + 1];
        if lo > hi2 {
            return Some(new);
        }
        let mut pre = vec![0u128; cap + 2];
        for s in 0..=cap {
            pre[s + 1] = pre[s] + counts[s];
        }
        for s in 0..=cap {
            if s < lo {
                continue;
            }
            let b = s - lo;
            let a = s.saturating_sub(hi2);
            new[s] = pre[b + 1] - pre[a];
        }
        counts = new;
    }
    Some(counts)
}

/// Exact lattice-point count; None if the domain is infinite.
pub fn points(owner: &[bool], lower: &[u64], upper: &[Option<u64>], rank: Option<u32>, pb: PB) -> Option<u128> {
    let t = owner.iter().filter(|&&b| b).count() as i64;
    let (acap, rcap) = limits(owner, rank, pb);
    let act: Vec<_> = (0..owner.len()).filter(|&i| owner[i]).map(|i| (lower[i], upper[i])).collect();
    let ina: Vec<_> = (0..owner.len()).filter(|&i| !owner[i]).map(|i| (lower[i], upper[i])).collect();
    let da = dist(&act, acap)?;
    let dr = dist(&ina, rcap)?;
    let mut total = 0u128;
    for (sa, &ca) in da.iter().enumerate() {
        if ca == 0 {
            continue;
        }
        let a = sa as i64 + t;
        for (sr, &cr) in dr.iter().enumerate() {
            if cr == 0 {
                continue;
            }
            let d = a - sr as i64;
            if pb.min_power_difference.is_some_and(|m| d < m) || pb.max_power_difference.is_some_and(|m| d > m) {
                continue;
            }
            total += ca * cr;
        }
    }
    Some(total)
}

/// Enumerate points packed 6 bits per coordinate (first axis most
/// significant). Returns false if the domain is infinite or a coordinate
/// reaches 63.
pub fn enumerate(owner: &[bool], lower: &[u64], upper: &[Option<u64>], rank: Option<u32>, pb: PB, out: &mut dyn FnMut(u128)) -> bool {
    let n = owner.len();
    let t = owner.iter().filter(|&&b| b).count() as i64;
    let (acap, rcap) = limits(owner, rank, pb);
    let mut hi = vec![0u64; n];
    for i in 0..n {
        let cap = if owner[i] { acap } else { rcap };
        hi[i] = match (upper[i], cap) {
            (Some(u), Some(c)) => u.min(c.max(0) as u64),
            (Some(u), None) => u,
            (None, Some(c)) => c.max(0) as u64,
            (None, None) => return false,
        };
        if hi[i] >= 63 {
            return false;
        }
    }
    if (0..n).any(|i| lower[i] > hi[i]) {
        return true;
    }
    let mut rest_a = vec![0i64; n + 1];
    let mut rest_r = vec![0i64; n + 1];
    for i in (0..n).rev() {
        rest_a[i] = rest_a[i + 1] + if owner[i] { lower[i] as i64 } else { 0 };
        rest_r[i] = rest_r[i + 1] + if owner[i] { 0 } else { lower[i] as i64 };
    }
    #[allow(clippy::too_many_arguments)]
    fn rec(
        i: usize, key: u128, sa: i64, sr: i64, n: usize, owner: &[bool], lower: &[u64], hi: &[u64],
        acap: Option<i64>, rcap: Option<i64>, t: i64, pb: PB, rest_a: &[i64], rest_r: &[i64], out: &mut dyn FnMut(u128),
    ) {
        if i == n {
            let d = sa + t - sr;
            if pb.min_power_difference.is_none_or(|m| d >= m) && pb.max_power_difference.is_none_or(|m| d <= m) {
                out(key);
            }
            return;
        }
        for v in lower[i]..=hi[i] {
            let (na, nr) = if owner[i] { (sa + v as i64, sr) } else { (sa, sr + v as i64) };
            if acap.is_some_and(|c| na + rest_a[i + 1] > c) || rcap.is_some_and(|c| nr + rest_r[i + 1] > c) {
                break;
            }
            rec(i + 1, (key << 6) | v as u128, na, nr, n, owner, lower, hi, acap, rcap, t, pb, rest_a, rest_r, out);
        }
    }
    rec(0, 0, 0, 0, n, owner, lower, &hi, acap, rcap, t, pb, &rest_a, &rest_r, out);
    true
}

/// Physical (A, R) of a packed point.
pub fn level_of(owner: &[bool], key: u128) -> (i64, i64) {
    let n = owner.len();
    let t = owner.iter().filter(|&&b| b).count() as i64;
    let (mut a, mut r) = (t, 0i64);
    for (i, &active) in owner.iter().enumerate() {
        let x = ((key >> (6 * (n - 1 - i))) & 63) as i64;
        if active {
            a += x
        } else {
            r += x
        }
    }
    (a, r)
}

pub fn binom(n: i64, k: i64) -> f64 {
    if k < 0 || n < k || n < 0 {
        return 0.0;
    }
    let mut r = 1.0f64;
    for i in 0..k {
        r = r * (n - i) as f64 / (i + 1) as f64;
    }
    r
}

/// Number of lattice points of the full (A = a, R = r) level of an owner.
pub fn level_size(owner: &[bool], a: i64, r: i64) -> f64 {
    let t = owner.iter().filter(|&&b| b).count() as i64;
    let u = owner.len() as i64 - t;
    let active = if t == 0 { if a == 0 { 1.0 } else { 0.0 } } else { binom(a - 1, t - 1) };
    let inactive = if u == 0 { if r == 0 { 1.0 } else { 0.0 } } else { binom(r + u - 1, u - 1) };
    active * inactive
}
