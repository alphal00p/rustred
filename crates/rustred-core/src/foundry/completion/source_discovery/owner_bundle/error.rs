use std::fmt;

use crate::foundry::cell::RuleCellError;
use crate::foundry::completion::frame::admission::{
    ExactCircuitOuterExtensionError, ExactCircuitOwnerCoverError, ExactCircuitSemanticError,
    ExactOwnerCoverObstructionKind,
};

use super::super::{CampaignError, ExactRuleCellPromotionError};
use crate::foundry::completion::stratum::StratumRegistryError;

/// Hard failure while pairing exact semantic authority with executable cells.
/// Normal guard-stratum obligations are represented by
/// `ExactExecutableOwnerProposal::Incomplete`, not by this type.
#[derive(Debug)]
pub(crate) enum ExactExecutableOwnerError {
    EmptyCandidates,
    EmptyOwners,
    WrongContext,
    CoverNotClosed {
        obstruction: ExactOwnerCoverObstructionKind,
    },
    ClosedCoverScopeMismatch {
        owner: usize,
        detail: &'static str,
    },
    Promotion {
        candidate: usize,
        error: ExactRuleCellPromotionError,
    },
    Campaign(CampaignError),
    Semantic(ExactCircuitSemanticError),
    OuterExtension {
        owner: usize,
        error: ExactCircuitOuterExtensionError,
    },
    Cover(ExactCircuitOwnerCoverError),
    CellSelection(RuleCellError),
    ContentOrder(StratumRegistryError),
    AuthorityMismatch {
        candidate: usize,
        detail: &'static str,
    },
    PairingInvariant(&'static str),
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

impl fmt::Display for ExactExecutableOwnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCandidates => formatter.write_str(
                "an exact semantic executable owner needs at least one canonical candidate",
            ),
            Self::EmptyOwners => {
                formatter.write_str("an executable owner cover needs at least one owner")
            }
            Self::WrongContext => {
                formatter.write_str("executable-owner compilation uses another indexed context")
            }
            Self::CoverNotClosed { obstruction } => write!(
                formatter,
                "an incomplete executable-owner cover cannot be sealed: {obstruction:?}"
            ),
            Self::ClosedCoverScopeMismatch { owner, detail } => write!(
                formatter,
                "executable owner {owner} is outside the closed cover scope: {detail}"
            ),
            Self::Promotion { candidate, error } => {
                write!(
                    formatter,
                    "exact candidate {candidate} promotion failed: {error}"
                )
            }
            Self::Campaign(error) => write!(formatter, "canonical owner campaign failed: {error}"),
            Self::Semantic(error) => error.fmt(formatter),
            Self::OuterExtension { owner, error } => {
                write!(
                    formatter,
                    "exact executable owner {owner} cannot extend: {error}"
                )
            }
            Self::Cover(error) => error.fmt(formatter),
            Self::CellSelection(error) => {
                write!(
                    formatter,
                    "paired executable-cell selection failed: {error}"
                )
            }
            Self::ContentOrder(error) => error.fmt(formatter),
            Self::AuthorityMismatch { candidate, detail } => write!(
                formatter,
                "exact candidate {candidate} lost its retained authority: {detail}"
            ),
            Self::PairingInvariant(detail) => {
                write!(
                    formatter,
                    "semantic/executable pairing invariant failed: {detail}"
                )
            }
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
        }
    }
}

impl std::error::Error for ExactExecutableOwnerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Promotion { error, .. } => Some(error),
            Self::Campaign(error) => Some(error),
            Self::Semantic(error) => Some(error),
            Self::OuterExtension { error, .. } => Some(error),
            Self::Cover(error) => Some(error),
            Self::CellSelection(error) => Some(error),
            Self::ContentOrder(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CampaignError> for ExactExecutableOwnerError {
    fn from(value: CampaignError) -> Self {
        Self::Campaign(value)
    }
}

impl From<ExactCircuitSemanticError> for ExactExecutableOwnerError {
    fn from(value: ExactCircuitSemanticError) -> Self {
        Self::Semantic(value)
    }
}

impl From<ExactCircuitOwnerCoverError> for ExactExecutableOwnerError {
    fn from(value: ExactCircuitOwnerCoverError) -> Self {
        Self::Cover(value)
    }
}

impl From<RuleCellError> for ExactExecutableOwnerError {
    fn from(value: RuleCellError) -> Self {
        Self::CellSelection(value)
    }
}

impl From<StratumRegistryError> for ExactExecutableOwnerError {
    fn from(value: StratumRegistryError) -> Self {
        Self::ContentOrder(value)
    }
}
