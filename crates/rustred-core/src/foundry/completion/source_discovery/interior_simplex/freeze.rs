use super::InteriorSimplexPlanError;
use super::canonical::CanonicalScope;
use super::limits::InteriorSimplexLimits;
use super::model::{InteriorSimplexBoxKey, InteriorSimplexScopeKey};
use super::resource::{check_limit, checked_add, checked_mul, try_copy_string, try_reserve_exact};
use super::target::try_chart_point_to_target_shift;

pub(super) struct FrozenSelectedBox {
    pub(super) key: InteriorSimplexBoxKey,
    pub(super) free_axes: Vec<usize>,
}

pub(super) struct FrozenSelectedScope {
    pub(super) key: InteriorSimplexScopeKey,
    pub(super) boxes: Vec<FrozenSelectedBox>,
}

pub(super) struct FrozenGeometry {
    pub(super) scopes: Vec<FrozenSelectedScope>,
    pub(super) selected_box_count: usize,
    pub(super) total_tasks: usize,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn try_freeze_maximal_geometry(
    canonical_scopes: &[CanonicalScope<'_>],
    maximal_free_dimension: usize,
    interior_margin: u64,
    degree_ceiling: u64,
    simplex_sample_count: usize,
    limits: InteriorSimplexLimits,
) -> Result<FrozenGeometry, InteriorSimplexPlanError> {
    let mut frozen_scopes = Vec::new();
    try_reserve_exact(
        &mut frozen_scopes,
        canonical_scopes.len(),
        "selected canonical scopes",
    )?;
    let mut selected_boxes = 0usize;
    let mut selected_box_coordinate_cells = 0usize;
    let mut selected_free_axis_cells = 0usize;
    let mut total_tasks = 0usize;
    let mut task_coordinate_cells = 0usize;

    for (canonical_scope_ordinal, canonical) in canonical_scopes.iter().enumerate() {
        let selected_in_scope = canonical
            .canonical_boxes
            .iter()
            .filter(|lattice_box| lattice_box.free_dimension() == maximal_free_dimension)
            .count();
        if selected_in_scope == 0 {
            continue;
        }
        let next_selected_boxes =
            checked_add("selected maximal boxes", selected_boxes, selected_in_scope)?;
        check_limit(
            "selected maximal boxes",
            next_selected_boxes,
            limits.max_selected_boxes,
        )?;
        let cells_per_box = checked_mul(
            "selected maximal-box coordinate cells",
            canonical.input.sector.arity(),
            2,
        )?;
        let scope_box_cells = checked_mul(
            "selected maximal-box coordinate cells",
            selected_in_scope,
            cells_per_box,
        )?;
        let next_box_cells = checked_add(
            "selected maximal-box coordinate cells",
            selected_box_coordinate_cells,
            scope_box_cells,
        )?;
        check_limit(
            "selected maximal-box coordinate cells",
            next_box_cells,
            limits.max_selected_box_coordinate_cells,
        )?;
        let scope_free_axes = checked_mul(
            "selected maximal-box free-axis cells",
            selected_in_scope,
            maximal_free_dimension,
        )?;
        let next_free_axis_cells = checked_add(
            "selected maximal-box free-axis cells",
            selected_free_axis_cells,
            scope_free_axes,
        )?;
        check_limit(
            "selected maximal-box free-axis cells",
            next_free_axis_cells,
            limits.max_selected_free_axis_cells,
        )?;
        let scope_tasks = checked_mul(
            "interior-simplex tasks",
            selected_in_scope,
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
        total_tasks = next_total_tasks;
        task_coordinate_cells = next_task_coordinate_cells;
        let scope_key = InteriorSimplexScopeKey::new(
            try_copy_string(canonical.input.stable_scope_key, "stable scope key")?,
            canonical.input.sector.clone(),
        );
        let mut boxes = Vec::new();
        try_reserve_exact(&mut boxes, selected_in_scope, "selected maximal boxes")?;
        for (box_ordinal, lattice_box) in canonical
            .canonical_boxes
            .iter()
            .copied()
            .filter(|lattice_box| lattice_box.free_dimension() == maximal_free_dimension)
            .enumerate()
        {
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
                maximal_free_dimension,
                "selected box free axes",
            )?;
            free_axes.extend(
                lattice_box
                    .upper()
                    .iter()
                    .enumerate()
                    .filter_map(|(position, upper)| upper.is_none().then_some(position)),
            );
            preflight_box_coordinates(
                canonical_scope_ordinal,
                box_ordinal,
                &scope_key,
                &lower,
                &free_axes,
                interior_margin,
                degree_ceiling,
                limits.max_arity,
            )?;
            boxes.push(FrozenSelectedBox {
                key: InteriorSimplexBoxKey::new(lower, upper),
                free_axes,
            });
        }
        frozen_scopes.push(FrozenSelectedScope {
            key: scope_key,
            boxes,
        });
    }

    if selected_boxes == 0 {
        return Err(InteriorSimplexPlanError::Invariant {
            detail: "a positive maximal free dimension selected no boxes",
        });
    }
    Ok(FrozenGeometry {
        scopes: frozen_scopes,
        selected_box_count: selected_boxes,
        total_tasks,
    })
}

#[allow(clippy::too_many_arguments)]
fn preflight_box_coordinates(
    canonical_scope_ordinal: usize,
    box_ordinal: usize,
    scope: &InteriorSimplexScopeKey,
    lower: &[u64],
    free_axes: &[usize],
    interior_margin: u64,
    degree_ceiling: u64,
    max_arity: usize,
) -> Result<(), InteriorSimplexPlanError> {
    let mut worst_target = Vec::new();
    try_reserve_exact(&mut worst_target, lower.len(), "worst-case lattice target")?;
    worst_target.extend_from_slice(lower);
    for &position in free_axes {
        worst_target[position] = worst_target[position]
            .checked_add(interior_margin)
            .and_then(|coordinate| coordinate.checked_add(degree_ceiling))
            .ok_or(InteriorSimplexPlanError::CoordinateOverflow {
                canonical_scope_ordinal,
                box_ordinal,
                position,
            })?;
    }
    // This componentwise worst point conservatively preflights carrier
    // representability for every member of the total-degree simplex.
    try_chart_point_to_target_shift(scope, &worst_target, max_arity)?;
    Ok(())
}
