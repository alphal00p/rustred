//! Authenticated, resource-bounded access to Symbolica's exact coefficient and
//! matrix algebra.
//!
//! This module is deliberately provenance-neutral.  Symbolica owns coefficient
//! powers plus matrix rank, determinant, inversion, and multiplication;
//! RustRed supplies only the authenticated coefficient domain, admission
//! policy, typed failure transport, and replay checks needed by proof-bearing
//! callers.
//!
//! Input and every retained native output are censused by exact clone-owned
//! capacity.  Symbolica's public scalar API does not expose a complete bound on
//! polynomial GCD, quotient, or dense-multiplication scratch, so that remaining
//! native scratch gap is explicit rather than being disguised as a byte proof.
//! Typed scalar failures cross Symbolica's infallible field traits through a
//! private unwind payload.  This boundary therefore requires Rust's
//! `panic = "unwind"`; `panic = "abort"` builds cannot recover a typed failure.
//!

#[cfg(not(panic = "unwind"))]
compile_error!(
    "RustRed's authenticated Symbolica algebra boundary requires panic=\"unwind\" for typed failure transport"
);

mod admission;
mod error;
mod field;
mod operations;

pub(crate) use admission::{
    DEFAULT_MAX_EXACT_OPERATIONS, DEFAULT_MAX_INPUT_RETAINED_BYTES,
    DEFAULT_MAX_OUTPUT_RETAINED_BYTES, SymbolicaCoefficientMatrixLimits,
    SymbolicaCoefficientMatrixStats,
};
pub(crate) use error::{
    SymbolicaCoefficientMatrixError, SymbolicaInverseSide, SymbolicaNativeMatrixErrorKind,
};
pub(crate) use operations::{
    congruence_of_coefficient_matrix, determinant_of_coefficient_matrix,
    invert_and_verify_coefficient_matrix, multiply_coefficient_matrices,
    multiply_three_coefficient_matrices, rank_of_coefficient_matrix,
    verify_coefficient_matrix_inverse,
};

#[cfg(test)]
mod tests;
