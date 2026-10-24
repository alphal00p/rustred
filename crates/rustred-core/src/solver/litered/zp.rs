//! Word-sized prime fields, sample points and polynomials compiled at a point.
//!
//! Exact data never changes here: a compiled polynomial is the image of one
//! exact RustRed polynomial after substituting a sample point for the base
//! parameters (`d`, `eta`, ...). Index variables stay symbolic and are
//! evaluated per integral. A zero image is never trusted as an exact zero;
//! callers confirm exact zeros with the exact specialization.

use std::collections::HashMap;

use symbolica::domains::finite_field::{FiniteFieldCore, ToFiniteField, Zp64};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::LiteRedError;

/// The Mersenne prime `2^61 - 1`, the default structure-building prime.
pub const DEFAULT_PRIME: u64 = (1 << 61) - 1;

/// A prime field `Z/pZ` with `3 <= p < 2^63`, using `u128` products.
#[derive(Clone, Debug)]
pub struct PrimeField {
    p: u64,
    native: Zp64,
}

impl PartialEq for PrimeField {
    fn eq(&self, other: &Self) -> bool {
        self.p == other.p
    }
}

impl PrimeField {
    pub fn new(p: u64) -> Result<Self, LiteRedError> {
        if !(3..(1u64 << 63)).contains(&p) || !is_prime_u64(p) {
            return Err(LiteRedError::InvalidInput(format!(
                "{p} is not an odd prime below 2^63"
            )));
        }
        Ok(Self {
            p,
            native: Zp64::new(p),
        })
    }

    #[inline]
    pub fn prime(&self) -> u64 {
        self.p
    }
    #[inline]
    pub fn add(&self, a: u64, b: u64) -> u64 {
        let s = a + b;
        if s >= self.p { s - self.p } else { s }
    }
    #[inline]
    pub fn sub(&self, a: u64, b: u64) -> u64 {
        if a >= b { a - b } else { a + self.p - b }
    }
    #[inline]
    pub fn neg(&self, a: u64) -> u64 {
        if a == 0 { 0 } else { self.p - a }
    }
    #[inline]
    pub fn mul(&self, a: u64, b: u64) -> u64 {
        ((u128::from(a) * u128::from(b)) % u128::from(self.p)) as u64
    }
    /// `acc + a * b`.
    #[inline]
    pub fn mul_add(&self, acc: u64, a: u64, b: u64) -> u64 {
        ((u128::from(a) * u128::from(b) + u128::from(acc)) % u128::from(self.p)) as u64
    }
    pub fn pow(&self, mut base: u64, mut exponent: u64) -> u64 {
        let mut result = 1 % self.p;
        while exponent > 0 {
            if exponent & 1 == 1 {
                result = self.mul(result, base);
            }
            base = self.mul(base, base);
            exponent >>= 1;
        }
        result
    }
    /// Multiplicative inverse; `None` for zero.
    pub fn inv(&self, a: u64) -> Option<u64> {
        if a == 0 {
            return None;
        }
        let (mut t, mut new_t) = (0i128, 1i128);
        let (mut r, mut new_r) = (i128::from(self.p), i128::from(a));
        while new_r != 0 {
            let q = r / new_r;
            (t, new_t) = (new_t, t - q * new_t);
            (r, new_r) = (new_r, r - q * new_r);
        }
        debug_assert_eq!(r, 1);
        Some(t.rem_euclid(i128::from(self.p)) as u64)
    }
    #[inline]
    pub fn from_i64(&self, value: i64) -> u64 {
        i128::from(value).rem_euclid(i128::from(self.p)) as u64
    }
    pub fn from_integer(&self, value: &Integer) -> u64 {
        let element = value.to_finite_field(&self.native);
        self.native.from_element(&element)
    }
    /// Image of an exact rational; `None` when its denominator vanishes mod p.
    pub fn from_rational(&self, value: &Rational) -> Option<u64> {
        let numerator = self.from_integer(value.numerator_ref());
        let denominator = self.from_integer(value.denominator_ref());
        self.inv(denominator)
            .map(|inverse| self.mul(numerator, inverse))
    }
}

/// Deterministic Miller-Rabin for 64-bit integers.
pub fn is_prime_u64(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    for p in [2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        if n.is_multiple_of(p) {
            return n == p;
        }
    }
    let mul = |a: u64, b: u64| ((u128::from(a) * u128::from(b)) % u128::from(n)) as u64;
    let pow = |mut b: u64, mut e: u64| {
        let mut r = 1u64;
        while e > 0 {
            if e & 1 == 1 {
                r = mul(r, b);
            }
            b = mul(b, b);
            e >>= 1;
        }
        r
    };
    let (mut d, mut s) = (n - 1, 0);
    while d % 2 == 0 {
        d /= 2;
        s += 1;
    }
    'witness: for a in [2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        let mut x = pow(a, d);
        if x == 1 || x == n - 1 {
            continue;
        }
        for _ in 1..s {
            x = mul(x, x);
            if x == n - 1 {
                continue 'witness;
            }
        }
        return false;
    }
    true
}

/// Deterministic descending sequence of 62-bit primes, distinct from
/// [`DEFAULT_PRIME`], used for multi-prime reconstruction.
pub fn reconstruction_primes(count: usize) -> Vec<u64> {
    let mut primes = Vec::with_capacity(count);
    let mut candidate = (1u64 << 62) - 1;
    while primes.len() < count {
        if is_prime_u64(candidate) {
            primes.push(candidate);
        }
        candidate -= 2;
    }
    primes
}

/// One modular sample: a prime and the images of every base parameter, in
/// the family coefficient context's parameter order.
#[derive(Clone, Debug, PartialEq)]
pub struct ZpSample {
    pub field: PrimeField,
    pub base: Vec<u64>,
}

impl ZpSample {
    pub fn new(field: PrimeField, base: Vec<u64>) -> Self {
        Self { field, base }
    }

    /// Image of a rational sample point; `None` if a denominator vanishes.
    pub fn from_rationals(field: PrimeField, point: &[Rational]) -> Option<Self> {
        let base = point
            .iter()
            .map(|value| field.from_rational(value))
            .collect::<Option<Vec<_>>>()?;
        Some(Self { field, base })
    }

    /// Image of an exact base-field coefficient; `None` for a vanishing
    /// denominator (an unlucky or singular sample for this coefficient).
    pub fn coefficient(&self, value: &Coefficient) -> Result<Option<u64>, LiteRedError> {
        let numerator = ZpIndexPolynomial::compile(&value.numerator, self, 0)?;
        let denominator = ZpIndexPolynomial::compile(&value.denominator, self, 0)?;
        let numerator = numerator.evaluate(&[], &self.field);
        let denominator = denominator.evaluate(&[], &self.field);
        Ok(self
            .field
            .inv(denominator)
            .map(|inverse| self.field.mul(numerator, inverse)))
    }
}

/// A polynomial in the integral indices with Zp coefficients, obtained by
/// substituting a sample for every base parameter of an exact polynomial.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ZpIndexPolynomial {
    /// `(coefficient, first factor, factor count)` per monomial.
    terms: Vec<(u64, u32, u32)>,
    /// `(index position, exponent)` factors.
    factors: Vec<(u16, u16)>,
}

impl ZpIndexPolynomial {
    /// Compile an exact polynomial on `base ++ index` variables. The variable
    /// map must have exactly `sample.base.len() + index_count` variables.
    pub fn compile(
        polynomial: &CoefficientPolynomial,
        sample: &ZpSample,
        index_count: usize,
    ) -> Result<Self, LiteRedError> {
        let nbase = sample.base.len();
        let nvars = polynomial.nvars();
        if nvars != nbase + index_count {
            return Err(LiteRedError::InvalidInput(format!(
                "polynomial has {nvars} variables, expected {nbase} base + {index_count} index"
            )));
        }
        let field = &sample.field;
        let mut merged: HashMap<Vec<(u16, u16)>, u64> = HashMap::new();
        let mut order: Vec<Vec<(u16, u16)>> = Vec::new();
        for term in 0..polynomial.nterms() {
            let exponents = &polynomial.exponents[term * nvars..(term + 1) * nvars];
            let mut value = field.from_integer(&polynomial.coefficients[term]);
            for (position, &exponent) in exponents[..nbase].iter().enumerate() {
                if exponent > 0 {
                    value = field.mul(value, field.pow(sample.base[position], u64::from(exponent)));
                }
            }
            if value == 0 {
                continue;
            }
            let key: Vec<(u16, u16)> = exponents[nbase..]
                .iter()
                .enumerate()
                .filter(|(_, exponent)| **exponent > 0)
                .map(|(position, &exponent)| (position as u16, exponent))
                .collect();
            match merged.get_mut(&key) {
                Some(old) => *old = field.add(*old, value),
                None => {
                    order.push(key.clone());
                    merged.insert(key, value);
                }
            }
        }
        let mut compiled = Self::default();
        for key in order {
            let value = merged[&key];
            if value == 0 {
                continue;
            }
            compiled
                .terms
                .push((value, compiled.factors.len() as u32, key.len() as u32));
            compiled.factors.extend(key);
        }
        Ok(compiled)
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    /// Value at an index point given as residues mod p.
    #[inline]
    pub fn evaluate(&self, indices: &[u64], field: &PrimeField) -> u64 {
        let mut total = 0u64;
        for &(coefficient, start, count) in &self.terms {
            let mut value = coefficient;
            for &(position, exponent) in &self.factors[start as usize..(start + count) as usize] {
                let base = indices[position as usize];
                let mut power = base;
                for _ in 1..exponent {
                    power = field.mul(power, base);
                }
                value = field.mul(value, power);
            }
            total = field.add(total, value);
        }
        total
    }

    pub fn monomials(&self) -> usize {
        self.terms.len()
    }
}
