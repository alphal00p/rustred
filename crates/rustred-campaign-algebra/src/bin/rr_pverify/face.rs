//! Affine faces of the key lattice: an exact reduced row-echelon system `k_p = c_p + sum_f a_pf k_f` (pivots p,
//! free variables f > p), canonical (the RREF of a row space is unique), intersected with the REGION: the chart
//! (lines k >= 1, numerators k <= 0) and optionally D + r <= R. Rational feasibility of a face in the region is decided
//! exactly by Fourier-Motzkin elimination; lattice points are enumerated exactly.

use crate::rat::{Lin, Rat};

#[derive(Clone, Debug)]
pub struct Region {
    pub mask: Vec<bool>,
    pub r: Option<i128>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Face {
    pub rows: Vec<(usize, Lin)>,
}

impl Face {
    pub fn top() -> Face {
        Face { rows: vec![] }
    }

    /// Human-readable face: `n3=1, n6=2+n9, ...`.
    pub fn describe(&self) -> String {
        fn rs(r: &Rat) -> String {
            if r.d == 1 {
                format!("{}", r.n)
            } else {
                format!("{}/{}", r.n, r.d)
            }
        }
        self.rows
            .iter()
            .map(|(p, e)| {
                let mut s = format!("n{p}=");
                let mut first = true;
                if !e.c.is_zero() || e.is_const() {
                    s += &rs(&e.c);
                    first = false;
                }
                for (i, a) in e.a.iter().enumerate() {
                    if a.is_zero() {
                        continue;
                    }
                    if !first && a.n > 0 {
                        s += "+";
                    }
                    if *a == Rat::ONE {
                        s += &format!("n{i}");
                    } else if *a == Rat::int(-1) {
                        s += &format!("-n{i}");
                    } else {
                        s += &format!("{}*n{i}", rs(a));
                    }
                    first = false;
                }
                s
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn pivot_expr(&self, v: usize) -> Option<&Lin> {
        self.rows
            .binary_search_by_key(&v, |(p, _)| *p)
            .ok()
            .map(|i| &self.rows[i].1)
    }

    pub fn subst(&self, l: &Lin) -> Lin {
        let mut out = l.clone();
        for (p, e) in &self.rows {
            let a = out.a[*p];
            if !a.is_zero() {
                out.a[*p] = Rat::ZERO;
                out = out.add(&e.scale(a));
            }
        }
        out
    }

    /// The face intersected with `l = 0`; None if empty (a nonzero constant).
    pub fn add_eq(&self, l: &Lin) -> Option<Face> {
        let l = self.subst(l);
        if l.is_const() {
            return if l.c.is_zero() {
                Some(self.clone())
            } else {
                None
            };
        }
        let v = l.vars()[0];
        let a = l.a[v];
        let mut e = l.scale(Rat::int(-1).div(a));
        e.a[v] = Rat::ZERO;
        let mut rows: Vec<(usize, Lin)> = self
            .rows
            .iter()
            .map(|(p, r)| {
                let c = r.a[v];
                if c.is_zero() {
                    (*p, r.clone())
                } else {
                    let mut r2 = r.clone();
                    r2.a[v] = Rat::ZERO;
                    (*p, r2.add(&e.scale(c)))
                }
            })
            .collect();
        rows.push((v, e));
        rows.sort_by_key(|(p, _)| *p);
        Some(Face { rows })
    }

    pub fn pin(&self, n: usize, v: usize, value: i64) -> Option<Face> {
        let mut l = Lin::var(n, v);
        l.c = Rat::int(-(value as i128));
        self.add_eq(&l)
    }

    pub fn pins(&self, n: usize) -> Vec<Option<Rat>> {
        let mut out = vec![None; n];
        for (p, e) in &self.rows {
            if e.is_const() {
                out[*p] = Some(e.c);
            }
        }
        out
    }

    pub fn dim(&self, n: usize) -> usize {
        n - self.rows.len()
    }

    pub fn free_vars(&self, n: usize) -> Vec<usize> {
        (0..n).filter(|v| self.pivot_expr(*v).is_none()).collect()
    }

    /// A row without integer solutions: the face holds no lattice point. Row k_p = c + sum a_f k_f, scaled by the
    /// lcm L of its denominators: L k_p - sum (L a_f) k_f = L c has an integer solution only if the gcd of the integer
    /// coefficients divides L c (a necessary condition; a pinned non-integer coordinate is the special case).
    pub fn has_fractional_pin(&self) -> bool {
        self.rows.iter().any(|(_, e)| {
            let mut l: i128 = e.c.d;
            for a in &e.a {
                if !a.is_zero() {
                    l = l / crate::expr::gcd(l, a.d) * a.d;
                }
            }
            let mut g: i128 = l; // coefficient of k_p
            for a in &e.a {
                if !a.is_zero() {
                    g = crate::expr::gcd(g, a.n * (l / a.d));
                }
            }
            let c = e.c.n * (l / e.c.d);
            g != 0 && c % g != 0
        })
    }

    /// Region constraints as affine forms `g >= 0` in the key variables (pivots substituted).
    pub fn constraints(&self, region: &Region) -> Vec<Lin> {
        let n = region.mask.len();
        let mut out = vec![];
        let mut sum = Lin::zero(n);
        for i in 0..n {
            let k = match self.pivot_expr(i) {
                Some(e) => e.clone(),
                None => Lin::var(n, i),
            };
            // y_i = k_i - 1 (line) or -k_i (numerator), y_i >= 0
            let y = if region.mask[i] {
                let mut y = k.clone();
                y.c = y.c.sub(Rat::ONE);
                y
            } else {
                k.scale(Rat::int(-1))
            };
            sum = sum.add(&y);
            out.push(y);
        }
        if let Some(r) = region.r {
            let mut g = sum.scale(Rat::int(-1));
            g.c = g.c.add(Rat::int(r));
            out.push(g);
        }
        out
    }

    /// Exact rational feasibility of the face in the region (Fourier-Motzkin). `true` = a rational point exists.
    pub fn feasible(&self, region: &Region) -> bool {
        if self.has_fractional_pin() {
            return false;
        }
        fm_feasible(self.constraints(region), region.mask.len())
    }

    /// Implicit equalities of the region on the face: a coordinate whose excess y_i (k_i - 1 on a line, -k_i on a
    /// numerator) cannot reach 1 anywhere on the face in the region is pinned to its chart bound (exact rational
    /// infeasibility of face + region + y_i >= 1). Returns the closed face (unchanged if nothing is implied).
    pub fn close_implicit(&self, region: &Region) -> Face {
        let n = region.mask.len();
        let mut f = self.clone();
        loop {
            let mut changed = false;
            let cons = f.constraints(region);
            for i in 0..n {
                if f.pivot_expr(i).is_some_and(Lin::is_const) {
                    continue;
                }
                let mut c = cons.clone();
                let mut g = cons[i].clone();
                g.c = g.c.sub(Rat::ONE);
                c.push(g);
                if !fm_feasible(c, n) {
                    let b = if region.mask[i] { 1 } else { 0 };
                    if let Some(h) = f.pin(n, i, b) {
                        f = h;
                        changed = true;
                        break;
                    }
                }
            }
            if !changed {
                return f;
            }
        }
    }

    /// If the face has finitely many points in the CHART (empty recession cone: no nonzero direction keeps every
    /// excess y_i >= 0 on the homogeneous face), an exact upper bound of D + r over it; None if unbounded.
    pub fn chart_dr_bound(&self, mask: &[bool]) -> Option<i128> {
        let n = mask.len();
        // recession cone: rows without constants
        let h = Face {
            rows: self
                .rows
                .iter()
                .map(|(p, e)| {
                    let mut e = e.clone();
                    e.c = Rat::ZERO;
                    (*p, e)
                })
                .collect(),
        };
        let mut cons = vec![];
        let mut sum = Lin::zero(n);
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            let k = match h.pivot_expr(i) {
                Some(e) => e.clone(),
                None => Lin::var(n, i),
            };
            let y = if mask[i] { k } else { k.scale(Rat::int(-1)) };
            sum = sum.add(&y);
            cons.push(y);
        }
        let mut norm = sum;
        norm.c = norm.c.sub(Rat::ONE);
        cons.push(norm);
        if fm_feasible(cons, n) {
            return None;
        }
        // bounded: smallest power of two M with no point of D + r >= M
        let base = self.constraints(&Region {
            mask: mask.to_vec(),
            r: None,
        });
        let mut m: i128 = 1;
        while m < (1i128 << 40) {
            let mut c = base.clone();
            let mut g = Lin::zero(n);
            for y in &base {
                g = g.add(y);
            }
            g.c = g.c.sub(Rat::int(m));
            c.push(g);
            if !fm_feasible(c, n) {
                return Some(m - 1);
            }
            m *= 2;
        }
        None
    }

    /// Lower bound of D + r on the face (exact LP optimum is not needed: the pinned part, pivots >= their chart bound).
    pub fn pinned_dr(&self, region: &Region) -> i128 {
        let mut s = 0i128;
        for (p, e) in &self.rows {
            if e.is_const() && e.c.is_int() {
                s += if region.mask[*p] { e.c.n - 1 } else { -e.c.n };
            }
        }
        s
    }

    /// The first lattice points (at most `cap`) of the face in a BOUNDED region, and whether there are more.
    pub fn lattice_points_upto(&self, region: &Region, cap: usize) -> (Vec<Vec<i64>>, bool) {
        let mut out = vec![];
        let more = self.lattice_points_capped(region, cap, &mut out).is_err();
        out.truncate(cap);
        (out, more)
    }

    /// Every lattice point of the face in a BOUNDED region, or Err(count so far) when more than `cap`.
    pub fn lattice_points(&self, region: &Region, cap: usize) -> Result<Vec<Vec<i64>>, usize> {
        let mut out = vec![];
        self.lattice_points_capped(region, cap, &mut out)?;
        Ok(out)
    }

    fn lattice_points_capped(
        &self,
        region: &Region,
        cap: usize,
        out: &mut Vec<Vec<i64>>,
    ) -> Result<(), usize> {
        let n = region.mask.len();
        let r = region
            .r
            .expect("lattice enumeration needs a bounded region");
        let free = self.free_vars(n);
        let budget = r - self.pinned_dr(region);
        if budget < 0 {
            return Ok(());
        }
        let mut assign = vec![0i64; n];
        // visited assignments are capped too (most may be rejected): exceeding it counts as "more points"
        let mut visits: usize = 0;
        let visit_cap = cap.saturating_mul(200).max(2_000_000);
        #[allow(clippy::too_many_arguments)]
        fn rec(
            face: &Face,
            region: &Region,
            free: &[usize],
            idx: usize,
            left: i128,
            assign: &mut Vec<i64>,
            out: &mut Vec<Vec<i64>>,
            cap: usize,
            r: i128,
            visits: &mut usize,
            visit_cap: usize,
        ) -> Result<(), usize> {
            let n = region.mask.len();
            *visits += 1;
            if *visits > visit_cap {
                return Err(out.len());
            }
            if idx == free.len() {
                let mut k = vec![0i64; n];
                for i in 0..n {
                    match face.pivot_expr(i) {
                        Some(e) => {
                            let v = e.eval(assign);
                            if !v.is_int() {
                                return Ok(());
                            }
                            k[i] = v.n as i64;
                        }
                        None => k[i] = assign[i],
                    }
                }
                if crate::dispatch::in_chart(&region.mask, &k)
                    && crate::dispatch::dr(&region.mask, &k) <= r
                {
                    out.push(k);
                    if out.len() > cap {
                        return Err(out.len());
                    }
                }
                return Ok(());
            }
            let v = free[idx];
            for y in 0..=left {
                assign[v] = if region.mask[v] {
                    1 + y as i64
                } else {
                    -(y as i64)
                };
                rec(
                    face,
                    region,
                    free,
                    idx + 1,
                    left - y,
                    assign,
                    out,
                    cap,
                    r,
                    visits,
                    visit_cap,
                )?;
            }
            Ok(())
        }
        rec(
            self,
            region,
            &free,
            0,
            budget,
            &mut assign,
            out,
            cap,
            r,
            &mut visits,
            visit_cap,
        )?;
        Ok(())
    }
}

/// Fourier-Motzkin: is {x : g(x) >= 0 for all g} nonempty over Q? Exact. A blow-up beyond a cap answers `true`
/// (sound for pruning: only `false` is ever used to drop a face).
pub fn fm_feasible(mut cons: Vec<Lin>, n: usize) -> bool {
    const CAP: usize = 4000;
    loop {
        // constant constraints
        let mut keep = vec![];
        for g in cons {
            if g.is_const() {
                if g.c < Rat::ZERO {
                    return false;
                }
            } else {
                keep.push(g);
            }
        }
        cons = keep;
        if cons.is_empty() {
            return true;
        }
        // choose the variable minimising the product of positive and negative counts
        let mut best: Option<(usize, usize)> = None;
        for v in 0..n {
            let p = cons.iter().filter(|g| g.a[v] > Rat::ZERO).count();
            let m = cons.iter().filter(|g| g.a[v] < Rat::ZERO).count();
            if p + m == 0 {
                continue;
            }
            let cost = p * m;
            if best.is_none_or(|(_, c)| cost < c) {
                best = Some((v, cost));
            }
        }
        let Some((v, _)) = best else { return true };
        let (mut pos, mut neg, mut zero) = (vec![], vec![], vec![]);
        for g in cons {
            if g.a[v] > Rat::ZERO {
                pos.push(g)
            } else if g.a[v] < Rat::ZERO {
                neg.push(g)
            } else {
                zero.push(g)
            }
        }
        if pos.is_empty() || neg.is_empty() {
            cons = zero;
            continue;
        }
        if pos.len() * neg.len() + zero.len() > CAP {
            return true;
        }
        for p in &pos {
            for m in &neg {
                // p/a_p + m/(-a_m) >= 0 eliminates v
                let g = p
                    .scale(Rat::ONE.div(p.a[v]))
                    .add(&m.scale(Rat::ONE.div(m.a[v].neg())));
                zero.push(g);
            }
        }
        // drop duplicates
        zero.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
        zero.dedup();
        cons = zero;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn faces_and_region() {
        let n = 3;
        let region = Region {
            mask: vec![true, true, false],
            r: Some(4),
        };
        let f = Face::top();
        assert!(f.feasible(&region));
        // k0 = k1 + 3, k1 >= 1, D+r = (k0-1)+(k1-1)-k2 <= 4 -> 2 k1 + 1 - k2 <= 4, k1 = 1 ok
        let mut l = Lin::var(n, 0);
        l.a[1] = Rat::int(-1);
        l.c = Rat::int(-3);
        let g = f.add_eq(&l).unwrap();
        assert!(g.feasible(&region));
        let pts = g.lattice_points(&region, 100).unwrap();
        assert!(pts.iter().all(|k| k[0] == k[1] + 3));
        assert_eq!(pts.len(), 2); // (4,1,0),(4,1,-1); (5,2,0) has D + r = 5
        // k0 = 7 makes D + r >= 6 > 4: infeasible
        let h = g.pin(n, 0, 7).unwrap();
        assert!(!h.feasible(&region));
        // canonical: same face reached in two orders
        let a = f.pin(n, 1, 1).unwrap().pin(n, 0, 4).unwrap();
        let b = f.pin(n, 0, 4).unwrap().pin(n, 1, 1).unwrap();
        assert_eq!(a, b);
        assert_eq!(g.pin(n, 1, 1).unwrap(), a);
        // integrality: 3 k0 + 3 k1 = 7 has no integer point; 2 k0 - 4 k1 = 6 has
        let mut l2 = Lin::var(n, 0);
        l2.a[0] = Rat::int(3);
        l2.a[1] = Rat::int(3);
        l2.c = Rat::int(-7);
        assert!(!f.add_eq(&l2).unwrap().feasible(&region));
        let mut l3 = Lin::var(n, 0);
        l3.a[0] = Rat::int(2);
        l3.a[1] = Rat::int(-4);
        l3.c = Rat::int(-6);
        assert!(!f.add_eq(&l3).unwrap().has_fractional_pin());
    }
}
