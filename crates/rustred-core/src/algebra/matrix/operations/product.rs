//! Authenticated two- and three-matrix products through Symbolica.

use crate::algebra::matrix::admission::{
    authenticate_native, check_limit, checked_add, checked_shape, increment_session_counter,
    inspect_rows, matrix_from_rows, native_into_rows, product_operation_bound,
};
use crate::algebra::matrix::field::{CheckedCoefficientField, call_native};
use crate::algebra::matrix::{
    SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixLimits,
    SymbolicaCoefficientMatrixStats,
};
use crate::algebra::{Coefficient, CoefficientContext};

/// Multiply two authenticated matrices through Symbolica.
pub(crate) fn multiply_coefficient_matrices(
    context: &CoefficientContext,
    left: &[Vec<Coefficient>],
    right: &[Vec<Coefficient>],
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<(Vec<Vec<Coefficient>>, SymbolicaCoefficientMatrixStats), SymbolicaCoefficientMatrixError>
{
    let left_shape = inspect_rows(left)?;
    let right_shape = inspect_rows(right)?;
    if left_shape.columns != right_shape.rows {
        return Err(SymbolicaCoefficientMatrixError::ShapeMismatch {
            left_rows: left_shape.rows,
            left_columns: left_shape.columns,
            right_rows: right_shape.rows,
            right_columns: right_shape.columns,
        });
    }
    let output_shape = checked_shape(left_shape.rows, right_shape.columns)?;
    let single_entries = left_shape
        .entries
        .max(right_shape.entries)
        .max(output_shape.entries);
    let live_entries = checked_add(
        "live Symbolica matrix entries",
        checked_add(
            "live Symbolica matrix entries",
            left_shape.entries,
            right_shape.entries,
        )?,
        output_shape.entries,
    )?;
    check_limit(
        "single Symbolica matrix entries",
        single_entries,
        limits.max_single_matrix_entries,
    )?;
    check_limit(
        "live Symbolica matrix entries",
        live_entries,
        limits.max_live_matrix_entries,
    )?;
    let operations =
        product_operation_bound(left_shape.rows, left_shape.columns, right_shape.columns)?;
    check_limit(
        "Symbolica coefficient matrix exact operations",
        operations,
        limits.max_exact_operations,
    )?;
    let field =
        CheckedCoefficientField::new(context, limits, single_entries, live_entries, operations);
    let state = field.state.clone();
    let left = matrix_from_rows(left, left_shape, field.clone())?;
    let right = matrix_from_rows(right, right_shape, field)?;
    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let product = call_native("product", || &left * &right)?;
    if product.nrows() != output_shape.rows || product.ncols() != output_shape.columns {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "product",
        });
    }
    authenticate_native(context, &product, limits, &state)?;
    let product = native_into_rows(product, &state)?;
    let stats = state.borrow().stats;
    Ok((product, stats))
}

/// Multiply three authenticated coefficient matrices in one native session.
/// The intermediate product is authenticated before it is consumed, while
/// RustRed owns only shape/resource policy and result transport.
pub(crate) fn multiply_three_coefficient_matrices(
    context: &CoefficientContext,
    left: &[Vec<Coefficient>],
    middle: &[Vec<Coefficient>],
    right: &[Vec<Coefficient>],
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<(Vec<Vec<Coefficient>>, SymbolicaCoefficientMatrixStats), SymbolicaCoefficientMatrixError>
{
    let left_shape = inspect_rows(left)?;
    let middle_shape = inspect_rows(middle)?;
    let right_shape = inspect_rows(right)?;
    if left_shape.columns != middle_shape.rows {
        return Err(SymbolicaCoefficientMatrixError::ShapeMismatch {
            left_rows: left_shape.rows,
            left_columns: left_shape.columns,
            right_rows: middle_shape.rows,
            right_columns: middle_shape.columns,
        });
    }
    if middle_shape.columns != right_shape.rows {
        return Err(SymbolicaCoefficientMatrixError::ShapeMismatch {
            left_rows: middle_shape.rows,
            left_columns: middle_shape.columns,
            right_rows: right_shape.rows,
            right_columns: right_shape.columns,
        });
    }

    let intermediate_shape = checked_shape(left_shape.rows, middle_shape.columns)?;
    let output_shape = checked_shape(left_shape.rows, right_shape.columns)?;
    let single_entries = left_shape
        .entries
        .max(middle_shape.entries)
        .max(right_shape.entries)
        .max(intermediate_shape.entries)
        .max(output_shape.entries);
    let first_live_entries = checked_add(
        "live Symbolica matrix entries",
        checked_add(
            "live Symbolica matrix entries",
            checked_add(
                "live Symbolica matrix entries",
                left_shape.entries,
                middle_shape.entries,
            )?,
            right_shape.entries,
        )?,
        intermediate_shape.entries,
    )?;
    let second_live_entries = checked_add(
        "live Symbolica matrix entries",
        checked_add(
            "live Symbolica matrix entries",
            right_shape.entries,
            intermediate_shape.entries,
        )?,
        output_shape.entries,
    )?;
    let live_entries = first_live_entries.max(second_live_entries);
    check_limit(
        "single Symbolica matrix entries",
        single_entries,
        limits.max_single_matrix_entries,
    )?;
    check_limit(
        "live Symbolica matrix entries",
        live_entries,
        limits.max_live_matrix_entries,
    )?;
    let operations = checked_add(
        "Symbolica coefficient matrix exact operations",
        product_operation_bound(left_shape.rows, left_shape.columns, middle_shape.columns)?,
        product_operation_bound(
            intermediate_shape.rows,
            intermediate_shape.columns,
            right_shape.columns,
        )?,
    )?;
    check_limit(
        "Symbolica coefficient matrix exact operations",
        operations,
        limits.max_exact_operations,
    )?;

    let field =
        CheckedCoefficientField::new(context, limits, single_entries, live_entries, operations);
    let state = field.state.clone();
    let left = matrix_from_rows(left, left_shape, field.clone())?;
    let middle = matrix_from_rows(middle, middle_shape, field.clone())?;
    let right = matrix_from_rows(right, right_shape, field)?;

    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let intermediate = call_native("first three-matrix product", || &left * &middle)?;
    if intermediate.nrows() != intermediate_shape.rows
        || intermediate.ncols() != intermediate_shape.columns
    {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "first three-matrix product",
        });
    }
    authenticate_native(context, &intermediate, limits, &state)?;
    drop(left);
    drop(middle);

    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let product = call_native("second three-matrix product", || &intermediate * &right)?;
    if product.nrows() != output_shape.rows || product.ncols() != output_shape.columns {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "second three-matrix product",
        });
    }
    authenticate_native(context, &product, limits, &state)?;
    let product = native_into_rows(product, &state)?;
    let stats = state.borrow().stats;
    Ok((product, stats))
}
