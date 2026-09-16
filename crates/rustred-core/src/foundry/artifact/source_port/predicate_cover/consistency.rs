//! Discharge Boolean literals contradicted by exact affine consequences.
//!
//! Called on possible uncovered boxes before further Boolean branching.
//! Native Symbolica partial substitution owns all algebra. Varying axes are
//! not sampled. A bounded native affine rank check handles joint implications
//! only after these cheaper assignment-independent classifications.

use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};

use symbolica::prelude::Integer;

use super::{Atom, LatticeBox};

#[path = "consistency/implication.rs"]
mod implication;

/// Shared by the entire Boolean traversal, not reset for each failed box.
struct WorkBudget {
    remaining: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct WorkExhausted;

impl Default for WorkBudget {
    fn default() -> Self {
        Self {
            remaining: 4_194_304,
        }
    }
}

impl WorkBudget {
    fn charge(&mut self, atom: &Atom<'_>, singletons: usize) -> Result<usize, WorkExhausted> {
        let Some(work) = atom
            .equation
            .exponents
            .len()
            .checked_add(atom.equation.coefficients.len())
            .and_then(|cells| cells.checked_mul(singletons + 1))
        else {
            self.remaining = 0;
            return Err(WorkExhausted);
        };
        let work = work.max(1);
        self.charge_units(work)?;
        Ok(work)
    }

    fn charge_units(&mut self, work: usize) -> Result<(), WorkExhausted> {
        let Some(remaining) = self.remaining.checked_sub(work) else {
            self.remaining = 0;
            return Err(WorkExhausted);
        };
        self.remaining = remaining;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RestrictionTruth {
    Zero,
    NonzeroConstant,
    Unknown,
    // Retain the original malformed-atom short circuit, independently of a
    // valid polynomial whose restriction is merely nonconstant or panicked.
    Unsupported,
}

#[derive(Clone, Copy)]
struct CacheLimits {
    boxes: usize,
    atom_slots: usize,
    coordinate_cells: usize,
}

impl Default for CacheLimits {
    fn default() -> Self {
        Self {
            boxes: 4_096,
            atom_slots: 131_072,
            coordinate_cells: 131_072,
        }
    }
}

/// Small observational counters; never consulted to establish a proof.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CacheStatistics {
    hits: usize,
    misses: usize,
    native_work: usize,
    uncached_results: usize,
    atom_slots: usize,
    coordinate_cells: usize,
}

/// Assignment-independent native restrictions for one immutable traversal.
///
/// The borrowed sector and atom table are bound at construction: an ordinal
/// cannot accidentally refer to a different polynomial, map, or sector in a
/// later call. Exact box keys retain all endpoints, not sampled points. Only
/// tiny classifications are cached, never Symbolica expressions. Capacity
/// exhaustion falls back to the same budgeted native calculation.
pub(super) struct RestrictionCache<'a> {
    sector: &'a [bool],
    atoms: &'a [Atom<'a>],
    budget: WorkBudget,
    limits: CacheLimits,
    entries: BTreeMap<LatticeBox, Box<[Option<RestrictionTruth>]>>,
    statistics: CacheStatistics,
}

impl<'a> RestrictionCache<'a> {
    pub(super) fn new(sector: &'a [bool], atoms: &'a [Atom<'a>]) -> Self {
        Self {
            sector,
            atoms,
            budget: WorkBudget::default(),
            limits: CacheLimits::default(),
            entries: BTreeMap::new(),
            statistics: CacheStatistics::default(),
        }
    }

    /// `true` means at least one assigned atom cannot hold anywhere in this
    /// box. `false` includes unsupported cases. Budget exhaustion is explicit,
    /// never disguised as a potentially uncovered mathematical domain.
    pub(super) fn contradicts(
        &mut self,
        cell: &LatticeBox,
        assignments: &[Option<bool>],
    ) -> Result<bool, WorkExhausted> {
        if cell.arity() != self.sector.len() || self.atoms.len() != assignments.len() {
            return Ok(false);
        }
        // Coordinate-only covers and the completely unassigned root need no
        // native integers, polynomial copies, or consistency-budget charge.
        if self.atoms.is_empty() || assignments.iter().all(Option::is_none) {
            return Ok(false);
        }
        let has_singletons =
            (0..cell.arity()).any(|axis| cell.upper()[axis] == Some(cell.lower()[axis]));
        // All-hit calls construct neither native integer endpoints nor copies
        // of exact polynomials. A miss builds the singleton values only once.
        let mut singleton = None;
        let mut unresolved = Vec::new();
        for (ordinal, &assignment) in assignments.iter().enumerate() {
            let Some(expected_zero) = assignment else {
                continue;
            };
            let truth = if !has_singletons {
                RestrictionTruth::Unknown
            } else {
                match self.entries.get(cell).and_then(|row| row[ordinal]) {
                    Some(truth) => {
                        self.statistics.hits += 1;
                        truth
                    }
                    None => {
                        self.statistics.misses += 1;
                        let singleton =
                            singleton.get_or_insert_with(|| singleton_values(self.sector, cell));
                        let atom = &self.atoms[ordinal];
                        self.statistics.native_work += self.budget.charge(atom, singleton.len())?;
                        let truth = restrict_atom(atom, self.sector.len(), singleton);
                        self.remember(cell, ordinal, truth);
                        truth
                    }
                }
            };
            match truth {
                RestrictionTruth::Zero if !expected_zero => return Ok(true),
                RestrictionTruth::NonzeroConstant if expected_zero => return Ok(true),
                RestrictionTruth::Unsupported => return Ok(false),
                RestrictionTruth::Unknown => unresolved.push((ordinal, expected_zero)),
                _ => {}
            }
        }
        let before = self.budget.remaining;
        let result =
            implication::contradicts(self.sector, self.atoms, cell, &unresolved, &mut self.budget);
        self.statistics.native_work += before - self.budget.remaining;
        result
    }

    fn remember(&mut self, cell: &LatticeBox, ordinal: usize, truth: RestrictionTruth) {
        if let Some(row) = self.entries.get_mut(cell) {
            row[ordinal] = Some(truth);
            return;
        }
        let stored = (|| {
            if self.entries.len() >= self.limits.boxes {
                return None;
            }
            let atom_slots = self.statistics.atom_slots.checked_add(self.atoms.len())?;
            let coordinate_cells = self
                .statistics
                .coordinate_cells
                .checked_add(cell.arity().checked_mul(2)?)?;
            if atom_slots > self.limits.atom_slots
                || coordinate_cells > self.limits.coordinate_cells
            {
                return None;
            }
            let mut row = Vec::new();
            row.try_reserve_exact(self.atoms.len()).ok()?;
            row.resize(self.atoms.len(), None);
            row[ordinal] = Some(truth);
            let key =
                LatticeBox::try_new(cell.lower().iter().copied(), cell.upper().iter().copied())
                    .ok()?;
            self.entries.insert(key, row.into_boxed_slice());
            self.statistics.atom_slots = atom_slots;
            self.statistics.coordinate_cells = coordinate_cells;
            Some(())
        })();
        if stored.is_none() {
            self.statistics.uncached_results += 1;
        }
    }
}

fn singleton_values(sector: &[bool], cell: &LatticeBox) -> Vec<(usize, Integer)> {
    sector
        .iter()
        .enumerate()
        .filter_map(|(axis, &active)| {
            if cell.upper()[axis] != Some(cell.lower()[axis]) {
                return None;
            }
            let local = Integer::from(cell.lower()[axis]);
            Some((
                axis,
                if active {
                    local + Integer::one()
                } else {
                    -local
                },
            ))
        })
        .collect()
}

fn restrict_atom(
    atom: &Atom<'_>,
    arity: usize,
    singleton: &[(usize, Integer)],
) -> RestrictionTruth {
    if !admitted_atom(atom, arity) {
        return RestrictionTruth::Unsupported;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let mut restricted = atom.equation.clone();
        for (axis, value) in singleton {
            restricted = restricted.replace(atom.indices[*axis], value);
        }
        if restricted.is_zero() {
            RestrictionTruth::Zero
        } else if restricted.is_constant() {
            RestrictionTruth::NonzeroConstant
        } else {
            RestrictionTruth::Unknown
        }
    }))
    .unwrap_or(RestrictionTruth::Unknown)
}

fn admitted_atom(atom: &Atom<'_>, arity: usize) -> bool {
    let equation = atom.equation;
    if atom.indices.len() != arity
        || atom.indices.iter().enumerate().any(|(axis, &index)| {
            index >= equation.nvars() || atom.indices[..axis].contains(&index)
        })
        || equation.coefficients.len().checked_mul(equation.nvars())
            != Some(equation.exponents.len())
    {
        return false;
    }
    // Atoms come from authenticated affine domains. Keep malformed index
    // maps and unsupported original equations inconclusive even if native
    // specialization of this box would hide their unsupported content.
    for term in 0..equation.coefficients.len() {
        let mut variable_seen = false;
        for (position, &power) in equation.exponents(term).iter().enumerate() {
            if power == 0 {
                continue;
            }
            if power != 1 || variable_seen || !atom.indices.contains(&position) {
                return false;
            }
            variable_seen = true;
        }
    }
    true
}

#[cfg(test)]
#[path = "consistency/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "consistency/branching_tests.rs"]
mod branching_tests;

#[cfg(test)]
#[path = "consistency/cache_tests.rs"]
mod cache_tests;
