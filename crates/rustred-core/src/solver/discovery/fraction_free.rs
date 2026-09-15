//! Opt-in native dense polynomial elimination of a pruned exact source frame.
//!
//! This is a bounded diagnostic alternative, not a custom elimination engine.
//! Constant rational coefficients use native Q-polynomials; genuinely rational
//! functions are rejected. Integer-only frames retain the native Z lane.

use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::domains::{EuclideanDomain, Ring};
use symbolica::poly::gcd::PolynomialGCD;
use symbolica::poly::polynomial::{MultivariatePolynomial, PolynomialRing};
use symbolica::prelude::{IntegerRing, Q, Rational, Z};
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
    observe: impl FnMut(MaterializationEvent<N>),
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
    // Fail before dense allocation. These are polynomials over Z or Q, not
    // rational functions in the variables. No source/domain condition is
    // discarded, and no row denominator clearing is performed.
    let mut rational_coefficients = false;
    for (row, source) in rows.iter().enumerate() {
        for (term, entry) in source.iter().enumerate() {
            let denominator = &entry.coefficient.denominator;
            if denominator.is_zero() || !denominator.is_constant() {
                return Err(MaterializationError::FractionFreeNonPolynomialCoefficient {
                    row: row + 1,
                    term: term + 1,
                });
            }
            rational_coefficients |= !denominator.is_one();
        }
    }
    let shape = (row_count, column_count);
    if rational_coefficients {
        let matrix = matrix_from_rows(rows, columns, order, variables, shape, Q, |coefficient| {
            let denominator = &coefficient.denominator.coefficients[0];
            coefficient.numerator.map_coeff(
                |value| Rational::from((value.clone(), denominator.clone())),
                Q,
            )
        })?;
        target_from_matrix(matrix, columns, target_column, variables, true, observe)
    } else {
        let matrix = matrix_from_rows(rows, columns, order, variables, shape, Z, |coefficient| {
            coefficient.numerator
        })?;
        target_from_matrix(matrix, columns, target_column, variables, false, observe)
    }
}

/// Remap once, without altering the physical columns or source-row order.
fn matrix_from_rows<const N: usize, R: Ring>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    variables: &FrameVariables,
    shape: (u32, u32),
    ring: R,
    convert: impl Fn(Coefficient) -> MultivariatePolynomial<R, u16>,
) -> Result<Matrix<PolynomialRing<R>>, MaterializationError> {
    let mut matrix = Matrix::new(shape.0, shape.1, PolynomialRing::new(ring));
    for (row, source) in rows.iter().enumerate() {
        for entry in source {
            if !entry.coefficient.is_zero() {
                let column = columns
                    .binary_search_by(|column| order.compare(column, &entry.integral))
                    .expect("common exact-frame column registry contains every source term");
                matrix[(row as u32, column as u32)] =
                    convert(variables.map_coefficient(&entry.coefficient)?);
            }
        }
    }
    Ok(matrix)
}

fn target_from_matrix<const N: usize, R>(
    mut matrix: Matrix<PolynomialRing<R>>,
    columns: &[Integral<N>],
    target_column: usize,
    variables: &FrameVariables,
    rational_coefficients: bool,
    mut observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError>
where
    R: EuclideanDomain + PolynomialGCD<u16>,
    Coefficient: FromNumeratorAndDenominator<R, IntegerRing, u16>,
{
    let reduction_columns = target_column + 1;
    observe(MaterializationEvent::DenseFractionFreeStarted {
        rows: matrix.nrows(),
        columns: columns.len(),
        reduction_columns,
        rational_coefficients,
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
