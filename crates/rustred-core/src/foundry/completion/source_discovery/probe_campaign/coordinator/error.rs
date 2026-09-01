use std::fmt;

use super::super::super::boundary_simplex::BoundarySimplexPlanError;
use super::super::super::cover_delta::ExactOwnerCoverDeltaError;
use super::super::ProbeCampaignError;

/// Hard construction or execution failure. Search-inconclusive outcomes use
/// NeedsRefinement, while caps and incomplete probe execution use the typed
/// operational stop; neither masquerades as failure, exhaustion, or closure.
#[derive(Debug)]
pub(crate) enum ProbeCoordinatorFailure {
    EmptyDeclaredCampaignKey,
    EmptyProbeBatch,
    ProbeCountMismatch {
        expected: usize,
        actual: usize,
    },
    ZeroInteriorMargin,
    EmptyUncoveredPartition,
    WrongPartitionBoxArity {
        box_ordinal: usize,
        expected: usize,
        actual: usize,
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
    Cover(ExactOwnerCoverDeltaError),
    BoundaryPlan(BoundarySimplexPlanError),
    Campaign(ProbeCampaignError),
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for ProbeCoordinatorFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDeclaredCampaignKey => {
                formatter.write_str("probe coordinator declared campaign key is empty")
            }
            Self::EmptyProbeBatch => {
                formatter.write_str("probe coordinator declared an empty task probe batch")
            }
            Self::ProbeCountMismatch { expected, actual } => write!(
                formatter,
                "probe coordinator task supplied {actual} probes, expected exactly {expected}"
            ),
            Self::ZeroInteriorMargin => formatter
                .write_str("probe coordinator positive-dimensional profile has zero margin"),
            Self::EmptyUncoveredPartition => formatter.write_str(
                "probe coordinator received an empty partition without compiler closure",
            ),
            Self::WrongPartitionBoxArity {
                box_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "probe coordinator partition box {box_ordinal} has arity {actual}, expected {expected}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "probe coordinator {resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "probe coordinator {resource} needs {requested}, exceeding limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for probe coordinator {resource}"
            ),
            Self::Cover(error) => error.fmt(formatter),
            Self::BoundaryPlan(error) => error.fmt(formatter),
            Self::Campaign(error) => error.fmt(formatter),
            Self::Invariant { detail } => {
                write!(formatter, "probe coordinator invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for ProbeCoordinatorFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cover(error) => Some(error),
            Self::BoundaryPlan(error) => Some(error),
            Self::Campaign(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ExactOwnerCoverDeltaError> for ProbeCoordinatorFailure {
    fn from(error: ExactOwnerCoverDeltaError) -> Self {
        Self::Cover(error)
    }
}

impl From<BoundarySimplexPlanError> for ProbeCoordinatorFailure {
    fn from(error: BoundarySimplexPlanError) -> Self {
        Self::BoundaryPlan(error)
    }
}

impl From<ProbeCampaignError> for ProbeCoordinatorFailure {
    fn from(error: ProbeCampaignError) -> Self {
        Self::Campaign(error)
    }
}
