use std::fmt;

use super::super::ExactExecutableOwnerError;
use crate::foundry::completion::stratum::StratumRegistryError;
use crate::sector::Error as SectorError;

/// Hard failure at the staged wave boundary. Ordinary lack of closure is a
/// typed `StagedSectorClosureStop`, never an error and never publication.
#[derive(Debug)]
pub(crate) enum StagedSectorClosureError {
    EmptyFrontier,
    InvalidPredecessor,
    WrongPredecessorContext,
    WrongSectorArity {
        sector: usize,
        expected: usize,
        actual: usize,
    },
    MixedFrontierActiveCount {
        sector: usize,
        expected: usize,
        actual: usize,
    },
    DuplicateSector,
    PreviewRequiresSingleSector {
        actual: usize,
    },
    PreviewRequiresOwner,
    UnregisteredSector,
    OwnerScope {
        detail: &'static str,
    },
    TerminalOutsideSector,
    UnauthenticatedTerminal,
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
    Sector(SectorError),
    Executable(ExactExecutableOwnerError),
    Registry(StratumRegistryError),
}

impl fmt::Display for StagedSectorClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFrontier => formatter.write_str("a staged closure wave has no sectors"),
            Self::InvalidPredecessor => {
                formatter.write_str("the staged closure predecessor failed cold verification")
            }
            Self::WrongPredecessorContext => formatter
                .write_str("the staged closure context differs from its predecessor snapshot"),
            Self::WrongSectorArity {
                sector,
                expected,
                actual,
            } => write!(
                formatter,
                "staged sector {sector} has arity {actual}, expected {expected}"
            ),
            Self::MixedFrontierActiveCount {
                sector,
                expected,
                actual,
            } => write!(
                formatter,
                "staged sector {sector} has active count {actual}, expected wave rank {expected}"
            ),
            Self::DuplicateSector => {
                formatter.write_str("a staged closure wave repeats one sector and ordering")
            }
            Self::PreviewRequiresSingleSector { actual } => write!(
                formatter,
                "an exact cover preview requires one staged sector, found {actual}"
            ),
            Self::PreviewRequiresOwner => {
                formatter.write_str("an exact cover preview requires at least one staged owner")
            }
            Self::UnregisteredSector => {
                formatter.write_str("an owner or terminal targets an unstaged sector")
            }
            Self::OwnerScope { detail } => {
                write!(
                    formatter,
                    "staged executable owner has the wrong scope: {detail}"
                )
            }
            Self::TerminalOutsideSector => {
                formatter.write_str("an explicit terminal lies outside its staged sector")
            }
            Self::UnauthenticatedTerminal => formatter.write_str(
                "an explicit terminal is not covered by retained root terminal authority",
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}"
            ),
            Self::Sector(error) => error.fmt(formatter),
            Self::Executable(error) => error.fmt(formatter),
            Self::Registry(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for StagedSectorClosureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sector(error) => Some(error),
            Self::Executable(error) => Some(error),
            Self::Registry(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SectorError> for StagedSectorClosureError {
    fn from(value: SectorError) -> Self {
        Self::Sector(value)
    }
}

impl From<ExactExecutableOwnerError> for StagedSectorClosureError {
    fn from(value: ExactExecutableOwnerError) -> Self {
        Self::Executable(value)
    }
}

impl From<StratumRegistryError> for StagedSectorClosureError {
    fn from(value: StratumRegistryError) -> Self {
        Self::Registry(value)
    }
}
