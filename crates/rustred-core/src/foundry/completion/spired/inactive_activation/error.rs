use std::fmt;

use crate::algebra::IndexedAlgebraError;

/// Failure to construct non-authoritative inactive-line activation geometry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredInactiveActivationError {
    WrongContext,
    GuardedParentGeometry {
        guard_branches: usize,
    },
    DuplicatePhysicalColumn {
        physical_column: usize,
    },
    WrongShiftArity {
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
    Sector(crate::sector::Error),
    IndexedAlgebra(IndexedAlgebraError),
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for SpiredInactiveActivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongContext => formatter.write_str(
                "inactive-activation analysis uses a different indexed coefficient context",
            ),
            Self::GuardedParentGeometry { guard_branches } => write!(
                formatter,
                "inactive-activation equality analysis received a parent with {guard_branches} guard branches"
            ),
            Self::DuplicatePhysicalColumn { physical_column } => write!(
                formatter,
                "inactive-activation replay repeats physical column {physical_column}"
            ),
            Self::WrongShiftArity { expected, actual } => write!(
                formatter,
                "inactive-activation shift has arity {actual}, expected {expected}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "inactive-activation {resource} count overflowed")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "inactive-activation {resource} requires {requested}, limit is {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} bounded entries for inactive-activation {resource}"
            ),
            Self::Sector(error) => write!(formatter, "inactive-activation domain error: {error}"),
            Self::IndexedAlgebra(error) => {
                write!(formatter, "inactive-activation coefficient error: {error}")
            }
            Self::Invariant { detail } => {
                write!(formatter, "inactive-activation invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for SpiredInactiveActivationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sector(error) => Some(error),
            Self::IndexedAlgebra(error) => Some(error),
            _ => None,
        }
    }
}

impl From<crate::sector::Error> for SpiredInactiveActivationError {
    fn from(value: crate::sector::Error) -> Self {
        Self::Sector(value)
    }
}
