//! Normal form of case domains for the partition: per rule EQ literals (must vanish), GE literals (must be >= 0) and
//! BAD branches (AND-lists; the rule is excluded where every member of one branch vanishes). Literals are interned
//! once with their slot support. Pole loci become BAD branches (the AND of the d-coefficients of each exact linear
//! factor and of the cofactor: Z[n,d] has no zero divisors, so the zero set is the union over the factors).

use std::collections::HashMap;

use super::exact::{ExactError, MPoly, linear_factors, slot_names};
use super::{DomainSource, ElidedPolicy, PartitionError};
use crate::domain::{CaseDomain, TargetMap};
use crate::key::NodeFrame;
use crate::poly::Poly;

/// One interned literal.
#[derive(Clone, Debug)]
pub struct Lit {
    /// The literal as a polynomial in the slots (nvars = arity).
    pub poly: MPoly,
    /// The same as a core polynomial (exact evaluation with big-integer fallback).
    pub core: Poly,
    /// Slots that occur, ascending.
    pub support: Vec<usize>,
    /// GE literal (sign matters; normalised by positive content only) or zero-set literal (primitive, canonical sign).
    pub is_ge: bool,
    pub text: String,
}

/// A rule in normal form.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NfRule {
    pub eq: Vec<u32>,
    pub ge: Vec<u32>,
    pub bad: Vec<Vec<u32>>,
}

/// Counters of the normal form.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct NfStats {
    pub domains: usize,
    pub distinct: usize,
    pub dropped_out_of_chart: usize,
    pub dropped_elided: usize,
    pub dropped_dead_constant: usize,
    pub excluded_unsupported_target: usize,
    pub pole_branches: usize,
    pub pole_linear_factors: usize,
    pub literals: usize,
    /// FailClosed sub-problem: number of rules making up the complement of the elided domain.
    pub blocked_variants: usize,
}

/// The rules of a partition run.
#[derive(Clone, Debug)]
pub struct NfRules {
    pub arity: usize,
    /// `lines[i]`: slot i is a line (floor 1, values >= 1), else a numerator (floor 0, values <= 0).
    pub lines: Vec<bool>,
    pub lits: Vec<Lit>,
    pub rules: Vec<NfRule>,
    /// Source indices merged into each rule.
    pub sources: Vec<Vec<usize>>,
    pub stats: NfStats,
    /// Per rule the fixed slots read from its one-slot unit EQ literals `n_s - v` (fast pre-check of the oracle).
    pub quick: Vec<Vec<(usize, i64)>>,
    /// FailClosed: source indices of the elided records (removed here; their blocked regions are separate).
    pub elided: Vec<usize>,
}

/// Options of the normal form.
#[derive(Clone, Debug)]
pub struct NfOptions {
    pub elided: ElidedPolicy,
    pub dedup: bool,
    pub subset: Option<Vec<usize>>,
    /// Split product members of exception branches by their exact linear factors (default false = the checker's form).
    pub factor_exceptions: bool,
    /// FailClosed sub-problem: this (elided) source index enters as the COMPLEMENT of its domain.
    pub complement_of: Option<usize>,
}

struct Builder {
    arity: usize,
    lits: Vec<Lit>,
    index: HashMap<(bool, MPoly), u32>,
}

enum Norm {
    Zero,
    NonZeroConst,
    Lit(u32),
}

impl Builder {
    fn intern(&mut self, p: &MPoly, is_ge: bool) -> Result<Norm, PartitionError> {
        if let Some(c) = p.as_constant() {
            return Ok(if is_ge {
                if c >= 0 {
                    Norm::Zero
                } else {
                    Norm::NonZeroConst
                }
            } else if c == 0 {
                Norm::Zero
            } else {
                Norm::NonZeroConst
            });
        }
        let q = if is_ge {
            let g = p.content();
            p.scale(1).map(|x| {
                let mut r = MPoly::zero(x.nvars);
                for (e, &c) in &x.terms {
                    r.terms.insert(e.clone(), c / g);
                }
                r
            })
        } else {
            p.primitive()
        }
        .map_err(PartitionError::Exact)?;
        let key = (is_ge, q.clone());
        if let Some(&i) = self.index.get(&key) {
            return Ok(Norm::Lit(i));
        }
        let id = self.lits.len() as u32;
        let names = slot_names(self.arity, false);
        self.lits.push(Lit {
            core: q.to_poly(self.arity),
            support: q.support(),
            text: q.render(&names),
            poly: q,
            is_ge,
        });
        self.index.insert(key, id);
        Ok(Norm::Lit(id))
    }

    /// AND-list of zero-set literals -> Some(branch) (possibly empty = always true), None = never true.
    fn branch(&mut self, members: &[MPoly]) -> Result<Option<Vec<u32>>, PartitionError> {
        let mut b = vec![];
        for m in members {
            match self.intern(m, false)? {
                Norm::Zero => {}
                Norm::NonZeroConst => return Ok(None),
                Norm::Lit(i) => b.push(i),
            }
        }
        b.sort_unstable();
        b.dedup();
        Ok(Some(b))
    }
}

fn mp(p: &Poly, arity: usize) -> Result<MPoly, PartitionError> {
    MPoly::from_poly(p, arity, false).map_err(PartitionError::Exact)
}

/// Branches of a pole locus given by its d-coefficients: union over the exact linear factors of the reconstructed
/// polynomial `sum_k c_k d^k` and its cofactor of the AND of their d-coefficients. Returns the AND-lists (d-free).
fn pole_branches(
    coeffs: &[Poly],
    arity: usize,
    nlin: &mut usize,
) -> Result<Vec<Vec<MPoly>>, PartitionError> {
    if coeffs.is_empty() {
        return Ok(vec![vec![]]); // identically zero: always forbidden
    }
    let nv = arity + 1;
    let mut p = MPoly::zero(nv);
    for (k, c) in coeffs.iter().enumerate() {
        let cm = MPoly::from_poly(c, arity, false).map_err(PartitionError::Exact)?;
        let mut lifted = MPoly::zero(nv);
        for (e, &x) in &cm.terms {
            let mut e2 = e.clone();
            e2.push(k as u32);
            lifted.terms.insert(e2, x);
        }
        p = p.add(&lifted).map_err(PartitionError::Exact)?;
    }
    let (fs, rest) = match linear_factors(&p) {
        Ok(x) => x,
        Err(ExactError::Overflow) | Err(ExactError::TooLarge(_)) => (vec![], p.clone()),
    };
    *nlin += fs.len();
    let mut out = vec![];
    for f in fs.iter().chain(std::iter::once(&rest)) {
        if f.as_constant().is_some_and(|c| c != 0) {
            continue;
        }
        // d-coefficients of f
        let cs = f.coeffs_in(arity);
        let mut and = vec![];
        let mut empty = false;
        for c in cs {
            if c.is_zero() {
                continue;
            }
            let mut d = MPoly::zero(arity);
            for (e, &x) in &c.terms {
                d.terms.insert(e[..arity].to_vec(), x);
            }
            if d.as_constant().is_some_and(|x| x != 0) {
                empty = true;
                break;
            }
            and.push(d);
        }
        if !empty {
            out.push(and);
        }
    }
    Ok(out)
}

fn split_products(members: &[MPoly]) -> Result<Vec<Vec<MPoly>>, PartitionError> {
    // AND(f g, h) = AND(f, h) OR AND(g, h)
    let mut acc: Vec<Vec<MPoly>> = vec![vec![]];
    for m in members {
        let (fs, rest) = linear_factors(m).unwrap_or((vec![], m.clone()));
        let mut parts: Vec<MPoly> = fs;
        if rest.as_constant().is_none() {
            parts.push(rest);
        }
        if parts.is_empty() {
            parts.push(m.clone());
        }
        let mut next = vec![];
        for a in &acc {
            for p in &parts {
                let mut b = a.clone();
                b.push(p.clone());
                next.push(b);
            }
        }
        acc = next;
        if acc.len() > 64 {
            return Ok(vec![members.to_vec()]);
        }
    }
    Ok(acc)
}

/// Build the normal form of the selected domains of `src`.
pub fn build(
    src: &dyn DomainSource,
    frame: &NodeFrame,
    opts: &NfOptions,
) -> Result<NfRules, PartitionError> {
    let arity = frame.arity();
    if src.arity() != arity {
        return Err(PartitionError::Arity {
            expected: arity,
            got: src.arity(),
        });
    }
    let lines: Vec<bool> = (0..arity).map(|i| frame.is_line(i)).collect();
    let mut b = Builder {
        arity,
        lits: vec![],
        index: HashMap::new(),
    };
    let mut stats = NfStats::default();
    let mut rules: Vec<NfRule> = vec![];
    let mut sources: Vec<Vec<usize>> = vec![];
    let mut seen: HashMap<NfRule, usize> = HashMap::new();
    let mut elided: Vec<usize> = vec![];
    let mut pole_cache: HashMap<Vec<Poly>, Vec<Vec<MPoly>>> = HashMap::new();
    let idx: Vec<usize> = match &opts.subset {
        Some(s) => s.clone(),
        None => (0..src.len()).collect(),
    };
    'dom: for i in idx {
        stats.domains += 1;
        let d: &CaseDomain = src.domain(i);
        if d.arity != arity {
            return Err(PartitionError::Arity {
                expected: arity,
                got: d.arity,
            });
        }
        if let Some(c) = &d.chart
            && c.as_slice() != lines.as_slice()
        {
            return Err(PartitionError::ChartNotFrame { source: i });
        }
        if d.target != TargetMap::Canonical {
            stats.excluded_unsupported_target += 1;
            continue;
        }
        let elided_here = d.witness_elided && opts.elided != ElidedPolicy::Ordinary;
        if elided_here {
            stats.dropped_elided += 1;
        }
        if elided_here && opts.elided == ElidedPolicy::Absent {
            continue;
        }
        let mut r = NfRule {
            eq: vec![],
            ge: vec![],
            bad: vec![],
        };
        for (s, f) in d.fixed.iter().enumerate() {
            if let Some(v) = f {
                if (lines[s] && *v < 1) || (!lines[s] && *v > 0) {
                    stats.dropped_out_of_chart += 1;
                    continue 'dom;
                }
                let p = MPoly::var(arity, s)
                    .sub(&MPoly::constant(arity, *v as i128))
                    .map_err(PartitionError::Exact)?;
                if let Norm::Lit(l) = b.intern(&p, false)? {
                    r.eq.push(l);
                }
            }
        }
        for e in &d.equations {
            match b.intern(&mp(e, arity)?, false)? {
                Norm::Zero => {}
                Norm::NonZeroConst => {
                    stats.dropped_dead_constant += 1;
                    continue 'dom;
                }
                Norm::Lit(l) => r.eq.push(l),
            }
        }
        for g in d.bounds.iter().chain(&d.recenter_bounds) {
            match b.intern(&mp(g, arity)?, true)? {
                Norm::Zero => {}
                Norm::NonZeroConst => {
                    stats.dropped_dead_constant += 1;
                    continue 'dom;
                }
                Norm::Lit(l) => r.ge.push(l),
            }
        }
        let mut branches: Vec<Vec<MPoly>> = vec![];
        for br in &d.exceptions {
            let ms: Vec<MPoly> = br.iter().map(|p| mp(p, arity)).collect::<Result<_, _>>()?;
            if opts.factor_exceptions {
                branches.extend(split_products(&ms)?);
            } else {
                branches.push(ms);
            }
        }
        for pl in &d.poles {
            let pb = match pole_cache.get(&pl.coeffs) {
                Some(x) => x.clone(),
                None => {
                    let x = pole_branches(&pl.coeffs, arity, &mut stats.pole_linear_factors)?;
                    pole_cache.insert(pl.coeffs.clone(), x.clone());
                    x
                }
            };
            stats.pole_branches += pb.len();
            branches.extend(pb);
        }
        for br in branches {
            match b.branch(&br)? {
                None => {}
                Some(v) if v.is_empty() => {
                    stats.dropped_dead_constant += 1;
                    continue 'dom;
                }
                Some(v) => r.bad.push(v),
            }
        }
        r.eq.sort_unstable();
        r.eq.dedup();
        r.ge.sort_unstable();
        r.ge.dedup();
        r.bad.sort();
        r.bad.dedup();
        let variants = if elided_here {
            if opts.complement_of != Some(i) {
                // FailClosed: the record credits nothing; its BLOCKED region is computed by a separate partition
                // (see `partition::gap_partition`)
                elided.push(i);
                continue;
            }
            let universal = NfRule {
                eq: vec![],
                ge: vec![],
                bad: vec![],
            };
            let v = subtract(&mut b, &universal, &r)?;
            stats.blocked_variants += v.len();
            v
        } else {
            vec![r]
        };
        for r in variants {
            if opts.dedup {
                if let Some(&k) = seen.get(&r) {
                    sources[k].push(i);
                    continue;
                }
                seen.insert(r.clone(), rules.len());
            }
            rules.push(r);
            sources.push(vec![i]);
        }
    }
    stats.distinct = rules.len();
    stats.literals = b.lits.len();
    let quick = rules
        .iter()
        .map(|r| {
            r.eq.iter()
                .filter_map(|&l| unit_fixed(&b.lits[l as usize]))
                .collect()
        })
        .collect();
    Ok(NfRules {
        arity,
        lines,
        lits: b.lits,
        rules,
        sources,
        stats,
        quick,
        elided,
    })
}

/// `v \ h` as a union of normal-form rules: v with `l != 0` for each equation l of h (a BAD branch [l]), with
/// `g <= -1` for each bound g of h, and with every member of each exception branch of h as equations. Disjoint
/// fixed slots: v unchanged.
fn subtract(b: &mut Builder, v: &NfRule, h: &NfRule) -> Result<Vec<NfRule>, PartitionError> {
    let fv: Vec<(usize, i64)> =
        v.eq.iter()
            .filter_map(|&l| unit_fixed(&b.lits[l as usize]))
            .collect();
    for &l in &h.eq {
        if let Some((s, x)) = unit_fixed(&b.lits[l as usize])
            && fv.iter().any(|&(t, y)| t == s && y != x)
        {
            return Ok(vec![v.clone()]);
        }
    }
    let mut out = vec![];
    for &l in &h.eq {
        if v.eq.contains(&l) {
            continue; // l = 0 on all of v: this variant is empty
        }
        let mut w = v.clone();
        w.bad.push(vec![l]);
        w.bad.sort();
        w.bad.dedup();
        out.push(w);
    }
    for &g in &h.ge {
        let neg = b.lits[g as usize]
            .poly
            .scale(-1)
            .and_then(|p| p.sub(&MPoly::constant(p.nvars, 1)));
        let neg = neg.map_err(PartitionError::Exact)?;
        match b.intern(&neg, true)? {
            Norm::Zero => out.push(v.clone()),
            Norm::NonZeroConst => {}
            Norm::Lit(n) => {
                let mut w = v.clone();
                w.ge.push(n);
                w.ge.sort_unstable();
                w.ge.dedup();
                out.push(w);
            }
        }
    }
    for br in &h.bad {
        let mut w = v.clone();
        w.eq.extend(br.iter().copied());
        w.eq.sort_unstable();
        w.eq.dedup();
        out.push(w);
    }
    Ok(out)
}

/// `n_s - v` (or `v - n_s`) -> (s, v).
fn unit_fixed(l: &Lit) -> Option<(usize, i64)> {
    if l.support.len() != 1 {
        return None;
    }
    let (coef, c) = l.poly.linear_form()?;
    let s = l.support[0];
    match coef[s] {
        1 => Some((s, -c as i64)),
        -1 => Some((s, c as i64)),
        _ => None,
    }
}

impl NfRules {
    /// Exact value of literal `l` at a key (big-integer fallback).
    pub fn lit_sign(&self, l: u32, key: &[i32]) -> Result<i32, PartitionError> {
        self.lits[l as usize]
            .core
            .eval(key)
            .map(|v| v.signum())
            .map_err(|e| PartitionError::Eval(format!("{e}")))
    }

    /// Union coverage of a chart key by rule `r` (exact).
    pub fn rule_covers(&self, r: usize, key: &[i32]) -> Result<bool, PartitionError> {
        let rule = &self.rules[r];
        for &(s, v) in &self.quick[r] {
            if key[s] as i64 != v {
                return Ok(false);
            }
        }
        for &l in &rule.eq {
            if self.lit_sign(l, key)? != 0 {
                return Ok(false);
            }
        }
        for &l in &rule.ge {
            if self.lit_sign(l, key)? < 0 {
                return Ok(false);
            }
        }
        'b: for br in &rule.bad {
            for &l in br {
                if self.lit_sign(l, key)? != 0 {
                    continue 'b;
                }
            }
            return Ok(false);
        }
        Ok(true)
    }

    /// Is the chart key covered by some rule (union semantics)?
    pub fn covered(&self, key: &[i32]) -> Result<bool, PartitionError> {
        for r in 0..self.rules.len() {
            if self.rule_covers(r, key)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn floor(&self, slot: usize) -> i64 {
        if self.lines[slot] { 1 } else { 0 }
    }
}
