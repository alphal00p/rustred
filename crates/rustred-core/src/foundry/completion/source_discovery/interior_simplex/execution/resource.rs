use super::InteriorSimplexExecutionError;

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, InteriorSimplexExecutionError> {
    left.checked_add(right)
        .ok_or(InteriorSimplexExecutionError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, InteriorSimplexExecutionError> {
    left.checked_mul(right)
        .ok_or(InteriorSimplexExecutionError::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), InteriorSimplexExecutionError> {
    if requested > limit {
        Err(InteriorSimplexExecutionError::ResourceLimit {
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
) -> Result<(), InteriorSimplexExecutionError> {
    let requested = checked_add(resource, retained.len(), additional)?;
    retained.try_reserve_exact(additional).map_err(|_| {
        InteriorSimplexExecutionError::AllocationFailure {
            resource,
            requested,
        }
    })
}
