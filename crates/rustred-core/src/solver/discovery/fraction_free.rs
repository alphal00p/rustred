//! Opt-in native dense polynomial elimination of a pruned exact source frame.
//!
//! This is a bounded diagnostic alternative, not a custom elimination engine.
//! It deliberately rejects rational input coefficients in the first pilot.

use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::poly::polynomial::PolynomialRing;
use symbolica::prelude::Z;
use symbolica::tensors::matrix::Matrix;

use crate::algebra::Coefficient;

use super::variables::FrameVariables;
use super::{ExactRow, Integral, IntegralOrder, MaterializationError, MaterializationEvent, Term};

pub(super) fn materialize<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    max_matrix_entries: usize,
    mut observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    // The common physical-column validation runs before this private helper.
    // Native partial reduction indexes row zero; never call it on an empty frame.
    if rows.is_empty() || columns.is_empty() || target_column >= columns.len() {
        return Err(MaterializationError::TargetAbsent);
    }
    let row_count = u32::try_from(rows.len())
        .map_err(|_| MaterializationError::FractionFreeDimensionOverflow)?;
    let column_count = u32::try_from(columns.len())
        .map_err(|_| MaterializationError::FractionFreeDimensionOverflow)?;
    let entries = rows
        .len()
        .checked_mul(columns.len())
        .ok_or(MaterializationError::FractionFreeDimensionOverflow)?;
    if entries > max_matrix_entries {
        return Err(MaterializationError::FractionFreeMatrixBudget {
            rows: rows.len(),
            columns: columns.len(),
            limit: max_matrix_entries,
        });
    }
    // Fail before dense allocation. Inherited source/domain conditions are not
    // discarded or replaced by denominator clearing in this controlled pilot.
    for (row, source) in rows.iter().enumerate() {
        for (term, entry) in source.iter().enumerate() {
            if !entry.coefficient.denominator.is_one() {
                return Err(MaterializationError::FractionFreeNonPolynomialCoefficient {
                    row: row + 1,
                    term: term + 1,
                });
            }
        }
    }
    let mut matrix = Matrix::new(row_count, column_count, PolynomialRing::new(Z));
    for (row, source) in rows.iter().enumerate() {
        for entry in source {
            if !entry.coefficient.is_zero() {
                let column = columns
                    .binary_search_by(|column| order.compare(column, &entry.integral))
                    .expect("common exact-frame column registry contains every source term");
                matrix[(row as u32, column as u32)] =
                    variables.map_coefficient(&entry.coefficient)?.numerator;
            }
        }
    }
    let reduction_columns = target_column + 1;
    observe(MaterializationEvent::DenseFractionFreeStarted {
        rows: rows.len(),
        columns: columns.len(),
        reduction_columns,
    });
    // All columns are retained, including the complete RHS tail. No dense back
    // substitution or generic polynomial-only solve is needed for a target row.
    let rank = matrix.partial_row_reduce_fraction_free(reduction_columns as u32);
    observe(MaterializationEvent::DenseFractionFreeFinished {
        rank: rank as usize,
    });
    for row in matrix.row_iter().take(rank as usize) {
        if row.iter().position(|value| !value.is_zero()) != Some(target_column) {
            continue;
        }
        let pivot = &row[target_column];
        return row
            .iter()
            .enumerate()
            .filter(|(_, value)| !value.is_zero())
            .map(|(column, value)| {
                // Normalize once, with native exact rational-polynomial CAS.
                // Any resulting pivot denominator reaches the ordinary guard
                // extractor; original-source replay remains a separate gate.
                let coefficient = Coefficient::from_num_den(value.clone(), pivot.clone(), &Z, true);
                Ok(Term {
                    integral: columns[column],
                    coefficient: variables.restore_coefficient(&coefficient)?,
                })
            })
            .collect();
    }
    Err(MaterializationError::TargetNotPivot)
}

#[cfg(test)]
mod tests;
