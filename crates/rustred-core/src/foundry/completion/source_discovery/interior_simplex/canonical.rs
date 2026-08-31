use crate::foundry::completion::LatticeBox;

use super::InteriorSimplexPlanError;
use super::limits::InteriorSimplexLimits;
use super::model::InteriorSimplexScopePartition;
use super::resource::{check_limit, checked_add, checked_mul, try_reserve_exact, try_reserve_one};

pub(super) struct CanonicalScope<'a> {
    pub(super) original_input_ordinal: usize,
    pub(super) input: InteriorSimplexScopePartition<'a>,
    pub(super) canonical_boxes: Vec<&'a LatticeBox>,
}

pub(super) fn try_collect_canonical_scopes<'a>(
    scopes: impl IntoIterator<Item = InteriorSimplexScopePartition<'a>>,
    limits: InteriorSimplexLimits,
) -> Result<(Vec<CanonicalScope<'a>>, usize), InteriorSimplexPlanError> {
    let mut canonical_scopes = Vec::new();
    let mut aggregate_scope_key_bytes = 0usize;
    let mut input_boxes = 0usize;
    let mut input_box_coordinate_cells = 0usize;
    let mut maximal_free_dimension = 0usize;

    for (input_ordinal, input) in scopes.into_iter().enumerate() {
        let requested_scopes = checked_add("input scopes", canonical_scopes.len(), 1)?;
        check_limit("input scopes", requested_scopes, limits.max_scopes)?;
        if input.stable_scope_key.is_empty() {
            return Err(InteriorSimplexPlanError::EmptyStableScopeKey { input_ordinal });
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

        for (box_ordinal, lattice_box) in input.uncovered.boxes().iter().enumerate() {
            if lattice_box.arity() != input.sector.arity() {
                return Err(InteriorSimplexPlanError::WrongPartitionBoxArity {
                    input_scope_ordinal: input_ordinal,
                    box_ordinal,
                    expected: input.sector.arity(),
                    actual: lattice_box.arity(),
                });
            }
            maximal_free_dimension = maximal_free_dimension.max(lattice_box.free_dimension());
        }
        let scope_box_count = input.uncovered.boxes().len();
        input_boxes = checked_add("input uncovered boxes", input_boxes, scope_box_count)?;
        check_limit("input uncovered boxes", input_boxes, limits.max_input_boxes)?;
        let cells_per_box = checked_mul(
            "input uncovered-box coordinate cells",
            input.sector.arity(),
            2,
        )?;
        let scope_box_cells = checked_mul(
            "input uncovered-box coordinate cells",
            scope_box_count,
            cells_per_box,
        )?;
        input_box_coordinate_cells = checked_add(
            "input uncovered-box coordinate cells",
            input_box_coordinate_cells,
            scope_box_cells,
        )?;
        check_limit(
            "input uncovered-box coordinate cells",
            input_box_coordinate_cells,
            limits.max_input_box_coordinate_cells,
        )?;
        let mut canonical_boxes = Vec::new();
        try_reserve_exact(
            &mut canonical_boxes,
            scope_box_count,
            "canonical input boxes",
        )?;
        canonical_boxes.extend(input.uncovered.boxes());
        canonical_boxes.sort_unstable();
        try_reserve_one(&mut canonical_scopes, "canonical input scopes")?;
        canonical_scopes.push(CanonicalScope {
            original_input_ordinal: input_ordinal,
            input,
            canonical_boxes,
        });
    }

    if canonical_scopes.is_empty() {
        return Err(InteriorSimplexPlanError::EmptyScopeSchedule);
    }
    if maximal_free_dimension == 0 {
        return Err(InteriorSimplexPlanError::NoUnboundedGeometry);
    }
    reject_duplicate_keys_and_install_chronology(&mut canonical_scopes)?;
    Ok((canonical_scopes, maximal_free_dimension))
}

fn reject_duplicate_keys_and_install_chronology(
    scopes: &mut [CanonicalScope<'_>],
) -> Result<(), InteriorSimplexPlanError> {
    scopes.sort_unstable_by(|left, right| {
        left.input
            .stable_scope_key
            .cmp(right.input.stable_scope_key)
            .then_with(|| left.input.sector.cmp(right.input.sector))
            .then_with(|| {
                left.original_input_ordinal
                    .cmp(&right.original_input_ordinal)
            })
    });
    for ordinal in 1..scopes.len() {
        if scopes[ordinal - 1].input.stable_scope_key == scopes[ordinal].input.stable_scope_key {
            return Err(InteriorSimplexPlanError::DuplicateStableScopeKey {
                first_canonical_ordinal: ordinal - 1,
                duplicate_canonical_ordinal: ordinal,
            });
        }
    }
    scopes.sort_unstable_by(|left, right| {
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
    Ok(())
}
