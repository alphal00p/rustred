//! Exact square determinant through Symbolica.

use crate::algebra::matrix::admission::{
    authenticate_output_coefficient, check_limit, checked_mul, determinant_operation_bound,
    increment_session_counter, inspect_rows, matrix_from_rows, require_square,
};
use crate::algebra::matrix::field::{CheckedCoefficientField, call_native_result};
use crate::algebra::matrix::{
    SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixLimits,
    SymbolicaCoefficientMatrixStats,
};
use crate::algebra::{Coefficient, CoefficientContext};

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
