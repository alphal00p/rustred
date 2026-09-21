//! Exact sector-sign bounds for native integer affine rows.
//!
//! Symbolica's public equation solver retains unresolved positivity conditions
//! and does not accept general inequality systems. This is domain bookkeeping
//! over its native `Integer` arithmetic, not an LP or polyhedron solver.

use symbolica::prelude::Integer;

use super::CoordinateCase;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RowBounds<const N: usize> {
    Excluded,
    Unresolved,
    /// These free original axes must be at their integer sector endpoints:
    /// one for active indices, zero for inactive indices.
    Endpoints([bool; N]),
}

/// Prove `a.n = b` impossible using the independent sector bounds of each axis.
/// `None` bounds mean unbounded, never a sampled or compact-encoding endpoint.
pub(super) fn excludes_rhs<const N: usize>(
    row: &[Integer],
    face: &CoordinateCase<N>,
    sector: &[bool; N],
) -> bool {
    classify(row, face, sector) == RowBounds::Excluded
}

/// With a finite row bound, every free integer coordinate consumes at least
/// the absolute value of its coefficient per step away from its endpoint.
/// A coefficient strictly larger than the remaining slack forces that axis
/// to its endpoint. Zero slack recovers complete endpoint saturation.
/// This is an exact consequence of one row, not a general feasibility claim.
pub(super) fn classify<const N: usize>(
    row: &[Integer],
    face: &CoordinateCase<N>,
    sector: &[bool; N],
) -> RowBounds<N> {
    debug_assert_eq!(row.len(), N + 1);
    let mut lower = Some(Integer::zero());
    let mut upper = Some(Integer::zero());
    for (axis, coefficient) in row[..N].iter().enumerate() {
        if coefficient.is_zero() {
            continue;
        }
        if let Some(value) = face.fixed()[axis] {
            let contribution = coefficient * Integer::from(value);
            if let Some(bound) = &mut lower {
                *bound += &contribution;
            }
            if let Some(bound) = &mut upper {
                *bound += &contribution;
            }
        } else {
            // Positive indices start at one; inactive indices end at zero.
            // Multiplication by a negative coefficient reverses the bounds.
            if sector[axis] != coefficient.is_negative() {
                upper = None;
                if sector[axis]
                    && let Some(bound) = &mut lower
                {
                    *bound += coefficient;
                }
            } else {
                lower = None;
                if sector[axis]
                    && let Some(bound) = &mut upper
                {
                    *bound += coefficient;
                }
            }
        }
    }
    if lower.as_ref().is_some_and(|bound| row[N] < *bound)
        || upper.as_ref().is_some_and(|bound| row[N] > *bound)
    {
        return RowBounds::Excluded;
    }
    // There is no finite slack if opposing unbounded contributions can
    // cancel. Do not use only a subset of the row to invent a bound.
    let slack = if let Some(bound) = lower {
        &row[N] - bound
    } else if let Some(bound) = upper {
        bound - &row[N]
    } else {
        return RowBounds::Unresolved;
    };
    debug_assert!(!slack.is_negative());
    let endpoints = std::array::from_fn(|axis| {
        face.fixed()[axis].is_none() && !row[axis].is_zero() && row[axis].abs_cmp(&slack).is_gt()
    });
    if endpoints.iter().any(|&forced| forced) {
        RowBounds::Endpoints(endpoints)
    } else {
        RowBounds::Unresolved
    }
}

#[cfg(test)]
#[path = "bounds_audit.rs"]
mod audit;
