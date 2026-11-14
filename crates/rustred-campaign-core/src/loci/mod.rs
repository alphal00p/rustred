//! Exact resolution of the FRAGILE classes of the partition into coupled LOCI (plan 2.4.2 "F classes -> resolve Z
//! exactly"), and domain emptiness / containment (R33, [`contain`]).
//!
//! An F class is covered except on the zero sets of the live exception branches of its fragile rules. The region of
//! a class cell is a lattice region `x = x0 + M t` with integer constraints; every literal becomes a polynomial in
//! `t`. Recursion (the checker's resolve.py v2, made exact): a robust rule covers the region; no alive rule = an
//! uncovered PIECE; otherwise the fragile rule with the fewest live branches is chosen and the region is cut on one
//! member of each live branch (linear members first): a linear factor -> lattice parametrisation of its integer
//! solutions (any integer coefficients, R32/R34); a one-parameter nonlinear factor -> exact integer roots; anything
//! else -> UNRESOLVED (equation, region, exact window enumeration to a stated bound; never dropped, never sampled
//! into a verdict). Region emptiness by exact Fourier-Motzkin with integer rounding. Case equations that are not
//! constant on a piece are resolved recursively on the piece and their covered sub-loci are SUBTRACTED
//! (`covered_on`).
//!
//! No floating point anywhere.

pub mod contain;
pub mod region;

use std::collections::BTreeMap;

use serde_json::{Value as Json, json};

use crate::key::Key;
use crate::partition::classes::{Label, Status};
use crate::partition::exact::{Affine, MPoly, int_roots, linear_factors};
use crate::partition::nf::NfRules;
use crate::partition::{ANY_OFF, CellState, PartitionError, PartitionReport, Proof};
pub use region::Region;

/// Options of the locus resolution.
#[derive(Clone, Debug)]
pub struct LociOptions {
    /// Recursion node cap per class (beyond: the class is reported UNRESOLVED).
    pub max_nodes: u64,
    /// Window (L1 radius around an anchor point, in parameter space) for UNRESOLVED pieces, by dimension.
    pub window_radius: [i64; 6],
    /// Tries per level of the exact witness search.
    pub tries: usize,
    /// Witness points per piece.
    pub witnesses: usize,
}

impl Default for LociOptions {
    fn default() -> Self {
        LociOptions {
            max_nodes: 20_000,
            window_radius: [0, 40, 40, 40, 24, 12],
            tries: 8,
            witnesses: 3,
        }
    }
}

/// A sub-locus of a piece on which a case equation holds, resolved recursively.
#[derive(Clone, Debug)]
pub struct CoveredSubLocus {
    pub equation: String,
    /// All of it covered (no uncovered sub-piece found)?
    pub fully_covered: bool,
}

/// One uncovered piece.
#[derive(Clone, Debug)]
pub struct Piece {
    pub region: Region,
    /// Cut equations in slot variables (or parameters) in order.
    pub trail: Vec<String>,
    pub covered_on: Vec<CoveredSubLocus>,
    pub witness: Vec<Key>,
    pub unbounded: Option<bool>,
}

/// A cut the recursion could not resolve (nonlinear in >= 2 parameters).
#[derive(Clone, Debug)]
pub struct Unresolved {
    pub region: Region,
    pub equation: String,
    pub trail: Vec<String>,
    /// Exact window: bound and the uncovered points on the equation found inside it.
    pub bound: String,
    pub points: Vec<Key>,
    pub n_window: u64,
}

/// A locus: pieces of one face sharing their cut equations.
#[derive(Clone, Debug)]
pub struct Locus {
    pub id: String,
    pub face: (Vec<usize>, Vec<usize>),
    pub equations: Vec<String>,
    pub linear: bool,
    pub unresolved: bool,
    pub pieces: Vec<Piece>,
    pub unresolved_parts: Vec<Unresolved>,
    pub n_classes: usize,
    pub witness: Vec<Key>,
    pub proof: Proof,
    pub unbounded_in_dots: bool,
    pub dim: usize,
}

/// Result of resolving every F class.
#[derive(Clone, Debug, Default)]
pub struct LociReport {
    pub loci: Vec<Locus>,
    pub classes_resolved: usize,
    pub classes_with_gaps: usize,
    pub classes_capped: usize,
    pub exact_errors: u64,
}

impl LociReport {
    pub fn n_linear(&self) -> usize {
        self.loci.iter().filter(|l| !l.unresolved).count()
    }
    pub fn n_unresolved(&self) -> usize {
        self.loci.iter().filter(|l| l.unresolved).count()
    }
    pub fn loci_json(&self) -> Vec<Json> {
        self.loci
            .iter()
            .map(|l| {
                json!({
                    "id": l.id, "face": {"Sd": l.face.0, "Sn": l.face.1}, "equations": l.equations,
                    "kind": if l.linear { "linear" } else { "nonlinear" },
                    "status": if l.unresolved { "UNRESOLVED" } else { "resolved" },
                    "proof": l.proof.tag(), "dim": l.dim, "n_classes": l.n_classes,
                    "pieces": l.pieces.iter().map(|p| json!({
                        "x": p.region.render_x(), "params": p.region.names, "constraints_ge0": p.region.render_cons(),
                        "dim": p.region.k, "finite": p.unbounded.map(|u| !u), "trail": p.trail,
                        "covered_on": p.covered_on.iter().map(|c| json!({"equation": c.equation, "fully_covered": c.fully_covered})).collect::<Vec<_>>(),
                        "witness": p.witness,
                    })).collect::<Vec<_>>(),
                    "unresolved": l.unresolved_parts.iter().map(|u| json!({
                        "equation": u.equation, "x": u.region.render_x(), "params": u.region.names,
                        "constraints_ge0": u.region.render_cons(), "trail": u.trail,
                        "window": {"bound": u.bound, "n_window": u.n_window, "points": u.points},
                    })).collect::<Vec<_>>(),
                    "witness": l.witness,
                })
            })
            .collect()
    }
}

/// The region of a cell (one state or ANY_OFF per slot).
pub fn cell_region(rep: &PartitionReport, cell: &[CellState]) -> Region {
    let n = rep.arity();
    let mut x = vec![];
    let mut cons = vec![];
    let mut names = vec![];
    let free: Vec<usize> = (0..n)
        .filter(|&s| cell[s] == ANY_OFF || rep.states[s][cell[s] as usize].singleton().is_none())
        .collect();
    let k = free.len();
    for s in 0..n {
        if let Some(j) = free.iter().position(|&f| f == s) {
            x.push(Affine::var(k, j));
            names.push(format!("n{s}"));
            let (lo, hi) = if cell[s] == ANY_OFF {
                if rep.nf.lines[s] { (Some(2), None) } else { (None, Some(-1)) }
            } else {
                let r = rep.states[s][cell[s] as usize];
                (r.lo, r.hi)
            };
            if let Some(l) = lo {
                let mut a = Affine::var(k, j);
                a.c = -(l as i128);
                cons.push(a);
            }
            if let Some(h) = hi {
                let mut a = Affine::var(k, j);
                a.coef[j] = -1;
                a.c = h as i128;
                cons.push(a);
            }
        } else {
            let v = rep.states[s][cell[s] as usize].singleton().expect("singleton");
            x.push(Affine::constant(k, v as i128));
        }
    }
    Region { k, x, cons, names }
}

/// Status of a zero-set literal (as a polynomial in the parameters) on a region: Z0 / NZ / V, plus an error flag
/// (an exact step failed: the status is then V, i.e. conservative).
pub fn lit_status(r: &Region, q: &MPoly) -> (Status, bool) {
    if q.is_zero() {
        return (Status::Z0, false);
    }
    if q.as_constant().is_some() {
        return (Status::NZ, false);
    }
    let vars = q.support();
    if vars.len() == 1 {
        let j = vars[0];
        let u = q.univariate(j).expect("univariate");
        let roots = match int_roots(&u) {
            Ok(v) => v,
            Err(_) => return (Status::V, true),
        };
        for x in roots {
            match r.fix(j, x as i128).and_then(|s| s.feasible()) {
                Ok(true) => return (Status::V, false),
                Ok(false) => {}
                Err(_) => return (Status::V, true),
            }
        }
        return (Status::NZ, false);
    }
    if let Some((coef, c)) = q.linear_form() {
        return match r.cut(&coef, c) {
            Ok(None) => (Status::NZ, false),
            Ok(Some(s)) => match s.feasible() {
                Ok(true) => (Status::V, false),
                Ok(false) => (Status::NZ, false),
                Err(_) => (Status::V, true),
            },
            Err(_) => (Status::V, true),
        };
    }
    (Status::V, false)
}

pub(crate) struct Resolver<'a> {
    pub nf: &'a NfRules,
    pub opts: &'a LociOptions,
    pub nodes: u64,
    pub errors: u64,
    pub pieces: Vec<Piece>,
    pub unresolved: Vec<Unresolved>,
    pub capped: bool,
}

enum GeV {
    True,
    False,
    Varies,
}

impl<'a> Resolver<'a> {
    pub fn new(nf: &'a NfRules, opts: &'a LociOptions) -> Self {
        Resolver {
            nf,
            opts,
            nodes: 0,
            errors: 0,
            pieces: vec![],
            unresolved: vec![],
            capped: false,
        }
    }

    fn status(&mut self, r: &Region, q: &MPoly) -> Status {
        let (st, err) = lit_status(r, q);
        if err {
            self.errors += 1;
        }
        st
    }

    fn ge(&mut self, r: &Region, q: &MPoly) -> GeV {
        if let Some(c) = q.as_constant() {
            return if c >= 0 { GeV::True } else { GeV::False };
        }
        if let Some((coef, c)) = q.linear_form() {
            let neg = Affine {
                coef: coef.iter().map(|a| -a).collect(),
                c: -c - 1,
            }; // q <= -1
            let pos = Affine { coef, c };
            let can_neg = r.with(neg).feasible().unwrap_or(true);
            let can_pos = r.with(pos).feasible().unwrap_or(true);
            return match (can_pos, can_neg) {
                (true, false) => GeV::True,
                (false, true) => GeV::False,
                _ => GeV::Varies,
            };
        }
        GeV::Varies
    }

    /// Resolve the uncovered set of `region` w.r.t. the rules `among` (indices into nf.rules).
    pub fn resolve(&mut self, region: Region, among: &[usize], trail: Vec<String>, depth: usize) {
        self.nodes += 1;
        if self.nodes > self.opts.max_nodes || depth > 40 {
            self.capped = true;
            return;
        }
        let mut qcache: BTreeMap<u32, MPoly> = BTreeMap::new();
        let mut scache: BTreeMap<u32, Status> = BTreeMap::new();
        let mut alive: Vec<(usize, Vec<Vec<(u32, MPoly)>>)> = vec![];
        let mut eq_loci: Vec<(usize, MPoly)> = vec![];
        let mut next_among = vec![];
        for &ri in among {
            let rule = &self.nf.rules[ri];
            let mut dead = false;
            for &l in &rule.eq {
                let q = match qcache.get(&l) {
                    Some(q) => q.clone(),
                    None => match region.lit(&self.nf.lits[l as usize].poly) {
                        Ok(q) => {
                            qcache.insert(l, q.clone());
                            q
                        }
                        Err(_) => {
                            self.errors += 1;
                            dead = true;
                            break;
                        }
                    },
                };
                let s = match scache.get(&l) {
                    Some(&s) => s,
                    None => {
                        let s = self.status(&region, &q);
                        scache.insert(l, s);
                        s
                    }
                };
                match s {
                    Status::Z0 => {}
                    Status::NZ => {
                        dead = true;
                        break;
                    }
                    Status::V => {
                        dead = true;
                        eq_loci.push((ri, q));
                        next_among.push(ri);
                        break;
                    }
                }
            }
            if dead {
                continue;
            }
            for &l in &rule.ge {
                let q = match region.lit(&self.nf.lits[l as usize].poly) {
                    Ok(q) => q,
                    Err(_) => {
                        self.errors += 1;
                        dead = true;
                        break;
                    }
                };
                match self.ge(&region, &q) {
                    GeV::True => {}
                    GeV::False => {
                        dead = true;
                        break;
                    }
                    GeV::Varies => {
                        // split the region exactly on the bound: q >= 0 and q <= -1
                        if let Some((coef, c)) = q.linear_form() {
                            let pos = Affine { coef: coef.clone(), c };
                            let neg = Affine { coef: coef.iter().map(|a| -a).collect(), c: -c - 1 };
                            let mut t = trail.clone();
                            t.push(format!("{}>=0", self.nf.lits[l as usize].text));
                            self.resolve(region.with(pos), among, t, depth + 1);
                            let mut t = trail.clone();
                            t.push(format!("{}<=-1", self.nf.lits[l as usize].text));
                            self.resolve(region.with(neg), among, t, depth + 1);
                            return;
                        }
                        dead = true;
                        break;
                    }
                }
            }
            if dead {
                continue;
            }
            next_among.push(ri);
            let mut live = vec![];
            for b in &rule.bad {
                let mut all_z = true;
                let mut all_zv = true;
                let mut vs = vec![];
                for &l in b {
                    let q = match qcache.get(&l) {
                        Some(q) => q.clone(),
                        None => match region.lit(&self.nf.lits[l as usize].poly) {
                            Ok(q) => {
                                qcache.insert(l, q.clone());
                                q
                            }
                            Err(_) => {
                                self.errors += 1;
                                all_z = false;
                                continue;
                            }
                        },
                    };
                    let s = match scache.get(&l) {
                        Some(&s) => s,
                        None => {
                            let s = self.status(&region, &q);
                            scache.insert(l, s);
                            s
                        }
                    };
                    match s {
                        Status::Z0 => {}
                        Status::V => {
                            all_z = false;
                            vs.push((l, q));
                        }
                        Status::NZ => {
                            all_z = false;
                            all_zv = false;
                            break;
                        }
                    }
                }
                if all_z {
                    dead = true;
                    break;
                }
                if all_zv {
                    live.push(vs);
                }
            }
            if dead {
                next_among.pop();
                continue;
            }
            if live.is_empty() {
                return; // robust: covered
            }
            alive.push((ri, live));
        }
        if alive.is_empty() {
            self.emit_piece(region, &next_among, &eq_loci, trail, depth);
            return;
        }
        // fragile rule with the fewest live branches, preferring branches with a linear member
        let nonlin = |live: &Vec<Vec<(u32, MPoly)>>| {
            live.iter()
                .filter(|a| !a.iter().any(|(_, q)| q.total_degree() == 1 || q.support().len() == 1))
                .count()
        };
        alive.sort_by_key(|(ri, live)| (nonlin(live), live.len(), *ri));
        let (_, live) = alive.swap_remove(0);
        let mut done: Vec<MPoly> = vec![];
        for a in live {
            // member with the simplest factorisation
            let mut best: Option<((usize, usize), Vec<MPoly>, u32)> = None;
            for (lid, q) in &a {
                let (fs, rest) = linear_factors(q).unwrap_or((vec![], q.clone()));
                let mut facs = fs;
                if rest.as_constant().is_none() {
                    facs.push(rest);
                }
                let nl = facs.iter().filter(|f| f.total_degree() > 1 && f.support().len() > 1).count();
                let key = (nl, facs.len());
                if best.as_ref().is_none_or(|b| key < b.0) {
                    best = Some((key, facs, *lid));
                }
            }
            let Some((_, facs, lid)) = best else { continue };
            let nfacs = facs.len();
            let lit_text = self.nf.lits[lid as usize].text.clone();
            for (fi, f) in facs.into_iter().enumerate() {
                let f = f.primitive().unwrap_or(f);
                if f.as_constant().is_some() || done.contains(&f) {
                    continue;
                }
                done.push(f.clone());
                let label = if nfacs == 1 { lit_text.clone() } else { format!("{lit_text} [factor {fi}: {}]", f.render(&region.names)) };
                if let Some((coef, c)) = f.linear_form() {
                    match region.cut(&coef, c) {
                        Ok(Some(sub)) => match sub.feasible() {
                            Ok(false) => {}
                            _ => {
                                let mut t = trail.clone();
                                t.push(label);
                                self.resolve(sub, &next_among, t, depth + 1);
                            }
                        },
                        Ok(None) => {}
                        Err(_) => self.errors += 1,
                    }
                } else if f.support().len() == 1 {
                    let j = f.support()[0];
                    let u = f.univariate(j).expect("univariate");
                    match int_roots(&u) {
                        Ok(rs) => {
                            for x in rs {
                                if let Ok(sub) = region.fix(j, x as i128)
                                    && sub.feasible().unwrap_or(true)
                                {
                                    let mut t = trail.clone();
                                    t.push(label.clone());
                                    self.resolve(sub, &next_among, t, depth + 1);
                                }
                            }
                        }
                        Err(_) => {
                            self.errors += 1;
                            self.unresolve(region.clone(), f, label, trail.clone());
                        }
                    }
                } else {
                    self.unresolve(region.clone(), f, label, trail.clone());
                }
            }
        }
    }

    fn unresolve(&mut self, region: Region, f: MPoly, label: String, trail: Vec<String>) {
        // exact window: L1 ball around an anchor point of the region, f = 0 and uncovered
        let k = region.k;
        let radius = self.opts.window_radius[k.min(5)];
        let anchor = region.point(self.opts.tries).ok().flatten().unwrap_or(vec![0; k]);
        let mut pts = vec![];
        let mut n = 0u64;
        let mut t = vec![0i128; k];
        fn rec(
            j: usize,
            budget: i64,
            t: &mut Vec<i128>,
            anchor: &[i128],
            f: &MPoly,
            region: &Region,
            nf: &NfRules,
            n: &mut u64,
            pts: &mut Vec<Key>,
        ) {
            if j == t.len() {
                *n += 1;
                if !region.contains_t(t).unwrap_or(false) || f.eval(t).map(|v| v != 0).unwrap_or(true) {
                    return;
                }
                if let Ok(x) = region.at(t) {
                    let key: Option<Key> = x.iter().map(|&v| i32::try_from(v).ok()).collect();
                    if let Some(key) = key
                        && nf.covered(&key).ok() == Some(false)
                        && pts.len() < 200
                    {
                        pts.push(key);
                    }
                }
                return;
            }
            for d in -budget..=budget {
                t[j] = anchor[j] + d as i128;
                rec(j + 1, budget - d.abs(), t, anchor, f, region, nf, n, pts);
            }
        }
        rec(0, radius, &mut t, &anchor, &f, &region, self.nf, &mut n, &mut pts);
        self.unresolved.push(Unresolved {
            bound: format!("L1 <= {radius} around parameters {anchor:?}"),
            region,
            equation: label,
            trail,
            points: pts,
            n_window: n,
        });
    }

    fn emit_piece(&mut self, region: Region, among: &[usize], eq_loci: &[(usize, MPoly)], trail: Vec<String>, depth: usize) {
        if !region.feasible().unwrap_or(true) {
            return;
        }
        // covered sub-loci: resolve the piece on each linear case-equation locus
        let mut covered_on = vec![];
        let mut seen: Vec<MPoly> = vec![];
        for (_, q) in eq_loci {
            let Some((coef, c)) = q.linear_form() else {
                covered_on.push(CoveredSubLocus {
                    equation: q.render(&region.names),
                    fully_covered: false,
                });
                continue;
            };
            let p = q.primitive().unwrap_or(q.clone());
            if seen.contains(&p) {
                continue;
            }
            seen.push(p);
            if let Ok(Some(sub)) = region.cut(&coef, c)
                && sub.feasible().unwrap_or(true)
            {
                let mut inner = Resolver::new(self.nf, self.opts);
                inner.resolve(sub, among, vec![], depth + 1);
                covered_on.push(CoveredSubLocus {
                    equation: q.render(&region.names),
                    fully_covered: inner.pieces.is_empty() && inner.unresolved.is_empty() && !inner.capped,
                });
            }
        }
        // witnesses: integer points of the piece that the oracle confirms uncovered
        let mut witness = vec![];
        let mut tried = 0;
        let mut anchors: Vec<Vec<i128>> = vec![vec![0; region.k]];
        for j in 0..region.k {
            for d in [3i128, 7, -3, -7] {
                let mut a = vec![0; region.k];
                a[j] = d;
                anchors.push(a);
            }
        }
        for a in anchors {
            if witness.len() >= self.opts.witnesses || tried > 24 {
                break;
            }
            tried += 1;
            if let Ok(Some(t)) = region::find_point(&region.cons, region.k, self.opts.tries, &a)
                && let Ok(x) = region.at(&t)
            {
                let key: Option<Key> = x.iter().map(|&v| i32::try_from(v).ok()).collect();
                if let Some(key) = key
                    && !witness.contains(&key)
                    && self.nf.covered(&key).ok() == Some(false)
                {
                    witness.push(key);
                }
            }
        }
        let unbounded = region.unbounded().ok();
        self.pieces.push(Piece {
            region,
            trail,
            covered_on,
            witness,
            unbounded,
        });
    }
}

/// Resolve every F class of a partition (each `any` slot expanded into floor / off-floor first, so every piece lies
/// on one face) and group the pieces into loci (face, cut equations).
pub fn resolve_fragile_classes(rep: &PartitionReport, opts: &LociOptions) -> Result<LociReport, PartitionError> {
    let mut out = LociReport::default();
    type Gk = ((Vec<usize>, Vec<usize>), Vec<String>, bool);
    let mut groups: BTreeMap<Gk, Locus> = BTreeMap::new();
    let all: Vec<usize> = (0..rep.nf.rules.len()).collect();
    for c in rep.classes.iter().filter(|c| c.label == Label::Fragile) {
        for (face, cell) in rep.expand(c) {
            let region = cell_region(rep, &cell);
            let mut r = Resolver::new(&rep.nf, opts);
            r.resolve(region, &all, vec![], 0);
            out.classes_resolved += 1;
            out.exact_errors += r.errors;
            if r.capped {
                out.classes_capped += 1;
            }
            let has = !r.pieces.is_empty() || r.unresolved.iter().any(|u| !u.points.is_empty());
            if has || !r.unresolved.is_empty() {
                out.classes_with_gaps += usize::from(has);
            }
            for p in r.pieces {
                if p.witness.is_empty() && p.covered_on.iter().any(|c| c.fully_covered) {
                    // every searched point covered on a case equation: keep, flagged by the empty witness list
                }
                let key = (face.clone(), p.trail.clone(), false);
                let e = groups.entry(key).or_insert_with(|| Locus {
                    id: String::new(),
                    face: face.clone(),
                    equations: p.trail.clone(),
                    linear: true,
                    unresolved: false,
                    pieces: vec![],
                    unresolved_parts: vec![],
                    n_classes: 0,
                    witness: vec![],
                    proof: Proof::ExactPartition,
                    unbounded_in_dots: false,
                    dim: 0,
                });
                e.n_classes += 1;
                e.dim = e.dim.max(p.region.k);
                for w in &p.witness {
                    if e.witness.len() < 6 && !e.witness.contains(w) {
                        e.witness.push(w.clone());
                    }
                }
                if p.unbounded == Some(true) {
                    let lines = &rep.nf.lines;
                    if p.region.x.iter().enumerate().any(|(s, a)| lines[s] && !a.is_constant()) {
                        e.unbounded_in_dots = true;
                    }
                }
                e.pieces.push(p);
            }
            for u in r.unresolved {
                let mut eqs = u.trail.clone();
                eqs.push(u.equation.clone());
                let key = (face.clone(), eqs.clone(), true);
                let e = groups.entry(key).or_insert_with(|| Locus {
                    id: String::new(),
                    face: face.clone(),
                    equations: eqs.clone(),
                    linear: false,
                    unresolved: true,
                    pieces: vec![],
                    unresolved_parts: vec![],
                    n_classes: 0,
                    witness: vec![],
                    proof: Proof::Tested {
                        bound: u.bound.clone(),
                    },
                    unbounded_in_dots: false,
                    dim: u.region.k,
                });
                e.n_classes += 1;
                for w in &u.points {
                    if e.witness.len() < 6 && !e.witness.contains(w) {
                        e.witness.push(w.clone());
                    }
                }
                e.unresolved_parts.push(u);
            }
        }
    }
    let mut loci: Vec<Locus> = groups.into_values().collect();
    // a resolved locus without any confirmed uncovered witness is kept only if no searched point was covered
    loci.retain(|l| l.unresolved || !l.witness.is_empty() || l.pieces.iter().any(|p| p.covered_on.is_empty()));
    for (i, l) in loci.iter_mut().enumerate() {
        l.id = format!("L{}", i + 1);
    }
    out.loci = loci;
    Ok(out)
}
