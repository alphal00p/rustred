//! Multi-term SYMMETRY FACE RULES as slice rules (`rr_node symface`, engine line 7; interface
//! plan/auto-campaign/interfaces/coord-pole-engine.md sections 9b-9c).
//!
//! A face F of a chart: lines at 1 except the DOTTED lines (free, n >= 2); numerators at 0 except the face numerators
//! (free, n <= -1). For a group element g of the chart (node_group: verified momentum map T, unit Jacobian, lines onto
//! lines) every line and every face numerator either maps EXACTLY (`action` = slot) or, for a numerator, goes to a
//! combination: D_j(kT) = sum_i c_ij D_i(k) + c0_j. The combination is computed here by exact linear algebra over Q in
//! the space of symmetric bilinear forms over (k_1..k_L, q) (the slot forms plus the q.q form, q^2 = 1) and CHECKED
//! exactly (decomposition times basis == image form); a slot whose image is not in that span refuses the element.
//!
//! With k -> kT, I(a) = int prod_i D_i(kT)^(-a_i). Exactly mapped slots move their (symbolic) power to the target slot;
//! a substituted numerator of power -b contributes (sum_i c_ij D_i + c0_j)^b, multinomially expanded: a factor D_k^e
//! lowers slot k's power by e. The number of terms grows with b, so there is no fixed finite RHS for all b: one SLICE
//! RULE per value of the substituted powers (fixed in the record, so a key outside the generated slices matches no
//! slice rule), symbolic in every exactly mapped free slot. Terms in proved-zero sectors are dropped; equal integrals are
//! merged; every coefficient is an integer. Descent: every RHS term must be strictly lower than the target under the
//! node order for every key of the slice: same-chart terms by the order's linear forms (affine in the free slots: the
//! first form whose difference is not identically zero must be a constant of the right sign), subsector terms by the
//! order itself at sample keys (lower support); a not-lower term refuses the element for the face.
use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::symmetry::{ChartForms, NodeGroup, SymElement, node_group, parse_props};

// ---------------------------------------------------------------- exact rationals (small)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Q {
    n: i128,
    d: i128,
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl Q {
    fn new(n: i128, d: i128) -> Q {
        assert!(d != 0);
        let g = gcd(n, d).max(1);
        let s = if d < 0 { -1 } else { 1 };
        Q { n: s * n / g, d: s * d / g }
    }
    fn int(n: i128) -> Q {
        Q { n, d: 1 }
    }
    fn zero() -> Q {
        Q::int(0)
    }
    fn is_zero(&self) -> bool {
        self.n == 0
    }
    #[allow(dead_code)]
    fn add(self, o: Q) -> Q {
        Q::new(self.n * o.d + o.n * self.d, self.d * o.d)
    }
    fn sub(self, o: Q) -> Q {
        Q::new(self.n * o.d - o.n * self.d, self.d * o.d)
    }
    fn mul(self, o: Q) -> Q {
        Q::new(self.n * o.n, self.d * o.d)
    }
    fn div(self, o: Q) -> Q {
        Q::new(self.n * o.d, self.d * o.n)
    }
}

// ---------------------------------------------------------------- forms and the decomposition
/// The symmetric bilinear form of a squared momentum m (over w = L+1 components), as the upper triangle.
fn form(m: &[i64]) -> Vec<i128> {
    let w = m.len();
    let mut out = Vec::with_capacity(w * (w + 1) / 2);
    for a in 0..w {
        for b in a..w {
            out.push(i128::from(m[a]) * i128::from(m[b]));
        }
    }
    out
}

/// m T (row-vector convention v -> v T; T row-major w x w).
fn mapped(m: &[i64], t: &[i64]) -> Vec<i64> {
    let w = m.len();
    (0..w)
        .map(|col| (0..w).map(|row| m[row] * t[row * w + col]).sum())
        .collect()
}

/// A numerator image as a combination: D_j(kT) = sum_i c[i] D_i(k) + c0.
#[derive(Clone, Debug, PartialEq)]
pub struct Combination {
    pub coef: Vec<i64>,
    pub constant: i64,
}

/// Exact decomposition of the form `target` in the basis (slot forms, q.q form); None if not in the span or not integral.
fn decompose(basis: &[Vec<i128>], target: &[i128]) -> Option<(Vec<Q>, bool)> {
    let rows = target.len();
    let cols = basis.len();
    // augmented matrix rows x (cols + 1)
    let mut a: Vec<Vec<Q>> = (0..rows)
        .map(|r| {
            let mut v: Vec<Q> = (0..cols).map(|c| Q::int(basis[c][r])).collect();
            v.push(Q::int(target[r]));
            v
        })
        .collect();
    let mut pivots = Vec::new();
    let mut r = 0;
    for c in 0..cols {
        let Some(p) = (r..rows).find(|&i| !a[i][c].is_zero()) else {
            continue;
        };
        a.swap(r, p);
        let inv = Q::int(1).div(a[r][c]);
        for k in c..=cols {
            a[r][k] = a[r][k].mul(inv);
        }
        for i in 0..rows {
            if i != r && !a[i][c].is_zero() {
                let f = a[i][c];
                for k in c..=cols {
                    let v = a[r][k].mul(f);
                    a[i][k] = a[i][k].sub(v);
                }
            }
        }
        pivots.push(c);
        r += 1;
        if r == rows {
            break;
        }
    }
    // consistency: rows without pivot must have zero rhs
    if (r..rows).any(|i| !a[i][cols].is_zero()) {
        return None;
    }
    let mut x = vec![Q::zero(); cols];
    for (i, &c) in pivots.iter().enumerate() {
        x[c] = a[i][cols];
    }
    // the basis is a basis only if every column has a pivot; else the solution is one of several (free = 0)
    let unique = pivots.len() == cols;
    Some((x, unique))
}

/// The family's slot momenta (presented slots) and the decomposition of every numerator image of `e`.
pub struct FamilyForms {
    pub loops: usize,
    pub momenta: Vec<Vec<i64>>,
    basis: Vec<Vec<i128>>,
}

impl FamilyForms {
    pub fn new(text: &str) -> Result<Self, String> {
        let (loops, props) = parse_props(text)?;
        let momenta: Vec<Vec<i64>> = props.into_iter().map(|(m, _)| m).collect();
        let w = loops + 1;
        let mut basis: Vec<Vec<i128>> = momenta.iter().map(|m| form(m)).collect();
        let mut q = vec![0i64; w];
        q[loops] = 1;
        basis.push(form(&q));
        Ok(Self {
            loops,
            momenta,
            basis,
        })
    }

    /// D_j(kT) as a combination of the D_i(k) and q^2 = 1, exact and checked; Err when not in the span, not integral
    /// or not unique.
    pub fn combination(&self, matrix: &[i64], j: usize) -> Result<Combination, String> {
        let image = form(&mapped(&self.momenta[j], matrix));
        let (x, unique) = decompose(&self.basis, &image)
            .ok_or_else(|| format!("slot {j}: image form not in the span of the slot forms"))?;
        if !unique {
            return Err(format!("slot {j}: the slot forms are not a basis (decomposition not unique)"));
        }
        if x.iter().any(|q| q.d != 1) {
            return Err(format!("slot {j}: non-integral decomposition"));
        }
        // exact check: sum x_i basis_i == image
        for r in 0..image.len() {
            let s: i128 = (0..self.basis.len()).map(|c| x[c].n * self.basis[c][r]).sum();
            if s != image[r] {
                return Err(format!("SYMMETRY_FORM_CHECK_FAILED: slot {j}, component {r}"));
            }
        }
        let n = self.momenta.len();
        Ok(Combination {
            coef: (0..n).map(|i| x[i].n as i64).collect(),
            constant: x[n].n as i64,
        })
    }
}

// ---------------------------------------------------------------- the face and the slices
/// A power in a pattern: optional symbolic variable (a key slot index) plus a constant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pow {
    pub var: Option<usize>,
    pub c: i64,
}

fn pow_text(p: &Pow) -> String {
    match (p.var, p.c) {
        (None, c) => c.to_string(),
        (Some(v), 0) => format!("n{v}"),
        (Some(v), c) if c > 0 => format!("n{v}+{c}"),
        (Some(v), c) => format!("n{v}{c}"),
    }
}

pub fn pattern_text(p: &[Pow]) -> String {
    format!("I({})", p.iter().map(pow_text).collect::<Vec<_>>().join(","))
}

/// One face: chart mask, dotted lines (free n >= 2), face numerators (free n <= -1).
#[derive(Clone, Debug)]
pub struct Face {
    pub chart: Vec<bool>,
    pub dotted: Vec<usize>,
    pub numerators: Vec<usize>,
}

impl Face {
    pub fn free(&self, i: usize) -> bool {
        self.dotted.contains(&i) || self.numerators.contains(&i)
    }
    pub fn signature(&self) -> Value {
        json!({"dotted": self.dotted, "numerators": self.numerators})
    }
}

/// One slice rule: the substituted slots' powers, its target pattern, its terms (pattern, integer coefficient).
#[derive(Clone, Debug)]
pub struct SliceRule {
    pub slice: BTreeMap<usize, i64>,
    pub target: Vec<Pow>,
    pub terms: Vec<(Vec<Pow>, i128)>,
    pub dropped_zero_sector: usize,
    pub expanded: usize,
}

/// How element `e` acts on the face: Err when a line or an exactly needed slot has no image, else the substituted
/// numerator slots with their combinations.
pub fn face_action(
    ff: &FamilyForms,
    e: &SymElement,
    face: &Face,
) -> Result<BTreeMap<usize, Combination>, String> {
    let n = face.chart.len();
    let mut subs = BTreeMap::new();
    for i in 0..n {
        let nonzero = face.chart[i] || face.free(i); // lines always carry power >= 1
        if !nonzero {
            continue;
        }
        match e.action[i] {
            Some((t, flip)) => {
                if flip {
                    return Err(format!("slot {i}: sign flip (not a squared propagator)"));
                }
                if face.chart[i] != face.chart[t] {
                    return Err(format!("slot {i} -> {t}: line / numerator mismatch"));
                }
            }
            None if face.chart[i] => return Err(format!("line {i} goes to a combination")),
            None => {
                subs.insert(i, ff.combination(&e.matrix, i)?);
            }
        }
    }
    Ok(subs)
}

/// Expand the image of the face key pattern under `e` at one slice (substituted powers fixed: slot -> power <= -1).
pub fn slice_rule(
    e: &SymElement,
    face: &Face,
    subs: &BTreeMap<usize, Combination>,
    slice: &BTreeMap<usize, i64>,
    zero: &dyn Fn(&[bool]) -> bool,
) -> Result<SliceRule, String> {
    let n = face.chart.len();
    // target pattern
    let target: Vec<Pow> = (0..n)
        .map(|i| match slice.get(&i) {
            Some(&v) => Pow { var: None, c: v },
            None if face.free(i) => Pow { var: Some(i), c: 0 },
            None => Pow {
                var: None,
                c: i64::from(face.chart[i]),
            },
        })
        .collect();
    // exactly mapped slots move their power
    let mut base = vec![Pow { var: None, c: 0 }; n];
    for i in 0..n {
        if subs.contains_key(&i) {
            continue;
        }
        let p = target[i];
        if p.var.is_none() && p.c == 0 {
            continue;
        }
        let (t, _) = e.action[i].ok_or_else(|| format!("slot {i} has no image"))?;
        base[t] = p;
    }
    // expansion: polynomial over exponent vectors of the D_k (len n) with integer coefficients
    let mut poly: BTreeMap<Vec<u32>, i128> = BTreeMap::new();
    poly.insert(vec![0; n], 1);
    for (&j, comb) in subs {
        let b = -slice[&j];
        if b < 0 {
            return Err(format!("slice power {} on slot {j} is not a numerator power", slice[&j]));
        }
        let factor: Vec<(Option<usize>, i128)> = comb
            .coef
            .iter()
            .enumerate()
            .filter(|(_, c)| **c != 0)
            .map(|(k, c)| (Some(k), i128::from(*c)))
            .chain((comb.constant != 0).then_some((None, i128::from(comb.constant))))
            .collect();
        for _ in 0..b {
            let mut next: BTreeMap<Vec<u32>, i128> = BTreeMap::new();
            for (ex, c) in &poly {
                for (k, f) in &factor {
                    let mut e2 = ex.clone();
                    if let Some(k) = k {
                        e2[*k] += 1;
                    }
                    let v = c.checked_mul(*f).ok_or("coefficient overflow")?;
                    let slot = next.entry(e2).or_insert(0);
                    *slot = slot.checked_add(v).ok_or("coefficient overflow")?;
                }
            }
            next.retain(|_, c| *c != 0);
            poly = next;
        }
    }
    let expanded = poly.len();
    let mut terms: BTreeMap<Vec<Pow>, i128> = BTreeMap::new();
    let mut dropped = 0usize;
    for (ex, c) in poly {
        let mut p = base.clone();
        for k in 0..n {
            p[k].c -= i64::from(ex[k]);
        }
        // the term's sector: lines with power >= 1 uniformly on the slice (symbolic dotted powers are >= 2)
        let mut mask = vec![false; n];
        for k in 0..n {
            if !face.chart[k] {
                continue;
            }
            mask[k] = match p[k].var {
                None => p[k].c >= 1,
                Some(v) => {
                    // n_v >= 2 on the face (a dotted line): n_v + c >= 1 for all n_v iff c >= -1
                    if !face.dotted.contains(&v) {
                        return Err(format!("symbolic power of a non-dotted slot {v} on line {k}"));
                    }
                    if p[k].c >= -1 {
                        true
                    } else {
                        return Err(format!(
                            "line {k}: power n{v}{} is not uniformly in or out of the sector on the face",
                            p[k].c
                        ));
                    }
                }
            };
        }
        if zero(&mask) {
            dropped += 1;
            continue;
        }
        *terms.entry(p).or_insert(0) += c;
    }
    terms.retain(|_, c| *c != 0);
    Ok(SliceRule {
        slice: slice.clone(),
        target,
        terms: terms.into_iter().collect(),
        dropped_zero_sector: dropped,
        expanded,
    })
}

/// Descent of one slice rule: every term strictly lower than the target for every key of the slice. Same-chart
/// terms: the order's linear forms (affine in the free slots); other terms: `concrete` (the engine order) at sample keys.
pub fn slice_descends(
    rule: &SliceRule,
    face: &Face,
    forms: &ChartForms,
    concrete: &dyn Fn(&[i64], &[i64]) -> std::cmp::Ordering,
    samples: usize,
) -> Result<(), String> {
    let n = face.chart.len();
    let free: Vec<usize> = (0..n)
        .filter(|&i| rule.target[i].var.is_some())
        .collect();
    let in_chart = |p: &[Pow]| {
        (0..n).all(|k| {
            !face.chart[k] || p[k].var.is_some() || p[k].c >= 1
        })
    };
    for (p, _) in &rule.terms {
        if in_chart(p) {
            // affine difference per form: w.target - w.term = sum_k w_k (t_k - p_k); variables must cancel or
            // the form is undecided (refuse)
            let mut decided = false;
            for (w, reversed) in &forms.forms {
                let mut coeff_var: BTreeMap<usize, i128> = BTreeMap::new();
                let mut constant: i128 = 0;
                for k in 0..n {
                    let wk = i128::from(w[k]);
                    if wk == 0 {
                        continue;
                    }
                    constant += wk * i128::from(rule.target[k].c - p[k].c);
                    if let Some(v) = rule.target[k].var {
                        *coeff_var.entry(v).or_insert(0) += wk;
                    }
                    if let Some(v) = p[k].var {
                        *coeff_var.entry(v).or_insert(0) -= wk;
                    }
                }
                if coeff_var.values().any(|c| *c != 0) {
                    return Err(format!(
                        "term {}: order form depends on the free slots (undecided)",
                        pattern_text(p)
                    ));
                }
                let c = if *reversed { 0.cmp(&constant) } else { constant.cmp(&0) };
                match c {
                    std::cmp::Ordering::Equal => continue,
                    // `Less` = left harder: target harder than term = term lower
                    std::cmp::Ordering::Less => {
                        decided = true;
                        break;
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(format!("term {} is not lower than the target", pattern_text(p)));
                    }
                }
            }
            if !decided {
                return Err(format!("term {} equals the target under the order", pattern_text(p)));
            }
        }
        // concrete check at sample keys (all terms; the subsector terms' only check)
        for s in 0..samples.max(1) {
            let val = |pw: &Pow, key: &[i64]| pw.var.map_or(0, |v| key[v]) + pw.c;
            let mut key = vec![0i64; n];
            for (idx, &v) in free.iter().enumerate() {
                let step = ((s / (idx + 1)) % 5) as i64;
                key[v] = if face.chart[v] { 2 + step } else { -1 - step };
            }
            let t: Vec<i64> = (0..n).map(|k| val(&rule.target[k], &key)).collect();
            let r: Vec<i64> = (0..n).map(|k| val(&p[k], &key)).collect();
            if concrete(&t, &r) != std::cmp::Ordering::Less {
                return Err(format!(
                    "term {} is not lower at the sample key {:?}",
                    pattern_text(p),
                    t
                ));
            }
        }
    }
    Ok(())
}

/// The slices: every assignment of powers -1..-N to the substituted slots, bounded per slot (`each`) or by the sum of
/// their absolute values (`total`).
pub fn slices(subs: &[usize], max: i64, total: bool) -> Vec<BTreeMap<usize, i64>> {
    let mut out = vec![BTreeMap::new()];
    for &s in subs {
        let mut next = Vec::new();
        for m in &out {
            let used: i64 = m.values().map(|v: &i64| -v).sum();
            for b in 1..=max {
                if total && used + b > max {
                    break;
                }
                let mut m2 = m.clone();
                m2.insert(s, -b);
                next.push(m2);
            }
        }
        out = next;
    }
    out
}

/// Result for one element on one face: its slice rules, or why it was refused.
pub struct ElementResult {
    pub element: usize,
    pub substituted: Vec<usize>,
    pub rules: Vec<SliceRule>,
    pub refused: Option<String>,
}

impl ElementResult {
    pub fn total_terms(&self) -> usize {
        self.rules.iter().map(|r| r.terms.len()).sum()
    }
}

/// Try every group element on the face; returns all results (the caller picks the smallest accepted one).
#[allow(clippy::too_many_arguments)]
pub fn try_elements(
    ff: &FamilyForms,
    group: &NodeGroup,
    face: &Face,
    slice_max: i64,
    total: bool,
    zero: &dyn Fn(&[bool]) -> bool,
    forms: &ChartForms,
    concrete: &dyn Fn(&[i64], &[i64]) -> std::cmp::Ordering,
    only: Option<usize>,
) -> Vec<ElementResult> {
    let mut out = Vec::new();
    for (k, e) in group.elements.iter().enumerate() {
        if only.is_some_and(|o| o != k) {
            continue;
        }
        let mut res = ElementResult {
            element: k,
            substituted: vec![],
            rules: vec![],
            refused: None,
        };
        match face_action(ff, e, face) {
            Err(m) => res.refused = Some(m),
            Ok(subs) => {
                res.substituted = subs.keys().copied().collect();
                let slice_list = if subs.is_empty() {
                    vec![BTreeMap::new()]
                } else {
                    slices(&res.substituted, slice_max, total)
                };
                for sl in &slice_list {
                    match slice_rule(e, face, &subs, sl, zero)
                        .and_then(|r| slice_descends(&r, face, forms, concrete, 25).map(|_| r))
                    {
                        Ok(r) => res.rules.push(r),
                        Err(m) => {
                            res.refused = Some(format!("slice {sl:?}: {m}"));
                            res.rules.clear();
                            break;
                        }
                    }
                }
            }
        }
        out.push(res);
    }
    out
}

/// The rule records of the chosen element (ordinary `"type":"rule"` lines with the `symface` provenance member).
pub fn rule_records(
    res: &ElementResult,
    e: &SymElement,
    face: &Face,
    chart_label: &str,
    first_index: usize,
    slice_max: i64,
    bound: &str,
) -> Vec<String> {
    let n = face.chart.len();
    let line_perm: Vec<Value> = (0..n)
        .filter(|&i| face.chart[i])
        .map(|i| e.action[i].map_or(Value::Null, |(t, _)| json!(t)))
        .collect();
    res.rules
        .iter()
        .enumerate()
        .map(|(k, r)| {
            let fixed: Vec<Value> = (0..n)
                .map(|i| match r.target[i].var {
                    Some(_) => Value::Null,
                    None => json!(r.target[i].c),
                })
                .collect();
            let rhs: Vec<Value> = r
                .terms
                .iter()
                .map(|(p, c)| json!([pattern_text(p), c.to_string()]))
                .collect();
            let rec = json!({
                "type": "rule", "chart": chart_label, "index": first_index + k,
                "case": format!("symface slice {:?}", r.slice),
                "fixed": fixed, "equations": [], "target": pattern_text(&r.target), "rhs": rhs,
                "exceptions": [], "dispatch": "Partition", "seeds": [], "guard_version": 2,
                "symface": {"schema": "rustred.symface.v1",
                    "element": {"index": res.element, "line_permutation": line_perm, "matrix": e.matrix},
                    "face": face.signature(), "substituted": res.substituted,
                    "slice": r.slice.iter().map(|(s, v)| (s.to_string(), json!(v))).collect::<serde_json::Map<_, _>>(),
                    "slice_max": slice_max, "slice_bound": bound,
                    "terms": {"expanded": r.expanded, "zero_sector_dropped": r.dropped_zero_sector, "kept": r.terms.len()},
                    "proof": {"automorphism": "verified (node_group: owner_orbit + sector::symmetry::verify, unit Jacobian)",
                        "form_check": "exact", "expansion_check": "exact (multinomial expansion over Z)",
                        "descent": "affine_forms (same-chart terms) + engine order at sample keys"}}
            });
            // type first, as the engine writes rule lines
            let mut obj = match rec {
                Value::Object(m) => m,
                _ => unreachable!(),
            };
            obj.remove("type");
            format!("{{\"type\":\"rule\",{}", &Value::Object(obj).to_string()[1..])
        })
        .collect()
}

/// Options of `rr_node symface`.
pub struct RunOptions {
    pub family_text: String,
    pub order: super::order::NodeOrder,
    pub face: Face,
    pub slice_max: i64,
    pub total_bound: bool,
    pub element: Option<usize>,
    pub out: std::path::PathBuf,
}

macro_rules! with_n {
    ($n:expr, $N:ident, $body:block) => {
        match $n {
            5 => { const $N: usize = 5; $body }
            9 => { const $N: usize = 9; $body }
            12 => { const $N: usize = 12; $body }
            14 => { const $N: usize = 14; $body }
            20 => { const $N: usize = 20; $body }
            other => Err(format!("arity {other}: symface compiled for 5, 9, 12, 14, 20")),
        }
    };
}

/// Run the verb: writes OUT/F.table.jsonl (header, chart, slice rules, end) and OUT/REPORT.json; returns the report.
pub fn run(o: &RunOptions) -> Result<Value, String> {
    use crate::sector::{Mask, zero};
    let t0 = std::time::Instant::now();
    if o.out.exists() {
        return Err(format!("{} exists (no-clobber)", o.out.display()));
    }
    let n = o.face.chart.len();
    let fam = super::driver::compile_family(&o.family_text)?;
    if fam.denominator_count() != n {
        return Err(format!("chart arity {n} != family arity {}", fam.denominator_count()));
    }
    // zero-sector census of the chart's lines (the recipe verb's analyzer and downset)
    let analyzer = zero::Analyzer::try_unrestricted(&fam).map_err(|e| format!("zero analyzer: {e:?}"))?;
    let scope: Vec<usize> = (0..n).filter(|&i| o.face.chart[i]).collect();
    let mut zeros: std::collections::BTreeSet<Vec<bool>> = Default::default();
    for bits in 0u64..(1u64 << scope.len()) {
        let mut active = vec![false; n];
        for (j, &i) in scope.iter().enumerate() {
            active[i] = bits >> j & 1 == 1;
        }
        let m = Mask::try_new(active.iter().copied()).map_err(|e| format!("mask: {e:?}"))?;
        if let zero::Decision::ProvedZero(_) = analyzer.analyze(&m).map_err(|e| format!("zero analysis: {e:?}"))? {
            zeros.insert(active);
        }
    }
    let zero_fn = |mask: &[bool]| zeros.contains(mask);
    let ff = FamilyForms::new(&o.family_text)?;
    let group = node_group(&o.family_text, &o.face.chart)?;
    let label: String = o.face.chart.iter().map(|&b| if b { '1' } else { '0' }).collect();
    let results: Vec<ElementResult> = with_n!(n, N, {
        let mut sector = [false; N];
        sector[..n].copy_from_slice(&o.face.chart);
        let io = super::descent::integral_order::<N>(&o.order, sector, n)?;
        let forms = ChartForms::from_order(&io, &o.face.chart)
            .ok_or("the node order is not modelled by linear forms on this chart")?;
        let concrete = |a: &[i64], b: &[i64]| -> std::cmp::Ordering {
            let to = |v: &[i64]| {
                crate::solver::Integral::<N>::new(std::array::from_fn(|i| {
                    crate::solver::Power::new(false, if i < n { v[i] as i16 } else { 0 }).expect("power")
                }))
            };
            io.compare(&to(a), &to(b))
        };
        Ok(try_elements(&ff, &group, &o.face, o.slice_max, o.total_bound, &zero_fn, &forms, &concrete, o.element))
    })?;
    // choice: the accepted element with the fewest total terms; ties by element index
    let chosen = results
        .iter()
        .filter(|r| r.refused.is_none() && !r.rules.is_empty())
        .min_by_key(|r| (r.total_terms(), r.element));
    let bound = if o.total_bound { "sum |substituted power| <= slice_max" } else { "each |substituted power| <= slice_max" };
    let per_element: Vec<Value> = results
        .iter()
        .map(|r| json!({"element": r.element, "substituted": r.substituted, "accepted": r.refused.is_none(),
            "refused": r.refused, "slice_rules": r.rules.len(), "terms": r.total_terms(),
            "max_terms": r.rules.iter().map(|x| x.terms.len()).max()}))
        .collect();
    std::fs::create_dir_all(&o.out).map_err(|e| e.to_string())?;
    let mut report = json!({"schema": "rustred.symface-report.v1", "face": o.face.signature(), "chart": label,
        "group_order": group.group_order, "elements_tried": results.len(), "per_element": per_element,
        "slice_max": o.slice_max, "slice_bound": bound, "zero_sectors": zeros.len(),
        "chosen": chosen.map(|r| r.element)});
    if let Some(c) = chosen {
        let e = &group.elements[c.element];
        let lines = rule_records(c, e, &o.face, &label, 0, o.slice_max, bound);
        let header = json!({"type": "header", "schema": "table_dump.v1", "node_schema": "rustred.node.v1",
            "source_base": "rustred-node symface", "family_source": o.family_text, "charts": [label],
            "node_order": serde_json::from_str::<Value>(&o.order.to_json()).unwrap_or(Value::Null),
            "guard_version": 2,
            "symface_tables": {"slice_max": o.slice_max, "slice_bound": bound,
                "faces": [{"face": o.face.signature(), "element": c.element, "slices": c.rules.len(),
                           "rules": c.rules.len(), "rhs_terms": c.total_terms()}]}});
        let mut text = String::new();
        let mut h = header.as_object().unwrap().clone();
        h.remove("type");
        text.push_str(&format!("{{\"type\":\"header\",{}\n", &Value::Object(h).to_string()[1..]));
        text.push_str(&format!("{{\"type\":\"chart\",\"chart\":\"{label}\",\"guards\":[],\"zero\":false}}\n"));
        for l in &lines {
            text.push_str(l);
            text.push('\n');
        }
        text.push_str(&format!(
            "{{\"type\":\"end\",\"charts\":1,\"rules\":{},\"residuals\":0,\"chart_errors\":0,\"harness\":\"rustred node symface\",\"symface\":{{\"faces\":1,\"slice_max\":{},\"slice_bound\":\"{bound}\"}}}}\n",
            lines.len(), o.slice_max
        ));
        let table = o.out.join("F.table.jsonl");
        std::fs::write(&table, &text).map_err(|e| e.to_string())?;
        report["table"] = json!(table.display().to_string());
        report["rules"] = json!(lines.len());
        report["rhs_terms"] = json!(c.total_terms());
        report["max_terms_per_rule"] = json!(c.rules.iter().map(|x| x.terms.len()).max());
        report["table_bytes"] = json!(text.len());
        report["table_blake3"] = json!(blake3::hash(text.as_bytes()).to_hex().to_string());
    }
    report["seconds"] = json!(t0.elapsed().as_secs_f64());
    std::fs::write(o.out.join("REPORT.json"), serde_json::to_string_pretty(&report).unwrap()).map_err(|e| e.to_string())?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decomposition_is_exact_and_checked() {
        // two loops: slots D1=k1^2, D2=k2^2, D3=(k1+k2)^2 lines; D4=(k1+q)^2, D5=(k2+q)^2 numerators
        let text = "I(name(t),loops(k1,k2),externals(q),dimension(d),prop(D1,(k1)^2,1),prop(D2,(k2)^2,1),prop(D3,(k1+k2)^2,1),prop(D4,(k1+q)^2,0),prop(D5,(k2+q)^2,0))";
        let ff = FamilyForms::new(text).unwrap();
        // T: k1 -> -k1 - q ... use k1 -> k1, k2 -> -k1-k2 (swaps D2 and D3), q -> q: rows over (k1,k2,q)
        // v -> v T with v = (a,b,c): new = (a - b, -b, c)
        let t = vec![1, 0, 0, -1, -1, 0, 0, 0, 1];
        let c = ff.combination(&t, 4).unwrap(); // D5 = (k2+q)^2 -> (-k1-k2+q)^2 = D3 - 2 k1.q ... combination
        // check by evaluation: (-k1-k2+q)^2 = sum c_i D_i + c0 as forms
        let img = form(&mapped(&ff.momenta[4], &t));
        let mut s = vec![0i128; img.len()];
        for (i, ci) in c.coef.iter().enumerate() {
            for (r, v) in form(&ff.momenta[i]).iter().enumerate() {
                s[r] += i128::from(*ci) * v;
            }
        }
        s[img.len() - 1] += i128::from(c.constant);
        assert_eq!(s, img);
    }

    #[test]
    fn slices_each_and_total() {
        assert_eq!(slices(&[3, 5], 2, false).len(), 4);
        assert_eq!(slices(&[3, 5], 3, true).len(), 3); // (1,1) (1,2) (2,1)
    }
}
