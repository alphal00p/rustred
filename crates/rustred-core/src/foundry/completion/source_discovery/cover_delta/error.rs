use std::fmt;

use crate::foundry::completion::CompletionGeometryError;

use super::super::StagedSectorClosureError;
use super::ExactOwnerLedgerRevision;

/// Hard failure while staging or exactly comparing one canonical proposal.
#[derive(Debug)]
pub(crate) enum ExactOwnerCoverDeltaError {
    Staging(StagedSectorClosureError),
    Geometry(CompletionGeometryError),
    NonMonotoneExactCover,
    ForeignLedgerSnapshotIdentity,
    StaleLedgerSnapshotIdentity {
        expected: ExactOwnerLedgerRevision,
        actual: ExactOwnerLedgerRevision,
    },
    LedgerRevisionOverflow,
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

impl fmt::Display for ExactOwnerCoverDeltaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Staging(error) => error.fmt(formatter),
            Self::Geometry(error) => error.fmt(formatter),
            Self::NonMonotoneExactCover => formatter.write_str(
                "adding a canonical exact owner enlarged the compiler's uncovered box union",
            ),
            Self::ForeignLedgerSnapshotIdentity => formatter.write_str(
                "the exact owner-ledger snapshot identity belongs to another ledger authority",
            ),
            Self::StaleLedgerSnapshotIdentity { expected, actual } => write!(
                formatter,
                "the exact owner-ledger snapshot revision {} is stale; current revision is {}",
                actual.get(),
                expected.get(),
            ),
            Self::LedgerRevisionOverflow => {
                formatter.write_str("the exact owner-ledger revision overflowed u64")
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

impl std::error::Error for ExactOwnerCoverDeltaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Staging(error) => Some(error),
            Self::Geometry(error) => Some(error),
            _ => None,
        }
    }
}

impl From<StagedSectorClosureError> for ExactOwnerCoverDeltaError {
    fn from(value: StagedSectorClosureError) -> Self {
        Self::Staging(value)
    }
}

impl From<CompletionGeometryError> for ExactOwnerCoverDeltaError {
    fn from(value: CompletionGeometryError) -> Self {
        Self::Geometry(value)
    }
}
