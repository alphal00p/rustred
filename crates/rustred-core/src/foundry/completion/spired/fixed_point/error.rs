use std::fmt;

use crate::family::IntegralKeyError;
use crate::foundry::completion::source_discovery::ExactOwnerCoverDeltaError;
use crate::foundry::completion::source_discovery::leader_walk::LeaderWalkPlanError;

/// Typed failure from the injected exact target runner.
///
/// Normal no-hit, rejected-hit, guard, and finite-budget outcomes belong in
/// `SpiredTargetPortfolioDisposition::Incomplete`, not here.
#[derive(Debug)]
pub(crate) enum SpiredTargetRunnerError {
    Failed { detail: &'static str },
}

/// Hard failure of the fixed-point coordinator.
#[derive(Debug)]
pub(crate) enum SpiredFixedPointError {
    WrongCoordinatePriorityArity {
        expected: usize,
        actual: usize,
    },
    TargetPowerOverflow {
        position: usize,
        corner: i64,
        shift: i64,
    },
    IntegralKey(IntegralKeyError),
    LeaderWalk(LeaderWalkPlanError),
    CoverDelta(ExactOwnerCoverDeltaError),
    Runner(SpiredTargetRunnerError),
    RunnerExceededBudget {
        resource: &'static str,
        reported: usize,
        remaining: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for SpiredTargetRunnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed { detail } => write!(formatter, "SpIRed target runner failed: {detail}"),
        }
    }
}

impl std::error::Error for SpiredTargetRunnerError {}

impl fmt::Display for SpiredFixedPointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongCoordinatePriorityArity { expected, actual } => write!(
                formatter,
                "SpIRed discovery priority has arity {actual}, expected {expected}"
            ),
            Self::TargetPowerOverflow {
                position,
                corner,
                shift,
            } => write!(
                formatter,
                "SpIRed target power overflowed at position {position}: {corner} + {shift}"
            ),
            Self::IntegralKey(error) => error.fmt(formatter),
            Self::LeaderWalk(error) => error.fmt(formatter),
            Self::CoverDelta(error) => error.fmt(formatter),
            Self::Runner(error) => error.fmt(formatter),
            Self::RunnerExceededBudget {
                resource,
                reported,
                remaining,
            } => write!(
                formatter,
                "SpIRed target runner reported {reported} {resource}, but only {remaining} remained"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "SpIRed fixed-point {resource} count overflowed usize"
                )
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for SpIRed fixed-point {resource}"
            ),
            Self::Invariant { detail } => {
                write!(formatter, "SpIRed fixed-point invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for SpiredFixedPointError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IntegralKey(error) => Some(error),
            Self::LeaderWalk(error) => Some(error),
            Self::CoverDelta(error) => Some(error),
            Self::Runner(error) => Some(error),
            _ => None,
        }
    }
}

impl From<IntegralKeyError> for SpiredFixedPointError {
    fn from(error: IntegralKeyError) -> Self {
        Self::IntegralKey(error)
    }
}

impl From<LeaderWalkPlanError> for SpiredFixedPointError {
    fn from(error: LeaderWalkPlanError) -> Self {
        Self::LeaderWalk(error)
    }
}

impl From<ExactOwnerCoverDeltaError> for SpiredFixedPointError {
    fn from(error: ExactOwnerCoverDeltaError) -> Self {
        Self::CoverDelta(error)
    }
}

impl From<SpiredTargetRunnerError> for SpiredFixedPointError {
    fn from(error: SpiredTargetRunnerError) -> Self {
        Self::Runner(error)
    }
}
