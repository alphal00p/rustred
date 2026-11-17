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
//! reason (audit-part-A M1): reported as an open gap region, making the partition outcome `Incomplete`. A capped
//! cell is reported as ONE open region (the whole cell, with the number of pending regions), never as one region per
//! pending call (re-audit RA2-M5: memory bounded by the number of cells); at most `LociOptions::max_open_regions`
//! open regions are kept per cell, beyond which the whole cell is reported open.
//!
//! Overflow (re-audit RA2-C1): every complement and every coefficient operation is checked; a rule whose condition
//! cannot be decided exactly (overflow, nonlinear bound in several parameters) is treated as NOT covering the region
//! and every piece emitted below is flagged `covered_part_unresolved` (a superset of the uncovered set; the partition
//! outcome is then `Incomplete`). A piece is emitted only with a point (exact search, else the exhaustive search of
//! a bounded region): a PROVED integer-empty piece is dropped, an undecided one is flagged (re-audit RA2-M3).
//!
//! No floating point anywhere.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

pub mod contain;
pub mod region;

use std::collections::BTreeMap;

use serde_json::{Value as Json, json};

use crate::key::Key;
use crate::partition::classes::{Label, Status};
use crate::partition::exact::{Affine, ExactError, MPoly, int_roots, linear_factors, neg};
use crate::partition::nf::NfRules;
use crate::partition::{
    ANY_OFF, CellState, FaceKey, PartitionError, PartitionReport, Proof, WorkMeter,
};
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
    /// Wall-clock deadline (set from the partition's `Budget::wall`): regions reached after it are reported as
    /// OPEN unresolved regions (audit-part-B M3).
    pub deadline: Option<std::time::Instant>,
    /// Open regions kept per cell (beyond: the whole cell is reported as one open region; re-audit RA2-M5).
    pub max_open_regions: usize,
    /// Values tried by the exhaustive integer search that decides whether a piece without a found point is empty
    /// (re-audit RA2-M3; beyond: the piece is flagged).
    pub max_enumeration: u64,
    /// Make implicit equalities of a region explicit before resolving it ([`region::Region::reduce_equalities`]):
    /// needed for AFFINE pieces (a lattice line / plane cut out of a face by equations). Off by default.
    pub reduce_equalities: bool,
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
            deadline: None,
            max_open_regions: 16,
            max_enumeration: 100_000,
            reduce_equalities: false,
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
    /// Cells not resolved because the call's work budget was exhausted (re-audit RA2-M5); the first
    /// `LociOptions::max_open_regions` are listed as open regions (whole cells), the outcome is aborted.
    pub cells_unresolved_budget: usize,
    /// Pieces dropped because their region was PROVED integer-empty (re-audit RA2-M3).
    pub pieces_proved_empty: usize,
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
    /// Pieces flagged `covered_part_unresolved` (supersets of their uncovered set).
    pub fn n_flagged_pieces(&self) -> usize {
        self.loci
            .iter()
            .map(|l| {
                l.pieces
                    .iter()
                    .filter(|p| p.covered_part_unresolved)
                    .count()
            })
            .sum()
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
        .filter(|&s| {
            cell[s] == ANY_OFF || rep.states()[s][usize::from(cell[s])].singleton().is_none()
        })
        .collect();
    let k = free.len();
    for (s, &cs) in cell.iter().enumerate().take(n) {
        if let Some(j) = free.iter().position(|&f| f == s) {
            x.push(Affine::var(k, j));
            names.push(format!("n{s}"));
            let (lo, hi) = if cs == ANY_OFF {
                if rep.nf().lines[s] {
                    (Some(2), None)
                } else {
                    (None, Some(-1))
                }
            } else {
                let r = rep.states()[s][usize::from(cs)];
                (r.lo, r.hi)
            };
            // i64 -> i128 and the negation of an i64 never overflow in i128
            if let Some(l) = lo {
                cons.push(Affine::unit(k, j, 1, i128::from(l).saturating_neg()));
            }
            if let Some(h) = hi {
                cons.push(Affine::unit(k, j, -1, i128::from(h)));
            }
        } else {
            let v = rep.states()[s][usize::from(cs)].singleton().unwrap_or(0);
            x.push(Affine::constant(k, i128::from(v)));
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
    if let [j] = *vars.as_slice() {
        let Some(u) = q.univariate(j) else {
            return (Status::V, true);
        };
        let roots = match int_roots(&u) {
            Ok(v) => v,
            Err(_) => return (Status::V, true),
        };
        for x in roots {
            match r.fix(j, x).and_then(|s| s.feasible()) {
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
    /// The call's ONE work budget (re-audit RA2-M5): one unit per resolve call and per emptiness split node.
    meter: &'a WorkMeter,
    pub nodes: u64,
    pub errors: u64,
    pub pieces: Vec<Piece>,
    pub unresolved: Vec<Unresolved>,
    pub capped: bool,
    /// Why the cell was capped (first reason).
    cap_reason: Option<String>,
    /// Regions not resolved after the cap (counted, never materialised).
    pending: u64,
    /// Open regions beyond `max_open_regions` (counted; the whole cell is then reported open).
    dropped_open: u64,
    /// Pieces dropped as PROVED integer-empty.
    proved_empty: usize,
    /// Case-equation hyperplanes the current region lies on (stack).
    refs: Vec<String>,
}

enum GeV {
    True,
    False,
    Varies,
    /// The sign cannot be decided exactly (overflow, nonlinear in several parameters): the rule is treated as NOT
    /// covering here and the pieces below are flagged as supersets.
    Undecided,
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
    pub fn new(nf: &'a NfRules, opts: &'a LociOptions, meter: &'a WorkMeter) -> Self {
        Resolver {
            nf,
            opts,
            meter,
            nodes: 0,
            errors: 0,
            pieces: vec![],
            unresolved: vec![],
            capped: false,
            cap_reason: None,
            pending: 0,
            dropped_open: 0,
            proved_empty: 0,
            refs: vec![],
        }
    }

    fn status(&mut self, r: &Region, q: &MPoly) -> Status {
        let (st, err) = lit_status(r, q);
        if err {
            self.errors = self.errors.saturating_add(1);
        }
        st
    }

    fn ge(&mut self, r: &Region, q: &MPoly) -> GeV {
        if let Some(c) = q.as_constant() {
            return if c >= 0 { GeV::True } else { GeV::False };
        }
        if let Some((coef, c)) = q.linear_form() {
            let pos = Affine { coef, c };
            // q <= -1, checked (re-audit RA2-C1: an unchecked negation of a -2^127 coefficient kept the wrong sign)
            let Ok(negs) = pos.complement() else {
                self.errors = self.errors.saturating_add(1);
                return GeV::Undecided;
            };
            let can_neg = r.with(negs).feasible().unwrap_or(true);
            let can_pos = r.with(pos).feasible().unwrap_or(true);
            return match (can_pos, can_neg) {
                (true, false) => GeV::True,
                (false, true) => GeV::False,
                (false, false) => GeV::False, // the region is empty (both halves refuted): nothing to cover
                (true, true) => GeV::Varies,
            };
        }
        let vars = q.support();
        if let [j] = *vars.as_slice()
            && let Some(u) = q.univariate(j)
        {
            // one parameter, nonlinear: decided exactly on the projected range of t_j when its sign is constant
            if let Ok((lo, hi)) = r.range_of(j) {
                if let (Some(l), Some(h)) = (lo, hi)
                    && l > h
                {
                    return GeV::False; // empty region: nothing to cover
                }
                if let Some(ge) = crate::partition::exact::univ_ge_on(&u, lo, hi, 400_000) {
                    return if ge { GeV::True } else { GeV::False };
                }
            }
        }
        GeV::Undecided
    }

    /// Stop resolving this cell (cap / budget): the region is counted as pending, never materialised.
    fn cap(&mut self, why: String) {
        self.capped = true;
        self.pending = self.pending.saturating_add(1);
        if self.cap_reason.is_none() {
            self.cap_reason = Some(why);
        }
    }

    /// An OPEN region the recursion cannot resolve (overflow, failed cut): reported, never dropped (audit-part-A M1);
    /// at most `max_open_regions` are kept, beyond which the whole cell is reported open (re-audit RA2-M5).
    fn open_region(&mut self, region: Region, trail: Vec<String>, reason: String) {
        if region.feasible() == Ok(false) {
            return; // proved integer-empty
        }
        self.errors = self.errors.saturating_add(1);
        if self.unresolved.len() >= self.opts.max_open_regions {
            self.dropped_open = self.dropped_open.saturating_add(1);
            return;
        }
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

    /// Resolve the uncovered set of `region` w.r.t. the rules `among` (indices into nf.rules). `superset`: a rule was
    /// treated as not covering without proof on the way here (the pieces below are flagged).
    pub fn resolve(
        &mut self,
        region: Region,
        among: &[usize],
        trail: Vec<String>,
        depth: usize,
        superset: bool,
    ) {
        self.nodes = self.nodes.saturating_add(1);
        if self.capped {
            self.pending = self.pending.saturating_add(1);
            return;
        }
        // one unit per node plus the Fourier-Motzkin / point-search work done since the last node (audit-part3 m4)
        if !self.meter.tick(1u64.saturating_add(region::take_work())) {
            let why = self
                .meter
                .reason()
                .unwrap_or_else(|| "work budget exhausted".into());
            self.cap(why);
            return;
        }
        let late = self
            .opts
            .deadline
            .is_some_and(|d| std::time::Instant::now() > d);
        if self.nodes > self.opts.max_nodes || depth > self.opts.max_depth || late {
            let why = if late {
                "wall budget reached before this region was resolved".to_string()
            } else if depth > self.opts.max_depth {
                format!("locus recursion depth cap {} reached", self.opts.max_depth)
            } else {
                format!("locus recursion node cap {} reached", self.opts.max_nodes)
            };
            self.cap(why);
            return;
        }
        let region = if self.opts.reduce_equalities {
            match region.reduce_equalities() {
                Ok(Some(r)) => r,
                Ok(None) => return, // no integer point: nothing uncovered here
                Err(e) => {
                    self.errors = self.errors.saturating_add(1);
                    self.open_region(region, trail, format!("equality reduction failed: {e}"));
                    return;
                }
            }
        } else {
            region
        };
        let d1 = depth.saturating_add(1);
        let mut superset = superset;
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
                            // the rule is treated as NOT covering here (conservative: pieces below are supersets)
                            self.errors = self.errors.saturating_add(1);
                            superset = true;
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
                        self.errors = self.errors.saturating_add(1);
                        superset = true;
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
                    GeV::Undecided => {
                        // cannot be decided exactly: treated as not covering (the pieces below are supersets)
                        superset = true;
                        dead = true;
                        break;
                    }
                    GeV::Varies => {
                        // split the region exactly on the bound: q >= 0 and q <= -1 (checked complement)
                        if let Some((coef, c)) = q.linear_form() {
                            let pos = Affine { coef, c };
                            match pos.complement() {
                                Ok(negs) => {
                                    let txt = &self.nf.lits[l as usize].text;
                                    let mut t = trail.clone();
                                    t.push(format!("{txt}>=0"));
                                    let mut t2 = trail.clone();
                                    t2.push(format!("{txt}<=-1"));
                                    self.resolve(region.with(pos), among, t, d1, superset);
                                    self.resolve(region.with(negs), among, t2, d1, superset);
                                    return;
                                }
                                Err(_) => {
                                    self.errors = self.errors.saturating_add(1);
                                    superset = true;
                                    dead = true;
                                    break;
                                }
                            }
                        }
                        // unreachable (Varies is only returned for linear bounds): conservative
                        superset = true;
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
                                self.errors = self.errors.saturating_add(1);
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
                if failed {
                    superset = true;
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
            self.subtract_and_emit(region, &next_among, &eq_loci, trail, depth, superset);
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
                                self.resolve(sub, &next_among, t, d1, superset);
                            }
                        },
                        Ok(None) => {}
                        Err(e) => self.open_region(
                            region.clone(),
                            trail.clone(),
                            format!("lattice cut on {label}: {e}"),
                        ),
                    }
                } else if let [j] = *f.support().as_slice() {
                    let roots = f
                        .univariate(j)
                        .ok_or(ExactError::TooLarge("not univariate"))
                        .and_then(|u| int_roots(&u));
                    match roots {
                        Ok(rs) => {
                            for x in rs {
                                match region.fix(j, x) {
                                    Ok(sub) => {
                                        if sub.feasible().unwrap_or(true) {
                                            let mut t = trail.clone();
                                            t.push(label.clone());
                                            self.resolve(sub, &next_among, t, d1, superset);
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
                            self.errors = self.errors.saturating_add(1);
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
        if self.unresolved.len() >= self.opts.max_open_regions {
            self.dropped_open = self.dropped_open.saturating_add(1);
            return;
        }
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
        // BUDGETED (audit-part3 M2 / M3 shape, found by the fuzz at arity 20): the L1 ball has ~D(k, radius) points (k
        // parameters); every visited node costs one unit of the call's meter and at most `max_enumeration` nodes are
        // visited; a stopped window is reported as such (never "tested to the bound")
        struct Walk<'w> {
            meter: &'w WorkMeter,
            left: u64,
            stopped: bool,
        }
        let mut walk = Walk {
            meter: self.meter,
            left: self.opts.max_enumeration,
            stopped: false,
        };
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
            w: &mut Walk<'_>,
        ) {
            if w.stopped {
                return;
            }
            if w.left == 0 || !w.meter.tick(1) {
                w.stopped = true;
                return;
            }
            w.left = w.left.saturating_sub(1);
            if j == t.len() {
                *n = n.saturating_add(1);
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
            let Some(nb) = budget.checked_neg() else {
                return;
            };
            for d in nb..=budget {
                let Some(tj) = anchor[j].checked_add(i128::from(d)) else {
                    continue;
                };
                t[j] = tj;
                let left = budget.saturating_sub(d.saturating_abs());
                rec(
                    j.saturating_add(1),
                    left,
                    t,
                    anchor,
                    f,
                    region,
                    nf,
                    n,
                    pts,
                    w,
                );
                if w.stopped {
                    return;
                }
            }
        }
        rec(
            0, radius, &mut t, &anchor, &f, &region, self.nf, &mut n, &mut pts, &mut walk,
        );
        let stopped = walk.stopped;
        self.unresolved.push(Unresolved {
            bound: if stopped {
                format!(
                    "window L1 <= {radius} around parameters {anchor:?} STOPPED after {n} points (work budget / max_enumeration)"
                )
            } else {
                format!("L1 <= {radius} around parameters {anchor:?}")
            },
            region,
            reason,
            equation: label,
            trail,
            points: pts,
            // a stopped window is not a window enumerated to its bound: reported as an open (Unresolved) region
            n_window: if stopped { 0 } else { n },
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
        superset: bool,
    ) {
        if region.feasible() == Ok(false) {
            return;
        }
        let d1 = depth.saturating_add(1);
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
                let Some((coef, c)) = p.linear_form() else {
                    unsubtracted.push(p.render(&region.names));
                    continue;
                };
                forms.push((coef, c, p.render(&region.names)));
            } else if let [j] = *p.support().as_slice() {
                // one parameter: the zero set is the finite set of integer roots, each a hyperplane t_j = x
                let roots = p
                    .univariate(j)
                    .ok_or(ExactError::TooLarge("not univariate"))
                    .and_then(|u| int_roots(&u));
                let mut ok = true;
                if let Ok(rs) = &roots {
                    for &x in rs {
                        match neg(x) {
                            Ok(nx) => forms.push((
                                Affine::unit(region.k, j, 1, 0).coef,
                                nx,
                                format!("{}={x}", region.names[j]),
                            )),
                            Err(_) => ok = false,
                        }
                    }
                }
                match roots {
                    Ok(_) if ok => {}
                    Ok(_) => {
                        self.errors = self.errors.saturating_add(1);
                        unsubtracted.push(format!("{} (root overflow)", p.render(&region.names)));
                    }
                    Err(e) => {
                        self.errors = self.errors.saturating_add(1);
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
                    self.resolve(sub, among, trail.clone(), d1, superset);
                    self.refs.pop();
                    covered_on.push(CoveredSubLocus {
                        equation: txt.clone(),
                        fully_covered: self.pieces.len() == np
                            && self.unresolved.len() == nu
                            && !self.capped,
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
        // the part OFF every hyperplane: dropped only if PROVED empty; emitted only with a point or flagged
        // (re-audit RA2-M3: Fourier-Motzkin with rounding is not an integer-emptiness decision)
        let mut flagged = superset || !unsubtracted.is_empty();
        let first: Option<Vec<i128>> = if minus.is_empty() {
            match region.decide_point(self.opts.tries, self.opts.max_enumeration) {
                Ok(Some(Some(t))) => Some(t),
                Ok(Some(None)) => {
                    self.proved_empty = self.proved_empty.saturating_add(1);
                    return; // PROVED integer-empty
                }
                Ok(None) | Err(_) => {
                    flagged = true; // integer emptiness undecided: a superset
                    None
                }
            }
        } else {
            match self.minus_point(&region, &minus) {
                Minus::Empty => {
                    self.proved_empty = self.proved_empty.saturating_add(1);
                    return;
                }
                Minus::Point(t) => Some(t),
                Minus::Unknown => {
                    flagged = true;
                    None
                }
            }
        };
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
    /// {f >= 1} / {f <= -1}, node-capped; a leaf without a found point is decided by the exhaustive search).
    fn minus_point(&mut self, region: &Region, minus: &[Hyper]) -> Minus {
        // cheap first: points of the region near its anchor (a found point is checked exactly)
        if let Some(t) = self.search_off(region, minus, 2) {
            return Minus::Point(t);
        }
        struct Split<'m> {
            nodes: u64,
            cap: u64,
            tries: usize,
            enumeration: u64,
            meter: &'m WorkMeter,
        }
        fn split(r: &Region, hs: &[Hyper], sp: &mut Split<'_>) -> Minus {
            sp.nodes = sp.nodes.saturating_add(1);
            if sp.nodes > sp.cap || !sp.meter.tick(1u64.saturating_add(region::take_work())) {
                return Minus::Unknown;
            }
            match r.feasible() {
                Ok(false) => return Minus::Empty,
                Ok(true) => {}
                Err(_) => return Minus::Unknown,
            }
            let Some(((coef, c, _), rest)) = hs.split_first() else {
                return match r.decide_point(sp.tries, sp.enumeration) {
                    Ok(Some(Some(t))) => Minus::Point(t),
                    Ok(Some(None)) => Minus::Empty,
                    _ => Minus::Unknown,
                };
            };
            let Ok((pos, negs)) = region::off_sides(coef, *c) else {
                return Minus::Unknown;
            };
            let mut unknown = false;
            for part in [r.with(pos), r.with(negs)] {
                match split(&part, rest, sp) {
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
        let mut sp = Split {
            nodes: 0,
            cap: self.opts.max_split_nodes,
            tries: self.opts.tries,
            enumeration: self.opts.max_enumeration,
            meter: self.meter,
        };
        split(region, minus, &mut sp)
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
                let nl = left.checked_neg()?;
                for dlt in nl..=left {
                    let mut t2 = t.clone();
                    let Some(v) = t2[j].checked_add(dlt) else {
                        continue;
                    };
                    t2[j] = v;
                    stack.push((
                        j.saturating_add(1),
                        left.saturating_sub(dlt.saturating_abs()),
                        t2,
                    ));
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

/// The whole cell as ONE open region (a capped / budget-stopped cell; re-audit RA2-M5).
fn open_cell(region: Region, reason: String) -> Unresolved {
    Unresolved {
        region,
        equation: reason.clone(),
        reason,
        trail: vec![],
        bound: "none (the whole cell is open)".into(),
        points: vec![],
        n_window: 0,
    }
}

/// Resolve every F class of a partition (each `any` slot expanded into floor / off-floor first, so every piece lies
/// on one face) and group the pieces into loci (face, cut equations). Every piece and every unresolved region is
/// kept (audit-part-A C1: no locus is dropped because sampled points were covered). Unlimited work budget (the
/// per-cell caps of `opts` apply).
pub fn resolve_fragile_classes(
    rep: &PartitionReport,
    opts: &LociOptions,
) -> Result<LociReport, PartitionError> {
    let meter = WorkMeter::new(None, opts.deadline);
    resolve_fragile_classes_with(rep, opts, &meter)
}

/// [`resolve_fragile_classes`] charged to the call's ONE work budget `meter` (re-audit RA2-M5): once it is exhausted
/// no further cell is resolved (counted in `cells_unresolved_budget`, the first `max_open_regions` listed as open
/// whole-cell regions); a capped cell is ONE open region.
pub fn resolve_fragile_classes_with(
    rep: &PartitionReport,
    opts: &LociOptions,
    meter: &WorkMeter,
) -> Result<LociReport, PartitionError> {
    let mut out = LociReport::default();
    type Gk = ((Vec<usize>, Vec<usize>), Vec<String>, bool);
    let mut groups: BTreeMap<Gk, Locus> = BTreeMap::new();
    let all: Vec<usize> = (0..rep.nf().rules.len()).collect();
    let mut budget_cells: Vec<(FaceKey, Unresolved)> = vec![];
    let _ = region::take_work(); // leftovers of earlier calls on this thread are not this call's work
    for c in rep.classes().iter().filter(|c| c.label == Label::Fragile) {
        if meter.exhausted() {
            // count the class's cells as unresolved without expanding them (the run is aborted anyway)
            out.cells_unresolved_budget = out.cells_unresolved_budget.saturating_add(1);
            continue;
        }
        // a class above `max_any_slots` is listed as a face family (outcome Incomplete), not expanded; the
        // expansion is charged to the meter (audit-part3 M2)
        let Some(cells) = rep.expand_with(c, meter) else {
            continue;
        };
        for (face, cell) in cells {
            let region = cell_region(rep, &cell);
            if meter.exhausted() {
                out.cells_unresolved_budget = out.cells_unresolved_budget.saturating_add(1);
                if budget_cells.len() < opts.max_open_regions {
                    let why = format!(
                        "not resolved: {}",
                        meter
                            .reason()
                            .unwrap_or_else(|| "work budget exhausted".into())
                    );
                    budget_cells.push((face, open_cell(region, why)));
                }
                continue;
            }
            let mut r = Resolver::new(rep.nf(), opts, meter);
            r.resolve(region.clone(), &all, vec![], 0, false);
            out.classes_resolved = out.classes_resolved.saturating_add(1);
            out.exact_errors = out.exact_errors.saturating_add(r.errors);
            out.pieces_proved_empty = out.pieces_proved_empty.saturating_add(r.proved_empty);
            if r.capped || r.dropped_open > 0 {
                // ONE open region for the whole cell, with the summary (re-audit RA2-M5)
                let mut why = vec![];
                if r.capped {
                    out.classes_capped = out.classes_capped.saturating_add(1);
                    why.push(format!(
                        "{}: {} region(s) pending",
                        r.cap_reason.clone().unwrap_or_default(),
                        r.pending
                    ));
                }
                if r.dropped_open > 0 {
                    why.push(format!(
                        "{} open region(s) beyond max_open_regions = {}",
                        r.dropped_open, opts.max_open_regions
                    ));
                }
                r.unresolved.push(open_cell(
                    region,
                    format!("cell resolution capped ({})", why.join("; ")),
                ));
            }
            let has = !r.pieces.is_empty() || !r.unresolved.is_empty();
            out.classes_with_gaps = out.classes_with_gaps.saturating_add(usize::from(has));
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
                e.n_classes = e.n_classes.saturating_add(1);
                e.dim = e.dim.max(p.region.k);
                for w in &p.witness {
                    if e.witness.len() < 6 && !e.witness.contains(w) {
                        e.witness.push(w.clone());
                    }
                }
                if p.unbounded == Some(true) {
                    let lines = &rep.nf().lines;
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
                push_unresolved(&mut groups, face.clone(), u);
            }
        }
    }
    for (face, u) in budget_cells {
        push_unresolved(&mut groups, face, u);
    }
    let mut loci: Vec<Locus> = groups.into_values().collect();
    for (i, l) in loci.iter_mut().enumerate() {
        l.id = format!("L{}", i.saturating_add(1));
    }
    out.loci = loci;
    Ok(out)
}

type GroupKey = ((Vec<usize>, Vec<usize>), Vec<String>, bool);

fn push_unresolved(
    groups: &mut BTreeMap<GroupKey, Locus>,
    face: (Vec<usize>, Vec<usize>),
    u: Unresolved,
) {
    let mut eqs = u.trail.clone();
    eqs.push(u.equation.clone());
    let key = (face.clone(), eqs.clone(), true);
    let capped = u.n_window == 0;
    let e = groups.entry(key).or_insert_with(|| Locus {
        id: String::new(),
        face,
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
    e.n_classes = e.n_classes.saturating_add(1);
    for w in &u.points {
        if e.witness.len() < 6 && !e.witness.contains(w) {
            e.witness.push(w.clone());
        }
    }
    e.unresolved_parts.push(u);
}
