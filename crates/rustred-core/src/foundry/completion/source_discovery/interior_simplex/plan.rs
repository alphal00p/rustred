use super::InteriorSimplexPlanError;
use super::build::try_build_tasks;
use super::canonical::try_collect_canonical_scopes;
use super::freeze::try_freeze_maximal_geometry;
use super::limits::InteriorSimplexLimits;
use super::model::{
    InteriorSimplexGeometryEpochIdentity, InteriorSimplexPlan, InteriorSimplexScopePartition,
};
use super::resource::{check_limit, checked_mul};
use super::simplex::{try_build_simplex_offsets, try_simplex_sample_count};

/// Freeze maximal-free blind components and plan their complete interior
/// simplex samples.
///
/// Canonical scope chronology is sector, complete endpoint tuple, then stable
/// key. For each graded simplex offset, boxes are interleaved round-robin
/// across canonical scopes. Every count, coordinate cell, coordinate sum, and
/// retained variable-size allocation is checked before any plan escapes. A
/// returned plan is proposal geometry only and carries no execution or closure
/// authority.
pub(crate) fn try_plan_interior_simplex_samples<'a>(
    epoch_ordinal: u64,
    scopes: impl IntoIterator<Item = InteriorSimplexScopePartition<'a>>,
    interior_margin: u64,
    polynomial_degree_ceiling: usize,
    limits: InteriorSimplexLimits,
) -> Result<InteriorSimplexPlan, InteriorSimplexPlanError> {
    if interior_margin == 0 {
        return Err(InteriorSimplexPlanError::ZeroInteriorMargin);
    }
    if interior_margin > limits.max_interior_margin {
        return Err(InteriorSimplexPlanError::ValueLimit {
            resource: "interior margin",
            requested: interior_margin,
            limit: limits.max_interior_margin,
        });
    }
    check_limit(
        "polynomial degree ceiling",
        polynomial_degree_ceiling,
        limits.max_polynomial_degree_ceiling,
    )?;
    let degree_u64 = u64::try_from(polynomial_degree_ceiling).map_err(|_| {
        InteriorSimplexPlanError::ResourceCountOverflow {
            resource: "polynomial degree coordinate",
        }
    })?;

    let (canonical_scopes, maximal_free_dimension) = try_collect_canonical_scopes(scopes, limits)?;
    let simplex_sample_count =
        try_simplex_sample_count(maximal_free_dimension, polynomial_degree_ceiling)?;
    check_limit(
        "complete simplex samples",
        simplex_sample_count,
        limits.max_simplex_samples,
    )?;
    let simplex_coordinate_cells = checked_mul(
        "simplex offset coordinate cells",
        simplex_sample_count,
        maximal_free_dimension,
    )?;
    check_limit(
        "simplex offset coordinate cells",
        simplex_coordinate_cells,
        limits.max_simplex_coordinate_cells,
    )?;

    let input_scope_count = canonical_scopes.len();
    let frozen = try_freeze_maximal_geometry(
        &canonical_scopes,
        maximal_free_dimension,
        interior_margin,
        degree_u64,
        simplex_sample_count,
        limits,
    )?;

    // Construct the shared design only after all aggregate result sizes and
    // every selected box's worst coordinate have passed preflight.
    let offsets = try_build_simplex_offsets(
        maximal_free_dimension,
        polynomial_degree_ceiling,
        simplex_sample_count,
    )?;
    let epoch_identity = InteriorSimplexGeometryEpochIdentity::fresh();
    let tasks = try_build_tasks(
        epoch_identity.clone(),
        epoch_ordinal,
        &frozen.scopes,
        &offsets,
        interior_margin,
        frozen.total_tasks,
        limits.max_arity,
    )?;

    Ok(InteriorSimplexPlan {
        epoch_identity,
        epoch_ordinal,
        input_scope_count,
        selected_scope_count: frozen.scopes.len(),
        selected_box_count: frozen.selected_box_count,
        maximal_free_dimension,
        interior_margin,
        polynomial_degree_ceiling,
        simplex_sample_count,
        tasks,
    })
}
