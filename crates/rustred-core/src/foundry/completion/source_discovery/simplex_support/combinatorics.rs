use std::sync::Arc;

use super::SimplexSupportError;

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SimplexSupportError> {
    left.checked_add(right)
        .ok_or(SimplexSupportError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SimplexSupportError> {
    left.checked_mul(right)
        .ok_or(SimplexSupportError::ResourceCountOverflow { resource })
}

fn try_reserve_exact<T>(
    retained: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), SimplexSupportError> {
    let requested = checked_add(resource, retained.len(), additional)?;
    retained
        .try_reserve_exact(additional)
        .map_err(|_| SimplexSupportError::AllocationFailure {
            resource,
            requested,
        })
}

fn try_reserve_one<T>(
    retained: &mut Vec<T>,
    resource: &'static str,
) -> Result<(), SimplexSupportError> {
    try_reserve_exact(retained, 1, resource)
}

pub(crate) fn checked_binomial(
    n: usize,
    k: usize,
    resource: &'static str,
) -> Result<usize, SimplexSupportError> {
    if k > n {
        return Ok(0);
    }
    let k = k.min(n - k);
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
            return Err(SimplexSupportError::Invariant {
                detail: "exact binomial cancellation left a denominator",
            });
        }
        result = checked_mul(resource, result, numerator)?;
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

/// Exact cardinality `binomial(dimension + degree, degree)`.
pub(crate) fn try_simplex_sample_count(
    dimension: usize,
    degree: usize,
) -> Result<usize, SimplexSupportError> {
    let n = checked_add("simplex binomial upper argument", dimension, degree)?;
    checked_binomial(n, degree.min(dimension), "complete simplex samples")
}

/// Build the complete graded weak-composition simplex in canonical order.
pub(crate) fn try_build_simplex_offsets(
    dimension: usize,
    degree: usize,
    expected_count: usize,
) -> Result<Vec<Arc<Vec<u64>>>, SimplexSupportError> {
    if dimension == 0 {
        if expected_count != 1 {
            return Err(SimplexSupportError::Invariant {
                detail: "a zero-dimensional simplex did not preflight one empty offset",
            });
        }
        let mut offsets = Vec::new();
        try_reserve_exact(&mut offsets, 1, "complete simplex offsets")?;
        offsets.push(Arc::new(Vec::new()));
        return Ok(offsets);
    }

    let mut offsets = Vec::new();
    try_reserve_exact(&mut offsets, expected_count, "complete simplex offsets")?;
    for total_degree in 0..=degree {
        append_weak_compositions(dimension, total_degree, &mut offsets)?;
    }
    if offsets.len() != expected_count {
        return Err(SimplexSupportError::Invariant {
            detail: "simplex enumeration disagreed with its exact binomial preflight",
        });
    }
    Ok(offsets)
}

fn append_weak_compositions(
    dimension: usize,
    total_degree: usize,
    output: &mut Vec<Arc<Vec<u64>>>,
) -> Result<(), SimplexSupportError> {
    if dimension == 1 {
        let coordinate = u64::try_from(total_degree).map_err(|_| {
            SimplexSupportError::ResourceCountOverflow {
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
                SimplexSupportError::ResourceCountOverflow {
                    resource: "simplex offset coordinate",
                }
            })?);
            previous = Some(bar);
        }
        let last = slot_count
            - previous.ok_or(SimplexSupportError::Invariant {
                detail: "a positive stars-and-bars dimension retained no bars",
            })?
            - 1;
        offset.push(u64::try_from(last).map_err(|_| {
            SimplexSupportError::ResourceCountOverflow {
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

/// Exact Cartesian-product cardinality of all finite axes.
pub(crate) fn try_finite_assignment_count(
    lower: &[u64],
    upper: &[Option<u64>],
    resource: &'static str,
) -> Result<usize, SimplexSupportError> {
    if lower.len() != upper.len() {
        return Err(SimplexSupportError::Invariant {
            detail: "finite-assignment endpoints have different arities",
        });
    }
    let mut count = 1usize;
    for (&lower, &upper) in lower.iter().zip(upper) {
        let Some(upper) = upper else {
            continue;
        };
        let width = upper
            .checked_sub(lower)
            .and_then(|span| span.checked_add(1))
            .ok_or(SimplexSupportError::ResourceCountOverflow { resource })?;
        let width = usize::try_from(width)
            .map_err(|_| SimplexSupportError::ResourceCountOverflow { resource })?;
        count = checked_mul(resource, count, width)?;
    }
    Ok(count)
}

/// Install one last-finite-axis-fast Cartesian assignment in place.
pub(crate) fn try_apply_finite_assignment(
    lower: &[u64],
    upper: &[Option<u64>],
    assignment_ordinal: usize,
    target: &mut [u64],
) -> Result<(), SimplexSupportError> {
    if lower.len() != upper.len() || lower.len() != target.len() {
        return Err(SimplexSupportError::Invariant {
            detail: "finite-assignment target has the wrong arity",
        });
    }
    let mut remaining = assignment_ordinal;
    for position in (0..lower.len()).rev() {
        let Some(upper) = upper[position] else {
            continue;
        };
        let width = upper
            .checked_sub(lower[position])
            .and_then(|span| span.checked_add(1))
            .and_then(|width| usize::try_from(width).ok())
            .ok_or(SimplexSupportError::Invariant {
                detail: "a preflighted finite-assignment width became invalid",
            })?;
        let digit = remaining % width;
        remaining /= width;
        target[position] = lower[position]
            .checked_add(
                u64::try_from(digit).map_err(|_| SimplexSupportError::Invariant {
                    detail: "a finite-assignment digit did not fit its coordinate carrier",
                })?,
            )
            .ok_or(SimplexSupportError::Invariant {
                detail: "a preflighted finite-assignment coordinate overflowed",
            })?;
    }
    if remaining != 0 {
        return Err(SimplexSupportError::Invariant {
            detail: "finite-assignment ordinal exceeded its exact Cartesian product",
        });
    }
    Ok(())
}
