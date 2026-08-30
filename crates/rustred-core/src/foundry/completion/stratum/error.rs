use std::fmt;

use crate::algebra::IndexedAlgebraError;
use crate::sector;

/// Typed failures while binding physical columns to one exact stratum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StratumRegistryError {
    EmptyIdentity {
        identity: &'static str,
    },
    DuplicateGuardPredicate {
        predicate: String,
    },
    ContradictoryGuardPredicate {
        predicate: String,
    },
    ZeroGuardPolynomial,
    WrongFrameFamily,
    WrongFrameContext,
    WrongOwnerFamily,
    WrongOwnerContext,
    WrongFrameSector,
    WrongOwnerArity {
        owner: usize,
        expected: usize,
        actual: usize,
    },
    TargetColumnOutOfRange {
        target: usize,
        columns: usize,
    },
    UncoveredPhysicalShift {
        column: usize,
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
    IndexedAlgebra(IndexedAlgebraError),
    Sector(sector::Error),
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for StratumRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIdentity { identity } => {
                write!(formatter, "decorated stratum has an empty {identity}")
            }
            Self::DuplicateGuardPredicate { predicate } => write!(
                formatter,
                "decorated stratum repeats guard predicate {predicate:?}"
            ),
            Self::ContradictoryGuardPredicate { predicate } => write!(
                formatter,
                "decorated stratum assigns both branches to guard predicate {predicate:?}"
            ),
            Self::ZeroGuardPolynomial => {
                formatter.write_str("an exact guard predicate polynomial is identically zero")
            }
            Self::WrongFrameFamily => formatter
                .write_str("decorated stratum and physical frame belong to different families"),
            Self::WrongFrameContext => formatter.write_str(
                "decorated stratum and physical frame use different coefficient contexts",
            ),
            Self::WrongOwnerFamily => formatter.write_str(
                "immutable owner snapshot and decorated stratum belong to different families",
            ),
            Self::WrongOwnerContext => formatter.write_str(
                "immutable owner snapshot and decorated stratum use different coefficient contexts",
            ),
            Self::WrongFrameSector => formatter
                .write_str("decorated stratum domain and physical frame use different sectors"),
            Self::WrongOwnerArity {
                owner,
                expected,
                actual,
            } => write!(
                formatter,
                "immutable owner {owner} has arity {actual}, expected {expected}"
            ),
            Self::TargetColumnOutOfRange { target, columns } => write!(
                formatter,
                "target physical column {target} is outside the {columns}-column frame"
            ),
            Self::UncoveredPhysicalShift { column } => write!(
                formatter,
                "decorated stratum does not keep physical column {column} representable"
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
                "{resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}"
            ),
            Self::IndexedAlgebra(error) => {
                write!(
                    formatter,
                    "exact guard polynomial authentication failed: {error}"
                )
            }
            Self::Sector(error) => write!(formatter, "sector proof failed: {error}"),
            Self::Invariant { detail } => write!(
                formatter,
                "decorated-stratum column registry invariant failed: {detail}"
            ),
        }
    }
}

impl std::error::Error for StratumRegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IndexedAlgebra(error) => Some(error),
            Self::Sector(error) => Some(error),
            _ => None,
        }
    }
}

impl From<IndexedAlgebraError> for StratumRegistryError {
    fn from(error: IndexedAlgebraError) -> Self {
        Self::IndexedAlgebra(error)
    }
}

impl From<sector::Error> for StratumRegistryError {
    fn from(error: sector::Error) -> Self {
        Self::Sector(error)
    }
}
