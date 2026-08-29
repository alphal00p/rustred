use std::fmt;

use crate::algebra::ExactAlgebraError;
use crate::algebra::matrix::RightKernelError;
use crate::family::symanzik::FeynmanPolynomialError;
use crate::sector;

/// Typed failures at the zero-sector proof boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    WrongRestrictionsArity {
        expected: usize,
        actual: usize,
    },
    UnsupportedNonzeroIntegerPowerShift {
        denominator: usize,
    },
    UnsupportedShiftedCut {
        denominator: usize,
    },
    UnsupportedIntegerSeparatedPowerShifts {
        left: usize,
        right: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
    },
    MatrixDimensionOverflow {
        rows: usize,
        columns: usize,
    },
    MatrixShape {
        rows: usize,
        columns: usize,
        entries: usize,
    },
    KernelInvariant {
        stage: &'static str,
    },
    ExactAlgebra(ExactAlgebraError),
    Feynman(FeynmanPolynomialError),
    Sector(sector::Error),
    SymbolicaPanic,
}

impl Error {
    pub(super) fn from_right_kernel(value: RightKernelError) -> Self {
        match value {
            RightKernelError::ResourceLimit {
                resource,
                requested,
                limit,
            } => Self::ResourceLimit {
                resource,
                requested,
                limit,
            },
            RightKernelError::CountOverflow { resource } => {
                Self::ResourceCountOverflow { resource }
            }
            RightKernelError::AllocationFailure { resource } => {
                Self::AllocationFailure { resource }
            }
            RightKernelError::DimensionOverflow { rows, columns } => {
                Self::MatrixDimensionOverflow { rows, columns }
            }
            RightKernelError::Shape {
                rows,
                columns,
                entries,
            } => Self::MatrixShape {
                rows,
                columns,
                entries,
            },
            RightKernelError::NativePanic => Self::SymbolicaPanic,
            RightKernelError::ZeroColumns => Self::KernelInvariant {
                stage: "zero-column rank matrix",
            },
            RightKernelError::MissingPivot => Self::KernelInvariant {
                stage: "missing RREF pivot",
            },
            RightKernelError::RepeatedPivot => Self::KernelInvariant {
                stage: "repeated RREF pivot",
            },
            RightKernelError::UnnormalizedPivot => Self::KernelInvariant {
                stage: "unnormalized RREF pivot",
            },
            RightKernelError::MissingFreeColumn => Self::KernelInvariant {
                stage: "missing free RREF column",
            },
            RightKernelError::NonIntegralPrimitive => Self::KernelInvariant {
                stage: "nonintegral primitive part",
            },
            RightKernelError::ZeroPrimitive => Self::KernelInvariant {
                stage: "zero primitive part",
            },
            RightKernelError::ReplayFailure => Self::KernelInvariant {
                stage: "native integer-kernel replay",
            },
            RightKernelError::NativeShape => Self::KernelInvariant {
                stage: "native matrix construction",
            },
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongRestrictionsArity { expected, actual } => write!(
                formatter,
                "sector restrictions have arity {actual}, expected {expected}"
            ),
            Self::UnsupportedNonzeroIntegerPowerShift { denominator } => write!(
                formatter,
                "power shift {denominator} is a known nonzero integer; formal-generic sector support is unsound for integer reindexing"
            ),
            Self::UnsupportedShiftedCut { denominator } => write!(
                formatter,
                "cut denominator {denominator} has a nonzero power shift; shifted-cut semantics are not defined"
            ),
            Self::UnsupportedIntegerSeparatedPowerShifts { left, right } => write!(
                formatter,
                "power shifts {left} and {right} differ by a known nonzero integer"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure { resource } => {
                write!(formatter, "could not allocate {resource}")
            }
            Self::MatrixDimensionOverflow { rows, columns } => write!(
                formatter,
                "rank matrix shape {rows} x {columns} cannot be represented safely"
            ),
            Self::MatrixShape {
                rows,
                columns,
                entries,
            } => write!(
                formatter,
                "rank matrix has {entries} entries for shape {rows} x {columns}"
            ),
            Self::KernelInvariant { stage } => {
                write!(formatter, "right-kernel invariant failed during {stage}")
            }
            Self::ExactAlgebra(error) => {
                write!(formatter, "exact power-shift algebra failed: {error}")
            }
            Self::Feynman(error) => {
                write!(formatter, "Feynman-polynomial construction failed: {error}")
            }
            Self::Sector(error) => write!(formatter, "sector foundation failed: {error}"),
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during checked zero-sector analysis")
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<ExactAlgebraError> for Error {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

impl From<FeynmanPolynomialError> for Error {
    fn from(value: FeynmanPolynomialError) -> Self {
        Self::Feynman(value)
    }
}

impl From<sector::Error> for Error {
    fn from(value: sector::Error) -> Self {
        Self::Sector(value)
    }
}
