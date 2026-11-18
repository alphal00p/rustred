//! (a) ROW RECOVERY for `v0+rows` certificates (lane gencheck; plan/auto-campaign/interfaces/emission-check.md section 10,
//! implementer review 2026-11-18 (b)).
//!
//! A v0 certificate's trace rows A_j are rows of the producer's preconditioned basis instantiated at seeds. The engine's
//! preconditioner builds every basis row as a POLYNOMIAL combination, at the same seed, of the ordinary IBP source rows
//! (rustred-core solver/precondition/provenance.rs, `Node::Subtract` with polynomial scales). So each carried row should
//! lie in the span, over Q(n, d), of THIS tool's own ordinary rows O_m instantiated at the row's seed. The recovery solves
//!     A_j = sum_m c_jm O_m(seed_j)
//! EXACTLY (Symbolica `Matrix::solve_any` over the field of rational functions Q(n, d); no evaluation point, no sampling)
//! and then VERIFIES the solution term by term with exact arithmetic, independently of the solver. A row that cannot be
//! recovered leaves the stage "not verified": the class cannot be whole domain.
//! Result: the triples (seed_j, m, w_j c_jm), i.e. the certificate rewritten on ORDINARY rows (the v1 `composed` form
//! derived by the checker itself), and the denominators of every c_jm: they join the factor list (possible loci).
//! Solutions are cached per (basis row, numeric part of the seed) and re-used at another seed by the symbolic shift of
//! the seed offsets; a re-used solution is verified exactly like a fresh one (and solved fresh when it fails).
use std::collections::HashMap;
use std::time::Instant;

use symbolica::prelude::{Matrix, RationalPolynomialField, Z};

use super::alg::{self, Poly, Rf, Vars};
use super::{seed_aff, seed_term};
use crate::core::{Pat, pat_str};
use crate::stream::Rows;

/// Outcome of the recovery over all weighted sources.
pub struct Recovery {
    /// (seed, ordinary row m, w_j * c_jm) for every weighted source j and every nonzero c_jm.
    pub triples: Vec<(Pat, i64, Rf)>,
    /// Denominators (non-constant) of every recovered c_jm.
    pub dens: Vec<Poly>,
    /// Weighted sources considered / recovered / recovered by a shifted cached solution.
    pub sources: usize,
    pub recovered: usize,
    pub by_cache: usize,
    /// "source j (basis row k at seed): reason" for every row not recovered (first 10 kept).
    pub failed: Vec<String>,
    pub failed_count: usize,
    pub max_equations: usize,
    pub max_unknowns: usize,
    pub seconds: f64,
    /// rows recovered only MODULO the stated zero-sector record (integrals in zero sectors dropped on both sides)
    pub modulo_zero_rows: usize,
    /// zero-sector terms dropped in those rows (carried row + ordinary rows)
    pub zero_terms_dropped: usize,
}

impl Recovery {
    pub fn complete(&self) -> bool {
        self.failed_count == 0 && self.recovered == self.sources
    }
    pub fn json(&self) -> String {
        format!(
            "{{\"weighted_sources\":{},\"recovered\":{},\"by_cache\":{},\"failed\":{},\"failed_first\":[{}],\"max_equations\":{},\"max_unknowns\":{},\"denominators\":{},\"modulo_zero_rows\":{},\"zero_terms_dropped\":{},\"seconds\":{:.3},\"method\":\"exact: A_j = sum_m c_jm O_m(seed_j) solved by Symbolica Matrix::solve_any over Q(n,d) with this tool's ordinary rows, then verified term by term (Symbolica is_zero)\"}}",
            self.sources,
            self.recovered,
            self.by_cache,
            self.failed_count,
            self.failed
                .iter()
                .map(|s| crate::core::jstr(s))
                .collect::<Vec<_>>()
                .join(","),
            self.max_equations,
            self.max_unknowns,
            self.dens.len(),
            self.modulo_zero_rows,
            self.zero_terms_dropped,
            self.seconds
        )
    }
}

/// The ordinary rows of the generator instantiated at `seed`: (m, [(integral, coefficient)]), rows that vanish
/// identically at the seed dropped; m in ascending order (deterministic).
fn instantiate(vars: &Vars, seed: &Pat, er: &Rows) -> Result<Vec<(i64, Vec<(Pat, Rf)>)>, String> {
    let arity = seed.len();
    let mut ms: Vec<i64> = er.keys().copied().collect();
    ms.sort_unstable();
    let mut out = vec![];
    for m in ms {
        let mut acc: HashMap<Pat, Rf> = HashMap::new();
        for (off, a) in &er[&m] {
            if off.len() < arity {
                return Err("ordinary row arity below the rule's".into());
            }
            let c = Rf::from(seed_aff(vars, seed, a)?);
            if c.is_zero() {
                continue;
            }
            let p = seed_term(seed, &off[..arity]);
            let e = acc.entry(p).or_insert_with(|| vars.zero_rf());
            *e = &*e + &c;
        }
        let mut terms: Vec<(Pat, Rf)> = acc.into_iter().filter(|(_, c)| !c.is_zero()).collect();
        terms.sort_by(|a, b| a.0.cmp(&b.0));
        if !terms.is_empty() {
            out.push((m, terms));
        }
    }
    Ok(out)
}

/// Exact check: target == sum_k c_k inst_k (every integral). Returns the first differing integral.
fn verify(
    vars: &Vars,
    inst: &[(i64, Vec<(Pat, Rf)>)],
    c: &[(i64, Rf)],
    target: &[(Pat, Rf)],
) -> Option<String> {
    let mut acc: HashMap<&Pat, Rf> = HashMap::new();
    for (p, t) in target {
        let e = acc.entry(p).or_insert_with(|| vars.zero_rf());
        *e = &*e + t;
    }
    let by_m: HashMap<i64, &Vec<(Pat, Rf)>> = inst.iter().map(|(m, t)| (*m, t)).collect();
    for (m, cm) in c {
        let Some(terms) = by_m.get(m) else {
            return Some(format!("solution names row {m} which vanishes at the seed"));
        };
        for (p, a) in terms.iter() {
            let e = acc.entry(p).or_insert_with(|| vars.zero_rf());
            *e = &*e - &(cm * a);
        }
    }
    let mut bad: Vec<&Pat> = acc
        .iter()
        .filter(|(_, v)| !v.is_zero())
        .map(|(p, _)| *p)
        .collect();
    bad.sort();
    bad.first().map(|p| {
        format!(
            "residual at {}: {}",
            pat_str(p),
            alg::short(acc[*p].to_string(), 160)
        )
    })
}

/// Solve target = sum_k c_k inst_k exactly; Err(reason) when the target is not in the span.
fn solve(
    vars: &Vars,
    inst: &[(i64, Vec<(Pat, Rf)>)],
    target: &[(Pat, Rf)],
    max_eq: &mut usize,
    max_unk: &mut usize,
) -> Result<Vec<(i64, Rf)>, String> {
    let mut eq: HashMap<Pat, usize> = HashMap::new();
    let mut order: Vec<Pat> = vec![];
    for (_, terms) in inst {
        for (p, _) in terms {
            if !eq.contains_key(p) {
                eq.insert(p.clone(), order.len());
                order.push(p.clone());
            }
        }
    }
    for (p, _) in target {
        if !eq.contains_key(p) {
            return Err(format!(
                "integral {} of the carried row is in no ordinary row at the seed",
                pat_str(p)
            ));
        }
    }
    let (ne, nu) = (order.len(), inst.len());
    *max_eq = (*max_eq).max(ne);
    *max_unk = (*max_unk).max(nu);
    let mut a: Vec<Vec<Rf>> = vec![vec![vars.zero_rf(); nu]; ne];
    for (k, (_, terms)) in inst.iter().enumerate() {
        for (p, c) in terms {
            a[eq[p]][k] = c.clone();
        }
    }
    let mut b: Vec<Vec<Rf>> = vec![vec![vars.zero_rf()]; ne];
    for (p, c) in target {
        let i = eq[p];
        b[i][0] = &b[i][0] + c;
    }
    let field = RationalPolynomialField::<_, u16>::new(Z);
    let am = Matrix::from_nested_vec(a, field.clone()).map_err(|e| format!("matrix: {e}"))?;
    let bm = Matrix::from_nested_vec(b, field).map_err(|e| format!("rhs: {e}"))?;
    let x = am
        .solve_any(&bm)
        .map_err(|e| format!("not in the span of the ordinary rows at the seed ({e})"))?;
    let mut out = vec![];
    for (k, (m, _)) in inst.iter().enumerate() {
        let v = x[(k as u32, 0)].clone();
        if !v.is_zero() {
            out.push((*m, v));
        }
    }
    Ok(out)
}

/// Drop the terms whose integral is in a zero sector; returns the number dropped.
fn drop_zero(terms: &mut Vec<(Pat, Rf)>, zero: &dyn Fn(&Pat) -> bool) -> usize {
    let n = terms.len();
    terms.retain(|(p, _)| !zero(p));
    n - terms.len()
}

/// Recover every weighted carried row. `sources[j]` = (basis row, seed); `rows[j]` = the carried row of source j (terms
/// already parsed); `weights` = (j, w_j), nonzero. `zero`: when given (a STATED zero-sector record, raw frame), a row
/// that is not recovered exactly is tried again with every zero-sector integral dropped on both sides (the producer's
/// eliminated rows drop them); such rows are counted in `modulo_zero_rows`.
pub fn recover(
    vars: &Vars,
    sources: &[(i64, Pat)],
    rows: &[Vec<(Pat, Rf)>],
    weights: &[(usize, Rf)],
    er: &Rows,
    zero: Option<&dyn Fn(&Pat) -> bool>,
) -> Recovery {
    let t0 = Instant::now();
    let mut rec = Recovery {
        triples: vec![],
        dens: vec![],
        sources: weights.len(),
        recovered: 0,
        by_cache: 0,
        failed: vec![],
        failed_count: 0,
        max_equations: 0,
        max_unknowns: 0,
        seconds: 0.0,
        modulo_zero_rows: 0,
        zero_terms_dropped: 0,
    };
    // (basis row, numeric part of the seed) -> (seed, solution)
    type Key = (i64, Vec<Option<i64>>);
    let mut cache: HashMap<Key, (Pat, Vec<(i64, Rf)>)> = HashMap::new();
    let fail = |rec: &mut Recovery, s: String| {
        rec.failed_count += 1;
        if rec.failed.len() < 10 {
            rec.failed.push(s);
        }
    };
    for (j, w) in weights {
        let (Some((k, seed)), Some(target)) = (sources.get(*j), rows.get(*j)) else {
            fail(&mut rec, format!("source {j}: no seed or no carried row"));
            continue;
        };
        let label = format!("source {j} (basis row {k} at {})", pat_str(seed));
        let inst = match instantiate(vars, seed, er) {
            Ok(i) => i,
            Err(e) => {
                fail(&mut rec, format!("{label}: {e}"));
                continue;
            }
        };
        let key: Key = (
            *k,
            seed.iter()
                .map(|&(s, v)| if s { None } else { Some(v) })
                .collect(),
        );
        let mut sol: Option<Vec<(i64, Rf)>> = None;
        if let Some((s0, c0)) = cache.get(&key) {
            let shifts: Vec<(usize, i64)> = seed
                .iter()
                .zip(s0)
                .enumerate()
                .filter(|(_, (a, b))| a.0 && a.1 != b.1)
                .map(|(i, (a, b))| (Vars::slot(i), a.1 - b.1))
                .collect();
            let c: Vec<(i64, Rf)> = c0
                .iter()
                .map(|(m, r)| (*m, alg::shift_rf(r, &shifts)))
                .collect();
            if verify(vars, &inst, &c, target).is_none() {
                rec.by_cache += 1;
                sol = Some(c);
            }
        }
        if sol.is_none() {
            let mut why = String::new();
            match solve(
                vars,
                &inst,
                target,
                &mut rec.max_equations,
                &mut rec.max_unknowns,
            ) {
                Ok(c) => match verify(vars, &inst, &c, target) {
                    None => {
                        cache.insert(key.clone(), (seed.clone(), c.clone()));
                        sol = Some(c);
                    }
                    Some(w) => why = format!("solver result not verified: {w}"),
                },
                Err(e) => why = e,
            }
            if let (true, Some(z)) = (sol.is_none(), zero) {
                // modulo the stated zero sectors: drop their integrals on both sides and solve again
                let mut t2 = target.clone();
                let mut dropped = drop_zero(&mut t2, z);
                let mut inst2 = inst.clone();
                for (_, terms) in inst2.iter_mut() {
                    dropped += drop_zero(terms, z);
                }
                inst2.retain(|(_, t)| !t.is_empty());
                match solve(
                    vars,
                    &inst2,
                    &t2,
                    &mut rec.max_equations,
                    &mut rec.max_unknowns,
                ) {
                    Ok(c) if verify(vars, &inst2, &c, &t2).is_none() => {
                        rec.modulo_zero_rows += 1;
                        rec.zero_terms_dropped += dropped;
                        sol = Some(c);
                    }
                    Ok(_) => why = format!("{why}; modulo zero sectors: not verified"),
                    Err(e) => why = format!("{why}; modulo zero sectors: {e}"),
                }
            }
            if sol.is_none() {
                fail(&mut rec, format!("{label}: {why}"));
            }
        }
        let Some(c) = sol else { continue };
        rec.recovered += 1;
        for (m, cm) in c {
            if !cm.denominator.is_constant() {
                rec.dens.push(cm.denominator.clone());
            }
            rec.triples.push((seed.clone(), m, w * &cm));
        }
    }
    rec.seconds = t0.elapsed().as_secs_f64();
    rec
}
