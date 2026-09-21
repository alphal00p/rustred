//! Complete integer refinement of `A*x*y+B*x+C*y+D=0`, with `A != 0`.
//!
//! `(A*x+C)*(A*y+B)=B*C-A*D` reduces this narrow case to signed
//! divisors, not a bounded search in the coordinates. Symbolica owns all
//! arithmetic, coefficient extraction, factoring, primality and division.
//! Every output is an AND of affine equations; the caller must retain the
//! complete parent and original conjunction on each OR branch.

use symbolica::prelude::Integer;

use crate::algebra::CoefficientPolynomial;

use super::{
    CaseIntersectionFailure, CaseIntersectionLimits, CaseIntersectionStats, integer_divisors,
};

struct Bilinear {
    x: usize,
    y: usize,
    a: Integer,
    b: Integer,
    c: Integer,
    d: Integer,
}

fn authenticate<const N: usize>(
    equation: &CoefficientPolynomial,
    indices: &[usize; N],
) -> Option<Bilinear> {
    let support: Vec<_> = (0..equation.nvars())
        .filter(|&variable| equation.degree(variable) != 0)
        .collect();
    let &[x, y] = support.as_slice() else {
        return None;
    };
    if !indices.contains(&x)
        || !indices.contains(&y)
        || equation.degree(x) != 1
        || equation.degree(y) != 1
    {
        return None;
    }
    let mut powers = vec![0; equation.nvars()];
    let mut coefficient = |px, py| {
        powers[x] = px;
        powers[y] = py;
        equation.coefficient(&powers).unwrap_or_else(Integer::zero)
    };
    let a = coefficient(1, 1);
    if a.is_zero() {
        return None;
    }
    Some(Bilinear {
        x,
        y,
        a,
        b: coefficient(1, 0),
        c: coefficient(0, 1),
        d: coefficient(0, 0),
    })
}

fn linear(
    source: &CoefficientPolynomial,
    variable: usize,
    slope: Integer,
    constant: Integer,
) -> CoefficientPolynomial {
    let mut result = source.zero();
    let mut powers = vec![0; source.nvars()];
    result.append_monomial(constant, &powers);
    powers[variable] = 1;
    result.append_monomial(slope, &powers);
    result
}

/// `None` means this service made no inference. All native factorizations and
/// divisor visits share the enclosing intersection budgets. The deterministic
/// native prime guarantee is only used through u64; larger constants remain
/// unsupported rather than receiving a probable-prime completeness claim.
pub(super) fn refine<const N: usize>(
    equation: &CoefficientPolynomial,
    indices: &[usize; N],
    limits: CaseIntersectionLimits,
    stats: &mut CaseIntersectionStats,
    pending: usize,
) -> Result<Option<Vec<Vec<CoefficientPolynomial>>>, CaseIntersectionFailure> {
    let Some(Bilinear { x, y, a, b, c, d }) = authenticate(equation, indices) else {
        return Ok(None);
    };
    let k = &b * &c - &a * &d;
    if k.is_zero() {
        // These are faces, not points: one original variable stays free.
        return Ok(Some(vec![
            vec![linear(equation, x, a.clone(), c)],
            vec![linear(equation, y, a, b)],
        ]));
    }
    let mut points = Vec::new();
    if !integer_divisors::visit(&k, limits, stats, pending, |u| {
        let (v, remainder) = k.quot_rem(u);
        if !remainder.is_zero() {
            return Err(CaseIntersectionFailure::NativeAlgebra);
        }
        let (x_value, x_remainder) = (u - &c).quot_rem(&a);
        let (y_value, y_remainder) = (&v - &b).quot_rem(&a);
        if x_remainder.is_zero() && y_remainder.is_zero() {
            points.push((x_value, y_value));
        }
        Ok(())
    })? {
        return Ok(None);
    }
    points.sort();
    points.dedup();
    Ok(Some(
        points
            .into_iter()
            .map(|(x_value, y_value)| {
                vec![
                    linear(equation, x, Integer::one(), -x_value),
                    linear(equation, y, Integer::one(), -y_value),
                ]
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests;
