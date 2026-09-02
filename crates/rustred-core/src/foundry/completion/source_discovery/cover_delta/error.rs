use std::fmt;

use crate::foundry::completion::CompletionGeometryError;

use super::super::{ExactExecutableOwnerError, StagedSectorClosureError};
use super::{ExactOwnerLedgerCoverStatus, ExactOwnerLedgerRevision};

/// Failure to consume one exact owner ledger into publication authority.
///
/// Ordinary discovery may leave a ledger owner-free or with an incomplete
/// compiler verdict. Neither state is an error while searching, but both must
/// be rejected explicitly at the consuming publication boundary.
#[derive(Debug)]
pub(crate) enum ExactOwnerLedgerSealError {
    NotClosed { status: ExactOwnerLedgerCoverStatus },
    ScopeMismatch { detail: &'static str },
    Executable(ExactExecutableOwnerError),
}

impl fmt::Display for ExactOwnerLedgerSealError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotClosed { status } => write!(
                formatter,
                "an exact owner ledger with status {status:?} cannot be sealed for publication"
            ),
            Self::ScopeMismatch { detail } => {
                write!(
                    formatter,
                    "exact owner-ledger closure scope mismatch: {detail}"
                )
            }
            Self::Executable(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ExactOwnerLedgerSealError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Executable(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ExactExecutableOwnerError> for ExactOwnerLedgerSealError {
    fn from(value: ExactExecutableOwnerError) -> Self {
        Self::Executable(value)
    }
}

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
