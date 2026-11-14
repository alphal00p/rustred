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
//!
//! Semantics (audit-part-A C2 / M3 / m5):
//! - `is_empty(d)`: d APPLIES at no chart key (geometric: its elided flag is ignored, poles / recenter bounds are
//!   holes). A record that is selected somewhere but never applies (pole everywhere, elided) is empty in this sense:
//!   removing it never loses coverage, but it does change the dispatch (keys it blocked become reachable).
//! - `contained_in(d, others)`: every key where d applies is a key where some other APPLIES (an elided other never
//!   applies and is ignored).
//! - `shadowed(src, i)`: every key where record i applies has an EARLIER record that MATCHES it (is selected: served
//!   or blocked, its poles / recenter / range bounds / elision do not matter): under first match record i never
//!   fires. This is the meaning of [`shadowed`]; it is the only proof offered for [`removal_preserves_dispatch`].
//! - A domain the normal form had to exclude or weaken (an unrepresentable literal) is never the subject of a proof
//!   (`Unknown`); excluded / weakened OTHERS only shrink (stated in the certificate).

use super::region::Region;
use super::{Status, lit_status};
use crate::cover::RuleRef;
use crate::domain::{CaseDomain, Stage, Verdict};
use crate::key::{Key, NodeFrame};
use crate::partition::exact::{Affine, MPoly, int_roots};
use crate::partition::nf::{self, NfNote, NfOptions, NfRule, NfRules};
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
    depth: usize,
    max_depth: usize,
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
    match (
        r.with(pos).feasible().unwrap_or(true),
        r.with(neg).feasible().unwrap_or(true),
    ) {
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
        if self.depth > self.max_depth {
            return Sub::Unknown(format!("sign-split depth cap {} reached", self.max_depth));
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
                Ok(None) => {
                    Sub::Unknown("no integer point found in a rationally feasible leaf".into())
                }
                Err(e) => Sub::Unknown(format!("{e}")),
            };
        }
        // pick a splitting literal: linear zero-set literal, then linear bound, then a one-parameter polynomial
        let zs = holes.iter().flatten().chain(
            others
                .iter()
                .flat_map(|o| o.eq.iter().chain(o.bad.iter().flatten())),
        );
        let mut lin: Option<MPoly> = None;
        let mut uni: Option<MPoly> = None;
        for q in zs {
            // audit-part-A m4: a literal that cannot be composed is never used as a parameter polynomial
            let tq = match r.lit(q) {
                Ok(x) => x,
                Err(e) => return Sub::Unknown(format!("composing a literal with the region: {e}")),
            };
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
            let pos = Affine {
                coef: coef.clone(),
                c: c - 1,
            }; // f >= 1
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
        let mut ge_lin: Option<MPoly> = None;
        for q in others.iter().flat_map(|o| o.ge.iter()) {
            match r.lit(q) {
                Ok(tq) if tq.total_degree() == 1 => {
                    ge_lin = Some(tq);
                    break;
                }
                Ok(_) => {}
                Err(e) => return Sub::Unknown(format!("composing a bound with the region: {e}")),
            }
        }
        if let Some(g) = ge_lin {
            let (coef, c) = g.linear_form().expect("linear");
            let pos = Affine {
                coef: coef.clone(),
                c,
            };
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
                    // the parameter is FIXED (not bounded by a pair of inequalities: the literal would stay a
                    // nonconstant polynomial in t_j and the split would recurse forever; audit-part-B shadow_hang)
                    Ok(fx) => parts.push(fx),
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
                return Sub::Unknown(
                    "univariate literal without integer root left undecided".into(),
                );
            }
            return self.combine(parts, &holes, &others, &r);
        }
        // only nonlinear multi-parameter literals left: look for a verified witness, else Unknown
        match r.point(10) {
            Ok(Some(t)) => Sub::Witness(t), // verified by the caller against the domains themselves
            _ => Sub::Unknown("nonlinear literals in two or more parameters stay undecided".into()),
        }
    }

    fn combine(
        &mut self,
        parts: Vec<Region>,
        holes: &[Vec<MPoly>],
        others: &[Other],
        parent: &Region,
    ) -> Sub {
        let mut unknown: Option<String> = None;
        for p in parts {
            let _ = parent;
            self.depth += 1;
            let sub = self.solve(p, holes.to_vec(), others.to_vec());
            self.depth -= 1;
            match sub {
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

fn nf_of(ds: &[CaseDomain], frame: &NodeFrame, elided: ElidedPolicy) -> Result<NfRules, String> {
    let mut l = DomainList::new(frame.arity());
    for (i, d) in ds.iter().enumerate() {
        l.push(
            RuleRef {
                table: 0,
                record: i,
                label: i as i64,
            },
            d.clone(),
        );
    }
    nf::build(
        &l,
        frame,
        &NfOptions {
            elided,
            dedup: false,
            subset: None,
            factor_exceptions: true,
        },
    )
    .map_err(|e| format!("{e}"))
}

/// The chart region (every slot a parameter) restricted to the rule's linear case equations and bounds. Nonlinear
/// equations and bounds are returned to be kept as conditions.
fn domain_region(nfr: &NfRules, rule: &NfRule) -> Result<(Region, Vec<MPoly>, Vec<MPoly>), String> {
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
    let mut eq_nl = vec![];
    let mut ge_nl = vec![];
    for &l in &rule.eq {
        let q = reg
            .lit(&nfr.lits[l as usize].poly)
            .map_err(|e| format!("{e}"))?;
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
        let q = reg
            .lit(&nfr.lits[l as usize].poly)
            .map_err(|e| format!("{e}"))?;
        match q.linear_form() {
            Some((coef, c)) => reg.cons.push(Affine { coef, c }),
            None => ge_nl.push(nfr.lits[l as usize].poly.clone()),
        }
    }
    Ok((reg, eq_nl, ge_nl))
}

/// The subject applies (geometric: an elided subject is read as an ordinary domain).
fn subject_applies(d: &CaseDomain, key: &[i32]) -> bool {
    matches!(d.applies(key), Verdict::Applies)
        || (d.witness_elided && matches!(d.applies(key), Verdict::Elided))
}

/// How the OTHER domains count.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OthersAre {
    /// An other counts where it APPLIES (union coverage).
    Applying,
    /// An other counts where it is SELECTED (first match stops there, served or blocked).
    Matching,
}

fn other_counts(o: &CaseDomain, key: &[i32], mode: OthersAre) -> bool {
    let v = o.applies(key);
    match mode {
        OthersAre::Applying => v == Verdict::Applies,
        OthersAre::Matching => v.stage() != Stage::NotSelected,
    }
}

fn lits(nfr: &NfRules, ls: &[u32]) -> Vec<MPoly> {
    ls.iter()
        .map(|&l| nfr.lits[l as usize].poly.clone())
        .collect()
}

/// Core of [`contained_in`] / [`shadowed`].
fn containment(
    d: &CaseDomain,
    others: &[&CaseDomain],
    frame: &NodeFrame,
    mode: OthersAre,
) -> Containment {
    // the subject is read geometrically (elided flag ignored); in Applying mode an elided other never applies
    let mut subject = d.clone();
    subject.witness_elided = false;
    let mut all: Vec<CaseDomain> = vec![subject];
    let mut kept: Vec<&CaseDomain> = vec![];
    for o in others {
        if mode == OthersAre::Applying && o.witness_elided {
            continue;
        }
        all.push((*o).clone());
        kept.push(o);
    }
    // FailClosed so that every other's SELECTED set is available (`selected`), its APPLIES set as its rule
    let nfr = match nf_of(&all, frame, ElidedPolicy::FailClosed) {
        Ok(x) => x,
        Err(e) => return Containment::Unknown { reason: e },
    };
    for n in nfr.notes_of(0) {
        match n {
            NfNote::Excluded(w) | NfNote::Weakened(w) => {
                return Containment::Unknown {
                    reason: format!(
                        "the domain is not represented exactly in the normal form: {w}"
                    ),
                };
            }
            NfNote::DroppedEmpty(w) => {
                return Containment::Proved {
                    certificate: format!("the domain never applies (exact): {w}"),
                };
            }
            NfNote::ElidedRemoved | NfNote::ElidedCredited => {}
        }
    }
    let Some(r0) = nfr.sources.iter().position(|s| s == &vec![0]) else {
        return Containment::Unknown {
            reason: "the domain has no rule in the normal form and no exact reason for it".into(),
        };
    };
    let subject_rule = nfr.rules[r0].clone();
    let (reg, eq_nl, ge_nl) = match domain_region(&nfr, &subject_rule) {
        Ok(x) => x,
        Err(e) => return Containment::Unknown { reason: e },
    };
    let holes: Vec<Vec<MPoly>> = subject_rule.bad.iter().map(|b| lits(&nfr, b)).collect();
    if !eq_nl.is_empty() || !ge_nl.is_empty() {
        // nonlinear case equations of d: a point must satisfy them; treating their complement as holes is not
        // exact, so fall back to Unknown unless the region is already empty
        if let Ok(false) = reg.feasible() {
            return Containment::Proved {
                certificate: "linear part of the case is integer-infeasible (Fourier-Motzkin)"
                    .into(),
            };
        }
        return Containment::Unknown {
            reason: "nonlinear case equation or bound in the domain".into(),
        };
    }
    // the others: Applying = their rules (applies sets); Matching = their SELECTED sets where known
    let mut os = vec![];
    let mut shrunk = 0usize;
    for s in 1..all.len() {
        if nfr.notes_of(s).any(NfNote::forbids_proof_about_domain) {
            shrunk += 1; // excluded / weakened: represented by a subset (only shrinks the union)
        }
        let rule = match (mode, nfr.selected.get(&s)) {
            (OthersAre::Matching, Some(sel)) => Some(sel.clone()),
            _ => nfr
                .sources
                .iter()
                .position(|x| x == &vec![s])
                .map(|k| nfr.rules[k].clone()),
        };
        if let Some(rule) = rule {
            os.push(Other {
                eq: lits(&nfr, &rule.eq),
                ge: lits(&nfr, &rule.ge),
                bad: rule.bad.iter().map(|b| lits(&nfr, b)).collect(),
            });
        }
    }
    let mut sp = Splitter {
        nodes: 0,
        leaves: 0,
        max_nodes: 200_000,
        depth: 0,
        max_depth: 400,
    };
    match sp.solve(reg.clone(), holes, os) {
        Sub::Empty => Containment::Proved {
            certificate: format!(
                "exact sign split: {} nodes, {} leaves, each integer-infeasible (Fourier-Motzkin with rounding), on an exception branch, or inside a listed domain ({}){}",
                sp.nodes,
                sp.leaves,
                match mode {
                    OthersAre::Applying => "applies sets",
                    OthersAre::Matching => "selected sets",
                },
                if shrunk > 0 {
                    format!(
                        "; {shrunk} other domain(s) represented by a subset (only shrinks them)"
                    )
                } else {
                    String::new()
                }
            ),
        },
        Sub::Unknown(r) => Containment::Unknown { reason: r },
        Sub::Witness(_) => {
            // the witness is in parameters of some leaf region; search the domain's region for a verified witness
            match verified_witness(&reg, d, &kept, mode) {
                Some(k) => Containment::NotContained { witness: k },
                None => Containment::Unknown {
                    reason: "a nonempty leaf was found but no verified integer witness".into(),
                },
            }
        }
    }
}

/// Is `d \ (others)` empty (every key where d applies is a key where some other APPLIES)? Exact for linear
/// structure; `Unknown` otherwise and whenever d is not represented exactly.
pub fn contained_in(d: &CaseDomain, others: &[&CaseDomain], frame: &NodeFrame) -> Containment {
    containment(d, others, frame, OthersAre::Applying)
}

/// Exact search for a key in `d` and in none of `others`, verified by `CaseDomain::applies`.
fn verified_witness(
    reg: &Region,
    d: &CaseDomain,
    others: &[&CaseDomain],
    mode: OthersAre,
) -> Option<Key> {
    let k = reg.k;
    let check = |t: &[i128]| -> Option<Key> {
        let x = reg.at(t).ok()?;
        let key: Key = x
            .iter()
            .map(|&v| i32::try_from(v).ok())
            .collect::<Option<_>>()?;
        if subject_applies(d, &key) && !others.iter().any(|o| other_counts(o, &key, mode)) {
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
                if left == 0
                    && reg.contains_t(&t).unwrap_or(false)
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

/// Is the domain empty (it APPLIES at no chart key)? See the module documentation for what this does NOT say about
/// dispatch.
pub fn is_empty(d: &CaseDomain, frame: &NodeFrame) -> Emptiness {
    match contained_in(d, &[], frame) {
        Containment::Proved { certificate } => Emptiness::ProvedEmpty { certificate },
        Containment::NotContained { witness } => Emptiness::NonEmpty { witness },
        Containment::Unknown { reason } => Emptiness::Unknown { reason },
    }
}

/// Is record `i` of the source SHADOWED under the dispatch order: every key where it applies has an EARLIER record
/// that MATCHES it (is selected there: served or blocked), so under first match record `i` never fires? Earlier
/// records whose fixed slots conflict with record i's are skipped (they never match its keys).
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
    containment(d, &others, frame, OthersAre::Matching)
}

/// Does removing record `i` leave EVERY key's dispatch verdict unchanged (audit-part-A M3 (d))? Removing i changes
/// nothing iff record i is never the first selected record at any key. Offered proof: [`shadowed`] (every key where
/// i applies has an earlier MATCHING record) AND record i is never selected-but-blocked first (its blocked set is
/// empty: no pole locus, recenter bound, target-pattern range bound or elision; then "never first where it applies"
/// is "never first"). A record with a blocked set gives `Unknown` (its keys falling to later records with the same
/// verdict is not offered as a proof).
pub fn removal_preserves_dispatch(
    src: &dyn DomainSource,
    i: usize,
    frame: &NodeFrame,
) -> Containment {
    let d = src.domain(i);
    let has_blocker = !d.poles.is_empty()
        || !d.recenter_bounds.is_empty()
        || d.witness_elided
        || matches!(&d.target, crate::domain::TargetMap::Pattern(t)
            if t.iter().any(|x| matches!(x, crate::domain::TargetSlot::Param { offset, .. } if *offset != 0)));
    if has_blocker {
        return Containment::Unknown {
            reason: "the record has a blocked set (pole locus, recenter bound, target range or elision): where it is selected but blocked its removal changes the dispatch; not offered as a proof".into(),
        };
    }
    match shadowed(src, i, frame) {
        Containment::Proved { certificate } => Containment::Proved {
            certificate: format!(
                "the record has no blocked set and is shadowed (never the first match): {certificate}"
            ),
        },
        o => o,
    }
}
