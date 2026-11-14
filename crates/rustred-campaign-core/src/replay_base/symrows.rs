//! NEW (lane I-ROWS, task rows2): SYMMETRY ROWS for the campaign's independent identity test (interfaces/ibp-rows.md
//! section 10). Not a copy: the engine's definition (sym-symbolic line, `node/symmetry.rs`, `solver/numeric.rs`,
//! `node/replay/mod.rs` `sym_rows_at` / `check_symmetry_record`) is re-implemented from the family text only.
//!
//! - [`SymGroup`]: a chart's symmetry group AS DATA (schema `rustred.campaign.symgroup.v1`, or the engine's
//!   `symmetry_group` table record): per element the momentum map T (`v -> v T` over `(k_1..k_L, q)`) and the slot
//!   images (`Exact` one slot, `Linear` a combination of the D's and q^2, `Unknown`).
//! - [`derive_group`]: an INDEPENDENT derivation from the family's propagator momenta (bounded search, typed
//!   "not derived" outcome).
//! - [`verify_group`]: the exact momentum proof of every element; the ONLY way to obtain a [`VerifiedGroup`], which is
//!   the only form the identity test accepts.
//! - [`cross_check`]: two groups (derived / supplied / engine / checker) compared on the key action; typed verdict.
//! - [`rows_at`]: the symmetry rows at one seed (integer key or generic pattern), engine (one-term) or expanded
//!   (multi-term numerator maps, the checker's superset) mode, exact rational coefficients, bounded expansion.
//!
//! Nothing here is specific to a family: loops, arity, chart and group order are all read from the input.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Serialize;
use serde_json::{Value, json};

use super::arith::Q;
use super::family::Family;

/// Schema of the group data file.
pub const SYMGROUP_SCHEMA: &str = "rustred.campaign.symgroup.v1";

/// Image of one slot under an element: `D_i(k T) = ...` in terms of the D's of the family at k.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotImage {
    /// `D_i(kT) = (+-) D_target(k)` (`flip` = minus; never true for squared propagators)
    Exact { target: usize, flip: bool },
    /// `D_i(kT) = sum_m c_m D_m(k) + constant * q^2` (q^2 = 1); terms sorted by slot, nonzero
    Linear { terms: Vec<(usize, Q)>, constant: Q },
    /// not given: the element acts only on keys with power 0 at this slot
    Unknown,
}

/// One group element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymElement {
    /// T, (L+1)^2 entries row-major, row-vector convention `v -> v T` over `(k_1..k_L, q)`; `None` = not given
    pub matrix: Option<Vec<i64>>,
    pub action: Vec<SlotImage>,
}

impl SymElement {
    pub fn is_identity(&self) -> bool {
        self.action.iter().enumerate().all(|(i, a)| {
            *a == SlotImage::Exact {
                target: i,
                flip: false,
            }
        })
    }

    /// The images of the chart lines (the line permutation), `None` if a chart line is not mapped exactly.
    pub fn line_perm(&self, chart: &[bool]) -> Option<Vec<usize>> {
        (0..chart.len())
            .filter(|&i| chart[i])
            .map(|i| match self.action.get(i) {
                Some(SlotImage::Exact { target, .. }) => Some(*target),
                _ => None,
            })
            .collect()
    }
}

/// A chart's symmetry group as data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymGroup {
    pub arity: usize,
    /// loop count (0 = not stated; verification uses the family's)
    pub loops: usize,
    pub chart: Vec<bool>,
    /// non-identity elements (an identity in the input is dropped)
    pub elements: Vec<SymElement>,
}

fn q_text(q: &Q) -> String {
    if q.d == 1 {
        q.n.to_string()
    } else {
        format!("{}/{}", q.n, q.d)
    }
}

fn q_from(v: &Value) -> Result<Q, String> {
    match v {
        Value::String(s) => Q::parse(s),
        Value::Number(n) => n
            .as_i64()
            .map(|x| Q::int(x as i128))
            .ok_or_else(|| format!("rational {n}")),
        _ => Err(format!("rational expected, got {v}")),
    }
}

fn chart_from(v: &Value, arity: usize) -> Result<Vec<bool>, String> {
    let s = v.as_str().ok_or("chart must be a 0/1 string")?;
    if s.len() != arity || !s.bytes().all(|b| b == b'0' || b == b'1') {
        return Err(format!("chart {s:?} is not a 0/1 string of length {arity}"));
    }
    Ok(s.bytes().map(|b| b == b'1').collect())
}

fn slot_from(v: &Value, arity: usize) -> Result<SlotImage, String> {
    match v {
        Value::Null => Ok(SlotImage::Unknown),
        Value::Array(a) if a.len() == 2 => {
            let target = a[0].as_u64().ok_or("action target")? as usize;
            if target >= arity {
                return Err(format!("action target {target} >= arity {arity}"));
            }
            let flip = a[1].as_bool().ok_or("action flip")?;
            Ok(SlotImage::Exact { target, flip })
        }
        Value::Object(o) => {
            let mut terms: BTreeMap<usize, Q> = BTreeMap::new();
            for t in o
                .get("terms")
                .and_then(Value::as_array)
                .ok_or("linear image without terms")?
            {
                let pair = t
                    .as_array()
                    .filter(|p| p.len() == 2)
                    .ok_or("term [slot, c]")?;
                let m = pair[0].as_u64().ok_or("term slot")? as usize;
                if m >= arity {
                    return Err(format!("term slot {m} >= arity {arity}"));
                }
                let c = q_from(&pair[1])?;
                let e = terms.entry(m).or_insert(Q::ZERO);
                *e = e.add(c)?;
            }
            let constant = match o.get("constant") {
                None | Some(Value::Null) => Q::ZERO,
                Some(c) => q_from(c)?,
            };
            Ok(SlotImage::Linear {
                terms: terms.into_iter().filter(|(_, c)| !c.is_zero()).collect(),
                constant,
            })
        }
        _ => Err(format!(
            "slot image must be null, [t, flip] or {{terms, constant}}: {v}"
        )),
    }
}

fn slot_json(s: &SlotImage) -> Value {
    match s {
        SlotImage::Exact { target, flip } => json!([target, flip]),
        SlotImage::Linear { terms, constant } => json!({
            "terms": terms.iter().map(|(m, c)| json!([m, q_text(c)])).collect::<Vec<_>>(),
            "constant": q_text(constant),
        }),
        SlotImage::Unknown => Value::Null,
    }
}

fn elements_from(els: &[Value], arity: usize, w: Option<usize>) -> Result<Vec<SymElement>, String> {
    let mut out = Vec::new();
    for (k, e) in els.iter().enumerate() {
        let matrix = match e.get("matrix") {
            None | Some(Value::Null) => None,
            Some(Value::Array(a)) => {
                let m: Vec<i64> = a
                    .iter()
                    .map(|x| x.as_i64().ok_or("matrix entry"))
                    .collect::<Result<_, _>>()
                    .map_err(|x| format!("element {k}: {x}"))?;
                if let Some(w) = w
                    && m.len() != w * w
                {
                    return Err(format!(
                        "element {k}: matrix of {} entries, expected {}",
                        m.len(),
                        w * w
                    ));
                }
                Some(m)
            }
            Some(_) => return Err(format!("element {k}: matrix is not an array")),
        };
        let action: Vec<SlotImage> = e
            .get("action")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("element {k}: no action array"))?
            .iter()
            .map(|s| slot_from(s, arity))
            .collect::<Result<_, _>>()
            .map_err(|x| format!("element {k}: {x}"))?;
        if action.len() != arity {
            return Err(format!(
                "element {k}: action of length {} for arity {arity}",
                action.len()
            ));
        }
        let el = SymElement { matrix, action };
        if !el.is_identity() {
            out.push(el);
        }
    }
    Ok(out)
}

impl SymGroup {
    /// Read the generic group file (schema [`SYMGROUP_SCHEMA`]; the field `schema` may be absent).
    pub fn from_json(v: &Value) -> Result<SymGroup, String> {
        if let Some(s) = v.get("schema").and_then(Value::as_str)
            && s != SYMGROUP_SCHEMA
        {
            return Err(format!("unknown group schema {s}"));
        }
        let arity = v
            .get("arity")
            .and_then(Value::as_u64)
            .ok_or("group without arity")? as usize;
        let loops = v.get("loops").and_then(Value::as_u64).unwrap_or(0) as usize;
        let chart = chart_from(v.get("chart").ok_or("group without chart")?, arity)?;
        let w = (loops > 0).then_some(loops + 1);
        let els = v
            .get("elements")
            .and_then(Value::as_array)
            .ok_or("group without elements")?;
        Ok(SymGroup {
            arity,
            loops,
            chart,
            elements: elements_from(els, arity, w)?,
        })
    }

    /// Read the engine's `{"type":"symmetry_group", .., "group": {..}}` table record (or its `group` object):
    /// elements `{matrix, action: [[t, flip] | null, ..]}`; a `null` slot is `Unknown` (the engine's partial action).
    pub fn from_engine_record(v: &Value) -> Result<SymGroup, String> {
        let g = v.get("group").unwrap_or(v);
        let arity = g
            .get("arity")
            .and_then(Value::as_u64)
            .ok_or("engine group without arity")? as usize;
        let loops = g.get("loops").and_then(Value::as_u64).unwrap_or(0) as usize;
        let chart = chart_from(g.get("chart").ok_or("engine group without chart")?, arity)?;
        let w = (loops > 0).then_some(loops + 1);
        let els = g
            .get("elements")
            .and_then(Value::as_array)
            .ok_or("engine group without elements")?;
        Ok(SymGroup {
            arity,
            loops,
            chart,
            elements: elements_from(els, arity, w)?,
        })
    }

    /// Read the coordinator fixtures' group file (schema `c-fix.symmetry-group.v1`, interfaces/coord-fixtures.md: the
    /// M2 nodes' `SYMMETRY.json`): `slots` (names in slot order), `lines` (the chart), per element `line_perm`
    /// (name -> name), `momentum_map` (`k_images`: loop name -> {loop name | "q": coefficient}, `q_image` "+-1*q"),
    /// `numerator_images` (name -> {name | "1": rational}: `D_i(kT) = sum c_m D_m + c_1`; absent = Unknown). Slot
    /// names must be the family's `D<k>` labels in order; loop names are the family's.
    pub fn from_cfix_json(v: &Value, family: &Family) -> Result<SymGroup, String> {
        let slots: Vec<String> = v
            .get("slots")
            .and_then(Value::as_array)
            .ok_or("group file without slots")?
            .iter()
            .map(|x| x.as_str().map(str::to_string).ok_or("slot name"))
            .collect::<Result<_, _>>()?;
        let n = slots.len();
        if n != family.n {
            return Err(format!("group file has {n} slots, family {}", family.n));
        }
        for (i, s) in slots.iter().enumerate() {
            if *s != format!("D{}", i + 1) {
                return Err(format!("slot {i} is named {s}, expected D{}", i + 1));
            }
        }
        let slot = |name: &str| -> Result<usize, String> {
            slots
                .iter()
                .position(|s| s == name)
                .ok_or_else(|| format!("unknown slot name {name}"))
        };
        let mut chart = vec![false; n];
        for l in v
            .get("lines")
            .and_then(Value::as_array)
            .ok_or("group file without lines")?
        {
            chart[slot(l.as_str().ok_or("line name")?)?] = true;
        }
        let l = family.loops;
        let w = l + 1;
        let var = |name: &str| -> Result<usize, String> {
            if name == "q" {
                return Ok(l);
            }
            family
                .loop_names
                .iter()
                .position(|s| s == name)
                .ok_or_else(|| format!("unknown loop momentum {name}"))
        };
        let mut elements = Vec::new();
        for (k, e) in v
            .get("elements")
            .and_then(Value::as_array)
            .ok_or("group file without elements")?
            .iter()
            .enumerate()
        {
            let mut action = vec![SlotImage::Unknown; n];
            for (a, b) in e
                .get("line_perm")
                .and_then(Value::as_object)
                .ok_or_else(|| format!("element {k}: no line_perm"))?
            {
                action[slot(a)?] = SlotImage::Exact {
                    target: slot(b.as_str().ok_or("line_perm target")?)?,
                    flip: false,
                };
            }
            if let Some(ni) = e.get("numerator_images").and_then(Value::as_object) {
                for (a, form) in ni {
                    let i = slot(a)?;
                    let mut terms: BTreeMap<usize, Q> = BTreeMap::new();
                    let mut constant = Q::ZERO;
                    for (m, c) in form.as_object().ok_or("numerator image is not an object")? {
                        let c = q_from(c)?;
                        if m == "1" {
                            constant = constant.add(c)?;
                        } else {
                            let e2 = terms.entry(slot(m)?).or_insert(Q::ZERO);
                            *e2 = e2.add(c)?;
                        }
                    }
                    let terms: Vec<(usize, Q)> =
                        terms.into_iter().filter(|(_, c)| !c.is_zero()).collect();
                    action[i] = if constant.is_zero()
                        && terms.len() == 1
                        && terms[0].1 == Q::ONE
                        && !chart[terms[0].0]
                    {
                        SlotImage::Exact {
                            target: terms[0].0,
                            flip: false,
                        }
                    } else {
                        SlotImage::Linear { terms, constant }
                    };
                }
            }
            let matrix = match e.get("momentum_map") {
                Some(mm) if !mm.is_null() => {
                    let mut t = vec![0i64; w * w];
                    for (a, img) in mm
                        .get("k_images")
                        .and_then(Value::as_object)
                        .ok_or_else(|| format!("element {k}: momentum_map without k_images"))?
                    {
                        let r = var(a)?;
                        for (b, c) in img.as_object().ok_or("k image is not an object")? {
                            t[r * w + var(b)?] = c.as_i64().ok_or("k image coefficient")?;
                        }
                    }
                    let qi = mm.get("q_image").and_then(Value::as_str).ok_or("q_image")?;
                    let compact: String = qi.chars().filter(|c| !c.is_whitespace()).collect();
                    t[l * w + l] = match compact.as_str() {
                        "q" | "1*q" | "+1*q" | "+q" => 1,
                        "-q" | "-1*q" => -1,
                        _ => return Err(format!("element {k}: q_image {qi} is not +-q")),
                    };
                    Some(t)
                }
                _ => None,
            };
            let el = SymElement { matrix, action };
            if !el.is_identity() {
                elements.push(el);
            }
        }
        Ok(SymGroup {
            arity: n,
            loops: l,
            chart,
            elements,
        })
    }

    pub fn chart_text(&self) -> String {
        self.chart
            .iter()
            .map(|&b| if b { '1' } else { '0' })
            .collect()
    }

    /// The canonical JSON (elements in input order).
    pub fn to_json(&self) -> Value {
        json!({
            "schema": SYMGROUP_SCHEMA,
            "arity": self.arity,
            "loops": self.loops,
            "chart": self.chart_text(),
            "elements": self.elements.iter().map(|e| json!({
                "matrix": e.matrix,
                "action": e.action.iter().map(slot_json).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    }

    /// sha256 over schema, arity, chart and the sorted slot actions of the elements (the matrices are witnesses of
    /// the proof, not part of what the rows use, and are left out).
    pub fn digest(&self) -> String {
        let mut acts: Vec<String> = self
            .elements
            .iter()
            .map(|e| Value::Array(e.action.iter().map(slot_json).collect()).to_string())
            .collect();
        acts.sort();
        acts.dedup();
        let text = format!(
            "{SYMGROUP_SCHEMA}\x00{}\x00{}\x00{}",
            self.arity,
            self.chart_text(),
            acts.join("\x01")
        );
        super::sha256::sha256_hex(text.as_bytes())
    }
}

// ------------------------------------------------------------------ exact linear algebra ---------------------------

fn det_i128(mut a: Vec<Vec<i128>>) -> Option<i128> {
    // Bareiss fraction-free elimination, checked
    let n = a.len();
    if n == 0 {
        return Some(1);
    }
    let mut sign = 1i128;
    let mut prev = 1i128;
    for k in 0..n {
        if a[k][k] == 0 {
            match (k + 1..n).find(|&r| a[r][k] != 0) {
                Some(r) => {
                    a.swap(k, r);
                    sign = -sign;
                }
                None => return Some(0),
            }
        }
        for i in k + 1..n {
            for j in k + 1..n {
                let v = a[i][j]
                    .checked_mul(a[k][k])?
                    .checked_sub(a[i][k].checked_mul(a[k][j])?)?;
                a[i][j] = v / prev;
            }
        }
        prev = a[k][k];
    }
    a[n - 1][n - 1].checked_mul(sign)
}

fn inverse_q(m: &[Vec<Q>]) -> Result<Vec<Vec<Q>>, String> {
    let n = m.len();
    let mut a: Vec<Vec<Q>> = m
        .iter()
        .enumerate()
        .map(|(i, row)| {
            row.iter()
                .copied()
                .chain((0..n).map(|j| if i == j { Q::ONE } else { Q::ZERO }))
                .collect()
        })
        .collect();
    for c in 0..n {
        let p = (c..n)
            .find(|&r| !a[r][c].is_zero())
            .ok_or("singular matrix")?;
        a.swap(c, p);
        let f = a[c][c];
        a[c] = a[c].iter().map(|x| x.div(f)).collect::<Result<_, _>>()?;
        for r in 0..n {
            if r != c && !a[r][c].is_zero() {
                let g = a[r][c];
                let pivot = a[c].clone();
                a[r] = a[r]
                    .iter()
                    .zip(&pivot)
                    .map(|(x, y)| x.sub(g.mul(*y)?))
                    .collect::<Result<_, _>>()?;
            }
        }
    }
    Ok(a.into_iter().map(|row| row[n..].to_vec()).collect())
}

/// Momentum vector up to sign: first nonzero entry positive.
fn canon(v: &[i64]) -> Vec<i64> {
    match v.iter().find(|&&x| x != 0) {
        Some(&x) if x < 0 => v.iter().map(|y| -y).collect(),
        _ => v.to_vec(),
    }
}

fn vec_times(v: &[i64], t: &[i64], w: usize) -> Option<Vec<i64>> {
    (0..w)
        .map(|col| {
            (0..w).try_fold(0i64, |acc, row| {
                acc.checked_add(v[row].checked_mul(t[row * w + col])?)
            })
        })
        .collect()
}

/// The family's quadratic basis: `D_i = sum_pair M[i][pair] sp_pair + c0_i q^2`; decomposes any squared momentum.
struct QuadBasis {
    loops: usize,
    pairs: Vec<(usize, usize)>,
    /// `minv[pair][i]`
    minv: Vec<Vec<Q>>,
    c0: Vec<Q>,
}

impl QuadBasis {
    fn new(family: &Family) -> Result<QuadBasis, String> {
        let l = family.loops;
        let c = family.momenta();
        let n = family.n;
        let pairs: Vec<(usize, usize)> = (0..=l)
            .flat_map(|a| (a..=l).map(move |b| (a, b)))
            .filter(|&(a, b)| !(a == l && b == l))
            .collect();
        if pairs.len() != n {
            return Err(format!(
                "{} scalar products for {n} propagators: incomplete basis",
                pairs.len()
            ));
        }
        let mut m = vec![vec![Q::ZERO; n]; n];
        let mut c0 = vec![Q::ZERO; n];
        for i in 0..n {
            for (k, &(a, b)) in pairs.iter().enumerate() {
                let co = c[i][a] as i128 * c[i][b] as i128 * if a == b { 1 } else { 2 };
                m[i][k] = Q::int(co);
            }
            c0[i] = Q::int(c[i][l] as i128 * c[i][l] as i128);
        }
        let mi = inverse_q(&m)?; // mi[pair][i]
        Ok(QuadBasis {
            loops: l,
            pairs,
            minv: mi,
            c0,
        })
    }

    /// `(w . (k, q))^2 = sum_i x_i D_i + constant q^2`
    fn decompose(&self, w: &[i64]) -> Result<(Vec<(usize, Q)>, Q), String> {
        let l = self.loops;
        let n = self.c0.len();
        let mut x = vec![Q::ZERO; n];
        for (k, &(a, b)) in self.pairs.iter().enumerate() {
            let v = w[a] as i128 * w[b] as i128 * if a == b { 1 } else { 2 };
            if v == 0 {
                continue;
            }
            let vq = Q::int(v);
            for (i, xi) in x.iter_mut().enumerate() {
                let mm = self.minv[k][i];
                if !mm.is_zero() {
                    *xi = xi.add(vq.mul(mm)?)?;
                }
            }
        }
        let mut constant = Q::int(w[l] as i128 * w[l] as i128);
        for (i, xi) in x.iter().enumerate() {
            if !xi.is_zero() && !self.c0[i].is_zero() {
                constant = constant.sub(xi.mul(self.c0[i])?)?;
            }
        }
        Ok((
            x.into_iter()
                .enumerate()
                .filter(|(_, q)| !q.is_zero())
                .collect(),
            constant,
        ))
    }
}

/// Context of one family + chart: momenta, eps shifts, line basis, lookups.
struct Geometry<'f> {
    family: &'f Family,
    chart: Vec<bool>,
    w: usize,
    lines: Vec<usize>,
    basis: Vec<usize>,
    /// adj(B) (integer, row-major w x w) and det(B), B = [c_basis; e_q]
    adj: Vec<i128>,
    det_b: i128,
    /// canonical momentum -> slot, chart lines / other slots
    line_of: HashMap<Vec<i64>, usize>,
    other_of: HashMap<Vec<i64>, usize>,
    quad: QuadBasis,
}

impl<'f> Geometry<'f> {
    fn new(family: &'f Family, chart: &[bool]) -> Result<Geometry<'f>, GroupNotDerived> {
        let n = family.n;
        if chart.len() != n {
            return Err(GroupNotDerived::Family(format!(
                "chart of length {} for arity {n}",
                chart.len()
            )));
        }
        let l = family.loops;
        let w = l + 1;
        let c = family.momenta();
        if c.iter().any(|v| v.len() != w) {
            return Err(GroupNotDerived::Family(
                "momenta not over (loops, q): exactly one external momentum is supported".into(),
            ));
        }
        let lines: Vec<usize> = (0..n).filter(|&i| chart[i]).collect();
        let mut line_of = HashMap::new();
        let mut other_of = HashMap::new();
        for i in 0..n {
            let key = canon(&c[i]);
            if key.iter().all(|&x| x == 0) {
                return Err(GroupNotDerived::Family(format!(
                    "slot {i} has zero momentum"
                )));
            }
            let map = if chart[i] {
                &mut line_of
            } else {
                &mut other_of
            };
            if map.insert(key, i).is_some() {
                return Err(GroupNotDerived::Family(format!(
                    "two slots with the same momentum up to sign (slot {i})"
                )));
            }
        }
        // greedy line basis of the loop parts
        let mut basis: Vec<usize> = Vec::new();
        let mut rows: Vec<Vec<Q>> = Vec::new();
        for &i in &lines {
            if basis.len() == l {
                break;
            }
            let mut cand = rows.clone();
            cand.push(c[i][..l].iter().map(|&x| Q::int(x as i128)).collect());
            if rank_q(&cand) == cand.len() {
                rows = cand;
                basis.push(i);
            }
        }
        if basis.len() < l {
            return Err(GroupNotDerived::NoLineBasis);
        }
        let b: Vec<Vec<i128>> = basis
            .iter()
            .map(|&i| c[i].iter().map(|&x| x as i128).collect())
            .chain(std::iter::once(
                (0..w).map(|j| i128::from(j == l)).collect(),
            ))
            .collect();
        let det_b =
            det_i128(b.clone()).ok_or_else(|| GroupNotDerived::Overflow("det(B)".into()))?;
        let bq: Vec<Vec<Q>> = b
            .iter()
            .map(|r| r.iter().map(|&x| Q::int(x)).collect())
            .collect();
        let inv = inverse_q(&bq).map_err(GroupNotDerived::Overflow)?;
        let mut adj = vec![0i128; w * w];
        for r in 0..w {
            for s in 0..w {
                let v = inv[r][s]
                    .mul(Q::int(det_b))
                    .map_err(GroupNotDerived::Overflow)?;
                if v.d != 1 {
                    return Err(GroupNotDerived::Overflow("adjugate not integer".into()));
                }
                adj[r * w + s] = v.n;
            }
        }
        let quad = QuadBasis::new(family).map_err(GroupNotDerived::Family)?;
        Ok(Geometry {
            family,
            chart: chart.to_vec(),
            w,
            lines,
            basis,
            adj,
            det_b,
            line_of,
            other_of,
            quad,
        })
    }

    /// T = adj(B) B_img / det(B) when integer.
    fn solve_t(&self, img: &[Vec<i64>]) -> Option<Vec<i64>> {
        let w = self.w;
        let mut t = vec![0i64; w * w];
        for r in 0..w {
            for s in 0..w {
                let mut acc: i128 = 0;
                for (k, row) in img.iter().enumerate() {
                    acc = acc.checked_add(self.adj[r * w + k].checked_mul(row[s] as i128)?)?;
                }
                if acc % self.det_b != 0 {
                    return None;
                }
                t[r * w + s] = i64::try_from(acc / self.det_b).ok()?;
            }
        }
        Some(t)
    }

    /// q -> +-q and |det A| = 1.
    fn unimodular(&self, t: &[i64]) -> Result<(), String> {
        let w = self.w;
        let l = w - 1;
        if (0..l).any(|j| t[l * w + j] != 0) || t[l * w + l].abs() != 1 {
            return Err("q is not mapped to +-q".into());
        }
        let a: Vec<Vec<i128>> = (0..l)
            .map(|i| (0..l).map(|j| t[i * w + j] as i128).collect())
            .collect();
        match det_i128(a) {
            Some(d) if d.abs() == 1 => Ok(()),
            Some(d) => Err(format!("loop part has determinant {d}")),
            None => Err("determinant overflow".into()),
        }
    }

    /// The full slot action of T, or the reason it is not a chart automorphism.
    fn action_of(&self, t: &[i64]) -> Result<Vec<SlotImage>, String> {
        let c = self.family.momenta();
        let ks = &self.family.kshift;
        let n = self.family.n;
        let mut out = Vec::with_capacity(n);
        let mut hit = vec![false; n];
        for i in 0..n {
            let img = vec_times(&c[i], t, self.w).ok_or("momentum overflow")?;
            let key = canon(&img);
            let exact = if self.chart[i] {
                self.line_of.get(&key)
            } else {
                self.other_of.get(&key)
            };
            match exact {
                Some(&tg) => {
                    if ks[i] != ks[tg] {
                        return Err(format!("slot {i} -> {tg} changes the eps power shift"));
                    }
                    if std::mem::replace(&mut hit[tg], true) {
                        return Err(format!("slot {tg} hit twice"));
                    }
                    out.push(SlotImage::Exact {
                        target: tg,
                        flip: false,
                    });
                }
                None if self.chart[i] => {
                    return Err(format!("chart line {i} is not mapped onto a chart line"));
                }
                None => {
                    if ks[i] != 0 {
                        return Err(format!("slot {i} with an eps shift goes to a combination"));
                    }
                    let (terms, constant) = self.quad.decompose(&img)?;
                    out.push(SlotImage::Linear { terms, constant });
                }
            }
        }
        Ok(out)
    }
}

fn rank_q(rows: &[Vec<Q>]) -> usize {
    let mut a: Vec<Vec<Q>> = rows.to_vec();
    let m = a.first().map_or(0, Vec::len);
    let mut r = 0;
    for c in 0..m {
        let Some(p) = (r..a.len()).find(|&i| !a[i][c].is_zero()) else {
            continue;
        };
        a.swap(r, p);
        for i in r + 1..a.len() {
            if a[i][c].is_zero() {
                continue;
            }
            let Ok(f) = a[i][c].div(a[r][c]) else {
                return 0;
            };
            let pivot = a[r].clone();
            for (x, y) in a[i].iter_mut().zip(&pivot) {
                let Ok(v) = f.mul(*y).and_then(|z| x.sub(z)) else {
                    return 0;
                };
                *x = v;
            }
        }
        r += 1;
    }
    r
}

// ------------------------------------------------------------------ derivation --------------------------------------

#[derive(Clone, Debug)]
pub struct DeriveOptions {
    /// bound on the enumeration (injective line-image tuples x signs); above it: `SearchBound`
    pub max_candidates: u64,
}

impl Default for DeriveOptions {
    fn default() -> Self {
        DeriveOptions {
            max_candidates: 50_000_000,
        }
    }
}

/// Why the group was not derived (typed; the caller then needs a supplied group).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum GroupNotDerived {
    /// the chart lines do not span the loop momenta
    NoLineBasis,
    /// the enumeration would exceed `max_candidates`
    SearchBound {
        candidates: u128,
        bound: u64,
    },
    /// the family is outside the supported class (one external momentum, distinct momenta, complete basis)
    Family(String),
    Overflow(String),
}

/// Statistics of a derivation.
#[derive(Clone, Debug, Default, Serialize)]
pub struct DeriveStats {
    pub candidates: u128,
    pub unimodular: u64,
    pub automorphisms: u64,
}

/// INDEPENDENT derivation of the chart's symmetry group from the family text alone: every integer T with
/// q -> +-q, |det A| = 1 that maps each chart line momentum to +- a chart line momentum (eps shift kept), found by
/// fixing the images of a basis of chart lines (injective, with signs); the full slot action of each T (`Exact` onto
/// one slot of the same kind, else the exact `Linear` decomposition). Deduplicated by action, identity dropped.
pub fn derive_group(
    family: &Family,
    chart: &[bool],
    opts: &DeriveOptions,
) -> Result<(SymGroup, DeriveStats), GroupNotDerived> {
    let g = Geometry::new(family, chart)?;
    let l = family.loops;
    let nl = g.lines.len() as u128;
    let mut candidates: u128 = 1;
    for j in 0..l as u128 {
        candidates = candidates.saturating_mul(nl.saturating_sub(j));
    }
    candidates = candidates.saturating_mul(1u128 << (l + 1).min(127));
    if candidates > opts.max_candidates as u128 {
        return Err(GroupNotDerived::SearchBound {
            candidates,
            bound: opts.max_candidates,
        });
    }
    let c = family.momenta();
    let ks = &family.kshift;
    let mut stats = DeriveStats {
        candidates,
        ..DeriveStats::default()
    };
    let mut seen: BTreeSet<Vec<String>> = BTreeSet::new();
    let mut elements: Vec<SymElement> = Vec::new();
    // iterative DFS over (target, sign) of each basis line
    let mut chosen: Vec<(usize, i64)> = Vec::with_capacity(l);
    let mut used = vec![false; family.n];
    let mut stack: Vec<usize> = vec![0]; // per depth: next option index (target position * 2 + sign)
    let opts_per_depth = g.lines.len() * 2;
    while let Some(top) = stack.last_mut() {
        let depth = chosen.len();
        if depth == l {
            for sq in [1i64, -1] {
                let mut img: Vec<Vec<i64>> = chosen
                    .iter()
                    .map(|&(t, s)| c[t].iter().map(|x| x * s).collect())
                    .collect();
                img.push((0..g.w).map(|j| if j == l { sq } else { 0 }).collect());
                let Some(t) = g.solve_t(&img) else { continue };
                if g.unimodular(&t).is_err() {
                    continue;
                }
                stats.unimodular += 1;
                let Ok(action) = g.action_of(&t) else {
                    continue;
                };
                stats.automorphisms += 1;
                let el = SymElement {
                    matrix: Some(t),
                    action,
                };
                if el.is_identity() {
                    continue;
                }
                let key: Vec<String> = el.action.iter().map(|s| slot_json(s).to_string()).collect();
                if seen.insert(key) {
                    elements.push(el);
                }
            }
            stack.pop();
            if let Some((t, _)) = chosen.pop() {
                used[t] = false;
            }
            continue;
        }
        if *top >= opts_per_depth {
            stack.pop();
            if let Some((t, _)) = chosen.pop() {
                used[t] = false;
            }
            continue;
        }
        let o = *top;
        *top += 1;
        let tgt = g.lines[o / 2];
        let sign = if o % 2 == 0 { 1 } else { -1 };
        if used[tgt] || ks[tgt] != ks[g.basis[depth]] {
            continue;
        }
        used[tgt] = true;
        chosen.push((tgt, sign));
        stack.push(0);
    }
    Ok((
        SymGroup {
            arity: family.n,
            loops: l,
            chart: chart.to_vec(),
            elements,
        },
        stats,
    ))
}

// ------------------------------------------------------------------ verification ------------------------------------

/// Where the group used by the identity test comes from (recorded with its digest).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Provenance {
    /// given as data (file, engine table record, checker map); `source` names it
    Supplied { source: String },
    /// computed by [`derive_group`] from the family text
    Derived,
    /// derived AND equal (by [`cross_check`]) to every listed source
    CrossChecked { sources: Vec<String> },
}

/// What was proved for one element.
#[derive(Clone, Debug, Serialize)]
pub struct ElementProof {
    /// the T used for the proof
    pub matrix: Vec<i64>,
    /// T was not given and was reconstructed from the line action
    pub reconstructed: bool,
    pub exact_slots: usize,
    pub linear_slots: usize,
    pub unknown_slots: usize,
}

/// A group every element of which passed the exact momentum proof. The only form [`rows_at`] and the identity test
/// accept.
#[derive(Clone, Debug)]
pub struct VerifiedGroup {
    pub group: SymGroup,
    /// sha256 of the family text the proofs were made on (the identity test refuses the group on another family)
    pub family_digest: String,
    pub provenance: Provenance,
    pub digest: String,
    pub proofs: Vec<ElementProof>,
}

/// A refused group: the first element that failed and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GroupRefused {
    pub element: Option<usize>,
    pub reason: String,
}

fn check_against(g: &Geometry, el: &SymElement, t: &[i64]) -> Result<ElementProof, String> {
    g.unimodular(t)?;
    let computed = g.action_of(t)?;
    let (mut exact_slots, mut linear_slots, mut unknown_slots) = (0, 0, 0);
    for (i, (given, got)) in el.action.iter().zip(&computed).enumerate() {
        match given {
            SlotImage::Unknown => unknown_slots += 1,
            SlotImage::Exact { flip: true, .. } => {
                return Err(format!("slot {i}: a squared propagator cannot change sign"));
            }
            SlotImage::Exact { target, .. } => {
                // the stated image must be the geometric one (an ISP stated exact onto a line is compared by value)
                let ok = match got {
                    SlotImage::Exact { target: t2, .. } => t2 == target,
                    SlotImage::Linear { terms, constant } => {
                        constant.is_zero() && terms.len() == 1 && terms[0] == (*target, Q::ONE)
                    }
                    SlotImage::Unknown => false,
                };
                if !ok {
                    return Err(format!(
                        "slot {i}: stated image D{target} is not the momentum image"
                    ));
                }
                exact_slots += 1;
            }
            SlotImage::Linear { .. } => {
                let as_linear = match got {
                    SlotImage::Exact { target, .. } => SlotImage::Linear {
                        terms: vec![(*target, Q::ONE)],
                        constant: Q::ZERO,
                    },
                    x => x.clone(),
                };
                if *given != as_linear {
                    return Err(format!(
                        "slot {i}: stated linear image differs from the momentum image"
                    ));
                }
                linear_slots += 1;
            }
        }
    }
    Ok(ElementProof {
        matrix: t.to_vec(),
        reconstructed: false,
        exact_slots,
        linear_slots,
        unknown_slots,
    })
}

/// The exact momentum proof of every element of `group` on `family` (interfaces/ibp-rows.md 10.3): a T (given, or
/// reconstructed from the line action over all sign choices on a line basis) with q -> +-q, |det A| = 1, every chart
/// line onto its stated chart line, every stated slot image equal to the geometric one, no flip. Any failure refuses
/// the WHOLE group (a wrong element must never reach a span test).
pub fn verify_group(
    family: &Family,
    group: SymGroup,
    provenance: Provenance,
) -> Result<VerifiedGroup, GroupRefused> {
    let refuse = |element: Option<usize>, reason: String| GroupRefused { element, reason };
    if group.arity != family.n {
        return Err(refuse(
            None,
            format!("group arity {} != family {}", group.arity, family.n),
        ));
    }
    if group.loops != 0 && group.loops != family.loops {
        return Err(refuse(
            None,
            format!("group loops {} != family {}", group.loops, family.loops),
        ));
    }
    let g = Geometry::new(family, &group.chart).map_err(|e| refuse(None, format!("{e:?}")))?;
    let l = family.loops;
    let w = l + 1;
    let c = family.momenta();
    let mut proofs = Vec::with_capacity(group.elements.len());
    for (k, el) in group.elements.iter().enumerate() {
        if el.action.len() != family.n {
            return Err(refuse(Some(k), "action length".into()));
        }
        let Some(perm) = el.line_perm(&group.chart) else {
            return Err(refuse(Some(k), "a chart line has no exact image".into()));
        };
        if perm.iter().any(|&t| !group.chart[t]) {
            return Err(refuse(
                Some(k),
                "a chart line goes onto a non-line slot".into(),
            ));
        }
        let proof = match &el.matrix {
            Some(t) => {
                if t.len() != w * w {
                    return Err(refuse(Some(k), "matrix size".into()));
                }
                check_against(&g, el, t).map_err(|e| refuse(Some(k), e))?
            }
            None => {
                // reconstruct T from the images of the basis lines, all sign choices
                let targets: Vec<usize> = g
                    .basis
                    .iter()
                    .map(|&b| match el.action[b] {
                        SlotImage::Exact { target, .. } => target,
                        _ => usize::MAX,
                    })
                    .collect();
                let mut last = String::from("no sign choice gives an integer unimodular T");
                let mut found = None;
                for signs in 0u64..(1u64 << w) {
                    let mut img: Vec<Vec<i64>> = targets
                        .iter()
                        .enumerate()
                        .map(|(j, &t)| {
                            let s = if signs >> j & 1 == 1 { -1 } else { 1 };
                            c[t].iter().map(|x| x * s).collect()
                        })
                        .collect();
                    let sq = if signs >> l & 1 == 1 { -1 } else { 1 };
                    img.push((0..w).map(|j| if j == l { sq } else { 0 }).collect());
                    let Some(t) = g.solve_t(&img) else { continue };
                    match check_against(&g, el, &t) {
                        Ok(mut p) => {
                            p.reconstructed = true;
                            found = Some(p);
                            break;
                        }
                        Err(e) => last = e,
                    }
                }
                found.ok_or_else(|| refuse(Some(k), last))?
            }
        };
        proofs.push(proof);
    }
    let digest = group.digest();
    Ok(VerifiedGroup {
        group,
        family_digest: family.source_digest.clone(),
        provenance,
        digest,
        proofs,
    })
}

// ------------------------------------------------------------------ cross-check -------------------------------------

/// Result of comparing two groups on the key action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum GroupCheck {
    /// same chart, same line permutations, and every slot image both sides state is equal
    Equal {
        elements: usize,
        /// slot images compared exactly / skipped because one side leaves them unknown
        compared_slots: usize,
        skipped_slots: usize,
    },
    Mismatch {
        reason: String,
        only_a: Vec<Vec<usize>>,
        only_b: Vec<Vec<usize>>,
        differing: Vec<String>,
    },
}

type PermIndex<'a> = BTreeMap<Vec<usize>, Vec<&'a SymElement>>;

fn perm_index(g: &SymGroup) -> Result<PermIndex<'_>, String> {
    let mut m: PermIndex<'_> = BTreeMap::new();
    for e in &g.elements {
        m.entry(
            e.line_perm(&g.chart)
                .ok_or("an element without exact line images")?,
        )
        .or_default()
        .push(e);
    }
    Ok(m)
}

/// Compare two groups (e.g. derived vs supplied vs engine record vs checker map). Elements are matched by their line
/// permutation; for each matched pair every slot image stated by BOTH is compared exactly (`Exact` onto a slot equals
/// `Linear` `1 * D_slot`). A mismatch is a typed verdict.
pub fn cross_check(a: &SymGroup, b: &SymGroup) -> GroupCheck {
    let mismatch = |reason: String| GroupCheck::Mismatch {
        reason,
        only_a: Vec::new(),
        only_b: Vec::new(),
        differing: Vec::new(),
    };
    if a.arity != b.arity || a.chart != b.chart {
        return mismatch(format!(
            "arity/chart differ: {} {} vs {} {}",
            a.arity,
            a.chart_text(),
            b.arity,
            b.chart_text()
        ));
    }
    let (ia, ib) = match (perm_index(a), perm_index(b)) {
        (Ok(x), Ok(y)) => (x, y),
        (Err(e), _) | (_, Err(e)) => return mismatch(e),
    };
    let only_a: Vec<Vec<usize>> = ia
        .keys()
        .filter(|k| !ib.contains_key(*k))
        .cloned()
        .collect();
    let only_b: Vec<Vec<usize>> = ib
        .keys()
        .filter(|k| !ia.contains_key(*k))
        .cloned()
        .collect();
    let norm = |s: &SlotImage| match s {
        SlotImage::Exact {
            target,
            flip: false,
        } => SlotImage::Linear {
            terms: vec![(*target, Q::ONE)],
            constant: Q::ZERO,
        },
        x => x.clone(),
    };
    let (mut compared, mut skipped) = (0usize, 0usize);
    let mut differing = Vec::new();
    for (perm, ea) in &ia {
        let Some(eb) = ib.get(perm) else { continue };
        // every element of a with this line permutation must agree with some element of b (and vice versa)
        let agrees = |x: &SymElement, y: &SymElement| -> Option<(usize, usize)> {
            let (mut cmp, mut skp) = (0, 0);
            for (sx, sy) in x.action.iter().zip(&y.action) {
                if *sx == SlotImage::Unknown || *sy == SlotImage::Unknown {
                    skp += 1;
                } else if norm(sx) == norm(sy) {
                    cmp += 1;
                } else {
                    return None;
                }
            }
            Some((cmp, skp))
        };
        for x in ea {
            match eb.iter().find_map(|y| agrees(x, y)) {
                Some((cmp, skp)) => {
                    compared += cmp;
                    skipped += skp;
                }
                None => differing.push(format!("line permutation {perm:?}: slot images differ")),
            }
        }
        for y in eb {
            if !ea.iter().any(|x| agrees(x, y).is_some()) {
                differing.push(format!(
                    "line permutation {perm:?}: an element of b has no match in a"
                ));
            }
        }
    }
    if only_a.is_empty() && only_b.is_empty() && differing.is_empty() {
        GroupCheck::Equal {
            elements: ia.values().map(Vec::len).sum(),
            compared_slots: compared,
            skipped_slots: skipped,
        }
    } else {
        GroupCheck::Mismatch {
            reason: format!(
                "{} permutations only in a, {} only in b, {} differing",
                only_a.len(),
                only_b.len(),
                differing.len()
            ),
            only_a,
            only_b,
            differing,
        }
    }
}

// ------------------------------------------------------------------ rows --------------------------------------------

/// Which symmetry rows are generated (interfaces/ibp-rows.md 10.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Default)]
pub enum SymRowMode {
    /// the engine's definition: one-term rows, chart keys (every chart line >= 1), elements exact on the seed
    #[default]
    Engine,
    /// the checker's superset: also chart lines <= 0, and multi-term numerator images expanded exactly
    Expanded,
}

/// A seed / column: (symbolic-slot mask, values); mask 0 = integer key; a set bit = offset to the slot's symbol.
pub type PatKey = (u64, Vec<i64>);

/// One symmetry row at a seed: `sum c_j I(key_j) = 0` (terms sorted by key; the seed has coefficient 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymRow {
    pub element: usize,
    pub terms: Vec<(PatKey, Q)>,
}

/// The symmetry rows at one seed. Symbolic slots (mask bits) must be fixed pointwise by an element for it to give a
/// row (the engine's case-search rule; symbolic chart lines count as >= 1). `Engine`: the seed's constant chart lines
/// must be >= 1, every nonzero constant slot must have an `Exact` image. `Expanded`: `Linear` images of NEGATIVE
/// constant slots are expanded (`(sum c_m D_m + c0)^{-a}`; a factor D_m lowers index m by 1). `max_terms` bounds the
/// expansion of one row (Err over it, or on rational overflow).
pub fn rows_at(
    group: &VerifiedGroup,
    seed: &PatKey,
    mode: SymRowMode,
    max_terms: usize,
) -> Result<Vec<SymRow>, String> {
    let g = &group.group;
    let (mask, vals) = seed;
    let n = g.arity;
    if vals.len() != n {
        return Err(format!("seed of length {} for arity {n}", vals.len()));
    }
    let sym = |i: usize| mask >> i & 1 == 1;
    if mode == SymRowMode::Engine && (0..n).any(|i| g.chart[i] && !sym(i) && vals[i] < 1) {
        return Ok(Vec::new());
    }
    let mut out: Vec<SymRow> = Vec::new();
    'el: for (k, e) in g.elements.iter().enumerate() {
        let mut base = vec![0i64; n];
        let mut taken = vec![false; n];
        let mut neg = false;
        for i in (0..n).filter(|&i| sym(i)) {
            if e.action[i]
                != (SlotImage::Exact {
                    target: i,
                    flip: false,
                })
            {
                continue 'el;
            }
            base[i] = vals[i];
            taken[i] = true;
        }
        let mut expand: Vec<(usize, i64)> = Vec::new();
        for i in (0..n).filter(|&i| !sym(i) && vals[i] != 0) {
            match &e.action[i] {
                SlotImage::Exact { target, flip } => {
                    if taken[*target] {
                        continue 'el;
                    }
                    taken[*target] = true;
                    base[*target] = vals[i];
                    if *flip && vals[i] % 2 != 0 {
                        neg = !neg;
                    }
                }
                SlotImage::Linear { .. } if mode == SymRowMode::Expanded && vals[i] < 0 => {
                    expand.push((i, -vals[i]));
                }
                _ => continue 'el,
            }
        }
        // expansion: decrement vector -> coefficient
        let mut terms: BTreeMap<Vec<i64>, Q> = BTreeMap::new();
        terms.insert(vec![0; n], Q::ONE);
        for (i, times) in expand {
            let SlotImage::Linear {
                terms: lin,
                constant,
            } = &e.action[i]
            else {
                unreachable!()
            };
            for _ in 0..times {
                let mut next: BTreeMap<Vec<i64>, Q> = BTreeMap::new();
                for (dec, c) in &terms {
                    if !constant.is_zero() {
                        let v = c.mul(*constant)?;
                        let en = next.entry(dec.clone()).or_insert(Q::ZERO);
                        *en = en.add(v)?;
                    }
                    for (m, a) in lin {
                        let mut d2 = dec.clone();
                        d2[*m] += 1;
                        let v = c.mul(*a)?;
                        let en = next.entry(d2).or_insert(Q::ZERO);
                        *en = en.add(v)?;
                    }
                }
                next.retain(|_, c| !c.is_zero());
                if next.len() > max_terms {
                    return Err(format!(
                        "symmetry row expansion bound: element {k} gives more than {max_terms} terms"
                    ));
                }
                terms = next;
            }
        }
        let mut row: BTreeMap<Vec<i64>, Q> = BTreeMap::new();
        row.insert(vals.clone(), Q::ONE);
        for (dec, c) in terms {
            let key: Vec<i64> = base.iter().zip(&dec).map(|(b, d)| b - d).collect();
            let v = if neg { c } else { c.neg() };
            let en = row.entry(key).or_insert(Q::ZERO);
            *en = en.add(v)?;
        }
        row.retain(|_, c| !c.is_zero());
        if row.is_empty() {
            continue;
        }
        out.push(SymRow {
            element: k,
            terms: row.into_iter().map(|(v, c)| ((*mask, v), c)).collect(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUNSET: &str = "I(name(S),loops(k1,k2),externals(q),dimension(d),prop(D1,(k1)^2,1),prop(D2,(k2)^2,1),prop(D3,(k1+k2+q)^2,1),prop(D4,(k1+q)^2,0),prop(D5,(k2+q)^2,0),gram(q,q,1))";

    #[test]
    fn sunset_group_is_s3_with_one_multi_term_numerator_map() {
        let f = Family::parse(SUNSET).unwrap();
        let chart = [true, true, true, false, false];
        let (g, st) = derive_group(&f, &chart, &DeriveOptions::default()).unwrap();
        assert_eq!(g.elements.len(), 5, "{:?}", g.to_json());
        assert!(st.automorphisms >= 5);
        // the swap k1 <-> k2: D1<->D2, D4<->D5 exact
        assert!(g.elements.iter().any(|e| e.action[0]
            == SlotImage::Exact {
                target: 1,
                flip: false
            }
            && e.action[2]
                == SlotImage::Exact {
                    target: 2,
                    flip: false
                }
            && e.action[3]
                == SlotImage::Exact {
                    target: 4,
                    flip: false
                }));
        // some element maps an ISP to a combination: (k1+k2)^2 = D1 + D2 + D3 - D4 - D5 + q^2 shape
        assert!(g.elements.iter().any(|e| {
            e.action
                .iter()
                .any(|s| matches!(s, SlotImage::Linear { .. }))
        }));
        let v = verify_group(&f, g.clone(), Provenance::Derived).unwrap();
        assert_eq!(v.proofs.len(), 5);
        // round trip through JSON and through a matrix-free copy (reconstruction)
        let back = SymGroup::from_json(&g.to_json()).unwrap();
        assert_eq!(back, g);
        let mut nomat = g.clone();
        for e in &mut nomat.elements {
            e.matrix = None;
        }
        let v2 = verify_group(
            &f,
            nomat,
            Provenance::Supplied {
                source: "test".into(),
            },
        )
        .unwrap();
        assert!(v2.proofs.iter().all(|p| p.reconstructed));
        assert_eq!(v2.digest, v.digest);
        assert!(matches!(cross_check(&g, &back), GroupCheck::Equal { .. }));
    }

    #[test]
    fn a_wrong_element_is_refused() {
        let f = Family::parse(SUNSET).unwrap();
        let chart = vec![true, true, true, false, false];
        // swap D1 <-> D2 but leave the ISPs in place: geometric image of D4 is D5, so the stated D4 -> D4 is wrong
        let bad = SymGroup {
            arity: 5,
            loops: 2,
            chart: chart.clone(),
            elements: vec![SymElement {
                matrix: None,
                action: vec![
                    SlotImage::Exact {
                        target: 1,
                        flip: false,
                    },
                    SlotImage::Exact {
                        target: 0,
                        flip: false,
                    },
                    SlotImage::Exact {
                        target: 2,
                        flip: false,
                    },
                    SlotImage::Exact {
                        target: 3,
                        flip: false,
                    },
                    SlotImage::Exact {
                        target: 4,
                        flip: false,
                    },
                ],
            }],
        };
        let r = verify_group(
            &f,
            bad,
            Provenance::Supplied {
                source: "planted".into(),
            },
        );
        assert!(r.is_err(), "{r:?}");
        // a flip is refused
        let flip = SymGroup {
            arity: 5,
            loops: 2,
            chart,
            elements: vec![SymElement {
                matrix: None,
                action: vec![
                    SlotImage::Exact {
                        target: 1,
                        flip: true,
                    },
                    SlotImage::Exact {
                        target: 0,
                        flip: false,
                    },
                    SlotImage::Exact {
                        target: 2,
                        flip: false,
                    },
                    SlotImage::Exact {
                        target: 4,
                        flip: false,
                    },
                    SlotImage::Exact {
                        target: 3,
                        flip: false,
                    },
                ],
            }],
        };
        assert!(verify_group(&f, flip, Provenance::Derived).is_err());
    }

    #[test]
    fn rows_engine_and_expanded() {
        let f = Family::parse(SUNSET).unwrap();
        let chart = [true, true, true, false, false];
        let (g, _) = derive_group(&f, &chart, &DeriveOptions::default()).unwrap();
        let v = verify_group(&f, g, Provenance::Derived).unwrap();
        // numerator-free dotted key: every element exact, images distinct from the key except the stabiliser
        let r = rows_at(&v, &(0, vec![2, 1, 1, 0, 0]), SymRowMode::Engine, 1000).unwrap();
        assert!(!r.is_empty());
        assert!(r.iter().all(|x| {
            x.terms.len() == 2
                && x.terms
                    .iter()
                    .any(|t| t.0 == (0, vec![2, 1, 1, 0, 0]) && t.1 == Q::ONE)
                && x.terms.iter().any(|t| t.1 == Q::ONE.neg())
        }));
        // with a numerator on D4: engine rows only from elements exact on D4; expanded rows from all
        let y = (0, vec![1, 1, 1, -1, 0]);
        let re = rows_at(&v, &y, SymRowMode::Engine, 1000).unwrap();
        let rx = rows_at(&v, &y, SymRowMode::Expanded, 1000).unwrap();
        assert!(rx.len() > re.len(), "{} vs {}", rx.len(), re.len());
        assert!(rx.iter().any(|x| x.terms.len() > 2));
        // subsector seed: no engine rows
        assert!(
            rows_at(&v, &(0, vec![0, 1, 1, 0, 0]), SymRowMode::Engine, 1000)
                .unwrap()
                .is_empty()
        );
        // bound
        let deep = (0, vec![1, 1, 1, -12, 0]);
        assert!(rows_at(&v, &deep, SymRowMode::Expanded, 3).is_err());
        // generic seed with a symbolic line: only elements fixing slot 0 pointwise
        let gs = (0b1, vec![0, 1, 1, 0, 0]);
        let rg = rows_at(&v, &gs, SymRowMode::Engine, 1000).unwrap();
        assert!(rg.iter().all(|x| v.group.elements[x.element].action[0]
            == SlotImage::Exact {
                target: 0,
                flip: false
            }));
    }
}
