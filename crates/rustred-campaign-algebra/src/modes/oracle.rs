//! THE SHARED-LOGIC ORACLE of the `Both` mode (rows4, re-audit RA3-M3). Both algebra backends run the SAME domain logic
//! (`ibp_rows`, `replay_base`: applicability, served set, seed placement, IBP row assembly, zero-sector columns, rule
//! vector, controls, the combination of cells and points); a defect there is invisible to a comparison of the two
//! backends (a sign flip in the rule vector makes a TRUE rule `AgreeFail`; a "never fail" makes false rules
//! `AgreePass`). This module re-decides those SHARED DECISIONS by brute force at every integer key of the run, with
//! code of its own (std and the referee's routines only; nothing from `ibp_rows` / `replay_base` except the rule's
//! parsed fields and the data types):
//!
//! - its own reader of the family text (loops, propagator momenta, power shifts) and its own derivation of the IBP
//!   operators from the momenta (scalar products by an exact inverse over Q);
//! - zero sectors by Lee's criterion on the SUPPORTS of U and F computed by Cauchy-Binet (a monomial x^S is in U iff
//!   the minor of the loop-momentum coefficients on S is nonzero; in the bordered F with the external column), eps
//!   axes always active: not the shared U / F assembly;
//! - applicability by the definition (target constants, fixed slots, case equations zero, no exception branch all
//!   zero, chart orthant), the seed keys, the rule vector, membership, the +1 mutant and corner controls, the cell and
//!   point combination, at the recorded (prime, d) of every cell.
//!
//! A difference is reported as "shared logic differs from the oracle" (`Triage::SharedLogicDiffers`), distinct from the
//! agree / differ verdicts. For a rule that both backends agree FAILS at a key, [`probe`] tests the rule vector against
//! the rows of every seed whose rows reach the target (one IBP step) plus the recorded seeds: a pass there means "the
//! identity holds, its RECORDED seeds do not derive it" (`Triage::RecordInconsistent`), not "the rule is bad".
//! Limits (explicit, reported as `OracleSkip`): integer keys only (generic points are not re-decided); symmetry rows
//! are not modelled; descent under the node comparator is not decided by the identity test at all (checked elsewhere).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rustred_campaign_core::ibp_rows::{CellStatus, PointSpec, PointStatus, RowsRule, RowsVerdict};
use rustred_campaign_core::replay_base::algebra::{Ctx, Eliminator, Q};

use crate::referee::{self as r, RefElim};

/// The family as the oracle reads it.
pub struct OracleFamily {
    loops: usize,
    /// momentum coefficients per slot over (k_1..k_L, q)
    c: Vec<Vec<i64>>,
    kshift: Vec<i64>,
    /// per operator (j, v): per slot i: (t0, [(m, t_m)])
    ops: Vec<(bool, Vec<(Q, Vec<(usize, Q)>)>)>,
    zero: std::cell::RefCell<HashMap<Vec<bool>, bool>>,
}

fn args<'a>(compact: &'a str, head: &str) -> Vec<&'a str> {
    let inner = compact
        .strip_prefix("I(")
        .and_then(|x| x.strip_suffix(')'))
        .unwrap_or("");
    let (mut depth, mut start, mut out) = (0i32, 0usize, Vec::new());
    for (k, b) in inner.bytes().chain([b',']).enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                let call = &inner[start..k.min(inner.len())];
                if let Some(a) = call
                    .strip_prefix(head)
                    .and_then(|x| x.strip_prefix('('))
                    .and_then(|x| x.strip_suffix(')'))
                {
                    out.push(a);
                }
                start = k + 1;
            }
            _ => {}
        }
    }
    out
}

fn momentum(text: &str, names: &[String]) -> Result<Vec<i64>, String> {
    let mut v = vec![0i64; names.len()];
    let s = text.as_bytes();
    let mut i = 0;
    while i < s.len() {
        let sign = match s[i] {
            b'-' => {
                i += 1;
                -1
            }
            b'+' => {
                i += 1;
                1
            }
            _ => 1,
        };
        let st = i;
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
        }
        let coef: i64 = if i > st {
            text[st..i].parse().map_err(|_| "oracle: momentum")?
        } else {
            1
        };
        if s.get(i) == Some(&b'*') {
            i += 1;
        }
        let st = i;
        while i < s.len() && s[i].is_ascii_alphanumeric() {
            i += 1;
        }
        let k = names
            .iter()
            .position(|x| x.as_bytes() == &s[st..i])
            .ok_or_else(|| format!("oracle: momentum {text}"))?;
        v[k] += sign * coef;
    }
    Ok(v)
}

impl OracleFamily {
    pub fn read(text: &str) -> Result<OracleFamily, String> {
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let mut names: Vec<String> = args(&compact, "loops")
            .first()
            .ok_or("oracle: no loops")?
            .split(',')
            .map(str::to_string)
            .collect();
        let loops = names.len();
        names.push("q".into());
        let mut c = Vec::new();
        for a in args(&compact, "prop") {
            let mom = a
                .split_once(",(")
                .and_then(|(_, m)| m.split_once(")^2"))
                .ok_or("oracle: prop")?
                .0;
            c.push(momentum(mom, &names)?);
        }
        let n = c.len();
        let mut kshift = vec![0i64; n];
        for a in args(&compact, "power_shift") {
            let (lab, val) = a.split_once(',').ok_or("oracle: power_shift")?;
            let i: usize = lab
                .trim_start_matches('D')
                .parse::<usize>()
                .map_err(|_| "oracle: label")?
                - 1;
            // value = k (4 - d)/2 = 2k - (k/2) d: read its affine form with the referee's evaluator (d = variable 0)
            let lin = r::Referee.affine_in_d(val)?;
            kshift[i] = i64::try_from(lin.0.n / 2).map_err(|_| "oracle: shift")?;
        }
        // scalar products sp(a, b), a <= b, (q, q) = 1 excluded
        let pairs: Vec<(usize, usize)> = (0..=loops)
            .flat_map(|a| (a..=loops).map(move |b| (a, b)))
            .filter(|&p| p != (loops, loops))
            .collect();
        if pairs.len() != n {
            return Err("oracle: scalar products do not match the propagators".into());
        }
        let mut m = vec![vec![Q::ZERO; n]; n];
        let mut c0 = vec![Q::ZERO; n];
        for i in 0..n {
            c0[i] = Q::int(i128::from(c[i][loops] * c[i][loops]));
            for (s, &(a, b)) in pairs.iter().enumerate() {
                m[i][s] = Q::int(i128::from(c[i][a] * c[i][b] * if a == b { 1 } else { 2 }));
            }
        }
        let mi =
            <r::Referee as rustred_campaign_core::replay_base::algebra::CheckAlgebra>::q_inverse(
                &r::Referee,
                &m,
            )?;
        let mut ops = Vec::new();
        for j in 0..loops {
            for v in 0..=loops {
                let mut tab = Vec::new();
                for ci in &c {
                    let (mut t0, mut tm) = (Q::ZERO, vec![Q::ZERO; n]);
                    for a in 0..=loops {
                        let f = Q::int(i128::from(2 * ci[j] * ci[a]));
                        if f.is_zero() {
                            continue;
                        }
                        if (a.min(v), a.max(v)) == (loops, loops) {
                            t0 = r::qadd(t0, f)?;
                            continue;
                        }
                        let s = pairs
                            .iter()
                            .position(|&p| p == (a.min(v), a.max(v)))
                            .ok_or("oracle: pair")?;
                        for mm in 0..n {
                            let x = r::qmul(f, mi[s][mm])?;
                            tm[mm] = r::qadd(tm[mm], x)?;
                            t0 = r::qadd(t0, r::qneg(r::qmul(x, c0[mm])?)?)?;
                        }
                    }
                    tab.push((
                        t0,
                        tm.iter()
                            .enumerate()
                            .filter(|(_, x)| !x.is_zero())
                            .map(|(k, x)| (k, *x))
                            .collect(),
                    ));
                }
                ops.push((j == v, tab));
            }
        }
        Ok(OracleFamily {
            loops,
            c,
            kshift,
            ops,
            zero: Default::default(),
        })
    }

    /// The IBP rows at integer exponents `a` (+ the eps shifts), dimension d, mod p: key -> coefficient per row.
    fn rows(&self, a: &[i64], d: u64, p: u64) -> Vec<BTreeMap<Vec<i64>, u64>> {
        let eps = r::fmul(p, r::fsub(p, 4 % p, d), r::finv(p, 2).unwrap_or(0));
        let al: Vec<u64> = a
            .iter()
            .zip(&self.kshift)
            .map(|(&x, &k)| {
                r::fadd(
                    p,
                    r::fred(p, i128::from(x)),
                    r::fmul(p, r::fred(p, i128::from(k)), eps),
                )
            })
            .collect();
        let modq = |q: Q| r::fmul(p, r::fred(p, q.n), r::finv(p, r::fred(p, q.d)).unwrap_or(0));
        self.ops
            .iter()
            .map(|(diag, tab)| {
                let mut row: BTreeMap<Vec<i64>, u64> = BTreeMap::new();
                let mut put = |k: Vec<i64>, v: u64| {
                    let e = row.entry(k).or_insert(0);
                    *e = r::fadd(p, *e, v);
                };
                if *diag {
                    put(a.to_vec(), d % p);
                }
                for (i, (t0, tm)) in tab.iter().enumerate() {
                    let mut up = a.to_vec();
                    up[i] += 1;
                    put(up.clone(), r::fsub(p, 0, r::fmul(p, al[i], modq(*t0))));
                    for &(m, x) in tm {
                        let mut k = up.clone();
                        k[m] -= 1;
                        put(k, r::fsub(p, 0, r::fmul(p, al[i], modq(x))));
                    }
                }
                row.retain(|_, v| *v != 0);
                row
            })
            .collect()
    }

    /// Scaleless sector by Lee's criterion on the U / F supports from Cauchy-Binet minors (eps axes always active);
    /// `None` above 24 active axes (the subset enumeration is the oracle's limit).
    fn zero(&self, key: &[i64]) -> Option<bool> {
        let act: Vec<bool> = key
            .iter()
            .zip(&self.kshift)
            .map(|(&x, &k)| x > 0 || k != 0)
            .collect();
        if let Some(z) = self.zero.borrow().get(&act) {
            return Some(*z);
        }
        let axes: Vec<usize> = (0..act.len()).filter(|&i| act[i]).collect();
        let t = axes.len();
        if t > 24 {
            return None;
        }
        // the supports: every subset S of `size` active axes with a nonzero minor of the momentum coefficients on S
        // (loop parts for U, with the external column for the bordered F), as 0/1 rows with a trailing 1; Lee: the
        // sector is scaleless iff the ones vector e_t is NOT in their row space. The rank is taken modulo a 62-bit
        // prime: exact here (every minor of a 0/1 matrix with <= 25 columns is below 2^62 by Hadamard's bound).
        const P: u64 = 4_611_686_018_427_387_847; // 2^62 - 57
        let mut basis: BTreeMap<usize, Vec<u64>> = BTreeMap::new();
        let add = |v: Vec<u64>, basis: &mut BTreeMap<usize, Vec<u64>>| {
            let mut v = v;
            for (&c, row) in basis.iter() {
                if v[c] != 0 {
                    let f = v[c];
                    for (x, y) in v.iter_mut().zip(row) {
                        *x = r::fsub(P, *x, r::fmul(P, f, *y));
                    }
                }
            }
            if let Some(c) = v.iter().position(|&x| x != 0) {
                let inv = r::finv(P, v[c]).unwrap_or(0);
                basis.insert(c, v.iter().map(|&x| r::fmul(P, x, inv)).collect());
                // keep the basis reduced on its pivot columns
                let row = basis[&c].clone();
                for (&c2, r2) in basis.iter_mut() {
                    if c2 != c && r2[c] != 0 {
                        let f = r2[c];
                        for (x, y) in r2.iter_mut().zip(&row) {
                            *x = r::fsub(P, *x, r::fmul(P, f, *y));
                        }
                    }
                }
            }
        };
        let mut found = false;
        for size in [self.loops, self.loops + 1] {
            let mut pick: Vec<usize> = Vec::new();
            self.subsets(&axes, size, 0, &mut pick, &mut |sel: &[usize]| {
                let rows: Vec<Vec<i128>> = sel
                    .iter()
                    .map(|&b| {
                        self.c[axes[b]][..size]
                            .iter()
                            .map(|&x| i128::from(x))
                            .collect()
                    })
                    .collect();
                if r::det_bareiss(&rows).is_some_and(|dd| dd != 0) {
                    let mut v = vec![0u64; t + 1];
                    for &b in sel {
                        v[b] = 1;
                    }
                    v[t] = 1;
                    add(v, &mut basis);
                    found = true;
                }
            });
        }
        // e_t in the row space iff reducing it by the (reduced) basis leaves nothing
        let mut e = vec![0u64; t + 1];
        e[t] = 1;
        for (&c, row) in &basis {
            if e[c] != 0 {
                let f = e[c];
                for (x, y) in e.iter_mut().zip(row) {
                    *x = r::fsub(P, *x, r::fmul(P, f, *y));
                }
            }
        }
        let z = t == 0 || !found || e.iter().any(|&x| x != 0);
        self.zero.borrow_mut().insert(act, z);
        Some(z)
    }
}

impl OracleFamily {
    /// every `size`-subset of positions `0..axes.len()` (in increasing order), handed to `f`
    fn subsets(
        &self,
        axes: &[usize],
        size: usize,
        from: usize,
        pick: &mut Vec<usize>,
        f: &mut dyn FnMut(&[usize]),
    ) {
        if pick.len() == size {
            f(pick);
            return;
        }
        for b in from..axes.len() {
            if axes.len() - b < size - pick.len() {
                break;
            }
            pick.push(b);
            self.subsets(axes, size, b + 1, pick, f);
            pick.pop();
        }
    }
}

impl r::Referee {
    /// (constant, coefficient of d) of an affine expression in d (power shifts)
    fn affine_in_d(&self, text: &str) -> Result<(Q, Q), String> {
        let lin =
            <r::Referee as rustred_campaign_core::replay_base::algebra::CheckAlgebra>::expr_linear(
                self, text, 0,
            )?;
        Ok((lin[0], lin[1]))
    }
}

/// One difference between the shared logic and the oracle.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SharedDiff {
    pub key: Vec<i64>,
    pub what: String,
    pub shared: String,
    pub oracle: String,
}

/// What the oracle did not re-decide (explicit limits).
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct OracleRun {
    pub keys_checked: usize,
    pub cells_checked: usize,
    pub skipped: Vec<String>,
    pub diffs: Vec<SharedDiff>,
}

/// The oracle's decision of one key: rule point or not applicable.
fn applicable(rule: &RowsRule, key: &[i64]) -> Result<Option<Vec<i64>>, String> {
    let n = rule.arity;
    if key.len() != n {
        return Ok(None);
    }
    let mut pt: Vec<i64> = rule.fixed.iter().map(|f| f.unwrap_or(0)).collect();
    for (j, &(ax, off)) in rule.target.iter().enumerate() {
        match ax {
            Some(a) => pt[a] = key[j] - off,
            None if key[j] != off => return Ok(None),
            None => {}
        }
    }
    if rule
        .fixed
        .iter()
        .enumerate()
        .any(|(j, f)| f.is_some_and(|v| key[j] != v))
    {
        return Ok(None);
    }
    let zero =
        |s: &str| -> Result<bool, String> { Ok(r::poly_at(&r::literal(s, n)?, &pt)?.is_zero()) };
    for e in &rule.equations {
        if !zero(e)? {
            return Ok(None);
        }
    }
    for b in &rule.exceptions {
        let mut all = !b.is_empty();
        for l in b {
            all &= zero(l)?;
        }
        if all {
            return Ok(None);
        }
    }
    if rule
        .chart
        .iter()
        .enumerate()
        .any(|(j, &line)| (line && key[j] < 1) || (!line && key[j] > 0))
    {
        return Ok(None);
    }
    Ok(Some(pt))
}

fn at(pat: &[(Option<usize>, i64)], pt: &[i64]) -> Vec<i64> {
    pat.iter()
        .map(|&(ax, off)| ax.map_or(off, |a| pt[a] + off))
        .collect()
}

/// One cell decided by the oracle: (in span, +1 mutant rejected, a corner in span), or `None` on a pole.
struct Cell {
    in_span: bool,
    rejected: Option<bool>,
    corner: bool,
    target_zero: bool,
}

fn cell(
    fam: &OracleFamily,
    rule: &RowsRule,
    pt: &[i64],
    seeds: &[Vec<i64>],
    p: u64,
    d: u64,
) -> Result<Option<Cell>, String> {
    let mut vals: Vec<u64> = pt.iter().map(|&x| r::fred(p, i128::from(x))).collect();
    vals.push(d);
    let ctx = [Ctx { p, vals }];
    let mut coef = Vec::new();
    for (_, text) in &rule.rhs {
        let (mut nn, mut dd) = ([0u64], [0u64]);
        r::eval_text(text, rule.arity, &ctx, &mut nn, &mut dd)?;
        let Some(iv) = r::finv(p, dd[0]) else {
            return Ok(None);
        };
        coef.push(r::fmul(p, nn[0], iv));
    }
    let mut rows: Vec<BTreeMap<Vec<i64>, u64>> = Vec::new();
    for y in seeds.iter().collect::<BTreeSet<_>>() {
        rows.extend(fam.rows(y, d, p));
    }
    let target = at(&rule.target, pt);
    let rhs: Vec<Vec<i64>> = rule.rhs.iter().map(|(k, _)| at(k, pt)).collect();
    let mut cols: BTreeSet<Vec<i64>> = rows.iter().flat_map(|r| r.keys().cloned()).collect();
    cols.insert(target.clone());
    cols.extend(rhs.iter().cloned());
    let mut live: Vec<Vec<i64>> = Vec::new();
    for k in cols {
        if !fam.zero(&k).ok_or("oracle: more than 24 active slots")? {
            live.push(k);
        }
    }
    // pivot order only affects fill-in (membership does not depend on it): more active slots, more excess first
    let weight = |k: &Vec<i64>| -> (usize, i64) {
        let act = k.iter().filter(|&&v| v > 0).count();
        (act, k.iter().map(|&v| if v > 0 { v - 1 } else { -v }).sum())
    };
    live.sort_by(|a, b| weight(b).cmp(&weight(a)).then_with(|| a.cmp(b)));
    let id: HashMap<&Vec<i64>, usize> = live.iter().enumerate().map(|(i, k)| (k, i)).collect();
    if !id.contains_key(&target) {
        return Ok(Some(Cell {
            in_span: false,
            rejected: None,
            corner: false,
            target_zero: true,
        }));
    }
    let enc = |m: &BTreeMap<Vec<i64>, u64>| -> Vec<(usize, u64)> {
        let mut v: Vec<(usize, u64)> = m
            .iter()
            .filter(|(_, x)| **x != 0)
            .filter_map(|(k, x)| id.get(k).map(|&i| (i, *x)))
            .collect();
        v.sort_unstable();
        v
    };
    let el = RefElim::new(&rows.iter().map(enc).collect::<Vec<_>>(), p, usize::MAX)?;
    let mut desired: BTreeMap<Vec<i64>, u64> = [(target, 1)].into();
    for (k, &v) in rhs.iter().zip(&coef) {
        let e = desired.entry(k.clone()).or_insert(0);
        *e = r::fsub(p, *e, v);
    }
    let in_span = el.solve(&enc(&desired))?.in_span;
    let mut out = Cell {
        in_span,
        rejected: None,
        corner: false,
        target_zero: false,
    };
    if !in_span {
        return Ok(Some(out));
    }
    if let Some(mi) = rhs
        .iter()
        .zip(&coef)
        .position(|(k, &v)| v != 0 && id.contains_key(k))
    {
        let mut mutant = desired.clone();
        let e = mutant.entry(rhs[mi].clone()).or_insert(0);
        *e = r::fsub(p, *e, 1);
        out.rejected = Some(!el.solve(&enc(&mutant))?.in_span);
    }
    // per sector touched: its lowest-weight numerator-free column (ties: the smallest key)
    let mut probe: BTreeMap<Vec<bool>, (i64, &Vec<i64>)> = BTreeMap::new();
    for k in live.iter().filter(|k| k.iter().all(|&v| v >= 0)) {
        let w: i64 = k.iter().sum();
        let e = probe
            .entry(k.iter().map(|&v| v > 0).collect())
            .or_insert((w, k));
        if (w, k) < (e.0, e.1) {
            *e = (w, k);
        }
    }
    for (_, k) in probe.values() {
        out.corner |= el.solve(&[(id[k], 1)])?.in_span;
    }
    Ok(Some(out))
}

fn seeds_at(rule: &RowsRule, pt: &[i64]) -> Vec<Vec<i64>> {
    rule.seeds
        .iter()
        .map(|s| {
            s.iter()
                .enumerate()
                .map(|(i, &(ax, off))| ax.map_or(off, |a| pt[a] + off + rule.recenter[i]))
                .collect()
        })
        .collect()
}

/// Re-decide every integer key point of `v` (a run of the shared logic) and list the differences.
pub fn check(family_text: &str, rule: &RowsRule, v: &RowsVerdict) -> OracleRun {
    let mut run = OracleRun::default();
    let fam = match OracleFamily::read(family_text) {
        Ok(f) => f,
        Err(e) => {
            run.skipped
                .push(format!("family not read by the oracle: {e}"));
            return run;
        }
    };
    let diff = |run: &mut OracleRun, key: &[i64], what: &str, s: String, o: String| {
        run.diffs.push(SharedDiff {
            key: key.to_vec(),
            what: what.into(),
            shared: s,
            oracle: o,
        });
    };
    for pr in &v.points {
        let PointSpec::Key(key) = &pr.spec else {
            continue;
        };
        if pr.cells.iter().any(|c| c.sym_rows > 0) {
            run.skipped.push(format!(
                "{key:?}: symmetry rows (not modelled by the oracle)"
            ));
            continue;
        }
        let shared_pt = pr
            .rule_point
            .clone()
            .filter(|_| !matches!(pr.status, PointStatus::NotApplicable(_)));
        let ours = match applicable(rule, key) {
            Ok(x) => x,
            Err(e) => {
                run.skipped
                    .push(format!("{key:?}: literal not read by the oracle: {e}"));
                continue;
            }
        };
        run.keys_checked += 1;
        if shared_pt != ours {
            diff(
                &mut run,
                key,
                "applicability / rule point",
                format!("{shared_pt:?}"),
                format!("{ours:?}"),
            );
            continue;
        }
        let Some(pt) = ours else { continue };
        let seeds = seeds_at(rule, &pt);
        let mut oracle_cells = Vec::new();
        for c in &pr.cells {
            let decided = matches!(
                c.status,
                CellStatus::Pass | CellStatus::Fail | CellStatus::Refused(_)
            ) || matches!(&c.status, CellStatus::Undecided { reason } if reason.starts_with("control failed"));
            if !decided {
                continue;
            }
            let o = match cell(&fam, rule, &pt, &seeds, c.prime, c.d) {
                Ok(Some(o)) => o,
                Ok(None) => {
                    diff(
                        &mut run,
                        key,
                        "coefficient pole at the cell's d",
                        format!("{:?}", c.status),
                        "pole".into(),
                    );
                    continue;
                }
                Err(e) => {
                    run.skipped
                        .push(format!("{key:?}: cell not decided by the oracle: {e}"));
                    continue;
                }
            };
            run.cells_checked += 1;
            let shared_zero = matches!(c.status, CellStatus::Refused(_));
            if shared_zero != o.target_zero {
                diff(
                    &mut run,
                    key,
                    "target in a zero sector",
                    format!("{shared_zero}"),
                    format!("{}", o.target_zero),
                );
                continue;
            }
            if shared_zero {
                continue;
            }
            if c.in_span != o.in_span {
                diff(
                    &mut run,
                    key,
                    &format!("rule vector in the span (prime {})", c.prime),
                    format!("{}", c.in_span),
                    format!("{}", o.in_span),
                );
            } else if o.in_span
                && (c.mutation_rejected != o.rejected || c.corners_in_span.is_empty() == o.corner)
            {
                diff(
                    &mut run,
                    key,
                    &format!("controls (prime {})", c.prime),
                    format!(
                        "mutant rejected {:?}, corner in span {}",
                        c.mutation_rejected,
                        !c.corners_in_span.is_empty()
                    ),
                    format!(
                        "mutant rejected {:?}, corner in span {}",
                        o.rejected, o.corner
                    ),
                );
            }
            let st = if !o.in_span {
                "fail"
            } else if o.rejected == Some(true) && !o.corner {
                "pass"
            } else {
                "control"
            };
            let sh = match &c.status {
                CellStatus::Pass => "pass",
                CellStatus::Fail => "fail",
                _ => "control",
            };
            if st != sh {
                diff(
                    &mut run,
                    key,
                    &format!("cell status (prime {})", c.prime),
                    sh.into(),
                    st.into(),
                );
            }
            oracle_cells.push(st);
        }
        if oracle_cells.len() == pr.cells.len() && !oracle_cells.is_empty() {
            let os = if oracle_cells.iter().all(|s| *s == "pass") {
                "Pass"
            } else if oracle_cells.iter().all(|s| *s == "fail") {
                "Fail"
            } else {
                "Undecided"
            };
            let ss = match pr.status {
                PointStatus::Pass => "Pass",
                PointStatus::Fail => "Fail",
                _ => "Undecided",
            };
            if os != ss && pr.status != PointStatus::SeedsInsufficient {
                diff(
                    &mut run,
                    key,
                    "point status (combination of the cells)",
                    ss.into(),
                    os.into(),
                );
            }
        }
    }
    run
}

/// For a rule both backends agree fails: does the rule vector lie in the span of the rows of the recorded seeds PLUS
/// every seed whose rows reach the target in one IBP step, at the first failing key, at every decided cell? `Some(key)`
/// = yes (the identity holds there; its recorded seeds do not derive it).
pub fn probe(family_text: &str, rule: &RowsRule, v: &RowsVerdict) -> Option<Vec<i64>> {
    let fam = OracleFamily::read(family_text).ok()?;
    let pr = v
        .points
        .iter()
        .find(|p| matches!(p.spec, PointSpec::Key(_)) && p.status == PointStatus::Fail)?;
    let PointSpec::Key(key) = &pr.spec else {
        return None;
    };
    let pt = pr.rule_point.clone()?;
    let target = at(&rule.target, &pt);
    let n = rule.arity;
    let mut seeds = seeds_at(rule, &pt);
    for i in 0..n {
        for m in 0..=n {
            let mut s = target.clone();
            s[i] -= 1;
            if m < n {
                s[m] += 1;
            }
            seeds.push(s);
        }
    }
    seeds.push(target);
    let all =
        pr.cells.iter().filter(|c| c.d != 0).all(
            |c| matches!(cell(&fam, rule, &pt, &seeds, c.prime, c.d), Ok(Some(o)) if o.in_span),
        );
    all.then(|| key.clone())
}
