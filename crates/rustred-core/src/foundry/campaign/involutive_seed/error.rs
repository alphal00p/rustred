use std::fmt;

use crate::foundry::completion::involutive::{InvolutiveError, OrdinaryChartLiftError};
use crate::foundry::completion::source_discovery::RequestedDomainSupportError;
use crate::identity::TranslatedSourceError;

/// Typed failure before an involutive seed report can be produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InvolutiveSeedError {
    EmptyStableScopeKey,
    ChartLift(OrdinaryChartLiftError),
    Involutive(InvolutiveError),
    RequestedSupport(RequestedDomainSupportError),
    IntegralShift(TranslatedSourceError),
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

impl fmt::Display for InvolutiveSeedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyStableScopeKey => {
                formatter.write_str("involutive seed stable scope key is empty")
            }
            Self::ChartLift(error) => error.fmt(formatter),
            Self::Involutive(error) => error.fmt(formatter),
            Self::RequestedSupport(error) => error.fmt(formatter),
            Self::IntegralShift(error) => error.fmt(formatter),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "involutive seed {resource} count overflowed usize"
                )
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for involutive seed {resource}"
            ),
            Self::Invariant { detail } => {
                write!(formatter, "involutive seed invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for InvolutiveSeedError {}

impl From<OrdinaryChartLiftError> for InvolutiveSeedError {
    fn from(value: OrdinaryChartLiftError) -> Self {
        Self::ChartLift(value)
    }
}

impl From<InvolutiveError> for InvolutiveSeedError {
    fn from(value: InvolutiveError) -> Self {
        Self::Involutive(value)
    }
}

impl From<RequestedDomainSupportError> for InvolutiveSeedError {
    fn from(value: RequestedDomainSupportError) -> Self {
        Self::RequestedSupport(value)
    }
}

impl From<TranslatedSourceError> for InvolutiveSeedError {
    fn from(value: TranslatedSourceError) -> Self {
        Self::IntegralShift(value)
    }
}
