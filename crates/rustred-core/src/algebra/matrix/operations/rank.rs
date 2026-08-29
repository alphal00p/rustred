//! Exact rectangular rank through Symbolica's destructive row reduction.

use crate::algebra::matrix::admission::{
    authenticate_native, check_limit, increment_session_counter, inspect_rows, matrix_from_rows,
};
use crate::algebra::matrix::field::{CheckedCoefficientField, call_native};
use crate::algebra::matrix::{
    SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixLimits,
    SymbolicaCoefficientMatrixStats,
};
use crate::algebra::{Coefficient, CoefficientContext};

/// Compute the exact rank of a nonempty rectangular coefficient matrix through
/// Symbolica's destructive field row reduction.
///
/// Calling `Matrix::partial_row_reduce` on the owned native matrix avoids the
/// additional full clone performed by `Matrix::rank`. RustRed does not select
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
    // The only native matrix is destructively reduced in place. The borrowed
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
