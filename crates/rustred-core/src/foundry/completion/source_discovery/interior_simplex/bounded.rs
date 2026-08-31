use super::InteriorSimplexPlanError;
use super::resource::checked_mul;

const FINITE_ASSIGNMENTS: &str = "finite coordinate assignments";

/// Exact Cartesian-product cardinality of the finite axes of one lattice box.
///
/// An all-unbounded box has the single empty finite assignment.  The full
/// product is counted before any assignment is materialized.
pub(super) fn try_finite_assignment_count(
    lower: &[u64],
    upper: &[Option<u64>],
) -> Result<usize, InteriorSimplexPlanError> {
    if lower.len() != upper.len() {
        return Err(InteriorSimplexPlanError::Invariant {
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
            .ok_or(InteriorSimplexPlanError::ResourceCountOverflow {
                resource: FINITE_ASSIGNMENTS,
            })?;
        let width = usize::try_from(width).map_err(|_| {
            InteriorSimplexPlanError::ResourceCountOverflow {
                resource: FINITE_ASSIGNMENTS,
            }
        })?;
        count = checked_mul(FINITE_ASSIGNMENTS, count, width)?;
    }
    Ok(count)
}

/// Install one lexicographically ordered finite-axis assignment in place.
///
/// Ascending coordinate positions define lexicographic order, so the last
/// finite axis varies fastest.  Unbounded coordinates are left untouched for
/// the positive-margin simplex layer.
pub(super) fn try_apply_finite_assignment(
    lower: &[u64],
    upper: &[Option<u64>],
    assignment_ordinal: usize,
    target: &mut [u64],
) -> Result<(), InteriorSimplexPlanError> {
    if lower.len() != upper.len() || lower.len() != target.len() {
        return Err(InteriorSimplexPlanError::Invariant {
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
            .ok_or(InteriorSimplexPlanError::Invariant {
                detail: "a preflighted finite-assignment width became invalid",
            })?;
        let digit = remaining % width;
        remaining /= width;
        target[position] = lower[position]
            .checked_add(
                u64::try_from(digit).map_err(|_| InteriorSimplexPlanError::Invariant {
                    detail: "a finite-assignment digit did not fit its coordinate carrier",
                })?,
            )
            .ok_or(InteriorSimplexPlanError::Invariant {
                detail: "a preflighted finite-assignment coordinate overflowed",
            })?;
    }
    if remaining != 0 {
        return Err(InteriorSimplexPlanError::Invariant {
            detail: "finite-assignment ordinal exceeded its exact Cartesian product",
        });
    }
    Ok(())
}
