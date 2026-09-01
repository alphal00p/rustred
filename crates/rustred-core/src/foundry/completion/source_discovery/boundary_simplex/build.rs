use std::sync::Arc;

use super::super::simplex_support::try_apply_finite_assignment;
use super::BoundarySimplexPlanError;
use super::freeze::FrozenFace;
use super::model::{
    BoundarySimplexGeometryEpochIdentity, BoundarySimplexSamplingProfile, BoundarySimplexTask,
    BoundarySimplexTaskKey,
};
use super::preflight::SCHEDULER_VISITS;
use super::resource::{checked_add, try_reserve_exact};
use super::target::try_chart_point_to_target_shift;

#[allow(clippy::too_many_arguments)]
pub(super) fn try_build_tasks(
    epoch_identity: BoundarySimplexGeometryEpochIdentity,
    epoch_ordinal: u64,
    faces: &[FrozenFace],
    offsets: &[Arc<Vec<u64>>],
    parent_dimension: usize,
    codimension: usize,
    profile: BoundarySimplexSamplingProfile,
    expected_task_count: usize,
    scheduler_visits: &mut usize,
    max_arity: usize,
) -> Result<Vec<BoundarySimplexTask>, BoundarySimplexPlanError> {
    let mut tasks = Vec::new();
    try_reserve_exact(&mut tasks, expected_task_count, "boundary-simplex tasks")?;
    let mut current_frontier = Vec::new();
    try_reserve_exact(
        &mut current_frontier,
        faces.len(),
        "scheduler current frontier",
    )?;
    let mut next_frontier = Vec::new();
    try_reserve_exact(&mut next_frontier, faces.len(), "scheduler next frontier")?;
    let margin = match profile {
        BoundarySimplexSamplingProfile::Simplex {
            interior_margin, ..
        } => Some(interior_margin),
        BoundarySimplexSamplingProfile::Vertex => None,
    };

    for offset in offsets {
        if !current_frontier.is_empty() || !next_frontier.is_empty() {
            return Err(BoundarySimplexPlanError::Invariant {
                detail: "a boundary-simplex offset began with a nonempty frontier",
            });
        }
        for face_ordinal in 0..faces.len() {
            *scheduler_visits = checked_add(SCHEDULER_VISITS, *scheduler_visits, 1)?;
            current_frontier.push(face_ordinal);
        }
        let mut finite_assignment_ordinal = 0usize;
        while !current_frontier.is_empty() {
            let next_assignment_ordinal = finite_assignment_ordinal.checked_add(1).ok_or(
                BoundarySimplexPlanError::ResourceCountOverflow {
                    resource: "finite assignment scheduler ordinal",
                },
            )?;
            for &face_ordinal in &current_frontier {
                *scheduler_visits = checked_add(SCHEDULER_VISITS, *scheduler_visits, 1)?;
                let face = faces
                    .get(face_ordinal)
                    .ok_or(BoundarySimplexPlanError::Invariant {
                        detail: "the active frontier referenced no frozen face",
                    })?;
                if finite_assignment_ordinal >= face.finite_assignment_count {
                    return Err(BoundarySimplexPlanError::Invariant {
                        detail: "the active frontier retained an exhausted face",
                    });
                }
                let parent = face.key.parent();
                let mut lattice_target = Vec::new();
                try_reserve_exact(
                    &mut lattice_target,
                    parent.arity(),
                    "lattice-target coordinates",
                )?;
                lattice_target.extend_from_slice(parent.lower());
                try_apply_finite_assignment(
                    parent.lower(),
                    parent.upper(),
                    finite_assignment_ordinal,
                    &mut lattice_target,
                )?;
                if let Some(margin) = margin {
                    for (axis_rank, &ambient_axis) in face.key.remaining_axes().iter().enumerate() {
                        let offset_coordinate = offset.get(axis_rank).copied().ok_or(
                            BoundarySimplexPlanError::Invariant {
                                detail: "a simplex offset omitted a remaining face axis",
                            },
                        )?;
                        let coordinate = lattice_target.get_mut(ambient_axis).ok_or(
                            BoundarySimplexPlanError::Invariant {
                                detail: "a remaining face axis exceeded its parent arity",
                            },
                        )?;
                        *coordinate = coordinate
                            .checked_add(margin)
                            .and_then(|coordinate| coordinate.checked_add(offset_coordinate))
                            .ok_or(BoundarySimplexPlanError::Invariant {
                                detail: "a preflighted boundary coordinate overflowed",
                            })?;
                    }
                }
                let target_shift = try_chart_point_to_target_shift(
                    face.scope.sector(),
                    &lattice_target,
                    max_arity,
                )?;
                let canonical_ordinal = tasks.len();
                if canonical_ordinal >= expected_task_count {
                    return Err(BoundarySimplexPlanError::Invariant {
                        detail: "task construction exceeded its exact preflight",
                    });
                }
                tasks.push(BoundarySimplexTask::new(
                    epoch_identity.clone(),
                    epoch_ordinal,
                    canonical_ordinal,
                    BoundarySimplexTaskKey::new(
                        face.scope.clone(),
                        face.key.clone(),
                        parent_dimension,
                        codimension,
                        profile,
                        offset.clone(),
                        finite_assignment_ordinal,
                    ),
                    lattice_target,
                    target_shift,
                ));
                if face.finite_assignment_count > next_assignment_ordinal {
                    next_frontier.push(face_ordinal);
                }
            }
            current_frontier.clear();
            std::mem::swap(&mut current_frontier, &mut next_frontier);
            finite_assignment_ordinal = next_assignment_ordinal;
        }
    }
    if tasks.len() != expected_task_count {
        return Err(BoundarySimplexPlanError::Invariant {
            detail: "active-frontier construction lost a face assignment or simplex sample",
        });
    }
    Ok(tasks)
}
