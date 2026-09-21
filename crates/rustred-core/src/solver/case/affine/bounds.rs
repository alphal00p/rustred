//! Exact sector-sign bounds for native integer affine rows.
//!
//! Symbolica's public equation solver retains unresolved positivity conditions
//! and does not accept general inequality systems. This is domain bookkeeping
//! over its native `Integer` arithmetic, not an LP or polyhedron solver.

use symbolica::prelude::Integer;

use super::CoordinateCase;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RowBounds {
    Excluded,
    Unresolved,
    /// Every unfixed original axis with a nonzero coefficient is at its sector
    /// endpoint (one for active indices, zero for inactive indices).
    Saturated,
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

/// At an attained finite extremum, every individual nonnegative slack is zero.
/// Hence all nonzero unfixed terms attain their unique half-line endpoints.
/// This is an exact consequence of one row, not a general feasibility claim.
pub(super) fn classify<const N: usize>(
    row: &[Integer],
    face: &CoordinateCase<N>,
    sector: &[bool; N],
) -> RowBounds {
    debug_assert_eq!(row.len(), N + 1);
    let mut lower = Some(Integer::zero());
    let mut upper = Some(Integer::zero());
    let mut has_unfixed_term = false;
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
            has_unfixed_term = true;
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
        RowBounds::Excluded
    } else if has_unfixed_term
        && (lower.as_ref().is_some_and(|bound| row[N] == *bound)
            || upper.as_ref().is_some_and(|bound| row[N] == *bound))
    {
        RowBounds::Saturated
    } else {
        RowBounds::Unresolved
    }
}

#[cfg(test)]
#[path = "bounds_audit.rs"]
mod audit;
