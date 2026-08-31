use std::fmt;

use crate::foundry::completion::source_discovery::SourceDiscoveryError;
use crate::foundry::completion::source_discovery::scheduler::ProbeLocalSchedulerError;
use crate::foundry::completion::stratum::StratumRegistryError;
use crate::identity::TranslatedSourceError;
use crate::sector;

use super::super::InteriorSimplexPlanError;

/// Typed failure of the outer proposal-only simplex executor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InteriorSimplexExecutionError {
    Plan(InteriorSimplexPlanError),
    EmptyProbeSchedule,
    WrongSourceLayout {
        actual: &'static str,
    },
    WrongTaskArity {
        canonical_ordinal: usize,
        object: &'static str,
        expected: usize,
        actual: usize,
    },
    WrongImmutableOwnerScope {
        detail: &'static str,
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
    SourceScope(TranslatedSourceError),
    SourceTranslation(TranslatedSourceError),
    SourceDiscovery(SourceDiscoveryError),
    Sector(sector::Error),
    Stratum(StratumRegistryError),
    Scheduler(ProbeLocalSchedulerError),
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for InteriorSimplexExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plan(error) => write!(formatter, "invalid interior-simplex plan: {error}"),
            Self::EmptyProbeSchedule => formatter.write_str(
                "interior-simplex execution requires at least one declared finite-field probe",
            ),
            Self::WrongSourceLayout { actual } => write!(
                formatter,
                "interior-simplex execution requires complete ordinary IBP sources, got {actual}"
            ),
            Self::WrongTaskArity {
                canonical_ordinal,
                object,
                expected,
                actual,
            } => write!(
                formatter,
                "interior-simplex task {canonical_ordinal} {object} has arity {actual}, expected {expected}"
            ),
            Self::WrongImmutableOwnerScope { detail } => {
                write!(
                    formatter,
                    "interior-simplex immutable-owner mismatch: {detail}"
                )
            }
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "interior-simplex execution {resource} overflowed usize"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "interior-simplex execution {resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for interior-simplex execution {resource}"
            ),
            Self::SourceScope(error) => {
                write!(
                    formatter,
                    "interior-simplex source scope is invalid: {error}"
                )
            }
            Self::SourceTranslation(error) => write!(
                formatter,
                "interior-simplex bootstrap source translation failed: {error}"
            ),
            Self::SourceDiscovery(error) => write!(
                formatter,
                "interior-simplex bootstrap nomination failed: {error}"
            ),
            Self::Sector(error) => write!(
                formatter,
                "interior-simplex maximal bootstrap domain failed: {error}"
            ),
            Self::Stratum(error) => write!(
                formatter,
                "interior-simplex guard-blind stratum failed: {error}"
            ),
            Self::Scheduler(error) => {
                write!(
                    formatter,
                    "interior-simplex probe-local scheduler failed: {error}"
                )
            }
            Self::Invariant { detail } => write!(
                formatter,
                "interior-simplex execution invariant failed: {detail}"
            ),
        }
    }
}

impl std::error::Error for InteriorSimplexExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Plan(error) => Some(error),
            Self::SourceScope(error) | Self::SourceTranslation(error) => Some(error),
            Self::SourceDiscovery(error) => Some(error),
            Self::Sector(error) => Some(error),
            Self::Stratum(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            _ => None,
        }
    }
}
