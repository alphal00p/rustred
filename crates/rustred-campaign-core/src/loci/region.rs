//! Lattice regions `x = x0 + M t` (t in Z^k) with integer constraints `A t + b >= 0`: exact rational
//! Fourier-Motzkin with Chvatal-Gomory rounding (rational-infeasible implies integer-infeasible; rounding keeps every
//! integer point), exact lattice parametrisation of the integer solutions of one linear equation (extended-gcd
//! column reduction = Hermite normal form of one row; any integer coefficients), exact integer witness search.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use std::cell::Cell;
use std::collections::HashSet;

use crate::partition::exact::{
    Affine, ExactError, MPoly, XResult, add, ceil_neg_div, div_floor, ext_gcd, floor_neg_div,
    gcd_all, mul, quot, rem, succ,
};

/// Constraints kept at most by one Fourier-Motzkin step (beyond: `TooLarge`, undecided). Checked WHILE the step is
/// generated (audit-part3 m4: it used to be checked only after all |P| x |N| combinations were materialised).
pub const FM_MAX_CONSTRAINTS: usize = 20_000;
/// Pairwise combinations tried at most by one Fourier-Motzkin step (beyond: `TooLarge`, undecided).
pub const FM_MAX_PAIRS: usize = 4_000_000;
/// Values tried at most by one [`find_point`] call over all its recursion levels (beyond: "no point found", which no
/// caller reads as emptiness; the search used to be `tries^k` in the worst case).
pub const POINT_SEARCH_CAP: u64 = 20_000;

thread_local! {
    /// Work units of Fourier-Motzkin steps (pairs + kept constraints) and point searches (values tried) on this
    /// thread since the last [`take_work`] (audit-part3 m4).
    static WORK: Cell<u64> = const { Cell::new(0) };
}

fn charge(n: usize) {
    let n = u64::try_from(n).unwrap_or(u64::MAX);
    WORK.with(|w| w.set(w.get().saturating_add(n)));
}

/// The Fourier-Motzkin / point-search work done on this thread since the last call, and reset (audit-part3 m4):
/// the partition's resolver and the containment splitter charge it to their budgets at every node (and discard
/// leftovers when they start), so the work INSIDE one node is counted.
pub fn take_work() -> u64 {
    WORK.with(|w| w.replace(0))
}

/// One elimination step: `rest` plus every normalised combination of `pos` x `negs` on variable `v`, deduplicated.
/// `Ok(None)` = a constant violated combination (the system is infeasible). The kept-constraint cap is checked as the
/// step grows, the pair count before it starts.
fn eliminate(
    pos: &[Affine],
    negs: &[Affine],
    mut rest: Vec<Affine>,
    v: usize,
    drop_constant: bool,
    what: &'static str,
) -> XResult<Option<Vec<Affine>>> {
    let pairs = pos.len().saturating_mul(negs.len());
    charge(pairs.saturating_add(rest.len()));
    if pairs > FM_MAX_PAIRS {
        return Err(ExactError::TooLarge(what));
    }
    let mut seen: HashSet<Affine> = rest.iter().cloned().collect();
    for p in pos {
        for n in negs {
            let (ap, an) = (p.coef[v], crate::partition::exact::neg(n.coef[v])?);
            if let Some(nc) = norm_or_raw(&combine(p, n, ap, an)?) {
                if drop_constant && nc.is_constant() {
                    if nc.c < 0 {
                        return Ok(None);
                    }
                    continue;
                }
                if seen.insert(nc.clone()) {
                    rest.push(nc);
                    if rest.len() > FM_MAX_CONSTRAINTS {
                        return Err(ExactError::TooLarge(what));
                    }
                }
            }
        }
    }
    Ok(Some(rest))
}

/// A lattice region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    /// Number of parameters.
    pub k: usize,
    /// Per slot: affine in the parameters.
    pub x: Vec<Affine>,
    /// Constraints `>= 0`, affine in the parameters.
    pub cons: Vec<Affine>,
    /// Parameter names (for rendering).
    pub names: Vec<String>,
}

/// Normalise a constraint `a t + b >= 0`: divide by gcd(a), round b down (valid for integer t). `Ok(None)` = trivially
/// true; a constant negative constraint becomes the constant `-1` (infeasible marker). `Err(Overflow)` only when the
/// gcd is 2^127 (re-audit RA2-C1: never a clamped divisor).
pub fn normalise(c: &Affine) -> XResult<Option<Affine>> {
    let g = gcd_all(c.coef.iter().copied())?;
    if g == 0 {
        return Ok(if c.c >= 0 {
            None
        } else {
            Some(Affine::constant(c.coef.len(), -1))
        });
    }
    Ok(Some(Affine {
        coef: c.coef.iter().map(|&a| quot(a, g)).collect::<XResult<_>>()?,
        c: div_floor(c.c, g)?,
    }))
}

/// [`normalise`], keeping the constraint as it is when normalising overflows (the raw constraint is the same set:
/// normalisation only strengthens it for integers).
fn norm_or_raw(c: &Affine) -> Option<Affine> {
    match normalise(c) {
        Ok(n) => n,
        Err(_) => Some(c.clone()),
    }
}

/// The Fourier-Motzkin combination `an * p + ap * n` (eliminating a variable with coefficients ap > 0 in p and
/// -an < 0 in n), checked.
fn combine(p: &Affine, n: &Affine, ap: i128, an: i128) -> XResult<Affine> {
    let coef = p
        .coef
        .iter()
        .zip(&n.coef)
        .map(|(&pj, &nj)| add(mul(an, pj)?, mul(ap, nj)?))
        .collect::<XResult<Vec<_>>>()?;
    let c = add(mul(an, p.c)?, mul(ap, n.c)?)?;
    Ok(Affine { coef, c })
}

/// The two sides `f >= 1` and `f <= -1` of the hyperplane `f = coef . t + c = 0`, checked (re-audit RA2-C1: never an
/// unchecked negation; `Err(Overflow)` when a coefficient is `i128::MIN`).
pub fn off_sides(coef: &[i128], c: i128) -> XResult<(Affine, Affine)> {
    let f = Affine {
        coef: coef.to_vec(),
        c,
    };
    let pos = f.shifted(1)?; // f - 1 >= 0
    let negs = Affine {
        coef: crate::partition::exact::neg_all(coef)?,
        c: crate::partition::exact::pred(crate::partition::exact::neg(c)?)?,
    }; // -f - 1 >= 0
    Ok((pos, negs))
}

/// Rational feasibility (with integer rounding) of `cons >= 0` in `k` variables. `Ok(false)` = PROVED integer-empty;
/// `Err` (overflow, cap) = undecided (callers never read it as empty).
pub fn fm_feasible(cons: &[Affine], k: usize) -> XResult<bool> {
    let mut cur: Vec<Affine> = vec![];
    let mut seen: HashSet<Affine> = HashSet::new();
    for c in cons {
        if let Some(n) = norm_or_raw(c) {
            if n.is_constant() {
                if n.c < 0 {
                    return Ok(false);
                }
                continue;
            }
            if seen.insert(n.clone()) {
                cur.push(n);
            }
        }
    }
    let mut alive: Vec<usize> = (0..k).collect();
    while !alive.is_empty() {
        // pick the variable with the smallest |P| * |N|
        let mut best = (usize::MAX, 0usize);
        for (ai, &v) in alive.iter().enumerate() {
            let p = cur.iter().filter(|c| c.coef[v] > 0).count();
            let n = cur.iter().filter(|c| c.coef[v] < 0).count();
            let pn = p.saturating_mul(n);
            if pn < best.0 {
                best = (pn, ai);
            }
        }
        let v = alive.remove(best.1);
        let (mut pos, mut negs, mut rest) = (vec![], vec![], vec![]);
        for c in cur {
            match c.coef[v].signum() {
                1 => pos.push(c),
                -1 => negs.push(c),
                _ => rest.push(c),
            }
        }
        match eliminate(
            &pos,
            &negs,
            rest,
            v,
            true,
            "Fourier-Motzkin: more than 20000 constraints (or 4e6 pairs) in one step",
        )? {
            None => return Ok(false),
            Some(r) => cur = r,
        }
    }
    Ok(true)
}

/// Bounds of variable 0 implied by constraints in which only variable 0 occurs; `ok = false`: a constant constraint
/// is violated (empty). Checked (re-audit RA2-M1: `-c / a` without an unchecked negation).
#[allow(clippy::type_complexity)]
fn bounds0(cons: &[Affine]) -> XResult<(Option<i128>, Option<i128>, bool)> {
    let mut lo: Option<i128> = None;
    let mut hi: Option<i128> = None;
    for c in cons {
        if c.coef.iter().skip(1).any(|&a| a != 0) {
            continue;
        }
        let a = c.coef.first().copied().unwrap_or(0);
        if a == 0 {
            if c.c < 0 {
                return Ok((None, None, false));
            }
            continue;
        }
        if a > 0 {
            // a t + c >= 0 -> t >= ceil(-c / a)
            let l = ceil_neg_div(c.c, a)?;
            lo = Some(lo.map_or(l, |x: i128| x.max(l)));
        } else {
            // a t + c >= 0, a < 0 -> t <= floor(-c / a)
            let h = floor_neg_div(c.c, a)?;
            hi = Some(hi.map_or(h, |x: i128| x.min(h)));
        }
    }
    Ok((lo, hi, true))
}

/// Eliminate variables 1..k-1 (projection onto variable 0), exact.
fn project0(cons: &[Affine], k: usize) -> XResult<Vec<Affine>> {
    let mut cur: Vec<Affine> = cons.iter().filter_map(norm_or_raw).collect();
    for v in 1..k {
        let (mut pos, mut negs, mut rest) = (vec![], vec![], vec![]);
        for c in cur {
            match c.coef[v].signum() {
                1 => pos.push(c),
                -1 => negs.push(c),
                _ => rest.push(c),
            }
        }
        cur = eliminate(
            &pos,
            &negs,
            rest,
            v,
            false,
            "projection: more than 20000 constraints (or 4e6 pairs) in one step",
        )?
        .unwrap_or_default();
    }
    Ok(cur)
}

/// Substitute variable 0 = v and drop it.
fn fix0(cons: &[Affine], v: i128) -> XResult<Vec<Affine>> {
    cons.iter()
        .map(|c| {
            Ok(Affine {
                coef: c.coef.get(1..).unwrap_or(&[]).to_vec(),
                c: add(c.c, mul(c.coef.first().copied().unwrap_or(0), v)?)?,
            })
        })
        .collect()
}

/// An integer point of `cons >= 0` found by exact search (variable by variable from the projected bounds, values
/// nearest to a bound or to `anchor` first, at most `tries` values per level). `None` = none found (not a proof).
pub fn find_point(
    cons: &[Affine],
    k: usize,
    tries: usize,
    anchor: &[i128],
) -> XResult<Option<Vec<i128>>> {
    let mut left = POINT_SEARCH_CAP;
    find_point_capped(cons, k, tries, anchor, &mut left)
}

/// [`find_point`] with the values left to try over ALL recursion levels (each value tried is charged to the thread's
/// work counter); exhausted = "no point found".
fn find_point_capped(
    cons: &[Affine],
    k: usize,
    tries: usize,
    anchor: &[i128],
    left: &mut u64,
) -> XResult<Option<Vec<i128>>> {
    let Some(k1) = k.checked_sub(1) else {
        return Ok(if cons.iter().all(|c| c.c >= 0) {
            Some(vec![])
        } else {
            None
        });
    };
    let proj = project0(cons, k)?;
    let (lo, hi, ok) = bounds0(&proj)?;
    if !ok {
        return Ok(None);
    }
    if let (Some(l), Some(h)) = (lo, hi)
        && l > h
    {
        return Ok(None);
    }
    let a = anchor.first().copied().unwrap_or(0);
    let start = match (lo, hi) {
        (Some(l), Some(h)) => a.clamp(l, h),
        (Some(l), None) => a.max(l),
        (None, Some(h)) => a.min(h),
        (None, None) => a,
    };
    let inside = |c: i128| lo.is_none_or(|l| c >= l) && hi.is_none_or(|h| c <= h);
    let mut cands = vec![start];
    let reach = i128::try_from(tries.saturating_mul(4).saturating_add(4)).unwrap_or(i128::MAX);
    let mut d: i128 = 1;
    while cands.len() < tries && d < reach {
        for c in [start.checked_add(d), start.checked_sub(d)]
            .into_iter()
            .flatten()
        {
            if inside(c) {
                cands.push(c);
            }
        }
        d = d.saturating_add(1);
    }
    for v in cands.into_iter().take(tries) {
        if *left == 0 {
            return Ok(None);
        }
        *left = left.saturating_sub(1);
        charge(1);
        let sub = fix0(cons, v)?;
        if let Some(mut rest) =
            find_point_capped(&sub, k1, tries, anchor.get(1..).unwrap_or(&[]), left)?
        {
            rest.insert(0, v);
            return Ok(Some(rest));
        }
    }
    Ok(None)
}

/// EXHAUSTIVE integer search of `cons >= 0` (audit-part-B M1): every value of each variable inside its projected
/// bounds (Fourier-Motzkin with rounding keeps every integer point, so the range is a superset). `Some(Some(t))` = a
/// point, `Some(None)` = PROVED integer-empty, `None` = undecided (an unbounded projection or the `budget` of tried
/// values exhausted). `Err` (overflow) = undecided as well.
pub fn exhaustive_point(
    cons: &[Affine],
    k: usize,
    budget: &mut u64,
) -> XResult<Option<Option<Vec<i128>>>> {
    let Some(k1) = k.checked_sub(1) else {
        return Ok(Some(if cons.iter().all(|c| c.c >= 0) {
            Some(vec![])
        } else {
            None
        }));
    };
    let proj = project0(cons, k)?;
    let (lo, hi, ok) = bounds0(&proj)?;
    if !ok {
        return Ok(Some(None));
    }
    let (Some(l), Some(h)) = (lo, hi) else {
        return Ok(None);
    };
    if l > h {
        return Ok(Some(None));
    }
    let mut unknown = false;
    let mut v = l;
    loop {
        if *budget == 0 {
            return Ok(None);
        }
        *budget = budget.saturating_sub(1);
        charge(1);
        let sub = fix0(cons, v)?;
        match exhaustive_point(&sub, k1, budget)? {
            Some(Some(mut r)) => {
                r.insert(0, v);
                return Ok(Some(Some(r)));
            }
            Some(None) => {}
            None => unknown = true,
        }
        if v >= h {
            break;
        }
        v = succ(v)?;
    }
    Ok(if unknown { None } else { Some(None) })
}

impl Region {
    /// The point of the region at parameter values `t`.
    pub fn at(&self, t: &[i128]) -> XResult<Vec<i128>> {
        self.x.iter().map(|a| a.eval(t)).collect()
    }

    pub fn contains_t(&self, t: &[i128]) -> XResult<bool> {
        for c in &self.cons {
            if c.eval(t)? < 0 {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Rational/rounded feasibility: `Ok(false)` = proved integer-empty.
    pub fn feasible(&self) -> XResult<bool> {
        fm_feasible(&self.cons, self.k)
    }

    /// A literal (polynomial in the slots) on the region, as a polynomial in the parameters.
    pub fn lit(&self, p: &MPoly) -> XResult<MPoly> {
        p.compose(&self.x, self.k)
    }

    /// IMPLICIT EQUALITIES made explicit (lane parametric 2026-11-17): while two constraints are exact opposites
    /// (`a t + b >= 0` and `-a t - b >= 0`, `a != 0`), the region is restricted to `a t + b = 0` by the exact lattice
    /// [`Region::cut`] (one parameter fewer per equation). Same integer points, fewer parameters, so a cut that was
    /// "nonlinear in two or more parameters" on the thin region may become univariate. `Ok(None)` = no integer point.
    pub fn reduce_equalities(&self) -> XResult<Option<Region>> {
        let mut r = self.clone();
        loop {
            let mut pair = None;
            'find: for (i, a) in r.cons.iter().enumerate() {
                if a.coef.iter().all(|&x| x == 0) {
                    continue;
                }
                // int8: the module denies arithmetic side effects (an index + 1 cannot overflow, but the lint rules)
                for b in r.cons.iter().skip(i.saturating_add(1)) {
                    if b.c == a.c.checked_neg().unwrap_or(i128::MIN)
                        && b.coef.len() == a.coef.len()
                        && b.coef
                            .iter()
                            .zip(&a.coef)
                            .all(|(&x, &y)| y.checked_neg() == Some(x))
                    {
                        pair = Some((a.coef.clone(), a.c));
                        break 'find;
                    }
                }
            }
            let Some((coef, c)) = pair else {
                return Ok(Some(r));
            };
            match r.cut(&coef, c)? {
                Some(nr) => {
                    if nr.k >= r.k {
                        // no progress (should not happen: a cut drops one parameter)
                        return Ok(Some(nr));
                    }
                    r = nr;
                }
                None => return Ok(None),
            }
        }
    }

    /// Restrict to the integer solutions of `f(t) = 0` (f linear in t). `Ok(None)` = no integer solution.
    pub fn cut(&self, coef: &[i128], c: i128) -> XResult<Option<Region>> {
        let k = self.k;
        let Some(nk) = k.checked_sub(1) else {
            // no parameter: the equation is the constant c
            return Ok(if c == 0 { Some(self.clone()) } else { None });
        };
        // column reduction of the row `coef` to (g, 0, ..., 0) by unimodular U: coef U = (g, 0, ...)
        let mut row = coef.to_vec();
        let mut u: Vec<Vec<i128>> = (0..k)
            .map(|i| (0..k).map(|j| i128::from(i == j)).collect())
            .collect();
        // bring a nonzero entry to column 0 by combining columns
        for j in 1..k {
            if row[j] == 0 {
                continue;
            }
            let (g, x, y) = ext_gcd(row[0], row[j])?;
            if g == 0 {
                continue;
            }
            let (a0, aj) = (quot(row[0], g)?, quot(row[j], g)?);
            // new col0 = x col0 + y colj ; new colj = -aj col0 + a0 colj  (determinant x a0 + y aj = 1)
            for r in u.iter_mut() {
                let c0 = r[0];
                let cj = r[j];
                r[0] = add(mul(x, c0)?, mul(y, cj)?)?;
                r[j] = crate::partition::exact::sub(mul(a0, cj)?, mul(aj, c0)?)?;
            }
            row[0] = g;
            row[j] = 0;
        }
        let g = row[0];
        if g == 0 {
            return Ok(if c == 0 { Some(self.clone()) } else { None });
        }
        if g.unsigned_abs() != 1 && rem(c, g)? != 0 {
            return Ok(None);
        }
        // coef U s = g s0 => s0 = -c / g (exact division)
        let s0 = floor_neg_div(c, g)?;
        // t = U s, s = (s0, s1..s_{k-1}); new parameters s1..
        let map: Vec<Affine> = (0..k)
            .map(|i| -> XResult<Affine> {
                Ok(Affine {
                    coef: (1..k).map(|j| u[i][j]).collect(),
                    c: mul(u[i][0], s0)?,
                })
            })
            .collect::<XResult<_>>()?;
        let comp = |a: &Affine| -> XResult<Affine> {
            let mut coef = vec![0i128; nk];
            let mut cc = a.c;
            for (i, &ai) in a.coef.iter().enumerate() {
                if ai == 0 {
                    continue;
                }
                cc = add(cc, mul(ai, map[i].c)?)?;
                for (cj, mj) in coef.iter_mut().zip(&map[i].coef) {
                    *cj = add(*cj, mul(ai, *mj)?)?;
                }
            }
            Ok(Affine { coef, c: cc })
        };
        let x = self.x.iter().map(comp).collect::<XResult<Vec<_>>>()?;
        let cons = self.cons.iter().map(comp).collect::<XResult<Vec<_>>>()?;
        let cons: Vec<Affine> = cons.iter().filter_map(norm_or_raw).collect();
        let names = (0..nk)
            .map(|j| format!("s{}_{j}", self.names.len()))
            .collect();
        Ok(Some(Region {
            k: nk,
            x,
            cons,
            names,
        }))
    }

    /// Fix parameter `j` to `v`.
    pub fn fix(&self, j: usize, v: i128) -> XResult<Region> {
        let sub = |a: &Affine| -> XResult<Affine> {
            let mut coef = a.coef.clone();
            let cj = coef.remove(j);
            Ok(Affine {
                coef,
                c: add(a.c, mul(cj, v)?)?,
            })
        };
        let mut names = self.names.clone();
        names.remove(j);
        Ok(Region {
            k: self.k.saturating_sub(1),
            x: self.x.iter().map(sub).collect::<XResult<_>>()?,
            cons: self
                .cons
                .iter()
                .map(sub)
                .collect::<XResult<Vec<_>>>()?
                .iter()
                .filter_map(norm_or_raw)
                .collect(),
            names,
        })
    }

    /// Add a constraint `a(t) >= 0` (normalised when possible; kept as given when normalising overflows).
    pub fn with(&self, c: Affine) -> Region {
        let mut r = self.clone();
        if let Some(n) = norm_or_raw(&c) {
            r.cons.push(n);
        }
        r
    }

    /// An integer point (parameters) of the region, if the exact search finds one.
    pub fn point(&self, tries: usize) -> XResult<Option<Vec<i128>>> {
        find_point(&self.cons, self.k, tries, &vec![0; self.k])
    }

    /// Exact integer emptiness decision of the region: a point by the cheap search, else the EXHAUSTIVE search of a
    /// bounded region (`budget` values). `Ok(Some(Some(t)))` a point, `Ok(Some(None))` PROVED integer-empty,
    /// `Ok(None)` / `Err` undecided (re-audit RA2-M3).
    pub fn decide_point(&self, tries: usize, budget: u64) -> XResult<Option<Option<Vec<i128>>>> {
        if !self.feasible()? {
            return Ok(Some(None));
        }
        if let Some(t) = self.point(tries)? {
            return Ok(Some(Some(t)));
        }
        let mut b = budget;
        exhaustive_point(&self.cons, self.k, &mut b)
    }

    /// The integer range of parameter `j` implied by the constraints (Fourier-Motzkin projection with rounding: a
    /// superset of the true range). An empty region gives `lo > hi`.
    pub fn range_of(&self, j: usize) -> XResult<(Option<i128>, Option<i128>)> {
        let cons: Vec<Affine> = self
            .cons
            .iter()
            .map(|a| {
                let mut coef = a.coef.clone();
                coef.swap(0, j);
                Affine { coef, c: a.c }
            })
            .collect();
        let proj = project0(&cons, self.k)?;
        let (lo, hi, ok) = bounds0(&proj)?;
        if !ok {
            return Ok((Some(1), Some(0)));
        }
        Ok((lo, hi))
    }

    /// Is the region unbounded (rationally)? Exact: some direction e_j or -e_j stays feasible from a point.
    pub fn unbounded(&self) -> XResult<bool> {
        for j in 0..self.k {
            for s in [1i128, -1] {
                // recession cone test: A d >= 0 with d_j = s
                let mut cons: Vec<Affine> = self
                    .cons
                    .iter()
                    .map(|c| Affine {
                        coef: c.coef.clone(),
                        c: 0,
                    })
                    .collect();
                cons.push(Affine::unit(self.k, j, s, -1)); // s d_j >= 1
                if fm_feasible(&cons, self.k)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub fn render_x(&self) -> Vec<String> {
        self.x.iter().map(|a| a.render(&self.names)).collect()
    }
    pub fn render_cons(&self) -> Vec<String> {
        self.cons.iter().map(|a| a.render(&self.names)).collect()
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)] // test arithmetic on small literals
mod tests {
    use super::*;

    fn aff(coef: &[i128], c: i128) -> Affine {
        Affine {
            coef: coef.to_vec(),
            c,
        }
    }

    #[test]
    fn fm_and_points() {
        // t0 >= 2, t1 <= -1, t0 - t1 <= 2 : infeasible
        let c = vec![aff(&[1, 0], -2), aff(&[0, -1], -1), aff(&[-1, 1], 2)];
        assert!(!fm_feasible(&c, 2).unwrap());
        // integer-infeasible but rationally feasible: 2 t0 = 1 encoded as 2 t0 - 1 >= 0, -2 t0 + 1 >= 0
        let c = vec![aff(&[2], -1), aff(&[-2], 1)];
        assert!(!fm_feasible(&c, 1).unwrap(), "rounding proves it");
        let c = vec![aff(&[1, 0], -2), aff(&[0, -1], 0), aff(&[-1, 1], 3)];
        let p = find_point(&c, 2, 8, &[0, 0]).unwrap().unwrap();
        assert!(c.iter().all(|a| a.eval(&p).unwrap() >= 0));
    }

    #[test]
    fn lattice_cut() {
        // x = (t0, t1, t2), cut 2 t0 - t1 - 2 t2 + 4 = 0 (non-unit coefficients)
        let r = Region {
            k: 3,
            x: (0..3).map(|j| Affine::var(3, j)).collect(),
            cons: vec![
                aff(&[1, 0, 0], -2),
                aff(&[0, 1, 0], -2),
                aff(&[0, 0, 1], -2),
            ],
            names: vec!["a".into(), "b".into(), "c".into()],
        };
        let s = r.cut(&[2, -1, -2], 4).unwrap().unwrap();
        assert_eq!(s.k, 2);
        let mut n = 0;
        for a in -6..6 {
            for b in -6..6 {
                let x = s.at(&[a, b]).unwrap();
                assert_eq!(2 * x[0] - x[1] - 2 * x[2] + 4, 0);
                n += 1;
            }
        }
        assert!(n > 0);
        // every integer solution is hit: (2, 4, 2) -> 4 - 4 - 4 + 4 = 0
        let mut hit = false;
        for a in -20..20 {
            for b in -20..20 {
                if s.at(&[a, b]).unwrap() == vec![2, 4, 2] {
                    hit = true;
                }
            }
        }
        assert!(hit);
        // 2 t0 + 4 t1 = 1: no integer solution
        assert!(r.cut(&[2, 4, 0], -1).unwrap().is_none());
    }
}
