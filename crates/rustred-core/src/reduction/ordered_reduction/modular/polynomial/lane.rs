//! One prime of a polynomial replay job.
//!
//! Elements are raw Montgomery residues (`u64`, `R = 2^64`), the
//! representation of Symbolica's `Zp64` (`Fe::inner`), so values convert
//! freely at the boundaries while the hot loops never leave plain words.
//! Multiplication by a fixed scalar uses Shoup's method: for a Montgomery
//! residue `x` and a scalar `c` in standard form, `x * c mod p` is the
//! Montgomery residue of the product, computed with one high and two low
//! word products. Dense polynomials are ascending `Vec<u64>` without
//! trailing zeros.
use symbolica::domains::finite_field::Zp64;
use symbolica::domains::{Field, Ring};

use super::super::field::Fe;

/// A dense polynomial of raw residues (ascending, trimmed).
pub(super) type Raw = Vec<u64>;

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
    /// `-p^-1 mod 2^64`.
    m: u64,
}

impl Lane {
    pub fn new(prime: u64) -> Self {
        assert!(
            prime % 2 == 1 && prime < 1 << 63,
            "lane primes are odd and below 2^63"
        );
        // Newton iteration for p^-1 mod 2^64 (p * p = 1 mod 8 for odd p).
        let mut inverse = prime;
        for _ in 0..5 {
            inverse = inverse.wrapping_mul(2u64.wrapping_sub(prime.wrapping_mul(inverse)));
        }
        Self {
            field: Zp64::new(prime),
            prime,
            m: inverse.wrapping_neg(),
        }
    }

    /// `x mod p` for `x < 2p`, without a data-dependent branch (the
    /// conditional subtractions of the hot loops are unpredictable).
    #[inline(always)]
    fn reduce_once(&self, x: u64) -> u64 {
        let t = x.wrapping_sub(self.prime);
        // All ones when x < p (then t wrapped below zero; p < 2^63).
        let mask = ((t as i64) >> 63) as u64;
        t.wrapping_add(self.prime & mask)
    }

    /// Montgomery product (the same residue as `Zp64::mul`). With
    /// `p < 2^63` and reduced operands, `t + m p < 2^128` and the result
    /// before the final subtraction is below `2p`.
    #[inline(always)]
    pub fn mul(&self, a: u64, b: u64) -> u64 {
        let t = u128::from(a) * u128::from(b);
        let m = (t as u64).wrapping_mul(self.m);
        let u = ((t + u128::from(m) * u128::from(self.prime)) >> 64) as u64;
        self.reduce_once(u)
    }

    #[inline(always)]
    pub fn add(&self, a: u64, b: u64) -> u64 {
        // Both operands are below p < 2^63, so the sum does not overflow.
        self.reduce_once(a + b)
    }

    #[inline(always)]
    pub fn sub(&self, a: u64, b: u64) -> u64 {
        let t = a.wrapping_sub(b);
        let mask = ((t as i64) >> 63) as u64;
        t.wrapping_add(self.prime & mask)
    }

    #[inline(always)]
    pub fn neg(&self, a: u64) -> u64 {
        self.sub(0, a)
    }

    pub fn inv(&self, a: u64) -> u64 {
        *self.field.inv(&Fe::from_inner(a)).inner()
    }

    pub fn one(&self) -> u64 {
        *self.field.one().inner()
    }

    /// Standard representative of a residue.
    #[inline(always)]
    pub fn standard(&self, a: u64) -> u64 {
        self.mul(a, 1)
    }

    /// The Shoup form of the residue `x`.
    pub fn scalar(&self, x: u64) -> Scalar {
        let value = self.standard(x);
        Scalar {
            value,
            shoup: ((u128::from(value) << 64) / u128::from(self.prime)) as u64,
        }
    }

    /// `x * s` for any 64-bit `x`; the result is reduced.
    #[inline(always)]
    pub fn mul_scalar(&self, x: u64, s: Scalar) -> u64 {
        let q = ((u128::from(x) * u128::from(s.shoup)) >> 64) as u64;
        // Below 2p (Shoup), so one conditional subtraction.
        let r = x
            .wrapping_mul(s.value)
            .wrapping_sub(q.wrapping_mul(self.prime));
        self.reduce_once(r)
    }

    pub fn trim(polynomial: &mut Raw) {
        while polynomial.last() == Some(&0) {
            polynomial.pop();
        }
    }

    /// Product of dense polynomials.
    pub fn poly_mul(&self, a: &[u64], b: &[u64]) -> Raw {
        let mut product = vec![];
        self.poly_mul_into(a, b, &mut product);
        product
    }

    /// rr-alloc: [`Self::poly_mul`] into a reused buffer.
    pub fn poly_mul_into(&self, a: &[u64], b: &[u64], product: &mut Raw) {
        product.clear();
        if a.is_empty() || b.is_empty() {
            return;
        }
        product.resize(a.len() + b.len() - 1, 0);
        for (i, &x) in a.iter().enumerate() {
            if x == 0 {
                continue;
            }
            for (p, &y) in product[i..i + b.len()].iter_mut().zip(b) {
                *p = self.add(*p, self.mul(x, y));
            }
        }
        Self::trim(product);
    }

    /// `a + b`.
    pub fn poly_add(&self, a: &[u64], b: &[u64]) -> Raw {
        let mut sum = vec![];
        self.poly_add_into(a, b, &mut sum);
        sum
    }

    /// rr-alloc: [`Self::poly_add`] into a reused buffer.
    pub fn poly_add_into(&self, a: &[u64], b: &[u64], sum: &mut Raw) {
        let (long, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
        sum.clear();
        sum.extend_from_slice(long);
        for (s, &x) in sum.iter_mut().zip(short) {
            *s = self.add(*s, x);
        }
        Self::trim(sum);
    }

    /// `a - b`.
    pub fn poly_sub(&self, a: &[u64], b: &[u64]) -> Raw {
        let mut difference = vec![];
        self.poly_sub_into(a, b, &mut difference);
        difference
    }

    /// rr-alloc: [`Self::poly_sub`] into a reused buffer.
    pub fn poly_sub_into(&self, a: &[u64], b: &[u64], difference: &mut Raw) {
        difference.clear();
        difference.extend_from_slice(a);
        if difference.len() < b.len() {
            difference.resize(b.len(), 0);
        }
        for (d, &x) in difference.iter_mut().zip(b) {
            *d = self.sub(*d, x);
        }
        Self::trim(difference);
    }

    /// Quotient and remainder of `a` by a monic `m` (no inversion).
    pub fn divrem_monic(&self, a: &[u64], m: &[u64]) -> (Raw, Raw) {
        let (mut quotient, mut remainder) = (vec![], vec![]);
        self.divrem_monic_into(a, m, &mut quotient, &mut remainder);
        (quotient, remainder)
    }

    /// rr-alloc: [`Self::divrem_monic`] into reused buffers.
    pub fn divrem_monic_into(&self, a: &[u64], m: &[u64], quotient: &mut Raw, remainder: &mut Raw) {
        let degree = m.len() - 1;
        remainder.clear();
        remainder.extend_from_slice(a);
        quotient.clear();
        Self::trim(remainder);
        if remainder.len() <= degree {
            return;
        }
        quotient.resize(remainder.len() - degree, 0);
        for shift in (0..quotient.len()).rev() {
            let factor = remainder[shift + degree];
            quotient[shift] = factor;
            if factor == 0 {
                continue;
            }
            for (r, &c) in remainder[shift..shift + degree].iter_mut().zip(m) {
                *r = self.sub(*r, self.mul(factor, c));
            }
            remainder[shift + degree] = 0;
        }
        remainder.truncate(degree);
        Self::trim(remainder);
        Self::trim(quotient);
    }

    /// Make `polynomial` monic; returns the inverse of its former leading
    /// coefficient (one for zero).
    pub fn make_monic(&self, polynomial: &mut Raw) -> u64 {
        let Some(&lead) = polynomial.last() else {
            return self.one();
        };
        let inverse = self.inv(lead);
        for c in polynomial.iter_mut() {
            *c = self.mul(*c, inverse);
        }
        inverse
    }

    /// Quotient and remainder of `a` by a nonzero `b`.
    pub fn divrem(&self, a: &[u64], b: &[u64]) -> (Raw, Raw) {
        let mut monic = b.to_vec();
        let inverse = self.make_monic(&mut monic);
        let (mut quotient, remainder) = self.divrem_monic(a, &monic);
        for c in quotient.iter_mut() {
            *c = self.mul(*c, inverse);
        }
        (quotient, remainder)
    }

    /// Monic gcd.
    pub fn gcd(&self, a: &[u64], b: &[u64]) -> Raw {
        let mut a = a.to_vec();
        let mut b = b.to_vec();
        Self::trim(&mut a);
        Self::trim(&mut b);
        while !b.is_empty() {
            let (_, remainder) = self.divrem(&a, &b);
            a = std::mem::replace(&mut b, remainder);
        }
        self.make_monic(&mut a);
        a
    }

    /// `f^e`.
    pub fn poly_pow(&self, f: &[u64], exponent: usize) -> Raw {
        let mut power = vec![self.one()];
        for _ in 0..exponent {
            power = self.poly_mul(&power, f);
        }
        power
    }

    /// The inverse of `a` modulo `m` (`deg m >= 1`), if `gcd(a, m) = 1`.
    pub fn poly_inverse_mod(&self, a: &[u64], m: &[u64]) -> Option<Raw> {
        // Extended Euclid tracking the cofactor of `a`.
        let mut r0 = m.to_vec();
        let mut r1 = self.divrem(a, m).1;
        let mut t0: Raw = vec![];
        let mut t1: Raw = vec![self.one()];
        while !r1.is_empty() {
            let (quotient, remainder) = self.divrem(&r0, &r1);
            let t2 = self.poly_sub(&t0, &self.poly_mul(&quotient, &t1));
            r0 = std::mem::replace(&mut r1, remainder);
            t0 = std::mem::replace(&mut t1, t2);
        }
        // r0 = gcd up to a unit; invertible only when it is a constant.
        if r0.len() != 1 {
            return None;
        }
        let unit = self.inv(r0[0]);
        let inverse: Raw = t0.iter().map(|&c| self.mul(c, unit)).collect();
        Some(self.divrem(&inverse, m).1)
    }

    /// Standard representatives (the form of a lift image).
    pub fn standard_poly(&self, polynomial: &[u64]) -> Vec<u64> {
        polynomial.iter().map(|&c| self.standard(c)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbolica::domains::RingOps;
    use symbolica::domains::finite_field::FiniteFieldCore;

    const P: u64 = 4_611_686_018_427_388_039; // 2^62 + 135, prime

    fn e(lane: &Lane, v: i64) -> u64 {
        let p = lane.prime as i64;
        *lane.field.to_element(v.rem_euclid(p) as u64).inner()
    }

    #[test]
    fn products_agree_with_symbolica() {
        for prime in [P, 7, 1_000_003, (1 << 63) - 25] {
            let lane = Lane::new(prime);
            for (x, c) in [
                (0, 0),
                (1, 1),
                (-1, -1),
                (123_456_789, -987_654_321),
                (-1, 2),
                (2, -3),
            ] {
                let (x, c) = (e(&lane, x), e(&lane, c));
                let (fx, fc) = (Fe::from_inner(x), Fe::from_inner(c));
                let expected = *lane.field.mul(&fx, &fc).inner();
                assert_eq!(lane.mul(x, c), expected);
                assert_eq!(lane.mul_scalar(x, lane.scalar(c)), expected);
                assert_eq!(lane.add(x, c), *lane.field.add(&fx, &fc).inner());
                assert_eq!(lane.sub(x, c), *lane.field.sub(&fx, &fc).inner());
                assert_eq!(lane.neg(x), *lane.field.neg(&fx).inner());
            }
        }
    }

    #[test]
    fn inverses_modulo_a_polynomial() {
        let lane = Lane::new(P);
        // m = d^2 + 1, a = d + 2: (d + 2)(-d + 2)/5 = (4 - d^2)/5 = 1 mod m.
        let m = vec![e(&lane, 1), e(&lane, 0), e(&lane, 1)];
        let a = vec![e(&lane, 2), e(&lane, 1)];
        let inverse = lane.poly_inverse_mod(&a, &m).unwrap();
        assert_eq!(
            lane.divrem(&lane.poly_mul(&a, &inverse), &m).1,
            vec![lane.one()]
        );
        // (d - 1)(d + 1) has no inverse modulo d^2 - 1.
        let m = vec![e(&lane, -1), e(&lane, 0), e(&lane, 1)];
        assert!(
            lane.poly_inverse_mod(&[e(&lane, -1), e(&lane, 1)], &m)
                .is_none()
        );
        // gcd((d-1)(d-2), (d-1)(d+5)) = d - 1.
        let a = vec![e(&lane, 2), e(&lane, -3), e(&lane, 1)];
        let b = vec![e(&lane, -5), e(&lane, 4), e(&lane, 1)];
        assert_eq!(lane.gcd(&a, &b), vec![e(&lane, -1), e(&lane, 1)]);
    }
}
