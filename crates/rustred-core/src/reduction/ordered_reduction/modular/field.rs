//! Small prime-field helpers: Montgomery elements of Symbolica's `Zp64`,
//! batched inversion, dense univariate polynomials and exact coefficient
//! evaluation.
use symbolica::domains::finite_field::{FiniteFieldCore, FiniteFieldElement, ToFiniteField, Zp64};
use symbolica::domains::{Field, Ring, RingOps};
use symbolica::prelude::Integer;

use crate::algebra::Coefficient;

pub(super) type Fe = FiniteFieldElement<u64>;

/// Exact univariate rational function in dense ascending form. Constant
/// coefficients use one-element vectors.
#[derive(Clone, Debug)]
pub(super) struct DenseRational {
    pub numerator: Vec<Integer>,
    pub denominator: Vec<Integer>,
}

impl DenseRational {
    /// The coefficient context has at most one variable (checked by the caller).
    pub fn from_coefficient(coefficient: &Coefficient) -> Self {
        Self {
            numerator: dense(&coefficient.numerator),
            denominator: dense(&coefficient.denominator),
        }
    }

    pub fn is_constant(&self) -> bool {
        self.numerator.len() <= 1 && self.denominator.len() <= 1
    }
}

fn dense(polynomial: &crate::algebra::CoefficientPolynomial) -> Vec<Integer> {
    let mut result = Vec::new();
    for (index, coefficient) in polynomial.coefficients.iter().enumerate() {
        let degree = polynomial.exponents(index).first().copied().unwrap_or(0) as usize;
        if result.len() <= degree {
            result.resize(degree + 1, Integer::zero());
        }
        result[degree] = coefficient.clone();
    }
    if result.is_empty() {
        result.push(Integer::zero());
    }
    result
}

#[inline(always)]
pub(super) fn reduce(field: &Zp64, value: &Integer) -> Fe {
    value.to_finite_field(field)
}

/// Horner evaluation of reduced dense coefficients (ascending order).
#[inline]
pub(super) fn horner(field: &Zp64, coefficients: &[Fe], x: Fe) -> Fe {
    let mut value = field.zero();
    for coefficient in coefficients.iter().rev() {
        value = field.add(&field.mul(&value, &x), coefficient);
    }
    value
}

/// Replace every nonzero element by its inverse with one field inversion.
/// Zero entries are left untouched and reported through `on_zero`.
pub(super) fn batch_invert(field: &Zp64, values: &mut [Fe], mut on_zero: impl FnMut(usize)) {
    let one = field.one();
    let mut prefix = Vec::with_capacity(values.len());
    let mut accumulator = one;
    for (index, value) in values.iter().enumerate() {
        prefix.push(accumulator);
        if field.is_zero(value) {
            on_zero(index);
        } else {
            accumulator = field.mul(&accumulator, value);
        }
    }
    let mut inverse = field.inv(&accumulator);
    for index in (0..values.len()).rev() {
        if field.is_zero(&values[index]) {
            continue;
        }
        let value = values[index];
        values[index] = field.mul(&inverse, &prefix[index]);
        inverse = field.mul(&inverse, &value);
    }
}

/// Dense polynomial over a prime field, ascending, without trailing zeros.
pub(super) type Poly = Vec<Fe>;

pub(super) fn trim(field: &Zp64, polynomial: &mut Poly) {
    while polynomial.last().is_some_and(|c| field.is_zero(c)) {
        polynomial.pop();
    }
}

pub(super) fn eval(field: &Zp64, polynomial: &Poly, x: Fe) -> Fe {
    horner(field, polynomial, x)
}

/// `a * s + (x - c) * b`
pub(super) fn thiele_step(field: &Zp64, a: &Poly, s: Fe, c: Fe, b: &Poly) -> Poly {
    let length = a.len().max(b.len() + 1);
    let mut result = vec![field.zero(); length];
    for (index, coefficient) in a.iter().enumerate() {
        result[index] = field.mul(coefficient, &s);
    }
    for (index, coefficient) in b.iter().enumerate() {
        result[index + 1] = field.add(&result[index + 1], coefficient);
        result[index] = field.sub(&result[index], &field.mul(coefficient, &c));
    }
    trim(field, &mut result);
    result
}

/// Remainder and quotient of `a / b` for nonzero `b`.
pub(super) fn quot_rem(field: &Zp64, a: &Poly, b: &Poly) -> (Poly, Poly) {
    let mut remainder = a.clone();
    trim(field, &mut remainder);
    if remainder.len() < b.len() {
        return (vec![], remainder);
    }
    let lead_inverse = field.inv(b.last().expect("nonzero divisor"));
    let mut quotient = vec![field.zero(); remainder.len() - b.len() + 1];
    while remainder.len() >= b.len() && !remainder.is_empty() {
        let shift = remainder.len() - b.len();
        let factor = field.mul(remainder.last().unwrap(), &lead_inverse);
        quotient[shift] = factor;
        for (index, coefficient) in b.iter().enumerate() {
            remainder[shift + index] =
                field.sub(&remainder[shift + index], &field.mul(&factor, coefficient));
        }
        trim(field, &mut remainder);
    }
    trim(field, &mut quotient);
    (quotient, remainder)
}

pub(super) fn gcd(field: &Zp64, a: &Poly, b: &Poly) -> Poly {
    let mut a = a.clone();
    let mut b = b.clone();
    trim(field, &mut a);
    trim(field, &mut b);
    while !b.is_empty() {
        let (_, remainder) = quot_rem(field, &a, &b);
        a = b;
        b = remainder;
    }
    make_monic(field, &mut a);
    a
}

pub(super) fn make_monic(field: &Zp64, polynomial: &mut Poly) {
    if let Some(lead) = polynomial.last().copied() {
        let inverse = field.inv(&lead);
        for coefficient in polynomial.iter_mut() {
            *coefficient = field.mul(&*coefficient, &inverse);
        }
    }
}

/// Standard representative in `0..p`.
#[inline]
pub(super) fn to_u64(field: &Zp64, value: &Fe) -> u64 {
    field.from_element(value)
}

pub(super) fn integer_from_fe(field: &Zp64, value: &Fe) -> Integer {
    Integer::from(to_u64(field, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_inversion_and_gcd() {
        let field = Zp64::new(1_000_000_007);
        let mut values: Vec<Fe> = [3u64, 0, 5, 7].iter().map(|v| field.to_element(*v)).collect();
        let mut zeros = vec![];
        batch_invert(&field, &mut values, |i| zeros.push(i));
        assert_eq!(zeros, [1]);
        assert_eq!(field.mul(&values[0], &field.to_element(3)), field.one());
        assert_eq!(field.mul(&values[3], &field.to_element(7)), field.one());
        // (x-1)(x-2) and (x-1)(x+5)
        let e = |v: i64| field.to_element(v.rem_euclid(1_000_000_007) as u64);
        let a = vec![e(2), e(-3), e(1)];
        let b = vec![e(-5), e(4), e(1)];
        assert_eq!(gcd(&field, &a, &b), vec![e(-1), e(1)]);
    }
}
