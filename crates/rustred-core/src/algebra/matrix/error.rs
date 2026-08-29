//! Typed failures and verified-result transport for native matrix sessions.

use std::fmt;

use crate::algebra::ExactAlgebraError;

/// Which certified inverse product failed to replay to the identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SymbolicaInverseSide {
    MatrixTimesInverse,
    InverseTimesMatrix,
}

impl fmt::Display for SymbolicaInverseSide {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MatrixTimesInverse => formatter.write_str("A A^-1"),
            Self::InverseTimesMatrix => formatter.write_str("A^-1 A"),
        }
    }
}

/// Bounded classification of native Matrix errors.  It intentionally carries
/// no Matrix payload and is never created by formatting `MatrixError`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SymbolicaNativeMatrixErrorKind {
    Underdetermined,
    Inconsistent,
    NotSquare,
    Singular,
    ShapeMismatch,
    RightHandSideIsNotVector,
    ResultNotInDomain,
}

/// Typed failures at the authenticated Symbolica matrix boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SymbolicaCoefficientMatrixError {
    EmptyMatrix,
    RaggedMatrix {
        row: usize,
        expected_columns: usize,
        actual_columns: usize,
    },
    NotSquare {
        rows: usize,
        columns: usize,
    },
    ShapeMismatch {
        left_rows: usize,
        left_columns: usize,
        right_rows: usize,
        right_columns: usize,
    },
    DimensionOverflow {
        rows: usize,
        columns: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    InvalidCoefficient {
        row: usize,
        column: usize,
        error: ExactAlgebraError,
    },
    ExactAlgebra(ExactAlgebraError),
    NativePowerExponentLimit {
        requested: u64,
        limit: u32,
    },
    Singular,
    NativeError {
        operation: &'static str,
        kind: SymbolicaNativeMatrixErrorKind,
    },
    NativePanic {
        operation: &'static str,
    },
    InverseVerificationFailure {
        side: SymbolicaInverseSide,
        row: usize,
        column: usize,
    },
    InternalShapeFailure {
        operation: &'static str,
    },
}

impl fmt::Display for SymbolicaCoefficientMatrixError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMatrix => formatter.write_str("a coefficient matrix cannot be empty"),
            Self::RaggedMatrix {
                row,
                expected_columns,
                actual_columns,
            } => write!(
                formatter,
                "coefficient matrix row {row} has {actual_columns} columns, expected {expected_columns}"
            ),
            Self::NotSquare { rows, columns } => {
                write!(
                    formatter,
                    "coefficient matrix is {rows}x{columns}, not square"
                )
            }
            Self::ShapeMismatch {
                left_rows,
                left_columns,
                right_rows,
                right_columns,
            } => write!(
                formatter,
                "coefficient matrix shapes {left_rows}x{left_columns} and {right_rows}x{right_columns} are incompatible"
            ),
            Self::DimensionOverflow { rows, columns } => write!(
                formatter,
                "coefficient matrix shape {rows}x{columns} exceeds Symbolica's native representation"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(formatter, "failed to reserve {requested} {resource}"),
            Self::InvalidCoefficient { row, column, error } => write!(
                formatter,
                "coefficient matrix entry ({row},{column}) is invalid: {error}"
            ),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::NativePowerExponentLimit { requested, limit } => write!(
                formatter,
                "Symbolica coefficient power exponent {requested} exceeds its native limit {limit}"
            ),
            Self::Singular => formatter.write_str("coefficient matrix is singular"),
            Self::NativeError { operation, kind } => {
                write!(
                    formatter,
                    "Symbolica matrix {operation} failed with {kind:?}"
                )
            }
            Self::NativePanic { operation } => {
                write!(
                    formatter,
                    "Symbolica panicked while computing matrix {operation}"
                )
            }
            Self::InverseVerificationFailure { side, row, column } => write!(
                formatter,
                "{side} differs from identity at ({row},{column})"
            ),
            Self::InternalShapeFailure { operation } => write!(
                formatter,
                "Symbolica returned an incompatible shape from matrix {operation}"
            ),
        }
    }
}

impl std::error::Error for SymbolicaCoefficientMatrixError {}
