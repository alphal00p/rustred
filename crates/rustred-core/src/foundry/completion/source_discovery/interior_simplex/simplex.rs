use std::sync::Arc;

use super::InteriorSimplexPlanError;
use super::resource::{checked_add, checked_mul, try_reserve_exact, try_reserve_one};

/// Exact cardinality `binomial(free_dimension + degree, degree)`.
pub(super) fn try_simplex_sample_count(
    free_dimension: usize,
    degree: usize,
) -> Result<usize, InteriorSimplexPlanError> {
    let n = checked_add("simplex binomial upper argument", free_dimension, degree)?;
    checked_binomial(n, degree.min(free_dimension))
}

/// Build the complete graded simplex without a recursive or rectangular walk.
///
/// Within each total degree, weak compositions are ordered by the
/// lexicographic order of their stars-and-bars positions.  This is a stable
/// scheduling convention only; no monomial-order semantics are inferred.
pub(super) fn try_build_simplex_offsets(
    free_dimension: usize,
    degree_ceiling: usize,
    expected_count: usize,
) -> Result<Vec<Arc<Vec<u64>>>, InteriorSimplexPlanError> {
    if free_dimension == 0 {
        return Err(InteriorSimplexPlanError::Invariant {
            detail: "an interior simplex requires a positive free dimension",
        });
    }
    let mut offsets = Vec::new();
    try_reserve_exact(&mut offsets, expected_count, "complete simplex offsets")?;
    for total_degree in 0..=degree_ceiling {
        append_weak_compositions(free_dimension, total_degree, &mut offsets)?;
    }
    if offsets.len() != expected_count {
        return Err(InteriorSimplexPlanError::Invariant {
            detail: "simplex enumeration disagreed with its exact binomial preflight",
        });
    }
    Ok(offsets)
}

fn append_weak_compositions(
    dimension: usize,
    total_degree: usize,
    output: &mut Vec<Arc<Vec<u64>>>,
) -> Result<(), InteriorSimplexPlanError> {
    if dimension == 1 {
        let coordinate = u64::try_from(total_degree).map_err(|_| {
            InteriorSimplexPlanError::ResourceCountOverflow {
                resource: "simplex offset coordinate",
            }
        })?;
        let mut offset = Vec::new();
        try_reserve_exact(&mut offset, 1, "simplex offset coordinates")?;
        offset.push(coordinate);
        try_reserve_one(output, "complete simplex offsets")?;
        output.push(Arc::new(offset));
        return Ok(());
    }

    let bar_count = dimension - 1;
    let slot_count = checked_add("stars-and-bars slot count", total_degree, bar_count)?;
    let mut bars = Vec::new();
    try_reserve_exact(&mut bars, bar_count, "stars-and-bars positions")?;
    bars.extend(0..bar_count);

    loop {
        let mut offset = Vec::new();
        try_reserve_exact(&mut offset, dimension, "simplex offset coordinates")?;
        let mut previous = None;
        for &bar in &bars {
            let coordinate = match previous {
                None => bar,
                Some(previous) => bar - previous - 1,
            };
            offset.push(u64::try_from(coordinate).map_err(|_| {
                InteriorSimplexPlanError::ResourceCountOverflow {
                    resource: "simplex offset coordinate",
                }
            })?);
            previous = Some(bar);
        }
        let last = slot_count
            - previous.ok_or(InteriorSimplexPlanError::Invariant {
                detail: "a positive stars-and-bars dimension retained no bars",
            })?
            - 1;
        offset.push(u64::try_from(last).map_err(|_| {
            InteriorSimplexPlanError::ResourceCountOverflow {
                resource: "simplex offset coordinate",
            }
        })?);
        try_reserve_one(output, "complete simplex offsets")?;
        output.push(Arc::new(offset));

        let Some(pivot) = (0..bar_count)
            .rev()
            .find(|&position| bars[position] < slot_count - bar_count + position)
        else {
            break;
        };
        bars[pivot] += 1;
        for position in (pivot + 1)..bar_count {
            bars[position] = bars[position - 1] + 1;
        }
    }
    Ok(())
}

fn checked_binomial(n: usize, k: usize) -> Result<usize, InteriorSimplexPlanError> {
    let mut result = 1usize;
    for step in 1..=k {
        let mut numerator = n - k + step;
        let mut denominator = step;
        let common = gcd(numerator, denominator);
        numerator /= common;
        denominator /= common;
        let common = gcd(result, denominator);
        result /= common;
        denominator /= common;
        if denominator != 1 {
            return Err(InteriorSimplexPlanError::Invariant {
                detail: "exact binomial cancellation left a denominator",
            });
        }
        result = checked_mul("complete simplex samples", result, numerator)?;
    }
    Ok(result)
}

fn gcd(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
