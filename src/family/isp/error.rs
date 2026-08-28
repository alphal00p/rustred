//! Failures from checked ISP completion.

use std::fmt;

use crate::algebra::ExactAlgebraError;
use crate::family::IntegralFamilyError;

/// Typed failures from exact automatic ISP completion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IspCompletionError {
    NoLoopMomenta,
    NoInputDenominators,
    ScalarProductCountOverflow {
        loops: usize,
        externals: usize,
    },
    TooManyInputDenominators {
        maximum: usize,
        actual: usize,
    },
    WrongInputPowerShiftCount {
        expected: usize,
        actual: usize,
    },
    WrongDenominatorRowSize {
        denominator: usize,
        expected: usize,
        actual: usize,
    },
    InvalidInputCoefficient {
        denominator: usize,
        coordinate: Option<usize>,
        error: ExactAlgebraError,
    },
    DependentInputDenominators {
        denominators: usize,
        generic_rank: usize,
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
        requested: usize,
    },
    ExactAlgebra(ExactAlgebraError),
    Family(IntegralFamilyError),
    InternalVerificationFailure {
        detail: String,
    },
}

impl fmt::Display for IspCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLoopMomenta => {
                formatter.write_str("automatic ISP completion needs at least one loop momentum")
            }
            Self::NoInputDenominators => {
                formatter.write_str("automatic ISP completion needs at least one input denominator")
            }
            Self::ScalarProductCountOverflow { loops, externals } => write!(
                formatter,
                "the scalar-product count for {loops} loops and {externals} external momenta overflowed usize"
            ),
            Self::TooManyInputDenominators { maximum, actual } => write!(
                formatter,
                "an independent basis has at most {maximum} input denominators, received {actual}"
            ),
            Self::WrongInputPowerShiftCount { expected, actual } => write!(
                formatter,
                "received {actual} input power shifts for {expected} supplied denominators"
            ),
            Self::WrongDenominatorRowSize {
                denominator,
                expected,
                actual,
            } => write!(
                formatter,
                "input denominator {denominator} has {actual} scalar-product coefficients, expected {expected}"
            ),
            Self::InvalidInputCoefficient {
                denominator,
                coordinate,
                error,
            } => match coordinate {
                Some(coordinate) => write!(
                    formatter,
                    "invalid coefficient {coordinate} of input denominator {denominator}: {error}"
                ),
                None => write!(
                    formatter,
                    "invalid constant of input denominator {denominator}: {error}"
                ),
            },
            Self::DependentInputDenominators {
                denominators,
                generic_rank,
            } => write!(
                formatter,
                "the {denominators} supplied denominators have generic rank {generic_rank}; dependent sets require partial fractioning"
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
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(formatter, "failed to reserve {requested} {resource}"),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::Family(error) => error.fmt(formatter),
            Self::InternalVerificationFailure { detail } => {
                write!(
                    formatter,
                    "ISP completion internal verification failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for IspCompletionError {}

impl From<ExactAlgebraError> for IspCompletionError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

impl From<IntegralFamilyError> for IspCompletionError {
    fn from(value: IntegralFamilyError) -> Self {
        Self::Family(value)
    }
}
