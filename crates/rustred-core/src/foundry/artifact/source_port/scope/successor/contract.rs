//! Entry-contract queries reuse the successor owner's exact covers and ledger.

use super::super::contract::EntryScope;
use super::*;
use crate::foundry::completion::UncoveredPartition;

impl PreparedSuccessorScope {
    pub(in crate::foundry::artifact) fn destinations(
        &self,
    ) -> impl ExactSizeIterator<Item = DestinationScope<'_>> {
        self.destinations
            .iter()
            .map(|(sector, cover)| DestinationScope {
                sector,
                boxes: cover.boxes(),
            })
    }

    pub(in crate::foundry::artifact) fn sector_boxes(
        &self,
        sector: &[bool],
    ) -> Option<&[LatticeBox]> {
        self.destinations
            .binary_search_by(|(key, _)| key.as_slice().cmp(sector))
            .ok()
            .map(|index| self.destinations[index].1.boxes())
    }

    pub(in crate::foundry::artifact) fn entry_is_contained(
        &mut self,
        entry: &EntryScope,
    ) -> Result<bool, ArtifactError> {
        if entry.root().arity() != self.arity {
            return Err(invalid("entry and successor coordinate arities differ"));
        }
        let count = super::super::sector_count(entry.root())?;
        // Charge the entire root traversal before allocation, including a
        // conservative per-sector mask allowance and one reused full box.
        self.budget.charge(
            count,
            add(mul(count, self.arity)?, mul(self.arity, 2)?)?,
            mul(add(count, 1)?, self.arity)?,
        )?;
        let mut sector = Vec::new();
        reserve(&mut sector, self.arity)?;
        sector.resize(self.arity, false);
        let full = LatticeBox::try_new(
            std::iter::repeat_n(0, self.arity),
            std::iter::repeat_n(None, self.arity),
        )
        .map_err(geometry_error)?;
        for code in 0..count {
            let mut active_axis = 0;
            for (bit, &allowed) in sector.iter_mut().zip(entry.root().active_bits()) {
                *bit = allowed && (code >> active_axis) & 1 != 0;
                active_axis += usize::from(allowed);
            }
            let Some(complement) = self.uncovered_in_sector(&full, &sector)? else {
                return Ok(false);
            };
            self.budget
                .charge(0, 0, mul(mul(complement.boxes().len(), self.arity)?, 3)?)?;
            for missing in complement.boxes() {
                if entry.intersects_local_box(&sector, missing)? {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    pub(in crate::foundry::artifact) fn contains_box(
        &mut self,
        source: &LatticeBox,
        sector: &[bool],
    ) -> Result<bool, ArtifactError> {
        Ok(self
            .uncovered_in_sector(source, sector)?
            .is_some_and(|pieces| pieces.is_empty()))
    }

    fn uncovered_in_sector(
        &mut self,
        source: &LatticeBox,
        sector: &[bool],
    ) -> Result<Option<UncoveredPartition>, ArtifactError> {
        if source.arity() != self.arity || sector.len() != self.arity {
            return Err(invalid("proof-envelope source sector arity mismatch"));
        }
        self.budget.charge(
            1,
            mul(self.arity, 2)?,
            mul(self.arity, add(3, ceil_log2(self.destinations.len()))?)?,
        )?;
        let Ok(index) = self
            .destinations
            .binary_search_by(|(key, _)| key.as_slice().cmp(sector))
        else {
            return Ok(None);
        };
        self.query_complement(index, copy_box(source)?).map(Some)
    }

    /// One ledger for entry queries, source clipping checks and translated RHS
    /// queries. A failed native box subtraction cannot reset its work allowance.
    pub(super) fn query_complement(
        &mut self,
        index: usize,
        image: LatticeBox,
    ) -> Result<UncoveredPartition, ArtifactError> {
        let remaining = self.budget.remaining()?;
        let complement = match self.destinations[index].1.uncovered_within_budget(
            image,
            remaining.max_uncovered_boxes,
            remaining.max_uncovered_box_coordinate_cells,
            remaining.max_split_operations,
        ) {
            Ok(complement) => complement,
            Err(issue) => {
                self.budget.work = self.budget.limits.max_split_operations;
                return Err(geometry_error(issue));
            }
        };
        self.budget.work = add(self.budget.work, complement.split_operations())?;
        self.budget.uncovered_boxes = add(self.budget.uncovered_boxes, complement.boxes().len())?;
        self.budget.uncovered_coordinates = add(
            self.budget.uncovered_coordinates,
            mul(mul(complement.boxes().len(), self.arity)?, 2)?,
        )?;
        Ok(complement)
    }
}
