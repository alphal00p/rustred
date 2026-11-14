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
//! into a verdict). Region emptiness by exact Fourier-Motzkin with integer rounding.
//!
//! Covered sub-loci (audit-part-A C1, no sampling in any decision): where no rule is alive on a region except rules
//! whose case equation `f` varies there, such a rule covers at most points of `{f = 0}`. The uncovered set is then
//! EXACTLY `(region minus every hyperplane {f_i = 0})` together with the uncovered set of each `region n {f_i = 0}`,
//! resolved recursively with every rule (a lattice cut). The first part is reported as a piece carrying the list of
//! subtracted hyperplanes; it is dropped only when PROVED empty (exact sign split, node-capped), never because
//! sampled points were covered. A nonlinear covered sub-locus in two or more parameters is not subtracted (the piece
//! is a superset, flagged `covered_part_unresolved`). Witnesses are only attached to reported pieces.
//!
//! Every cap (nodes, depth), exact-arithmetic overflow or failed cut yields an explicit UNRESOLVED region with its
//! reason (audit-part-A M1): reported as an open gap region, making the partition outcome `Incomplete`.
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
    /// Recursion node cap per class (beyond: the remaining region is reported UNRESOLVED).
    pub max_nodes: u64,
    /// Recursion depth cap (beyond: the region is reported UNRESOLVED).
    pub max_depth: usize,
    /// Node cap of the exact emptiness test of a piece minus its subtracted hyperplanes (beyond: the piece is
    /// reported, flagged `covered_part_unresolved`).
    pub max_split_nodes: u64,
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
            max_depth: 40,
            max_split_nodes: 4_096,
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

/// One uncovered piece: `region` minus the hyperplanes `minus` (exact), unless `covered_part_unresolved`.
#[derive(Clone, Debug)]
pub struct Piece {
    pub region: Region,
    /// Cut equations in slot variables (or parameters) in order.
    pub trail: Vec<String>,
    /// Covered sub-loci of the piece's region: each hyperplane is subtracted exactly from this piece and its
    /// uncovered part is resolved (and reported) separately; `fully_covered` = that resolution found no gap.
    pub covered_on: Vec<CoveredSubLocus>,
    /// The piece is `region` minus these hyperplanes (linear case equations, in parameters).
    pub minus: Vec<String>,
    /// Refinements on the way here (case-equation hyperplanes the region lies on).
    pub refinements: Vec<String>,
    /// A covered sub-locus (nonlinear in >= 2 parameters) could not be subtracted, or the emptiness of
    /// `region minus hyperplanes` could not be decided: the piece is a SUPERSET of the uncovered set (conservative).
    pub covered_part_unresolved: bool,
    /// Uncovered keys of the piece confirmed by the exact oracle (witnesses only, never a decision).
    pub witness: Vec<Key>,
    pub unbounded: Option<bool>,
}

/// A region the recursion could not resolve: a nonlinear cut in >= 2 parameters, a cap or an exact-arithmetic
/// failure. Always an OPEN gap region.
#[derive(Clone, Debug)]
pub struct Unresolved {
    pub region: Region,
    /// Why it is unresolved.
    pub reason: String,
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
    /// Number of unresolved parts (every one an open gap region).
    pub fn n_unresolved_parts(&self) -> usize {
        self.loci.iter().map(|l| l.unresolved_parts.len()).sum()
    }
    /// The distinct reasons of the unresolved parts.
    pub fn unresolved_reasons(&self) -> Vec<String> {
        let mut r: Vec<String> = self
            .loci
            .iter()
            .flat_map(|l| l.unresolved_parts.iter().map(|u| u.reason.clone()))
            .collect();
        r.sort();
        r.dedup();
        r
    }
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
                        "minus": p.minus, "refinements": p.refinements, "covered_part_unresolved": p.covered_part_unresolved,
                        "witness": p.witness,
                    })).collect::<Vec<_>>(),
                    "unresolved": l.unresolved_parts.iter().map(|u| json!({
                        "reason": u.reason, "equation": u.equation, "x": u.region.render_x(), "params": u.region.names,
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
    for (s, &cs) in cell.iter().enumerate().take(n) {
        if let Some(j) = free.iter().position(|&f| f == s) {
            x.push(Affine::var(k, j));
            names.push(format!("n{s}"));
            let (lo, hi) = if cs == ANY_OFF {
                if rep.nf.lines[s] {
                    (Some(2), None)
                } else {
                    (None, Some(-1))
                }
            } else {
                let r = rep.states[s][cs as usize];
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
            let v = rep.states[s][cs as usize].singleton().expect("singleton");
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

/// Live exception branches of a rule on a region: per branch its varying members (literal id, polynomial in t).
type LiveBranches = Vec<Vec<(u32, MPoly)>>;

pub(crate) struct Resolver<'a> {
    pub nf: &'a NfRules,
    pub opts: &'a LociOptions,
    pub nodes: u64,
    pub errors: u64,
    pub pieces: Vec<Piece>,
    pub unresolved: Vec<Unresolved>,
    pub capped: bool,
    /// Case-equation hyperplanes the current region lies on (stack).
    refs: Vec<String>,
}

enum GeV {
    True,
    False,
    Varies,
}

/// Exact emptiness of `region minus the hyperplanes`.
enum Minus {
    Empty,
    Point(Vec<i128>),
    Unknown,
}

/// A hyperplane `coef . t + c = 0` in the parameters, with its label.
type Hyper = (Vec<i128>, i128, String);

fn on_hyper(h: &Hyper, t: &[i128]) -> Option<bool> {
    Affine {
        coef: h.0.clone(),
        c: h.1,
    }
    .eval(t)
    .ok()
    .map(|v| v == 0)
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
            refs: vec![],
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

    /// An OPEN region the recursion cannot resolve (cap, overflow): reported, never dropped (audit-part-A M1).
    fn open_region(&mut self, region: Region, trail: Vec<String>, reason: String) {
        if region.feasible() == Ok(false) {
            return; // proved integer-empty
        }
        self.errors += 1;
        self.unresolved.push(Unresolved {
            region,
            equation: reason.clone(),
            reason,
            trail,
            bound: "none (no window for a capped / failed region)".into(),
            points: vec![],
            n_window: 0,
        });
    }

    /// Resolve the uncovered set of `region` w.r.t. the rules `among` (indices into nf.rules).
    pub fn resolve(&mut self, region: Region, among: &[usize], trail: Vec<String>, depth: usize) {
        self.nodes += 1;
        if self.nodes > self.opts.max_nodes || depth > self.opts.max_depth {
            self.capped = true;
            let why = if depth > self.opts.max_depth {
                format!("locus recursion depth cap {} reached", self.opts.max_depth)
            } else {
                format!("locus recursion node cap {} reached", self.opts.max_nodes)
            };
            self.open_region(region, trail, why);
            return;
        }
        let mut qcache: BTreeMap<u32, MPoly> = BTreeMap::new();
        let mut scache: BTreeMap<u32, Status> = BTreeMap::new();
        let mut alive: Vec<(usize, LiveBranches)> = vec![];
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
                            // the rule is treated as NOT covering here (conservative)
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
                            let pos = Affine {
                                coef: coef.clone(),
                                c,
                            };
                            let neg = Affine {
                                coef: coef.iter().map(|a| -a).collect(),
                                c: -c - 1,
                            };
                            let mut t = trail.clone();
                            t.push(format!("{}>=0", self.nf.lits[l as usize].text));
                            self.resolve(region.with(pos), among, t, depth + 1);
                            let mut t = trail.clone();
                            t.push(format!("{}<=-1", self.nf.lits[l as usize].text));
                            self.resolve(region.with(neg), among, t, depth + 1);
                            return;
                        }
                        // a nonlinear bound that varies: the rule is treated as not covering (conservative)
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
                let mut failed = false;
                for &l in b {
                    let q = match qcache.get(&l) {
                        Some(q) => q.clone(),
                        None => match region.lit(&self.nf.lits[l as usize].poly) {
                            Ok(q) => {
                                qcache.insert(l, q.clone());
                                q
                            }
                            Err(_) => {
                                // a member that cannot be computed may vanish anywhere: the rule is treated as
                                // NOT covering on this region (audit-part-A M1: never a live branch without member)
                                self.errors += 1;
                                failed = true;
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
                if failed || all_z {
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
            self.subtract_and_emit(region, &next_among, &eq_loci, trail, depth);
            return;
        }
        // fragile rule with the fewest live branches, preferring branches with a linear member
        let nonlin = |live: &LiveBranches| {
            live.iter()
                .filter(|a| {
                    !a.iter()
                        .any(|(_, q)| q.total_degree() == 1 || q.support().len() == 1)
                })
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
                let nl = facs
                    .iter()
                    .filter(|f| f.total_degree() > 1 && f.support().len() > 1)
                    .count();
                let key = (nl, facs.len());
                if best.as_ref().is_none_or(|b| key < b.0) {
                    best = Some((key, facs, *lid));
                }
            }
            let Some((_, facs, lid)) = best else {
                // unreachable: a live branch has at least one varying member
                self.open_region(
                    region.clone(),
                    trail.clone(),
                    "live exception branch without a varying member".into(),
                );
                continue;
            };
            let nfacs = facs.len();
            let lit_text = self.nf.lits[lid as usize].text.clone();
            for (fi, f) in facs.into_iter().enumerate() {
                let f = f.primitive().unwrap_or(f);
                if f.as_constant().is_some() || done.contains(&f) {
                    continue;
                }
                done.push(f.clone());
                let label = if nfacs == 1 {
                    lit_text.clone()
                } else {
                    format!("{lit_text} [factor {fi}: {}]", f.render(&region.names))
                };
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
                        Err(e) => self.open_region(
                            region.clone(),
                            trail.clone(),
                            format!("lattice cut on {label}: {e}"),
                        ),
                    }
                } else if f.support().len() == 1 {
                    let j = f.support()[0];
                    let u = f.univariate(j).expect("univariate");
                    match int_roots(&u) {
                        Ok(rs) => {
                            for x in rs {
                                match region.fix(j, x as i128) {
                                    Ok(sub) => {
                                        if sub.feasible().unwrap_or(true) {
                                            let mut t = trail.clone();
                                            t.push(label.clone());
                                            self.resolve(sub, &next_among, t, depth + 1);
                                        }
                                    }
                                    Err(e) => self.open_region(
                                        region.clone(),
                                        trail.clone(),
                                        format!("fixing a root of {label}: {e}"),
                                    ),
                                }
                            }
                        }
                        Err(_) => {
                            self.errors += 1;
                            self.unresolve(
                                region.clone(),
                                f,
                                label,
                                trail.clone(),
                                "integer roots of a one-parameter cut not computable".into(),
                            );
                        }
                    }
                } else {
                    self.unresolve(
                        region.clone(),
                        f,
                        label,
                        trail.clone(),
                        "nonlinear cut in two or more parameters".into(),
                    );
                }
            }
        }
    }

    fn unresolve(
        &mut self,
        region: Region,
        f: MPoly,
        label: String,
        trail: Vec<String>,
        reason: String,
    ) {
        // exact window: L1 ball around an anchor point of the region, f = 0 and uncovered
        let k = region.k;
        let radius = self.opts.window_radius[k.min(5)];
        let anchor = region
            .point(self.opts.tries)
            .ok()
            .flatten()
            .unwrap_or(vec![0; k]);
        let mut pts = vec![];
        let mut n = 0u64;
        let mut t = vec![0i128; k];
        #[allow(clippy::too_many_arguments)]
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
                if !region.contains_t(t).unwrap_or(false)
                    || f.eval(t).map(|v| v != 0).unwrap_or(true)
                {
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
        rec(
            0, radius, &mut t, &anchor, &f, &region, self.nf, &mut n, &mut pts,
        );
        self.unresolved.push(Unresolved {
            bound: format!("L1 <= {radius} around parameters {anchor:?}"),
            region,
            reason,
            equation: label,
            trail,
            points: pts,
            n_window: n,
        });
    }

    /// No rule is alive on `region` except rules whose case equation varies (`eq_loci`): the uncovered set is
    /// EXACTLY (region minus the hyperplanes of the linear ones) plus the uncovered set on each hyperplane, resolved
    /// recursively with every rule (audit-part-A C1).
    fn subtract_and_emit(
        &mut self,
        region: Region,
        among: &[usize],
        eq_loci: &[(usize, MPoly)],
        trail: Vec<String>,
        depth: usize,
    ) {
        if region.feasible() == Ok(false) {
            return;
        }
        // distinct linear forms (primitive, canonical sign) and the nonlinear equations
        let mut forms: Vec<Hyper> = vec![];
        let mut seen: Vec<MPoly> = vec![];
        let mut unsubtracted: Vec<String> = vec![];
        let mut covered_on = vec![];
        for (_, q) in eq_loci {
            let p = q.primitive().unwrap_or(q.clone());
            if seen.contains(&p) {
                continue;
            }
            seen.push(p.clone());
            if p.total_degree() == 1 {
                let (coef, c) = p.linear_form().expect("linear");
                forms.push((coef, c, p.render(&region.names)));
            } else if p.support().len() == 1 {
                // one parameter: the zero set is the finite set of integer roots, each a hyperplane t_j = x
                let j = p.support()[0];
                match int_roots(&p.univariate(j).expect("univariate")) {
                    Ok(rs) => {
                        for x in rs {
                            let mut coef = vec![0i128; region.k];
                            coef[j] = 1;
                            forms.push((coef, -(x as i128), format!("{}={x}", region.names[j])));
                        }
                    }
                    Err(e) => {
                        self.errors += 1;
                        unsubtracted.push(format!("{} (roots: {e})", p.render(&region.names)));
                    }
                }
            } else {
                unsubtracted.push(p.render(&region.names));
            }
        }
        // the uncovered part ON each hyperplane: resolved with every rule (reported as its own pieces)
        let mut minus: Vec<Hyper> = vec![];
        for (coef, c, txt) in forms {
            match region.cut(&coef, c) {
                Ok(Some(sub)) => {
                    if sub.feasible() == Ok(false) {
                        continue;
                    }
                    let (np, nu) = (self.pieces.len(), self.unresolved.len());
                    self.refs.push(format!("{txt}=0"));
                    self.resolve(sub, among, trail.clone(), depth + 1);
                    self.refs.pop();
                    covered_on.push(CoveredSubLocus {
                        equation: txt.clone(),
                        fully_covered: self.pieces.len() == np && self.unresolved.len() == nu,
                    });
                    minus.push((coef, c, txt));
                }
                Ok(None) => {} // no integer point on the hyperplane: nothing to subtract
                Err(e) => {
                    // the sub-locus cannot be parametrised: the WHOLE region stays open
                    self.open_region(
                        region.clone(),
                        trail.clone(),
                        format!("lattice cut on the case equation {txt}: {e}"),
                    );
                    return;
                }
            }
        }
        for u in &unsubtracted {
            covered_on.push(CoveredSubLocus {
                equation: u.clone(),
                fully_covered: false,
            });
        }
        // the part OFF every hyperplane: dropped only if PROVED empty
        let mut flagged = !unsubtracted.is_empty();
        let mut first: Option<Vec<i128>> = None;
        if !minus.is_empty() {
            match self.minus_point(&region, &minus) {
                Minus::Empty => return,
                Minus::Point(t) => first = Some(t),
                Minus::Unknown => flagged = true,
            }
        }
        let witness = self.witnesses(&region, &minus, first);
        let unbounded = region.unbounded().ok();
        self.pieces.push(Piece {
            region,
            trail,
            covered_on,
            minus: minus.into_iter().map(|m| m.2).collect(),
            refinements: self.refs.clone(),
            covered_part_unresolved: flagged,
            witness,
            unbounded,
        });
    }

    /// An integer point of `region` off every hyperplane, or a proof that there is none (exact sign split
    /// {f >= 1} / {f <= -1}, node-capped).
    fn minus_point(&mut self, region: &Region, minus: &[Hyper]) -> Minus {
        // cheap first: points of the region near its anchor (a found point is checked exactly)
        if let Some(t) = self.search_off(region, minus, 2) {
            return Minus::Point(t);
        }
        fn split(r: &Region, hs: &[Hyper], nodes: &mut u64, cap: u64, tries: usize) -> Minus {
            *nodes += 1;
            if *nodes > cap {
                return Minus::Unknown;
            }
            match r.feasible() {
                Ok(false) => return Minus::Empty,
                Ok(true) => {}
                Err(_) => return Minus::Unknown,
            }
            let Some(((coef, c, _), rest)) = hs.split_first() else {
                return match r.point(tries) {
                    Ok(Some(t)) => Minus::Point(t),
                    _ => Minus::Unknown,
                };
            };
            let pos = Affine {
                coef: coef.clone(),
                c: c - 1,
            };
            let neg = Affine {
                coef: coef.iter().map(|a| -a).collect(),
                c: -c - 1,
            };
            let mut unknown = false;
            for part in [r.with(pos), r.with(neg)] {
                match split(&part, rest, nodes, cap, tries) {
                    Minus::Point(t) => return Minus::Point(t),
                    Minus::Unknown => unknown = true,
                    Minus::Empty => {}
                }
            }
            if unknown {
                Minus::Unknown
            } else {
                Minus::Empty
            }
        }
        let mut nodes = 0u64;
        split(
            region,
            minus,
            &mut nodes,
            self.opts.max_split_nodes,
            self.opts.tries,
        )
    }

    /// Exact search near the region's anchor point for a parameter point off every hyperplane (L1 walk).
    fn search_off(&self, region: &Region, minus: &[Hyper], radius: i128) -> Option<Vec<i128>> {
        let off = |t: &[i128]| {
            region.contains_t(t).unwrap_or(false)
                && minus.iter().all(|h| on_hyper(h, t) == Some(false))
        };
        let base = region.point(self.opts.tries).ok()??;
        let k = region.k;
        for r in 0..=radius {
            let mut stack: Vec<(usize, i128, Vec<i128>)> = vec![(0, r, base.clone())];
            while let Some((j, left, t)) = stack.pop() {
                if j == k {
                    if left == 0 && off(&t) {
                        return Some(t);
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

    /// Witness keys of a reported piece (confirmed uncovered by the exact oracle; never used for a decision).
    fn witnesses(&self, region: &Region, minus: &[Hyper], first: Option<Vec<i128>>) -> Vec<Key> {
        let mut witness = vec![];
        let mut cands: Vec<Vec<i128>> = first.into_iter().collect();
        let mut anchors: Vec<Vec<i128>> = vec![vec![0; region.k]];
        for j in 0..region.k {
            for d in [3i128, 7, -3, -7] {
                let mut a = vec![0; region.k];
                a[j] = d;
                anchors.push(a);
            }
        }
        for a in anchors.iter().take(25) {
            if let Ok(Some(t)) = region::find_point(&region.cons, region.k, self.opts.tries, a) {
                cands.push(t);
            }
        }
        for t in cands {
            if witness.len() >= self.opts.witnesses {
                break;
            }
            if minus.iter().any(|h| on_hyper(h, &t) != Some(false)) {
                continue;
            }
            if let Ok(x) = region.at(&t) {
                let key: Option<Key> = x.iter().map(|&v| i32::try_from(v).ok()).collect();
                if let Some(key) = key
                    && !witness.contains(&key)
                    && self.nf.covered(&key).ok() == Some(false)
                {
                    witness.push(key);
                }
            }
        }
        witness
    }
}

/// Resolve every F class of a partition (each `any` slot expanded into floor / off-floor first, so every piece lies
/// on one face) and group the pieces into loci (face, cut equations). Every piece and every unresolved region is
/// kept (audit-part-A C1: no locus is dropped because sampled points were covered).
pub fn resolve_fragile_classes(
    rep: &PartitionReport,
    opts: &LociOptions,
) -> Result<LociReport, PartitionError> {
    let mut out = LociReport::default();
    type Gk = ((Vec<usize>, Vec<usize>), Vec<String>, bool);
    let mut groups: BTreeMap<Gk, Locus> = BTreeMap::new();
    let all: Vec<usize> = (0..rep.nf.rules.len()).collect();
    for c in rep.classes.iter().filter(|c| c.label == Label::Fragile) {
        // a class above `max_any_slots` is listed as a face family (outcome Incomplete), not expanded
        let Some(cells) = rep.expand(c) else { continue };
        for (face, cell) in cells {
            let region = cell_region(rep, &cell);
            let mut r = Resolver::new(&rep.nf, opts);
            r.resolve(region, &all, vec![], 0);
            out.classes_resolved += 1;
            out.exact_errors += r.errors;
            if r.capped {
                out.classes_capped += 1;
            }
            let has = !r.pieces.is_empty() || !r.unresolved.is_empty();
            out.classes_with_gaps += usize::from(has);
            for p in r.pieces {
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
                    if p.region
                        .x
                        .iter()
                        .enumerate()
                        .any(|(s, a)| lines[s] && !a.is_constant())
                    {
                        e.unbounded_in_dots = true;
                    }
                }
                e.pieces.push(p);
            }
            for u in r.unresolved {
                let mut eqs = u.trail.clone();
                eqs.push(u.equation.clone());
                let key = (face.clone(), eqs.clone(), true);
                let capped = u.n_window == 0;
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
                    proof: if capped {
                        Proof::Unresolved {
                            reason: u.reason.clone(),
                        }
                    } else {
                        Proof::Tested {
                            bound: u.bound.clone(),
                        }
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
    for (i, l) in loci.iter_mut().enumerate() {
        l.id = format!("L{}", i + 1);
    }
    out.loci = loci;
    Ok(out)
}
