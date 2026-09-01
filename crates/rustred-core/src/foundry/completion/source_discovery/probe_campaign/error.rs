use std::fmt;

use crate::foundry::completion::source_discovery::{
    ExactOwnerCoverDeltaError, InteriorReplayRunError, SourceDiscoveryError,
};
use crate::foundry::completion::stratum::StratumRegistryError;
use crate::identity::TranslatedSourceError;
use crate::sector;

use super::super::boundary_simplex::BoundarySimplexPlanError;
use super::super::interior_simplex::InteriorSimplexPlanError;

/// Hard failure of one plan-to-ledger semantic transaction.
#[derive(Debug)]
pub(crate) enum ProbeCampaignError {
    InteriorPlan(InteriorSimplexPlanError),
    BoundaryPlan(BoundarySimplexPlanError),
    SourceScope(TranslatedSourceError),
    WrongSourceLayout {
        actual: &'static str,
    },
    Scope {
        detail: &'static str,
    },
    StaleParentGeometry,
    SourceDiscovery(SourceDiscoveryError),
    SourceTranslation(TranslatedSourceError),
    Sector(sector::Error),
    Stratum(StratumRegistryError),
    Replay(InteriorReplayRunError),
    CoverDelta(ExactOwnerCoverDeltaError),
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
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for ProbeCampaignError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InteriorPlan(error) => {
                write!(formatter, "invalid interior-simplex probe plan: {error}")
            }
            Self::BoundaryPlan(error) => {
                write!(formatter, "invalid boundary-simplex probe plan: {error}")
            }
            Self::SourceScope(error) => {
                write!(formatter, "invalid completed source scope: {error}")
            }
            Self::WrongSourceLayout { actual } => write!(
                formatter,
                "probe campaign requires complete ordinary sources, got {actual}"
            ),
            Self::Scope { detail } => {
                write!(formatter, "probe campaign scope mismatch: {detail}")
            }
            Self::StaleParentGeometry => formatter
                .write_str("the planned task parent box is absent from the bound ledger geometry"),
            Self::SourceDiscovery(error) => error.fmt(formatter),
            Self::SourceTranslation(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
            Self::Stratum(error) => error.fmt(formatter),
            Self::Replay(error) => error.fmt(formatter),
            Self::CoverDelta(error) => error.fmt(formatter),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "probe campaign {resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "probe campaign {resource} needs {requested}, exceeding limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for probe campaign {resource}"
            ),
            Self::Invariant { detail } => {
                write!(formatter, "probe campaign invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for ProbeCampaignError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InteriorPlan(error) => Some(error),
            Self::BoundaryPlan(error) => Some(error),
            Self::SourceScope(error) | Self::SourceTranslation(error) => Some(error),
            Self::SourceDiscovery(error) => Some(error),
            Self::Sector(error) => Some(error),
            Self::Stratum(error) => Some(error),
            Self::Replay(error) => Some(error),
            Self::CoverDelta(error) => Some(error),
            _ => None,
        }
    }
}

impl From<InteriorReplayRunError> for ProbeCampaignError {
    fn from(value: InteriorReplayRunError) -> Self {
        Self::Replay(value)
    }
}

impl From<ExactOwnerCoverDeltaError> for ProbeCampaignError {
    fn from(value: ExactOwnerCoverDeltaError) -> Self {
        Self::CoverDelta(value)
    }
}
