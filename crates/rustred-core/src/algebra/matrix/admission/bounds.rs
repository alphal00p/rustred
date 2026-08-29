//! Checked resource arithmetic and native-operation upper bounds.

use crate::algebra::matrix::SymbolicaCoefficientMatrixError;

use super::shape::checked_shape;

pub(in crate::algebra::matrix) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    left.checked_add(right)
        .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow { resource })
}

pub(in crate::algebra::matrix) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    left.checked_mul(right)
        .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow { resource })
}

pub(in crate::algebra::matrix) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SymbolicaCoefficientMatrixError> {
    if requested > limit {
        Err(SymbolicaCoefficientMatrixError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn square_sum(bound: usize) -> Result<usize, SymbolicaCoefficientMatrixError> {
    let a = bound;
    let b = bound
        .checked_add(1)
        .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "Symbolica determinant operation bound",
        })?;
    let c = bound
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "Symbolica determinant operation bound",
        })?;
    // Cancel 2 and 3 before multiplying to avoid rejecting representable sums.
    let mut factors = [a, b, c];
    let even = factors.iter().position(|value| value % 2 == 0).unwrap_or(0);
    factors[even] /= 2;
    let by_three = factors.iter().position(|value| value % 3 == 0).unwrap_or(0);
    factors[by_three] /= 3;
    checked_mul(
        "Symbolica determinant operation bound",
        checked_mul(
            "Symbolica determinant operation bound",
            factors[0],
            factors[1],
        )?,
        factors[2],
    )
}

pub(in crate::algebra::matrix) fn determinant_operation_bound(
    size: usize,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    match size {
        0 => Ok(0),
        1 => Ok(0),
        2 => Ok(3),
        3 => Ok(14),
        _ => {
            let cells = square_sum(size - 1)?;
            let divisions = square_sum(size - 2)?;
            checked_add(
                "Symbolica determinant operation bound",
                checked_add(
                    "Symbolica determinant operation bound",
                    checked_mul("Symbolica determinant operation bound", 3, cells)?,
                    divisions,
                )?,
                1,
            )
        }
    }
}

pub(in crate::algebra::matrix) fn inverse_operation_bound(
    size: usize,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    match size {
        0 => Ok(0),
        1 => Ok(4),
        2 => Ok(10),
        3 => Ok(42),
        _ => {
            let cube = checked_mul(
                "Symbolica inverse operation bound",
                checked_mul("Symbolica inverse operation bound", size, size)?,
                size,
            )?;
            let square = checked_mul("Symbolica inverse operation bound", size, size)?;
            let positive = checked_add(
                "Symbolica inverse operation bound",
                checked_mul("Symbolica inverse operation bound", 3, cube)?,
                checked_mul("Symbolica inverse operation bound", 3, size)?,
            )?;
            positive
                .checked_sub(checked_mul("Symbolica inverse operation bound", 2, square)?)
                .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow {
                    resource: "Symbolica inverse operation bound",
                })
        }
    }
}

pub(in crate::algebra::matrix) fn product_operation_bound(
    rows: usize,
    inner: usize,
    columns: usize,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    checked_mul(
        "Symbolica matrix product operation bound",
        2,
        checked_mul(
            "Symbolica matrix product operation bound",
            checked_mul("Symbolica matrix product operation bound", rows, inner)?,
            columns,
        )?,
    )
}

pub(in crate::algebra::matrix) fn square_representation_bounds(
    size: usize,
) -> Result<(usize, usize, usize), SymbolicaCoefficientMatrixError> {
    let shape = checked_shape(size, size)?;
    let doubled =
        size.checked_mul(2)
            .ok_or(SymbolicaCoefficientMatrixError::DimensionOverflow {
                rows: size,
                columns: size,
            })?;
    let doubled_u32 =
        u32::try_from(doubled).map_err(|_| SymbolicaCoefficientMatrixError::DimensionOverflow {
            rows: size,
            columns: doubled,
        })?;
    shape.rows_u32.checked_mul(doubled_u32).ok_or(
        SymbolicaCoefficientMatrixError::DimensionOverflow {
            rows: size,
            columns: doubled,
        },
    )?;
    let augmented = checked_mul("augmented Symbolica matrix entries", shape.entries, 2)?;
    let peak_live = checked_mul("live Symbolica matrix entries", shape.entries, 4)?;
    Ok((shape.entries, augmented, peak_live))
}
