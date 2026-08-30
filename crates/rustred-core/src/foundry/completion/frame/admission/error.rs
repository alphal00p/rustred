use std::fmt;

use crate::foundry::completion::stratum::StratumRegistryError;

/// Typed failures while rebinding an exact circuit to its exhaustive guard
/// refinement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExactGuardRefinementError {
    WrongContext,
    CircuitStratumMismatch,
    CircuitOwnerSnapshotMismatch,
    CircuitTargetMismatch,
    CircuitTargetShiftMismatch,
    PartitionVerification(StratumRegistryError),
    Stratum(StratumRegistryError),
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

impl fmt::Display for ExactGuardRefinementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongContext => formatter
                .write_str("exact guard refinement uses a different indexed coefficient context"),
            Self::CircuitStratumMismatch => formatter
                .write_str("exact circuit and target partition bind different parent strata"),
            Self::CircuitOwnerSnapshotMismatch => formatter.write_str(
                "exact circuit and target partition bind different lower-owner snapshots",
            ),
            Self::CircuitTargetMismatch => {
                formatter.write_str("exact circuit and target partition select different columns")
            }
            Self::CircuitTargetShiftMismatch => formatter.write_str(
                "exact circuit target shift differs from the target partition's physical column",
            ),
            Self::PartitionVerification(error) => {
                write!(formatter, "target partition verification failed: {error}")
            }
            Self::Stratum(error) => write!(formatter, "guard stratum construction failed: {error}"),
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
            Self::Invariant { detail } => {
                write!(
                    formatter,
                    "exact guard refinement invariant failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for ExactGuardRefinementError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PartitionVerification(error) | Self::Stratum(error) => Some(error),
            _ => None,
        }
    }
}
