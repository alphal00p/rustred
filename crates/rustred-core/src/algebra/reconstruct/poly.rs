//! Dense univariate polynomials over `Zp64`, coefficients from degree zero.
//!
//! Only what the reconstruction needs: evaluation, remainder, monic gcd,
//! exact division, multiplication, least common multiple and the
//! normalisation of a fraction. Textbook algorithms (von zur Gathen and
//! Gerhard, *Modern Computer Algebra*, ch. 2-3).

use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::{Field, RingOps};
use symbolica::prelude::Ring;

use super::Zp;

/// `sum c_i x^i` by Horner's rule.
pub fn horner(field: &Zp64, coefficients: &[Zp], x: &Zp) -> Zp {
    let mut value = field.zero();
    for coefficient in coefficients.iter().rev() {
        value = field.mul(&value, x);
        field.add_assign(&mut value, coefficient);
    }
    value
}

/// Drop leading zeros; the zero polynomial is `[0]`.
pub fn trim(field: &Zp64, coefficients: &mut Vec<Zp>) {
    while coefficients.len() > 1 && field.is_zero(coefficients.last().expect("nonempty")) {
        coefficients.pop();
    }
    if coefficients.is_empty() {
        coefficients.push(field.zero());
    }
}

pub fn is_zero(field: &Zp64, coefficients: &[Zp]) -> bool {
    coefficients.iter().all(|value| field.is_zero(value))
}

/// Degree of a trimmed polynomial (`0` for constants, including zero).
pub fn degree(coefficients: &[Zp]) -> usize {
    coefficients.len().saturating_sub(1)
}

/// Canonical `u64` representatives.
pub fn to_u64(field: &Zp64, coefficients: &[Zp]) -> Vec<u64> {
    coefficients
        .iter()
        .map(|value| field.from_element(value))
        .collect()
}

pub fn from_u64(field: &Zp64, coefficients: &[u64]) -> Vec<Zp> {
    coefficients
        .iter()
        .map(|&value| field.to_element(value % field.get_prime()))
        .collect()
}

/// `(quotient, remainder)` of `a / b`; `b` must be nonzero.
pub fn div_rem(field: &Zp64, a: &[Zp], b: &[Zp]) -> (Vec<Zp>, Vec<Zp>) {
    let mut remainder = a.to_vec();
    trim(field, &mut remainder);
    let mut divisor = b.to_vec();
    trim(field, &mut divisor);
    assert!(!is_zero(field, &divisor), "division by the zero polynomial");
    if remainder.len() < divisor.len() || is_zero(field, &remainder) {
        return (vec![field.zero()], remainder);
    }
    let lead_inverse = field.inv(divisor.last().expect("nonempty"));
    let mut quotient = vec![field.zero(); remainder.len() - divisor.len() + 1];
    for shift in (0..quotient.len()).rev() {
        let factor = field.mul(&remainder[shift + divisor.len() - 1], &lead_inverse);
        if field.is_zero(&factor) {
            continue;
        }
        quotient[shift] = factor;
        for (i, coefficient) in divisor.iter().enumerate() {
            field.sub_mul_assign(&mut remainder[shift + i], &factor, coefficient);
        }
    }
    remainder.truncate(divisor.len() - 1);
    trim(field, &mut remainder);
    trim(field, &mut quotient);
    (quotient, remainder)
}

/// Monic gcd; `gcd(0, 0) = 0`.
pub fn gcd(field: &Zp64, a: &[Zp], b: &[Zp]) -> Vec<Zp> {
    let (mut a, mut b) = (a.to_vec(), b.to_vec());
    trim(field, &mut a);
    trim(field, &mut b);
    while !is_zero(field, &b) {
        let (_, remainder) = div_rem(field, &a, &b);
        a = std::mem::replace(&mut b, remainder);
    }
    if is_zero(field, &a) {
        return a;
    }
    let inverse = field.inv(a.last().expect("nonempty"));
    a.iter().map(|value| field.mul(value, &inverse)).collect()
}

/// Product of two nonempty coefficient vectors.
pub fn mul(field: &Zp64, a: &[Zp], b: &[Zp]) -> Vec<Zp> {
    debug_assert!(!a.is_empty() && !b.is_empty());
    let mut product = vec![field.zero(); a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        if field.is_zero(x) {
            continue;
        }
        for (j, y) in b.iter().enumerate() {
            field.add_mul_assign(&mut product[i + j], x, y);
        }
    }
    trim(field, &mut product);
    product
}

/// Exact quotient; `b` divides `a`.
pub fn div_exact(field: &Zp64, a: &[Zp], b: &[Zp]) -> Vec<Zp> {
    let (quotient, remainder) = div_rem(field, a, b);
    debug_assert!(is_zero(field, &remainder), "inexact polynomial division");
    quotient
}

/// Least common multiple, normalised like a denominator (see
/// [`normalise_denominator`]).
pub fn lcm(field: &Zp64, a: &[Zp], b: &[Zp]) -> Vec<Zp> {
    let g = gcd(field, a, b);
    let mut l = mul(field, &div_exact(field, a, &g), b);
    normalise_denominator(field, &mut l);
    l
}

/// Index of the lowest nonzero coefficient (`0` for zero).
pub fn low_degree(field: &Zp64, coefficients: &[Zp]) -> usize {
    coefficients
        .iter()
        .position(|value| !field.is_zero(value))
        .unwrap_or(0)
}

/// Scale so that the lowest-degree nonzero coefficient is one.
pub fn normalise_denominator(field: &Zp64, coefficients: &mut [Zp]) {
    let low = low_degree(field, coefficients);
    if field.is_zero(&coefficients[low]) {
        return;
    }
    let inverse = field.inv(&coefficients[low]);
    for value in coefficients.iter_mut() {
        field.mul_assign(value, &inverse);
    }
}

/// `numerator / denominator` in lowest terms, with the denominator's
/// lowest-degree nonzero coefficient equal to one; zero is `0/1`. `None`
/// for a zero denominator.
///
/// This normalisation commutes with reduction modulo a prime that keeps
/// the degrees, so every lucky prime yields the image of one exact
/// representative over Q.
pub fn normalise(field: &Zp64, numerator: &[Zp], denominator: &[Zp]) -> Option<(Vec<Zp>, Vec<Zp>)> {
    if is_zero(field, denominator) {
        return None;
    }
    if is_zero(field, numerator) {
        return Some((vec![field.zero()], vec![field.one()]));
    }
    let g = gcd(field, numerator, denominator);
    let mut numerator = div_exact(field, numerator, &g);
    let mut denominator = div_exact(field, denominator, &g);
    let low = low_degree(field, &denominator);
    let inverse = field.inv(&denominator[low]);
    for value in numerator.iter_mut().chain(denominator.iter_mut()) {
        field.mul_assign(value, &inverse);
    }
    trim(field, &mut numerator);
    trim(field, &mut denominator);
    Some((numerator, denominator))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(field: &Zp64, values: &[i64]) -> Vec<Zp> {
        values
            .iter()
            .map(|&v| {
                let m = field.to_element(v.unsigned_abs());
                if v < 0 { field.neg(&m) } else { m }
            })
            .collect()
    }

    #[test]
    fn gcd_and_normalisation_cancel_common_factors() {
        let field = Zp64::new((1 << 61) - 1);
        // (x+1)(x+2) / (2x(x+1)) = (x+2)/(2x) -> normalised (x/2 + 1)/x.
        let numerator = mul(&field, &p(&field, &[1, 1]), &p(&field, &[2, 1]));
        let denominator = mul(&field, &p(&field, &[0, 2]), &p(&field, &[1, 1]));
        let (n, d) = normalise(&field, &numerator, &denominator).unwrap();
        let half = field.inv(&field.to_element(2));
        assert_eq!(n, vec![field.one(), half]);
        assert_eq!(d, vec![field.zero(), field.one()]);
        let l = lcm(&field, &p(&field, &[0, 1]), &p(&field, &[1, 1]));
        assert_eq!(l, p(&field, &[0, 1, 1]));
    }
}
