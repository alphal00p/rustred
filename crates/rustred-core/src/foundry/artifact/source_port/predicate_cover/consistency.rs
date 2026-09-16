//! Discharge only Boolean literals contradicted by exact fixed coordinates.
//!
//! Called on possible uncovered boxes before further Boolean branching.
//! Native Symbolica partial substitution owns all algebra. Varying axes are
//! not sampled, and implications between equations are not inferred.

use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::prelude::Integer;

use super::{Atom, LatticeBox};

/// Shared by the entire Boolean traversal, not reset for each failed box.
pub(super) struct WorkBudget {
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
    /// `true` means at least one assigned atom cannot hold anywhere in this
    /// box. `false` includes unsupported cases. Budget exhaustion is explicit,
    /// never disguised as a potentially uncovered mathematical domain.
    pub(super) fn contradicts(
        &mut self,
        sector: &[bool],
        cell: &LatticeBox,
        atoms: &[Atom<'_>],
        assignments: &[Option<bool>],
    ) -> Result<bool, WorkExhausted> {
        if cell.arity() != sector.len() || atoms.len() != assignments.len() {
            return Ok(false);
        }
        // Coordinate-only covers and the completely unassigned root need no
        // native integers, polynomial copies, or consistency-budget charge.
        if atoms.is_empty() || assignments.iter().all(Option::is_none) {
            return Ok(false);
        }
        let singleton: Vec<_> = sector
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
            .collect();
        if singleton.is_empty() {
            return Ok(false);
        }
        for (atom, &assignment) in atoms.iter().zip(assignments) {
            let Some(expected_zero) = assignment else {
                continue;
            };
            let equation = atom.equation;
            let Some(work) = equation
                .exponents
                .len()
                .checked_add(equation.coefficients.len())
                .and_then(|cells| cells.checked_mul(singleton.len() + 1))
            else {
                self.remaining = 0;
                return Err(WorkExhausted);
            };
            let Some(remaining) = self.remaining.checked_sub(work.max(1)) else {
                self.remaining = 0;
                return Err(WorkExhausted);
            };
            self.remaining = remaining;
            if atom.indices.len() != sector.len()
                || atom.indices.iter().enumerate().any(|(axis, &index)| {
                    index >= equation.nvars() || atom.indices[..axis].contains(&index)
                })
                || equation.coefficients.len().checked_mul(equation.nvars())
                    != Some(equation.exponents.len())
            {
                return Ok(false);
            }
            // Atoms come from authenticated affine domains. Keep malformed
            // index maps and unsupported original equations inconclusive even
            // if specializing this box would hide their unsupported content.
            for term in 0..equation.coefficients.len() {
                let mut variable_seen = false;
                for (position, &power) in equation.exponents(term).iter().enumerate() {
                    if power == 0 {
                        continue;
                    }
                    if power != 1 || variable_seen || !atom.indices.contains(&position) {
                        return Ok(false);
                    }
                    variable_seen = true;
                }
            }
            let contradicted = catch_unwind(AssertUnwindSafe(|| {
                let mut restricted = equation.clone();
                for (axis, value) in &singleton {
                    restricted = restricted.replace(atom.indices[*axis], value);
                }
                if expected_zero {
                    restricted.is_constant() && !restricted.is_zero()
                } else {
                    restricted.is_zero()
                }
            }))
            .unwrap_or(false);
            if contradicted {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
#[path = "consistency/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "consistency/branching_tests.rs"]
mod branching_tests;
