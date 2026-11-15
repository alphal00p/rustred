//! Independent checker of a cone-coverage certificate (interfaces/parametric.md section 2). It shares NO decision code
//! with [`super::cone`]: no partition, no lattice regions, no Fourier-Motzkin. It reads the certificate's numbers and
//! asks the real dispatcher (`cover::Dispatcher::first_match`) at points:
//! 1. schema, version and the tables digest of the dispatcher it is given;
//! 2. every witness of an `uncovered` element: in the chart orthant, in the cone (own integer arithmetic), OPEN;
//! 3. every probe key: in the cone; a probe answered `[table, record]` is served by exactly that record; a probe
//!    answered open is open AND lies in the region of the element the certificate names (membership by its own exact
//!    rational elimination of `x(t) = key`, integrality, constraints `>= 0`);
//! 4. the verdict word against the lists (covered: no element meets the cone and no probe is open);
//! 5. optionally an EXHAUSTIVE box (small instances): every key of the box inside the cone is dispatched; an open key
//!    must lie in a listed element (and none may exist for `covered`).
//!
//! A cross-check of the evidence, not a second proof.

use serde_json::Value;

use crate::cover::{Dispatch, Dispatcher};
use crate::key::NodeFrame;
use crate::partition::DispatcherSource;

/// What the checker found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CheckReport {
    pub problems: Vec<String>,
    pub witnesses_checked: usize,
    pub probes_checked: usize,
    pub exhaustive_keys: u64,
    pub exhaustive_open: u64,
}

impl CheckReport {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

fn in_cone(cone: &[(Vec<i128>, i128)], k: &[i32]) -> bool {
    cone.iter().all(|(a, b)| {
        let mut s: i128 = 0;
        for (x, y) in a.iter().zip(k) {
            match x.checked_mul(i128::from(*y)).and_then(|p| s.checked_add(p)) {
                Some(v) => s = v,
                None => return false,
            }
        }
        s <= *b
    })
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    i128::try_from(a).unwrap_or(1)
}

/// A rational p/q (q > 0), checked arithmetic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Q(i128, i128);

impl Q {
    fn int(v: i128) -> Q {
        Q(v, 1)
    }
    fn norm(p: i128, q: i128) -> Option<Q> {
        if q == 0 {
            return None;
        }
        let g = gcd(p, q).max(1);
        let (mut p, mut q) = (p / g, q / g);
        if q < 0 {
            p = p.checked_neg()?;
            q = q.checked_neg()?;
        }
        Some(Q(p, q))
    }
    fn sub(self, o: Q) -> Option<Q> {
        Q::norm(
            self.0
                .checked_mul(o.1)?
                .checked_sub(o.0.checked_mul(self.1)?)?,
            self.1.checked_mul(o.1)?,
        )
    }
    fn mul(self, o: Q) -> Option<Q> {
        Q::norm(self.0.checked_mul(o.0)?, self.1.checked_mul(o.1)?)
    }
    fn div(self, o: Q) -> Option<Q> {
        Q::norm(self.0.checked_mul(o.1)?, self.1.checked_mul(o.0)?)
    }
}

/// The certificate's region of an element: x[s] = coef . t + c, constraints coef . t + c >= 0.
struct Reg {
    k: usize,
    x: Vec<(Vec<i128>, i128)>,
    cons: Vec<(Vec<i128>, i128)>,
}

fn affine(v: &Value) -> Option<(Vec<i128>, i128)> {
    let coef = v
        .get(0)?
        .as_array()?
        .iter()
        .map(|c| c.as_i64().map(i128::from))
        .collect::<Option<Vec<_>>>()?;
    Some((coef, i128::from(v.get(1)?.as_i64()?)))
}

/// A cell listed by its per-slot intervals as a region with the identity parameter map.
fn reg_of_box(v: &Value) -> Option<Reg> {
    let b = v.as_array()?;
    let k = b.len();
    let mut x = Vec::with_capacity(k);
    let mut cons = Vec::new();
    for (s, iv) in b.iter().enumerate() {
        let mut coef = vec![0i128; k];
        coef[s] = 1;
        x.push((coef.clone(), 0));
        if let Some(l) = iv.get(0).and_then(Value::as_i64) {
            cons.push((coef.clone(), -i128::from(l)));
        }
        if let Some(h) = iv.get(1).and_then(Value::as_i64) {
            cons.push((coef.iter().map(|v| -v).collect(), i128::from(h)));
        }
    }
    Some(Reg { k, x, cons })
}

fn reg_of(v: &Value) -> Option<Reg> {
    let k = usize::try_from(v.get("k")?.as_u64()?).ok()?;
    let x = v["x"]
        .as_array()?
        .iter()
        .map(affine)
        .collect::<Option<Vec<_>>>()?;
    let cons = v["cons"]
        .as_array()?
        .iter()
        .map(affine)
        .collect::<Option<Vec<_>>>()?;
    Some(Reg { k, x, cons })
}

/// Membership by exact rational elimination of x(t) = key: `Some(true/false)`, `None` = not decidable here (a
/// rank-deficient map or an overflow).
fn member(r: &Reg, key: &[i32]) -> Option<bool> {
    let n = r.x.len();
    if key.len() != n {
        return Some(false);
    }
    // augmented rows [coef | key - c]
    let mut m: Vec<Vec<Q>> = Vec::with_capacity(n);
    for (s, (coef, c)) in r.x.iter().enumerate() {
        if coef.len() != r.k {
            return None;
        }
        let mut row: Vec<Q> = coef.iter().map(|v| Q::int(*v)).collect();
        row.push(Q::int(i128::from(key[s]).checked_sub(*c)?));
        m.push(row);
    }
    let mut piv_cols = Vec::new();
    let mut row = 0usize;
    for col in 0..r.k {
        let Some(p) = (row..n).find(|&i| m[i][col].0 != 0) else {
            continue;
        };
        m.swap(row, p);
        let pv = m[row][col];
        for v in m[row].iter_mut().skip(col) {
            *v = v.div(pv)?;
        }
        let prow = m[row].clone();
        for (i, ri) in m.iter_mut().enumerate() {
            if i != row && ri[col].0 != 0 {
                let f = ri[col];
                for (x, pj) in ri.iter_mut().zip(prow.iter()).skip(col) {
                    *x = x.sub(f.mul(*pj)?)?;
                }
            }
        }
        piv_cols.push(col);
        row += 1;
        if row == n {
            break;
        }
    }
    // inconsistent rows
    if (row..n).any(|i| m[i][r.k].0 != 0) {
        return Some(false);
    }
    if piv_cols.len() < r.k {
        return None;
    }
    let mut t = vec![0i128; r.k];
    for (i, &c) in piv_cols.iter().enumerate() {
        let v = m[i][r.k];
        if v.1 != 1 {
            return Some(false);
        }
        t[c] = v.0;
    }
    for (coef, c) in &r.cons {
        let mut s = *c;
        for (a, b) in coef.iter().zip(&t) {
            s = s.checked_add(a.checked_mul(*b)?)?;
        }
        if s < 0 {
            return Some(false);
        }
    }
    Some(true)
}

fn is_open(disp: &Dispatcher, k: &[i32]) -> bool {
    !matches!(disp.first_match(k), Ok(Dispatch::Covered(_)))
}

fn key_of(v: &Value) -> Option<Vec<i32>> {
    v.as_array()?
        .iter()
        .map(|x| x.as_i64().and_then(|y| i32::try_from(y).ok()))
        .collect()
}

/// Check a certificate against the dispatcher it is about (module doc). `exhaustive`: `(line_max, numerator_min)` of
/// a box enumerated key by key (small instances only).
pub fn check_cone_certificate(
    cert: &Value,
    disp: &Dispatcher,
    frame: &NodeFrame,
    exhaustive: Option<(i32, i32)>,
) -> CheckReport {
    let mut rep = CheckReport::default();
    let bad = |s: String, rep: &mut CheckReport| rep.problems.push(s);
    if cert["schema"] != super::CONE_COVERAGE_SCHEMA
        || cert["version"].as_u64() != Some(u64::from(super::CONE_COVERAGE_VERSION))
    {
        bad(
            format!("schema / version {} {}", cert["schema"], cert["version"]),
            &mut rep,
        );
        return rep;
    }
    let digest = crate::economy::table_set_digest(&DispatcherSource::new(disp), frame);
    if cert["tables_digest"].as_str() != Some(digest.as_str()) {
        bad(
            format!("tables digest {} != {digest}", cert["tables_digest"]),
            &mut rep,
        );
    }
    let n = frame.arity();
    let cone: Vec<(Vec<i128>, i128)> = cert["cone"]["constraints"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    let coef = c
                        .get(0)?
                        .as_array()?
                        .iter()
                        .map(|x| x.as_i64().map(i128::from))
                        .collect::<Option<Vec<_>>>()?;
                    Some((coef, i128::from(c.get(1)?.as_i64()?)))
                })
                .collect()
        })
        .unwrap_or_default();
    let orth = |k: &[i32]| {
        k.len() == n
            && k.iter()
                .enumerate()
                .all(|(i, &v)| if frame.is_line(i) { v >= 1 } else { v <= 0 })
    };
    let listed = cert["listed"].as_array().cloned().unwrap_or_default();
    let mut regions: Vec<(u64, Option<Reg>)> = Vec::new();
    let mut n_uncovered = 0usize;
    for e in &listed {
        let idx = e["element"].as_u64().unwrap_or(u64::MAX);
        let reg = if e.get("box").is_some() {
            reg_of_box(&e["box"])
        } else {
            reg_of(&e["region"])
        };
        regions.push((idx, reg));
        if e["v"]["verdict"] == "uncovered" {
            n_uncovered += 1;
            match key_of(&e["v"]["witness"]) {
                Some(w) => {
                    rep.witnesses_checked += 1;
                    if !orth(&w) || !in_cone(&cone, &w) {
                        bad(
                            format!("witness {w:?} outside the cone / orthant"),
                            &mut rep,
                        );
                    } else if !is_open(disp, &w) {
                        bad(
                            format!("witness {w:?} is served by the dispatcher"),
                            &mut rep,
                        );
                    } else if let Some(r) = if e.get("box").is_some() {
                        reg_of_box(&e["box"])
                    } else {
                        reg_of(&e["region"])
                    } && member(&r, &w) == Some(false)
                    {
                        bad(
                            format!("witness {w:?} not in its element's region"),
                            &mut rep,
                        );
                    }
                }
                None => bad(format!("element {idx}: unreadable witness"), &mut rep),
            }
        }
    }
    let mut open_probes = 0usize;
    for p in cert["probes"].as_array().cloned().unwrap_or_default() {
        let Some(k) = key_of(&p["key"]) else {
            bad("unreadable probe key".into(), &mut rep);
            continue;
        };
        rep.probes_checked += 1;
        if !orth(&k) || !in_cone(&cone, &k) {
            bad(format!("probe {k:?} outside the cone"), &mut rep);
            continue;
        }
        let got = disp.first_match(&k);
        match (&p["answer"], got) {
            (Value::Array(a), Ok(Dispatch::Covered(r))) => {
                if a.first().and_then(Value::as_u64) != Some(r.table as u64)
                    || a.get(1).and_then(Value::as_u64) != Some(r.record as u64)
                {
                    bad(
                        format!(
                            "probe {k:?}: served by ({}, {}), certificate {:?}",
                            r.table, r.record, a
                        ),
                        &mut rep,
                    );
                }
            }
            (Value::Null, Ok(Dispatch::Covered(r))) => bad(
                format!(
                    "probe {k:?}: certificate says open, served by ({}, {})",
                    r.table, r.record
                ),
                &mut rep,
            ),
            (Value::Array(_), other) => bad(
                format!("probe {k:?}: certificate says served, dispatcher {other:?}"),
                &mut rep,
            ),
            (Value::Null, _) => {
                open_probes += 1;
                let el = p["element"].as_u64();
                let inside = el
                    .and_then(|i| regions.iter().find(|(j, _)| *j == i))
                    .and_then(|(_, r)| r.as_ref())
                    .and_then(|r| member(r, &k));
                if inside != Some(true) {
                    bad(
                        format!("open probe {k:?} not verified inside element {el:?}"),
                        &mut rep,
                    );
                }
            }
            (a, _) => bad(format!("probe {k:?}: answer {a}"), &mut rep),
        }
    }
    match cert["verdict"].as_str() {
        Some("covered") => {
            if !listed.is_empty() || open_probes > 0 {
                bad(
                    format!(
                        "verdict covered with {} listed element(s) and {open_probes} open probe(s)",
                        listed.len()
                    ),
                    &mut rep,
                );
            }
        }
        Some("uncovered") => {
            if n_uncovered == 0 {
                bad(
                    "verdict uncovered without an uncovered element".into(),
                    &mut rep,
                );
            }
        }
        Some("undecided") => {}
        v => bad(format!("verdict {v:?}"), &mut rep),
    }
    if let Some((lmax, nmin)) = exhaustive {
        let lo: Vec<i32> = (0..n)
            .map(|i| if frame.is_line(i) { 1 } else { nmin })
            .collect();
        let hi: Vec<i32> = (0..n)
            .map(|i| if frame.is_line(i) { lmax } else { 0 })
            .collect();
        let mut k = lo.clone();
        let covered_verdict = cert["verdict"] == "covered";
        'outer: loop {
            if in_cone(&cone, &k) {
                rep.exhaustive_keys += 1;
                if is_open(disp, &k) {
                    rep.exhaustive_open += 1;
                    let inside = regions
                        .iter()
                        .any(|(_, r)| r.as_ref().and_then(|r| member(r, &k)) == Some(true));
                    if covered_verdict || !inside {
                        bad(
                            format!(
                                "exhaustive: open key {k:?} not inside a listed element (verdict {})",
                                cert["verdict"]
                            ),
                            &mut rep,
                        );
                        if rep.problems.len() > 50 {
                            break 'outer;
                        }
                    }
                }
            }
            let mut s = 0;
            loop {
                if s == n {
                    break 'outer;
                }
                if k[s] < hi[s] {
                    k[s] += 1;
                    break;
                }
                k[s] = lo[s];
                s += 1;
            }
        }
    }
    rep
}
