use crate::algebra::Coefficient;
use crate::algebra::matrix::{determinant_of_coefficient_matrix, multiply_coefficient_matrices};
use crate::family::IntegralFamily;

use super::super::{CoefficientMatrix, Error};
use super::algebra::ReplayAlgebra;

/// Compute an exact determinant through Symbolica's public matrix API.
///
/// Symbolica 2.2.0 reports a `0 x 0` determinant as singular, while the empty
/// external-momentum map of a vacuum family has determinant one. That unique
/// structural case is handled before entering the native boundary.
pub(super) fn checked_determinant(
    matrix: &CoefficientMatrix,
    algebra: &mut ReplayAlgebra<'_>,
) -> Result<Coefficient, Error> {
    debug_assert_eq!(matrix.rows, matrix.columns);
    if matrix.rows == 0 {
        return Ok(algebra.context.one());
    }

    let rows = clone_matrix_rows(matrix, algebra, "determinant input rows")?;
    let limits = algebra.remaining_symbolica_limits()?;
    let (determinant, stats) =
        match determinant_of_coefficient_matrix(algebra.context, &rows, limits) {
            Ok(result) => result,
            Err(error) => return Err(algebra.map_symbolica_matrix_error(error)),
        };
    algebra.absorb_symbolica_stats(stats)?;
    Ok(determinant)
}

pub(super) fn clone_matrix_rows(
    matrix: &CoefficientMatrix,
    algebra: &mut ReplayAlgebra<'_>,
    resource: &'static str,
) -> Result<Vec<Vec<Coefficient>>, Error> {
    algebra.charge_entries(matrix.entries().len())?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(matrix.rows)
        .map_err(|_| Error::AllocationFailure {
            resource,
            requested: matrix.rows,
        })?;
    for row in 0..matrix.rows {
        let mut values = Vec::new();
        values
            .try_reserve_exact(matrix.columns)
            .map_err(|_| Error::AllocationFailure {
                resource,
                requested: matrix.columns,
            })?;
        values.extend((0..matrix.columns).map(|column| matrix.at(row, column).clone()));
        rows.push(values);
    }
    Ok(rows)
}

pub(super) fn clone_denominator_coefficient_rows(
    family: &IntegralFamily,
    algebra: &mut ReplayAlgebra<'_>,
    resource: &'static str,
) -> Result<Vec<Vec<Coefficient>>, Error> {
    let rows = family.denominator_count();
    let entries = rows
        .checked_mul(rows)
        .ok_or(Error::ResourceCountOverflow { resource })?;
    algebra.charge_entries(entries)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(rows)
        .map_err(|_| Error::AllocationFailure {
            resource,
            requested: rows,
        })?;
    for denominator in family.denominators() {
        let mut row = Vec::new();
        row.try_reserve_exact(rows)
            .map_err(|_| Error::AllocationFailure {
                resource,
                requested: rows,
            })?;
        row.extend(denominator.coefficients().iter().cloned());
        output.push(row);
    }
    Ok(output)
}

pub(super) fn clone_coefficient_column(
    values: &[Coefficient],
    algebra: &mut ReplayAlgebra<'_>,
    resource: &'static str,
) -> Result<Vec<Vec<Coefficient>>, Error> {
    algebra.charge_entries(values.len())?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(values.len())
        .map_err(|_| Error::AllocationFailure {
            resource,
            requested: values.len(),
        })?;
    for value in values {
        let mut row = Vec::new();
        row.try_reserve_exact(1)
            .map_err(|_| Error::AllocationFailure {
                resource,
                requested: 1,
            })?;
        row.push(value.clone());
        output.push(row);
    }
    Ok(output)
}

pub(super) fn clone_denominator_constant_column(
    family: &IntegralFamily,
    algebra: &mut ReplayAlgebra<'_>,
    resource: &'static str,
) -> Result<Vec<Vec<Coefficient>>, Error> {
    let rows = family.denominator_count();
    algebra.charge_entries(rows)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(rows)
        .map_err(|_| Error::AllocationFailure {
            resource,
            requested: rows,
        })?;
    for denominator in family.denominators() {
        let mut row = Vec::new();
        row.try_reserve_exact(1)
            .map_err(|_| Error::AllocationFailure {
                resource,
                requested: 1,
            })?;
        row.push(denominator.constant().clone());
        output.push(row);
    }
    Ok(output)
}

pub(super) fn native_product(
    algebra: &mut ReplayAlgebra<'_>,
    left: &[Vec<Coefficient>],
    right: &[Vec<Coefficient>],
) -> Result<Vec<Vec<Coefficient>>, Error> {
    let expected_rows = left.len();
    let expected_columns = right.first().map_or(0, Vec::len);
    let limits = algebra.remaining_symbolica_limits()?;
    let (product, stats) = match multiply_coefficient_matrices(algebra.context, left, right, limits)
    {
        Ok(result) => result,
        Err(error) => return Err(algebra.map_symbolica_matrix_error(error)),
    };
    algebra.absorb_symbolica_stats(stats)?;
    if product.len() != expected_rows || product.iter().any(|row| row.len() != expected_columns) {
        return Err(Error::InternalSymbolicaAlgebra {
            detail: "matrix product returned the wrong shape".to_owned(),
        });
    }
    Ok(product)
}
