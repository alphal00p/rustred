//! Matrix shape admission, resource bounds, and retained-value census.

mod bounds;
mod input;
mod limits;
mod output;
mod session;
mod shape;

pub(super) use bounds::{
    check_limit, checked_add, checked_mul, determinant_operation_bound, inverse_operation_bound,
    product_operation_bound, square_representation_bounds,
};
pub(super) use input::{coefficient_retained_bytes, matrix_from_rows};
pub(crate) use limits::{
    DEFAULT_MAX_EXACT_OPERATIONS, DEFAULT_MAX_INPUT_RETAINED_BYTES,
    DEFAULT_MAX_OUTPUT_RETAINED_BYTES, SymbolicaCoefficientMatrixLimits,
    SymbolicaCoefficientMatrixStats,
};
pub(super) use output::{authenticate_native, authenticate_output_coefficient, native_into_rows};
pub(super) use session::increment_session_counter;
pub(super) use shape::{checked_shape, inspect_rows, require_square};
