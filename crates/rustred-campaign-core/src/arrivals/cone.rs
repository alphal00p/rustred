//! ARRIVAL CONES, the parts that need no walk (lane arrivals; reports/ARRIVAL-CONES.md; interfaces/arrivals.md section 9).
//!
//! Measures on a key of a node frame (lines L, numerators N): D = sum_{i in L} (k_i - 1) (dots), r = sum_{i in N} (-k_i)
//! (rank); top sector = every line >= 1, every numerator <= 0. A rule term in a SUB-sector (some lines <= 0) counts its
//! dropped lines as numerators: D + r(sub) = sum_{kept lines} (v - 1) + sum_{dropped lines} (-v) + sum_N (-v).
//!
//! 1. HAND-DOWN EXCESS of a rule term = D + r(sub) of the term - D + r of the target, maximised EXACTLY over the record's
//!    rule-point range for each DROP MASK (the set of term lines <= 0). With target and term slots of the form
//!    `n_p + o` or a constant, the excess is a sum of one function per rule-point parameter (a slot's contribution depends
//!    on one parameter only); with the drop mask fixed every slot is in one regime, each per-parameter function is LINEAR
//!    on an interval (the parameter's chart range, fixed slots, the regime constraints of its slots), so its maximum is at
//!    an end point (or +inf: unbounded). Equations and exception branches only shrink the domain: ignored (the bound stays
//!    valid, possibly loose). Positive-numerator terms are not hand-downs (they need a zero coefficient) and are skipped.
//!    Mask 0 = a TOP term: its excess <= 0 everywhere is the statement "top terms never raise D + r" (closure inside a node
//!    stays in D + r <= S).
//! 2. EDGE MAPS: a routed sector maps every kept parent line onto a child line (index carried over) and every other parent
//!    slot onto a linear form in child slots + a q^2 constant. A top-sector child key needs every child line hit by a kept
//!    parent line (a child line without one starts at 0 and can only go lower); then child D = D(kept) - #line factors and
//!    child r = #numerator factors, out of r(sub) factors: child D + r <= D + r(sub) of the parent term. `edge_image`
//!    states this per sector (checked) with the child slots that can receive numerator powers and the lines a form lowers.
//! 3. ARRIVAL STATISTICS (`DrStats`): D, r, D + r of the keys of an arrival file (the cross-check of a cone).
//!
//! Nothing here names a node, a count or a frame (L8).

use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::closure::{RhsSlot, parse_rhs_pattern};

use super::edge::{EdgeFile, SlotSub};

/// One rule record's patterns (coefficients not read).
#[derive(Clone, Debug)]
pub struct RulePat {
    pub index: i64,
    /// sha256 (hex) of the raw record line (the zero proofs' `record_sha256`).
    pub sha256: String,
    /// The record's chart ('1' = line slot) as written; compared with the frame.
    pub chart: Vec<bool>,
    pub fixed: Vec<Option<i64>>,
    pub target: Vec<RhsSlot>,
    pub terms: Vec<Vec<RhsSlot>>,
}

#[derive(Deserialize)]
struct LineIn {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    mask: Option<String>,
    #[serde(default)]
    index: Option<i64>,
    #[serde(default)]
    chart: Option<String>,
    #[serde(default)]
    fixed: Option<Vec<Option<i64>>>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    rhs: Option<Vec<(String, serde::de::IgnoredAny)>>,
}

fn bits(s: &str) -> Vec<bool> {
    s.bytes().map(|b| b == b'1').collect()
}

/// Read the frame (header `mask`) and every rule record's patterns of a table file (JSON lines), streaming; the
/// coefficient strings are skipped, not kept.
pub fn read_rules(path: &Path) -> Result<(Vec<bool>, Vec<RulePat>), String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut frame: Option<Vec<bool>> = None;
    let mut out = vec![];
    for (i, l) in std::io::BufReader::with_capacity(1 << 22, f)
        .lines()
        .enumerate()
    {
        let l = l.map_err(|e| format!("{}: {e}", path.display()))?;
        if l.trim().is_empty() {
            continue;
        }
        let at = || format!("{} line {}", path.display(), i + 1);
        let v: LineIn = serde_json::from_str(&l).map_err(|e| format!("{}: {e}", at()))?;
        match v.kind.as_str() {
            "header" => {
                frame = Some(bits(
                    v.mask
                        .as_deref()
                        .ok_or_else(|| format!("{}: header without mask", at()))?,
                ));
            }
            "rule" => {
                let fr = frame
                    .as_ref()
                    .ok_or_else(|| format!("{}: a rule before the header", at()))?;
                let n = fr.len();
                let chart = v.chart.as_deref().map(bits).unwrap_or_else(|| fr.clone());
                let fixed = v.fixed.unwrap_or_else(|| vec![None; n]);
                let target = parse_rhs_pattern(
                    v.target
                        .as_deref()
                        .ok_or_else(|| format!("{}: no target", at()))?,
                    n,
                )
                .map_err(|e| format!("{}: target: {e}", at()))?;
                let mut terms = vec![];
                for (p, _) in v.rhs.unwrap_or_default() {
                    terms
                        .push(parse_rhs_pattern(&p, n).map_err(|e| format!("{}: rhs: {e}", at()))?);
                }
                if fixed.len() != n || chart.len() != n {
                    return Err(format!("{}: fixed / chart of the wrong length", at()));
                }
                let mut h = crate::closure::sha256::Sha256::new();
                h.update(l.as_bytes());
                out.push(RulePat {
                    index: v.index.unwrap_or(-1),
                    sha256: h.finish_hex(),
                    chart,
                    fixed,
                    target,
                    terms,
                });
            }
            _ => {}
        }
    }
    Ok((
        frame.ok_or_else(|| format!("{}: no header", path.display()))?,
        out,
    ))
}

/// An inclusive integer interval; `None` ends are infinite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Iv {
    lo: Option<i64>,
    hi: Option<i64>,
}

impl Iv {
    const ALL: Iv = Iv { lo: None, hi: None };
    fn meet(self, o: Iv) -> Iv {
        Iv {
            lo: match (self.lo, o.lo) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            },
            hi: match (self.hi, o.hi) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            },
        }
    }
    fn empty(self) -> bool {
        matches!((self.lo, self.hi), (Some(a), Some(b)) if a > b)
    }
    fn ge(v: i64) -> Iv {
        Iv {
            lo: Some(v),
            hi: None,
        }
    }
    fn le(v: i64) -> Iv {
        Iv {
            lo: None,
            hi: Some(v),
        }
    }
    fn at(v: i64) -> Iv {
        Iv {
            lo: Some(v),
            hi: Some(v),
        }
    }
}

/// The excess of one (term, drop mask): Some(bound) or None = unbounded on the domain.
pub type Excess = Option<i64>;

/// Per term: every feasible drop mask (term lines <= 0; mask 0 = a top term) with the exact maximum excess on the record's
/// rule-point range, or Err for a target the bound does not handle (described).
pub fn term_excess(
    rule: &RulePat,
    term: &[RhsSlot],
    lines: &[bool],
) -> Result<Vec<(u64, Excess)>, String> {
    let n = lines.len();
    if term.len() != n || rule.target.len() != n || n > 64 {
        return Err("arity".into());
    }
    // the rule-point parameter ranges: the target slots in the record's chart (line >= 1, numerator <= 0), fixed slots
    let mut base = vec![Iv::ALL; n];
    let mut used = vec![false; n];
    let mut konst = 0i64; // constant part of -(target D + r)
    for (s, t) in rule.target.iter().enumerate() {
        let line = lines[s];
        match *t {
            RhsSlot::Param { param, offset } => {
                let o = i64::from(offset);
                used[param] = true;
                base[param] = base[param].meet(if rule.chart[s] {
                    Iv::ge(1 - o)
                } else {
                    Iv::le(-o)
                });
            }
            RhsSlot::Const(c) => {
                let c = i64::from(c);
                if (rule.chart[s] && c < 1) || (!rule.chart[s] && c > 0) {
                    return Ok(vec![]); // the record's target is outside its chart: no key
                }
                // contribution -(c - 1) for a line, -(-c) for a numerator
                konst -= if line { c - 1 } else { -c };
            }
        }
    }
    for (p, f) in rule.fixed.iter().enumerate() {
        if let Some(v) = f {
            base[p] = base[p].meet(Iv::at(*v));
        }
    }
    // a parameter the target does not carry is fixed by the domain (rule point: fixed value or 0)
    for p in 0..n {
        if !used[p] {
            base[p] = base[p].meet(Iv::at(rule.fixed[p].unwrap_or(0)));
        }
    }
    if base.iter().any(|b| b.empty()) {
        return Ok(vec![]);
    }
    // the target's per-parameter slope and constant: -(line: x + o - 1 | numerator: -(x + o))
    let mut slope = vec![0i64; n];
    for (s, t) in rule.target.iter().enumerate() {
        if let RhsSlot::Param { param, offset } = *t {
            let o = i64::from(offset);
            if lines[s] {
                slope[param] -= 1;
                konst -= o - 1;
            } else {
                slope[param] += 1;
                konst += o;
            }
        }
    }
    // term line slots: forced kept / forced dropped / free (both regimes possible on its own)
    let mut free = vec![];
    let mut forced_drop = 0u64;
    for (s, t) in term.iter().enumerate() {
        match *t {
            RhsSlot::Param { param, offset } => {
                let o = i64::from(offset);
                let r = base[param];
                if lines[s] {
                    let can_drop = !r.meet(Iv::le(-o)).empty();
                    let can_keep = !r.meet(Iv::ge(1 - o)).empty();
                    match (can_drop, can_keep) {
                        (true, true) => free.push(s),
                        (true, false) => forced_drop |= 1 << s,
                        (false, true) => {}
                        (false, false) => return Ok(vec![]),
                    }
                } else if r.meet(Iv::le(-o)).empty() {
                    return Ok(vec![]); // a numerator always positive: never a hand-down nor a top term
                }
            }
            RhsSlot::Const(c) => {
                if lines[s] {
                    if c <= 0 {
                        forced_drop |= 1 << s;
                    }
                } else if c > 0 {
                    return Ok(vec![]); // positive numerator everywhere
                }
            }
        }
    }
    if free.len() > 16 {
        return Err(format!("{} free line slots in one term", free.len()));
    }
    let mut out = vec![];
    for combo in 0u64..(1u64 << free.len()) {
        let mut mask = forced_drop;
        for (b, &s) in free.iter().enumerate() {
            if combo >> b & 1 == 1 {
                mask |= 1 << s;
            }
        }
        // per parameter: interval and slope under this mask; constants
        let mut iv = base.clone();
        let mut sl = slope.clone();
        let mut k = konst;
        let mut feasible = true;
        for (s, t) in term.iter().enumerate() {
            let dropped = mask >> s & 1 == 1;
            match *t {
                RhsSlot::Param { param, offset } => {
                    let o = i64::from(offset);
                    if lines[s] && !dropped {
                        iv[param] = iv[param].meet(Iv::ge(1 - o));
                        sl[param] += 1; // x + o - 1
                        k += o - 1;
                    } else {
                        // dropped line or numerator: value <= 0, contribution -(x + o)
                        iv[param] = iv[param].meet(Iv::le(-o));
                        sl[param] -= 1;
                        k -= o;
                    }
                }
                RhsSlot::Const(c) => {
                    let c = i64::from(c);
                    k += if lines[s] && !dropped { c - 1 } else { -c };
                }
            }
            if let RhsSlot::Param { param, .. } = *t
                && iv[param].empty()
            {
                feasible = false;
                break;
            }
        }
        if !feasible {
            continue;
        }
        let mut total: Excess = Some(k);
        for p in 0..n {
            if sl[p] == 0 {
                continue;
            }
            let end = if sl[p] > 0 { iv[p].hi } else { iv[p].lo };
            total = match (total, end) {
                (Some(t), Some(x)) => Some(t + sl[p] * x),
                _ => None,
            };
        }
        out.push((mask, total));
    }
    Ok(out)
}

/// The drop planes of a term under a drop mask: per dropped line slot, (slot, rule-point parameter, lowest value, highest
/// value) of the parameter where that slot is <= 0 on the record's range (the hand-down locus of zero-proofs.md section 6
/// is the union of the planes n_p = v of each dropped slot); a constant dropped slot is `None` (the whole domain).
pub type DropPlanes = Vec<(usize, Option<(usize, Option<i64>, Option<i64>)>)>;

/// The terms of a record with one of `masks` feasible, with the drop planes of each such mask.
pub fn terms_with_masks(
    rule: &RulePat,
    lines: &[bool],
    masks: &std::collections::BTreeSet<u64>,
) -> Vec<(usize, u64, DropPlanes)> {
    let n = lines.len();
    let mut out = vec![];
    for (j, t) in rule.terms.iter().enumerate() {
        let Ok(ms) = term_excess(rule, t, lines) else {
            continue;
        };
        for (m, _) in ms {
            if !masks.contains(&m) {
                continue;
            }
            // the parameter range from the target (chart) and fixed slots, as in term_excess
            let mut base = vec![Iv::ALL; n];
            let mut used = vec![false; n];
            for (s, ts) in rule.target.iter().enumerate() {
                if let RhsSlot::Param { param, offset } = *ts {
                    let o = i64::from(offset);
                    used[param] = true;
                    base[param] = base[param].meet(if rule.chart[s] {
                        Iv::ge(1 - o)
                    } else {
                        Iv::le(-o)
                    });
                }
            }
            for (q, f) in rule.fixed.iter().enumerate() {
                if let Some(v) = f {
                    base[q] = base[q].meet(Iv::at(*v));
                }
            }
            for q in 0..n {
                if !used[q] {
                    base[q] = base[q].meet(Iv::at(rule.fixed[q].unwrap_or(0)));
                }
            }
            let mut planes: DropPlanes = vec![];
            for (s, ts) in t.iter().enumerate() {
                if m >> s & 1 == 0 {
                    continue;
                }
                match *ts {
                    RhsSlot::Param { param, offset } => {
                        let r = base[param].meet(Iv::le(-i64::from(offset)));
                        planes.push((s, Some((param, r.lo, r.hi))));
                    }
                    RhsSlot::Const(_) => planes.push((s, None)),
                }
            }
            out.push((j, m, planes));
        }
    }
    out
}

/// Per node (the union of its tables' rule records): per drop mask the max excess and the number of (record, term) pairs,
/// the top terms' max excess (mask 0), the largest feasible mask, records with a chart other than the frame, terms the
/// bound does not handle.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct NodeExcess {
    pub records: u64,
    pub terms: u64,
    /// mask (as a bit string, slot 0 first) -> (max excess or null = unbounded, terms with that mask feasible)
    pub per_mask: BTreeMap<String, (Excess, u64)>,
    pub top_max: Excess,
    pub top_terms: u64,
    pub max_lines_dropped: u32,
    pub handdown_max: Excess,
    pub handdown_pairs: u64,
    pub chart_differs: u64,
    pub undecided: Vec<String>,
}

fn emax(a: Excess, b: Excess) -> Excess {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        _ => None,
    }
}

pub fn mask_string(m: u64, n: usize) -> String {
    (0..n)
        .map(|i| if m >> i & 1 == 1 { '1' } else { '0' })
        .collect()
}

impl NodeExcess {
    pub fn add_rule(&mut self, rule: &RulePat, lines: &[bool]) {
        self.records += 1;
        if rule.chart != lines {
            self.chart_differs += 1;
        }
        let n = lines.len();
        for (j, t) in rule.terms.iter().enumerate() {
            self.terms += 1;
            match term_excess(rule, t, lines) {
                Err(e) => {
                    if self.undecided.len() < 50 {
                        self.undecided
                            .push(format!("record {} term {j}: {e}", rule.index));
                    }
                }
                Ok(ms) => {
                    for (m, x) in ms {
                        if m == 0 {
                            self.top_terms += 1;
                            self.top_max = if self.top_terms == 1 {
                                x
                            } else {
                                emax(self.top_max, x)
                            };
                            continue;
                        }
                        self.max_lines_dropped = self.max_lines_dropped.max(m.count_ones());
                        let e = self.per_mask.entry(mask_string(m, n)).or_insert((x, 0));
                        e.0 = emax(e.0, x);
                        e.1 += 1;
                        self.handdown_pairs += 1;
                        self.handdown_max = if self.handdown_pairs == 1 {
                            x
                        } else {
                            emax(self.handdown_max, x)
                        };
                    }
                }
            }
        }
    }

    /// The max excess over the given drop masks (bit strings); `Some(i64::MIN)` = no term has any of them (empty route).
    pub fn max_over(&self, masks: &[String]) -> Excess {
        let mut m: Excess = Some(i64::MIN);
        for k in masks {
            if let Some((x, _)) = self.per_mask.get(k) {
                m = if m == Some(i64::MIN) { *x } else { emax(m, *x) };
            }
        }
        m
    }
}

/// The child-side description of one routed sector's image (section 2 of the module docs).
#[derive(Clone, Debug, serde::Serialize)]
pub struct SectorImage {
    pub dropped_mask: String,
    /// kept parent line -> child line
    pub line_map: Vec<(usize, usize)>,
    /// child lines hit by no kept parent line (non-empty: the sector sends nothing to the child's top sector)
    pub child_lines_unhit: Vec<usize>,
    /// child numerator slots some form can raise (others stay 0 in a direct arrival)
    pub numerators_reachable: Vec<usize>,
    /// child lines some form can lower (others keep the parent line's index)
    pub lines_lowerable: Vec<usize>,
    pub q2_constant: bool,
    /// child D + r <= D + r(sub) of the parent term holds by the structure (kept lines -> distinct child lines, linear forms)
    pub bound_holds: bool,
    pub problems: Vec<String>,
}

/// The images of the routed sectors of an edge file.
pub fn edge_image(e: &EdgeFile) -> Vec<SectorImage> {
    let child_lines: Vec<usize> = e.child.table_lines.clone();
    let cn = e.child.momenta.len();
    let is_line = |j: usize| child_lines.contains(&j);
    let mut out = vec![];
    for s in &e.sectors {
        let mut line_map = vec![];
        let mut reach = std::collections::BTreeSet::new();
        let mut lower = std::collections::BTreeSet::new();
        let mut q2 = false;
        let mut problems = vec![];
        let mut hit = std::collections::BTreeSet::new();
        for (t, sub) in s.subst.iter().enumerate() {
            match sub {
                SlotSub::Line(j) => {
                    if !s.kept.contains(&t) {
                        problems.push(format!("parent slot {t} maps to a line but is not kept"));
                    }
                    if !is_line(*j) {
                        problems.push(format!(
                            "parent line {t} maps to child slot {j}, not a line"
                        ));
                    }
                    if !hit.insert(*j) {
                        problems.push(format!("child line {j} hit twice"));
                    }
                    line_map.push((t, *j));
                }
                SlotSub::Form { coefs, q2: c } => {
                    if s.kept.contains(&t) {
                        problems.push(format!("kept parent line {t} maps to a form"));
                    }
                    q2 |= c.num != 0;
                    for (j, _) in coefs {
                        if *j >= cn {
                            problems.push(format!("form slot {j} outside the child's {cn} slots"));
                        } else if is_line(*j) {
                            lower.insert(*j);
                        } else {
                            reach.insert(*j);
                        }
                    }
                }
            }
        }
        let unhit: Vec<usize> = child_lines
            .iter()
            .copied()
            .filter(|j| !hit.contains(j))
            .collect();
        out.push(SectorImage {
            dropped_mask: format!("{:#x}", s.dropped_mask),
            line_map,
            child_lines_unhit: unhit,
            numerators_reachable: reach.into_iter().collect(),
            lines_lowerable: lower.into_iter().collect(),
            q2_constant: q2,
            bound_holds: problems.is_empty(),
            problems,
        });
    }
    out
}

/// D, r and D + r statistics of a set of keys of a frame (lines given): count, maxima, the histogram of D + r, keys outside
/// the top sector.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct DrStats {
    pub keys: u64,
    pub max_d: i64,
    pub max_r: i64,
    pub max_dr: i64,
    pub hist_dr: BTreeMap<i64, u64>,
    pub outside_top_sector: u64,
}

impl DrStats {
    pub fn add(&mut self, k: &[i32], lines: &[bool]) {
        let (mut d, mut r, mut out) = (0i64, 0i64, false);
        for (i, &v) in k.iter().enumerate() {
            let v = i64::from(v);
            if lines.get(i).copied().unwrap_or(false) {
                out |= v < 1;
                d += v - 1;
            } else {
                out |= v > 0;
                r -= v;
            }
        }
        self.keys += 1;
        self.outside_top_sector += out as u64;
        self.max_d = self.max_d.max(d);
        self.max_r = self.max_r.max(r);
        self.max_dr = self.max_dr.max(d + r);
        *self.hist_dr.entry(d + r).or_default() += 1;
    }

    /// Keys with D + r > s.
    pub fn above(&self, s: i64) -> u64 {
        self.hist_dr.range(s + 1..).map(|(_, c)| c).sum()
    }
}

/// Number of top-sector keys of a frame with `l` lines and `m` numerators and D + r = t: C(t + l + m - 1, l + m - 1).
pub fn simplex_layer(l: u64, m: u64, t: u64) -> f64 {
    let k = l + m;
    if k == 0 {
        return if t == 0 { 1.0 } else { 0.0 };
    }
    // C(t + k - 1, k - 1) in floating point (counts can exceed u64)
    let (n, r) = (t + k - 1, k - 1);
    let r = r.min(n - r);
    let mut c = 1.0f64;
    for i in 0..r {
        c = c * (n - i) as f64 / (i + 1) as f64;
    }
    c
}

/// The JSON of a cone in lane parametric's form (`Cone { constraints: [(a, b)] }`, sum_i a_i k_i <= b on the top-sector
/// orthant): D + r <= s.
pub fn simplex_cone_json(lines: &[bool], s: i64) -> Value {
    let a: Vec<i64> = lines.iter().map(|&l| if l { 1 } else { -1 }).collect();
    let nl = lines.iter().filter(|&&l| l).count() as i64;
    json!({"label": format!("D + r <= {s}"), "constraints": [[a, s + nl]]})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(target: &str, terms: &[&str], lines: &[bool], fixed: Vec<Option<i64>>) -> RulePat {
        let n = lines.len();
        RulePat {
            index: 0,
            sha256: String::new(),
            chart: lines.to_vec(),
            fixed,
            target: parse_rhs_pattern(target, n).unwrap(),
            terms: terms
                .iter()
                .map(|t| parse_rhs_pattern(t, n).unwrap())
                .collect(),
        }
    }

    /// Brute force over a box of rule points: the max excess per mask among keys where the record applies.
    fn brute(r: &RulePat, lines: &[bool], lo: i64, hi: i64) -> BTreeMap<(usize, u64), i64> {
        let n = lines.len();
        let mut out: BTreeMap<(usize, u64), i64> = BTreeMap::new();
        let mut pt = vec![lo; n];
        loop {
            // canonical target only in these tests: key = pt
            let in_chart = (0..n).all(|s| if lines[s] { pt[s] >= 1 } else { pt[s] <= 0 });
            let fixed_ok = (0..n).all(|s| r.fixed[s].is_none_or(|v| pt[s] == v));
            if in_chart && fixed_ok {
                let dr = |k: &[i64]| -> i64 {
                    (0..n)
                        .map(|s| {
                            if lines[s] {
                                if k[s] >= 1 { k[s] - 1 } else { -k[s] }
                            } else {
                                -k[s]
                            }
                        })
                        .sum()
                };
                let base = dr(&pt);
                for (j, t) in r.terms.iter().enumerate() {
                    let v: Vec<i64> = t
                        .iter()
                        .map(|x| match *x {
                            RhsSlot::Param { param, offset } => pt[param] + i64::from(offset),
                            RhsSlot::Const(c) => i64::from(c),
                        })
                        .collect();
                    if (0..n).any(|s| !lines[s] && v[s] > 0) {
                        continue;
                    }
                    let m: u64 = (0..n)
                        .filter(|&s| lines[s] && v[s] <= 0)
                        .map(|s| 1u64 << s)
                        .sum();
                    let e = dr(&v) - base;
                    let x = out.entry((j, m)).or_insert(i64::MIN);
                    *x = (*x).max(e);
                }
            }
            let mut i = 0;
            loop {
                if i == n {
                    return out;
                }
                pt[i] += 1;
                if pt[i] <= hi {
                    break;
                }
                pt[i] = lo;
                i += 1;
            }
        }
    }

    #[test]
    fn excess_is_exact_on_bounded_domains_and_matches_brute_force() {
        // 3 lines + 1 numerator; the domain is bounded by fixing / chart where the brute force box covers it
        let lines = [true, true, true, false];
        let cases: Vec<(Vec<Option<i64>>, Vec<&str>)> = vec![
            (
                vec![None, Some(2), None, Some(-1)],
                vec![
                    "I(n0-1,n1,n2,n3)",
                    "I(n0,n1-2,n2-1,n3)",
                    "I(n0-2,n1-1,n2+1,n3+1)",
                    "I(n0,n1,n2,n3-1)",
                ],
            ),
            (
                vec![Some(1), Some(3), Some(2), None],
                vec![
                    "I(n0-1,n1-3,n2,n3)",
                    "I(0,n1,n2-2,n3)",
                    "I(n1,n0,n2-1,n3)",
                    "I(n0+1,n1-1,n2,n3)",
                ],
            ),
        ];
        for (fixed, terms) in cases {
            // bound the free parameters for the brute force: chart + an explicit box via fixed where None stays free;
            // with free unbounded parameters only the masks whose maximum is attained inside the box are compared
            let r = rule("I(n0,n1,n2,n3)", &terms, &lines, fixed);
            let b = brute(&r, &lines, -4, 6);
            for (j, t) in r.terms.iter().enumerate() {
                for (m, x) in term_excess(&r, t, &lines).unwrap() {
                    let got = b.get(&(j, m)).copied();
                    match x {
                        Some(v) => {
                            // a bounded maximum is attained at an end point inside the brute-force box here
                            assert_eq!(got, Some(v), "term {j} mask {m:b}: brute {got:?} vs {v}");
                        }
                        None => {
                            // unbounded: the brute force grows with the box
                            assert!(got.is_some(), "term {j} mask {m:b} feasible");
                        }
                    }
                }
                // every mask the brute force saw is reported (no feasible mask missed)
                let ms: Vec<u64> = term_excess(&r, t, &lines)
                    .unwrap()
                    .into_iter()
                    .map(|x| x.0)
                    .collect();
                for &(jj, m) in b.keys() {
                    if jj == j {
                        assert!(ms.contains(&m), "term {j}: mask {m:b} missed");
                    }
                }
            }
        }
    }

    #[test]
    fn a_line_dropped_by_two_and_free_parameters() {
        let lines = [true, true, false];
        // target I(n0,n1,n2); term lowers n0 by 2: drop possible for n0 in {1, 2}: excess at n0 = 1 is (-(1-2)) - (1-1) = 1
        let r = rule(
            "I(n0,n1,n2)",
            &["I(n0-2,n1,n2)", "I(n0,n1+1,n2+1)", "I(n0,n1,n2-1)"],
            &lines,
            vec![None; 3],
        );
        let t0 = term_excess(&r, &r.terms[0], &lines).unwrap();
        assert!(t0.contains(&(1, Some(1))), "{t0:?}");
        assert!(t0.contains(&(0, Some(-2))), "kept: excess -2: {t0:?}");
        // n2 + 1 > 0 for n2 = 0: feasible only for n2 <= -1; top term, D + r +1 - 1 = 0
        let t1 = term_excess(&r, &r.terms[1], &lines).unwrap();
        assert_eq!(t1, vec![(0, Some(0))]);
        let t2 = term_excess(&r, &r.terms[2], &lines).unwrap();
        assert_eq!(
            t2,
            vec![(0, Some(1))],
            "a raised numerator raises D + r by 1"
        );
        let mut ne = NodeExcess::default();
        ne.add_rule(&r, &lines);
        assert_eq!(ne.max_lines_dropped, 1);
        assert_eq!(ne.top_max, Some(1));
        assert_eq!(ne.handdown_max, Some(1));
    }

    #[test]
    fn edge_image_of_a_toy_sector() {
        let pres = |v: &[[i64; 3]]| {
            v.iter()
                .map(|m| json!({"line": m, "eps_units": 0}))
                .collect::<Vec<_>>()
        };
        let v = json!({"schema": super::super::edge::EDGE_SCHEMA, "edge": "P->C",
            "parent": {"label": "P", "table": "p.jsonl", "slot_map_table_to_presentation": [0,1,2,3,4],
                "presentation_slots": pres(&[[1,0,0],[0,1,0],[1,0,-1],[0,1,-1],[1,-1,0]]), "table_lines": [0,1,2,3]},
            "child": {"label": "C", "table": "c.jsonl", "slot_map_table_to_presentation": [0,1,2,3,4],
                "presentation_slots": pres(&[[1,0,0],[0,1,0],[0,1,-1],[1,0,-1],[1,1,0]]), "table_lines": [0,1,2]},
            "owner": {"class": "toy"},
            "sectors": [{"fate": "routed onto the child's class", "parent_table_slots_dropped": [2],
                 "parent_table_lines_kept": [0,1,3], "class": "toy", "chosen_candidate_index": 0,
                 "substitution": {"chosen": [{"line": 0}, {"line": 1}, {"form": {"3": "1"}, "q2": "1"}, {"line": 2},
                     {"form": {"0": "2", "1": "2", "4": "-1"}, "q2": "0"}]}}]});
        let e = EdgeFile::parse(v.to_string().as_bytes(), "toy", "chosen").unwrap();
        let im = edge_image(&e);
        assert_eq!(im.len(), 1);
        let s = &im[0];
        assert!(s.bound_holds, "{s:?}");
        assert_eq!(s.line_map, vec![(0, 0), (1, 1), (3, 2)]);
        assert!(s.child_lines_unhit.is_empty());
        assert_eq!(s.numerators_reachable, vec![3, 4]);
        assert_eq!(s.lines_lowerable, vec![0, 1]);
        assert!(s.q2_constant);
        assert_eq!(mask_string(e.sectors[0].dropped_mask, 5), "00100");
        // the cone JSON is lane parametric's excess simplex: lines +1, numerators -1, b = S + #lines
        let c = simplex_cone_json(&[true, true, true, false, false], 7);
        assert_eq!(c["constraints"][0], json!([[1, 1, 1, -1, -1], 10]));
    }

    #[test]
    fn simplex_counts() {
        // 2 lines + 1 numerator, D + r = 2: distributions of 2 units over 3 slots = 6
        assert_eq!(simplex_layer(2, 1, 2), 6.0);
        let mut st = DrStats::default();
        st.add(&[2, 1, -1], &[true, true, false]);
        st.add(&[1, 1, 0], &[true, true, false]);
        assert_eq!((st.max_d, st.max_r, st.max_dr, st.above(1)), (1, 1, 2, 1));
    }
}
