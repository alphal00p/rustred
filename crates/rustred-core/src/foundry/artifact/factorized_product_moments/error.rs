//! Typed failures at the cold product-moment boundary.

use std::fmt;

use crate::algebra::ExactAlgebraError;
use crate::family::{IntegralFamilyError, IntegralKeyError};
use crate::reduction::ReductionError;

use super::super::factorized_numerator_lift::FactorizedNumeratorLiftError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FactorizedProductMomentError {
    Routing(FactorizedNumeratorLiftError),
    ExactAlgebra(ExactAlgebraError),
    Family(IntegralFamilyError),
    IntegralKey(IntegralKeyError),
    DependencyReduction(ReductionError),
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
    WrongTargetArity {
        expected: usize,
        actual: usize,
    },
    OutsideFactorizedSector {
        position: usize,
        power: i64,
        active: bool,
    },
    WrongMonomialWidth {
        component: &'static str,
        expected: usize,
        actual: usize,
    },
    NonpositiveActivePower {
        vector: usize,
        power: i64,
    },
    NativePolynomialExponentLimit {
        requested: u64,
        limit: u32,
    },
    NativePolynomialPanic,
    NativePolynomialSupportExceeded {
        actual: usize,
        limit: usize,
    },
    RankCoefficientOverflow {
        rank: u64,
    },
    RadialShiftOverflow {
        denominator_power: i64,
        shift: u64,
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

impl fmt::Display for FactorizedProductMomentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Routing(error) => error.fmt(formatter),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::Family(error) => error.fmt(formatter),
            Self::IntegralKey(error) => error.fmt(formatter),
            Self::DependencyReduction(error) => error.fmt(formatter),
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
            Self::WrongTargetArity { expected, actual } => write!(
                formatter,
                "product-moment target has arity {actual}, expected {expected}"
            ),
            Self::OutsideFactorizedSector {
                position,
                power,
                active,
            } => write!(
                formatter,
                "target power {power} at position {position} lies outside the {} product sector",
                if *active { "active" } else { "inactive" }
            ),
            Self::WrongMonomialWidth {
                component,
                expected,
                actual,
            } => write!(
                formatter,
                "product monomial {component} width is {actual}, expected {expected}"
            ),
            Self::NonpositiveActivePower { vector, power } => write!(
                formatter,
                "product vector {vector} has nonpositive denominator power {power}"
            ),
            Self::NativePolynomialExponentLimit { requested, limit } => write!(
                formatter,
                "product numerator power {requested} exceeds Symbolica's sparse-polynomial exponent limit {limit}"
            ),
            Self::NativePolynomialPanic => formatter.write_str(
                "Symbolica panicked while expanding a product numerator polynomial",
            ),
            Self::NativePolynomialSupportExceeded { actual, limit } => write!(
                formatter,
                "Symbolica retained {actual} product numerator terms, exceeding the configured limit {limit}"
            ),
            Self::RankCoefficientOverflow { rank } => write!(
                formatter,
                "angular or radial rank {rank} cannot be represented in an exact i64 coefficient"
            ),
            Self::RadialShiftOverflow {
                denominator_power,
                shift,
            } => write!(
                formatter,
                "radial shift {shift} cannot be subtracted from denominator power {denominator_power}"
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

impl From<ReductionError> for FactorizedProductMomentError {
    fn from(value: ReductionError) -> Self {
        Self::DependencyReduction(value)
    }
}
