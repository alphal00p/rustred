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
    WrongOwnerRouteCanonicalizer,
    WrongFrameSector,
    WrongOwnerArity {
        owner: usize,
        expected: usize,
        actual: usize,
    },
    EmptyClosedSectorLayerBatch,
    WrongClosedSectorLayerFamily {
        layer: usize,
    },
    WrongClosedSectorLayerContext {
        layer: usize,
    },
    WrongClosedSectorLayerPredecessor {
        layer: usize,
    },
    MixedClosedSectorLayerFrontier {
        layer: usize,
        expected_active_count: usize,
        actual_active_count: usize,
    },
    NonIncreasingClosedSectorLayerFrontier {
        previous_active_count: usize,
        incoming_active_count: usize,
    },
    DuplicateClosedSectorOwner {
        first_layer: usize,
        second_layer: usize,
    },
    TargetColumnOutOfRange {
        target: usize,
        columns: usize,
    },
    UncoveredPhysicalShift {
        column: usize,
    },
    InitialMaximalDomainMismatch,
    NonMonotoneMaximalDomain,
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
            Self::WrongOwnerRouteCanonicalizer => formatter.write_str(
                "immutable owner routes use a foreign or wrong-arity symmetry authority",
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
            Self::EmptyClosedSectorLayerBatch => {
                formatter.write_str("a closed-sector snapshot extension batch cannot be empty")
            }
            Self::WrongClosedSectorLayerFamily { layer } => write!(
                formatter,
                "closed-sector layer {layer} belongs to another family"
            ),
            Self::WrongClosedSectorLayerContext { layer } => write!(
                formatter,
                "closed-sector layer {layer} uses another coefficient context"
            ),
            Self::WrongClosedSectorLayerPredecessor { layer } => write!(
                formatter,
                "closed-sector layer {layer} does not retain the exact extension predecessor"
            ),
            Self::MixedClosedSectorLayerFrontier {
                layer,
                expected_active_count,
                actual_active_count,
            } => write!(
                formatter,
                "closed-sector layer {layer} has active count {actual_active_count}, expected the common frontier {expected_active_count}"
            ),
            Self::NonIncreasingClosedSectorLayerFrontier {
                previous_active_count,
                incoming_active_count,
            } => write!(
                formatter,
                "closed-sector frontier rank {incoming_active_count} does not strictly exceed the retained frontier rank {previous_active_count}"
            ),
            Self::DuplicateClosedSectorOwner {
                first_layer,
                second_layer,
            } => write!(
                formatter,
                "closed-sector layers {first_layer} and {second_layer} publish the same exact sector and ordering"
            ),
            Self::TargetColumnOutOfRange { target, columns } => write!(
                formatter,
                "target physical column {target} is outside the {columns}-column frame"
            ),
            Self::UncoveredPhysicalShift { column } => write!(
                formatter,
                "decorated stratum does not keep physical column {column} representable"
            ),
            Self::InitialMaximalDomainMismatch => formatter.write_str(
                "the initial decorated-stratum anchor is not the maximal domain of the first fresh frame",
            ),
            Self::NonMonotoneMaximalDomain => formatter.write_str(
                "a later fresh frame would widen beyond its immediately preceding maximal stratum",
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
