//! Univariate rational functions over Q, their images modulo primes, and
//! rational reconstruction of integers modulo `m`.
//!
//! - Wang's algorithm (P. S. Wang, SYMSAC 1981; FireFly arXiv:1904.00009
//!   §2.3 Alg. 1): `n/d = v mod m` with `|n|, d <= sqrt(m/2)`.
//! - Monagan's maximal-quotient rational reconstruction (ISSAC 2004;
//!   FireFly §2.3 Alg. 4) via Symbolica's
//!   `Rational::maximal_quotient_reconstruction`.

use symbolica::domains::RingOps;
use symbolica::domains::finite_field::{ToFiniteField, Zp64};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;
use symbolica::prelude::Ring;

use super::Zp;
use super::poly::horner;

/// `numerator(x) / denominator(x)` over Q, dense from degree zero, in lowest
/// terms, with the denominator's lowest-degree nonzero coefficient one.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RationalFunction {
    pub numerator: Vec<Rational>,
    pub denominator: Vec<Rational>,
}

impl RationalFunction {
    pub fn zero() -> Self {
        Self::constant(Rational::zero())
    }

    pub fn constant(value: Rational) -> Self {
        Self {
            numerator: vec![value],
            denominator: vec![Rational::one()],
        }
    }

    pub fn is_zero(&self) -> bool {
        self.numerator.iter().all(Rational::is_zero)
    }

    pub fn is_polynomial(&self) -> bool {
        self.denominator.len() == 1
    }

    /// `(numerator degree, denominator degree)`.
    pub fn degrees(&self) -> (usize, usize) {
        (self.numerator.len() - 1, self.denominator.len() - 1)
    }

    /// Image modulo `field`'s prime; `None` if a coefficient's denominator
    /// vanishes there.
    pub fn image(&self, field: &Zp64) -> Option<(Vec<Zp>, Vec<Zp>)> {
        let map = |values: &[Rational]| -> Option<Vec<Zp>> {
            values
                .iter()
                .map(|value| rational_image(value, field))
                .collect()
        };
        Some((map(&self.numerator)?, map(&self.denominator)?))
    }

    /// Value at a Zp point; `None` at a pole or for a prime dividing a
    /// coefficient denominator.
    pub fn evaluate(&self, field: &Zp64, x: &Zp) -> Option<Zp> {
        let (numerator, denominator) = self.image(field)?;
        let d = horner(field, &denominator, x);
        let inverse = field.try_inv(&d)?;
        Some(field.mul(&horner(field, &numerator, x), &inverse))
    }

    /// Exact value at a rational point; `None` at a pole.
    pub fn evaluate_rational(&self, x: &Rational) -> Option<Rational> {
        let horner = |coefficients: &[Rational]| {
            let mut value = Rational::zero();
            for coefficient in coefficients.iter().rev() {
                value = &(&value * x) + coefficient;
            }
            value
        };
        let d = horner(&self.denominator);
        if d.is_zero() {
            return None;
        }
        Some(&horner(&self.numerator) / &d)
    }

    /// `(numerator, denominator)` as strings in `variable`, for exchange
    /// formats (integers, `variable`, `+ - * / ^`).
    pub fn format_parts(&self, variable: &str) -> (String, String) {
        (
            format_polynomial(&self.numerator, variable),
            format_polynomial(&self.denominator, variable),
        )
    }

    /// The function as one string in `variable`.
    pub fn format(&self, variable: &str) -> String {
        let (numerator, denominator) = self.format_parts(variable);
        if self.is_polynomial() && self.denominator[0].is_one() {
            numerator
        } else {
            format!("({numerator})/({denominator})")
        }
    }
}

fn format_polynomial(coefficients: &[Rational], variable: &str) -> String {
    let mut out = String::new();
    for (degree, coefficient) in coefficients.iter().enumerate() {
        if coefficient.is_zero() {
            continue;
        }
        let text = coefficient.to_string();
        let negative = text.starts_with('-');
        let magnitude = text.trim_start_matches('-');
        if out.is_empty() {
            if negative {
                out.push('-');
            }
        } else {
            out.push_str(if negative { "-" } else { "+" });
        }
        match degree {
            0 => out.push_str(magnitude),
            _ => {
                if magnitude != "1" {
                    out.push_str(magnitude);
                    out.push('*');
                }
                out.push_str(variable);
                if degree > 1 {
                    out.push('^');
                    out.push_str(&degree.to_string());
                }
            }
        }
    }
    if out.is_empty() {
        out.push('0');
    }
    out
}

/// Image of an exact rational modulo `field`'s prime; `None` for a
/// vanishing denominator.
pub fn rational_image(value: &Rational, field: &Zp64) -> Option<Zp> {
    let numerator: Zp = value.numerator().to_finite_field(field);
    let denominator: Zp = value.denominator().to_finite_field(field);
    let inverse = field.try_inv(&denominator)?;
    Some(field.mul(&numerator, &inverse))
}

/// Wang's rational reconstruction of `value mod modulus`.
pub fn wang_reconstruction(value: &Integer, modulus: &Integer) -> Option<Rational> {
    let two = Integer::from(2);
    let mut v = value % modulus;
    if v < Integer::zero() {
        v += modulus;
    }
    let (mut old_r, mut r) = (modulus.clone(), v);
    let (mut old_t, mut t) = (Integer::zero(), Integer::one());
    // Run the extended Euclidean algorithm until 2 r^2 < m.
    while &(&two * &(&r * &r)) >= modulus {
        if r.is_zero() {
            return None;
        }
        let (quotient, remainder) = old_r.quot_rem(&r);
        (old_r, r) = (r, remainder);
        let next_t = &old_t - &(&quotient * &t);
        (old_t, t) = (t, next_t);
    }
    if t.is_zero() || &(&two * &(&t * &t)) >= modulus {
        return None;
    }
    let (mut n, mut d) = (r, t);
    if !n.gcd(&d).is_one() {
        return None;
    }
    if d < Integer::zero() {
        n = -n;
        d = -d;
    }
    Some((n, d).into())
}

/// Monagan's maximal-quotient rational reconstruction with Symbolica's
/// default acceptance threshold.
pub fn mqrr_reconstruction(value: &Integer, modulus: &Integer) -> Option<Rational> {
    Rational::maximal_quotient_reconstruction(value, modulus, None).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wang_and_mqrr_invert_small_fractions() {
        let modulus = Integer::from((1u64 << 61) - 1) * Integer::from(4611686018427387847u64);
        for (n, d) in [(1i64, 3i64), (-22, 7), (123456789, 987654321), (0, 1)] {
            let mut v = (&Integer::from(n) * &Integer::from(d).mod_inverse(&modulus)) % &modulus;
            if v < Integer::zero() {
                v += &modulus;
            }
            let expected = Rational::from((Integer::from(n), Integer::from(d)));
            assert_eq!(wang_reconstruction(&v, &modulus), Some(expected.clone()));
            assert_eq!(mqrr_reconstruction(&v, &modulus), Some(expected));
        }
    }

    #[test]
    fn formatting_and_exact_evaluation() {
        let q = |n: i64, d: i64| Rational::from((Integer::from(n), Integer::from(d)));
        let f = RationalFunction {
            numerator: vec![q(-1, 2), q(0, 1), q(3, 1)],
            denominator: vec![q(1, 1), q(-1, 7)],
        };
        assert_eq!(f.format("d"), "(-1/2+3*d^2)/(1-1/7*d)");
        assert_eq!(f.evaluate_rational(&q(7, 1)), None);
        assert_eq!(f.evaluate_rational(&q(1, 1)), Some(q(5 * 7, 2 * 6)));
        assert_eq!(RationalFunction::zero().format("x"), "0");
    }
}
