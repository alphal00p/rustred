//! The seed-search space of a case (plan 2.4.4 "THE SEED RUNG AS A SMALL SEARCH", D-41, D-43, 2.4.13; R21, R25,
//! R26, R29 (a), R37), attempt budgets that scale with the face (R34 (d), R38 oversized class) and the ONE mapping of
//! an abstract [`SeedSetting`] to engine argv ([`engine_seed_args`], EC-1 spelling).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::items::{CaseSpec, NodeParams};
use super::policy::{CentreSpec, SettingSpec, builtin_wave, face_type_of_case};

/// Which centre a setting uses (label; the offsets are authoritative).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CentreKind {
    Target,
    LineDown(usize),
    NumUp(usize),
    Pair { line: usize, num: usize },
    Combined,
    CombinedNum,
    /// A child's (or point's) fixed off-floor slot, one step towards its floor (H s.2).
    FixedSlot(usize),
    UnionLines,
    UnionPairs,
    Explicit,
}

impl CentreKind {
    pub fn spec(&self) -> CentreSpec {
        match self {
            CentreKind::Target | CentreKind::Explicit => CentreSpec::Target,
            CentreKind::LineDown(_) => CentreSpec::Line,
            CentreKind::NumUp(_) => CentreSpec::Num,
            CentreKind::Pair { .. } => CentreSpec::Pair,
            CentreKind::Combined => CentreSpec::Combined,
            CentreKind::CombinedNum => CentreSpec::CombinedNum,
            CentreKind::FixedSlot(_) => CentreSpec::Fixed,
            CentreKind::UnionLines => CentreSpec::UnionLines,
            CentreKind::UnionPairs => CentreSpec::UnionPairs,
        }
    }
}

/// One abstract seed setting: centres (offsets from the target; empty = the target; several = a union of seed sets),
/// floor filter on face lines, shell depth, tight seeds within the target's layer.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SeedSetting {
    pub centre: CentreKind,
    pub offsets: Vec<BTreeMap<usize, i32>>,
    pub floor: bool,
    pub depth: u8,
    pub tight: bool,
}

impl SeedSetting {
    pub fn target(depth: u8) -> Self {
        SeedSetting {
            centre: CentreKind::Target,
            offsets: vec![],
            floor: false,
            depth,
            tight: false,
        }
    }

    /// Canonical identity (offsets as a sorted set, floor, depth, tight); two settings with the same key are the
    /// same engine call.
    pub fn canonical_key(&self) -> String {
        let mut cs: Vec<String> = self
            .offsets
            .iter()
            .filter(|m| m.values().any(|&v| v != 0))
            .map(|m| {
                m.iter()
                    .filter(|(_, v)| **v != 0)
                    .map(|(s, v)| format!("{s}:{v}"))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect();
        cs.sort();
        cs.dedup();
        format!(
            "c=[{}];fl={};d={};t={}",
            cs.join(";"),
            u8::from(self.floor),
            self.depth,
            u8::from(self.tight)
        )
    }

    /// Short human label: "line:6/fl/d2", "union_lines(0,6)/unfl/d3".
    pub fn label(&self) -> String {
        let c = match &self.centre {
            CentreKind::Target => "target".to_string(),
            CentreKind::LineDown(x) => format!("line:{x}"),
            CentreKind::NumUp(j) => format!("num:{j}"),
            CentreKind::Pair { line, num } => format!("pair:{line}:{num}"),
            CentreKind::Combined => "combined".into(),
            CentreKind::CombinedNum => "combined_num".into(),
            CentreKind::FixedSlot(s) => format!("fixed:{s}"),
            CentreKind::UnionLines | CentreKind::UnionPairs | CentreKind::Explicit => {
                let slots: BTreeSet<usize> = self.offsets.iter().flat_map(|m| m.keys().copied()).collect();
                let name = match self.centre {
                    CentreKind::UnionLines => "union_lines",
                    CentreKind::UnionPairs => "union_pairs",
                    _ => "explicit",
                };
                format!(
                    "{name}({})",
                    slots.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(",")
                )
            }
        };
        format!(
            "{c}/{}/d{}{}",
            if self.floor { "fl" } else { "unfl" },
            self.depth,
            if self.tight { "/tight" } else { "" }
        )
    }

    /// The slots that carry a centre offset (for evidence records).
    pub fn slots(&self) -> Vec<usize> {
        let s: BTreeSet<usize> = self.offsets.iter().flat_map(|m| m.keys().copied()).collect();
        s.into_iter().collect()
    }
}

/// THE ONE mapping of a setting to engine argv (EC-1, notes/seed-centre.md): `--seed-centre-offset 'S:D[,..][;..]'`
/// (omitted for the target), `--seed-face-floor 1` (floor on only), `--max-depth 2|3`, and `--seed-tight-layer` for
/// tight seeds (requested from the engine; not an engine flag yet, R37).
pub fn engine_seed_args(s: &SeedSetting) -> Vec<String> {
    let mut a = Vec::new();
    let centres: Vec<String> = s
        .offsets
        .iter()
        .filter(|m| m.values().any(|&v| v != 0))
        .map(|m| {
            m.iter()
                .filter(|(_, v)| **v != 0)
                .map(|(slot, d)| format!("{slot}:{d}"))
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect();
    if !centres.is_empty() {
        a.push("--seed-centre-offset".into());
        a.push(centres.join(";"));
    }
    if s.floor {
        a.push("--seed-face-floor".into());
        a.push("1".into());
    }
    a.push("--max-depth".into());
    a.push(s.depth.to_string());
    if s.tight {
        a.push("--seed-tight-layer".into());
    }
    a
}

/// Instances of one policy entry on a case.
fn expand(spec: &SettingSpec, depth: u8, case: &CaseSpec, params: &NodeParams, class_tight: bool) -> Vec<SeedSetting> {
    let lines = &params.lines;
    let free = case.free();
    let lines_free: Vec<usize> = free.iter().copied().filter(|&s| lines[s] && case.face.sd.contains(&s)).collect();
    let nums_free: Vec<usize> = free.iter().copied().filter(|&s| !lines[s] && case.face.sn.contains(&s)).collect();
    let fixed_off = case.fixed_off_floor(lines);
    let toward = |s: usize| if lines[s] { -1 } else { 1 };
    let face_line = !lines_free.is_empty() || fixed_off.iter().any(|&s| lines[s]);
    let floor = spec.floor.unwrap_or(true) && face_line;
    let tight = spec.tight.unwrap_or(class_tight);
    let one = |m: &[(usize, i32)]| -> BTreeMap<usize, i32> { m.iter().copied().collect() };
    let mk = |centre: CentreKind, offsets: Vec<BTreeMap<usize, i32>>, floor: bool| SeedSetting {
        centre,
        offsets,
        floor,
        depth,
        tight,
    };
    let mut out = Vec::new();
    match spec.centre {
        CentreSpec::Target => out.push(mk(CentreKind::Target, vec![], false)),
        CentreSpec::Line => {
            for &x in &lines_free {
                out.push(mk(CentreKind::LineDown(x), vec![one(&[(x, -1)])], floor));
            }
        }
        CentreSpec::Fixed => {
            for &x in &fixed_off {
                out.push(mk(CentreKind::FixedSlot(x), vec![one(&[(x, toward(x))])], floor));
            }
        }
        CentreSpec::Num => {
            for &j in &nums_free {
                out.push(mk(CentreKind::NumUp(j), vec![one(&[(j, 1)])], floor));
            }
        }
        CentreSpec::Pair => {
            for &x in &lines_free {
                for &j in &nums_free {
                    out.push(mk(CentreKind::Pair { line: x, num: j }, vec![one(&[(x, -1), (j, 1)])], floor));
                }
            }
        }
        CentreSpec::Combined => {
            let mut m = BTreeMap::new();
            for &x in lines_free.iter().chain(&nums_free).chain(&fixed_off) {
                m.insert(x, toward(x));
            }
            if m.len() >= 2 {
                out.push(mk(CentreKind::Combined, vec![m], floor));
            }
        }
        CentreSpec::CombinedNum => {
            let mut m = BTreeMap::new();
            for &x in nums_free.iter().chain(fixed_off.iter().filter(|&&s| !lines[s])) {
                m.insert(x, 1);
            }
            if m.len() >= 2 {
                out.push(mk(CentreKind::CombinedNum, vec![m], floor));
            }
        }
        CentreSpec::UnionLines => {
            let cs: Vec<BTreeMap<usize, i32>> = lines_free
                .iter()
                .chain(fixed_off.iter().filter(|&&s| lines[s]))
                .map(|&x| one(&[(x, -1)]))
                .collect();
            if cs.len() >= 2 {
                out.push(mk(CentreKind::UnionLines, cs, floor));
            }
        }
        CentreSpec::UnionPairs => {
            let mut cs = Vec::new();
            for &x in &lines_free {
                for &j in &nums_free {
                    cs.push(one(&[(x, -1), (j, 1)]));
                }
            }
            if cs.len() >= 2 {
                out.push(mk(CentreKind::UnionPairs, cs, floor));
            }
        }
    }
    out
}

/// The seed-search space of a case, as WAVES (wave 0 = depth 2, wave 1 = depth 3): the policy's ordered entries for
/// the case's class, then (completeness rule) every built-in entry not yet present, duplicates removed, then a
/// stable reorder by the policy's evidence priors (never a removal). Finite and known when the item is created: its
/// total length is the seed-search sub-counter (D-43).
pub fn seed_space(case: &CaseSpec, params: &NodeParams) -> Vec<Vec<SeedSetting>> {
    let face_type = face_type_of_case(case);
    let class = params.policy.class_for(&params.node_class, &face_type);
    let priors = params.policy.priors(&face_type);
    let mut seen = BTreeSet::new();
    let mut waves = Vec::new();
    for depth in [2u8, 3u8] {
        let mut wave = Vec::new();
        let listed: Vec<&SettingSpec> = class.waves.iter().filter(|w| w.depth == depth).flat_map(|w| &w.settings).collect();
        let builtin = builtin_wave(depth);
        for spec in listed.into_iter().chain(builtin.settings.iter()) {
            for st in expand(spec, depth, case, params, class.tight) {
                if seen.insert(st.canonical_key()) {
                    wave.push(st);
                }
            }
        }
        if !priors.is_empty() {
            let rank = |st: &SeedSetting| -> f64 {
                priors.get(&(st.centre.spec(), st.floor, st.depth)).copied().unwrap_or(f64::INFINITY)
            };
            wave.sort_by(|a, b| rank(a).partial_cmp(&rank(b)).unwrap_or(std::cmp::Ordering::Equal));
        }
        waves.push(wave);
    }
    waves
}

/// Budget class of an attempt (R38: oversized rules get their own class).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BudgetClass {
    Standard,
    Deep,
    Point,
    Oversized,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttemptBudget {
    pub seconds: u64,
    pub mem_mb: u64,
    /// Work counter (preferred over wall time, D-54): estimated seeds x IBP rows per seed.
    pub max_rows: u64,
    pub class: BudgetClass,
}

fn binom(n: u64, k: u64) -> u64 {
    let mut r: u64 = 1;
    for i in 0..k {
        r = r.saturating_mul(n - i) / (i + 1);
    }
    r
}

/// Estimated seed count of a setting: per centre, all raising shells up to the depth (sum_k C(N+k-1, k) for arity N)
/// plus the lowering shells (~2N); times the number of centres of a union.
pub fn estimated_seeds(s: &SeedSetting, arity: usize) -> u64 {
    let n = arity as u64;
    let raises: u64 = (0..=s.depth as u64).map(|k| binom(n + k - 1, k)).sum();
    (raises + 2 * n) * (s.offsets.len().max(1) as u64)
}

/// Attempt budget: seconds = base(depth) x max(1, estimated seeds / `scale_reference_seeds`), capped (R34 (d)); memory by depth; the
/// point and oversized classes have their own figures.
pub fn attempt_budget(s: &SeedSetting, params: &NodeParams, point: bool, oversized: bool) -> AttemptBudget {
    let b = &params.budgets;
    let est = estimated_seeds(s, params.arity);
    let max_rows = est.saturating_mul(params.ibp_rows_per_seed as u64);
    if oversized {
        return AttemptBudget {
            seconds: b.oversized_seconds,
            mem_mb: b.oversized_mem_mb,
            max_rows,
            class: BudgetClass::Oversized,
        };
    }
    let (base, mem, class) = if point {
        (b.point_seconds, b.point_mem_mb, BudgetClass::Point)
    } else if s.depth >= 3 {
        (b.base_seconds_d3, b.mem_mb_d3, BudgetClass::Deep)
    } else {
        (b.base_seconds_d2, b.mem_mb_d2, BudgetClass::Standard)
    };
    let r = b
        .scale_reference_seeds
        .unwrap_or_else(|| {
            let one = SeedSetting {
                offsets: vec![BTreeMap::from([(0usize, -1i32)])],
                ..SeedSetting::target(3)
            };
            estimated_seeds(&one, params.arity)
        })
        .max(1);
    let scaled = if est > r { base.saturating_mul(est) / r } else { base };
    AttemptBudget {
        seconds: scaled.min(b.cap_seconds),
        mem_mb: mem,
        max_rows,
        class,
    }
}

#[cfg(test)]
mod tests {
    use super::super::items::Face;
    use super::*;

    #[test]
    fn single_line_face_has_six_settings() {
        let p = NodeParams::new("T178", 20, 10);
        let c = CaseSpec::face_case(Face::new(vec![6], vec![]));
        let w = seed_space(&c, &p);
        let n: usize = w.iter().map(|x| x.len()).sum();
        assert_eq!(n, 6, "{:?}", w.iter().flatten().map(|s| s.label()).collect::<Vec<_>>());
        assert_eq!(w[0][1].label(), "line:6/fl/d2");
        assert_eq!(
            engine_seed_args(&w[0][1]),
            vec!["--seed-centre-offset", "6:-1", "--seed-face-floor", "1", "--max-depth", "2"]
        );
    }

    #[test]
    fn union_and_pairs_present() {
        let p = NodeParams::new("T178", 20, 10);
        let c = CaseSpec::face_case(Face::new(vec![0, 6], vec![11]));
        let all: Vec<String> = seed_space(&c, &p).into_iter().flatten().map(|s| s.label()).collect();
        assert!(all.contains(&"union_lines(0,6)/fl/d2".to_string()));
        assert!(all.contains(&"union_lines(0,6)/unfl/d3".to_string()));
        assert!(all.contains(&"pair:0:11/fl/d2".to_string()));
        assert!(all.contains(&"combined/fl/d2".to_string()));
        assert!(all.contains(&"union_pairs(0,6,11)/fl/d2".to_string()));
        let u = seed_space(&c, &p)[0].iter().find(|s| s.centre == CentreKind::UnionLines).cloned().unwrap();
        assert_eq!(engine_seed_args(&u)[1], "0:-1;6:-1");
        let b = attempt_budget(&u, &p, false, false);
        assert_eq!(b.seconds, 300);
    }
}
