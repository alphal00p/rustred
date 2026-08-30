use std::fmt;

use crate::algebra::IndexedAlgebraError;
use crate::family::IntegralKeyError;
use crate::identity::{IdentityConditionError, ParametricRelationError};
use crate::sector;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleCellError {
    EmptySourceSelection,
    SourceOrdinalOutOfRange {
        ordinal: usize,
        available: usize,
    },
    DuplicateSourceOrdinal {
        ordinal: usize,
    },
    ForeignFamily,
    ForeignContext,
    SourceReplayMismatch {
        contribution: usize,
    },
    WrongApplicationArity {
        expected: usize,
        actual: usize,
    },
    ApplicationSectorMismatch,
    ApplicationNotTightened,
    FixedRestrictionMismatch {
        position: usize,
    },
    DuplicateFixedPosition {
        position: usize,
    },
    PrunedTermOutOfRange {
        ordinal: usize,
        available: usize,
    },
    DuplicatePrunedTerm {
        ordinal: usize,
    },
    PrunedTermNotZero {
        ordinal: usize,
    },
    EmptyRetainedRule,
    GuardIdenticallyZero {
        ordinal: usize,
    },
    UnsupportedMultivariateGuardLocus {
        ordinal: usize,
    },
    GuardAlgebra {
        ordinal: usize,
        source: IndexedAlgebraError,
    },
    GuardVanishesInApplicationDomain {
        ordinal: usize,
        position: usize,
        value: i64,
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
    IndexOverflow {
        position: usize,
    },
    ProjectionDomainArity {
        expected: usize,
        actual: usize,
    },
    ProjectionFixedCoordinateNotSingleton {
        position: usize,
        value: i64,
    },
    ProjectionZeroSectorArity {
        ordinal: usize,
        expected: usize,
        actual: usize,
    },
    ProjectionHasNoDomainStabilizer,
    IndexedAlgebra(IndexedAlgebraError),
    IdentityCondition(IdentityConditionError),
    Relation(ParametricRelationError),
    IntegralKey(IntegralKeyError),
    Sector(sector::Error),
}

impl fmt::Display for RuleCellError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySourceSelection => formatter.write_str("a rule cell needs source views"),
            Self::SourceOrdinalOutOfRange { ordinal, available } => write!(
                formatter,
                "source-view ordinal {ordinal} is outside {available} translated sources"
            ),
            Self::DuplicateSourceOrdinal { ordinal } => {
                write!(
                    formatter,
                    "source-view ordinal {ordinal} was selected twice"
                )
            }
            Self::ForeignFamily => {
                formatter.write_str("rule cell sources belong to another family")
            }
            Self::ForeignContext => {
                formatter.write_str("rule cell sources belong to another indexed context")
            }
            Self::SourceReplayMismatch { contribution } => write!(
                formatter,
                "rule source contribution {contribution} does not match its immutable source view"
            ),
            Self::WrongApplicationArity { expected, actual } => write!(
                formatter,
                "rule-cell application arity is {actual}, expected {expected}"
            ),
            Self::ApplicationSectorMismatch => {
                formatter.write_str("rule-cell application sector differs from its proof sector")
            }
            Self::ApplicationNotTightened => formatter
                .write_str("a tightened rule cell extends outside its original proof domain"),
            Self::FixedRestrictionMismatch { position } => write!(
                formatter,
                "fixed restriction at position {position} is not the matching singleton application bound"
            ),
            Self::DuplicateFixedPosition { position } => write!(
                formatter,
                "fixed restriction position {position} occurs twice"
            ),
            Self::PrunedTermOutOfRange { ordinal, available } => write!(
                formatter,
                "pruned RHS ordinal {ordinal} is outside {available} terms"
            ),
            Self::DuplicatePrunedTerm { ordinal } => {
                write!(formatter, "RHS ordinal {ordinal} is pruned twice")
            }
            Self::PrunedTermNotZero { ordinal } => write!(
                formatter,
                "RHS ordinal {ordinal} is not identically zero under the fixed restrictions"
            ),
            Self::EmptyRetainedRule => {
                formatter.write_str("rule-cell refinement pruned every RHS term")
            }
            Self::GuardIdenticallyZero { ordinal } => {
                write!(formatter, "rule guard {ordinal} is identically zero")
            }
            Self::UnsupportedMultivariateGuardLocus { ordinal } => write!(
                formatter,
                "rule guard {ordinal} has an unsupported multivariate exceptional locus"
            ),
            Self::GuardAlgebra { ordinal, source } => {
                write!(formatter, "rule guard {ordinal} algebra failed: {source}")
            }
            Self::GuardVanishesInApplicationDomain {
                ordinal,
                position,
                value,
            } => write!(
                formatter,
                "rule guard {ordinal} vanishes at index {position} = {value} inside the application domain"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "rule-cell {resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "rule-cell {resource} requested {requested}, limit is {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for rule-cell {resource}"
            ),
            Self::IndexOverflow { position } => write!(
                formatter,
                "rule-cell target arithmetic overflowed at position {position}"
            ),
            Self::ProjectionDomainArity { expected, actual } => write!(
                formatter,
                "residual projection domain arity is {actual}, expected {expected}"
            ),
            Self::ProjectionFixedCoordinateNotSingleton { position, value } => write!(
                formatter,
                "residual projection fixes coordinate {position} to {value}, but its domain is not that singleton"
            ),
            Self::ProjectionZeroSectorArity {
                ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "residual projection zero sector {ordinal} has arity {actual}, expected {expected}"
            ),
            Self::ProjectionHasNoDomainStabilizer => formatter
                .write_str("the canonical group has no route stabilizing the residual base domain"),
            Self::IndexedAlgebra(error) => error.fmt(formatter),
            Self::IdentityCondition(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::IntegralKey(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RuleCellError {}

impl From<IndexedAlgebraError> for RuleCellError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::IndexedAlgebra(value)
    }
}
impl From<IdentityConditionError> for RuleCellError {
    fn from(value: IdentityConditionError) -> Self {
        Self::IdentityCondition(value)
    }
}
impl From<IntegralKeyError> for RuleCellError {
    fn from(value: IntegralKeyError) -> Self {
        Self::IntegralKey(value)
    }
}
impl From<ParametricRelationError> for RuleCellError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}
impl From<sector::Error> for RuleCellError {
    fn from(value: sector::Error) -> Self {
        Self::Sector(value)
    }
}
