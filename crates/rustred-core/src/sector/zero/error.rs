use std::fmt;

use crate::algebra::ExactAlgebraError;
use crate::family::symanzik::FeynmanPolynomialError;
use crate::sector;

/// Typed failures at the proof boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ZeroSectorError {
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
    MatrixDimensionOverflow {
        rows: usize,
        columns: usize,
    },
    MatrixShape {
        detail: String,
    },
    ForeignCertificateFamily,
    CertificateSchemaMismatch,
    CertificateReplayFailure {
        detail: String,
    },
    ExactAlgebra(ExactAlgebraError),
    Feynman(FeynmanPolynomialError),
    Sector(sector::Error),
    SymbolicaPanic,
}

impl fmt::Display for ZeroSectorError {
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
            Self::MatrixDimensionOverflow { rows, columns } => write!(
                formatter,
                "rank matrix shape {rows} x {columns} cannot be represented safely"
            ),
            Self::MatrixShape { detail } => write!(formatter, "invalid rank matrix: {detail}"),
            Self::ForeignCertificateFamily => {
                formatter.write_str("zero-sector certificate belongs to a foreign family")
            }
            Self::CertificateSchemaMismatch => {
                formatter.write_str("zero-sector certificate schema is unsupported")
            }
            Self::CertificateReplayFailure { detail } => {
                write!(formatter, "zero-sector certificate replay failed: {detail}")
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

impl std::error::Error for ZeroSectorError {}

impl From<ExactAlgebraError> for ZeroSectorError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

impl From<FeynmanPolynomialError> for ZeroSectorError {
    fn from(value: FeynmanPolynomialError) -> Self {
        Self::Feynman(value)
    }
}

impl From<sector::Error> for ZeroSectorError {
    fn from(value: sector::Error) -> Self {
        Self::Sector(value)
    }
}
