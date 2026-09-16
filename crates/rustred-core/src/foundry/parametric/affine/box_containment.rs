//! Conservative whole-box containment in one complete affine predicate.
//! Native substitution owns the algebra; an inconclusive result is false.

use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::prelude::Integer;

use crate::foundry::completion::LatticeBox;

use super::AffineApplicationDomain;
use super::box_bounds::{MAX_MATRIX_CELLS, MAX_POLYNOMIAL_CELLS, linear_row};

impl AffineApplicationDomain {
    /// Prove that every point of `cell` satisfies the complete fixed face and
    /// every defining equality. Callers must additionally bind this domain's
    /// sector and native index map to their authenticated source context.
    ///
    /// Only singleton coordinates are substituted. A nonzero polynomial in
    /// any remaining coordinate is unknown, never evaluated at a corner.
    /// Cached chart/matrix metadata supplies no authority here.
    pub(crate) fn is_proved_to_contain_box(&self, cell: &LatticeBox) -> bool {
        if cell.arity() != self.sector.len()
            || self.fixed.len() != cell.arity()
            || self.indices.len() != cell.arity()
        {
            return false;
        }
        let Some(template) = self.equations.first() else {
            return false;
        };
        if self.indices.iter().enumerate().any(|(axis, &index)| {
            index >= template.nvars() || self.indices[..axis].contains(&index)
        }) || self.equations.iter().any(|equation| {
            equation.variables() != template.variables()
                || equation.coefficients.len().checked_mul(equation.nvars())
                    != Some(equation.exponents.len())
        }) {
            return false;
        }
        let input_work = self.equations.iter().try_fold(0usize, |total, equation| {
            total.checked_add(equation.exponents.len())
        });
        if input_work
            .and_then(|work| work.checked_mul(cell.arity().max(1)))
            .is_none_or(|work| work > MAX_POLYNOMIAL_CELLS)
            || self
                .equations
                .len()
                .checked_mul(cell.arity() + 1)
                .is_none_or(|work| work > MAX_MATRIX_CELLS)
        {
            return false;
        }
        catch_unwind(AssertUnwindSafe(|| {
            // Admit the ORIGINAL equations before specialization. Nonlinear
            // or parameter-dependent inputs cannot acquire affine authority
            // just because this particular box happens to fix a coordinate.
            if self
                .equations
                .iter()
                .any(|equation| linear_row(equation, &self.indices).is_none())
            {
                return false;
            }
            let mut singleton = Vec::new();
            for axis in 0..cell.arity() {
                let fixed = cell.upper()[axis] == Some(cell.lower()[axis]);
                if self.fixed[axis].is_some() && !fixed {
                    return false;
                }
                if fixed {
                    let local = Integer::from(cell.lower()[axis]);
                    let power = if self.sector[axis] {
                        local + Integer::one()
                    } else {
                        -local
                    };
                    if self.fixed[axis].is_some_and(|value| power != Integer::from(value)) {
                        return false;
                    }
                    singleton.push((self.indices[axis], power));
                }
            }
            self.equations.iter().all(|equation| {
                let mut restricted = equation.clone();
                for (index, value) in &singleton {
                    restricted = restricted.replace(*index, value);
                }
                restricted.is_zero()
            })
        }))
        .unwrap_or(false)
    }
}

#[cfg(test)]
#[path = "box_containment/tests.rs"]
mod tests;
