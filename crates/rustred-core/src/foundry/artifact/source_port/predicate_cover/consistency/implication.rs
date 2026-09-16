//! Sufficient exact affine implications, delegated to the existing native RREF.

use crate::solver::canonical_equalities;

use super::{Atom, LatticeBox, WorkBudget, WorkExhausted, admitted_atom, singleton_values};
use std::panic::{AssertUnwindSafe, catch_unwind};

const MAX_EQUATIONS: usize = 32;
const MAX_MATRIX_CELLS: usize = 65_536;
const MAX_POLYNOMIAL_CELLS: usize = 262_144;

/// Joint contradictions need at least two unresolved literals, one true.
/// Existing singleton/individual-domain proofs handle unary contradictions.
/// Unknown, unsupported and failed native calls confer no proof authority.
pub(super) fn contradicts(
    sector: &[bool],
    atoms: &[Atom<'_>],
    cell: &LatticeBox,
    unresolved: &[(usize, bool)],
    budget: &mut WorkBudget,
) -> Result<bool, WorkExhausted> {
    if unresolved.len() < 2 || !unresolved.iter().any(|&(_, truth)| truth) {
        return Ok(false);
    }
    if unresolved.len() > MAX_EQUATIONS {
        return Err(WorkExhausted);
    }
    let Some(template) = atoms.get(unresolved[0].0) else {
        return Ok(false);
    };
    if sector.len() != cell.arity() || sector.is_empty() {
        return Ok(false);
    }
    let mut input_cells = 0usize;
    for &(ordinal, _) in unresolved {
        let Some(atom) = atoms.get(ordinal) else {
            return Ok(false);
        };
        // Bind every original map and admit original affine support before
        // fixed substitution could hide malformed/nonlinear input.
        if !admitted_atom(atom, sector.len())
            || atom.indices != template.indices
            || atom.equation.variables() != template.equation.variables()
        {
            return Ok(false);
        }
        input_cells = input_cells
            .checked_add(atom.equation.exponents.len())
            .ok_or(WorkExhausted)?;
    }
    if input_cells > MAX_POLYNOMIAL_CELLS {
        return Err(WorkExhausted);
    }
    let columns = sector.len().checked_add(1).ok_or(WorkExhausted)?;
    let true_count = unresolved.iter().filter(|&&(_, truth)| truth).count();
    let maximum_rows = true_count.checked_add(1).ok_or(WorkExhausted)?;
    if maximum_rows
        .checked_mul(columns)
        .is_none_or(|cells| cells > MAX_MATRIX_CELLS)
    {
        return Err(WorkExhausted);
    }
    let singleton_count = (0..cell.arity())
        .filter(|&axis| cell.upper()[axis] == Some(cell.lower()[axis]))
        .count();
    for &(ordinal, _) in unresolved {
        budget.charge(&atoms[ordinal], singleton_count)?;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let singletons = singleton_values(sector, cell);
        let mut equalities = Vec::with_capacity(maximum_rows);
        let mut disequalities = Vec::new();
        for &(ordinal, truth) in unresolved {
            let atom = &atoms[ordinal];
            let mut equation = atom.equation.clone();
            for (axis, value) in &singletons {
                equation = equation.replace(atom.indices[*axis], value);
            }
            if truth {
                equalities.push(equation);
            } else {
                disequalities.push(equation);
            }
        }
        // Singleton values are already substituted as arbitrary Integers.
        // No compact fixed-coordinate narrowing or chart shortcut is used.
        let fixed = vec![None; sector.len()];
        charge_matrix(budget, equalities.len(), columns)?;
        let base = match canonical_equalities(&fixed, &equalities, template.indices) {
            Ok(None) => return Ok(true),
            Ok(Some((matrix, _))) => matrix.nrows(),
            Err(_) => return Ok(false),
        };
        for equation in disequalities {
            equalities.push(equation);
            charge_matrix(budget, equalities.len(), columns)?;
            let result = canonical_equalities(&fixed, &equalities, template.indices);
            equalities.pop();
            // The extended system must be consistent. Inconsistency of
            // T AND f=0 does NOT contradict the current branch T AND f!=0.
            match result {
                Ok(Some((matrix, _))) if matrix.nrows() == base => return Ok(true),
                Err(_) => return Ok(false),
                _ => {}
            }
        }
        Ok(false)
    }))
    .unwrap_or(Ok(false))
}

fn charge_matrix(
    budget: &mut WorkBudget,
    rows: usize,
    columns: usize,
) -> Result<(), WorkExhausted> {
    let work = rows
        .checked_mul(columns)
        .and_then(|cells| cells.checked_mul(columns))
        .ok_or(WorkExhausted)?;
    budget.charge_units(work)
}

#[cfg(test)]
#[path = "implication_tests.rs"]
mod tests;
