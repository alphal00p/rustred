//! Authenticated native rank, determinant, inverse, and matrix products.

use std::cell::RefCell;
use std::rc::Rc;

use symbolica::domains::SelfRing;
use symbolica::prelude::Matrix;

use crate::algebra::{Coefficient, CoefficientContext};

use super::admission::{
    authenticate_native, authenticate_output_coefficient, check_limit, checked_add, checked_mul,
    checked_shape, determinant_operation_bound, increment_session_counter, inspect_rows,
    inverse_operation_bound, matrix_from_rows, native_into_rows, product_operation_bound,
    require_square, square_representation_bounds,
};
use super::field::{CheckedCoefficientField, CheckedFieldState, call_native, call_native_result};
use super::{
    SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixLimits,
    SymbolicaCoefficientMatrixStats, SymbolicaInverseSide, SymbolicaNativeMatrixErrorKind,
};

/// A determinant, inverse, and native two-sided replay certificate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VerifiedSymbolicaCoefficientInverse {
    pub(super) inverse: Vec<Vec<Coefficient>>,
    pub(super) determinant: Coefficient,
    pub(super) stats: SymbolicaCoefficientMatrixStats,
}

impl VerifiedSymbolicaCoefficientInverse {
    pub(crate) fn into_parts(
        self,
    ) -> (
        Vec<Vec<Coefficient>>,
        Coefficient,
        SymbolicaCoefficientMatrixStats,
    ) {
        (self.inverse, self.determinant, self.stats)
    }
}

fn verify_identity_product(
    context: &CoefficientContext,
    product: &Matrix<CheckedCoefficientField<'_>>,
    size: usize,
    side: SymbolicaInverseSide,
    limits: SymbolicaCoefficientMatrixLimits,
    state: &Rc<RefCell<CheckedFieldState>>,
) -> Result<(), SymbolicaCoefficientMatrixError> {
    if product.nrows() != size || product.ncols() != size {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "inverse verification",
        });
    }
    authenticate_native(context, product, limits, state)?;
    for row in 0..size {
        for column in 0..size {
            let coefficient = &product[(row as u32, column as u32)];
            let valid = if row == column {
                coefficient.is_one()
            } else {
                coefficient.is_zero()
            };
            if !valid {
                return Err(
                    SymbolicaCoefficientMatrixError::InverseVerificationFailure {
                        side,
                        row,
                        column,
                    },
                );
            }
        }
    }
    Ok(())
}

/// Compute the exact rank of a nonempty rectangular coefficient matrix through
/// Symbolica's destructive field row reduction.
///
/// Calling `Matrix::partial_row_reduce` on the owned native matrix avoids the
/// additional full clone performed by `Matrix::rank`.  RustRed does not select
/// pivots or perform elimination here: it only authenticates the input and
/// discarded echelon output, enforces the data-dependent exact-arithmetic cap,
/// and transports typed failures across Symbolica's infallible field traits.
pub(crate) fn rank_of_coefficient_matrix(
    context: &CoefficientContext,
    rows: &[Vec<Coefficient>],
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<(usize, SymbolicaCoefficientMatrixStats), SymbolicaCoefficientMatrixError> {
    let shape = inspect_rows(rows)?;
    check_limit(
        "single Symbolica matrix entries",
        shape.entries,
        limits.max_single_matrix_entries,
    )?;
    // The only native matrix is destructively reduced in place.  The borrowed
    // RustRed rows remain caller-owned and are charged independently as input.
    check_limit(
        "live Symbolica matrix entries",
        shape.entries,
        limits.max_live_matrix_entries,
    )?;

    let field = CheckedCoefficientField::new(
        context,
        limits,
        shape.entries,
        shape.entries,
        limits.max_exact_operations,
    );
    let state = field.state.clone();
    let max_column = shape.columns_u32;
    let mut matrix = matrix_from_rows(rows, shape, field)?;

    increment_session_counter(&state, "Symbolica rank calls", |stats| {
        &mut stats.rank_calls
    })?;
    let rank = call_native("rank", || matrix.partial_row_reduce(max_column))? as usize;
    if rank > shape.rows.min(shape.columns) {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure { operation: "rank" });
    }
    authenticate_native(context, &matrix, limits, &state)?;
    let stats = state.borrow().stats;
    Ok((rank, stats))
}

/// Compute a determinant with Symbolica after authenticating the full matrix.
pub(crate) fn determinant_of_coefficient_matrix(
    context: &CoefficientContext,
    rows: &[Vec<Coefficient>],
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<(Coefficient, SymbolicaCoefficientMatrixStats), SymbolicaCoefficientMatrixError> {
    let shape = inspect_rows(rows)?;
    let size = require_square(shape)?;
    let operations = determinant_operation_bound(size)?;
    check_limit(
        "Symbolica coefficient matrix exact operations",
        operations,
        limits.max_exact_operations,
    )?;
    let determinant_live = checked_mul("live Symbolica matrix entries", shape.entries, 2)?;
    check_limit(
        "single Symbolica matrix entries",
        shape.entries,
        limits.max_single_matrix_entries,
    )?;
    check_limit(
        "live Symbolica matrix entries",
        determinant_live,
        limits.max_live_matrix_entries,
    )?;
    let field =
        CheckedCoefficientField::new(context, limits, shape.entries, determinant_live, operations);
    let state = field.state.clone();
    let matrix = matrix_from_rows(rows, shape, field)?;
    increment_session_counter(&state, "Symbolica determinant calls", |stats| {
        &mut stats.determinant_calls
    })?;
    let determinant = call_native_result("determinant", || matrix.det())?;
    authenticate_output_coefficient(context, &determinant, limits, &state)?;
    let stats = state.borrow().stats;
    Ok((determinant, stats))
}

/// Compute and certify an exact inverse using only Symbolica matrix algebra.
pub(crate) fn invert_and_verify_coefficient_matrix(
    context: &CoefficientContext,
    rows: &[Vec<Coefficient>],
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<VerifiedSymbolicaCoefficientInverse, SymbolicaCoefficientMatrixError> {
    let shape = inspect_rows(rows)?;
    let size = require_square(shape)?;
    let (entries, augmented_entries, peak_live_entries) = square_representation_bounds(size)?;
    check_limit(
        "single Symbolica matrix entries",
        augmented_entries,
        limits.max_single_matrix_entries,
    )?;
    check_limit(
        "live Symbolica matrix entries",
        peak_live_entries,
        limits.max_live_matrix_entries,
    )?;
    let product = product_operation_bound(size, size, size)?;
    let operations = checked_add(
        "Symbolica coefficient matrix exact operations",
        checked_add(
            "Symbolica coefficient matrix exact operations",
            determinant_operation_bound(size)?,
            inverse_operation_bound(size)?,
        )?,
        checked_mul("Symbolica coefficient matrix exact operations", 2, product)?,
    )?;
    check_limit(
        "Symbolica coefficient matrix exact operations",
        operations,
        limits.max_exact_operations,
    )?;

    let field = CheckedCoefficientField::new(
        context,
        limits,
        augmented_entries,
        peak_live_entries,
        operations,
    );
    let state = field.state.clone();
    let matrix = matrix_from_rows(rows, shape, field)?;

    increment_session_counter(&state, "Symbolica determinant calls", |stats| {
        &mut stats.determinant_calls
    })?;
    let determinant = call_native_result("inverse determinant guard", || matrix.det())?;
    authenticate_output_coefficient(context, &determinant, limits, &state)?;
    if determinant.is_zero() {
        return Err(SymbolicaCoefficientMatrixError::Singular);
    }

    increment_session_counter(&state, "Symbolica inverse calls", |stats| {
        &mut stats.inverse_calls
    })?;
    let inverse = match call_native_result("inverse", || matrix.inv()) {
        Err(SymbolicaCoefficientMatrixError::NativeError {
            kind: SymbolicaNativeMatrixErrorKind::Singular,
            ..
        }) => {
            return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
                operation: "inverse after nonzero determinant",
            });
        }
        result => result?,
    };
    if inverse.nrows() != size || inverse.ncols() != size {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "inverse",
        });
    }
    authenticate_native(context, &inverse, limits, &state)?;

    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let left = call_native("left inverse product", || &matrix * &inverse)?;
    verify_identity_product(
        context,
        &left,
        size,
        SymbolicaInverseSide::MatrixTimesInverse,
        limits,
        &state,
    )?;
    drop(left);
    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let right = call_native("right inverse product", || &inverse * &matrix)?;
    verify_identity_product(
        context,
        &right,
        size,
        SymbolicaInverseSide::InverseTimesMatrix,
        limits,
        &state,
    )?;
    drop(right);

    let inverse = native_into_rows(inverse, &state)?;
    let stats = state.borrow().stats;
    debug_assert_eq!(entries, inverse.iter().map(Vec::len).sum::<usize>());
    Ok(VerifiedSymbolicaCoefficientInverse {
        inverse,
        determinant,
        stats,
    })
}

/// Verify both inverse products through Symbolica for caller-retained matrices.
pub(crate) fn verify_coefficient_matrix_inverse(
    context: &CoefficientContext,
    rows: &[Vec<Coefficient>],
    inverse: &[Vec<Coefficient>],
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<SymbolicaCoefficientMatrixStats, SymbolicaCoefficientMatrixError> {
    let shape = inspect_rows(rows)?;
    let inverse_shape = inspect_rows(inverse)?;
    let size = require_square(shape)?;
    if inverse_shape.rows != size || inverse_shape.columns != size {
        return Err(SymbolicaCoefficientMatrixError::ShapeMismatch {
            left_rows: shape.rows,
            left_columns: shape.columns,
            right_rows: inverse_shape.rows,
            right_columns: inverse_shape.columns,
        });
    }
    let product_entries = shape.entries;
    let live_entries = checked_mul("live Symbolica matrix entries", product_entries, 3)?;
    check_limit(
        "single Symbolica matrix entries",
        product_entries,
        limits.max_single_matrix_entries,
    )?;
    check_limit(
        "live Symbolica matrix entries",
        live_entries,
        limits.max_live_matrix_entries,
    )?;
    let operations = checked_mul(
        "Symbolica coefficient matrix exact operations",
        2,
        product_operation_bound(size, size, size)?,
    )?;
    check_limit(
        "Symbolica coefficient matrix exact operations",
        operations,
        limits.max_exact_operations,
    )?;
    let field =
        CheckedCoefficientField::new(context, limits, product_entries, live_entries, operations);
    let state = field.state.clone();
    let matrix = matrix_from_rows(rows, shape, field.clone())?;
    let inverse = matrix_from_rows(inverse, inverse_shape, field)?;

    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let left = call_native("left inverse product", || &matrix * &inverse)?;
    verify_identity_product(
        context,
        &left,
        size,
        SymbolicaInverseSide::MatrixTimesInverse,
        limits,
        &state,
    )?;
    drop(left);
    increment_session_counter(&state, "Symbolica matrix product calls", |stats| {
        &mut stats.product_calls
    })?;
    let right = call_native("right inverse product", || &inverse * &matrix)?;
    verify_identity_product(
        context,
        &right,
        size,
        SymbolicaInverseSide::InverseTimesMatrix,
        limits,
        &state,
    )?;
    drop(right);
    let stats = state.borrow().stats;
    Ok(stats)
}

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

/// Compute `transform * middle * transform^T` through Symbolica's native
/// transpose and matrix products in one authenticated session.
///
/// Keeping the transpose inside this boundary prevents callers from growing a
/// second, handwritten standard-matrix implementation merely to form a
/// congruence.  The two native product outputs are both authenticated and
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
