use std::fmt;

use crate::family::symanzik::FeynmanPolynomialError;
use crate::sector::{self, zero};

/// Typed failures of the per-sector master count. A failure is never a
/// verdict: scaleless, counted, and inconclusive sectors are all
/// [`super::MasterCount`] values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MasterCountError {
    /// A verdict needs at least two independent samples; at most
    /// [`super::MAX_SAMPLES`] are admitted.
    InvalidSampleCount {
        samples: usize,
    },
    /// The count is defined for integer indices; a nonzero power shift moves
    /// the effective support and is not supported.
    UnsupportedNonzeroPowerShift {
        denominator: usize,
    },
    WrongArity {
        expected: usize,
        actual: usize,
    },
    /// The subsector Moebius sum enumerates `2^active` faces.
    SectorTooLarge {
        active: usize,
        limit: usize,
    },
    /// A quotient has more standard monomials than the counter admits.
    QuotientTooLarge {
        limit: usize,
    },
    /// A coefficient polynomial does not use the family's parameter map.
    ForeignCoefficient {
        expected: usize,
        actual: usize,
    },
    /// Every point drawn for one sample was inadmissible or had unlucky
    /// Euler exponents.
    AttemptsExhausted {
        sample: usize,
        attempts: usize,
    },
    SaturationSymbol {
        detail: String,
    },
    GramDeterminant {
        detail: String,
    },
    Feynman(FeynmanPolynomialError),
    ZeroSector(zero::Error),
    Sector(sector::Error),
    SymbolicaPanic {
        stage: &'static str,
    },
}

impl fmt::Display for MasterCountError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSampleCount { samples } => write!(
                formatter,
                "master counting needs between 2 and {} samples, got {samples}",
                super::MAX_SAMPLES
            ),
            Self::UnsupportedNonzeroPowerShift { denominator } => write!(
                formatter,
                "power shift {denominator} is nonzero; master counting supports integer indices only"
            ),
            Self::WrongArity { expected, actual } => {
                write!(formatter, "sector has arity {actual}, expected {expected}")
            }
            Self::SectorTooLarge { active, limit } => write!(
                formatter,
                "sector has {active} active denominators, above the subsector-sum limit {limit}"
            ),
            Self::QuotientTooLarge { limit } => write!(
                formatter,
                "critical-point quotient exceeds {limit} standard monomials"
            ),
            Self::ForeignCoefficient { expected, actual } => write!(
                formatter,
                "coefficient polynomial has {actual} variables, expected the family's {expected}"
            ),
            Self::AttemptsExhausted { sample, attempts } => write!(
                formatter,
                "sample {sample} found no admissible lucky point in {attempts} attempts"
            ),
            Self::SaturationSymbol { detail } => write!(
                formatter,
                "could not construct the Rabinowitsch saturation variable: {detail}"
            ),
            Self::GramDeterminant { detail } => {
                write!(formatter, "external Gram determinant failed: {detail}")
            }
            Self::Feynman(error) => {
                write!(formatter, "Feynman-polynomial construction failed: {error}")
            }
            Self::ZeroSector(error) => write!(formatter, "zero-sector analysis failed: {error}"),
            Self::Sector(error) => write!(formatter, "sector foundation failed: {error}"),
            Self::SymbolicaPanic { stage } => {
                write!(formatter, "Symbolica panicked during {stage}")
            }
        }
    }
}

impl std::error::Error for MasterCountError {}

impl From<FeynmanPolynomialError> for MasterCountError {
    fn from(value: FeynmanPolynomialError) -> Self {
        Self::Feynman(value)
    }
}

impl From<zero::Error> for MasterCountError {
    fn from(value: zero::Error) -> Self {
        Self::ZeroSector(value)
    }
}

impl From<sector::Error> for MasterCountError {
    fn from(value: sector::Error) -> Self {
        Self::Sector(value)
    }
}
