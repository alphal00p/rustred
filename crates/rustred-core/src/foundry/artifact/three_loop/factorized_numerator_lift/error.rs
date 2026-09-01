use std::fmt;

use crate::algebra::ExactAlgebraError;
use crate::sector::CoordinatePriorityError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ProbeError {
    WrongTransformedFormCount {
        expected: usize,
        actual: usize,
    },
    WrongSectorArity {
        expected: usize,
        actual: usize,
    },
    NonCornerActivePower {
        slot: usize,
        power: i64,
    },
    ForeignActivePower {
        slot: usize,
        power: i64,
    },
    UnsupportedCornerFactorization {
        detail: &'static str,
    },
    MissingDimensionParameter,
    DegreeOverflow {
        resource: &'static str,
    },
    DegreeLimit {
        resource: &'static str,
        requested: u64,
        limit: u64,
    },
    CountOverflow {
        resource: &'static str,
    },
    CountLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    RankCoefficientOverflow {
        rank: u64,
    },
    MultiplicityCoefficientOverflow {
        multiplicity: u64,
    },
    Invariant {
        detail: &'static str,
    },
    CoordinatePriority(CoordinatePriorityError),
    ExactAlgebra(ExactAlgebraError),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongTransformedFormCount { expected, actual } => write!(
                formatter,
                "the corner probe received {actual} transformed forms, expected {expected}"
            ),
            Self::WrongSectorArity { expected, actual } => write!(
                formatter,
                "the factorization sector has {actual} coordinates, expected {expected}"
            ),
            Self::NonCornerActivePower { slot, power } => write!(
                formatter,
                "corner-probe active slot {slot} has power {power}, expected one"
            ),
            Self::ForeignActivePower { slot, power } => write!(
                formatter,
                "corner-probe inactive slot {slot} has foreign positive power {power}"
            ),
            Self::UnsupportedCornerFactorization { detail } => {
                write!(formatter, "unsupported corner factorization: {detail}")
            }
            Self::MissingDimensionParameter => {
                formatter.write_str("the corner probe coefficient context has no d parameter")
            }
            Self::DegreeOverflow { resource } => {
                write!(formatter, "{resource} overflowed u64")
            }
            Self::DegreeLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested degree {requested}, configured limit is {limit}"
            ),
            Self::CountOverflow { resource } => {
                write!(formatter, "{resource} overflowed usize")
            }
            Self::CountLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested count {requested}, configured limit is {limit}"
            ),
            Self::RankCoefficientOverflow { rank } => write!(
                formatter,
                "angular rank {rank} cannot be represented by the exact integer-coefficient API"
            ),
            Self::MultiplicityCoefficientOverflow { multiplicity } => write!(
                formatter,
                "angular multiplicity {multiplicity} cannot be represented by the exact integer-coefficient API"
            ),
            Self::Invariant { detail } => {
                write!(
                    formatter,
                    "factorized-numerator probe invariant failed: {detail}"
                )
            }
            Self::CoordinatePriority(error) => error.fmt(formatter),
            Self::ExactAlgebra(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ProbeError {}

impl From<CoordinatePriorityError> for ProbeError {
    fn from(error: CoordinatePriorityError) -> Self {
        Self::CoordinatePriority(error)
    }
}

impl From<ExactAlgebraError> for ProbeError {
    fn from(error: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(error)
    }
}
