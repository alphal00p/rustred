use std::sync::Arc;

use super::InteriorSimplexPlanError;

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, InteriorSimplexPlanError> {
    left.checked_add(right)
        .ok_or(InteriorSimplexPlanError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, InteriorSimplexPlanError> {
    left.checked_mul(right)
        .ok_or(InteriorSimplexPlanError::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), InteriorSimplexPlanError> {
    if requested > limit {
        Err(InteriorSimplexPlanError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn try_reserve_exact<T>(
    retained: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), InteriorSimplexPlanError> {
    let requested = checked_add(resource, retained.len(), additional)?;
    retained.try_reserve_exact(additional).map_err(|_| {
        InteriorSimplexPlanError::AllocationFailure {
            resource,
            requested,
        }
    })
}

pub(super) fn try_reserve_one<T>(
    retained: &mut Vec<T>,
    resource: &'static str,
) -> Result<(), InteriorSimplexPlanError> {
    try_reserve_exact(retained, 1, resource)
}

pub(super) fn try_copy_string(
    value: &str,
    resource: &'static str,
) -> Result<Arc<String>, InteriorSimplexPlanError> {
    let mut retained = String::new();
    retained.try_reserve_exact(value.len()).map_err(|_| {
        InteriorSimplexPlanError::AllocationFailure {
            resource,
            requested: value.len(),
        }
    })?;
    retained.push_str(value);
    Ok(Arc::new(retained))
}
