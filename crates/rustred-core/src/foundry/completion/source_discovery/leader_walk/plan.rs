use std::sync::Arc;

use crate::foundry::completion::{LatticeBox, LatticePoint, SectorChart};
use crate::identity::IntegralShift;

use super::error::LeaderWalkPlanError;
use super::limits::LeaderWalkLimits;
use super::model::{
    LeaderWalkBoxKey, LeaderWalkDepth, LeaderWalkGeometryEpochIdentity, LeaderWalkPlan,
    LeaderWalkScopeKey, LeaderWalkScopePartition, LeaderWalkTask, LeaderWalkTaskKey,
    LeaderWalkWave,
};

struct CanonicalScope<'a> {
    original_input_ordinal: usize,
    input: LeaderWalkScopePartition<'a>,
    canonical_boxes: Vec<&'a LatticeBox>,
}

struct FrozenSelectedBox {
    key: LeaderWalkBoxKey,
    free_axes: Box<[usize]>,
}

struct FrozenSelectedScope {
    key: LeaderWalkScopeKey,
    boxes: Vec<FrozenSelectedBox>,
}

/// Freeze and plan the complete maximal-free-dimension leader census.
///
/// Scopes are ordered by sector and complete canonical endpoint tuple; stable
/// scope identity is only a tie-breaker. Boxes are interleaved round-robin
/// across canonical scopes. In the second wave, each selected box contributes
/// one separate task per unbounded axis, also interleaved by axis rank and
/// scope. Caps are preflighted over the complete two-wave result; no cap can
/// silently bias the census.
pub(crate) fn try_plan_maximal_orthant_leader_walk<'a>(
    epoch_ordinal: u64,
    scopes: impl IntoIterator<Item = LeaderWalkScopePartition<'a>>,
    limits: LeaderWalkLimits,
) -> Result<LeaderWalkPlan, LeaderWalkPlanError> {
    let mut canonical_scopes = Vec::new();
    let mut aggregate_scope_key_bytes = 0usize;
    let mut input_boxes = 0usize;
    let mut input_box_coordinate_cells = 0usize;
    let mut maximal_free_dimension = 0usize;

    for (input_ordinal, input) in scopes.into_iter().enumerate() {
        let requested_scopes = checked_add("input scopes", canonical_scopes.len(), 1)?;
        check_limit("input scopes", requested_scopes, limits.max_scopes)?;
        if input.stable_scope_key.is_empty() {
            return Err(LeaderWalkPlanError::EmptyStableScopeKey { input_ordinal });
        }
        aggregate_scope_key_bytes = checked_add(
            "aggregate stable-scope-key bytes",
            aggregate_scope_key_bytes,
            input.stable_scope_key.len(),
        )?;
        check_limit(
            "aggregate stable-scope-key bytes",
            aggregate_scope_key_bytes,
            limits.max_aggregate_scope_key_bytes,
        )?;
        check_limit("scope arity", input.sector.arity(), limits.max_arity)?;

        let mut canonical_boxes = Vec::new();
        try_reserve_exact(
            &mut canonical_boxes,
            input.uncovered.boxes().len(),
            "canonical input boxes",
        )?;
        for (box_ordinal, lattice_box) in input.uncovered.boxes().iter().enumerate() {
            if lattice_box.arity() != input.sector.arity() {
                return Err(LeaderWalkPlanError::WrongPartitionBoxArity {
                    input_scope_ordinal: input_ordinal,
                    box_ordinal,
                    expected: input.sector.arity(),
                    actual: lattice_box.arity(),
                });
            }
            input_boxes = checked_add("input uncovered boxes", input_boxes, 1)?;
            check_limit("input uncovered boxes", input_boxes, limits.max_input_boxes)?;
            let endpoint_cells = checked_mul(
                "input uncovered-box coordinate cells",
                lattice_box.arity(),
                2,
            )?;
            input_box_coordinate_cells = checked_add(
                "input uncovered-box coordinate cells",
                input_box_coordinate_cells,
                endpoint_cells,
            )?;
            check_limit(
                "input uncovered-box coordinate cells",
                input_box_coordinate_cells,
                limits.max_input_box_coordinate_cells,
            )?;
            maximal_free_dimension = maximal_free_dimension.max(lattice_box.free_dimension());
            canonical_boxes.push(lattice_box);
        }
        canonical_boxes.sort_unstable();

        try_reserve_one(&mut canonical_scopes, "canonical input scopes")?;
        canonical_scopes.push(CanonicalScope {
            original_input_ordinal: input_ordinal,
            input,
            canonical_boxes,
        });
    }

    if canonical_scopes.is_empty() {
        return Err(LeaderWalkPlanError::EmptyScopeSchedule);
    }
    if maximal_free_dimension == 0 {
        return Err(LeaderWalkPlanError::NoUnboundedGeometry);
    }

    // Stable keys are unique identities, but they do not define scheduling
    // chronology. Sort by them once only to reject duplicates without another
    // allocation, then install the documented sector/endpoint chronology.
    canonical_scopes.sort_unstable_by(|left, right| {
        left.input
            .stable_scope_key
            .cmp(right.input.stable_scope_key)
            .then_with(|| left.input.sector.cmp(right.input.sector))
            .then_with(|| {
                left.original_input_ordinal
                    .cmp(&right.original_input_ordinal)
            })
    });
    for canonical_ordinal in 1..canonical_scopes.len() {
        if canonical_scopes[canonical_ordinal - 1]
            .input
            .stable_scope_key
            == canonical_scopes[canonical_ordinal].input.stable_scope_key
        {
            return Err(LeaderWalkPlanError::DuplicateStableScopeKey {
                first_canonical_ordinal: canonical_ordinal - 1,
                duplicate_canonical_ordinal: canonical_ordinal,
            });
        }
    }
    canonical_scopes.sort_unstable_by(|left, right| {
        left.input
            .sector
            .cmp(right.input.sector)
            .then_with(|| left.canonical_boxes.cmp(&right.canonical_boxes))
            .then_with(|| {
                left.input
                    .stable_scope_key
                    .cmp(right.input.stable_scope_key)
            })
            .then_with(|| {
                left.original_input_ordinal
                    .cmp(&right.original_input_ordinal)
            })
    });

    let mut frozen_scopes = Vec::new();
    try_reserve_exact(
        &mut frozen_scopes,
        canonical_scopes.len(),
        "selected canonical scopes",
    )?;
    let mut selected_boxes = 0usize;
    let mut selected_box_coordinate_cells = 0usize;
    let mut selected_free_axis_cells = 0usize;
    let mut task_coordinate_cells = 0usize;

    for (canonical_scope_ordinal, canonical) in canonical_scopes.iter().enumerate() {
        let matching_box_count = canonical
            .canonical_boxes
            .iter()
            .filter(|lattice_box| lattice_box.free_dimension() == maximal_free_dimension)
            .count();
        if matching_box_count == 0 {
            continue;
        }

        let key = LeaderWalkScopeKey::new(
            try_copy_string(canonical.input.stable_scope_key, "stable scope key")?,
            canonical.input.sector.clone(),
        );
        let mut boxes = Vec::new();
        try_reserve_exact(&mut boxes, matching_box_count, "selected maximal boxes")?;
        for (box_ordinal, lattice_box) in canonical
            .canonical_boxes
            .iter()
            .copied()
            .filter(|lattice_box| lattice_box.free_dimension() == maximal_free_dimension)
            .enumerate()
        {
            selected_boxes = checked_add("selected maximal boxes", selected_boxes, 1)?;
            check_limit(
                "selected maximal boxes",
                selected_boxes,
                limits.max_selected_boxes,
            )?;
            let box_cells = checked_mul(
                "selected maximal-box coordinate cells",
                lattice_box.arity(),
                2,
            )?;
            selected_box_coordinate_cells = checked_add(
                "selected maximal-box coordinate cells",
                selected_box_coordinate_cells,
                box_cells,
            )?;
            check_limit(
                "selected maximal-box coordinate cells",
                selected_box_coordinate_cells,
                limits.max_selected_box_coordinate_cells,
            )?;
            selected_free_axis_cells = checked_add(
                "selected maximal-box free-axis cells",
                selected_free_axis_cells,
                lattice_box.free_dimension(),
            )?;
            check_limit(
                "selected maximal-box free-axis cells",
                selected_free_axis_cells,
                limits.max_selected_free_axis_cells,
            )?;
            let per_task_coordinate_cells =
                checked_mul("leader-walk task coordinate cells", lattice_box.arity(), 2)?;
            let per_box_task_count = checked_add(
                "leader-walk tasks per selected box",
                lattice_box.free_dimension(),
                1,
            )?;
            let per_box_task_cells = checked_mul(
                "leader-walk task coordinate cells",
                per_task_coordinate_cells,
                per_box_task_count,
            )?;
            task_coordinate_cells = checked_add(
                "leader-walk task coordinate cells",
                task_coordinate_cells,
                per_box_task_cells,
            )?;
            check_limit(
                "leader-walk task coordinate cells",
                task_coordinate_cells,
                limits.max_task_coordinate_cells,
            )?;

            // Preflight the second wave before constructing either wave. This
            // prevents a valid lower-corner prefix from escaping when the
            // required depth-one census is not representable.
            for (position, (&lower, upper)) in lattice_box
                .lower()
                .iter()
                .zip(lattice_box.upper())
                .enumerate()
            {
                if upper.is_none() && lower.checked_add(1).is_none() {
                    return Err(LeaderWalkPlanError::LeaderCoordinateOverflow {
                        canonical_scope_ordinal,
                        box_ordinal,
                        position,
                    });
                }
            }

            let mut lower = Vec::new();
            try_reserve_exact(
                &mut lower,
                lattice_box.arity(),
                "selected box lower endpoints",
            )?;
            lower.extend_from_slice(lattice_box.lower());
            let mut upper = Vec::new();
            try_reserve_exact(
                &mut upper,
                lattice_box.arity(),
                "selected box upper endpoints",
            )?;
            upper.extend_from_slice(lattice_box.upper());
            let mut free_axes = Vec::new();
            try_reserve_exact(
                &mut free_axes,
                lattice_box.free_dimension(),
                "selected box free axes",
            )?;
            free_axes.extend(
                lattice_box
                    .upper()
                    .iter()
                    .enumerate()
                    .filter_map(|(position, upper)| upper.is_none().then_some(position)),
            );
            boxes.push(FrozenSelectedBox {
                key: LeaderWalkBoxKey::new(lower, upper),
                free_axes: free_axes.into_boxed_slice(),
            });
        }
        frozen_scopes.push(FrozenSelectedScope { key, boxes });
    }

    if selected_boxes == 0 {
        return Err(LeaderWalkPlanError::Invariant {
            detail: "a positive maximal free dimension selected no boxes",
        });
    }
    let lower_corner_tasks = selected_boxes;
    let depth_one_tasks = selected_free_axis_cells;
    let total_tasks = checked_add(
        "tasks across both waves",
        lower_corner_tasks,
        depth_one_tasks,
    )?;
    check_limit("tasks across both waves", total_tasks, limits.max_tasks)?;

    let epoch_identity = LeaderWalkGeometryEpochIdentity::fresh();
    let lower_corner = build_wave(
        epoch_identity.clone(),
        epoch_ordinal,
        &frozen_scopes,
        LeaderWalkDepth::LowerCorner,
        lower_corner_tasks,
        maximal_free_dimension,
        limits.max_arity,
    )?;
    let depth_one = build_wave(
        epoch_identity.clone(),
        epoch_ordinal,
        &frozen_scopes,
        LeaderWalkDepth::DepthOne,
        depth_one_tasks,
        maximal_free_dimension,
        limits.max_arity,
    )?;

    Ok(LeaderWalkPlan {
        epoch_identity,
        epoch_ordinal,
        input_scope_count: canonical_scopes.len(),
        selected_scope_count: frozen_scopes.len(),
        selected_box_count: selected_boxes,
        planned_task_count: total_tasks,
        maximal_free_dimension,
        lower_corner,
        depth_one,
    })
}

fn build_wave(
    epoch_identity: LeaderWalkGeometryEpochIdentity,
    epoch_ordinal: u64,
    scopes: &[FrozenSelectedScope],
    depth: LeaderWalkDepth,
    task_count: usize,
    maximal_free_dimension: usize,
    max_arity: usize,
) -> Result<LeaderWalkWave, LeaderWalkPlanError> {
    let mut tasks = Vec::new();
    try_reserve_exact(&mut tasks, task_count, "leader-walk wave tasks")?;
    let round_count = scopes.iter().map(|scope| scope.boxes.len()).max().ok_or(
        LeaderWalkPlanError::Invariant {
            detail: "a selected leader-walk wave has no scopes",
        },
    )?;

    for round in 0..round_count {
        let axis_ranks = match depth {
            LeaderWalkDepth::LowerCorner => 1,
            LeaderWalkDepth::DepthOne => maximal_free_dimension,
        };
        for axis_rank in 0..axis_ranks {
            for scope in scopes {
                let Some(selected_box) = scope.boxes.get(round) else {
                    continue;
                };
                let depth_one_axis = match depth {
                    LeaderWalkDepth::LowerCorner => None,
                    LeaderWalkDepth::DepthOne => {
                        Some(*selected_box.free_axes.get(axis_rank).ok_or(
                            LeaderWalkPlanError::Invariant {
                                detail: "a maximal box has too few free axes",
                            },
                        )?)
                    }
                };
                let box_key = &selected_box.key;
                let mut leader = Vec::new();
                try_reserve_exact(&mut leader, box_key.arity(), "leader coordinates")?;
                leader.extend_from_slice(box_key.lower());
                if let Some(position) = depth_one_axis {
                    leader[position] =
                        leader[position]
                            .checked_add(1)
                            .ok_or(LeaderWalkPlanError::Invariant {
                                detail: "a preflighted depth-one coordinate overflowed",
                            })?;
                }
                let target_shift = chart_point_to_target_shift(&scope.key, &leader, max_arity)?;
                let canonical_ordinal = tasks.len();
                tasks.push(LeaderWalkTask::new(
                    epoch_identity.clone(),
                    epoch_ordinal,
                    canonical_ordinal,
                    LeaderWalkTaskKey::new(
                        scope.key.clone(),
                        box_key.clone(),
                        depth,
                        depth_one_axis,
                    ),
                    leader,
                    target_shift,
                ));
            }
        }
    }
    if tasks.len() != task_count {
        return Err(LeaderWalkPlanError::Invariant {
            detail: "round-robin wave construction lost a selected box",
        });
    }
    Ok(LeaderWalkWave::new(depth, tasks))
}

fn chart_point_to_target_shift(
    scope: &LeaderWalkScopeKey,
    coordinates: &[u64],
    max_arity: usize,
) -> Result<IntegralShift, LeaderWalkPlanError> {
    let point = LatticePoint::try_new(coordinates.iter().copied())?;
    let chart = SectorChart::new(scope.sector().clone());
    let target = chart.to_integral(&point)?;
    let mut shift = Vec::new();
    try_reserve_exact(&mut shift, coordinates.len(), "target-shift coordinates")?;
    for (&target_power, corner_power) in target.powers().iter().zip(scope.sector().corner_indices())
    {
        shift.push(target_power.checked_sub(corner_power).ok_or(
            LeaderWalkPlanError::Invariant {
                detail: "a chart point could not be displaced from its sector corner",
            },
        )?);
    }
    IntegralShift::try_new_with_component_limit(shift, max_arity).map_err(Into::into)
}

fn try_copy_string(
    value: &str,
    resource: &'static str,
) -> Result<Arc<String>, LeaderWalkPlanError> {
    let mut retained = String::new();
    retained.try_reserve_exact(value.len()).map_err(|_| {
        LeaderWalkPlanError::AllocationFailure {
            resource,
            requested: value.len(),
        }
    })?;
    retained.push_str(value);
    Ok(Arc::new(retained))
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, LeaderWalkPlanError> {
    left.checked_add(right)
        .ok_or(LeaderWalkPlanError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, LeaderWalkPlanError> {
    left.checked_mul(right)
        .ok_or(LeaderWalkPlanError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), LeaderWalkPlanError> {
    if requested > limit {
        Err(LeaderWalkPlanError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn try_reserve_one<T>(
    retained: &mut Vec<T>,
    resource: &'static str,
) -> Result<(), LeaderWalkPlanError> {
    let requested = checked_add(resource, retained.len(), 1)?;
    retained
        .try_reserve_exact(1)
        .map_err(|_| LeaderWalkPlanError::AllocationFailure {
            resource,
            requested,
        })
}

fn try_reserve_exact<T>(
    retained: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), LeaderWalkPlanError> {
    let requested = checked_add(resource, retained.len(), additional)?;
    retained
        .try_reserve_exact(additional)
        .map_err(|_| LeaderWalkPlanError::AllocationFailure {
            resource,
            requested,
        })
}
