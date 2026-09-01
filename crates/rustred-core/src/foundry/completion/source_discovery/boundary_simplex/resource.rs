use std::sync::Arc;

use super::BoundarySimplexPlanError;

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, BoundarySimplexPlanError> {
    left.checked_add(right)
        .ok_or(BoundarySimplexPlanError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, BoundarySimplexPlanError> {
    left.checked_mul(right)
        .ok_or(BoundarySimplexPlanError::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), BoundarySimplexPlanError> {
    if requested > limit {
        Err(BoundarySimplexPlanError::ResourceLimit {
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
) -> Result<(), BoundarySimplexPlanError> {
    let requested = checked_add(resource, retained.len(), additional)?;
    retained.try_reserve_exact(additional).map_err(|_| {
        BoundarySimplexPlanError::AllocationFailure {
            resource,
            requested,
        }
    })
}

pub(super) fn try_reserve_one<T>(
    retained: &mut Vec<T>,
    resource: &'static str,
) -> Result<(), BoundarySimplexPlanError> {
    try_reserve_exact(retained, 1, resource)
}

pub(super) fn try_copy_string(
    value: &str,
    resource: &'static str,
) -> Result<Arc<String>, BoundarySimplexPlanError> {
    let mut retained = String::new();
    retained.try_reserve_exact(value.len()).map_err(|_| {
        BoundarySimplexPlanError::AllocationFailure {
            resource,
            requested: value.len(),
        }
    })?;
    retained.push_str(value);
    Ok(Arc::new(retained))
}

pub(super) fn logical_sort_work(len: usize) -> Result<usize, BoundarySimplexPlanError> {
    if len <= 1 {
        return Ok(0);
    }
    let log = usize::try_from(usize::BITS - (len - 1).leading_zeros()).map_err(|_| {
        BoundarySimplexPlanError::ResourceCountOverflow {
            resource: "canonical logical sort work",
        }
    })?;
    checked_mul("canonical logical sort work", len, log)
}
