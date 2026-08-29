//! Dense matrix shape validation and conversion to Symbolica dimensions.

use crate::algebra::Coefficient;
use crate::algebra::matrix::SymbolicaCoefficientMatrixError;

#[derive(Clone, Copy, Debug)]
pub(in crate::algebra::matrix) struct MatrixShape {
    pub(in crate::algebra::matrix) rows: usize,
    pub(in crate::algebra::matrix) columns: usize,
    pub(in crate::algebra::matrix) rows_u32: u32,
    pub(in crate::algebra::matrix) columns_u32: u32,
    pub(in crate::algebra::matrix) entries: usize,
}

pub(in crate::algebra::matrix) fn inspect_rows<Row>(
    rows: &[Row],
) -> Result<MatrixShape, SymbolicaCoefficientMatrixError>
where
    Row: AsRef<[Coefficient]>,
{
    if rows.is_empty() {
        return Err(SymbolicaCoefficientMatrixError::EmptyMatrix);
    }
    let columns = rows[0].as_ref().len();
    if columns == 0 {
        return Err(SymbolicaCoefficientMatrixError::EmptyMatrix);
    }
    if let Some((row, actual_columns)) = rows.iter().enumerate().find_map(|(row, values)| {
        let actual_columns = values.as_ref().len();
        (actual_columns != columns).then_some((row, actual_columns))
    }) {
        return Err(SymbolicaCoefficientMatrixError::RaggedMatrix {
            row,
            expected_columns: columns,
            actual_columns,
        });
    }
    checked_shape(rows.len(), columns)
}

pub(in crate::algebra::matrix) fn checked_shape(
    rows: usize,
    columns: usize,
) -> Result<MatrixShape, SymbolicaCoefficientMatrixError> {
    let rows_u32 = u32::try_from(rows)
        .map_err(|_| SymbolicaCoefficientMatrixError::DimensionOverflow { rows, columns })?;
    let columns_u32 = u32::try_from(columns)
        .map_err(|_| SymbolicaCoefficientMatrixError::DimensionOverflow { rows, columns })?;
    let entries = rows.checked_mul(columns).ok_or(
        SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "coefficient matrix entries",
        },
    )?;
    rows_u32
        .checked_mul(columns_u32)
        .ok_or(SymbolicaCoefficientMatrixError::DimensionOverflow { rows, columns })?;
    Ok(MatrixShape {
        rows,
        columns,
        rows_u32,
        columns_u32,
        entries,
    })
}

pub(in crate::algebra::matrix) fn require_square(
    shape: MatrixShape,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    if shape.rows == shape.columns {
        Ok(shape.rows)
    } else {
        Err(SymbolicaCoefficientMatrixError::NotSquare {
            rows: shape.rows,
            columns: shape.columns,
        })
    }
}
