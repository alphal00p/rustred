use crate::foundry::completion::{BoxCover, LatticeBox, UncoveredPartition};

use super::{ExactOwnerCoverDeltaError, ExactOwnerCoverDeltaLimits};

const BOX_INPUTS: &str = "exact cover-delta comparison box inputs";
const COORDINATE_CELLS: &str = "exact cover-delta comparison coordinate cells";
const BOX_PAIR_PROBES: &str = "exact cover-delta comparison box-pair probes";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExactPartitionDelta {
    Equal,
    StrictSubset,
}

pub(super) fn try_clone_full_partition(
    arity: usize,
    limits: ExactOwnerCoverDeltaLimits,
) -> Result<UncoveredPartition, ExactOwnerCoverDeltaError> {
    preflight_boxes(1, arity, limits)?;
    let full = LatticeBox::try_new(
        std::iter::repeat_n(0, arity),
        std::iter::repeat_n(None, arity),
    )?;
    let mut boxes = Vec::new();
    boxes
        .try_reserve_exact(1)
        .map_err(|_| ExactOwnerCoverDeltaError::AllocationFailure {
            resource: BOX_INPUTS,
            requested: 1,
        })?;
    boxes.push(full);
    Ok(UncoveredPartition::new(boxes, 0))
}

pub(super) fn try_clone_partition(
    partition: &UncoveredPartition,
    arity: usize,
    limits: ExactOwnerCoverDeltaLimits,
) -> Result<UncoveredPartition, ExactOwnerCoverDeltaError> {
    preflight_boxes(partition.boxes().len(), arity, limits)?;
    validate_arity(partition.boxes(), arity)?;
    let boxes = try_clone_boxes(partition.boxes())?;
    Ok(UncoveredPartition::new(boxes, partition.split_operations()))
}

pub(super) fn try_compare_from_owner_free(
    arity: usize,
    updated: &UncoveredPartition,
    limits: ExactOwnerCoverDeltaLimits,
) -> Result<ExactPartitionDelta, ExactOwnerCoverDeltaError> {
    preflight_boxes(1, arity, limits)?;
    let full = LatticeBox::try_new(
        std::iter::repeat_n(0, arity),
        std::iter::repeat_n(None, arity),
    )?;
    try_compare_box_unions(std::slice::from_ref(&full), updated.boxes(), arity, limits)
}

pub(super) fn try_compare_partitions(
    baseline: &UncoveredPartition,
    updated: &UncoveredPartition,
    arity: usize,
    limits: ExactOwnerCoverDeltaLimits,
) -> Result<ExactPartitionDelta, ExactOwnerCoverDeltaError> {
    try_compare_box_unions(baseline.boxes(), updated.boxes(), arity, limits)
}

fn try_compare_box_unions(
    baseline: &[LatticeBox],
    updated: &[LatticeBox],
    arity: usize,
    limits: ExactOwnerCoverDeltaLimits,
) -> Result<ExactPartitionDelta, ExactOwnerCoverDeltaError> {
    let box_inputs = checked_add(BOX_INPUTS, baseline.len(), updated.len())?;
    check_limit(BOX_INPUTS, box_inputs, limits.max_comparison_box_inputs)?;
    let coordinate_cells = checked_mul(
        COORDINATE_CELLS,
        checked_mul(COORDINATE_CELLS, box_inputs, arity)?,
        2,
    )?;
    check_limit(
        COORDINATE_CELLS,
        coordinate_cells,
        limits.max_comparison_coordinate_cells,
    )?;
    let directed_pair_probes = checked_mul(BOX_PAIR_PROBES, baseline.len(), updated.len())?;
    let box_pair_probes = checked_mul(BOX_PAIR_PROBES, directed_pair_probes, 2)?;
    check_limit(
        BOX_PAIR_PROBES,
        box_pair_probes,
        limits.max_comparison_box_pair_probes,
    )?;

    preflight_boxes(baseline.len(), arity, limits)?;
    preflight_boxes(updated.len(), arity, limits)?;
    validate_arity(baseline, arity)?;
    validate_arity(updated, arity)?;
    let baseline_cover = try_clone_cover(arity, baseline, limits)?;
    let updated_cover = try_clone_cover(arity, updated, limits)?;

    if try_has_remainder(updated, &baseline_cover)? {
        return Err(ExactOwnerCoverDeltaError::NonMonotoneExactCover);
    }
    if try_has_remainder(baseline, &updated_cover)? {
        Ok(ExactPartitionDelta::StrictSubset)
    } else {
        Ok(ExactPartitionDelta::Equal)
    }
}

fn try_clone_cover(
    arity: usize,
    boxes: &[LatticeBox],
    limits: ExactOwnerCoverDeltaLimits,
) -> Result<BoxCover, ExactOwnerCoverDeltaError> {
    let retained = try_clone_boxes(boxes)?;
    Ok(BoxCover::try_new(
        arity,
        retained,
        limits.comparison_geometry,
    )?)
}

fn try_clone_boxes(boxes: &[LatticeBox]) -> Result<Vec<LatticeBox>, ExactOwnerCoverDeltaError> {
    let mut retained = Vec::new();
    retained.try_reserve_exact(boxes.len()).map_err(|_| {
        ExactOwnerCoverDeltaError::AllocationFailure {
            resource: BOX_INPUTS,
            requested: boxes.len(),
        }
    })?;
    for cell in boxes {
        retained.push(LatticeBox::try_new(
            cell.lower().iter().copied(),
            cell.upper().iter().copied(),
        )?);
    }
    Ok(retained)
}

fn try_has_remainder(
    universes: &[LatticeBox],
    cover: &BoxCover,
) -> Result<bool, ExactOwnerCoverDeltaError> {
    for universe in universes {
        let retained = LatticeBox::try_new(
            universe.lower().iter().copied(),
            universe.upper().iter().copied(),
        )?;
        if !cover.uncovered_within(retained)?.is_empty() {
            return Ok(true);
        }
    }
    Ok(false)
}

fn validate_arity(boxes: &[LatticeBox], arity: usize) -> Result<(), ExactOwnerCoverDeltaError> {
    if boxes.iter().all(|cell| cell.arity() == arity) {
        Ok(())
    } else {
        Err(ExactOwnerCoverDeltaError::NonMonotoneExactCover)
    }
}

fn preflight_boxes(
    box_count: usize,
    arity: usize,
    limits: ExactOwnerCoverDeltaLimits,
) -> Result<(), ExactOwnerCoverDeltaError> {
    check_limit(BOX_INPUTS, box_count, limits.max_comparison_box_inputs)?;
    check_limit(
        BOX_INPUTS,
        box_count,
        limits.comparison_geometry.max_requested_boxes,
    )?;
    let coordinate_cells = checked_mul(
        COORDINATE_CELLS,
        checked_mul(COORDINATE_CELLS, box_count, arity)?,
        2,
    )?;
    check_limit(
        COORDINATE_CELLS,
        coordinate_cells,
        limits.max_comparison_coordinate_cells,
    )?;
    check_limit(
        COORDINATE_CELLS,
        coordinate_cells,
        limits
            .comparison_geometry
            .max_requested_box_coordinate_cells,
    )
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactOwnerCoverDeltaError> {
    left.checked_add(right)
        .ok_or(ExactOwnerCoverDeltaError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactOwnerCoverDeltaError> {
    left.checked_mul(right)
        .ok_or(ExactOwnerCoverDeltaError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ExactOwnerCoverDeltaError> {
    if requested > limit {
        Err(ExactOwnerCoverDeltaError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
