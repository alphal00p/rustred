use std::fmt;

use crate::foundry::completion::frame::exact::ExactCircuitError;
use crate::foundry::completion::source_discovery::CampaignError;
use crate::foundry::completion::stratum::StratumRegistryError;

/// Infrastructure or validation failure while rebuilding and exactly lifting
/// one compact streaming support.
///
/// A fresh-frame modular miss and an exact support miss are ordinary typed
/// outcomes in [`super::SpiredCompactLift`], not errors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredCompactLiftError {
    Invariant {
        detail: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Stratum(StratumRegistryError),
    Campaign(CampaignError),
    Exact(ExactCircuitError),
}

impl fmt::Display for SpiredCompactLiftError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invariant { detail } => {
                write!(
                    formatter,
                    "SpIRed compact-support invariant failed: {detail}"
                )
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not allocate {requested} entries for {resource}"
            ),
            Self::Stratum(error) => write!(
                formatter,
                "SpIRed compact-support stratum anchor failed: {error}"
            ),
            Self::Campaign(error) => write!(
                formatter,
                "fresh SpIRed compact-support rematerialization failed: {error}"
            ),
            Self::Exact(error) => write!(formatter, "SpIRed exact support lift failed: {error}"),
        }
    }
}

impl std::error::Error for SpiredCompactLiftError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Invariant { .. } | Self::AllocationFailure { .. } => None,
            Self::Stratum(error) => Some(error),
            Self::Campaign(error) => Some(error),
            Self::Exact(error) => Some(error),
        }
    }
}

impl From<StratumRegistryError> for SpiredCompactLiftError {
    fn from(value: StratumRegistryError) -> Self {
        Self::Stratum(value)
    }
}

impl From<CampaignError> for SpiredCompactLiftError {
    fn from(value: CampaignError) -> Self {
        Self::Campaign(value)
    }
}

impl From<ExactCircuitError> for SpiredCompactLiftError {
    fn from(value: ExactCircuitError) -> Self {
        Self::Exact(value)
    }
}
