use std::sync::Arc;

use super::InteriorSimplexPlanError;
use super::freeze::FrozenSelectedScope;
use super::model::{
    InteriorSimplexGeometryEpochIdentity, InteriorSimplexTask, InteriorSimplexTaskKey,
};
use super::resource::try_reserve_exact;
use super::target::try_chart_point_to_target_shift;

#[allow(clippy::too_many_arguments)]
pub(super) fn try_build_tasks(
    epoch_identity: InteriorSimplexGeometryEpochIdentity,
    epoch_ordinal: u64,
    scopes: &[FrozenSelectedScope],
    offsets: &[Arc<Vec<u64>>],
    interior_margin: u64,
    expected_task_count: usize,
    max_arity: usize,
) -> Result<Vec<InteriorSimplexTask>, InteriorSimplexPlanError> {
    let mut tasks = Vec::new();
    try_reserve_exact(&mut tasks, expected_task_count, "interior-simplex tasks")?;
    let round_count = scopes.iter().map(|scope| scope.boxes.len()).max().ok_or(
        InteriorSimplexPlanError::Invariant {
            detail: "selected interior-simplex geometry has no scopes",
        },
    )?;

    for offset in offsets {
        for round in 0..round_count {
            for scope in scopes {
                let Some(selected_box) = scope.boxes.get(round) else {
                    continue;
                };
                let mut lattice_target = Vec::new();
                try_reserve_exact(
                    &mut lattice_target,
                    selected_box.key.arity(),
                    "lattice-target coordinates",
                )?;
                lattice_target.extend_from_slice(selected_box.key.lower());
                for (free_axis_rank, &position) in selected_box.free_axes.iter().enumerate() {
                    lattice_target[position] = lattice_target[position]
                        .checked_add(interior_margin)
                        .and_then(|coordinate| coordinate.checked_add(offset[free_axis_rank]))
                        .ok_or(InteriorSimplexPlanError::Invariant {
                            detail: "a preflighted simplex coordinate overflowed",
                        })?;
                }
                let target_shift =
                    try_chart_point_to_target_shift(&scope.key, &lattice_target, max_arity)?;
                let canonical_ordinal = tasks.len();
                if canonical_ordinal >= expected_task_count {
                    return Err(InteriorSimplexPlanError::Invariant {
                        detail: "simplex task construction exceeded its exact preflight",
                    });
                }
                tasks.push(InteriorSimplexTask::new(
                    epoch_identity.clone(),
                    epoch_ordinal,
                    canonical_ordinal,
                    InteriorSimplexTaskKey::new(
                        scope.key.clone(),
                        selected_box.key.clone(),
                        interior_margin,
                        offset.clone(),
                    ),
                    lattice_target,
                    target_shift,
                ));
            }
        }
    }
    if tasks.len() != expected_task_count {
        return Err(InteriorSimplexPlanError::Invariant {
            detail: "round-robin simplex construction lost a selected box or sample",
        });
    }
    Ok(tasks)
}
