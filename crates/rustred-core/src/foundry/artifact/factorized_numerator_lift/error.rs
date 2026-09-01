//! Typed failures at the factorized-numerator routing boundary.

use std::fmt;

use crate::algebra::matrix::SymbolicaCoefficientMatrixError;
use crate::family::{IntegralFamilyError, IntegralKeyError};
use crate::sector;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FactorizedNumeratorLiftError {
    Family(IntegralFamilyError),
    Matrix(SymbolicaCoefficientMatrixError),
    IntegralKey(IntegralKeyError),
    Sector(sector::Error),
    UnsupportedExternalKinematics {
        external_count: usize,
    },
    UnauthenticatedFactorizationRule,
    WrongFactorizationFamily,
    WrongRuleArity {
        expected: usize,
        actual: usize,
    },
    MalformedLoopBasis {
        expected_dimension: usize,
        actual_dimension: usize,
        expected_entries: usize,
        actual_entries: usize,
    },
    LoopBasisEntryOverflow {
        entry: usize,
    },
    NonUnimodularLoopBasis,
    RelationReplayFailure {
        denominator: usize,
        component: usize,
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
    UnitImageCollision {
        image: usize,
    },
    WrongTargetArity {
        expected: usize,
        actual: usize,
    },
    OutsideApplicationDomain {
        position: usize,
        power: i64,
        active: bool,
    },
    AffineRoutingRequired {
        position: usize,
        power: i64,
    },
    ForeignAuxiliaryState,
    EmptyAuxiliaryState,
    RoutedPowerUnderflow {
        position: usize,
        power: i64,
    },
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for FactorizedNumeratorLiftError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Family(error) => error.fmt(formatter),
            Self::Matrix(error) => error.fmt(formatter),
            Self::IntegralKey(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
            Self::UnsupportedExternalKinematics { external_count } => write!(
                formatter,
                "factorized numerator routing currently requires a vacuum family, received {external_count} external momenta"
            ),
            Self::UnauthenticatedFactorizationRule => formatter.write_str(
                "factorized numerator routing requires an installer-authenticated factorization rule",
            ),
            Self::WrongFactorizationFamily => formatter.write_str(
                "factorized numerator routing received a rule installed for another family",
            ),
            Self::WrongRuleArity { expected, actual } => write!(
                formatter,
                "factorization domain arity is {actual}, expected {expected}"
            ),
            Self::MalformedLoopBasis {
                expected_dimension,
                actual_dimension,
                expected_entries,
                actual_entries,
            } => write!(
                formatter,
                "factorization loop basis has dimension {actual_dimension} and {actual_entries} entries, expected dimension {expected_dimension} and {expected_entries} entries"
            ),
            Self::LoopBasisEntryOverflow { entry } => write!(
                formatter,
                "row-sign gauge negation overflows loop-basis entry {entry}"
            ),
            Self::NonUnimodularLoopBasis => formatter.write_str(
                "Symbolica replayed a factorization loop-basis determinant other than +1 or -1",
            ),
            Self::RelationReplayFailure {
                denominator,
                component,
            } => write!(
                formatter,
                "affine denominator relation {denominator} failed exact replay at component {component}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(formatter, "could not reserve {requested} {resource}"),
            Self::UnitImageCollision { image } => write!(
                formatter,
                "two transformed denominators have the same canonical unit image {image}"
            ),
            Self::WrongTargetArity { expected, actual } => write!(
                formatter,
                "factorized numerator target has arity {actual}, expected {expected}"
            ),
            Self::OutsideApplicationDomain {
                position,
                power,
                active,
            } => write!(
                formatter,
                "target power {power} at position {position} lies outside the {} factorization sector",
                if *active { "active" } else { "inactive" }
            ),
            Self::AffineRoutingRequired { position, power } => write!(
                formatter,
                "source power {power} at affine position {position} requires a numerator-lift action"
            ),
            Self::ForeignAuxiliaryState => formatter.write_str(
                "factorized numerator auxiliary state belongs to another compiled action",
            ),
            Self::EmptyAuxiliaryState => formatter
                .write_str("factorized numerator auxiliary recurrence is already exhausted"),
            Self::RoutedPowerUnderflow { position, power } => write!(
                formatter,
                "routed power {power} at position {position} cannot be lowered by one"
            ),
            Self::Invariant { detail } => {
                write!(
                    formatter,
                    "factorized numerator routing invariant failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for FactorizedNumeratorLiftError {}

impl From<IntegralFamilyError> for FactorizedNumeratorLiftError {
    fn from(value: IntegralFamilyError) -> Self {
        Self::Family(value)
    }
}

impl From<SymbolicaCoefficientMatrixError> for FactorizedNumeratorLiftError {
    fn from(value: SymbolicaCoefficientMatrixError) -> Self {
        Self::Matrix(value)
    }
}

impl From<IntegralKeyError> for FactorizedNumeratorLiftError {
    fn from(value: IntegralKeyError) -> Self {
        Self::IntegralKey(value)
    }
}

impl From<sector::Error> for FactorizedNumeratorLiftError {
    fn from(value: sector::Error) -> Self {
        Self::Sector(value)
    }
}
