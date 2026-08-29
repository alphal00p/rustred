//! Native `T M T^T` congruence through Symbolica.

use crate::algebra::matrix::admission::{
    authenticate_native, check_limit, checked_add, checked_mul, checked_shape,
    increment_session_counter, inspect_rows, matrix_from_rows, native_into_rows,
    product_operation_bound,
};
use crate::algebra::matrix::field::{CheckedCoefficientField, call_native};
use crate::algebra::matrix::{
    SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixLimits,
    SymbolicaCoefficientMatrixStats,
};
use crate::algebra::{Coefficient, CoefficientContext};

/// Compute `transform * middle * transform^T` through Symbolica's native
/// transpose and matrix products in one authenticated session.
///
/// Keeping the transpose inside this boundary prevents callers from growing a
/// second, handwritten standard-matrix implementation merely to form a
/// congruence. The two native product outputs are both authenticated and
/// charged to the output-byte census before the final matrix is returned.
pub(crate) fn congruence_of_coefficient_matrix(
    context: &CoefficientContext,
    transform: &[Vec<Coefficient>],
    middle: &[Vec<Coefficient>],
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<(Vec<Vec<Coefficient>>, SymbolicaCoefficientMatrixStats), SymbolicaCoefficientMatrixError>
{
    let transform_shape = inspect_rows(transform)?;
    let middle_shape = inspect_rows(middle)?;
    if transform_shape.columns != middle_shape.rows
        || middle_shape.columns != transform_shape.columns
    {
        return Err(SymbolicaCoefficientMatrixError::ShapeMismatch {
            left_rows: transform_shape.rows,
            left_columns: transform_shape.columns,
            right_rows: middle_shape.rows,
            right_columns: middle_shape.columns,
        });
    }

    let intermediate_shape = checked_shape(transform_shape.rows, middle_shape.columns)?;
    let output_shape = checked_shape(transform_shape.rows, transform_shape.rows)?;
    let single_entries = transform_shape
        .entries
        .max(middle_shape.entries)
        .max(intermediate_shape.entries)
        .max(output_shape.entries);
    let first_live_entries = checked_add(
        "live Symbolica matrix entries",
        checked_add(
            "live Symbolica matrix entries",
            checked_mul("live Symbolica matrix entries", 2, transform_shape.entries)?,
            middle_shape.entries,
        )?,
        intermediate_shape.entries,
    )?;
    let second_live_entries = checked_add(
        "live Symbolica matrix entries",
        checked_add(
            "live Symbolica matrix entries",
            transform_shape.entries,
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
        product_operation_bound(
            transform_shape.rows,
            transform_shape.columns,
            middle_shape.columns,
        )?,
        product_operation_bound(
            intermediate_shape.rows,
            intermediate_shape.columns,
            transform_shape.rows,
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
    let transform = matrix_from_rows(transform, transform_shape, field.clone())?;
    let middle = matrix_from_rows(middle, middle_shape, field)?;
    increment_session_counter(&state, "Symbolica matrix transpose calls", |stats| {
        &mut stats.transpose_calls
    })?;
    let transposed = call_native("congruence transpose", || transform.transpose())?;
    if transposed.nrows() != transform_shape.columns || transposed.ncols() != transform_shape.rows {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "congruence transpose",
        });
    }
    authenticate_native(context, &transposed, limits, &state)?;

    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let intermediate = call_native("left congruence product", || &transform * &middle)?;
    if intermediate.nrows() != intermediate_shape.rows
        || intermediate.ncols() != intermediate_shape.columns
    {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "left congruence product",
        });
    }
    authenticate_native(context, &intermediate, limits, &state)?;
    drop(transform);
    drop(middle);

    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let product = call_native("right congruence product", || &intermediate * &transposed)?;
    if product.nrows() != output_shape.rows || product.ncols() != output_shape.columns {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "right congruence product",
        });
    }
    authenticate_native(context, &product, limits, &state)?;
    let product = native_into_rows(product, &state)?;
    let stats = state.borrow().stats;
    Ok((product, stats))
}
