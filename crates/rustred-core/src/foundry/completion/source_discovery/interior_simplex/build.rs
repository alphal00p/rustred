use std::sync::Arc;

use super::InteriorSimplexPlanError;
use super::bounded::try_apply_finite_assignment;
use super::freeze::FrozenSelectedScope;
use super::model::{
    InteriorSimplexGeometryEpochIdentity, InteriorSimplexTask, InteriorSimplexTaskKey,
};
use super::resource::{checked_add, try_reserve_exact};
use super::target::try_chart_point_to_target_shift;

const SCHEDULER_VISITS: &str = "scheduler visits";

pub(super) struct BuiltInteriorSimplexTasks {
    pub(super) tasks: Vec<InteriorSimplexTask>,
    pub(super) scheduler_visits: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FlatBoxPosition {
    scope_ordinal: usize,
    box_ordinal: usize,
}

impl FlatBoxPosition {
    const UNINITIALIZED: Self = Self {
        scope_ordinal: usize::MAX,
        box_ordinal: usize::MAX,
    };
}

#[allow(clippy::too_many_arguments)]
pub(super) fn try_build_tasks(
    epoch_identity: InteriorSimplexGeometryEpochIdentity,
    epoch_ordinal: u64,
    scopes: &[FrozenSelectedScope],
    offsets: &[Arc<Vec<u64>>],
    interior_margin: u64,
    expected_task_count: usize,
    expected_scheduler_visits: usize,
    max_arity: usize,
) -> Result<BuiltInteriorSimplexTasks, InteriorSimplexPlanError> {
    let selected_box_count = scopes.iter().try_fold(0usize, |count, scope| {
        checked_add("selected maximal boxes", count, scope.boxes.len())
    })?;
    let (flat_boxes, mut scheduler_visits) =
        try_flatten_canonical_boxes(scopes, selected_box_count)?;

    let mut tasks = Vec::new();
    try_reserve_exact(&mut tasks, expected_task_count, "interior-simplex tasks")?;
    let mut current_frontier = Vec::new();
    try_reserve_exact(
        &mut current_frontier,
        selected_box_count,
        "scheduler current frontier",
    )?;
    let mut next_frontier = Vec::new();
    try_reserve_exact(
        &mut next_frontier,
        selected_box_count,
        "scheduler next frontier",
    )?;

    for offset in offsets {
        if !current_frontier.is_empty() || !next_frontier.is_empty() {
            return Err(InteriorSimplexPlanError::Invariant {
                detail: "an interior-simplex offset began with a nonempty scheduler frontier",
            });
        }
        for flat_ordinal in 0..flat_boxes.len() {
            scheduler_visits = checked_add(SCHEDULER_VISITS, scheduler_visits, 1)?;
            current_frontier.push(flat_ordinal);
        }

        let mut finite_assignment_ordinal = 0usize;
        while !current_frontier.is_empty() {
            let next_assignment_ordinal = finite_assignment_ordinal.checked_add(1).ok_or(
                InteriorSimplexPlanError::ResourceCountOverflow {
                    resource: "finite assignment scheduler ordinal",
                },
            )?;
            for &flat_ordinal in &current_frontier {
                scheduler_visits = checked_add(SCHEDULER_VISITS, scheduler_visits, 1)?;
                let position =
                    flat_boxes
                        .get(flat_ordinal)
                        .ok_or(InteriorSimplexPlanError::Invariant {
                            detail: "the active scheduler frontier referenced no flattened box",
                        })?;
                let scope = scopes.get(position.scope_ordinal).ok_or(
                    InteriorSimplexPlanError::Invariant {
                        detail: "the flattened scheduler referenced no selected scope",
                    },
                )?;
                let selected_box = scope.boxes.get(position.box_ordinal).ok_or(
                    InteriorSimplexPlanError::Invariant {
                        detail: "the flattened scheduler referenced no selected box",
                    },
                )?;
                if finite_assignment_ordinal >= selected_box.finite_assignment_count {
                    return Err(InteriorSimplexPlanError::Invariant {
                        detail: "the active scheduler frontier retained an exhausted box",
                    });
                }

                let mut lattice_target = Vec::new();
                try_reserve_exact(
                    &mut lattice_target,
                    selected_box.key.arity(),
                    "lattice-target coordinates",
                )?;
                lattice_target.extend_from_slice(selected_box.key.lower());
                try_apply_finite_assignment(
                    selected_box.key.lower(),
                    selected_box.key.upper(),
                    finite_assignment_ordinal,
                    &mut lattice_target,
                )?;
                for (free_axis_rank, &coordinate_position) in
                    selected_box.free_axes.iter().enumerate()
                {
                    lattice_target[coordinate_position] = lattice_target[coordinate_position]
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
                        finite_assignment_ordinal,
                    ),
                    lattice_target,
                    target_shift,
                ));

                if selected_box.finite_assignment_count > next_assignment_ordinal {
                    next_frontier.push(flat_ordinal);
                }
            }
            current_frontier.clear();
            std::mem::swap(&mut current_frontier, &mut next_frontier);
            finite_assignment_ordinal = next_assignment_ordinal;
        }
    }
    if tasks.len() != expected_task_count {
        return Err(InteriorSimplexPlanError::Invariant {
            detail: "active-frontier construction lost a selected box assignment or sample",
        });
    }
    if scheduler_visits != expected_scheduler_visits {
        return Err(InteriorSimplexPlanError::Invariant {
            detail: "scheduler visits differed from the exact aggregate preflight",
        });
    }
    Ok(BuiltInteriorSimplexTasks {
        tasks,
        scheduler_visits,
    })
}

/// Flatten `(box round, canonical scope)` chronology without scanning the
/// rectangular `max_boxes_per_scope * scope_count` envelope. Two linear box
/// passes and one round-prefix pass place every selected box exactly once.
fn try_flatten_canonical_boxes(
    scopes: &[FrozenSelectedScope],
    expected_box_count: usize,
) -> Result<(Vec<FlatBoxPosition>, usize), InteriorSimplexPlanError> {
    let round_count = scopes.iter().map(|scope| scope.boxes.len()).max().ok_or(
        InteriorSimplexPlanError::Invariant {
            detail: "selected interior-simplex geometry has no scopes",
        },
    )?;
    if round_count == 0 {
        return Err(InteriorSimplexPlanError::Invariant {
            detail: "selected interior-simplex geometry has no boxes",
        });
    }

    let mut round_starts = Vec::new();
    try_reserve_exact(&mut round_starts, round_count, "scheduler round starts")?;
    round_starts.resize(round_count, 0usize);
    let mut visits = 0usize;
    for scope in scopes {
        for box_ordinal in 0..scope.boxes.len() {
            visits = checked_add(SCHEDULER_VISITS, visits, 1)?;
            round_starts[box_ordinal] =
                checked_add("scheduler boxes in one round", round_starts[box_ordinal], 1)?;
        }
    }

    let mut flattened_count = 0usize;
    for start in &mut round_starts {
        visits = checked_add(SCHEDULER_VISITS, visits, 1)?;
        let count = *start;
        *start = flattened_count;
        flattened_count = checked_add("flattened scheduler boxes", flattened_count, count)?;
    }
    if flattened_count != expected_box_count {
        return Err(InteriorSimplexPlanError::Invariant {
            detail: "scheduler round counts differed from selected-box preflight",
        });
    }

    let mut round_written = Vec::new();
    try_reserve_exact(
        &mut round_written,
        round_count,
        "scheduler round write cursors",
    )?;
    round_written.resize(round_count, 0usize);
    let mut flat_boxes = Vec::new();
    try_reserve_exact(
        &mut flat_boxes,
        expected_box_count,
        "flattened scheduler boxes",
    )?;
    flat_boxes.resize(expected_box_count, FlatBoxPosition::UNINITIALIZED);

    for (scope_ordinal, scope) in scopes.iter().enumerate() {
        for box_ordinal in 0..scope.boxes.len() {
            visits = checked_add(SCHEDULER_VISITS, visits, 1)?;
            let position = checked_add(
                "flattened scheduler box position",
                round_starts[box_ordinal],
                round_written[box_ordinal],
            )?;
            let slot = flat_boxes
                .get_mut(position)
                .ok_or(InteriorSimplexPlanError::Invariant {
                    detail: "a scheduler round exceeded its flattened box allocation",
                })?;
            if *slot != FlatBoxPosition::UNINITIALIZED {
                return Err(InteriorSimplexPlanError::Invariant {
                    detail: "two scheduler boxes occupied one flattened position",
                });
            }
            *slot = FlatBoxPosition {
                scope_ordinal,
                box_ordinal,
            };
            round_written[box_ordinal] = checked_add(
                "scheduler round write cursor",
                round_written[box_ordinal],
                1,
            )?;
        }
    }
    if flat_boxes
        .iter()
        .any(|position| *position == FlatBoxPosition::UNINITIALIZED)
    {
        return Err(InteriorSimplexPlanError::Invariant {
            detail: "the canonical scheduler left an uninitialized flattened box",
        });
    }
    Ok((flat_boxes, visits))
}
