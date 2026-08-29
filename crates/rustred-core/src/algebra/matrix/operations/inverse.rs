//! Exact inversion and two-sided Symbolica verification.

use std::cell::RefCell;
use std::rc::Rc;

use symbolica::domains::SelfRing;
use symbolica::prelude::Matrix;

use crate::algebra::matrix::admission::{
    authenticate_native, authenticate_output_coefficient, check_limit, checked_add, checked_mul,
    determinant_operation_bound, increment_session_counter, inspect_rows, inverse_operation_bound,
    matrix_from_rows, native_into_rows, product_operation_bound, require_square,
    square_representation_bounds,
};
use crate::algebra::matrix::field::{
    CheckedCoefficientField, CheckedFieldState, call_native, call_native_result,
};
use crate::algebra::matrix::{
    SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixLimits,
    SymbolicaCoefficientMatrixStats, SymbolicaInverseSide, SymbolicaNativeMatrixErrorKind,
};
use crate::algebra::{Coefficient, CoefficientContext};

/// A determinant, inverse, and native two-sided replay certificate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VerifiedSymbolicaCoefficientInverse {
    pub(in crate::algebra::matrix) inverse: Vec<Vec<Coefficient>>,
    pub(in crate::algebra::matrix) determinant: Coefficient,
    pub(in crate::algebra::matrix) stats: SymbolicaCoefficientMatrixStats,
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

/// Compute and certify an exact inverse using only Symbolica matrix algebra.
pub(crate) fn invert_and_verify_coefficient_matrix<Row>(
    context: &CoefficientContext,
    rows: &[Row],
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<VerifiedSymbolicaCoefficientInverse, SymbolicaCoefficientMatrixError>
where
    Row: AsRef<[Coefficient]>,
{
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
#[cfg(test)]
pub(crate) fn verify_coefficient_matrix_inverse<Row, InverseRow>(
    context: &CoefficientContext,
    rows: &[Row],
    inverse: &[InverseRow],
    limits: SymbolicaCoefficientMatrixLimits,
) -> Result<SymbolicaCoefficientMatrixStats, SymbolicaCoefficientMatrixError>
where
    Row: AsRef<[Coefficient]>,
    InverseRow: AsRef<[Coefficient]>,
{
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
