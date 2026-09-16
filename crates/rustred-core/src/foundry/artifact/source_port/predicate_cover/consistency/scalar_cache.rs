//! Bounded successful scalar outcomes; no native polynomial survives a call.

use super::{LatticeBox, RestrictionCache, add};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BaseOutcome {
    Inconsistent,
    Rank(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExtensionOutcome {
    Contradicts,
    NoContradictionProved,
}

pub(super) struct ScalarEntry {
    cell: LatticeBox,
    true_ordinals: Box<[usize]>,
    pub base: BaseOutcome,
    extensions: Vec<(usize, ExtensionOutcome)>,
}

fn true_ordinals(assignments: &[Option<bool>]) -> impl Iterator<Item = usize> + '_ {
    assignments
        .iter()
        .enumerate()
        .filter_map(|(ordinal, &truth)| (truth == Some(true)).then_some(ordinal))
}

impl RestrictionCache<'_> {
    pub(super) fn scalar_position(
        &self,
        cell: &LatticeBox,
        assignments: &[Option<bool>],
    ) -> Result<usize, usize> {
        self.implications.binary_search_by(|entry| {
            entry.cell.cmp(cell).then_with(|| {
                entry
                    .true_ordinals
                    .iter()
                    .copied()
                    .cmp(true_ordinals(assignments))
            })
        })
    }

    pub(super) fn remember_base(
        &mut self,
        cell: &LatticeBox,
        assignments: &[Option<bool>],
        base: BaseOutcome,
    ) -> Option<usize> {
        let stored = (|| {
            let position = match self.scalar_position(cell, assignments) {
                Ok(position) => return Some(position),
                Err(position) => position,
            };
            // Include every true assignment, even one already classified Zero.
            let count = true_ordinals(assignments).count();
            let (slots, coordinates) =
                self.storage_after(count, cell.arity().checked_mul(2)?, true)?;
            let mut ordinals = Vec::new();
            ordinals.try_reserve_exact(count).ok()?;
            ordinals.extend(true_ordinals(assignments));
            let key =
                LatticeBox::try_new(cell.lower().iter().copied(), cell.upper().iter().copied())
                    .ok()?;
            self.implications.try_reserve_exact(1).ok()?;
            self.implications.insert(
                position,
                ScalarEntry {
                    cell: key,
                    true_ordinals: ordinals.into_boxed_slice(),
                    base,
                    extensions: Vec::new(),
                },
            );
            self.statistics.atom_slots = slots;
            self.statistics.coordinate_cells = coordinates;
            Some(position)
        })();
        if stored.is_none() {
            add(&mut self.statistics.base_fallbacks, 1);
        }
        stored
    }

    pub(super) fn cached_extension(
        &self,
        entry: Option<usize>,
        ordinal: usize,
    ) -> Option<ExtensionOutcome> {
        let extensions = &self.implications.get(entry?)?.extensions;
        extensions
            .binary_search_by_key(&ordinal, |&(key, _)| key)
            .ok()
            .map(|position| extensions[position].1)
    }

    pub(super) fn remember_extension(
        &mut self,
        entry: Option<usize>,
        ordinal: usize,
        outcome: ExtensionOutcome,
    ) {
        let stored = (|| {
            let entry = entry?;
            let (slots, coordinates) = self.storage_after(1, 0, false)?;
            let extensions = &mut self.implications[entry].extensions;
            let position = match extensions.binary_search_by_key(&ordinal, |&(key, _)| key) {
                Ok(_) => return Some(()),
                Err(position) => position,
            };
            extensions.try_reserve_exact(1).ok()?;
            extensions.insert(position, (ordinal, outcome));
            self.statistics.atom_slots = slots;
            self.statistics.coordinate_cells = coordinates;
            Some(())
        })();
        if stored.is_none() {
            add(&mut self.statistics.extension_fallbacks, 1);
        }
    }
}
