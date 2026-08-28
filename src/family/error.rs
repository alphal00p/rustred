//! Typed integral-family failures.

use std::fmt;

use crate::algebra::ExactAlgebraError;

use super::model::CoefficientLocation;

/// Typed construction and lookup failures for [`super::IntegralFamily`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntegralFamilyError {
    NoLoopMomenta,
    ScalarProductCountOverflow {
        loops: usize,
        externals: usize,
    },
    EmptyMomentumLabel {
        role: &'static str,
        index: usize,
    },
    DuplicateMomentumLabel {
        role: &'static str,
        label: String,
    },
    MomentumLabelOverlap {
        label: String,
    },
    WrongDenominatorCount {
        expected: usize,
        actual: usize,
    },
    WrongDenominatorRowSize {
        denominator: usize,
        expected: usize,
        actual: usize,
    },
    WrongPowerShiftCount {
        expected: usize,
        actual: usize,
    },
    WrongExternalGramRowCount {
        expected: usize,
        actual: usize,
    },
    WrongExternalGramColumnCount {
        row: usize,
        expected: usize,
        actual: usize,
    },
    AsymmetricExternalGram {
        row: usize,
        column: usize,
    },
    ForeignCoefficientContext {
        location: CoefficientLocation,
    },
    InvalidCoefficient {
        location: CoefficientLocation,
        error: ExactAlgebraError,
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
    ExactAlgebra(ExactAlgebraError),
    MatrixDimensionOverflow {
        size: usize,
    },
    SingularDenominatorBasis,
    LoopMomentumOutOfRange {
        index: usize,
        loops: usize,
    },
    ExternalMomentumOutOfRange {
        index: usize,
        externals: usize,
    },
    ScalarProductOutOfRange {
        index: usize,
        scalar_products: usize,
    },
    DenominatorOutOfRange {
        index: usize,
        denominators: usize,
    },
    InternalVerificationFailure {
        detail: String,
    },
}

impl fmt::Display for IntegralFamilyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLoopMomenta => {
                formatter.write_str("an integral family needs at least one loop momentum")
            }
            Self::ScalarProductCountOverflow { loops, externals } => write!(
                formatter,
                "the scalar-product count for {loops} loops and {externals} external momenta does not fit in usize"
            ),
            Self::EmptyMomentumLabel { role, index } => {
                write!(formatter, "{role} momentum {index} has an empty label")
            }
            Self::DuplicateMomentumLabel { role, label } => {
                write!(formatter, "{role} momentum label {label:?} is repeated")
            }
            Self::MomentumLabelOverlap { label } => write!(
                formatter,
                "momentum label {label:?} is used for both a loop and an external momentum"
            ),
            Self::WrongDenominatorCount { expected, actual } => write!(
                formatter,
                "a complete affine basis needs {expected} denominators, received {actual}"
            ),
            Self::WrongDenominatorRowSize {
                denominator,
                expected,
                actual,
            } => write!(
                formatter,
                "denominator {denominator} has {actual} scalar-product coefficients, expected {expected}"
            ),
            Self::WrongPowerShiftCount { expected, actual } => write!(
                formatter,
                "received {actual} power shifts for a basis of size {expected}"
            ),
            Self::WrongExternalGramRowCount { expected, actual } => write!(
                formatter,
                "external Gram matrix has {actual} rows, expected {expected}"
            ),
            Self::WrongExternalGramColumnCount {
                row,
                expected,
                actual,
            } => write!(
                formatter,
                "external Gram row {row} has {actual} entries, expected {expected}"
            ),
            Self::AsymmetricExternalGram { row, column } => write!(
                formatter,
                "external Gram entries ({row},{column}) and ({column},{row}) differ"
            ),
            Self::ForeignCoefficientContext { location } => write!(
                formatter,
                "coefficient at {location:?} does not use the family's exact Symbolica variable map"
            ),
            Self::InvalidCoefficient { location, error } => {
                write!(formatter, "invalid coefficient at {location:?}: {error}")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} bounded units for {resource}"
            ),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::MatrixDimensionOverflow { size } => write!(
                formatter,
                "an augmented denominator matrix of size {size} cannot be represented"
            ),
            Self::SingularDenominatorBasis => formatter
                .write_str("the affine denominator coefficient matrix is identically singular"),
            Self::LoopMomentumOutOfRange { index, loops } => write!(
                formatter,
                "loop momentum {index} is outside a family with {loops} loops"
            ),
            Self::ExternalMomentumOutOfRange { index, externals } => write!(
                formatter,
                "external momentum {index} is outside a family with {externals} external momenta"
            ),
            Self::ScalarProductOutOfRange {
                index,
                scalar_products,
            } => write!(
                formatter,
                "scalar-product coordinate {index} is outside a basis of size {scalar_products}"
            ),
            Self::DenominatorOutOfRange {
                index,
                denominators,
            } => write!(
                formatter,
                "denominator {index} is outside a basis of size {denominators}"
            ),
            Self::InternalVerificationFailure { detail } => {
                write!(formatter, "exact family replay failed: {detail}")
            }
        }
    }
}

impl std::error::Error for IntegralFamilyError {}

impl From<ExactAlgebraError> for IntegralFamilyError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

pub(super) fn check_family_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), IntegralFamilyError> {
    if requested > limit {
        Err(IntegralFamilyError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
