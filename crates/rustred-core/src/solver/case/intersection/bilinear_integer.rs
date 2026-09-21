//! Complete integer refinement of `A*x*y+B*x+C*y+D=0`, with `A != 0`.
//!
//! `(A*x+C)*(A*y+B)=B*C-A*D` reduces this narrow case to signed
//! divisors, not a bounded search in the coordinates. Symbolica owns all
//! arithmetic, coefficient extraction, factoring, primality and division.
//! Every output is an AND of affine equations; the caller must retain the
//! complete parent and original conjunction on each OR branch.

use std::time::Instant;

use symbolica::prelude::Integer;

use crate::algebra::CoefficientPolynomial;

use super::{
    CaseIntersectionBudget, CaseIntersectionFailure, CaseIntersectionLimits, CaseIntersectionStats,
    engine::spend,
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
    let magnitude = k.abs();
    if u64::try_from(magnitude.clone()).is_err() {
        return Ok(None);
    }
    let factors = if magnitude.is_one() {
        Vec::new()
    } else {
        spend(
            &mut stats.factorizations,
            limits.max_factorizations,
            CaseIntersectionBudget::Factorizations,
        )?;
        stats.integer_factorizations += 1;
        let start = Instant::now();
        let factors = magnitude.factor();
        stats.factorization_time += start.elapsed();
        factors
    };

    let budget_error = || CaseIntersectionFailure::Budget {
        kind: CaseIntersectionBudget::WorkItems,
        limit: limits.max_work_items,
    };
    let mut divisor_count = 1usize;
    let mut product = Integer::one();
    let mut exponents = Vec::with_capacity(factors.len());
    for (prime, exponent) in &factors {
        // Product equality alone would not prove completeness if an opaque
        // composite were returned as a factor. Native primality is exact here.
        if u64::try_from(prime.clone()).is_err() || !prime.is_prime(0) {
            return Err(CaseIntersectionFailure::NativeAlgebra);
        }
        let exponent = u32::try_from(exponent.clone())
            .ok()
            .filter(|&e| e > 0 && e <= 64)
            .ok_or(CaseIntersectionFailure::NativeAlgebra)?;
        product *= prime.pow(exponent as u64);
        divisor_count = divisor_count
            .checked_mul(exponent as usize + 1)
            .ok_or_else(budget_error)?;
        exponents.push(exponent);
    }
    if product != magnitude || factors.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return Err(CaseIntersectionFailure::NativeAlgebra);
    }
    let visits = divisor_count.checked_mul(2).ok_or_else(budget_error)?;
    if stats
        .work_items
        .checked_add(pending)
        .and_then(|n| n.checked_add(visits))
        .is_none_or(|n| n > limits.max_work_items)
    {
        return Err(budget_error());
    }

    // Enumerating native prime powers is finite combinatorial orchestration,
    // not a new integer factoring or polynomial-algebra implementation.
    let mut divisors = vec![Integer::one()];
    for ((prime, _), exponent) in factors.iter().zip(exponents) {
        let previous = divisors.len();
        let mut power = Integer::one();
        for _ in 0..exponent {
            power *= prime;
            for position in 0..previous {
                divisors.push(&divisors[position] * &power);
            }
        }
    }
    let mut points = Vec::new();
    for positive in divisors {
        for u in [positive.clone(), -positive] {
            spend(
                &mut stats.work_items,
                limits.max_work_items,
                CaseIntersectionBudget::WorkItems,
            )?;
            stats.integer_divisors += 1;
            let (v, remainder) = k.quot_rem(&u);
            if !remainder.is_zero() {
                return Err(CaseIntersectionFailure::NativeAlgebra);
            }
            let (x_value, x_remainder) = (&u - &c).quot_rem(&a);
            let (y_value, y_remainder) = (&v - &b).quot_rem(&a);
            if x_remainder.is_zero() && y_remainder.is_zero() {
                points.push((x_value, y_value));
            }
        }
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
