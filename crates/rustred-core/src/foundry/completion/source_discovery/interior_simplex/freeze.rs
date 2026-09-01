use super::super::simplex_support::try_finite_assignment_count;
use super::InteriorSimplexPlanError;
use super::canonical::CanonicalScope;
use super::limits::InteriorSimplexLimits;
use super::model::{InteriorSimplexBoxKey, InteriorSimplexScopeKey};
use super::resource::{check_limit, checked_add, checked_mul, try_copy_string, try_reserve_exact};
use super::target::try_chart_point_to_target_shift;

const FINITE_ASSIGNMENTS: &str = "finite coordinate assignments";

pub(super) struct FrozenSelectedBox {
    pub(super) key: InteriorSimplexBoxKey,
    pub(super) free_axes: Vec<usize>,
    pub(super) finite_assignment_count: usize,
}

pub(super) struct FrozenSelectedScope {
    pub(super) key: InteriorSimplexScopeKey,
    pub(super) boxes: Vec<FrozenSelectedBox>,
}

pub(super) struct FrozenGeometry {
    pub(super) scopes: Vec<FrozenSelectedScope>,
    pub(super) selected_box_count: usize,
    pub(super) finite_assignment_count: usize,
    pub(super) scheduler_workspace_entries: usize,
    pub(super) expected_scheduler_visits: usize,
    pub(super) total_tasks: usize,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn try_freeze_selected_geometry(
    canonical_scopes: &[CanonicalScope<'_>],
    selected_free_dimension: usize,
    interior_margin: u64,
    degree_ceiling: u64,
    simplex_sample_count: usize,
    limits: InteriorSimplexLimits,
) -> Result<FrozenGeometry, InteriorSimplexPlanError> {
    // First count and reject the complete aggregate design without allocating
    // any frozen scope key, endpoint buffer, worst target, or scheduler
    // workspace.  This keeps all variable-size construction behind the exact
    // aggregate preflight.
    let mut selected_scope_count = 0usize;
    let mut selected_boxes = 0usize;
    let mut maximal_box_round_count = 0usize;
    let mut selected_box_coordinate_cells = 0usize;
    let mut selected_free_axis_cells = 0usize;
    let mut finite_assignments = 0usize;
    let mut total_tasks = 0usize;
    let mut task_coordinate_cells = 0usize;

    for canonical in canonical_scopes {
        let selected_in_scope = canonical
            .canonical_boxes
            .iter()
            .filter(|lattice_box| lattice_box.free_dimension() == selected_free_dimension)
            .count();
        if selected_in_scope == 0 {
            continue;
        }
        selected_scope_count = checked_add("selected canonical scopes", selected_scope_count, 1)?;
        maximal_box_round_count = maximal_box_round_count.max(selected_in_scope);
        let next_selected_boxes = checked_add(
            "selected free-dimension boxes",
            selected_boxes,
            selected_in_scope,
        )?;
        check_limit(
            "selected free-dimension boxes",
            next_selected_boxes,
            limits.max_selected_boxes,
        )?;
        let cells_per_box = checked_mul(
            "selected free-dimension box coordinate cells",
            canonical.input.sector.arity(),
            2,
        )?;
        let scope_box_cells = checked_mul(
            "selected free-dimension box coordinate cells",
            selected_in_scope,
            cells_per_box,
        )?;
        let next_box_cells = checked_add(
            "selected free-dimension box coordinate cells",
            selected_box_coordinate_cells,
            scope_box_cells,
        )?;
        check_limit(
            "selected free-dimension box coordinate cells",
            next_box_cells,
            limits.max_selected_box_coordinate_cells,
        )?;
        let scope_free_axes = checked_mul(
            "selected free-dimension box free-axis cells",
            selected_in_scope,
            selected_free_dimension,
        )?;
        let next_free_axis_cells = checked_add(
            "selected free-dimension box free-axis cells",
            selected_free_axis_cells,
            scope_free_axes,
        )?;
        check_limit(
            "selected free-dimension box free-axis cells",
            next_free_axis_cells,
            limits.max_selected_free_axis_cells,
        )?;
        let mut scope_finite_assignments = 0usize;
        for lattice_box in canonical
            .canonical_boxes
            .iter()
            .copied()
            .filter(|lattice_box| lattice_box.free_dimension() == selected_free_dimension)
        {
            let assignment_count = try_finite_assignment_count(
                lattice_box.lower(),
                lattice_box.upper(),
                FINITE_ASSIGNMENTS,
            )?;
            check_limit(
                "finite assignments per selected box",
                assignment_count,
                limits.max_finite_assignments_per_box,
            )?;
            scope_finite_assignments = checked_add(
                "finite coordinate assignments",
                scope_finite_assignments,
                assignment_count,
            )?;
        }
        let next_finite_assignments = checked_add(
            "finite coordinate assignments",
            finite_assignments,
            scope_finite_assignments,
        )?;
        check_limit(
            "finite coordinate assignments",
            next_finite_assignments,
            limits.max_finite_assignments,
        )?;
        let scope_tasks = checked_mul(
            "interior-simplex tasks",
            scope_finite_assignments,
            simplex_sample_count,
        )?;
        let next_total_tasks = checked_add("interior-simplex tasks", total_tasks, scope_tasks)?;
        check_limit("interior-simplex tasks", next_total_tasks, limits.max_tasks)?;
        let scope_task_cells = checked_mul(
            "interior-simplex task coordinate cells",
            scope_tasks,
            cells_per_box,
        )?;
        let next_task_coordinate_cells = checked_add(
            "interior-simplex task coordinate cells",
            task_coordinate_cells,
            scope_task_cells,
        )?;
        check_limit(
            "interior-simplex task coordinate cells",
            next_task_coordinate_cells,
            limits.max_task_coordinate_cells,
        )?;

        selected_boxes = next_selected_boxes;
        selected_box_coordinate_cells = next_box_cells;
        selected_free_axis_cells = next_free_axis_cells;
        finite_assignments = next_finite_assignments;
        total_tasks = next_total_tasks;
        task_coordinate_cells = next_task_coordinate_cells;
    }

    if selected_boxes == 0 {
        return Err(InteriorSimplexPlanError::Invariant {
            detail: "a validated positive free dimension selected no boxes",
        });
    }

    // At construction time the flattened box order coexists with two
    // alternating active frontiers.  During flattening it instead coexists
    // with two round-index vectors, whose length cannot exceed the selected
    // box count.  Three entries per selected box are therefore an exact
    // conservative peak for this scheduler-only workspace.
    let scheduler_workspace_entries =
        checked_mul("scheduler workspace entries", selected_boxes, 3)?;
    check_limit(
        "scheduler workspace entries",
        scheduler_workspace_entries,
        limits.max_scheduler_workspace_entries,
    )?;

    // The flattened chronology performs two complete selected-box passes and
    // one pass over its nonempty round slots.  Every simplex offset then
    // seeds one selected-box frontier and visits only live assignments.  The
    // latter count is exactly the already preflighted task count.
    let flatten_visits = checked_add(
        "scheduler visits",
        checked_mul("scheduler visits", selected_boxes, 2)?,
        maximal_box_round_count,
    )?;
    let offset_seed_visits = checked_mul("scheduler visits", selected_boxes, simplex_sample_count)?;
    let expected_scheduler_visits = checked_add(
        "scheduler visits",
        checked_add("scheduler visits", flatten_visits, offset_seed_visits)?,
        total_tasks,
    )?;
    check_limit(
        "scheduler visits",
        expected_scheduler_visits,
        limits.max_scheduler_visits,
    )?;

    let mut frozen_scopes = Vec::new();
    try_reserve_exact(
        &mut frozen_scopes,
        selected_scope_count,
        "selected canonical scopes",
    )?;
    for (canonical_scope_ordinal, canonical) in canonical_scopes.iter().enumerate() {
        let selected_in_scope = canonical
            .canonical_boxes
            .iter()
            .filter(|lattice_box| lattice_box.free_dimension() == selected_free_dimension)
            .count();
        if selected_in_scope == 0 {
            continue;
        }
        let scope_key = InteriorSimplexScopeKey::new(
            try_copy_string(canonical.input.stable_scope_key, "stable scope key")?,
            canonical.input.sector.clone(),
        );
        let mut boxes = Vec::new();
        try_reserve_exact(
            &mut boxes,
            selected_in_scope,
            "selected free-dimension boxes",
        )?;
        for (box_ordinal, lattice_box) in canonical
            .canonical_boxes
            .iter()
            .copied()
            .filter(|lattice_box| lattice_box.free_dimension() == selected_free_dimension)
            .enumerate()
        {
            preflight_box_coordinates(
                canonical_scope_ordinal,
                box_ordinal,
                &scope_key,
                lattice_box.lower(),
                lattice_box.upper(),
                interior_margin,
                degree_ceiling,
                limits.max_arity,
            )?;
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
                selected_free_dimension,
                "selected box free axes",
            )?;
            free_axes.extend(
                lattice_box
                    .upper()
                    .iter()
                    .enumerate()
                    .filter_map(|(position, upper)| upper.is_none().then_some(position)),
            );
            let finite_assignment_count =
                try_finite_assignment_count(&lower, &upper, FINITE_ASSIGNMENTS)?;
            boxes.push(FrozenSelectedBox {
                key: InteriorSimplexBoxKey::new(lower, upper),
                free_axes,
                finite_assignment_count,
            });
        }
        frozen_scopes.push(FrozenSelectedScope {
            key: scope_key,
            boxes,
        });
    }

    Ok(FrozenGeometry {
        scopes: frozen_scopes,
        selected_box_count: selected_boxes,
        finite_assignment_count: finite_assignments,
        scheduler_workspace_entries,
        expected_scheduler_visits,
        total_tasks,
    })
}

#[allow(clippy::too_many_arguments)]
fn preflight_box_coordinates(
    canonical_scope_ordinal: usize,
    box_ordinal: usize,
    scope: &InteriorSimplexScopeKey,
    lower: &[u64],
    upper: &[Option<u64>],
    interior_margin: u64,
    degree_ceiling: u64,
    max_arity: usize,
) -> Result<(), InteriorSimplexPlanError> {
    let mut worst_target = Vec::new();
    try_reserve_exact(&mut worst_target, lower.len(), "worst-case lattice target")?;
    worst_target.extend_from_slice(lower);
    for (position, &upper) in upper.iter().enumerate() {
        match upper {
            Some(upper) => worst_target[position] = upper,
            None => {
                worst_target[position] = worst_target[position]
                    .checked_add(interior_margin)
                    .and_then(|coordinate| coordinate.checked_add(degree_ceiling))
                    .ok_or(InteriorSimplexPlanError::CoordinateOverflow {
                        canonical_scope_ordinal,
                        box_ordinal,
                        position,
                    })?;
            }
        }
    }
    // This componentwise worst point conservatively preflights carrier
    // representability for every member of the total-degree simplex.
    try_chart_point_to_target_shift(scope, &worst_target, max_arity)?;
    Ok(())
}
