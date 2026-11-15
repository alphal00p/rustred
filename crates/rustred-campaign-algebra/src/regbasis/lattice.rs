//! [`HermiteLattice`]: a lattice `Λ` with `t^s O^n ⊆ Λ ⊆ O^n` over the local ring `O = Q[t]_(t)` at d0, stored as its
//! canonical reduced Hermite form (API FOR OTHER LANES; mathematics and proofs in WS/notes/implementer.regbasis.md,
//! THEOREM 3 and HERMITE FORM OVER O).
//!
//! Representation: an upper-triangular `n x n` matrix `H` over Q[t] whose rows generate `Λ`; `H[j][j] = t^(a_j)` EXACTLY
//! (`0 <= a_j <= s`), `H[j][k] = 0` for `k < j`, and `H[j][k]` of degree `< a_k` for `k > j`. Because `t^s O^n ⊆ Λ`,
//! every input entry may be reduced modulo `t^s` (the change is an element of the lattice), so all arithmetic is exact
//! on polynomials of degree `< s`: no truncation error is possible.
//!
//! Operations: [`HermiteLattice::insert`] (streaming; returns how much the lattice grew), [`HermiteLattice::contains`],
//! [`HermiteLattice::rescale`] (multiply by `t^e`), [`HermiteLattice::equals`] (mutual containment),
//! [`HermiteLattice::smith_exponents`] (elementary divisors of `O^n / Λ` over the chain ring `O / t^s`),
//! [`HermiteLattice::length`] (`sum_j (s - a_j)` = length of `Λ / t^s O^n`).
//! Own LOGIC (pivoting, swap, reduction); every arithmetic step is a Symbolica polynomial operation (L11: Symbolica has
//! no Hermite / Smith form over a DVR, verified, notes).

use std::sync::Arc;

use symbolica::domains::rational::{Q, Rational};
use symbolica::poly::PolyVariable;

use super::series::{QP, div_t, low, mod_t, mul_t, quot_t, unit_inverse};

/// A lattice between `t^s O^n` and `O^n` in canonical reduced Hermite form.
#[derive(Clone, Debug)]
pub struct HermiteLattice {
    n: usize,
    s: u32,
    h: Vec<Vec<QP>>,
    a: Vec<u32>,
    vars: Arc<Vec<PolyVariable>>,
}

impl HermiteLattice {
    /// The lattice `t^s O^n`.
    pub fn new(n: usize, s: u32, vars: Arc<Vec<PolyVariable>>) -> HermiteLattice {
        let zero = QP::new(&Q, None, vars.clone());
        let h = (0..n)
            .map(|j| {
                (0..n)
                    .map(|k| if k == j { zero.monomial(Rational::one(), vec![s as u16]) } else { zero.clone() })
                    .collect()
            })
            .collect();
        HermiteLattice { n, s, h, a: vec![s; n], vars }
    }

    pub fn n(&self) -> usize {
        self.n
    }

    /// The socle level: `t^s O^n ⊆ Λ`.
    pub fn s(&self) -> u32 {
        self.s
    }

    /// The pivot exponents `a_j`.
    pub fn pivots(&self) -> &[u32] {
        &self.a
    }

    /// The Hermite rows (generators of the lattice).
    pub fn rows(&self) -> &[Vec<QP>] {
        &self.h
    }

    pub fn vars(&self) -> &Arc<Vec<PolyVariable>> {
        &self.vars
    }

    fn zero(&self) -> QP {
        QP::new(&Q, None, self.vars.clone())
    }

    /// The vector `t^e e_j` (a terminal's unit vector in coordinates scaled by `t^e`).
    pub fn unit(&self, j: usize, e: u32) -> Vec<QP> {
        (0..self.n)
            .map(|k| if k == j { self.zero().monomial(Rational::one(), vec![e as u16]) } else { self.zero() })
            .collect()
    }

    /// `sum_j (s - a_j)`: the length of `Λ / t^s O^n`.
    pub fn length(&self) -> u64 {
        self.a.iter().map(|&a| u64::from(self.s - a)).sum()
    }

    /// `v(det H) = sum_j a_j`.
    pub fn det_valuation(&self) -> u64 {
        self.a.iter().map(|&a| u64::from(a)).sum()
    }

    fn reduce_entries(&self, x: &mut [QP]) {
        for e in x.iter_mut() {
            *e = mod_t(e, self.s);
        }
    }

    /// x <- x - c * H[j] on columns >= j (entries reduced mod t^s).
    fn sub_row(&self, x: &mut [QP], c: &QP, j: usize) {
        for k in j..self.n {
            if !self.h[j][k].is_zero() {
                x[k] = mod_t(&(&x[k] - &(c * &self.h[j][k])), self.s);
            }
        }
    }

    /// Insert a generator `x` (entries: polynomials in t; reduced mod t^s here). Returns the growth of the lattice
    /// (the decrease of `sum_j a_j`; 0 = `x` was already in the lattice). Err on a wrong length.
    pub fn insert(&mut self, x: &[QP]) -> Result<u64, String> {
        if x.len() != self.n {
            return Err(format!("vector of length {}, lattice rank {}", x.len(), self.n));
        }
        let before = self.det_valuation();
        let mut x: Vec<QP> = x.to_vec();
        self.reduce_entries(&mut x);
        let mut changed = false;
        for j in 0..self.n {
            if x[j].is_zero() {
                continue;
            }
            let w = low(&x[j]).expect("non-zero");
            if w >= self.a[j] {
                let c = div_t(&x[j], self.a[j]);
                self.sub_row(&mut x, &c, j);
                debug_assert!(x[j].is_zero());
                x[j] = self.zero();
                continue;
            }
            // x becomes the new pivot row of column j: normalise its pivot to t^w by a unit (mod t^s)
            changed = true;
            let u = div_t(&x[j], w);
            let inv = unit_inverse(&u, self.s);
            for k in j..self.n {
                x[k] = mod_t(&(&x[k] * &inv), self.s);
            }
            x[j] = self.zero().monomial(Rational::one(), vec![w as u16]);
            let a_old = self.a[j];
            let old = std::mem::replace(&mut self.h[j], x);
            self.a[j] = w;
            // continue with old - t^(a_old - w) * new (zero in column j)
            let mut y = old;
            let c = self.zero().monomial(Rational::one(), vec![(a_old - w) as u16]);
            self.sub_row(&mut y, &c, j);
            y[j] = self.zero();
            x = y;
        }
        if changed {
            self.reduce_offdiagonal();
        }
        Ok(before - self.det_valuation())
    }

    fn reduce_offdiagonal(&mut self) {
        for k in 0..self.n {
            let ak = self.a[k];
            for j in 0..k {
                let c = quot_t(&self.h[j][k], ak);
                if c.is_zero() {
                    continue;
                }
                let row_k = self.h[k].clone();
                for m in k..self.n {
                    if !row_k[m].is_zero() {
                        self.h[j][m] = mod_t(&(&self.h[j][m] - &(&c * &row_k[m])), self.s);
                    }
                }
            }
        }
    }

    /// Membership of `x` (entries reduced mod t^s).
    pub fn contains(&self, x: &[QP]) -> bool {
        if x.len() != self.n {
            return false;
        }
        let mut x: Vec<QP> = x.to_vec();
        self.reduce_entries(&mut x);
        for j in 0..self.n {
            if x[j].is_zero() {
                continue;
            }
            let w = low(&x[j]).expect("non-zero");
            if w < self.a[j] {
                return false;
            }
            let c = div_t(&x[j], self.a[j]);
            self.sub_row(&mut x, &c, j);
        }
        x.iter().all(|e| e.is_zero())
    }

    /// Multiply the lattice by `t^e` (socle `s + e`).
    pub fn rescale(&mut self, e: u32) {
        if e == 0 {
            return;
        }
        for row in self.h.iter_mut() {
            for x in row.iter_mut() {
                *x = mul_t(x, e);
            }
        }
        for a in self.a.iter_mut() {
            *a += e;
        }
        self.s += e;
    }

    /// A copy multiplied by `t^e`.
    pub fn scaled(&self, e: u32) -> HermiteLattice {
        let mut c = self.clone();
        c.rescale(e);
        c
    }

    /// Lattice equality (mutual containment of the Hermite rows; the socles may differ).
    pub fn equals(&self, o: &HermiteLattice) -> bool {
        self.n == o.n && self.h.iter().all(|r| o.contains_exact(r)) && o.h.iter().all(|r| self.contains_exact(r))
    }

    /// Membership without reducing `x` mod this lattice's socle first when the other socle is larger (exact test of a
    /// generator of another lattice): `x` is in Λ iff its reduction mod t^s is (t^s O^n ⊆ Λ).
    fn contains_exact(&self, x: &[QP]) -> bool {
        self.contains(x)
    }

    /// Elementary divisors of `O^n / Λ`: exponents `e_1 <= .. <= e_n` (`0 <= e_k <= s`), by full pivoting on an entry of
    /// least valuation over the chain ring `O / t^s` (an entry `≡ 0 mod t^s` counts as valuation `s`). Their sum equals
    /// `sum_j a_j` (checked).
    pub fn smith_exponents(&self) -> Vec<u32> {
        let mut m: Vec<Vec<QP>> = self.h.clone();
        let mut cols: Vec<usize> = (0..self.n).collect();
        let mut out = Vec::with_capacity(self.n);
        while !m.is_empty() {
            let mut best: Option<(u32, usize, usize)> = None;
            for (i, r) in m.iter().enumerate() {
                for &c in &cols {
                    let e = mod_t(&r[c], self.s);
                    if let Some(w) = low(&e) {
                        if best.is_none_or(|(bw, _, _)| w < bw) {
                            best = Some((w, i, c));
                        }
                    }
                }
            }
            let Some((w, i, c)) = best else {
                // every remaining entry is 0 mod t^s
                out.extend(std::iter::repeat_n(self.s, m.len()));
                break;
            };
            out.push(w);
            let piv = m.swap_remove(i);
            let pc = mod_t(&piv[c], self.s);
            let u = div_t(&pc, w);
            let inv = unit_inverse(&u, self.s);
            for r in m.iter_mut() {
                let rc = mod_t(&r[c], self.s);
                if rc.is_zero() {
                    continue;
                }
                let f = div_t(&rc, w); // r_c / t^w (exact: valuation >= w)
                for &k in &cols {
                    // r <- r - (r_c / (t^w u)) * piv, i.e. f * (piv / u)
                    let pk = mod_t(&(&mod_t(&piv[k], self.s) * &inv), self.s);
                    r[k] = mod_t(&(&r[k] - &(&f * &pk)), self.s);
                }
            }
            cols.retain(|&x| x != c);
        }
        out.sort_unstable();
        debug_assert_eq!(out.iter().map(|&e| u64::from(e)).sum::<u64>(), self.det_valuation());
        out
    }

    /// Text of the Hermite rows (records): per row the entries as polynomials in t.
    pub fn rows_text(&self) -> Vec<Vec<String>> {
        self.h
            .iter()
            .map(|r| r.iter().map(|x| poly_text(x)).collect())
            .collect()
    }
}

/// Text of a polynomial in t: `c0 + c1*t + ...` with exact rational coefficients (records only).
pub fn poly_text(p: &QP) -> String {
    if p.is_zero() {
        return "0".into();
    }
    let mut parts = vec![];
    for i in 0..p.nterms() {
        let e = p.exponents(i)[0];
        let c = super::series::rational_text(&p.coefficients[i]);
        parts.push(match e {
            0 => c,
            1 => format!("({c})*t"),
            _ => format!("({c})*t^{e}"),
        });
    }
    parts.join(" + ")
}
