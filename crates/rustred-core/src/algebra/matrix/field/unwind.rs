//! Typed unwind transport across Symbolica's infallible field traits.

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use symbolica::prelude::Ring;
use symbolica::tensors::matrix::MatrixError;

use crate::algebra::ExactAlgebraError;
use crate::algebra::matrix::{SymbolicaCoefficientMatrixError, SymbolicaNativeMatrixErrorKind};

/// Private unwind payload for fallible trait methods. `resume_unwind` avoids
/// invoking the process-global panic hook; the nearest matrix boundary catches
/// and downcasts this exact type immediately.
struct CheckedFieldAbort(SymbolicaCoefficientMatrixError);

#[cold]
pub(in crate::algebra::matrix) fn abort_checked_field(error: ExactAlgebraError) -> ! {
    abort_checked_matrix(SymbolicaCoefficientMatrixError::ExactAlgebra(error))
}

#[cold]
pub(super) fn abort_checked_matrix(error: SymbolicaCoefficientMatrixError) -> ! {
    resume_unwind(Box::new(CheckedFieldAbort(error)))
}

fn map_native_error<F: Ring>(
    operation: &'static str,
    error: MatrixError<F>,
) -> SymbolicaCoefficientMatrixError {
    let kind = match error {
        MatrixError::Underdetermined { .. } => SymbolicaNativeMatrixErrorKind::Underdetermined,
        MatrixError::Inconsistent => SymbolicaNativeMatrixErrorKind::Inconsistent,
        MatrixError::NotSquare => SymbolicaNativeMatrixErrorKind::NotSquare,
        MatrixError::Singular => SymbolicaNativeMatrixErrorKind::Singular,
        MatrixError::ShapeMismatch => SymbolicaNativeMatrixErrorKind::ShapeMismatch,
        MatrixError::RightHandSideIsNotVector => {
            SymbolicaNativeMatrixErrorKind::RightHandSideIsNotVector
        }
        MatrixError::ResultNotInDomain => SymbolicaNativeMatrixErrorKind::ResultNotInDomain,
    };
    SymbolicaCoefficientMatrixError::NativeError { operation, kind }
}

pub(in crate::algebra::matrix) fn call_native<T>(
    operation: &'static str,
    callback: impl FnOnce() -> T,
) -> Result<T, SymbolicaCoefficientMatrixError> {
    match catch_unwind(AssertUnwindSafe(callback)) {
        Ok(value) => Ok(value),
        Err(payload) => match payload.downcast::<CheckedFieldAbort>() {
            Ok(abort) => Err(abort.0),
            Err(_) => Err(SymbolicaCoefficientMatrixError::NativePanic { operation }),
        },
    }
}

pub(in crate::algebra::matrix) fn call_native_result<T, F: Ring>(
    operation: &'static str,
    callback: impl FnOnce() -> Result<T, MatrixError<F>>,
) -> Result<T, SymbolicaCoefficientMatrixError> {
    call_native(operation, callback)?.map_err(|error| map_native_error(operation, error))
}
