//! Exhaust a single short integer interval using the existing native proofs.
//! Every other interval, including infinite directions, remains unchanged.

use crate::foundry::completion::MAX_BOUNDED_AXIS_FACES;

use super::diagnostics::{LocalCap, Shape, add};
use super::implication::{MAX_EQUATIONS, MAX_MATRIX_CELLS, MAX_POLYNOMIAL_CELLS};
use super::{LatticeBox, RestrictionCache, WorkExhausted, admitted_atom};

impl RestrictionCache<'_> {
    pub(super) fn contradicts_finite_axis(
        &mut self,
        cell: &LatticeBox,
        assignments: &[Option<bool>],
    ) -> Result<bool, WorkExhausted> {
        if cell.arity() != self.sector.len() || assignments.len() != self.atoms.len() {
            return Ok(false);
        }
        let true_count = assignments.iter().filter(|&&a| a == Some(true)).count();
        let false_count = assignments.iter().filter(|&&a| a == Some(false)).count();
        // This sufficient fallback targets the existing implication lane.
        // Coordinate-only/no-true/one-atom paths retain their cheap behavior.
        if true_count == 0 || false_count == 0 {
            return Ok(false);
        }
        let shape = Shape {
            rows: true_count + false_count,
            columns: cell.arity().saturating_add(1),
            true_count,
            false_count,
        };
        if shape.rows > MAX_EQUATIONS {
            return Err(self.local_exhaustion(LocalCap::Equations, Some(shape.rows), shape));
        }
        let first = assignments.iter().position(Option::is_some).unwrap();
        let template = &self.atoms[first];
        let mut input_cells = Some(0usize);
        for (atom, assignment) in self.atoms.iter().zip(assignments) {
            if assignment.is_none() {
                continue;
            }
            // Original admission precedes subdivision; singleton restriction
            // must not conceal nonlinear input or foreign parameter support.
            if !admitted_atom(atom, cell.arity())
                || atom.indices != template.indices
                || atom.equation.variables() != template.equation.variables()
            {
                return Ok(false);
            }
            input_cells = input_cells.and_then(|n| n.checked_add(atom.equation.exponents.len()));
        }
        let selected = (0..cell.arity())
            .filter_map(|axis| {
                let lower = cell.lower()[axis];
                let count = cell.upper()[axis]?.checked_sub(lower)?.checked_add(1)?;
                if !(2..=MAX_BOUNDED_AXIS_FACES as u64).contains(&count)
                    || !self
                        .atoms
                        .iter()
                        .zip(assignments)
                        .any(|(atom, assignment)| {
                            assignment.is_some() && atom.equation.contains(atom.indices[axis])
                        })
                {
                    return None;
                }
                Some((count as usize, axis))
            })
            .min();
        let Some((count, axis)) = selected else {
            return Ok(false);
        };
        // Bound the aggregate prospective native shapes before allocating a
        // child. Actual native work is charged by the same cache/budget below,
        // not by a fresh per-face counter or a worst-case duplicate charge.
        let matrix_cells = true_count
            .checked_add(1)
            .and_then(|rows| rows.checked_mul(shape.columns))
            .and_then(|n| n.checked_mul(count));
        if matrix_cells.is_none_or(|n| n > MAX_MATRIX_CELLS) {
            return Err(self.local_exhaustion(LocalCap::MatrixCells, matrix_cells, shape));
        }
        let polynomial_cells = input_cells.and_then(|n| n.checked_mul(count));
        if polynomial_cells.is_none_or(|n| n > MAX_POLYNOMIAL_CELLS) {
            return Err(self.local_exhaustion(LocalCap::PolynomialCells, polynomial_cells, shape));
        }
        add(&mut self.statistics.finite_axis_refinements, 1);
        for offset in 0..count {
            // Cardinality was checked above. Offset never reaches count, so
            // an interval ending at u64::MAX has no overflowing final step.
            let value = cell.lower()[axis] + offset as u64;
            let child = LatticeBox::try_new(
                cell.lower()
                    .iter()
                    .enumerate()
                    .map(|(i, &n)| if i == axis { value } else { n }),
                cell.upper()
                    .iter()
                    .enumerate()
                    .map(|(i, &n)| if i == axis { Some(value) } else { n }),
            )
            .map_err(|_| self.local_exhaustion(LocalCap::Allocation, None, shape))?;
            add(&mut self.statistics.finite_axis_faces, 1);
            if !self.contradicts_without_refinement(&child, assignments)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
#[path = "refinement_tests.rs"]
mod tests;
