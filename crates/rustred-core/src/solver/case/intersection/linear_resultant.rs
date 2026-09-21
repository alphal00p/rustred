//! Finite integer refinement for `(a*y+b)*x+Q(y)=0`, a nonzero.
//!
//! For nonzero native resultant R=Res(a*y+b,Q), every solution has
//! u=a*y+b nonzero and u|R. Exhaustive signed divisors, followed by exact
//! native divisibility of y=(u-b)/a and x=-Q(y)/u, are necessary AND sufficient.
//! This is case enumeration, not a resultant, polynomial-division or CAS kernel.

use std::time::Instant;

use symbolica::prelude::Integer;

use crate::algebra::CoefficientPolynomial;

use super::{
    CaseIntersectionBudget, CaseIntersectionFailure, CaseIntersectionLimits, CaseIntersectionStats,
    engine::spend, integer_divisors,
};

struct Equation {
    x: usize,
    y: usize,
    p: CoefficientPolynomial,
    q: CoefficientPolynomial,
}

fn authenticate<const N: usize>(
    source: &CoefficientPolynomial,
    indices: &[usize; N],
) -> Option<Equation> {
    let support: Vec<_> = (0..source.nvars())
        .filter(|&i| source.degree(i) != 0)
        .collect();
    let &[first, second] = support.as_slice() else {
        return None;
    };
    if !indices.contains(&first) || !indices.contains(&second) {
        return None;
    }
    let (x, y) = if source.degree(first) == 1 && source.degree(second) > 1 {
        (first, second)
    } else if source.degree(second) == 1 && source.degree(first) > 1 {
        (second, first)
    } else {
        // The smaller bilinear/affine lane remains unchanged and cheaper.
        return None;
    };
    let p = source.derivative(x);
    if p.degree(y) != 1 {
        return None;
    }
    let q = source.replace(x, &Integer::zero());
    debug_assert!((0..source.nvars()).all(|i| i == y || (p.degree(i) == 0 && q.degree(i) == 0)));
    Some(Equation { x, y, p, q })
}

pub(super) fn refine<const N: usize>(
    source: &CoefficientPolynomial,
    indices: &[usize; N],
    limits: CaseIntersectionLimits,
    stats: &mut CaseIntersectionStats,
    pending: usize,
) -> Result<Option<Vec<Vec<CoefficientPolynomial>>>, CaseIntersectionFailure> {
    let Some(Equation { x, y, p, q }) = authenticate(source, indices) else {
        return Ok(None);
    };
    // Dense univariate conversion allocates degree+1 coefficients. Charge and
    // bound that preparation before calling native algebra, without introducing
    // a topology-, loop- or rank-specific polynomial-degree cutoff.
    let preparation = usize::from(q.degree(y)) + 1;
    if stats
        .work_items
        .checked_add(pending)
        .and_then(|n| n.checked_add(preparation))
        .is_none_or(|n| n > limits.max_work_items)
    {
        return Err(CaseIntersectionFailure::Budget {
            kind: CaseIntersectionBudget::WorkItems,
            limit: limits.max_work_items,
        });
    }
    spend(
        &mut stats.normalizations,
        limits.max_normalizations,
        CaseIntersectionBudget::Normalizations,
    )?;
    stats.work_items += preparation;
    stats.integer_resultants += 1;
    let start = Instant::now();
    // Support is authenticated in ORIGINAL free index variables. Native
    // Symbolica recognizes the linear operand and uses its own exact fast path.
    let resultant = p
        .to_univariate_from_univariate(y)
        .resultant(&q.to_univariate_from_univariate(y));
    stats.normalization_time += start.elapsed();
    // A zero resultant may describe infinite factor faces/curves. Never turn
    // that into a finite list; ordinary factoring already had its opportunity.
    if resultant.is_zero() {
        return Ok(None);
    }
    let a = p.derivative(y).get_constant();
    let b = p.get_constant();
    let mut points = Vec::new();
    if !integer_divisors::visit(&resultant, limits, stats, pending, |u| {
        let (y_value, remainder) = (u - &b).quot_rem(&a);
        if !remainder.is_zero() {
            return Ok(());
        }
        let value = q.replace(y, &y_value);
        if !value.is_constant() {
            return Err(CaseIntersectionFailure::NativeAlgebra);
        }
        let (x_value, remainder) = (-value.get_constant()).quot_rem(u);
        if remainder.is_zero() {
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
                [(x, x_value), (y, y_value)]
                    .into_iter()
                    .map(|(variable, value)| {
                        let mut equation = source.zero();
                        let mut powers = vec![0; source.nvars()];
                        equation.append_monomial(-value, &powers);
                        powers[variable] = 1;
                        equation.append_monomial(Integer::one(), &powers);
                        equation
                    })
                    .collect()
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests;
