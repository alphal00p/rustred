//! Typed failures at the cold multi-affine expansion boundary.

use std::fmt;

use crate::algebra::ExactAlgebraError;
use crate::family::IntegralKeyError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MultiAffineNumeratorExpansionError {
    IntegralKey(IntegralKeyError),
    ExactAlgebra(ExactAlgebraError),
    WrongBaseArity {
        expected: usize,
        actual: usize,
    },
    WrongRelationArity {
        factor: usize,
        expected: usize,
        actual: usize,
    },
    NonconstantExpansionCoefficient {
        factor: usize,
        coefficient: usize,
    },
    NativeExponentLimit {
        factor: usize,
        requested: u64,
        limit: u32,
    },
    PowerShiftUnderflow {
        position: usize,
        power: i64,
        decrement: u64,
    },
    NativePolynomialPanic,
    NativePolynomialSupportExceeded {
        actual: usize,
        limit: usize,
    },
    NativeExponentWidth {
        expected: usize,
        actual: usize,
    },
    NativeExponentDegreeOverflow {
        position: usize,
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

impl fmt::Display for MultiAffineNumeratorExpansionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IntegralKey(error) => error.fmt(formatter),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::WrongBaseArity { expected, actual } => write!(
                formatter,
                "multi-affine base key has arity {actual}, expected {expected}"
            ),
            Self::WrongRelationArity {
                factor,
                expected,
                actual,
            } => write!(
                formatter,
                "multi-affine factor {factor} has {actual} denominator coefficients, expected {expected}"
            ),
            Self::NonconstantExpansionCoefficient {
                factor,
                coefficient,
            } => write!(
                formatter,
                "multi-affine factor {factor} coefficient {coefficient} is parameter-dependent; this bounded native expansion currently admits rational constants only"
            ),
            Self::NativeExponentLimit {
                factor,
                requested,
                limit,
            } => write!(
                formatter,
                "multi-affine factor {factor} power {requested} exceeds Symbolica's sparse-polynomial exponent limit {limit}"
            ),
            Self::PowerShiftUnderflow {
                position,
                power,
                decrement,
            } => write!(
                formatter,
                "multi-affine endpoint power {power} at position {position} cannot be lowered by {decrement}"
            ),
            Self::NativePolynomialPanic => formatter.write_str(
                "Symbolica panicked while expanding a multi-affine numerator polynomial",
            ),
            Self::NativePolynomialSupportExceeded { actual, limit } => write!(
                formatter,
                "Symbolica retained {actual} multi-affine monomials, exceeding the configured limit {limit}"
            ),
            Self::NativeExponentWidth { expected, actual } => write!(
                formatter,
                "Symbolica returned a multi-affine exponent row of width {actual}, expected {expected}"
            ),
            Self::NativeExponentDegreeOverflow { position } => write!(
                formatter,
                "aggregate multi-affine degree at denominator position {position} exceeds Symbolica's native exponent range"
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
                write!(
                    formatter,
                    "multi-affine expansion invariant failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for MultiAffineNumeratorExpansionError {}

impl From<IntegralKeyError> for MultiAffineNumeratorExpansionError {
    fn from(error: IntegralKeyError) -> Self {
        Self::IntegralKey(error)
    }
}

impl From<ExactAlgebraError> for MultiAffineNumeratorExpansionError {
    fn from(error: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(error)
    }
}
