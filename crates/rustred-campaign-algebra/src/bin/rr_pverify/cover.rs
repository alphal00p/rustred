//! COVERAGE by face recursion (the verifier's own decision procedure; it shares nothing with a partition of the
//! producer). Let U(F) be the set of region keys of an affine face F that NO rule selects (union semantics: a key some
//! rule selects is selected at first match by the first such rule). For a rule whose fixed slots and case equations
//! hold identically on F, U(F) lies inside the union over its exception branches B of F n Z(B), and Z(B) lies inside
//! the zero set of ANY one of B's members. So
//!     U(F) = union over live branches B of U(F n H_B),   H_B = zero set of one member of B,
//! with dim(F n H_B) < dim(F). The recursion is exact: every reported key is re-checked by the exact first match, and
//! every uncovered key is reached (the inclusions above). Leaves: a rule with no live branch (F covered), an empty face
//! (exact Fourier-Motzkin infeasibility in the region), a face with no generically applicable rule (its lattice points
//! are enumerated and dispatched one by one), a branch with only nonlinear members (its member is factored with
//! Symbolica; linear factors give faces, the integer zeros of a nonlinear factor are enumerated in the bounded region).
//! Memoised on the canonical face.

use std::collections::HashMap;
use std::rc::Rc;

use symbolica::domains::rational::{Q, Rational};
use symbolica::poly::factor::Factorize;

use crate::dispatch;
use crate::expr::{QPoly, poly_vars, to_zpoly, zpoly_to_q};
use crate::face::{Face, Region};
use crate::rat::{Lin, Rat};
use crate::table::{Member, Table};

#[derive(Default, Debug, Clone)]
pub struct Outcome {
    pub uncovered: std::collections::BTreeSet<Vec<i64>>,
    /// indices into `Cover::undecided_log` (shared, so a DAG of faces never copies messages)
    pub undecided: std::collections::BTreeSet<usize>,
}

#[derive(Default, Debug, Clone, serde::Serialize)]
pub struct Stats {
    pub faces: u64,
    pub memo_hits: u64,
    pub empty: u64,
    pub covered_by_one_rule: u64,
    pub split: u64,
    pub orphans: u64,
    pub orphan_points: u64,
    pub nonlinear_branches: u64,
    pub nonlinear_points_tested: u64,
    pub nonlinear_zeros: u64,
    pub max_depth: usize,
    pub points_dispatched: u64,
    pub implicit_pins: u64,
    pub univariate_without_integer_root: u64,
    pub bivariate_quadratic_exact: u64,
}

pub struct Cover<'a> {
    pub t: &'a Table,
    pub region: Region,
    pub n: usize,
    memo: HashMap<Face, Rc<Outcome>>,
    /// per rule: per branch: per member: precomputed nonlinear polynomial (None = affine member)
    npolys: Vec<Vec<Vec<Option<QPoly>>>>,
    pub stats: Stats,
    pub orphan_cap: usize,
    pub zero_enum_cap: u128,
    pub fixed_lists: Vec<Vec<(usize, i64)>>,
    /// per rule: why it applied (for the report): rule chosen at each covered face
    pub used_rules: HashMap<usize, u64>,
    pub undecided_log: Vec<String>,
    /// bounded runs: every orphan face, its lattice points and the rules serving them
    pub orphan_log: Vec<serde_json::Value>,
    /// pin coordinates forced to their chart bound (exact; on by default)
    pub close_implicit: bool,
}

enum BranchState {
    Dead,
    AllZero,
    /// the face cut by EVERY affine member of the branch (it contains the branch's zero set on F)
    Sub(Face),
    NonLin(Vec<QPoly>),
}

pub fn lin_to_qpoly(l: &Lin, template: &QPoly, n: usize) -> QPoly {
    let mut p = template.zero();
    let mut e = vec![0u16; n + 1];
    if !l.c.is_zero() {
        p.append_monomial(Rational::from((l.c.n, l.c.d)), &e);
    }
    for i in (0..n).rev() {
        // ascending lex order of exponent vectors: higher variable index first is NOT needed; append_monomial sorts
        if !l.a[i].is_zero() {
            e[i] = 1;
            p.append_monomial(Rational::from((l.a[i].n, l.a[i].d)), &e);
            e[i] = 0;
        }
    }
    p
}

pub fn total_degree(p: &QPoly) -> u32 {
    p.exponents_iter()
        .map(|e| e.iter().map(|&x| x as u32).sum::<u32>())
        .max()
        .unwrap_or(0)
}

/// Affine form of a polynomial of total degree <= 1 that does not use `d`.
pub fn qpoly_to_lin(p: &QPoly, n: usize) -> Option<Lin> {
    let mut l = Lin::zero(n);
    for (c, e) in p.coefficients.iter().zip(p.exponents_iter()) {
        let deg: u32 = e.iter().map(|&x| x as u32).sum();
        let num = c.numerator().to_i64()? as i128;
        let den = c.denominator().to_i64()? as i128;
        let r = Rat::new(num, den);
        if deg == 0 {
            l.c = r;
        } else if deg == 1 {
            let v = e.iter().position(|&x| x == 1)?;
            if v >= n {
                return None;
            }
            l.a[v] = r;
        } else {
            return None;
        }
    }
    Some(l)
}

pub fn qpoly_vars(p: &QPoly) -> Vec<usize> {
    let nv = p.nvars();
    (0..nv)
        .filter(|&v| p.exponents_iter().any(|e| e[v] > 0))
        .collect()
}

/// Exact value of a polynomial at an integer assignment of its variables (others 0).
pub fn qpoly_eval(p: &QPoly, assign: &[i64]) -> Rational {
    let mut s = Rational::zero();
    for (c, e) in p.coefficients.iter().zip(p.exponents_iter()) {
        let mut t = c.clone();
        for (v, &x) in e.iter().enumerate() {
            if x > 0 {
                t *= Rational::from(assign[v]).pow(x as u64);
            }
        }
        s += t;
    }
    s
}

impl<'a> Cover<'a> {
    pub fn new(t: &'a Table, r: Option<i128>) -> Result<Cover<'a>, String> {
        let n = t.n;
        let vars = poly_vars(n);
        let mut npolys = vec![];
        for rule in &t.rules {
            let mut bv = vec![];
            for br in &rule.exceptions {
                let mut mv = vec![];
                for m in br {
                    if m.lin.is_some() {
                        mv.push(None);
                    } else {
                        if m.uses_d {
                            return Err(format!(
                                "rule {}: exception member uses d (not handled): {}",
                                rule.pos, m.text
                            ));
                        }
                        let z = to_zpoly(&m.expr.root, n, &vars)
                            .map_err(|e| format!("rule {} member {}: {e}", rule.pos, m.text))?;
                        mv.push(Some(zpoly_to_q(&z)));
                    }
                }
                bv.push(mv);
            }
            for e in &rule.equations {
                if e.lin.is_none() {
                    return Err(format!(
                        "rule {}: nonlinear case equation (not handled)",
                        rule.pos
                    ));
                }
            }
            npolys.push(bv);
        }
        let fixed_lists = t
            .rules
            .iter()
            .map(|r| {
                r.fixed
                    .iter()
                    .enumerate()
                    .filter_map(|(i, f)| f.map(|x| (i, x)))
                    .collect()
            })
            .collect();
        Ok(Cover {
            t,
            region: Region {
                mask: t.mask.clone(),
                r,
            },
            n,
            memo: HashMap::new(),
            npolys,
            stats: Stats::default(),
            orphan_cap: 3_000_000,
            zero_enum_cap: 20_000_000,
            fixed_lists,
            used_rules: HashMap::new(),
            undecided_log: vec![],
            orphan_log: vec![],
            close_implicit: true,
        })
    }

    fn restrict_poly(&self, p: &QPoly, f: &Face) -> QPoly {
        let mut q = p.clone();
        for (v, e) in &f.rows {
            if q.degree(*v) > 0 {
                let ep = lin_to_qpoly(e, &q, self.n);
                q = q.replace_with_poly(*v, &ep);
            }
        }
        q
    }

    fn branch_state(&self, f: &Face, br: &[Member], np: &[Option<QPoly>]) -> BranchState {
        let mut lins: Vec<Lin> = vec![];
        let mut nls: Vec<QPoly> = vec![];
        for (m, p) in br.iter().zip(np) {
            match (&m.lin, p) {
                (Some(l), _) => {
                    let r = f.subst(l);
                    if r.is_const() {
                        if !r.c.is_zero() {
                            return BranchState::Dead;
                        }
                    } else {
                        lins.push(r);
                    }
                }
                (None, Some(p)) => {
                    let q = self.restrict_poly(p, f);
                    if q.is_zero() {
                        continue;
                    }
                    if q.is_constant() {
                        return BranchState::Dead;
                    }
                    if total_degree(&q) <= 1
                        && let Some(l) = qpoly_to_lin(&q, self.n)
                    {
                        lins.push(l);
                        continue;
                    }
                    nls.push(q);
                }
                (None, None) => unreachable!(),
            }
        }
        if !lins.is_empty() {
            let mut g = f.clone();
            for l in &lins {
                match g.add_eq(l) {
                    None => return BranchState::Dead,
                    Some(h) => g = h,
                }
            }
            if !g.feasible(&self.region) {
                return BranchState::Dead;
            }
            return BranchState::Sub(g);
        }
        if nls.is_empty() {
            return BranchState::AllZero;
        }
        BranchState::NonLin(nls)
    }

    /// Exact uncovered keys of face `f` in the region.
    pub fn solve(&mut self, f: &Face, depth: usize) -> Rc<Outcome> {
        if let Some(o) = self.memo.get(f) {
            self.stats.memo_hits += 1;
            return o.clone();
        }
        self.stats.faces += 1;
        self.stats.max_depth = self.stats.max_depth.max(depth);
        let out = Rc::new(self.solve_inner(f, depth));
        self.memo.insert(f.clone(), out.clone());
        out
    }

    fn solve_inner(&mut self, f: &Face, depth: usize) -> Outcome {
        if !f.feasible(&self.region) {
            self.stats.empty += 1;
            return Outcome::default();
        }
        if self.close_implicit {
            let g = f.close_implicit(&self.region);
            if &g != f {
                self.stats.implicit_pins += 1;
                let o = self.solve(&g, depth);
                return (*o).clone();
            }
        }
        let n = self.n;
        let pins = f.pins(n);
        // candidate rules
        let mut best: Option<(usize, (usize, usize), Vec<BranchState>)> = None;
        'rules: for (ri, rule) in self.t.rules.iter().enumerate() {
            if rule.skip {
                continue;
            }
            for &(j, x) in &self.fixed_lists[ri] {
                if pins[j] != Some(Rat::int(x as i128)) {
                    continue 'rules;
                }
            }
            for e in &rule.equations {
                let l = e.lin.as_ref().expect("checked affine");
                let r = f.subst(l);
                if !(r.is_const() && r.c.is_zero()) {
                    continue 'rules;
                }
            }
            let mut states = vec![];
            let (mut nnl, mut nlin) = (0usize, 0usize);
            for (bi, br) in rule.exceptions.iter().enumerate() {
                let st = self.branch_state(f, br, &self.npolys[ri][bi]);
                match st {
                    BranchState::AllZero => continue 'rules,
                    BranchState::Dead => continue,
                    BranchState::Sub(_) => nlin += 1,
                    BranchState::NonLin(_) => nnl += 1,
                }
                states.push(st);
            }
            let cost = (nnl, nlin);
            if cost == (0, 0) {
                self.stats.covered_by_one_rule += 1;
                *self.used_rules.entry(ri).or_default() += 1;
                return Outcome::default();
            }
            if best.as_ref().is_none_or(|(_, c, _)| cost < *c) {
                best = Some((ri, cost, states));
            }
        }
        let Some((ri, _, states)) = best else {
            return self.orphan(f);
        };
        *self.used_rules.entry(ri).or_default() += 1;
        self.stats.split += 1;
        let mut out = Outcome::default();
        for st in states {
            match st {
                BranchState::Sub(g) => {
                    let o = self.solve(&g, depth + 1);
                    merge(&mut out, &o);
                }
                BranchState::NonLin(polys) => {
                    self.stats.nonlinear_branches += 1;
                    match self.nonlinear_faces(f, &polys) {
                        Ok(faces) => {
                            for g in faces {
                                let o = self.solve(&g, depth + 1);
                                merge(&mut out, &o);
                            }
                        }
                        Err(e) => {
                            self.undecided_log
                                .push(format!("rule {ri}: {e} on face [{}]", f.describe()));
                            out.undecided.insert(self.undecided_log.len() - 1);
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    fn orphan(&mut self, f: &Face) -> Outcome {
        self.stats.orphans += 1;
        let mut out = Outcome::default();
        let mut region = self.region.clone();
        if region.r.is_none() {
            // a face with finitely many chart points is enumerated with its own exact D + r bound
            match f.chart_dr_bound(&region.mask) {
                Some(b) => region.r = Some(b),
                None => {
                    self.undecided_log.push(format!(
                        "no generically applicable rule on face [{}] (infinite in the chart: not enumerated)",
                        f.describe()
                    ));
                    out.undecided.insert(self.undecided_log.len() - 1);
                    return out;
                }
            }
        }
        match f.lattice_points(&region, self.orphan_cap) {
            Ok(pts) => {
                let mut hist: std::collections::BTreeMap<String, u64> = Default::default();
                let npts = pts.len();
                let mut max_dr = 0i128;
                for k in pts {
                    self.stats.orphan_points += 1;
                    self.stats.points_dispatched += 1;
                    max_dr = max_dr.max(dispatch::dr(&self.region.mask, &k));
                    match dispatch::first_match(self.t, &k) {
                        None => {
                            out.uncovered.insert(k);
                            *hist.entry("none".into()).or_default() += 1;
                        }
                        Some(i) => *hist.entry(i.to_string()).or_default() += 1,
                    }
                }
                self.orphan_log.push(
                    serde_json::json!({"face": f.describe(), "dim": f.dim(n_of(&self.region)),
                    "points": npts, "max_dr": max_dr.to_string(), "first_match_histogram": hist}),
                );
            }
            Err(c) => {
                self.undecided_log.push(format!(
                    "no generically applicable rule on face [{}], more than {c} lattice points",
                    f.describe()
                ));
                out.undecided.insert(self.undecided_log.len() - 1);
            }
        }
        out
    }

    /// Faces covering the integer zero set (in the region) of a branch whose members are all nonlinear on `f`.
    fn nonlinear_faces(&mut self, f: &Face, polys: &[QPoly]) -> Result<Vec<Face>, String> {
        let n = self.n;
        // the member with the fewest variables
        let mut ps: Vec<&QPoly> = polys.iter().collect();
        ps.sort_by_key(|p| (qpoly_vars(p).len(), total_degree(p)));
        let p = ps[0];
        let mut faces = vec![];
        for (fac, _) in p.factor() {
            if fac.is_constant() {
                continue;
            }
            if total_degree(&fac) <= 1 {
                let l = qpoly_to_lin(&fac, n).ok_or("linear factor with d")?;
                if let Some(g) = f.add_eq(&l) {
                    faces.push(g);
                }
                continue;
            }
            let vs = qpoly_vars(&fac);
            if vs.iter().any(|&v| v >= n) {
                return Err("nonlinear factor uses d".into());
            }
            if vs.len() == 1 {
                // irreducible over Q of degree >= 2 in one variable: no rational, hence no integer, root
                self.stats.univariate_without_integer_root += 1;
                continue;
            }
            if let Some(zs) = bivariate_quadratic_zeros(&fac, &vs) {
                // exact and complete: every integer zero of the curve (no region bound needed)
                self.stats.bivariate_quadratic_exact += 1;
                for (x, y) in zs {
                    if let Some(g) = f.pin(n, vs[0], x).and_then(|g| g.pin(n, vs[1], y)) {
                        faces.push(g);
                    }
                }
                continue;
            }
            let Some(r) = self.region.r else {
                return Err(format!(
                    "nonlinear factor in {} variables, unbounded region",
                    vs.len()
                ));
            };
            let budget = r - f.pinned_dr(&self.region);
            if budget < 0 {
                continue;
            }
            // simplex size C(budget + m, m)
            let m = vs.len() as u128;
            let mut size: u128 = 1;
            for i in 1..=m {
                size = size * (budget as u128 + i) / i;
            }
            if size > self.zero_enum_cap {
                return Err(format!(
                    "nonlinear factor in {} variables: {size} candidate points over the cap",
                    vs.len()
                ));
            }
            let mut assign = vec![0i64; n + 1];
            let mut zeros: Vec<Vec<i64>> = vec![];
            enum_simplex(&vs, &self.region.mask, budget, &mut assign, 0, &mut |a| {
                self.stats.nonlinear_points_tested += 1;
                if qpoly_eval(&fac, a).is_zero() {
                    // the other members must vanish there too (prune with those depending only on vs)
                    let ok = ps[1..].iter().all(|q| {
                        let qv = qpoly_vars(q);
                        if qv.iter().all(|v| vs.contains(v)) {
                            qpoly_eval(q, a).is_zero()
                        } else {
                            true
                        }
                    });
                    if ok {
                        zeros.push(vs.iter().map(|&v| a[v]).collect());
                    }
                }
            });
            self.stats.nonlinear_zeros += zeros.len() as u64;
            for z in zeros {
                let mut g = Some(f.clone());
                for (&v, &x) in vs.iter().zip(&z) {
                    g = g.and_then(|g| g.pin(n, v, x));
                }
                if let Some(g) = g {
                    faces.push(g);
                }
            }
        }
        Ok(faces)
    }
}

pub fn enum_simplex(
    vs: &[usize],
    mask: &[bool],
    left: i128,
    assign: &mut Vec<i64>,
    idx: usize,
    f: &mut dyn FnMut(&[i64]),
) {
    if idx == vs.len() {
        f(assign);
        return;
    }
    let v = vs[idx];
    for y in 0..=left {
        assign[v] = if mask[v] { 1 + y as i64 } else { -(y as i64) };
        enum_simplex(vs, mask, left - y, assign, idx + 1, f);
    }
}

fn n_of(r: &Region) -> usize {
    r.mask.len()
}

fn merge(out: &mut Outcome, o: &Outcome) {
    out.uncovered.extend(o.uncovered.iter().cloned());
    out.undecided.extend(o.undecided.iter().copied());
}

/// Rational check used by tests: Q is the field of the polynomials here.
pub fn _field() -> symbolica::domains::rational::RationalField {
    Q
}

fn to_rat(q: &Rational) -> Option<Rat> {
    Some(Rat::new(
        q.numerator().to_i64()? as i128,
        q.denominator().to_i64()? as i128,
    ))
}

/// Every integer zero of an irreducible quadratic in two variables whose quadratic part is a product of two
/// independent rational linear forms: p = (L1 + s)(L2 + t) - K, so integer zeros satisfy A1 * A2 = N with integral
/// affine forms A1, A2 (scaled) and the integer N; each divisor pair gives one linear system. None = not of that form
/// (then the caller enumerates in the bounded region or reports).
pub fn bivariate_quadratic_zeros(p: &QPoly, vs: &[usize]) -> Option<Vec<(i64, i64)>> {
    if vs.len() != 2 || total_degree(p) != 2 {
        return None;
    }
    let (x, y) = (vs[0], vs[1]);
    let mut co = [Rat::ZERO; 6]; // a x^2, b xy, c y^2, d x, e y, f
    for (c, ex) in p.coefficients.iter().zip(p.exponents_iter()) {
        let r = to_rat(c)?;
        let (ix, iy) = (ex[x], ex[y]);
        let k = match (ix, iy) {
            (2, 0) => 0,
            (1, 1) => 1,
            (0, 2) => 2,
            (1, 0) => 3,
            (0, 1) => 4,
            (0, 0) => 5,
            _ => return None,
        };
        co[k] = r;
    }
    let [a, b, c, d, e, f] = co;
    // quadratic form = L1 * L2 with L1 = al1 x + be1 y, L2 = al2 x + be2 y
    let (al1, be1, al2, be2);
    if a.is_zero() {
        // y (b x + c y)
        if b.is_zero() {
            return None;
        }
        (al1, be1, al2, be2) = (Rat::ZERO, Rat::ONE, b, c);
    } else {
        let disc = b.mul(b).sub(Rat::int(4).mul(a).mul(c));
        if disc.is_zero() || disc < Rat::ZERO {
            return None;
        }
        // exact integer square root (binary search), None if not a perfect square
        let sq = |v: i128| -> Option<i128> {
            if v < 0 {
                return None;
            }
            let (mut lo, mut hi) = (0i128, (1i128 << 62).min(v + 1));
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if mid.checked_mul(mid).is_some_and(|m| m < v) {
                    lo = mid + 1
                } else {
                    hi = mid
                }
            }
            (lo.checked_mul(lo) == Some(v)).then_some(lo)
        };
        let (sn, sd) = (sq(disc.n)?, sq(disc.d)?);
        let root = Rat::new(sn, sd);
        // a x^2 + b x y + c y^2 = a (x - r1 y)(x - r2 y), r1,2 = (-b +- root) / (2a)
        let two_a = Rat::int(2).mul(a);
        let r1 = b.neg().add(root).div(two_a);
        let r2 = b.neg().sub(root).div(two_a);
        (al1, be1, al2, be2) = (Rat::ONE, r1.neg(), a, a.mul(r2).neg());
    }
    let det = al2.mul(be1).sub(al1.mul(be2));
    if det.is_zero() {
        return None;
    }
    // s L2 + t L1 = d x + e y:  s al2 + t al1 = d,  s be2 + t be1 = e
    let s_ = d.mul(be1).sub(e.mul(al1)).div(det);
    let t_ = e.mul(al2).sub(d.mul(be2)).div(det);
    let k = s_.mul(t_).sub(f);
    if k.is_zero() {
        return None; // reducible: the factoriser would have split it
    }
    // integral scalings A1 = m1 (L1 + s), A2 = m2 (L2 + t)
    let lcm = |u: i128, v: i128| u / crate::expr::gcd(u, v) * v;
    let m1 = lcm(lcm(al1.d, be1.d), s_.d);
    let m2 = lcm(lcm(al2.d, be2.d), t_.d);
    let nn = k.mul(Rat::int(m1)).mul(Rat::int(m2));
    if !nn.is_int() {
        return Some(vec![]);
    }
    let big_n = nn.n;
    if big_n.abs() > 1_000_000_000_000 {
        return None;
    }
    let (a1x, a1y, a1c) = (
        al1.mul(Rat::int(m1)),
        be1.mul(Rat::int(m1)),
        s_.mul(Rat::int(m1)),
    );
    let (a2x, a2y, a2c) = (
        al2.mul(Rat::int(m2)),
        be2.mul(Rat::int(m2)),
        t_.mul(Rat::int(m2)),
    );
    let mut out = vec![];
    let mut u = 1i128;
    while u * u <= big_n.abs() {
        if big_n % u == 0 {
            let w = big_n / u;
            for (p1, p2) in [(u, w), (w, u), (-u, -w), (-w, -u)] {
                // a1x X + a1y Y = p1 - a1c ; a2x X + a2y Y = p2 - a2c
                let r1 = Rat::int(p1).sub(a1c);
                let r2 = Rat::int(p2).sub(a2c);
                let dd = a1x.mul(a2y).sub(a1y.mul(a2x));
                if dd.is_zero() {
                    continue;
                }
                let xx = r1.mul(a2y).sub(a1y.mul(r2)).div(dd);
                let yy = a1x.mul(r2).sub(r1.mul(a2x)).div(dd);
                if xx.is_int() && yy.is_int() {
                    out.push((xx.n as i64, yy.n as i64));
                }
            }
        }
        u += 1;
    }
    out.sort();
    out.dedup();
    // every reported pair must be a zero (exact re-check)
    for &(xv, yv) in &out {
        let mut assign = vec![0i64; p.nvars()];
        assign[x] = xv;
        assign[y] = yv;
        if !qpoly_eval(p, &assign).is_zero() {
            return None;
        }
    }
    Some(out)
}
