//! Lattice regions `x = x0 + M t` (t in Z^k) with integer constraints `A t + b >= 0`: exact rational
//! Fourier-Motzkin with Chvatal-Gomory rounding (rational-infeasible implies integer-infeasible; rounding keeps every
//! integer point), exact lattice parametrisation of the integer solutions of one linear equation (extended-gcd
//! column reduction = Hermite normal form of one row; any integer coefficients), exact integer witness search.

use std::collections::HashSet;

use crate::partition::exact::{
    Affine, ExactError, MPoly, XResult, div_ceil, div_floor, ext_gcd, gcd,
};

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

fn ck(o: Option<i128>) -> XResult<i128> {
    o.ok_or(ExactError::Overflow)
}

/// Normalise a constraint `a t + b >= 0`: divide by gcd(a), round b down (valid for integer t). `None` = trivially
/// true; `Some(Err)` never; a constant negative constraint stays (infeasible marker).
fn normalise(c: &Affine) -> Option<Affine> {
    let g = c.coef.iter().fold(0, |g, &a| gcd(g, a));
    if g == 0 {
        return if c.c >= 0 {
            None
        } else {
            Some(Affine::constant(c.coef.len(), -1))
        };
    }
    Some(Affine {
        coef: c.coef.iter().map(|&a| a / g).collect(),
        c: div_floor(c.c, g),
    })
}

/// Rational feasibility (with integer rounding) of `cons >= 0` in `k` variables. `Ok(false)` = PROVED integer-empty.
pub fn fm_feasible(cons: &[Affine], k: usize) -> XResult<bool> {
    let mut cur: Vec<Affine> = vec![];
    let mut seen: HashSet<Affine> = HashSet::new();
    for c in cons {
        if let Some(n) = normalise(c) {
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
            if p * n < best.0 {
                best = (p * n, ai);
            }
        }
        let v = alive.remove(best.1);
        let (mut pos, mut neg, mut rest) = (vec![], vec![], vec![]);
        for c in cur {
            match c.coef[v].signum() {
                1 => pos.push(c),
                -1 => neg.push(c),
                _ => rest.push(c),
            }
        }
        let mut seen: HashSet<Affine> = rest.iter().cloned().collect();
        for p in &pos {
            for n in &neg {
                let (ap, an) = (p.coef[v], -n.coef[v]);
                let mut coef = vec![0i128; k];
                for (j, cj) in coef.iter_mut().enumerate() {
                    *cj =
                        ck(ck(an.checked_mul(p.coef[j]))?
                            .checked_add(ck(ap.checked_mul(n.coef[j]))?))?;
                }
                let c = ck(ck(an.checked_mul(p.c))?.checked_add(ck(ap.checked_mul(n.c))?))?;
                if let Some(nc) = normalise(&Affine { coef, c }) {
                    if nc.is_constant() {
                        if nc.c < 0 {
                            return Ok(false);
                        }
                        continue;
                    }
                    if seen.insert(nc.clone()) {
                        rest.push(nc);
                    }
                }
            }
        }
        if rest.len() > 20_000 {
            return Err(ExactError::TooLarge(
                "Fourier-Motzkin: more than 20000 constraints",
            ));
        }
        cur = rest;
    }
    Ok(true)
}

/// Bounds of variable 0 implied by constraints in which only variable 0 occurs.
fn bounds0(cons: &[Affine]) -> (Option<i128>, Option<i128>, bool) {
    let mut lo: Option<i128> = None;
    let mut hi: Option<i128> = None;
    for c in cons {
        if c.coef.iter().skip(1).any(|&a| a != 0) {
            continue;
        }
        let a = c.coef[0];
        if a == 0 {
            if c.c < 0 {
                return (None, None, false);
            }
            continue;
        }
        if a > 0 {
            let l = div_ceil(-c.c, a);
            lo = Some(lo.map_or(l, |x: i128| x.max(l)));
        } else {
            let h = div_floor(-c.c, a);
            hi = Some(hi.map_or(h, |x: i128| x.min(h)));
        }
    }
    (lo, hi, true)
}

/// Eliminate variables 1..k-1 (projection onto variable 0), exact.
fn project0(cons: &[Affine], k: usize) -> XResult<Vec<Affine>> {
    let mut cur: Vec<Affine> = cons.iter().filter_map(normalise).collect();
    for v in 1..k {
        let (mut pos, mut neg, mut rest) = (vec![], vec![], vec![]);
        for c in cur {
            match c.coef[v].signum() {
                1 => pos.push(c),
                -1 => neg.push(c),
                _ => rest.push(c),
            }
        }
        let mut seen: HashSet<Affine> = rest.iter().cloned().collect();
        for p in &pos {
            for n in &neg {
                let (ap, an) = (p.coef[v], -n.coef[v]);
                let mut coef = vec![0i128; k];
                for (j, cj) in coef.iter_mut().enumerate() {
                    *cj =
                        ck(ck(an.checked_mul(p.coef[j]))?
                            .checked_add(ck(ap.checked_mul(n.coef[j]))?))?;
                }
                let c = ck(ck(an.checked_mul(p.c))?.checked_add(ck(ap.checked_mul(n.c))?))?;
                if let Some(nc) = normalise(&Affine { coef, c })
                    && seen.insert(nc.clone())
                {
                    rest.push(nc);
                }
            }
        }
        if rest.len() > 20_000 {
            return Err(ExactError::TooLarge(
                "projection: more than 20000 constraints",
            ));
        }
        cur = rest;
    }
    Ok(cur)
}

/// Substitute variable 0 = v and drop it.
fn fix0(cons: &[Affine], v: i128) -> XResult<Vec<Affine>> {
    cons.iter()
        .map(|c| {
            Ok(Affine {
                coef: c.coef[1..].to_vec(),
                c: ck(c.c.checked_add(ck(c.coef[0].checked_mul(v))?))?,
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
    if k == 0 {
        return Ok(if cons.iter().all(|c| c.c >= 0) {
            Some(vec![])
        } else {
            None
        });
    }
    let proj = project0(cons, k)?;
    let (lo, hi, ok) = bounds0(&proj);
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
    let mut cands = vec![start];
    let mut d = 1;
    while cands.len() < tries && d < 4 * tries as i128 + 4 {
        for c in [start + d, start - d] {
            if lo.is_none_or(|l| c >= l) && hi.is_none_or(|h| c <= h) {
                cands.push(c);
            }
        }
        d += 1;
    }
    for v in cands.into_iter().take(tries) {
        let sub = fix0(cons, v)?;
        if let Some(mut rest) = find_point(&sub, k - 1, tries, anchor.get(1..).unwrap_or(&[]))? {
            rest.insert(0, v);
            return Ok(Some(rest));
        }
    }
    Ok(None)
}

/// EXHAUSTIVE integer search of `cons >= 0` (audit-part-B M1): every value of each variable inside its projected
/// bounds (Fourier-Motzkin with rounding keeps every integer point, so the range is a superset). `Some(Some(t))` = a
/// point, `Some(None)` = PROVED integer-empty, `None` = undecided (an unbounded projection or the `budget` of tried
/// values exhausted).
pub fn exhaustive_point(
    cons: &[Affine],
    k: usize,
    budget: &mut u64,
) -> XResult<Option<Option<Vec<i128>>>> {
    if k == 0 {
        return Ok(Some(if cons.iter().all(|c| c.c >= 0) {
            Some(vec![])
        } else {
            None
        }));
    }
    let proj = project0(cons, k)?;
    let (lo, hi, ok) = bounds0(&proj);
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
    while v <= h {
        if *budget == 0 {
            return Ok(None);
        }
        *budget -= 1;
        let sub = fix0(cons, v)?;
        match exhaustive_point(&sub, k - 1, budget)? {
            Some(Some(mut r)) => {
                r.insert(0, v);
                return Ok(Some(Some(r)));
            }
            Some(None) => {}
            None => unknown = true,
        }
        v += 1;
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

    /// Restrict to the integer solutions of `f(t) = 0` (f linear in t). `Ok(None)` = no integer solution.
    pub fn cut(&self, coef: &[i128], c: i128) -> XResult<Option<Region>> {
        let k = self.k;
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
            let (a0, aj) = (row[0] / g, row[j] / g);
            // new col0 = x col0 + y colj ; new colj = -aj col0 + a0 colj  (determinant x a0 + y aj = 1)
            for r in u.iter_mut() {
                let c0 = r[0];
                let cj = r[j];
                r[0] = ck(ck(x.checked_mul(c0))?.checked_add(ck(y.checked_mul(cj))?))?;
                r[j] = ck(ck(a0.checked_mul(cj))?.checked_sub(ck(aj.checked_mul(c0))?))?;
            }
            row[0] = g;
            row[j] = 0;
        }
        let g = row[0];
        if g == 0 {
            return Ok(if c == 0 { Some(self.clone()) } else { None });
        }
        let g_abs = g.abs();
        if c % g_abs != 0 {
            return Ok(None);
        }
        // coef U s = g s0 => s0 = -c / g
        let s0 = ck((-c).checked_div(g))?;
        // t = U s, s = (s0, s1..s_{k-1}); new parameters s1..
        let nk = k - 1;
        let map: Vec<Affine> = (0..k)
            .map(|i| -> XResult<Affine> {
                Ok(Affine {
                    coef: (1..k).map(|j| u[i][j]).collect(),
                    c: ck(u[i][0].checked_mul(s0))?,
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
                cc = ck(cc.checked_add(ck(ai.checked_mul(map[i].c))?))?;
                for (cj, mj) in coef.iter_mut().zip(&map[i].coef) {
                    *cj = ck(cj.checked_add(ck(ai.checked_mul(*mj))?))?;
                }
            }
            Ok(Affine { coef, c: cc })
        };
        let x = self.x.iter().map(comp).collect::<XResult<Vec<_>>>()?;
        let cons = self.cons.iter().map(comp).collect::<XResult<Vec<_>>>()?;
        let cons: Vec<Affine> = cons.into_iter().filter_map(|c| normalise(&c)).collect();
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
                c: ck(a.c.checked_add(ck(cj.checked_mul(v))?))?,
            })
        };
        let mut names = self.names.clone();
        names.remove(j);
        Ok(Region {
            k: self.k - 1,
            x: self.x.iter().map(sub).collect::<XResult<_>>()?,
            cons: self
                .cons
                .iter()
                .map(sub)
                .collect::<XResult<Vec<_>>>()?
                .into_iter()
                .filter_map(|c| normalise(&c))
                .collect(),
            names,
        })
    }

    /// Add a constraint `a(t) >= 0`.
    pub fn with(&self, c: Affine) -> Region {
        let mut r = self.clone();
        if let Some(n) = normalise(&c) {
            r.cons.push(n);
        }
        r
    }

    /// An integer point (parameters) of the region, if the exact search finds one.
    pub fn point(&self, tries: usize) -> XResult<Option<Vec<i128>>> {
        find_point(&self.cons, self.k, tries, &vec![0; self.k])
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
                let mut e = Affine::constant(self.k, -1);
                e.coef[j] = s;
                cons.push(e); // s d_j >= 1
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
