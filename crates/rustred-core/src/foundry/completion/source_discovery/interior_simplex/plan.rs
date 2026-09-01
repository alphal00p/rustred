use super::InteriorSimplexPlanError;
use super::build::try_build_tasks;
use super::canonical::try_collect_canonical_scopes;
use super::freeze::try_freeze_selected_geometry;
use super::limits::InteriorSimplexLimits;
use super::model::{
    InteriorSimplexFreeDimensionSelection, InteriorSimplexGeometryEpochIdentity,
    InteriorSimplexPlan, InteriorSimplexScopePartition,
};
use super::resource::{check_limit, checked_mul};
use super::simplex::{try_build_simplex_offsets, try_simplex_sample_count};

/// Freeze maximal-free blind components and plan their complete interior
/// simplex samples.
///
/// Canonical scope chronology is sector, complete endpoint tuple, then stable
/// key. For each graded simplex offset, live boxes are interleaved round-robin
/// across canonical scopes and retired as soon as their finite product is
/// exhausted. Every count, coordinate cell, coordinate sum, scheduler visit,
/// and retained variable-size allocation is checked before any plan escapes.
/// A returned plan is proposal geometry only and carries no execution or
/// closure authority.
pub(crate) fn try_plan_interior_simplex_samples<'a>(
    epoch_ordinal: u64,
    scopes: impl IntoIterator<Item = InteriorSimplexScopePartition<'a>>,
    interior_margin: u64,
    polynomial_degree_ceiling: usize,
    limits: InteriorSimplexLimits,
) -> Result<InteriorSimplexPlan, InteriorSimplexPlanError> {
    try_plan_interior_simplex_samples_with_selection(
        epoch_ordinal,
        scopes,
        InteriorSimplexFreeDimensionSelection::Maximal,
        interior_margin,
        polynomial_degree_ceiling,
        limits,
    )
}

/// Freeze every box at one exact positive free dimension and plan its complete
/// interior-simplex samples.
///
/// Unlike the maximal-selection entry point, this never falls back to a
/// dimension that happens to exist. Missing, zero, and arity-invalid requests
/// are rejected distinctly so a fair outer driver can prove each dimension's
/// exhaustion without silently skipping geometry. Only boxes already having
/// the requested dimension are selected; this call does not slice boundary
/// faces out of higher-dimensional boxes.
pub(crate) fn try_plan_interior_simplex_samples_at_free_dimension<'a>(
    epoch_ordinal: u64,
    scopes: impl IntoIterator<Item = InteriorSimplexScopePartition<'a>>,
    requested_free_dimension: usize,
    interior_margin: u64,
    polynomial_degree_ceiling: usize,
    limits: InteriorSimplexLimits,
) -> Result<InteriorSimplexPlan, InteriorSimplexPlanError> {
    if requested_free_dimension == 0 {
        return Err(InteriorSimplexPlanError::ZeroRequestedFreeDimension);
    }
    try_plan_interior_simplex_samples_with_selection(
        epoch_ordinal,
        scopes,
        InteriorSimplexFreeDimensionSelection::Exact(requested_free_dimension),
        interior_margin,
        polynomial_degree_ceiling,
        limits,
    )
}

fn try_plan_interior_simplex_samples_with_selection<'a>(
    epoch_ordinal: u64,
    scopes: impl IntoIterator<Item = InteriorSimplexScopePartition<'a>>,
    free_dimension_selection: InteriorSimplexFreeDimensionSelection,
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

    let (canonical_scopes, maximal_free_dimension, maximal_input_arity) =
        try_collect_canonical_scopes(scopes, limits)?;
    let selected_free_dimension = match free_dimension_selection {
        InteriorSimplexFreeDimensionSelection::Maximal => {
            if maximal_free_dimension == 0 {
                return Err(InteriorSimplexPlanError::NoUnboundedGeometry);
            }
            maximal_free_dimension
        }
        InteriorSimplexFreeDimensionSelection::Exact(requested) => {
            if requested == 0 {
                return Err(InteriorSimplexPlanError::ZeroRequestedFreeDimension);
            }
            if requested > maximal_input_arity {
                return Err(InteriorSimplexPlanError::InvalidRequestedFreeDimension {
                    requested,
                    maximal_input_arity,
                });
            }
            let is_available = canonical_scopes.iter().any(|scope| {
                scope
                    .canonical_boxes
                    .iter()
                    .any(|lattice_box| lattice_box.free_dimension() == requested)
            });
            if !is_available {
                return Err(
                    InteriorSimplexPlanError::RequestedFreeDimensionUnavailable {
                        requested,
                        maximal_available: maximal_free_dimension,
                    },
                );
            }
            requested
        }
    };
    let simplex_sample_count =
        try_simplex_sample_count(selected_free_dimension, polynomial_degree_ceiling)?;
    check_limit(
        "complete simplex samples",
        simplex_sample_count,
        limits.max_simplex_samples,
    )?;
    let simplex_coordinate_cells = checked_mul(
        "simplex offset coordinate cells",
        simplex_sample_count,
        selected_free_dimension,
    )?;
    check_limit(
        "simplex offset coordinate cells",
        simplex_coordinate_cells,
        limits.max_simplex_coordinate_cells,
    )?;

    let input_scope_count = canonical_scopes.len();
    let frozen = try_freeze_selected_geometry(
        &canonical_scopes,
        selected_free_dimension,
        interior_margin,
        degree_u64,
        simplex_sample_count,
        limits,
    )?;

    // Construct the shared design only after all aggregate result sizes and
    // every selected box's worst coordinate have passed preflight.
    let offsets = try_build_simplex_offsets(
        selected_free_dimension,
        polynomial_degree_ceiling,
        simplex_sample_count,
    )?;
    let epoch_identity = InteriorSimplexGeometryEpochIdentity::fresh();
    let built = try_build_tasks(
        epoch_identity.clone(),
        epoch_ordinal,
        &frozen.scopes,
        &offsets,
        interior_margin,
        frozen.total_tasks,
        frozen.expected_scheduler_visits,
        limits.max_arity,
    )?;

    Ok(InteriorSimplexPlan {
        epoch_identity,
        epoch_ordinal,
        input_scope_count,
        selected_scope_count: frozen.scopes.len(),
        selected_box_count: frozen.selected_box_count,
        finite_assignment_count: frozen.finite_assignment_count,
        scheduler_workspace_entries: frozen.scheduler_workspace_entries,
        scheduler_visit_count: built.scheduler_visits,
        free_dimension_selection,
        selected_free_dimension,
        maximal_free_dimension,
        interior_margin,
        polynomial_degree_ceiling,
        simplex_sample_count,
        tasks: built.tasks,
    })
}
