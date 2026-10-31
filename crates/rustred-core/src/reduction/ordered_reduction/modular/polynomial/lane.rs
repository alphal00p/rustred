//! One prime of a polynomial replay job.
//!
//! Elements are Montgomery residues of Symbolica's `Zp64` (`R = 2^64`), as in
//! the sampled replay. Multiplication by a fixed scalar uses Shoup's method:
//! for a Montgomery residue `x` and a scalar `c` in standard form,
//! `x * c mod p` is the Montgomery residue of the product, computed with one
//! high and two low word products. The dense polynomial helpers complete those
//! of `field.rs` for the nonlinear-factor paths and the final images.
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::{Field, Ring, RingOps};

use super::super::field::{self, Fe, Poly};

/// A fixed multiplier in standard form with its Shoup quotient
/// `floor(value * 2^64 / p)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Scalar {
    value: u64,
    shoup: u64,
}

/// Arithmetic modulo one odd prime below `2^63`.
#[derive(Clone, Debug)]
pub(super) struct Lane {
    pub field: Zp64,
    prime: u64,
}

impl Lane {
    pub fn new(prime: u64) -> Self {
        debug_assert!(prime % 2 == 1 && prime < 1 << 63);
        Self {
            field: Zp64::new(prime),
            prime,
        }
    }

    /// The Shoup form of the Montgomery residue `x`.
    pub fn scalar(&self, x: Fe) -> Scalar {
        let value = self.field.from_element(&x);
        Scalar {
            value,
            shoup: ((u128::from(value) << 64) / u128::from(self.prime)) as u64,
        }
    }

    /// `x * s`, for any 64-bit `x`; the result is reduced.
    #[inline(always)]
    fn mul_raw(&self, x: u64, s: Scalar) -> u64 {
        let q = ((u128::from(x) * u128::from(s.shoup)) >> 64) as u64;
        let r = x
            .wrapping_mul(s.value)
            .wrapping_sub(q.wrapping_mul(self.prime));
        if r >= self.prime { r - self.prime } else { r }
    }

    #[inline(always)]
    fn add_raw(&self, a: u64, b: u64) -> u64 {
        // Both operands are below p < 2^63, so the sum does not overflow.
        let sum = a + b;
        if sum >= self.prime {
            sum - self.prime
        } else {
            sum
        }
    }

    #[inline(always)]
    pub fn mul_scalar(&self, x: Fe, s: Scalar) -> Fe {
        Fe::from_inner(self.mul_raw(*x.inner(), s))
    }

    /// `destination += s * source`.
    #[inline]
    pub fn axpy(&self, destination: &mut [Fe], source: &[Fe], s: Scalar) {
        for (d, x) in destination.iter_mut().zip(source) {
            *d = Fe::from_inner(self.add_raw(*d.inner(), self.mul_raw(*x.inner(), s)));
        }
    }

    /// `destination += source`.
    #[inline]
    pub fn add_into(&self, destination: &mut [Fe], source: &[Fe]) {
        for (d, x) in destination.iter_mut().zip(source) {
            *d = Fe::from_inner(self.add_raw(*d.inner(), *x.inner()));
        }
    }

    /// `destination = s * source`.
    #[inline]
    pub fn scale_into(&self, destination: &mut [Fe], source: &[Fe], s: Scalar) {
        for (d, x) in destination.iter_mut().zip(source) {
            *d = Fe::from_inner(self.mul_raw(*x.inner(), s));
        }
    }

    pub fn add(&self, a: Fe, b: Fe) -> Fe {
        self.field.add(&a, &b)
    }

    pub fn sub(&self, a: Fe, b: Fe) -> Fe {
        self.field.sub(&a, &b)
    }

    pub fn mul(&self, a: Fe, b: Fe) -> Fe {
        self.field.mul(&a, &b)
    }

    pub fn neg(&self, a: Fe) -> Fe {
        self.field.neg(&a)
    }

    pub fn inv(&self, a: Fe) -> Fe {
        self.field.inv(&a)
    }

    pub fn is_zero(a: &Fe) -> bool {
        *a.inner() == 0
    }

    pub fn zero(&self) -> Fe {
        self.field.zero()
    }

    pub fn one(&self) -> Fe {
        self.field.one()
    }

    /// Product of dense polynomials (ascending).
    pub fn poly_mul(&self, a: &[Fe], b: &[Fe]) -> Poly {
        if a.is_empty() || b.is_empty() {
            return vec![];
        }
        let mut product = vec![self.zero(); a.len() + b.len() - 1];
        for (i, x) in a.iter().enumerate() {
            if Self::is_zero(x) {
                continue;
            }
            let s = self.scalar(*x);
            self.axpy(&mut product[i..i + b.len()], b, s);
        }
        field::trim(&self.field, &mut product);
        product
    }

    /// `a - b` (dense, trimmed).
    pub fn poly_sub(&self, a: &[Fe], b: &[Fe]) -> Poly {
        let mut difference = a.to_vec();
        if difference.len() < b.len() {
            difference.resize(b.len(), self.zero());
        }
        for (d, x) in difference.iter_mut().zip(b) {
            *d = self.sub(*d, *x);
        }
        field::trim(&self.field, &mut difference);
        difference
    }

    /// `a + b` (dense, trimmed).
    pub fn poly_add(&self, a: &[Fe], b: &[Fe]) -> Poly {
        let mut sum = a.to_vec();
        if sum.len() < b.len() {
            sum.resize(b.len(), self.zero());
        }
        self.add_into(&mut sum[..b.len()], b);
        field::trim(&self.field, &mut sum);
        sum
    }

    /// Quotient and remainder of `a` by a nonzero `b`.
    pub fn poly_quot_rem(&self, a: &[Fe], b: &[Fe]) -> (Poly, Poly) {
        field::quot_rem(&self.field, &a.to_vec(), &b.to_vec())
    }

    pub fn poly_rem(&self, a: &[Fe], b: &[Fe]) -> Poly {
        self.poly_quot_rem(a, b).1
    }

    /// `f^e` (dense).
    pub fn poly_pow(&self, f: &[Fe], exponent: usize) -> Poly {
        let mut power = vec![self.one()];
        for _ in 0..exponent {
            power = self.poly_mul(&power, f);
        }
        power
    }

    /// The inverse of `a` modulo `m` (`deg m >= 1`), if `gcd(a, m) = 1`.
    pub fn poly_inverse_mod(&self, a: &[Fe], m: &[Fe]) -> Option<Poly> {
        // Extended Euclid tracking the cofactor of `a`.
        let mut r0 = m.to_vec();
        let mut r1 = self.poly_rem(a, m);
        let mut t0: Poly = vec![];
        let mut t1: Poly = vec![self.one()];
        while !r1.is_empty() {
            let (quotient, remainder) = self.poly_quot_rem(&r0, &r1);
            let t2 = self.poly_sub(&t0, &self.poly_mul(&quotient, &t1));
            r0 = std::mem::replace(&mut r1, remainder);
            t0 = std::mem::replace(&mut t1, t2);
        }
        // r0 = gcd up to a unit; invertible only when it is a constant.
        if r0.len() != 1 {
            return None;
        }
        let unit = self.inv(r0[0]);
        let s = self.scalar(unit);
        let mut inverse = t0;
        for c in inverse.iter_mut() {
            *c = self.mul_scalar(*c, s);
        }
        Some(self.poly_rem(&inverse, m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: u64 = 4_611_686_018_427_388_039; // 2^62 + 135, prime

    fn lane() -> Lane {
        Lane::new(P)
    }

    fn e(lane: &Lane, v: i64) -> Fe {
        lane.field.to_element(v.rem_euclid(P as i64) as u64)
    }

    #[test]
    fn shoup_products_agree_with_montgomery_products() {
        let lane = lane();
        for (x, c) in [
            (0, 0),
            (1, 1),
            (P as i64 - 1, P as i64 - 1),
            (123_456_789, -987_654_321),
            (-1, 2),
        ] {
            let (x, c) = (e(&lane, x), e(&lane, c));
            assert_eq!(lane.mul_scalar(x, lane.scalar(c)), lane.mul(x, c));
        }
        let mut destination = vec![e(&lane, 5), e(&lane, -7)];
        lane.axpy(
            &mut destination,
            &[e(&lane, 3), e(&lane, 4)],
            lane.scalar(e(&lane, -2)),
        );
        assert_eq!(destination, vec![e(&lane, -1), e(&lane, -15)]);
    }

    #[test]
    fn inverses_modulo_a_polynomial() {
        let lane = lane();
        // m = d^2 + 1, a = d + 2: (d + 2)(-d + 2)/5 = (4 - d^2)/5 = 1 mod m.
        let m = vec![e(&lane, 1), e(&lane, 0), e(&lane, 1)];
        let a = vec![e(&lane, 2), e(&lane, 1)];
        let inverse = lane.poly_inverse_mod(&a, &m).unwrap();
        assert_eq!(
            lane.poly_rem(&lane.poly_mul(&a, &inverse), &m),
            vec![lane.one()]
        );
        // (d - 1)(d + 1) has no inverse modulo d^2 - 1.
        let m = vec![e(&lane, -1), e(&lane, 0), e(&lane, 1)];
        assert!(
            lane.poly_inverse_mod(&[e(&lane, -1), e(&lane, 1)], &m)
                .is_none()
        );
    }
}
