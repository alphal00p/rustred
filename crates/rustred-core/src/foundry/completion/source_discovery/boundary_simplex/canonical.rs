use crate::foundry::completion::LatticeBox;

use super::model::BoundarySimplexScopePartition;
use super::resource::{
    check_limit, checked_add, checked_mul, logical_sort_work, try_reserve_exact, try_reserve_one,
};
use super::{BoundarySimplexLimits, BoundarySimplexPlanError};

pub(super) struct CanonicalScope<'a> {
    pub(super) input: BoundarySimplexScopePartition<'a>,
    pub(super) boxes: Vec<&'a LatticeBox>,
    original_input_ordinal: usize,
}

/// Validate and count each borrowed partition before retaining any box view.
///
/// The two-pass scope treatment is deliberate: an oversized rejected input
/// cannot force an allocation proportional to its box count before the
/// aggregate input, coordinate-cell, and sort-work caps have accepted it.
pub(super) fn try_collect_canonical_scopes<'a>(
    scopes: impl IntoIterator<Item = BoundarySimplexScopePartition<'a>>,
    limits: BoundarySimplexLimits,
) -> Result<(Vec<CanonicalScope<'a>>, usize, usize), BoundarySimplexPlanError> {
    let mut canonical = Vec::new();
    let mut aggregate_key_bytes = 0usize;
    let mut input_boxes = 0usize;
    let mut input_coordinate_cells = 0usize;
    let mut sort_work = 0usize;
    let mut maximal_available = 0usize;
    let mut maximal_arity = 0usize;

    for (input_ordinal, input) in scopes.into_iter().enumerate() {
        let scope_count = checked_add("input scopes", canonical.len(), 1)?;
        check_limit("input scopes", scope_count, limits.max_scopes)?;
        if input.stable_scope_key.is_empty() {
            return Err(BoundarySimplexPlanError::EmptyStableScopeKey { input_ordinal });
        }
        let next_key_bytes = checked_add(
            "aggregate stable-scope-key bytes",
            aggregate_key_bytes,
            input.stable_scope_key.len(),
        )?;
        check_limit(
            "aggregate stable-scope-key bytes",
            next_key_bytes,
            limits.max_aggregate_scope_key_bytes,
        )?;
        check_limit("scope arity", input.sector.arity(), limits.max_arity)?;

        let scope_box_count = input.uncovered.boxes().len();
        let next_input_boxes = checked_add("input uncovered boxes", input_boxes, scope_box_count)?;
        check_limit(
            "input uncovered boxes",
            next_input_boxes,
            limits.max_input_boxes,
        )?;
        let scope_coordinate_cells = checked_mul(
            "input uncovered-box coordinate cells",
            checked_mul(
                "input uncovered-box coordinate cells",
                scope_box_count,
                input.sector.arity(),
            )?,
            2,
        )?;
        let next_coordinate_cells = checked_add(
            "input uncovered-box coordinate cells",
            input_coordinate_cells,
            scope_coordinate_cells,
        )?;
        check_limit(
            "input uncovered-box coordinate cells",
            next_coordinate_cells,
            limits.max_input_box_coordinate_cells,
        )?;
        let next_sort_work = checked_add(
            "canonical logical sort work",
            sort_work,
            logical_sort_work(scope_box_count)?,
        )?;
        check_limit(
            "canonical logical sort work",
            next_sort_work,
            limits.max_canonical_sort_work,
        )?;
        for (box_ordinal, lattice_box) in input.uncovered.boxes().iter().enumerate() {
            if lattice_box.arity() != input.sector.arity() {
                return Err(BoundarySimplexPlanError::WrongPartitionBoxArity {
                    input_scope_ordinal: input_ordinal,
                    box_ordinal,
                    expected: input.sector.arity(),
                    actual: lattice_box.arity(),
                });
            }
            maximal_available = maximal_available.max(lattice_box.free_dimension());
        }

        let mut boxes = Vec::new();
        try_reserve_exact(&mut boxes, scope_box_count, "canonical input boxes")?;
        boxes.extend(input.uncovered.boxes());
        boxes.sort_unstable();
        try_reserve_one(&mut canonical, "canonical input scopes")?;
        maximal_arity = maximal_arity.max(input.sector.arity());
        canonical.push(CanonicalScope {
            original_input_ordinal: input_ordinal,
            input,
            boxes,
        });
        aggregate_key_bytes = next_key_bytes;
        input_boxes = next_input_boxes;
        input_coordinate_cells = next_coordinate_cells;
        sort_work = next_sort_work;
    }
    if canonical.is_empty() {
        return Err(BoundarySimplexPlanError::EmptyScopeSchedule);
    }

    let one_scope_sort = logical_sort_work(canonical.len())?;
    sort_work = checked_add(
        "canonical logical sort work",
        sort_work,
        checked_mul("canonical logical sort work", one_scope_sort, 2)?,
    )?;
    check_limit(
        "canonical logical sort work",
        sort_work,
        limits.max_canonical_sort_work,
    )?;
    canonical.sort_unstable_by(|left, right| {
        left.input
            .stable_scope_key
            .cmp(right.input.stable_scope_key)
            .then_with(|| left.input.sector.cmp(right.input.sector))
            .then_with(|| {
                left.original_input_ordinal
                    .cmp(&right.original_input_ordinal)
            })
    });
    for ordinal in 1..canonical.len() {
        if canonical[ordinal - 1].input.stable_scope_key
            == canonical[ordinal].input.stable_scope_key
        {
            return Err(BoundarySimplexPlanError::DuplicateStableScopeKey {
                first_canonical_ordinal: ordinal - 1,
                duplicate_canonical_ordinal: ordinal,
            });
        }
    }
    canonical.sort_unstable_by(|left, right| {
        left.input
            .sector
            .cmp(right.input.sector)
            .then_with(|| left.boxes.cmp(&right.boxes))
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
    Ok((canonical, maximal_available, maximal_arity))
}
