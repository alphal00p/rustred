use crate::foundry::completion::frame::PhysicalFrameError;
use crate::identity::TranslatedSourceError;

use super::TriangularSupportError;

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), TriangularSupportError> {
    if requested > limit {
        Err(TriangularSupportError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn check_selected_translation_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), TriangularSupportError> {
    if requested > limit {
        Err(TriangularSupportError::SourceTranslation(
            TranslatedSourceError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        ))
    } else {
        Ok(())
    }
}

pub(super) fn check_physical_frame_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), TriangularSupportError> {
    if requested > limit {
        Err(TriangularSupportError::PhysicalFrame(
            PhysicalFrameError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        ))
    } else {
        Ok(())
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, TriangularSupportError> {
    left.checked_add(right)
        .ok_or(TriangularSupportError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, TriangularSupportError> {
    left.checked_mul(right)
        .ok_or(TriangularSupportError::ResourceCountOverflow { resource })
}

pub(super) fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, TriangularSupportError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| TriangularSupportError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}
