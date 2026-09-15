//! Necessary sector-sign bounds for native integer affine rows.
//!
//! Symbolica's public inequality solver currently reports
//! `InequalitiesNotSupported`. This is only domain bookkeeping over its native
//! `Integer` arithmetic, not an LP or general integer-polyhedron solver.

use symbolica::prelude::Integer;

use super::CoordinateCase;

/// Prove `a.n = b` impossible using the independent sector bounds of each axis.
/// `None` bounds mean unbounded, never a sampled or compact-encoding endpoint.
pub(super) fn excludes_rhs<const N: usize>(
    row: &[Integer],
    face: &CoordinateCase<N>,
    sector: &[bool; N],
) -> bool {
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
    lower.as_ref().is_some_and(|bound| row[N] < *bound)
        || upper.as_ref().is_some_and(|bound| row[N] > *bound)
}

#[cfg(test)]
#[path = "bounds_audit.rs"]
mod audit;
