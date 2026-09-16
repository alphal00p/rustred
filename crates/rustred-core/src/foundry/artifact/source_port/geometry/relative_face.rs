//! Exact relative coordinate exclusions of an affine parent.

use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};

use super::{SourcePortAuditError, error};

/// Prove `parent ∩ child.fixed_face ⊆ child`, before subtracting that entire
/// face from the parent's rectangular prefilter. The child's equations can
/// simplify when its new fixed coordinates are imposed: the old parent chart
/// alone does not capture that implication. Every equality operation stays
/// with the existing Symbolica-backed affine service.
pub(super) fn is_coordinate_exception<const N: usize>(
    parent: &AffineCase<N>,
    child: &AffineCase<N>,
    sector: &[bool; N],
) -> Result<bool, SourcePortAuditError> {
    if parent.index_variables() != child.index_variables()
        || parent.equations()[0].variables() != child.equations()[0].variables()
    {
        return Err(error("relative affine face uses different index maps"));
    }
    let mut fixed = *parent.face().fixed();
    for (current, &extra) in fixed.iter_mut().zip(child.face().fixed()) {
        if let Some(value) = extra {
            if current.is_some_and(|old| old != value) {
                // The faces are disjoint; removing the child face cannot
                // remove any actual point of the parent.
                return Ok(true);
            }
            *current = Some(value);
        }
    }
    if parent
        .equations()
        .len()
        .checked_add(fixed.iter().filter(|value| value.is_some()).count())
        .and_then(|rows| rows.checked_mul(N + 1))
        .is_none_or(|cells| cells > 65_536)
    {
        // A missed optimization leaves the exact predicate in place.
        return Ok(false);
    }
    match AffineCase::from_coordinate(
        &CoordinateCase::new(fixed).map_err(error)?,
        parent.equations(),
        parent.index_variables(),
        sector,
    )
    .map_err(error)?
    {
        AffineIntersection::Empty => Ok(true),
        AffineIntersection::Coordinate(face) => Ok(child.contains_coordinate(&face)),
        AffineIntersection::Affine(intersection) => {
            child.contains_affine(&intersection).map_err(error)
        }
    }
}

#[cfg(test)]
mod tests;
