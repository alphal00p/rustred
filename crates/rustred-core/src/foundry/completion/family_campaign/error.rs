use std::fmt;

use crate::family::IntegralKeyError;
use crate::sector;
use crate::sector::symmetry::CanonicalizationError;

/// Typed failure while defining or enumerating one family coverage goal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FamilyCoverageError {
    NoPhysicalPropagators,
    WrongCanonicalizerFamily,
    WrongCanonicalizerArity {
        expected: usize,
        actual: usize,
    },
    SlotRolesNotSymmetryInvariant {
        group_element: usize,
        target_slot: usize,
        source_slot: usize,
    },
    PhysicalContractionCountOverflow {
        physical_slot_count: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    IntegralKey(IntegralKeyError),
    Sector(sector::Error),
    Canonicalization(CanonicalizationError),
}

impl fmt::Display for FamilyCoverageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPhysicalPropagators => formatter.write_str(
                "a complete physical-contraction goal needs at least one physical propagator",
            ),
            Self::WrongCanonicalizerFamily => formatter.write_str(
                "the physical-contraction goal and canonicalizer belong to different families",
            ),
            Self::WrongCanonicalizerArity { expected, actual } => write!(
                formatter,
                "family canonicalizer has arity {actual}, expected {expected}"
            ),
            Self::SlotRolesNotSymmetryInvariant {
                group_element,
                target_slot,
                source_slot,
            } => write!(
                formatter,
                "canonicalizer group element {group_element} routes source slot {source_slot} into target slot {target_slot} across the physical/auxiliary boundary"
            ),
            Self::PhysicalContractionCountOverflow {
                physical_slot_count,
            } => write!(
                formatter,
                "the complete downset of {physical_slot_count} physical slots is not representable by usize"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}"
            ),
            Self::IntegralKey(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
            Self::Canonicalization(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for FamilyCoverageError {}

impl From<IntegralKeyError> for FamilyCoverageError {
    fn from(value: IntegralKeyError) -> Self {
        Self::IntegralKey(value)
    }
}

impl From<sector::Error> for FamilyCoverageError {
    fn from(value: sector::Error) -> Self {
        Self::Sector(value)
    }
}

impl From<CanonicalizationError> for FamilyCoverageError {
    fn from(value: CanonicalizationError) -> Self {
        Self::Canonicalization(value)
    }
}
