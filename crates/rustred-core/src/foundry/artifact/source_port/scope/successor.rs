//! Exact rectangular successor containment, not artifact authority.
//!
//! The source-port executable pivot is zero: runtime computes physical
//! `child = target + rhs_shift`. Local coordinates are `n-1` on active axes
//! and `-n` on inactive axes; adding the shift directly to every local axis
//! would therefore be wrong. Existing sign partitioning owns the zero crossing.
//!
//! A successful check covers every point of the source rectangle. Affine
//! target/exclusion tightening and coefficient-zero pruning are deliberately
//! absent: a false result can be inconclusive for the actual predicate domain.
//! This mathematical-u64 geometry also does not prove runtime i64 arithmetic
//! safety, source replay, denominator guards, descent, or destination validity.

use crate::algebra::indexed::ceil_log2;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::completion::{
    BoxCover, CompletionGeometryError, CompletionGeometryLimits, LatticeBox,
};

use super::super::geometry::sign_partition_with_limits;

/// Caller-supplied rectangular proof-domain pieces in one physical sector.
/// Repeated sector entries contribute to the same union; none is zero evidence.
pub(in crate::foundry::artifact) struct DestinationScope<'a> {
    pub(in crate::foundry::artifact) sector: &'a [bool],
    pub(in crate::foundry::artifact) boxes: &'a [LatticeBox],
}

/// Immutable destination covers, admitted and copied once, together with one
/// cumulative allowance across all source pieces and RHS terms. This caches
/// only input geometry, not proof answers. An inconclusive image or error does
/// not reset the allowance. Changing the destination union requires preparation
/// of a new owner, not mutation of an already admitted cover.
pub(in crate::foundry::artifact) struct PreparedSuccessorScope {
    arity: usize,
    destinations: Vec<(Vec<bool>, BoxCover)>,
    budget: SuccessorScopeBudget,
}

/// Internal proof object for a rank-scoped successor-closed domain.
///
/// Construction first proves that every admitted entry piece is contained in
/// the immutable destination union.  Callers then submit every executable
/// RHS shift; each translated image is checked against that same union.  This
/// is deliberately not wired into artifact persistence or runtime admission:
/// it is a small exact building block for the future scoped-certification
/// contract, not a bounded closure claim by itself.
pub(in crate::foundry::artifact) struct SuccessorClosedScope {
    prepared: PreparedSuccessorScope,
}

impl SuccessorClosedScope {
    pub(in crate::foundry::artifact) fn try_new(
        arity: usize,
        entries: &[DestinationScope<'_>],
        destinations: &[DestinationScope<'_>],
        limits: CompletionGeometryLimits,
    ) -> Result<Self, ArtifactError> {
        let mut prepared = PreparedSuccessorScope::try_new(arity, destinations, limits)?;
        let zero = vec![0_i64; arity];
        for entry in entries {
            if entry.sector.len() != arity {
                return Err(invalid("successor entry sector arity mismatch"));
            }
            for source in entry.boxes {
                if !prepared.contains(source, entry.sector, &zero)? {
                    return Err(invalid(
                        "successor entry domain is not contained in destination union",
                    ));
                }
            }
        }
        Ok(Self { prepared })
    }

    pub(in crate::foundry::artifact) fn check_rule_images(
        &mut self,
        source: &LatticeBox,
        sector: &[bool],
        rhs_shifts: &[&[i64]],
    ) -> Result<(), ArtifactError> {
        for shift in rhs_shifts {
            if !self.prepared.contains(source, sector, shift)? {
                return Err(invalid("RHS image escapes successor-closed proof domain"));
            }
        }
        Ok(())
    }
}

struct SuccessorScopeBudget {
    limits: CompletionGeometryLimits,
    requested_boxes: usize,
    requested_coordinates: usize,
    work: usize,
    uncovered_boxes: usize,
    uncovered_coordinates: usize,
}

impl PreparedSuccessorScope {
    pub(in crate::foundry::artifact) fn try_new(
        arity: usize,
        destinations: &[DestinationScope<'_>],
        limits: CompletionGeometryLimits,
    ) -> Result<Self, ArtifactError> {
        if arity == 0 {
            return Err(invalid("successor coordinate space is empty"));
        }
        check("successor arity", arity, limits.max_arity)?;
        check(
            "successor destination sectors",
            destinations.len(),
            limits.max_requested_boxes,
        )?;
        // Admit every original entry before deduplication, grouping, allocation,
        // or any later successful/missing-sector query.
        let mut input_boxes = 0;
        for destination in destinations {
            input_boxes = add(input_boxes, destination.boxes.len())?;
            check(
                "successor input boxes",
                input_boxes,
                limits.max_requested_boxes,
            )?;
            if destination.sector.len() != arity
                || destination.boxes.iter().any(|cell| cell.arity() != arity)
            {
                return Err(invalid("successor destination arity mismatch"));
            }
        }
        let input_coordinates = mul(mul(input_boxes, arity)?, 2)?;
        check(
            "successor input coordinates",
            input_coordinates,
            limits.max_requested_box_coordinate_cells,
        )?;
        let mut budget = SuccessorScopeBudget {
            limits,
            requested_boxes: 0,
            requested_coordinates: 0,
            work: 0,
            uncovered_boxes: 0,
            uncovered_coordinates: 0,
        };
        // Two box-container slots per input (temporary grouping and BoxCover),
        // one endpoint copy, and at most one owned sector mask per input entry.
        // Charge sorting's structural n*log(n) proxy once, not on every RHS.
        let entries = destinations.len();
        let preparation_work = mul(
            arity,
            add(
                mul(entries, add(1, ceil_log2(entries))?)?,
                mul(input_boxes, add(1, ceil_log2(input_boxes))?)?,
            )?,
        )?;
        budget.charge(
            mul(input_boxes, 2)?,
            add(input_coordinates, mul(entries, arity)?)?,
            preparation_work,
        )?;
        let mut ordered = Vec::new();
        reserve(&mut ordered, entries)?;
        ordered.extend(destinations.iter());
        ordered.sort_unstable_by(|left, right| left.sector.cmp(right.sector));
        let mut prepared = Vec::new();
        reserve(&mut prepared, entries)?;
        let mut start = 0;
        while start < entries {
            let mut end = start + 1;
            while end < entries && ordered[end].sector == ordered[start].sector {
                end += 1;
            }
            let count = ordered[start..end]
                .iter()
                .try_fold(0usize, |count, domain| add(count, domain.boxes.len()))?;
            let mut boxes = Vec::new();
            reserve(&mut boxes, count)?;
            for domain in &ordered[start..end] {
                for cell in domain.boxes {
                    boxes.push(copy_box(cell)?);
                }
            }
            let mut sector = Vec::new();
            reserve(&mut sector, arity)?;
            sector.extend_from_slice(ordered[start].sector);
            prepared.push((
                sector,
                BoxCover::try_new(arity, boxes, limits).map_err(geometry_error)?,
            ));
            start = end;
        }
        Ok(Self {
            arity,
            destinations: prepared,
            budget,
        })
    }

    pub(in crate::foundry::artifact) fn contains(
        &mut self,
        source: &LatticeBox,
        sector: &[bool],
        rhs_shift: &[i64],
    ) -> Result<bool, ArtifactError> {
        let arity = self.arity;
        if sector.len() != arity || source.arity() != arity || rhs_shift.len() != arity {
            return Err(invalid("successor source/sector/shift arity mismatch"));
        }
        let limits = self.budget.limits;

        let pieces = partition_count(source, sector, rhs_shift)?;
        check("successor sign pieces", pieces, limits.max_uncovered_boxes)?;
        let piece_coordinates = mul(mul(pieces, arity)?, 2)?;
        check(
            "successor sign coordinates",
            piece_coordinates,
            limits.max_uncovered_box_coordinate_cells,
        )?;
        // At most 2P-1 partition boxes, P-1 pairs of temporary sign endpoints,
        // and 2P image endpoint pairs (temporary vectors plus final boxes).
        let allocated_boxes = mul(pieces, 5)?.checked_sub(2).ok_or_else(overflow)?;
        let allocated_coordinates = add(
            mul(mul(allocated_boxes, arity)?, 2)?,
            mul(pieces, arity)?, // child-sector masks
        )?;
        let image_work = mul(
            mul(pieces, arity)?,
            add(7, ceil_log2(self.destinations.len()))?,
        )?;
        self.budget.charge(
            allocated_boxes,
            allocated_coordinates,
            add(add(arity, image_work)?, pieces - 1)?,
        )?;

        // The exact prospective product bound is charged before the existing
        // partitioner allocates. Give it only this call's reserved envelope.
        let partition_limits = CompletionGeometryLimits {
            max_uncovered_boxes: pieces,
            max_uncovered_box_coordinate_cells: piece_coordinates,
            max_split_operations: pieces - 1,
            ..limits
        };
        let partitions = sign_partition_with_limits(source, sector, rhs_shift, partition_limits)
            .map_err(|_| invalid("preflighted successor sign partition failed"))?;
        if partitions.len() != pieces {
            return Err(invalid("successor sign partition disagrees with preflight"));
        }
        for piece in partitions {
            let (child_sector, image) = translated_image(&piece, sector, rhs_shift)?;
            let Ok(index) = self
                .destinations
                .binary_search_by(|(sector, _)| sector.cmp(&child_sector))
            else {
                return Ok(false);
            };
            let cover = &self.destinations[index].1;
            let remaining = self.budget.remaining()?;
            let complement = match cover.uncovered_within_budget(
                image,
                remaining.max_uncovered_boxes,
                remaining.max_uncovered_box_coordinate_cells,
                remaining.max_split_operations,
            ) {
                Ok(complement) => complement,
                Err(issue) => {
                    // The failed geometry call does not expose its partial
                    // work counter. Consume the remaining allowance rather
                    // than granting it again on a caller's subsequent term.
                    self.budget.work = limits.max_split_operations;
                    return Err(geometry_error(issue));
                }
            };
            self.budget.work = add(self.budget.work, complement.split_operations())?;
            self.budget.uncovered_boxes =
                add(self.budget.uncovered_boxes, complement.boxes().len())?;
            self.budget.uncovered_coordinates = add(
                self.budget.uncovered_coordinates,
                mul(mul(complement.boxes().len(), arity)?, 2)?,
            )?;
            if !complement.is_empty() {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl SuccessorScopeBudget {
    fn charge(
        &mut self,
        boxes: usize,
        coordinates: usize,
        work: usize,
    ) -> Result<(), ArtifactError> {
        let boxes = add(self.requested_boxes, boxes)?;
        let coordinates = add(self.requested_coordinates, coordinates)?;
        let work = add(self.work, work)?;
        check(
            "successor allocated boxes",
            boxes,
            self.limits.max_requested_boxes,
        )?;
        check(
            "successor allocated coordinates",
            coordinates,
            self.limits.max_requested_box_coordinate_cells,
        )?;
        check(
            "successor structural/split work",
            work,
            self.limits.max_split_operations,
        )?;
        self.requested_boxes = boxes;
        self.requested_coordinates = coordinates;
        self.work = work;
        Ok(())
    }

    fn remaining(&self) -> Result<CompletionGeometryLimits, ArtifactError> {
        let subtract = |limit: usize, used: usize| {
            limit
                .checked_sub(used)
                .ok_or(ArtifactError::ResourceBudgetExhausted {
                    resource: "successor geometry",
                })
        };
        Ok(CompletionGeometryLimits {
            max_split_operations: subtract(self.limits.max_split_operations, self.work)?,
            max_uncovered_boxes: subtract(self.limits.max_uncovered_boxes, self.uncovered_boxes)?,
            max_uncovered_box_coordinate_cells: subtract(
                self.limits.max_uncovered_box_coordinate_cells,
                self.uncovered_coordinates,
            )?,
            ..self.limits
        })
    }
}

fn partition_count(
    source: &LatticeBox,
    sector: &[bool],
    shifts: &[i64],
) -> Result<usize, ArtifactError> {
    let mut count = 1usize;
    for (axis, (&active, &shift)) in sector.iter().zip(shifts).enumerate() {
        let threshold = if active && shift < 0 {
            shift.unsigned_abs()
        } else if !active && shift > 0 {
            shift as u64
        } else {
            continue;
        };
        if source.lower()[axis] < threshold
            && source.upper()[axis].is_none_or(|end| end >= threshold)
        {
            count = mul(count, 2)?;
        }
    }
    Ok(count)
}

fn translated_image(
    source: &LatticeBox,
    sector: &[bool],
    shifts: &[i64],
) -> Result<(Vec<bool>, LatticeBox), ArtifactError> {
    let mut child_sector = Vec::new();
    let mut lower = Vec::new();
    let mut upper = Vec::new();
    reserve(&mut child_sector, sector.len())?;
    reserve(&mut lower, sector.len())?;
    reserve(&mut upper, sector.len())?;
    for (axis, (&active, &shift)) in sector.iter().zip(shifts).enumerate() {
        let translated = |local: u64| {
            let physical = if active {
                i128::from(local) + 1
            } else {
                -i128::from(local)
            };
            physical
                .checked_add(i128::from(shift))
                .ok_or_else(|| invalid("successor physical coordinate overflow"))
        };
        let first = translated(source.lower()[axis])?;
        let child_active = first > 0;
        let local = |physical: i128| {
            if (physical > 0) != child_active {
                return Err(invalid(
                    "successor partition crosses a child-sector boundary",
                ));
            }
            let value = if child_active {
                physical - 1
            } else {
                -physical
            };
            u64::try_from(value).map_err(|_| invalid("successor local coordinate exceeds u64"))
        };
        let first = local(first)?;
        let last = source.upper()[axis]
            .map(|value| translated(value).and_then(local))
            .transpose()?;
        if active == child_active {
            lower.push(first);
            upper.push(last);
        } else {
            // A sign-reversing branch must be bounded by the crossing point.
            lower.push(last.ok_or_else(|| invalid("unbounded sign-reversing successor piece"))?);
            upper.push(Some(first));
        }
        child_sector.push(child_active);
    }
    Ok((
        child_sector,
        LatticeBox::try_new(lower, upper).map_err(geometry_error)?,
    ))
}

fn copy_box(cell: &LatticeBox) -> Result<LatticeBox, ArtifactError> {
    LatticeBox::try_new(cell.lower().iter().copied(), cell.upper().iter().copied())
        .map_err(geometry_error)
}
fn reserve<T>(items: &mut Vec<T>, count: usize) -> Result<(), ArtifactError> {
    items
        .try_reserve_exact(count)
        .map_err(|_| ArtifactError::AllocationFailure {
            resource: "successor geometry",
            requested: count,
        })
}
fn add(a: usize, b: usize) -> Result<usize, ArtifactError> {
    a.checked_add(b).ok_or_else(overflow)
}
fn mul(a: usize, b: usize) -> Result<usize, ArtifactError> {
    a.checked_mul(b).ok_or_else(overflow)
}
fn overflow() -> ArtifactError {
    ArtifactError::ResourceCountOverflow {
        resource: "successor geometry",
    }
}
fn invalid(detail: &'static str) -> ArtifactError {
    ArtifactError::InvalidRuleShape { detail }
}
fn check(resource: &'static str, requested: usize, limit: usize) -> Result<(), ArtifactError> {
    if requested > limit {
        return Err(ArtifactError::ResourceLimit {
            resource,
            requested,
            limit,
        });
    }
    Ok(())
}
fn geometry_error(error: CompletionGeometryError) -> ArtifactError {
    match error {
        CompletionGeometryError::ResourceCountOverflow { resource } => {
            ArtifactError::ResourceCountOverflow { resource }
        }
        CompletionGeometryError::ResourceLimit {
            resource,
            requested,
            limit,
        } => ArtifactError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        CompletionGeometryError::AllocationFailure {
            resource,
            requested,
        } => ArtifactError::AllocationFailure {
            resource,
            requested,
        },
        _ => invalid("successor box geometry is invalid"),
    }
}

#[cfg(test)]
mod tests;
