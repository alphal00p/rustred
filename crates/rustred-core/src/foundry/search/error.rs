use std::fmt;

use crate::identity::TranslatedSourceError;

/// Typed failure while planning one bounded exact-sector search diamond.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SectorSearchError {
    DepthNotRepresentable {
        depth: usize,
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
    IntegralShift(TranslatedSourceError),
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for SectorSearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DepthNotRepresentable { depth } => write!(
                formatter,
                "sector-search depth {depth} cannot be represented by an i64 lattice offset"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requires {requested} units, exceeding limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} bounded entries for {resource}"
            ),
            Self::IntegralShift(error) => {
                write!(
                    formatter,
                    "could not retain a sector-search offset: {error}"
                )
            }
            Self::Invariant { detail } => {
                write!(formatter, "sector-search invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for SectorSearchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IntegralShift(error) => Some(error),
            _ => None,
        }
    }
}
