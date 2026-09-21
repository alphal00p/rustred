use super::model::*;
use crate::foundry::completion::{BoxCover, CompletionGeometryLimits, LatticeBox};

pub(super) struct Budget {
    pub limits: OwnerDomainMatchLimits,
    pub stats: OwnerDomainMatchStats,
}
pub(super) fn charge(
    value: &mut usize,
    n: usize,
    limit: usize,
    resource: &'static str,
) -> Result<(), OwnerDomainMatchFailure> {
    let requested = value
        .checked_add(n)
        .ok_or(OwnerDomainMatchFailure::CountOverflow { resource })?;
    if requested > limit {
        return Err(OwnerDomainMatchFailure::ResourceLimit {
            resource,
            requested,
            limit,
        });
    }
    *value = requested;
    Ok(())
}
pub(super) fn geometry(error: impl ToString) -> OwnerDomainMatchFailure {
    OwnerDomainMatchFailure::Geometry(error.to_string())
}
impl Budget {
    pub fn cells<const N: usize>(&mut self, count: usize) -> Result<(), OwnerDomainMatchFailure> {
        let coordinates = count.checked_mul(N).and_then(|n| n.checked_mul(2)).ok_or(
            OwnerDomainMatchFailure::CountOverflow {
                resource: "coordinate cells",
            },
        )?;
        // Validate both allowances before committing either counter.
        let mut cells = self.stats.cells;
        let mut entries = self.stats.coordinate_cells;
        charge(&mut cells, count, self.limits.max_cells, "cells")?;
        charge(
            &mut entries,
            coordinates,
            self.limits.max_coordinate_cells,
            "coordinate cells",
        )?;
        self.stats.cells = cells;
        self.stats.coordinate_cells = entries;
        Ok(())
    }
    pub fn copy<const N: usize>(
        &mut self,
        cell: &LatticeBox,
    ) -> Result<LatticeBox, OwnerDomainMatchFailure> {
        self.cells::<N>(1)?;
        LatticeBox::try_new(cell.lower().iter().copied(), cell.upper().iter().copied())
            .map_err(geometry)
    }
    /// Intersect with coordinate equalities, returning None for disjoint boxes.
    pub fn restrict<const N: usize>(
        &mut self,
        cell: &LatticeBox,
        fixed: impl IntoIterator<Item = (usize, u64)>,
    ) -> Result<Option<LatticeBox>, OwnerDomainMatchFailure> {
        self.cells::<N>(2)?; // temporary endpoints plus admitted native box
        let mut lower = cell.lower().to_vec();
        let mut upper = cell.upper().to_vec();
        for (axis, x) in fixed {
            if x < lower[axis] || upper[axis].is_some_and(|u| x > u) {
                return Ok(None);
            }
            lower[axis] = x;
            upper[axis] = Some(x);
        }
        LatticeBox::try_new(lower, upper)
            .map(Some)
            .map_err(geometry)
    }
    /// Exact rectangle difference through the existing BoxCover. Reserve a
    /// conservative 2N+4 box envelope before its internal cloning/allocation.
    pub fn subtract<const N: usize>(
        &mut self,
        cell: LatticeBox,
        cut: &LatticeBox,
    ) -> Result<Vec<LatticeBox>, OwnerDomainMatchFailure> {
        let count = N.checked_mul(2).and_then(|x| x.checked_add(4)).ok_or(
            OwnerDomainMatchFailure::CountOverflow {
                resource: "split scratch",
            },
        )?;
        self.cells::<N>(count)?;
        let max_ops = N
            .checked_add(1)
            .ok_or(OwnerDomainMatchFailure::CountOverflow {
                resource: "split operations",
            })?;
        charge(
            &mut self.stats.split_operations,
            max_ops,
            self.limits.max_split_operations,
            "split operations",
        )?;
        let limits = CompletionGeometryLimits {
            max_arity: N,
            max_requested_boxes: 1,
            max_requested_box_coordinate_cells: 2 * N,
            max_uncovered_boxes: 2 * N,
            max_uncovered_box_coordinate_cells: 4 * N * N,
            max_split_operations: max_ops,
            ..Default::default()
        };
        let cover = BoxCover::try_new(
            N,
            [
                LatticeBox::try_new(cut.lower().iter().copied(), cut.upper().iter().copied())
                    .map_err(geometry)?,
            ],
            limits,
        )
        .map_err(geometry)?;
        let remainder = cover.uncovered_within(cell).map_err(geometry)?;
        let mut out = Vec::new();
        out.try_reserve_exact(remainder.boxes().len())
            .map_err(|_| OwnerDomainMatchFailure::AllocationFailure {
                resource: "split result",
            })?;
        for cell in remainder.boxes() {
            out.push(self.copy::<N>(cell)?);
        }
        Ok(out)
    }
}
pub(super) fn minimum_rank<const N: usize>(cell: &LatticeBox, owner: &[bool; N]) -> u128 {
    cell.lower()
        .iter()
        .zip(owner)
        .filter(|(_, active)| !**active)
        .map(|(&x, _)| u128::from(x))
        .sum()
}
