//! Sufficient exact affine implications, delegated to the existing native RREF.

use crate::{algebra::CoefficientPolynomial, solver::canonical_equalities};
use std::panic::{AssertUnwindSafe, catch_unwind};
use symbolica::prelude::Integer;

use super::diagnostics::{LocalCap, Shape, Stage, add};
use super::scalar_cache::{BaseOutcome, ExtensionOutcome};
use super::{
    LatticeBox, RestrictionCache, WorkExhausted, admitted_atom, polynomial_work, singleton_values,
};

#[cfg(test)]
use super::Atom;

pub(super) const MAX_EQUATIONS: usize = 32;
pub(super) const MAX_MATRIX_CELLS: usize = 65_536;
pub(super) const MAX_POLYNOMIAL_CELLS: usize = 262_144;

fn native_rank(
    fixed: &[Option<i16>],
    equations: &[CoefficientPolynomial],
    indices: &[usize],
) -> Result<Option<usize>, ()> {
    canonical_equalities(fixed, equations, indices)
        .map(|result| result.map(|(matrix, _)| matrix.nrows()))
        .map_err(|_| ())
}

impl RestrictionCache<'_> {
    pub(super) fn contradicts_implications(
        &mut self,
        cell: &LatticeBox,
        assignments: &[Option<bool>],
        unresolved: &[(usize, bool)],
    ) -> Result<bool, WorkExhausted> {
        self.contradicts_implications_using(cell, assignments, unresolved, native_rank)
    }

    /// The callback is a private test seam; production always uses precisely
    /// the existing canonical_equalities operation above.
    fn contradicts_implications_using<F>(
        &mut self,
        cell: &LatticeBox,
        assignments: &[Option<bool>],
        unresolved: &[(usize, bool)],
        mut native: F,
    ) -> Result<bool, WorkExhausted>
    where
        F: FnMut(&[Option<i16>], &[CoefficientPolynomial], &[usize]) -> Result<Option<usize>, ()>,
    {
        add(&mut self.statistics.implication_calls, 1);
        if unresolved.len() < 2 {
            add(&mut self.statistics.skipped_too_few, 1);
            return Ok(false);
        }
        if !unresolved.iter().any(|&(_, truth)| truth) {
            add(&mut self.statistics.skipped_no_true, 1);
            return Ok(false);
        }
        let shape = Shape {
            true_count: unresolved.iter().filter(|&&(_, truth)| truth).count(),
            false_count: unresolved.iter().filter(|&&(_, truth)| !truth).count(),
            columns: self.sector.len().saturating_add(1),
            rows: unresolved.len(),
        };
        // All original support, context, and size admissions precede lookup.
        if unresolved.len() > MAX_EQUATIONS {
            return Err(self.local_exhaustion(LocalCap::Equations, Some(unresolved.len()), shape));
        }
        let Some(template) = self.atoms.get(unresolved[0].0) else {
            add(&mut self.statistics.unsupported, 1);
            return Ok(false);
        };
        if self.sector.len() != cell.arity()
            || self.sector.is_empty()
            || self.atoms.len() != assignments.len()
        {
            add(&mut self.statistics.unsupported, 1);
            return Ok(false);
        }
        let mut input_cells = Some(0usize);
        for &(ordinal, _) in unresolved {
            let Some(atom) = self.atoms.get(ordinal) else {
                add(&mut self.statistics.unsupported, 1);
                return Ok(false);
            };
            if !admitted_atom(atom, self.sector.len())
                || atom.indices != template.indices
                || atom.equation.variables() != template.equation.variables()
            {
                add(&mut self.statistics.unsupported, 1);
                return Ok(false);
            }
            input_cells =
                input_cells.and_then(|cells| cells.checked_add(atom.equation.exponents.len()));
            if input_cells.is_none() {
                return Err(self.local_exhaustion(LocalCap::PolynomialCells, None, shape));
            }
        }
        if input_cells.is_none_or(|cells| cells > MAX_POLYNOMIAL_CELLS) {
            return Err(self.local_exhaustion(LocalCap::PolynomialCells, input_cells, shape));
        }
        let maximum_rows = shape.true_count.checked_add(1);
        let matrix_cells = maximum_rows.and_then(|rows| rows.checked_mul(shape.columns));
        if matrix_cells.is_none_or(|cells| cells > MAX_MATRIX_CELLS) {
            return Err(self.local_exhaustion(LocalCap::MatrixCells, matrix_cells, shape));
        }
        let singleton_count = (0..cell.arity())
            .filter(|&axis| cell.upper()[axis] == Some(cell.lower()[axis]))
            .count();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut equalities = None;
            let mut singletons = None;
            let mut fixed = None;
            let mut entry = self.scalar_position(cell, assignments).ok();
            let base = match entry {
                Some(position) => {
                    add(&mut self.statistics.base_hits, 1);
                    self.implications[position].base
                }
                None => {
                    add(&mut self.statistics.base_misses, 1);
                    self.materialize_true(
                        cell,
                        unresolved,
                        singleton_count,
                        shape,
                        &mut singletons,
                        &mut equalities,
                    )?;
                    let fixed = native_fixed(&mut fixed, self.sector.len());
                    self.charge_matrix(Stage::BaseReduction, shape.true_count, shape)?;
                    add(&mut self.statistics.base_calls, 1);
                    let base = match native(fixed, equalities.as_ref().unwrap(), template.indices) {
                        Ok(None) => {
                            add(&mut self.statistics.inconsistent_bases, 1);
                            BaseOutcome::Inconsistent
                        }
                        Ok(Some(rank)) => BaseOutcome::Rank(rank),
                        Err(()) => {
                            add(&mut self.statistics.native_errors, 1);
                            return Ok(false);
                        }
                    };
                    entry = self.remember_base(cell, assignments, base);
                    base
                }
            };
            let BaseOutcome::Rank(base_rank) = base else {
                return Ok(true);
            };
            for &(ordinal, truth) in unresolved {
                if truth {
                    continue;
                }
                let outcome = match self.cached_extension(entry, ordinal) {
                    Some(outcome) => {
                        add(&mut self.statistics.extension_hits, 1);
                        outcome
                    }
                    None => {
                        add(&mut self.statistics.extension_misses, 1);
                        // A cached base needs its true polynomials only if an
                        // unseen extension is reached. Rebuild them once, and
                        // never repeat its native base reduction.
                        self.materialize_true(
                            cell,
                            unresolved,
                            singleton_count,
                            shape,
                            &mut singletons,
                            &mut equalities,
                        )?;
                        let equation = self.materialize(
                            ordinal,
                            false,
                            singleton_count,
                            shape,
                            singletons.as_ref().unwrap(),
                        )?;
                        let equalities = equalities.as_mut().unwrap();
                        equalities.push(equation);
                        let fixed = native_fixed(&mut fixed, self.sector.len());
                        self.charge_matrix(Stage::ExtensionReduction, equalities.len(), shape)?;
                        add(&mut self.statistics.extension_calls, 1);
                        let result = native(fixed, equalities, template.indices);
                        equalities.pop();
                        let outcome = match result {
                            Ok(Some(rank)) if rank == base_rank => {
                                add(&mut self.statistics.equal_rank_contradictions, 1);
                                ExtensionOutcome::Contradicts
                            }
                            Ok(None) => {
                                // T AND f=0 inconsistent does NOT refute T AND f!=0.
                                add(&mut self.statistics.inconsistent_extensions, 1);
                                ExtensionOutcome::NoContradictionProved
                            }
                            Ok(Some(_)) => ExtensionOutcome::NoContradictionProved,
                            Err(()) => {
                                add(&mut self.statistics.native_errors, 1);
                                return Ok(false);
                            }
                        };
                        self.remember_extension(entry, ordinal, outcome);
                        outcome
                    }
                };
                if outcome == ExtensionOutcome::Contradicts {
                    return Ok(true);
                }
            }
            Ok(false)
        }));
        result.unwrap_or_else(|_| {
            add(&mut self.statistics.native_panics, 1);
            Ok(false)
        })
    }

    fn materialize_true(
        &mut self,
        cell: &LatticeBox,
        unresolved: &[(usize, bool)],
        singleton_count: usize,
        shape: Shape,
        singletons: &mut Option<Vec<(usize, Integer)>>,
        equalities: &mut Option<Vec<CoefficientPolynomial>>,
    ) -> Result<(), WorkExhausted> {
        if equalities.is_some() {
            return Ok(());
        }
        let mut equations = Vec::new();
        equations
            .try_reserve_exact(shape.true_count + 1)
            .map_err(|_| {
                self.local_exhaustion(LocalCap::Allocation, Some(shape.true_count + 1), shape)
            })?;
        let singletons = singletons.get_or_insert_with(|| singleton_values(self.sector, cell));
        for &(ordinal, truth) in unresolved {
            if truth {
                equations.push(self.materialize(
                    ordinal,
                    true,
                    singleton_count,
                    shape,
                    singletons,
                )?);
            }
        }
        *equalities = Some(equations);
        Ok(())
    }

    fn materialize(
        &mut self,
        ordinal: usize,
        truth: bool,
        singleton_count: usize,
        shape: Shape,
        singletons: &[(usize, Integer)],
    ) -> Result<CoefficientPolynomial, WorkExhausted> {
        let atom = &self.atoms[ordinal];
        let stage = if truth {
            Stage::TrueMaterialization
        } else {
            Stage::FalseMaterialization
        };
        self.charge_native(stage, polynomial_work(atom, singleton_count), shape)?;
        if truth {
            add(&mut self.statistics.true_materializations, 1);
        } else {
            add(&mut self.statistics.false_materializations, 1);
        }
        let mut equation = atom.equation.clone();
        for (axis, value) in singletons {
            add(&mut self.statistics.implication_replaces, 1);
            equation = equation.replace(atom.indices[*axis], value);
        }
        Ok(equation)
    }

    fn charge_matrix(
        &mut self,
        stage: Stage,
        rows: usize,
        mut shape: Shape,
    ) -> Result<(), WorkExhausted> {
        shape.rows = rows;
        let cells = rows.checked_mul(shape.columns);
        let work = cells.and_then(|cells| cells.checked_mul(shape.columns));
        let work = self.charge_native(stage, work, shape)?;
        add(&mut self.statistics.matrix_work, work);
        add(&mut self.statistics.matrix_rows, rows);
        add(&mut self.statistics.matrix_cells, cells.unwrap());
        Ok(())
    }
}

fn native_fixed(fixed: &mut Option<Vec<Option<i16>>>, arity: usize) -> &[Option<i16>] {
    // Arbitrary Integer singleton values were already substituted. No compact
    // fixed-coordinate narrowing and no restriction-chart shortcut is used.
    fixed.get_or_insert_with(|| vec![None; arity])
}

#[cfg(test)]
#[path = "implication_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "reuse_tests.rs"]
mod reuse_tests;
