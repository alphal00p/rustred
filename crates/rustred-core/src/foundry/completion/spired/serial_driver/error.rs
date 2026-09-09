use std::fmt;

use crate::family::IntegralKeyError;
use crate::foundry::completion::CompletionGeometryError;
use crate::foundry::completion::source_discovery::{CampaignError, ExactOwnerCoverDeltaError};
use crate::sector::Error as SectorError;

use super::super::{SpiredCoordinateCaseWorklistError, SpiredEqualityStepError};
use super::{SpiredSerialCaseReport, SpiredSerialDriverCensus};

/// Failure while retaining an explicit finite probe portfolio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredSerialProbePortfolioError {
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
}

/// Failure to map one exact equality case to a case-valid raw probe.
#[derive(Debug)]
pub(crate) enum SpiredSerialProbeBuildError {
    WrongBaseParameterArity {
        point_ordinal: usize,
        expected: usize,
        actual: usize,
    },
    WrongChartRankArity {
        point_ordinal: usize,
        expected: usize,
        actual: usize,
    },
    InvalidCaseCoordinate {
        position: usize,
        active: bool,
        lower: i64,
        upper: i64,
    },
    CoordinateOverflow {
        position: usize,
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
    Campaign(CampaignError),
}

/// Hard failure with all previously completed outer-driver telemetry.
///
/// The current exact obligation is retained for every variant.
#[derive(Debug)]
pub(crate) struct SpiredSerialDriverError {
    census: SpiredSerialDriverCensus,
    completed_cases: Box<[SpiredSerialCaseReport]>,
    cause: SpiredSerialDriverErrorCause,
}

#[derive(Debug)]
pub(super) enum SpiredSerialDriverErrorCause {
    WrongCaseFamily,
    WrongCaseContext,
    WrongCaseSector,
    CompletionGeometry(CompletionGeometryError),
    IntegralKey(IntegralKeyError),
    TerminalAuthority(SectorError),
    ProbeBuild(SpiredSerialProbeBuildError),
    EqualityStep(SpiredEqualityStepError),
    Worklist(SpiredCoordinateCaseWorklistError),
    OwnerLedger(ExactOwnerCoverDeltaError),
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
}

impl SpiredSerialDriverError {
    pub(crate) const fn census(&self) -> SpiredSerialDriverCensus {
        self.census
    }

    pub(crate) fn completed_cases(&self) -> &[SpiredSerialCaseReport] {
        &self.completed_cases
    }

    pub(super) fn new(
        census: SpiredSerialDriverCensus,
        completed_cases: Vec<SpiredSerialCaseReport>,
        cause: SpiredSerialDriverErrorCause,
    ) -> Self {
        Self {
            census,
            completed_cases: completed_cases.into_boxed_slice(),
            cause,
        }
    }
}

impl fmt::Display for SpiredSerialProbePortfolioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "SpIReD serial probe-portfolio {resource} overflowed usize"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "SpIReD serial probe-portfolio {resource} requires {requested}, exceeding {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for SpIReD serial probe-portfolio {resource}"
            ),
        }
    }
}

impl std::error::Error for SpiredSerialProbePortfolioError {}

impl fmt::Display for SpiredSerialProbeBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongBaseParameterArity {
                point_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "base-parameter point {point_ordinal} has arity {actual}, expected {expected}"
            ),
            Self::WrongChartRankArity {
                point_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "chart-rank point {point_ordinal} has arity {actual}, expected {expected}"
            ),
            Self::InvalidCaseCoordinate {
                position,
                active,
                lower,
                upper,
            } => write!(
                formatter,
                "case axis {position} has bounds [{lower}, {upper}] incompatible with the {} sector",
                if *active { "active" } else { "inactive" }
            ),
            Self::CoordinateOverflow { position } => {
                write!(
                    formatter,
                    "case-valid probe coordinate overflowed at axis {position}"
                )
            }
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "case-local probe {resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "case-local probe {resource} requires {requested}, exceeding {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for case-local probe {resource}"
            ),
            Self::Campaign(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SpiredSerialProbeBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Campaign(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for SpiredSerialDriverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SpIReD serial equality-case drive failed: ")?;
        match &self.cause {
            SpiredSerialDriverErrorCause::WrongCaseFamily => {
                formatter.write_str("current case and owner ledger have different families")
            }
            SpiredSerialDriverErrorCause::WrongCaseContext => formatter
                .write_str("current case and owner ledger have different coefficient contexts"),
            SpiredSerialDriverErrorCause::WrongCaseSector => {
                formatter.write_str("current case and owner ledger have different sectors")
            }
            SpiredSerialDriverErrorCause::CompletionGeometry(error) => error.fmt(formatter),
            SpiredSerialDriverErrorCause::IntegralKey(error) => error.fmt(formatter),
            SpiredSerialDriverErrorCause::TerminalAuthority(error) => error.fmt(formatter),
            SpiredSerialDriverErrorCause::ProbeBuild(error) => error.fmt(formatter),
            SpiredSerialDriverErrorCause::EqualityStep(error) => error.fmt(formatter),
            SpiredSerialDriverErrorCause::Worklist(error) => error.fmt(formatter),
            SpiredSerialDriverErrorCause::OwnerLedger(error) => error.fmt(formatter),
            SpiredSerialDriverErrorCause::ResourceCountOverflow { resource } => {
                write!(formatter, "serial-driver {resource} overflowed usize")
            }
            SpiredSerialDriverErrorCause::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for serial-driver {resource}"
            ),
        }
    }
}

impl std::error::Error for SpiredSerialDriverError {}
