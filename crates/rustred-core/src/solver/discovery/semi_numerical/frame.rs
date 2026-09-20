//! Immutable source layout shared by modular probes of one exact frame.
//!
//! Cache index lookup and Symbolica's variable-map conversion, not algebra.
//! Polynomial evaluation and sparse elimination remain native Symbolica calls.

use symbolica::domains::finite_field::{ToFiniteField, Zp64};
use symbolica::domains::{Field, Ring};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::Coefficient;

use super::{
    ExactRow, Fp, FrameVariables, Integral, IntegralOrder, MaterializationError, TargetImage,
};

pub(super) struct ProbeFrame {
    rows: Vec<Vec<(u32, Coefficient)>>,
    columns_with_sentinel: u32,
}

impl ProbeFrame {
    /// Prepared immutable coefficients and physical column IDs. This is layout
    /// access, not an alternative arithmetic implementation.
    pub(super) fn rows(&self) -> &[Vec<(u32, Coefficient)>] {
        &self.rows
    }

    pub(super) fn columns_with_sentinel(&self) -> u32 {
        self.columns_with_sentinel
    }

    pub(super) fn new<const N: usize>(
        rows: &[ExactRow<N>],
        columns: &[Integral<N>],
        order: &IntegralOrder<N>,
        variables: &FrameVariables,
    ) -> Result<Self, MaterializationError> {
        let columns_with_sentinel = u32::try_from(columns.len())
            .ok()
            .and_then(|count| count.checked_add(1))
            .ok_or(MaterializationError::TooManyColumns)?;
        let rows = rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|term| {
                        let column = columns
                            .binary_search_by(|candidate| order.compare(candidate, &term.integral))
                            .map_err(|_| MaterializationError::TargetAbsent)?;
                        Ok((column as u32, variables.map_coefficient(&term.coefficient)?))
                    })
                    .collect::<Result<_, MaterializationError>>()
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            rows,
            columns_with_sentinel,
        })
    }

    pub(super) fn target_row(
        &self,
        target_column: usize,
        field: &Zp64,
        point: &[Fp],
    ) -> Option<TargetImage> {
        let target = u32::try_from(target_column).ok()?;
        if target >= self.columns_with_sentinel - 1 {
            return None;
        }
        let mut reducer =
            SparseRowReducer::new(self.columns_with_sentinel, field.clone(), LuLMode::None);
        let mut values = Vec::new();
        let mut ids = Vec::new();
        let mut pivots = Vec::new();
        for row in &self.rows {
            values.clear();
            ids.clear();
            for (column, coefficient) in row {
                let numerator = coefficient.numerator.evaluate_with_coeff_map(
                    |value| value.to_finite_field(field),
                    point,
                    field,
                );
                let denominator = coefficient.denominator.evaluate_with_coeff_map(
                    |value| value.to_finite_field(field),
                    point,
                    field,
                );
                if field.is_zero(&denominator) {
                    return None;
                }
                let value = field.div(&numerator, &denominator);
                if !field.is_zero(&value) {
                    ids.push(*column);
                    values.push(value);
                }
            }
            let pivot = reducer.add_row(&values, &ids);
            pivots.push(pivot);
            if pivot == Some(target) {
                let upper = reducer.u();
                let row = upper.nrows() as usize - 1;
                let start = upper.row_ptrs()[row];
                let end = upper.row_ptrs()[row + 1];
                return Some(TargetImage {
                    row: upper.col_idcs()[start..end]
                        .iter()
                        .copied()
                        .zip(upper.values()[start..end].iter().copied())
                        .collect(),
                    pivots,
                });
            }
        }
        None
    }
}
