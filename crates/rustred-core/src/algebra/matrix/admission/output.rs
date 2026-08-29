//! Authentication and conversion of Symbolica-owned matrix outputs.

use std::cell::RefCell;
use std::rc::Rc;

use symbolica::prelude::{Matrix, Ring};

use crate::algebra::matrix::field::CheckedFieldState;
use crate::algebra::matrix::{SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixLimits};
use crate::algebra::{Coefficient, CoefficientContext};

use super::bounds::{check_limit, checked_add};
use super::input::coefficient_retained_bytes;

pub(in crate::algebra::matrix) fn authenticate_output_coefficient(
    context: &CoefficientContext,
    coefficient: &Coefficient,
    limits: SymbolicaCoefficientMatrixLimits,
    state: &Rc<RefCell<CheckedFieldState>>,
) -> Result<(), SymbolicaCoefficientMatrixError> {
    context
        .validate_with_limits(coefficient, limits.exact_algebra)
        .map_err(SymbolicaCoefficientMatrixError::ExactAlgebra)?;
    let bytes = coefficient_retained_bytes(coefficient)?;
    let mut state = state.borrow_mut();
    let prospective = checked_add(
        "coefficient matrix output retained bytes",
        state.stats.output_retained_bytes,
        bytes,
    )?;
    check_limit(
        "coefficient matrix output retained bytes",
        prospective,
        limits.max_output_retained_bytes,
    )?;
    state.stats.output_retained_bytes = prospective;
    state.stats.authenticated_entries = checked_add(
        "authenticated Symbolica matrix entries",
        state.stats.authenticated_entries,
        1,
    )?;
    Ok(())
}

pub(in crate::algebra::matrix) fn authenticate_native<F>(
    context: &CoefficientContext,
    matrix: &Matrix<F>,
    limits: SymbolicaCoefficientMatrixLimits,
    state: &Rc<RefCell<CheckedFieldState>>,
) -> Result<(), SymbolicaCoefficientMatrixError>
where
    F: Ring<Element = Coefficient>,
{
    let mut retained_bytes = 0usize;
    for (offset, coefficient) in matrix.iter().enumerate() {
        let columns = matrix.ncols();
        context
            .validate_with_limits(coefficient, limits.exact_algebra)
            .map_err(
                |error| SymbolicaCoefficientMatrixError::InvalidCoefficient {
                    row: offset / columns,
                    column: offset % columns,
                    error,
                },
            )?;
        retained_bytes = checked_add(
            "coefficient matrix output retained bytes",
            retained_bytes,
            coefficient_retained_bytes(coefficient)?,
        )?;
    }
    let count = matrix.nrows().checked_mul(matrix.ncols()).ok_or(
        SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "authenticated Symbolica matrix entries",
        },
    )?;
    let mut state = state.borrow_mut();
    let prospective_bytes = checked_add(
        "coefficient matrix output retained bytes",
        state.stats.output_retained_bytes,
        retained_bytes,
    )?;
    check_limit(
        "coefficient matrix output retained bytes",
        prospective_bytes,
        limits.max_output_retained_bytes,
    )?;
    state.stats.output_retained_bytes = prospective_bytes;
    state.stats.authenticated_entries = checked_add(
        "authenticated Symbolica matrix entries",
        state.stats.authenticated_entries,
        count,
    )?;
    Ok(())
}

pub(in crate::algebra::matrix) fn native_into_rows<F>(
    matrix: Matrix<F>,
    state: &Rc<RefCell<CheckedFieldState>>,
) -> Result<Vec<Vec<Coefficient>>, SymbolicaCoefficientMatrixError>
where
    F: Ring<Element = Coefficient>,
{
    let rows = matrix.nrows();
    let columns = matrix.ncols();
    let entries = rows.checked_mul(columns).ok_or(
        SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "coefficient matrix output entries",
        },
    )?;
    let mut output = Vec::new();
    output.try_reserve_exact(rows).map_err(|_| {
        SymbolicaCoefficientMatrixError::AllocationFailure {
            resource: "coefficient matrix output rows",
            requested: rows,
        }
    })?;
    let mut data = matrix.into_vec().into_iter();
    for _ in 0..rows {
        let mut row = Vec::new();
        row.try_reserve_exact(columns).map_err(|_| {
            SymbolicaCoefficientMatrixError::AllocationFailure {
                resource: "coefficient matrix output entries",
                requested: columns,
            }
        })?;
        row.extend(data.by_ref().take(columns));
        output.push(row);
    }
    if data.next().is_some() || output.iter().map(Vec::len).sum::<usize>() != entries {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "output conversion",
        });
    }
    let mut state = state.borrow_mut();
    state.stats.output_entries = checked_add(
        "coefficient matrix output entries",
        state.stats.output_entries,
        entries,
    )?;
    Ok(output)
}
