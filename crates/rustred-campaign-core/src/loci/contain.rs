//! Domain emptiness and containment, PROVED by exact sign splits (R33: economy steps on complete nodes are proved by
//! domain containment, never by sampling).
//!
//! `contained_in(d, others)`: is `d \ (others_1 u ... u others_m)` empty? The region of `d` (chart, linear case
//! equations by lattice parametrisation, linear bounds as constraints) is split recursively on undecided LINEAR
//! literals: a zero-set literal f into {f >= 1}, {f <= -1}, {f = 0}; a bound g into {g >= 0}, {g <= -1}; a
//! one-parameter polynomial at its integer roots. On every leaf each literal is decided; a leaf is empty when it is
//! rationally infeasible (exact Fourier-Motzkin with integer rounding), inside an exception branch of `d`, or inside
//! one of the `others`. A leaf left with a nonempty region gives an integer WITNESS, verified with the core's
//! [`CaseDomain::applies`]. Nonlinear multi-parameter literals that stay undecided give `Unknown` (never a guess).

use super::region::Region;
use super::{Status, lit_status};
use crate::cover::RuleRef;
use crate::domain::{CaseDomain, Verdict};
use crate::key::{Key, NodeFrame};
use crate::partition::exact::{Affine, MPoly, int_roots};
use crate::partition::nf::{self, NfOptions, NfRules};
use crate::partition::{DomainList, DomainSource, ElidedPolicy};

/// Verdict of [`is_empty`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Emptiness {
    ProvedEmpty { certificate: String },
    NonEmpty { witness: Key },
    Unknown { reason: String },
}

/// Verdict of [`contained_in`] / [`shadowed`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Containment {
    Proved { certificate: String },
    NotContained { witness: Key },
    Unknown { reason: String },
}

#[derive(Clone)]
struct Other {
    eq: Vec<MPoly>,
    ge: Vec<MPoly>,
    bad: Vec<Vec<MPoly>>,
}

enum Sub {
    Empty,
    Witness(Vec<i128>),
    Unknown(String),
}

struct Splitter {
    nodes: u64,
    leaves: u64,
    max_nodes: u64,
}

fn ge_val(r: &Region, q: &MPoly) -> Option<bool> {
    if let Some(c) = q.as_constant() {
        return Some(c >= 0);
    }
    let (coef, c) = q.linear_form()?;
    let neg = Affine {
        coef: coef.iter().map(|a| -a).collect(),
        c: -c - 1,
    };
    let pos = Affine { coef, c };
    match (r.with(pos).feasible().unwrap_or(true), r.with(neg).feasible().unwrap_or(true)) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

impl Splitter {
    fn solve(&mut self, r: Region, mut holes: Vec<Vec<MPoly>>, mut others: Vec<Other>) -> Sub {
        self.nodes += 1;
        if self.nodes > self.max_nodes {
            return Sub::Unknown(format!("sign-split node cap {} reached", self.max_nodes));
        }
        match r.feasible() {
            Ok(false) => {
                self.leaves += 1;
                return Sub::Empty;
            }
            Ok(true) => {}
            Err(e) => return Sub::Unknown(format!("Fourier-Motzkin: {e}")),
        }
        // literals are kept in slot variables and composed with the region's parametrisation at every node
        let t = |q: &MPoly| r.lit(q).unwrap_or_else(|_| q.clone());
        let st = |q: &MPoly| match r.lit(q) {
            Ok(tq) => lit_status(&r, &tq).0,
            Err(_) => Status::V,
        };
        // holes of d
        let mut nh = vec![];
        for h in holes.drain(..) {
            let mut left = vec![];
            let mut nz = false;
            for q in h {
                match st(&q) {
                    Status::Z0 => {}
                    Status::NZ => {
                        nz = true;
                        break;
                    }
                    Status::V => left.push(q),
                }
            }
            if nz {
                continue;
            }
            if left.is_empty() {
                self.leaves += 1;
                return Sub::Empty; // the whole region lies on an exception branch of d
            }
            nh.push(left);
        }
        holes = nh;
        // others
        let mut no = vec![];
        for o in others.drain(..) {
            let mut keep = Other {
                eq: vec![],
                ge: vec![],
                bad: vec![],
            };
            let mut dead = false;
            for q in o.eq {
                match st(&q) {
                    Status::Z0 => {}
                    Status::NZ => {
                        dead = true;
                        break;
                    }
                    Status::V => keep.eq.push(q),
                }
            }
            if dead {
                continue;
            }
            for q in o.ge {
                match r.lit(&q).ok().and_then(|tq| ge_val(&r, &tq)) {
                    Some(true) => {}
                    Some(false) => {
                        dead = true;
                        break;
                    }
                    None => keep.ge.push(q),
                }
            }
            if dead {
                continue;
            }
            for b in o.bad {
                let mut left = vec![];
                let mut nz = false;
                for q in b {
                    match st(&q) {
                        Status::Z0 => {}
                        Status::NZ => {
                            nz = true;
                            break;
                        }
                        Status::V => left.push(q),
                    }
                }
                if nz {
                    continue;
                }
                if left.is_empty() {
                    dead = true;
                    break;
                }
                keep.bad.push(left);
            }
            if dead {
                continue;
            }
            if keep.eq.is_empty() && keep.ge.is_empty() && keep.bad.is_empty() {
                self.leaves += 1;
                return Sub::Empty; // the region lies inside this other domain
            }
            no.push(keep);
        }
        others = no;
        if holes.is_empty() && others.is_empty() {
            return match r.point(10) {
                Ok(Some(t)) => Sub::Witness(t),
                Ok(None) => Sub::Unknown("no integer point found in a rationally feasible leaf".into()),
                Err(e) => Sub::Unknown(format!("{e}")),
            };
        }
        // pick a splitting literal: linear zero-set literal, then linear bound, then a one-parameter polynomial
        let zs = holes
            .iter()
            .flatten()
            .chain(others.iter().flat_map(|o| o.eq.iter().chain(o.bad.iter().flatten())));
        let mut lin: Option<MPoly> = None;
        let mut uni: Option<MPoly> = None;
        for q in zs {
            let tq = t(q);
            if tq.total_degree() == 1 {
                lin = Some(tq);
                break;
            }
            if uni.is_none() && tq.support().len() == 1 {
                uni = Some(tq);
            }
        }
        if let Some(f) = lin {
            let (coef, c) = f.linear_form().expect("linear");
            let pos = Affine { coef: coef.clone(), c: c - 1 }; // f >= 1
            let neg = Affine {
                coef: coef.iter().map(|a| -a).collect(),
                c: -c - 1,
            }; // f <= -1
            let mut parts = vec![r.with(pos), r.with(neg)];
            match r.cut(&coef, c) {
                Ok(Some(z)) => parts.push(z),
                Ok(None) => {}
                Err(e) => return Sub::Unknown(format!("{e}")),
            }
            return self.combine(parts, &holes, &others, &r);
        }
        if let Some(g) = others.iter().flat_map(|o| o.ge.iter()).map(t).find(|q| q.total_degree() == 1) {
            let (coef, c) = g.linear_form().expect("linear");
            let pos = Affine { coef: coef.clone(), c };
            let neg = Affine {
                coef: coef.iter().map(|a| -a).collect(),
                c: -c - 1,
            };
            return self.combine(vec![r.with(pos), r.with(neg)], &holes, &others, &r);
        }
        if let Some(u) = uni {
            let j = u.support()[0];
            let roots = match int_roots(&u.univariate(j).expect("univariate")) {
                Ok(v) => v,
                Err(e) => return Sub::Unknown(format!("{e}")),
            };
            // t_j = root for each root, and the open intervals between them
            let mut parts = vec![];
            let mut prev: Option<i128> = None;
            for &x in &roots {
                let x = x as i128;
                let mut iv = r.clone();
                let mut hi = Affine::constant(r.k, x - 1);
                hi.coef[j] = -1;
                iv = iv.with(hi);
                if let Some(p) = prev {
                    let mut lo = Affine::constant(r.k, -(p + 1));
                    lo.coef[j] = 1;
                    iv = iv.with(lo);
                }
                parts.push(iv);
                match r.fix(j, x) {
                    Ok(fx) => {
                        // re-embed as an equality region: keep parameter count by a pair of bounds
                        let _ = fx;
                        let mut a = Affine::constant(r.k, -x);
                        a.coef[j] = 1;
                        let mut b = Affine::constant(r.k, x);
                        b.coef[j] = -1;
                        parts.push(r.with(a).with(b));
                    }
                    Err(e) => return Sub::Unknown(format!("{e}")),
                }
                prev = Some(x);
            }
            let mut last = r.clone();
            if let Some(p) = prev {
                let mut lo = Affine::constant(r.k, -(p + 1));
                lo.coef[j] = 1;
                last = last.with(lo);
            }
            parts.push(last);
            if roots.is_empty() {
                // the polynomial has no integer root: it is NZ everywhere; lit_status says so, so this is unreachable
                return Sub::Unknown("univariate literal without integer root left undecided".into());
            }
            return self.combine(parts, &holes, &others, &r);
        }
        // only nonlinear multi-parameter literals left: look for a verified witness, else Unknown
        match r.point(10) {
            Ok(Some(t)) => Sub::Witness(t), // verified by the caller against the domains themselves
            _ => Sub::Unknown("nonlinear literals in two or more parameters stay undecided".into()),
        }
    }

    fn combine(&mut self, parts: Vec<Region>, holes: &[Vec<MPoly>], others: &[Other], parent: &Region) -> Sub {
        let mut unknown: Option<String> = None;
        for p in parts {
            let _ = parent;
            match self.solve(p, holes.to_vec(), others.to_vec()) {
                Sub::Empty => {}
                Sub::Witness(t) => return Sub::Witness(t),
                Sub::Unknown(why) => unknown = Some(why),
            }
        }
        match unknown {
            Some(w) => Sub::Unknown(w),
            None => Sub::Empty,
        }
    }
}

fn nf_of(ds: &[&CaseDomain], frame: &NodeFrame) -> Result<NfRules, String> {
    let mut l = DomainList::new(frame.arity());
    for (i, d) in ds.iter().enumerate() {
        l.push(
            RuleRef {
                table: 0,
                record: i,
                label: i as i64,
            },
            (*d).clone(),
        );
    }
    nf::build(
        &l,
        frame,
        &NfOptions {
            elided: ElidedPolicy::Ordinary,
            dedup: false,
            subset: None,
            factor_exceptions: true,
        },
    )
    .map_err(|e| format!("{e}"))
}

/// The chart region (every slot a parameter) restricted to the domain's linear case equations and bounds. Nonlinear
/// equations and bounds are returned to be kept as conditions.
fn domain_region(nfr: &NfRules, r: usize) -> Result<(Region, Vec<MPoly>, Vec<MPoly>), String> {
    let n = nfr.arity;
    let mut reg = Region {
        k: n,
        x: (0..n).map(|j| Affine::var(n, j)).collect(),
        cons: (0..n)
            .map(|j| {
                let mut a = Affine::var(n, j);
                if nfr.lines[j] {
                    a.c = -1;
                } else {
                    a.coef[j] = -1;
                }
                a
            })
            .collect(),
        names: (0..n).map(|j| format!("n{j}")).collect(),
    };
    let rule = &nfr.rules[r];
    let mut eq_nl = vec![];
    let mut ge_nl = vec![];
    for &l in &rule.eq {
        let q = reg.lit(&nfr.lits[l as usize].poly).map_err(|e| format!("{e}"))?;
        if let Some(c) = q.as_constant() {
            if c != 0 {
                reg.cons.push(Affine::constant(reg.k, -1));
            }
            continue;
        }
        match q.linear_form() {
            Some((coef, c)) => match reg.cut(&coef, c).map_err(|e| format!("{e}"))? {
                Some(s) => reg = s,
                None => {
                    reg.cons.push(Affine::constant(reg.k, -1));
                    return Ok((reg, vec![], vec![]));
                }
            },
            None => eq_nl.push(nfr.lits[l as usize].poly.clone()),
        }
    }
    for &l in &rule.ge {
        let q = reg.lit(&nfr.lits[l as usize].poly).map_err(|e| format!("{e}"))?;
        match q.linear_form() {
            Some((coef, c)) => reg.cons.push(Affine { coef, c }),
            None => ge_nl.push(nfr.lits[l as usize].poly.clone()),
        }
    }
    Ok((reg, eq_nl, ge_nl))
}

fn applies(d: &CaseDomain, key: &[i32]) -> bool {
    matches!(d.applies(key), Verdict::Applies) || (d.witness_elided && matches!(d.applies(key), Verdict::Elided))
}

/// Is `d \ (others)` empty? Exact for linear structure.
pub fn contained_in(d: &CaseDomain, others: &[&CaseDomain], frame: &NodeFrame) -> Containment {
    let mut all = vec![d];
    all.extend_from_slice(others);
    let nfr = match nf_of(&all, frame) {
        Ok(x) => x,
        Err(e) => return Containment::Unknown { reason: e },
    };
    if nfr.rules.is_empty() || nfr.sources[0] != vec![0] {
        // d itself was dropped by the normal form (fixed value outside the chart, constant contradiction)
        return Containment::Proved {
            certificate: "the domain is dropped by the normal form (fixed value outside the chart or a constant contradiction)".into(),
        };
    }
    let (reg, eq_nl, ge_nl) = match domain_region(&nfr, 0) {
        Ok(x) => x,
        Err(e) => return Containment::Unknown { reason: e },
    };
    let lift = |p: &MPoly| Some(p.clone());
    let mut holes: Vec<Vec<MPoly>> = vec![];
    for b in &nfr.rules[0].bad {
        match b.iter().map(|&l| lift(&nfr.lits[l as usize].poly)).collect::<Option<Vec<_>>>() {
            Some(v) => holes.push(v),
            None => return Containment::Unknown { reason: "overflow lifting a literal".into() },
        }
    }
    if !eq_nl.is_empty() || !ge_nl.is_empty() {
        // nonlinear case equations of d: a point must satisfy them; treat their complement as holes is not exact,
        // so fall back to Unknown unless the region is already empty
        if let Ok(false) = reg.feasible() {
            return Containment::Proved {
                certificate: "linear part of the case is integer-infeasible (Fourier-Motzkin)".into(),
            };
        }
        return Containment::Unknown {
            reason: "nonlinear case equation or bound in the domain".into(),
        };
    }
    let mut os = vec![];
    for (ri, rule) in nfr.rules.iter().enumerate().skip(1) {
        let _ = ri;
        let conv = |ls: &[u32]| ls.iter().map(|&l| lift(&nfr.lits[l as usize].poly)).collect::<Option<Vec<_>>>();
        let (Some(eq), Some(ge)) = (conv(&rule.eq), conv(&rule.ge)) else {
            return Containment::Unknown { reason: "overflow lifting a literal".into() };
        };
        let Some(bad) = rule.bad.iter().map(|b| conv(b)).collect::<Option<Vec<_>>>() else {
            return Containment::Unknown { reason: "overflow lifting a literal".into() };
        };
        os.push(Other { eq, ge, bad });
    }
    let mut sp = Splitter {
        nodes: 0,
        leaves: 0,
        max_nodes: 200_000,
    };
    match sp.solve(reg.clone(), holes, os) {
        Sub::Empty => Containment::Proved {
            certificate: format!(
                "exact sign split: {} nodes, {} leaves, each integer-infeasible (Fourier-Motzkin with rounding), on an exception branch, or inside a listed domain",
                sp.nodes, sp.leaves
            ),
        },
        Sub::Unknown(r) => Containment::Unknown { reason: r },
        Sub::Witness(_) => {
            // the witness is in parameters of some leaf region; re-find a key by the leaf search is not kept, so
            // search the domain's region for a verified witness point
            match verified_witness(&reg, d, others) {
                Some(k) => Containment::NotContained { witness: k },
                None => Containment::Unknown {
                    reason: "a nonempty leaf was found but no verified integer witness".into(),
                },
            }
        }
    }
}

/// Exact search for a key in `d` and in none of `others`, verified by `CaseDomain::applies`.
fn verified_witness(reg: &Region, d: &CaseDomain, others: &[&CaseDomain]) -> Option<Key> {
    let k = reg.k;
    let check = |t: &[i128]| -> Option<Key> {
        let x = reg.at(t).ok()?;
        let key: Key = x.iter().map(|&v| i32::try_from(v).ok()).collect::<Option<_>>()?;
        if applies(d, &key) && !others.iter().any(|o| applies(o, &key)) {
            Some(key)
        } else {
            None
        }
    };
    let base = reg.point(10).ok()??;
    if let Some(w) = check(&base) {
        return Some(w);
    }
    // L1 walk around the base point
    for radius in 1..=12i128 {
        let mut stack: Vec<(usize, i128, Vec<i128>)> = vec![(0, radius, base.clone())];
        while let Some((j, left, t)) = stack.pop() {
            if j == k {
                if left == 0 && reg.contains_t(&t).unwrap_or(false)
                    && let Some(w) = check(&t)
                {
                    return Some(w);
                }
                continue;
            }
            for dlt in -left..=left {
                let mut t2 = t.clone();
                t2[j] += dlt;
                stack.push((j + 1, left - dlt.abs(), t2));
            }
        }
    }
    None
}

/// Is the domain empty (no chart key applies)?
pub fn is_empty(d: &CaseDomain, frame: &NodeFrame) -> Emptiness {
    match contained_in(d, &[], frame) {
        Containment::Proved { certificate } => Emptiness::ProvedEmpty { certificate },
        Containment::NotContained { witness } => Emptiness::NonEmpty { witness },
        Containment::Unknown { reason } => Emptiness::Unknown { reason },
    }
}

/// Is domain `i` of the source shadowed under the dispatch order, i.e. contained in the union of the domains before
/// it (it never fires under first match)? Only earlier domains whose fixed slots are compatible are passed on.
pub fn shadowed(src: &dyn DomainSource, i: usize, frame: &NodeFrame) -> Containment {
    let d = src.domain(i);
    let mut others: Vec<&CaseDomain> = vec![];
    for j in 0..i {
        let o = src.domain(j);
        let compatible = d.fixed.iter().zip(&o.fixed).all(|(a, b)| match (a, b) {
            (Some(x), Some(y)) => x == y,
            _ => true,
        });
        if compatible {
            others.push(o);
        }
    }
    contained_in(d, &others, frame)
}
