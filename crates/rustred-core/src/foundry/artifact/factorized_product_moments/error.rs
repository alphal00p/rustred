//! Typed failures at the cold product-moment boundary.

use std::fmt;

use crate::algebra::ExactAlgebraError;
use crate::family::{IntegralFamilyError, IntegralKeyError};
use crate::sector::Error as SectorError;

use super::super::factorized_numerator_lift::FactorizedNumeratorLiftError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FactorizedProductMomentError {
    Routing(FactorizedNumeratorLiftError),
    ExactAlgebra(ExactAlgebraError),
    Family(IntegralFamilyError),
    IntegralKey(IntegralKeyError),
    Sector(SectorError),
    WrongFamily,
    MissingFactorizationRule {
        ordinal: usize,
    },
    MissingDependency {
        ordinal: usize,
    },
    DependencyMasterCount {
        ordinal: usize,
        count: usize,
    },
    UnsupportedDependencySemantic {
        ordinal: usize,
    },
    DependencyDimensionMismatch {
        ordinal: usize,
    },
    UnsupportedParentPowerShift {
        position: usize,
    },
    InvalidMasterEmbedding,
    UnsupportedFactorCount {
        expected: usize,
        actual: usize,
    },
    UnsupportedCorrelatedFactorCount {
        count: usize,
    },
    UnsupportedSingletonFactorCount {
        count: usize,
    },
    UnsupportedFactorShape {
        factor: usize,
        detail: &'static str,
    },
    UnsupportedShiftCertificate {
        detail: &'static str,
    },
    IncompleteFactorCover,
    UnsupportedCoordinate {
        coordinate: usize,
    },
    DuplicateScalarProductCoordinate {
        left: usize,
        right: usize,
    },
    MissingScalarProductCoordinate {
        left: usize,
        right: usize,
    },
    NonUnitRadialDenominator {
        vector: usize,
        parent_position: usize,
    },
    NonconstantAffineCoefficient {
        denominator: usize,
    },
    NonintegerAffineCoefficient {
        denominator: usize,
    },
    WrongMonomialWidth {
        component: &'static str,
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
    Invariant {
        detail: &'static str,
    },
}

impl FactorizedProductMomentError {
    /// Whether an authenticated generic corner factorization is sound but
    /// outside the complete numerator-moment compiler's current semantic
    /// envelope. Every other failure is a hard cold-boundary error and must
    /// never be hidden by falling back to the smaller corner domain.
    pub(super) fn is_unsupported_program_shape(&self) -> bool {
        matches!(
            self,
            Self::Routing(
                FactorizedNumeratorLiftError::UnsupportedExternalKinematics { .. }
                    | FactorizedNumeratorLiftError::NonconstantExpansionCoefficient
            ) | Self::DependencyMasterCount { .. }
                | Self::UnsupportedDependencySemantic { .. }
                | Self::UnsupportedParentPowerShift { .. }
                | Self::UnsupportedFactorCount { .. }
                | Self::UnsupportedCorrelatedFactorCount { .. }
                | Self::UnsupportedSingletonFactorCount { .. }
                | Self::UnsupportedFactorShape { .. }
                | Self::UnsupportedShiftCertificate { .. }
                | Self::IncompleteFactorCover
                | Self::UnsupportedCoordinate { .. }
                | Self::DuplicateScalarProductCoordinate { .. }
                | Self::MissingScalarProductCoordinate { .. }
                | Self::NonUnitRadialDenominator { .. }
                | Self::NonconstantAffineCoefficient { .. }
                | Self::NonintegerAffineCoefficient { .. }
        )
    }
}

impl fmt::Display for FactorizedProductMomentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Routing(error) => error.fmt(formatter),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::Family(error) => error.fmt(formatter),
            Self::IntegralKey(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
            Self::WrongFamily => formatter.write_str(
                "factorized product moment received a family other than its compiled family",
            ),
            Self::MissingFactorizationRule { ordinal } => write!(
                formatter,
                "terminal authority has no factorization rule {ordinal}"
            ),
            Self::MissingDependency { ordinal } => {
                write!(formatter, "terminal authority has no dependency {ordinal}")
            }
            Self::DependencyMasterCount { ordinal, count } => write!(
                formatter,
                "one-coordinate dependency {ordinal} has {count} masters, expected one"
            ),
            Self::UnsupportedDependencySemantic { ordinal } => write!(
                formatter,
                "factorization dependency {ordinal} does not provide the authenticated closed-block semantics required by this product lane"
            ),
            Self::DependencyDimensionMismatch { ordinal } => write!(
                formatter,
                "factorization dependency {ordinal} uses a dimension different from the parent family"
            ),
            Self::UnsupportedParentPowerShift { position } => write!(
                formatter,
                "parent power shift {position} is nonzero and cannot enter the unshifted K1 product prototype"
            ),
            Self::InvalidMasterEmbedding => formatter.write_str(
                "the product rule does not contain the complete exact Cartesian master embedding",
            ),
            Self::UnsupportedFactorCount { expected, actual } => write!(
                formatter,
                "the current product-moment fixture admits {expected} one-loop factors, received {actual}"
            ),
            Self::UnsupportedCorrelatedFactorCount { count } => write!(
                formatter,
                "the product-moment lane admits exactly one correlated dependency block, received {count}"
            ),
            Self::UnsupportedSingletonFactorCount { count } => write!(
                formatter,
                "a correlated product-moment lane needs at least one independent singleton block, received {count}"
            ),
            Self::UnsupportedFactorShape { factor, detail } => {
                write!(formatter, "factorized product factor {factor} is unsupported: {detail}")
            }
            Self::UnsupportedShiftCertificate { detail } => write!(
                formatter,
                "factorized product dependency-shift certificate is unsupported: {detail}"
            ),
            Self::IncompleteFactorCover => formatter.write_str(
                "dependency factors do not form a disjoint cover of the active denominators and transformed loops",
            ),
            Self::UnsupportedCoordinate { coordinate } => write!(
                formatter,
                "factorized product moment does not support scalar-product coordinate {coordinate}"
            ),
            Self::DuplicateScalarProductCoordinate { left, right } => write!(
                formatter,
                "scalar-product coordinate ({left},{right}) is duplicated"
            ),
            Self::MissingScalarProductCoordinate { left, right } => write!(
                formatter,
                "scalar-product coordinate ({left},{right}) is missing"
            ),
            Self::NonUnitRadialDenominator {
                vector,
                parent_position,
            } => write!(
                formatter,
                "factor {vector} parent denominator {parent_position} is not exactly q_{vector}^2-1 in the authenticated basis"
            ),
            Self::NonconstantAffineCoefficient { denominator } => write!(
                formatter,
                "transformed denominator {denominator} has a parameter-dependent affine coefficient"
            ),
            Self::NonintegerAffineCoefficient { denominator } => write!(
                formatter,
                "transformed denominator {denominator} has a noninteger affine coefficient"
            ),
            Self::WrongMonomialWidth {
                component,
                expected,
                actual,
            } => write!(
                formatter,
                "product monomial {component} width is {actual}, expected {expected}"
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
            Self::Invariant { detail } => {
                write!(formatter, "factorized product-moment invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for FactorizedProductMomentError {}

impl From<FactorizedNumeratorLiftError> for FactorizedProductMomentError {
    fn from(value: FactorizedNumeratorLiftError) -> Self {
        Self::Routing(value)
    }
}

impl From<ExactAlgebraError> for FactorizedProductMomentError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

impl From<IntegralKeyError> for FactorizedProductMomentError {
    fn from(value: IntegralKeyError) -> Self {
        Self::IntegralKey(value)
    }
}

impl From<IntegralFamilyError> for FactorizedProductMomentError {
    fn from(value: IntegralFamilyError) -> Self {
        Self::Family(value)
    }
}

impl From<SectorError> for FactorizedProductMomentError {
    fn from(value: SectorError) -> Self {
        Self::Sector(value)
    }
}
