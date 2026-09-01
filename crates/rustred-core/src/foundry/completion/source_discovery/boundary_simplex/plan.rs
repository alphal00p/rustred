use super::super::simplex_support::try_build_simplex_offsets;
use super::build::try_build_tasks;
use super::canonical::try_collect_canonical_scopes;
use super::freeze::{try_build_faces, try_freeze_parents};
use super::model::{
    BoundarySimplexGeometryEpochIdentity, BoundarySimplexPlan, BoundarySimplexSamplingProfile,
    BoundarySimplexScopePartition,
};
use super::preflight::{try_preflight, validate_profile};
use super::{BoundarySimplexLimits, BoundarySimplexPlanError};

/// Freeze every boundary face at one exact codimension of every parent box at
/// one exact free dimension, then plan its complete finite-assignment ×
/// simplex design.
///
/// `Simplex` is required when at least one unbounded axis remains. `Vertex` is
/// required when all parent free axes are pinned, including finite parents at
/// `d=0,c=0`. Faces exist only in this proposal plan; no partition is mutated.
#[allow(clippy::too_many_arguments)]
pub(crate) fn try_plan_boundary_simplex_samples<'a>(
    epoch_ordinal: u64,
    scopes: impl IntoIterator<Item = BoundarySimplexScopePartition<'a>>,
    parent_free_dimension: usize,
    boundary_codimension: usize,
    profile: BoundarySimplexSamplingProfile,
    limits: BoundarySimplexLimits,
) -> Result<BoundarySimplexPlan, BoundarySimplexPlanError> {
    if boundary_codimension > parent_free_dimension {
        return Err(BoundarySimplexPlanError::InvalidBoundaryCodimension {
            parent_free_dimension,
            requested: boundary_codimension,
        });
    }
    let face_dimension = parent_free_dimension - boundary_codimension;
    validate_profile(face_dimension, profile, limits)?;

    let (canonical_scopes, maximal_available_free_dimension, maximal_input_arity) =
        try_collect_canonical_scopes(scopes, limits)?;
    if parent_free_dimension > maximal_input_arity {
        return Err(BoundarySimplexPlanError::InvalidParentFreeDimension {
            requested: parent_free_dimension,
            maximal_input_arity,
        });
    }
    if !canonical_scopes.iter().any(|scope| {
        scope
            .boxes
            .iter()
            .any(|lattice_box| lattice_box.free_dimension() == parent_free_dimension)
    }) {
        return Err(BoundarySimplexPlanError::ParentFreeDimensionUnavailable {
            requested: parent_free_dimension,
            maximal_available: maximal_available_free_dimension,
        });
    }

    let preflight = try_preflight(
        &canonical_scopes,
        parent_free_dimension,
        boundary_codimension,
        face_dimension,
        profile,
        limits,
    )?;
    let frozen_scopes = try_freeze_parents(
        &canonical_scopes,
        parent_free_dimension,
        preflight.selected_scope_count,
        preflight.selected_parent_count,
    )?;
    let (faces, mut scheduler_visits) = try_build_faces(
        &frozen_scopes,
        preflight.selected_parent_count,
        preflight.parent_round_count,
        boundary_codimension,
        preflight.faces_per_parent,
        preflight.boundary_face_count,
    )?;
    drop(frozen_scopes);

    let degree = match profile {
        BoundarySimplexSamplingProfile::Simplex {
            polynomial_degree_ceiling,
            ..
        } => polynomial_degree_ceiling,
        BoundarySimplexSamplingProfile::Vertex => 0,
    };
    let offsets =
        try_build_simplex_offsets(face_dimension, degree, preflight.simplex_sample_count)?;
    let epoch_identity = BoundarySimplexGeometryEpochIdentity::fresh();
    let tasks = try_build_tasks(
        epoch_identity.clone(),
        epoch_ordinal,
        &faces,
        &offsets,
        parent_free_dimension,
        boundary_codimension,
        profile,
        preflight.task_count,
        &mut scheduler_visits,
        limits.max_arity,
    )?;
    if scheduler_visits != preflight.expected_scheduler_visits {
        return Err(BoundarySimplexPlanError::Invariant {
            detail: "scheduler visits differed from the exact aggregate preflight",
        });
    }

    Ok(BoundarySimplexPlan {
        epoch_identity,
        epoch_ordinal,
        input_scope_count: canonical_scopes.len(),
        selected_scope_count: preflight.selected_scope_count,
        selected_parent_box_count: preflight.selected_parent_count,
        boundary_face_count: preflight.boundary_face_count,
        parent_finite_assignment_count: preflight.parent_finite_assignments,
        face_finite_assignment_count: preflight.face_finite_assignments,
        scheduler_workspace_entries: preflight.scheduler_workspace_entries,
        scheduler_visit_count: scheduler_visits,
        subset_unrank_work_upper_bound: preflight.subset_unrank_work_upper_bound,
        parent_free_dimension,
        boundary_codimension,
        face_dimension,
        maximal_available_free_dimension,
        profile,
        simplex_sample_count: preflight.simplex_sample_count,
        tasks,
    })
}
