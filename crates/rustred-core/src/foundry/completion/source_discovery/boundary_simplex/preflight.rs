use crate::foundry::completion::LatticeBox;

use super::super::simplex_support::{
    checked_binomial, try_finite_assignment_count, try_simplex_sample_count,
};
use super::canonical::CanonicalScope;
use super::model::BoundarySimplexSamplingProfile;
use super::resource::{check_limit, checked_add, checked_mul, try_reserve_exact};
use super::target::try_chart_point_to_target_shift;
use super::{BoundarySimplexLimits, BoundarySimplexPlanError};

pub(super) const SCHEDULER_VISITS: &str = "scheduler visits";
const FINITE_ASSIGNMENTS: &str = "parent finite coordinate assignments";

pub(super) struct Preflight {
    pub(super) selected_scope_count: usize,
    pub(super) selected_parent_count: usize,
    pub(super) parent_round_count: usize,
    pub(super) faces_per_parent: usize,
    pub(super) boundary_face_count: usize,
    pub(super) parent_finite_assignments: usize,
    pub(super) face_finite_assignments: usize,
    pub(super) simplex_sample_count: usize,
    pub(super) subset_unrank_work_upper_bound: usize,
    pub(super) scheduler_workspace_entries: usize,
    pub(super) expected_scheduler_visits: usize,
    pub(super) task_count: usize,
}

pub(super) fn validate_profile(
    face_dimension: usize,
    profile: BoundarySimplexSamplingProfile,
    limits: BoundarySimplexLimits,
) -> Result<(), BoundarySimplexPlanError> {
    match profile {
        BoundarySimplexSamplingProfile::Simplex {
            interior_margin,
            polynomial_degree_ceiling,
        } => {
            if face_dimension == 0 {
                return Err(BoundarySimplexPlanError::SimplexProfileRequiresPositiveFaceDimension);
            }
            if interior_margin == 0 {
                return Err(BoundarySimplexPlanError::ZeroInteriorMargin);
            }
            if interior_margin > limits.max_interior_margin {
                return Err(BoundarySimplexPlanError::ValueLimit {
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
        }
        BoundarySimplexSamplingProfile::Vertex if face_dimension != 0 => {
            return Err(
                BoundarySimplexPlanError::VertexProfileRequiresZeroFaceDimension {
                    actual: face_dimension,
                },
            );
        }
        BoundarySimplexSamplingProfile::Vertex => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn try_preflight(
    scopes: &[CanonicalScope<'_>],
    parent_dimension: usize,
    codimension: usize,
    face_dimension: usize,
    profile: BoundarySimplexSamplingProfile,
    limits: BoundarySimplexLimits,
) -> Result<Preflight, BoundarySimplexPlanError> {
    let faces_per_parent =
        checked_binomial(parent_dimension, codimension, "boundary faces per parent")?;
    check_limit(
        "boundary faces per parent",
        faces_per_parent,
        limits.max_faces_per_parent,
    )?;
    let degree = match profile {
        BoundarySimplexSamplingProfile::Simplex {
            polynomial_degree_ceiling,
            ..
        } => polynomial_degree_ceiling,
        BoundarySimplexSamplingProfile::Vertex => 0,
    };
    let simplex_samples = try_simplex_sample_count(face_dimension, degree)?;
    check_limit(
        "complete simplex samples",
        simplex_samples,
        limits.max_simplex_samples,
    )?;
    let simplex_cells = checked_mul(
        "simplex offset coordinate cells",
        simplex_samples,
        face_dimension,
    )?;
    check_limit(
        "simplex offset coordinate cells",
        simplex_cells,
        limits.max_simplex_coordinate_cells,
    )?;

    let mut selected_scopes = 0usize;
    let mut selected_parents = 0usize;
    let mut parent_round_count = 0usize;
    let mut parent_cells = 0usize;
    let mut boundary_faces = 0usize;
    let mut face_axis_cells = 0usize;
    let mut parent_assignments = 0usize;
    let mut face_assignments = 0usize;
    let mut tasks = 0usize;
    let mut task_cells = 0usize;

    // Aggregate every combinatorial result and scheduler envelope before any
    // worst-target buffer or chart-conversion allocation is attempted.
    for scope in scopes {
        let selected_in_scope = scope
            .boxes
            .iter()
            .filter(|lattice_box| lattice_box.free_dimension() == parent_dimension)
            .count();
        if selected_in_scope == 0 {
            continue;
        }
        selected_scopes = checked_add("selected canonical scopes", selected_scopes, 1)?;
        parent_round_count = parent_round_count.max(selected_in_scope);
        selected_parents =
            checked_add("selected parent boxes", selected_parents, selected_in_scope)?;
        check_limit(
            "selected parent boxes",
            selected_parents,
            limits.max_selected_parent_boxes,
        )?;

        for lattice_box in scope
            .boxes
            .iter()
            .copied()
            .filter(|lattice_box| lattice_box.free_dimension() == parent_dimension)
        {
            parent_cells = checked_add(
                "selected parent-box coordinate cells",
                parent_cells,
                checked_mul(
                    "selected parent-box coordinate cells",
                    lattice_box.arity(),
                    2,
                )?,
            )?;
            check_limit(
                "selected parent-box coordinate cells",
                parent_cells,
                limits.max_selected_parent_coordinate_cells,
            )?;
            boundary_faces = checked_add("boundary faces", boundary_faces, faces_per_parent)?;
            check_limit("boundary faces", boundary_faces, limits.max_boundary_faces)?;
            face_axis_cells = checked_add(
                "boundary-face axis cells",
                face_axis_cells,
                checked_mul(
                    "boundary-face axis cells",
                    faces_per_parent,
                    parent_dimension,
                )?,
            )?;
            check_limit(
                "boundary-face axis cells",
                face_axis_cells,
                limits.max_boundary_face_axis_cells,
            )?;
            let assignments = try_finite_assignment_count(
                lattice_box.lower(),
                lattice_box.upper(),
                FINITE_ASSIGNMENTS,
            )?;
            check_limit(
                "finite assignments per parent box",
                assignments,
                limits.max_finite_assignments_per_parent,
            )?;
            parent_assignments =
                checked_add("parent finite assignments", parent_assignments, assignments)?;
            check_limit(
                "parent finite assignments",
                parent_assignments,
                limits.max_parent_finite_assignments,
            )?;
            let parent_face_assignments =
                checked_mul("face finite assignments", assignments, faces_per_parent)?;
            face_assignments = checked_add(
                "face finite assignments",
                face_assignments,
                parent_face_assignments,
            )?;
            check_limit(
                "face finite assignments",
                face_assignments,
                limits.max_face_finite_assignments,
            )?;
            let parent_tasks = checked_mul(
                "boundary-simplex tasks",
                parent_face_assignments,
                simplex_samples,
            )?;
            tasks = checked_add("boundary-simplex tasks", tasks, parent_tasks)?;
            check_limit("boundary-simplex tasks", tasks, limits.max_tasks)?;
            task_cells = checked_add(
                "boundary-simplex task coordinate cells",
                task_cells,
                checked_mul(
                    "boundary-simplex task coordinate cells",
                    parent_tasks,
                    checked_mul(
                        "boundary-simplex task coordinate cells",
                        lattice_box.arity(),
                        2,
                    )?,
                )?,
            )?;
            check_limit(
                "boundary-simplex task coordinate cells",
                task_cells,
                limits.max_task_coordinate_cells,
            )?;
        }
    }
    if selected_parents == 0 {
        return Err(BoundarySimplexPlanError::Invariant {
            detail: "an available parent free dimension selected no boxes",
        });
    }

    let subset_unrank_work_upper_bound = checked_mul(
        "boundary subset unrank work",
        boundary_faces,
        checked_mul(
            "boundary subset unrank work",
            parent_dimension,
            parent_dimension,
        )?,
    )?;
    check_limit(
        "boundary subset unrank work",
        subset_unrank_work_upper_bound,
        limits.max_subset_unrank_work,
    )?;

    let parent_workspace = checked_add(
        "scheduler workspace entries",
        checked_mul("scheduler workspace entries", selected_parents, 2)?,
        checked_mul("scheduler workspace entries", parent_round_count, 2)?,
    )?;
    let face_build_workspace = checked_add(
        "scheduler workspace entries",
        parent_workspace,
        boundary_faces,
    )?;
    let task_workspace = checked_mul("scheduler workspace entries", boundary_faces, 3)?;
    let scheduler_workspace_entries = face_build_workspace.max(task_workspace);
    check_limit(
        "scheduler workspace entries",
        scheduler_workspace_entries,
        limits.max_scheduler_workspace_entries,
    )?;

    let flatten_visits = checked_add(
        SCHEDULER_VISITS,
        checked_mul(SCHEDULER_VISITS, selected_parents, 2)?,
        parent_round_count,
    )?;
    let task_visits_per_offset = checked_add(SCHEDULER_VISITS, boundary_faces, face_assignments)?;
    let expected_scheduler_visits = checked_add(
        SCHEDULER_VISITS,
        checked_add(SCHEDULER_VISITS, flatten_visits, boundary_faces)?,
        checked_mul(SCHEDULER_VISITS, simplex_samples, task_visits_per_offset)?,
    )?;
    check_limit(
        SCHEDULER_VISITS,
        expected_scheduler_visits,
        limits.max_scheduler_visits,
    )?;

    // Only after every aggregate cap succeeds may chart conversion allocate a
    // bounded worst-case point/shift for each selected parent.
    for (scope_ordinal, scope) in scopes.iter().enumerate() {
        for (parent_ordinal, lattice_box) in scope
            .boxes
            .iter()
            .copied()
            .filter(|lattice_box| lattice_box.free_dimension() == parent_dimension)
            .enumerate()
        {
            preflight_worst_target(
                scope_ordinal,
                parent_ordinal,
                scope.input.sector,
                lattice_box,
                profile,
                limits.max_arity,
            )?;
        }
    }

    Ok(Preflight {
        selected_scope_count: selected_scopes,
        selected_parent_count: selected_parents,
        parent_round_count,
        faces_per_parent,
        boundary_face_count: boundary_faces,
        parent_finite_assignments: parent_assignments,
        face_finite_assignments: face_assignments,
        simplex_sample_count: simplex_samples,
        subset_unrank_work_upper_bound,
        scheduler_workspace_entries,
        expected_scheduler_visits,
        task_count: tasks,
    })
}

fn preflight_worst_target(
    canonical_scope_ordinal: usize,
    parent_box_ordinal: usize,
    sector: &crate::sector::Mask,
    lattice_box: &LatticeBox,
    profile: BoundarySimplexSamplingProfile,
    max_arity: usize,
) -> Result<(), BoundarySimplexPlanError> {
    let mut worst = Vec::new();
    try_reserve_exact(&mut worst, lattice_box.arity(), "worst-case lattice target")?;
    worst.extend_from_slice(lattice_box.lower());
    for (position, &upper) in lattice_box.upper().iter().enumerate() {
        match upper {
            Some(upper) => worst[position] = upper,
            None => {
                if let BoundarySimplexSamplingProfile::Simplex {
                    interior_margin,
                    polynomial_degree_ceiling,
                } = profile
                {
                    let degree = u64::try_from(polynomial_degree_ceiling).map_err(|_| {
                        BoundarySimplexPlanError::ResourceCountOverflow {
                            resource: "simplex offset coordinate",
                        }
                    })?;
                    worst[position] = worst[position]
                        .checked_add(interior_margin)
                        .and_then(|coordinate| coordinate.checked_add(degree))
                        .ok_or(BoundarySimplexPlanError::CoordinateOverflow {
                            canonical_scope_ordinal,
                            parent_box_ordinal,
                            position,
                        })?;
                }
            }
        }
    }
    let _ = try_chart_point_to_target_shift(sector, &worst, max_arity)?;
    Ok(())
}
