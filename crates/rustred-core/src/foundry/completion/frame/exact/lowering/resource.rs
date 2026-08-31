use super::ExactCircuitLoweringError;

pub(super) const SELECTED_ROWS: &str = "selected source rows";
pub(super) const SELECTED_SOURCE_TERMS: &str = "selected source terms";
pub(super) const GUARD_ORIGINS: &str = "guard origins";
pub(super) const GUARD_SOURCES: &str = "guard condition sources";
pub(super) const GUARD_COORDINATES: &str = "guard provenance coordinate cells";
pub(super) const REPLAY_OPERATIONS: &str = "full-span replay exact operations";

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactCircuitLoweringError> {
    left.checked_add(right)
        .ok_or(ExactCircuitLoweringError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactCircuitLoweringError> {
    left.checked_mul(right)
        .ok_or(ExactCircuitLoweringError::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ExactCircuitLoweringError> {
    if requested > limit {
        Err(ExactCircuitLoweringError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, ExactCircuitLoweringError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        ExactCircuitLoweringError::AllocationFailure {
            resource,
            requested: capacity,
        }
    })?;
    Ok(values)
}
