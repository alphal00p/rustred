//! Authentication and construction of caller-owned matrix inputs.

use symbolica::prelude::Matrix;

use crate::algebra::matrix::SymbolicaCoefficientMatrixError;
use crate::algebra::matrix::field::{CheckedCoefficientField, call_native};
use crate::algebra::{
    Coefficient, CoefficientContext, ExactAlgebraLimits,
    coefficient_clone_owned_retained_byte_bound,
};

use super::bounds::{check_limit, checked_add};
use super::shape::MatrixShape;

fn validate_rows<Row>(
    context: &CoefficientContext,
    rows: &[Row],
    limits: ExactAlgebraLimits,
) -> Result<(), SymbolicaCoefficientMatrixError>
where
    Row: AsRef<[Coefficient]>,
{
    for (row, values) in rows.iter().enumerate() {
        for (column, coefficient) in values.as_ref().iter().enumerate() {
            context
                .validate_with_limits(coefficient, limits)
                .map_err(
                    |error| SymbolicaCoefficientMatrixError::InvalidCoefficient {
                        row,
                        column,
                        error,
                    },
                )?;
        }
    }
    Ok(())
}

pub(in crate::algebra::matrix) fn coefficient_retained_bytes(
    coefficient: &Coefficient,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    coefficient_clone_owned_retained_byte_bound(coefficient).ok_or(
        SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "coefficient matrix retained bytes",
        },
    )
}

fn rows_retained_bytes<Row>(rows: &[Row]) -> Result<usize, SymbolicaCoefficientMatrixError>
where
    Row: AsRef<[Coefficient]>,
{
    let mut bytes = 0usize;
    for row in rows {
        for coefficient in row.as_ref() {
            bytes = checked_add(
                "coefficient matrix input retained bytes",
                bytes,
                coefficient_retained_bytes(coefficient)?,
            )?;
        }
    }
    Ok(bytes)
}

pub(in crate::algebra::matrix) fn matrix_from_rows<'context, Row>(
    rows: &[Row],
    shape: MatrixShape,
    field: CheckedCoefficientField<'context>,
) -> Result<Matrix<CheckedCoefficientField<'context>>, SymbolicaCoefficientMatrixError>
where
    Row: AsRef<[Coefficient]>,
{
    validate_rows(field.context, rows, field.limits.exact_algebra)?;
    let retained_bytes = rows_retained_bytes(rows)?;
    {
        let mut state = field.state.borrow_mut();
        let prospective_bytes = checked_add(
            "coefficient matrix input retained bytes",
            state.stats.input_retained_bytes,
            retained_bytes,
        )?;
        check_limit(
            "coefficient matrix input retained bytes",
            prospective_bytes,
            field.limits.max_input_retained_bytes,
        )?;
        state.stats.input_retained_bytes = prospective_bytes;
        state.stats.input_entries = checked_add(
            "coefficient matrix input entries",
            state.stats.input_entries,
            shape.entries,
        )?;
    }
    let mut data = Vec::new();
    data.try_reserve_exact(shape.entries).map_err(|_| {
        SymbolicaCoefficientMatrixError::AllocationFailure {
            resource: "coefficient matrix entries",
            requested: shape.entries,
        }
    })?;
    for row in rows {
        data.extend(row.as_ref().iter().cloned());
    }
    call_native("construction", || {
        Matrix::from_linear(data, shape.rows_u32, shape.columns_u32, field)
    })?
    .map_err(|_| SymbolicaCoefficientMatrixError::InternalShapeFailure {
        operation: "construction",
    })
}
