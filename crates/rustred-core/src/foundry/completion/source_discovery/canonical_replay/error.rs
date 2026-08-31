use std::fmt;

use crate::foundry::completion::frame::exact::ExactCircuitError;
use crate::identity::TranslatedSourceError;

use super::super::{CampaignError, SourceDiscoveryError};

/// Hard task-join, invariant, or transactional resource failure.
///
/// Probe-specific singular samples and algebraically inconclusive lifts are
/// retained in the attempt ledger instead of entering this error channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CanonicalReplayError {
    WrongSourceLayout {
        actual: &'static str,
    },
    WrongTaskScope {
        detail: &'static str,
    },
    ReplayedNominationJoin {
        nomination: usize,
        detail: &'static str,
    },
    DuplicateProbeNomination,
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
    Shift(TranslatedSourceError),
    SourceTranslation(TranslatedSourceError),
    SourceDiscovery(SourceDiscoveryError),
    Campaign(CampaignError),
    ExactLift(ExactCircuitError),
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for CanonicalReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSourceLayout { actual } => write!(
                formatter,
                "canonical replay requires complete ordinary IBP sources, got {actual}"
            ),
            Self::WrongTaskScope { detail } => {
                write!(formatter, "canonical replay task scope mismatch: {detail}")
            }
            Self::ReplayedNominationJoin { nomination, detail } => write!(
                formatter,
                "replayed nomination {nomination} failed its retained-task join: {detail}"
            ),
            Self::DuplicateProbeNomination => {
                formatter.write_str("canonical replay received the same raw modular probe twice")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "canonical replay {resource} count overflowed usize"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "canonical replay {resource} needs {requested}, exceeding limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for canonical replay {resource}"
            ),
            Self::Shift(error) => error.fmt(formatter),
            Self::SourceTranslation(error) => error.fmt(formatter),
            Self::SourceDiscovery(error) => error.fmt(formatter),
            Self::Campaign(error) => error.fmt(formatter),
            Self::ExactLift(error) => error.fmt(formatter),
            Self::Invariant { detail } => {
                write!(formatter, "canonical replay invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for CanonicalReplayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Shift(error) | Self::SourceTranslation(error) => Some(error),
            Self::SourceDiscovery(error) => Some(error),
            Self::Campaign(error) => Some(error),
            Self::ExactLift(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CampaignError> for CanonicalReplayError {
    fn from(value: CampaignError) -> Self {
        Self::Campaign(value)
    }
}

impl From<SourceDiscoveryError> for CanonicalReplayError {
    fn from(value: SourceDiscoveryError) -> Self {
        Self::SourceDiscovery(value)
    }
}

impl From<ExactCircuitError> for CanonicalReplayError {
    fn from(value: ExactCircuitError) -> Self {
        Self::ExactLift(value)
    }
}
