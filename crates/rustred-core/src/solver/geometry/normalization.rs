//! Cold exact normalization of a conjunction, never of unrelated OR branches.
//!
//! Symbolica's native F4/reduced Groebner basis preserves the polynomial ideal
//! over Q, hence its common integer zero set. This is not root enumeration,
//! radical computation, modular inference or rational-function reconstruction.

use symbolica::poly::groebner::GroebnerBasis;
use symbolica::prelude::{Integer, Q, Rational, Z};

use crate::algebra::CoefficientPolynomial;

use super::{CoordinateCase, GeometryError};

/// Input admission and panic containment belong to `geometry::intersect`.
/// `None` means the cold fallback is inapplicable, not an empty zero set.
pub(super) fn normalize<const N: usize>(
    parent: &CoordinateCase<N>,
    conjunction: &[CoefficientPolynomial],
    indices: &[usize; N],
) -> Result<Option<Vec<CoefficientPolynomial>>, GeometryError> {
    if conjunction.len() < 2 {
        return Ok(None);
    }
    let mut specialized = Vec::with_capacity(conjunction.len());
    for source in conjunction {
        let mut equation = source.clone();
        for (axis, value) in parent.fixed().iter().enumerate() {
            if let Some(value) = value {
                equation = equation.replace(indices[axis], &Integer::from(*value));
            }
        }
        if !equation.is_zero() && !specialized.contains(&equation) {
            specialized.push(equation);
        }
    }
    if specialized.len() < 2 || !specialized.iter().any(is_nonlinear) {
        return Ok(None);
    }
    let ideal: Vec<_> = specialized
        .iter()
        .map(|polynomial| polynomial.map_coeff(|value| Rational::from(value), Q))
        .collect();
    // For Q the pinned implementation uses exact field elimination. The
    // optimized finite-field path is selected only for a native Zp field.
    let basis = GroebnerBasis::new(&ideal, false);
    let mut normalized = Vec::with_capacity(basis.system.len());
    for polynomial in basis.system {
        if polynomial.is_zero() {
            continue;
        }
        // Over Q, native content is gcd(numerators)/lcm(denominators).
        // Removing it therefore gives an exact primitive integer polynomial.
        let primitive = polynomial.make_primitive();
        if primitive
            .coefficients
            .iter()
            .any(|value| !value.is_integer())
        {
            return Err(GeometryError::NativeAlgebra);
        }
        let integer = primitive.map_coeff(Rational::numerator, Z);
        if integer.variables() != conjunction[0].variables() {
            return Err(GeometryError::NativeAlgebra);
        }
        normalized.push(integer);
    }
    Ok(Some(normalized))
}

fn is_nonlinear(polynomial: &CoefficientPolynomial) -> bool {
    polynomial.exponents_iter().any(|exponents| {
        exponents.iter().any(|&power| power > 1)
            || exponents
                .iter()
                .filter(|&&power| power != 0)
                .take(2)
                .count()
                > 1
    })
}

#[cfg(test)]
#[path = "normalization/tests.rs"]
mod tests;
