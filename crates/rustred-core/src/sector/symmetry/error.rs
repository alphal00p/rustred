use std::fmt;

use crate::algebra::ExactAlgebraError;
use crate::family::IntegralFamilyError;

/// Typed failures at the affine-map verification boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    UnequalLoopCount {
        source: usize,
        target: usize,
    },
    UnequalExternalCount {
        source: usize,
        target: usize,
    },
    ForeignCoefficientContext,
    WrongMatrixShape {
        matrix: &'static str,
        expected_rows: usize,
        expected_columns: usize,
        actual_rows: usize,
        actual_columns: usize,
    },
    MatrixPayloadSize {
        rows: usize,
        columns: usize,
        expected: usize,
        actual: usize,
    },
    MatrixPayloadTooLarge {
        rows: usize,
        columns: usize,
        expected: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    ForeignMapCoefficient {
        matrix: &'static str,
        row: usize,
        column: usize,
    },
    SingularLoopMap,
    SingularExternalMap,
    ExternalGramMismatch {
        row: usize,
        column: usize,
    },
    DenominatorReplayMismatch {
        denominator: usize,
        coordinate: Option<usize>,
    },
    CertificateReplayMismatch,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    InternalSymbolicaAlgebra {
        detail: String,
    },
    ExactAlgebra(ExactAlgebraError),
    Family(IntegralFamilyError),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnequalLoopCount { source, target } => write!(
                formatter,
                "source has {source} loops but target has {target} loops"
            ),
            Self::UnequalExternalCount { source, target } => write!(
                formatter,
                "source has {source} external momenta but target has {target}"
            ),
            Self::ForeignCoefficientContext => formatter.write_str(
                "source and target do not share the authenticated coefficient variable map",
            ),
            Self::WrongMatrixShape {
                matrix,
                expected_rows,
                expected_columns,
                actual_rows,
                actual_columns,
            } => write!(
                formatter,
                "momentum matrix {matrix} is {actual_rows}x{actual_columns}, expected {expected_rows}x{expected_columns}"
            ),
            Self::MatrixPayloadSize {
                rows,
                columns,
                expected,
                actual,
            } => write!(
                formatter,
                "a {rows}x{columns} matrix needs {expected} entries, received {actual}"
            ),
            Self::MatrixPayloadTooLarge {
                rows,
                columns,
                expected,
            } => write!(
                formatter,
                "a {rows}x{columns} matrix needs exactly {expected} entries, but the payload contains more"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve storage for {requested} {resource}"
            ),
            Self::ForeignMapCoefficient {
                matrix,
                row,
                column,
            } => write!(
                formatter,
                "momentum matrix {matrix}[{row},{column}] uses a foreign coefficient map"
            ),
            Self::SingularLoopMap => {
                formatter.write_str("the exact loop-momentum matrix is singular")
            }
            Self::SingularExternalMap => {
                formatter.write_str("the exact external-momentum matrix is singular")
            }
            Self::ExternalGramMismatch { row, column } => write!(
                formatter,
                "external Gram transport fails at entry [{row},{column}]"
            ),
            Self::DenominatorReplayMismatch {
                denominator,
                coordinate,
            } => match coordinate {
                Some(coordinate) => write!(
                    formatter,
                    "affine denominator replay fails for D{denominator} at scalar coordinate {coordinate}"
                ),
                None => write!(
                    formatter,
                    "affine denominator replay fails for the constant of D{denominator}"
                ),
            },
            Self::CertificateReplayMismatch => {
                formatter.write_str("the retained affine-family certificate differs on replay")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "symmetry {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::InternalSymbolicaAlgebra { detail } => {
                write!(
                    formatter,
                    "native Symbolica symmetry algebra failed: {detail}"
                )
            }
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::Family(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for Error {}

impl From<ExactAlgebraError> for Error {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

impl From<IntegralFamilyError> for Error {
    fn from(value: IntegralFamilyError) -> Self {
        Self::Family(value)
    }
}
