//! Resource accounting for one explicitly scoped coverage query.

use super::*;
use crate::foundry::completion::UncoveredPartition;

pub(super) fn prepare_required(
    arity: usize,
    boxes: &[LatticeBox],
    limits: CompletionGeometryLimits,
) -> Result<BoxCover, PredicateCoverError> {
    if boxes.len() > limits.max_requested_boxes {
        return Err(PredicateCoverError::Budget("requested coverage boxes"));
    }
    if boxes
        .len()
        .checked_mul(arity)
        .and_then(|v| v.checked_mul(2))
        .is_none_or(|v| v > limits.max_requested_box_coordinate_cells)
    {
        return Err(PredicateCoverError::Budget(
            "requested coverage coordinate cells",
        ));
    }
    if boxes.iter().any(|domain| domain.arity() != arity) {
        return Err(PredicateCoverError::InvalidDomain("requested box arity"));
    }
    let mut copied = Vec::new();
    copied
        .try_reserve_exact(boxes.len())
        .map_err(|_| PredicateCoverError::Budget("requested coverage allocation"))?;
    for domain in boxes {
        copied.push(copy_box(domain)?);
    }
    BoxCover::try_new(arity, copied, limits).map_err(geometry)
}

/// All requested slices share these allowances as well as the parent
/// Boolean/native allowances. Pass remaining quotas BEFORE each subtraction;
/// checking only afterward would permit one extra full-budget allocation.
#[derive(Default)]
pub(super) struct GeometryWork {
    splits: usize,
    boxes: usize,
    coordinates: usize,
}

impl GeometryWork {
    /// Charge every monotone-degree probe before examining its coordinates.
    /// The same ledger also pays for box subtraction across Boolean branches.
    pub(super) fn charge_degree_probe(
        &mut self,
        arity: usize,
        limits: CompletionGeometryLimits,
    ) -> Result<(), PredicateCoverError> {
        let next = self
            .splits
            .checked_add(arity)
            .ok_or(PredicateCoverError::Budget("degree coverage work"))?;
        if next > limits.max_split_operations {
            return Err(PredicateCoverError::Budget("degree coverage work"));
        }
        self.splits = next;
        Ok(())
    }

    pub(super) fn remaining(
        &self,
        mut limits: CompletionGeometryLimits,
    ) -> Result<CompletionGeometryLimits, PredicateCoverError> {
        let subtract = |limit: usize, used| {
            limit
                .checked_sub(used)
                .ok_or(PredicateCoverError::Budget("scoped coverage geometry"))
        };
        limits.max_split_operations = subtract(limits.max_split_operations, self.splits)?;
        limits.max_uncovered_boxes = subtract(limits.max_uncovered_boxes, self.boxes)?;
        limits.max_uncovered_box_coordinate_cells =
            subtract(limits.max_uncovered_box_coordinate_cells, self.coordinates)?;
        Ok(limits)
    }

    pub(super) fn record(
        &mut self,
        complement: &UncoveredPartition,
        arity: usize,
    ) -> Result<(), PredicateCoverError> {
        let overflow = || PredicateCoverError::Budget("scoped coverage geometry overflow");
        self.splits = self
            .splits
            .checked_add(complement.split_operations())
            .ok_or_else(overflow)?;
        self.boxes = self
            .boxes
            .checked_add(complement.boxes().len())
            .ok_or_else(overflow)?;
        let coordinates = complement
            .boxes()
            .len()
            .checked_mul(arity)
            .and_then(|v| v.checked_mul(2))
            .ok_or_else(overflow)?;
        self.coordinates = self
            .coordinates
            .checked_add(coordinates)
            .ok_or_else(overflow)?;
        Ok(())
    }
}
