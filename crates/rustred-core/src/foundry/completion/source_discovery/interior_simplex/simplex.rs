use std::sync::Arc;

use super::super::simplex_support;
use super::InteriorSimplexPlanError;

/// Interior-typed adapter over the shared topology-neutral simplex counter.
pub(super) fn try_simplex_sample_count(
    free_dimension: usize,
    degree: usize,
) -> Result<usize, InteriorSimplexPlanError> {
    simplex_support::try_simplex_sample_count(free_dimension, degree).map_err(Into::into)
}

/// Interior-typed adapter over the shared canonical simplex enumeration.
pub(super) fn try_build_simplex_offsets(
    free_dimension: usize,
    degree_ceiling: usize,
    expected_count: usize,
) -> Result<Vec<Arc<Vec<u64>>>, InteriorSimplexPlanError> {
    if free_dimension == 0 {
        return Err(InteriorSimplexPlanError::Invariant {
            detail: "an interior simplex requires a positive free dimension",
        });
    }
    simplex_support::try_build_simplex_offsets(free_dimension, degree_ceiling, expected_count)
        .map_err(Into::into)
}
