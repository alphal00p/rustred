//! Caller-declared downward-closed sector scope, not zero evidence.
//!
//! A root mask permits positive indices only at its active positions. Every
//! subsector is included, with unrestricted integer numerator powers. The
//! existing root-bound carrier stores exactly this contract; arbitrary finite
//! windows are not accepted by this source-port producer.

use crate::sector::{InteriorBounds, Mask};

use super::super::error::ArtifactError;

// Internal exact entry geometry; publication wiring must additionally prove
// successor closure and persist/enforce the scope before exposing a rank flag.
#[allow(dead_code)]
pub(in crate::foundry::artifact) mod rank;
#[allow(dead_code)]
pub(in crate::foundry::artifact) mod successor;

pub(in crate::foundry::artifact) fn root_bounds(
    root: &Mask,
) -> Result<Box<[InteriorBounds]>, ArtifactError> {
    let mut bounds = Vec::new();
    bounds
        .try_reserve_exact(root.arity())
        .map_err(|_| ArtifactError::AllocationFailure {
            resource: "root-sector bounds",
            requested: root.arity(),
        })?;
    bounds.extend(
        root.active_bits()
            .iter()
            .map(|&active| InteriorBounds::new(i64::MIN, if active { i64::MAX } else { 0 })),
    );
    Ok(bounds.into_boxed_slice())
}

pub(in crate::foundry::artifact) fn root_from_bounds(
    bounds: &[InteriorBounds],
    arity: usize,
) -> Result<Mask, ArtifactError> {
    if bounds.len() != arity {
        return Err(ArtifactError::WrongArity {
            expected: arity,
            actual: bounds.len(),
        });
    }
    if bounds
        .iter()
        .any(|bound| bound.lower() != i64::MIN || !matches!(bound.upper(), 0 | i64::MAX))
    {
        return Err(ArtifactError::InvalidRuleShape {
            detail: "source-port root bounds must be unrestricted or nonpositive per index",
        });
    }
    Mask::try_new(bounds.iter().map(|bound| bound.upper() == i64::MAX)).map_err(Into::into)
}

pub(in crate::foundry::artifact) fn sector_count(root: &Mask) -> Result<usize, ArtifactError> {
    u32::try_from(root.active_count())
        .ok()
        .and_then(|active| 1usize.checked_shl(active))
        .ok_or(ArtifactError::ResourceCountOverflow {
            resource: "root-sector census",
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_scope_is_a_downset_with_unbounded_nonpositive_indices() {
        let root = Mask::try_new([true, false, true]).unwrap();
        let bounds = root_bounds(&root).unwrap();
        assert_eq!(root_from_bounds(&bounds, 3).unwrap(), root);
        assert_eq!(sector_count(&root).unwrap(), 4);
        assert_eq!(bounds[1], InteriorBounds::new(i64::MIN, 0));
        assert!(
            Mask::try_new([false; 3])
                .unwrap()
                .is_subsector_of(&root)
                .unwrap()
        );
        assert!(
            !Mask::try_new([false, true, false])
                .unwrap()
                .is_subsector_of(&root)
                .unwrap()
        );
        assert_eq!(
            sector_count(&Mask::try_new([false; 3]).unwrap()).unwrap(),
            1
        );
    }

    #[test]
    fn source_port_scope_does_not_accept_arbitrary_finite_root_windows() {
        for bounds in [
            InteriorBounds::new(0, i64::MAX),
            InteriorBounds::new(i64::MIN, -1),
            InteriorBounds::new(i64::MIN, 1),
        ] {
            assert!(root_from_bounds(&[bounds], 1).is_err());
        }
        assert!(root_from_bounds(&[], 1).is_err());
    }
}
