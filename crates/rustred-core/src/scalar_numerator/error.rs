use std::fmt;

use symbolica::atom::Atom;

use crate::algebra::ExactAlgebraError;
use crate::family::{IntegralFamilyError, IntegralKeyError};

/// Why a caller-supplied scalar-product head is not a passive Symbolica head.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarProductHeadViolation {
    Wildcard,
    BuiltIn,
    CustomBehavior,
    Aliases,
    Tags,
    UserData,
    Attributes,
}

impl fmt::Display for ScalarProductHeadViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Wildcard => "wildcard heads are not permitted",
            Self::BuiltIn => "built-in symbols are not permitted",
            Self::CustomBehavior => "custom Symbolica behavior is not permitted",
            Self::Aliases => "head aliases are not permitted",
            Self::Tags => "head tags are not permitted",
            Self::UserData => "head user data is not permitted",
            Self::Attributes => "the head has unsupported attributes",
        })
    }
}

/// Fail-closed scalar-numerator boundary errors.
#[derive(Debug)]
#[non_exhaustive]
pub enum ScalarNumeratorError {
    UnsupportedArtifact {
        detail: &'static str,
    },
    InvalidScalarProductHead {
        violation: ScalarProductHeadViolation,
    },
    WrongLoopMomentumCount {
        expected: usize,
        actual: usize,
    },
    DuplicateLoopMomentum {
        first: usize,
        second: usize,
    },
    ScalarProductHeadInLoopMomentum {
        momentum: usize,
    },
    MalformedScalarProduct {
        actual_arity: Option<usize>,
    },
    MixedLoopScalarProduct {
        expression: Atom,
    },
    LoopMomentumOutsideScalarProduct {
        momentum: Atom,
    },
    NestedScalarProductArgument {
        expression: Atom,
    },
    NonPolynomialScalarProducts {
        detail: String,
    },
    ScalarProductExponentOverflow,
    IntegralPowerUnderflow {
        denominator: usize,
        power: i64,
    },
    CommonMassPowerOverflow,
    WrongIntegralKeyArity {
        expected: usize,
        actual: usize,
    },
    OutsideCertifiedRootDomain {
        position: usize,
        value: i64,
        lower: i64,
        upper: i64,
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
    SymbolicaPanic {
        operation: &'static str,
    },
    ExactAlgebra(ExactAlgebraError),
    Family(IntegralFamilyError),
    IntegralKey(IntegralKeyError),
}

impl fmt::Display for ScalarNumeratorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedArtifact { detail } => {
                write!(
                    formatter,
                    "artifact cannot lower a common-mass scalar numerator: {detail}"
                )
            }
            Self::InvalidScalarProductHead { violation } => {
                write!(formatter, "invalid scalar-product head: {violation}")
            }
            Self::WrongLoopMomentumCount { expected, actual } => write!(
                formatter,
                "scalar numerator has {actual} loop labels, expected {expected}"
            ),
            Self::DuplicateLoopMomentum { first, second } => write!(
                formatter,
                "loop-momentum labels {first} and {second} are identical"
            ),
            Self::ScalarProductHeadInLoopMomentum { momentum } => write!(
                formatter,
                "loop-momentum label {momentum} contains the reserved scalar-product head"
            ),
            Self::MalformedScalarProduct { actual_arity } => match actual_arity {
                Some(actual) => write!(
                    formatter,
                    "scalar-product head has arity {actual}, expected 2"
                ),
                None => formatter.write_str("scalar-product head is not a function"),
            },
            Self::MixedLoopScalarProduct { expression } => write!(
                formatter,
                "loop-external scalar product remains after tensor projection: {expression}"
            ),
            Self::LoopMomentumOutsideScalarProduct { momentum } => write!(
                formatter,
                "loop momentum {momentum} occurs outside an admitted scalar product"
            ),
            Self::NestedScalarProductArgument { expression } => write!(
                formatter,
                "nested scalar-product syntax is not an atomic momentum label: {expression}"
            ),
            Self::NonPolynomialScalarProducts { detail } => write!(
                formatter,
                "scalar numerator is not polynomial in loop scalar products: {detail}"
            ),
            Self::ScalarProductExponentOverflow => {
                formatter.write_str("scalar-product exponent does not fit the lowering domain")
            }
            Self::IntegralPowerUnderflow { denominator, power } => write!(
                formatter,
                "cancelling denominator {denominator} from power {power} underflowed i64"
            ),
            Self::CommonMassPowerOverflow => {
                formatter.write_str("common-mass-squared power overflowed u32")
            }
            Self::WrongIntegralKeyArity { expected, actual } => write!(
                formatter,
                "integral key has {actual} powers, expected {expected}"
            ),
            Self::OutsideCertifiedRootDomain {
                position,
                value,
                lower,
                upper,
            } => write!(
                formatter,
                "lowered integral power {value} at position {position} is outside the artifact's certified root domain [{lower}, {upper}]"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "scalar-numerator resource limit exceeded for {resource}: requested {requested}, limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "scalar-numerator resource count overflowed for {resource}"
                )
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} cells for {resource}"
            ),
            Self::SymbolicaPanic { operation } => {
                write!(formatter, "Symbolica panicked during {operation}")
            }
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::Family(error) => error.fmt(formatter),
            Self::IntegralKey(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ScalarNumeratorError {}

impl From<ExactAlgebraError> for ScalarNumeratorError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

impl From<IntegralFamilyError> for ScalarNumeratorError {
    fn from(value: IntegralFamilyError) -> Self {
        Self::Family(value)
    }
}

impl From<IntegralKeyError> for ScalarNumeratorError {
    fn from(value: IntegralKeyError) -> Self {
        Self::IntegralKey(value)
    }
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ScalarNumeratorError> {
    if requested > limit {
        Err(ScalarNumeratorError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ScalarNumeratorError> {
    left.checked_add(right)
        .ok_or(ScalarNumeratorError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ScalarNumeratorError> {
    left.checked_mul(right)
        .ok_or(ScalarNumeratorError::ResourceCountOverflow { resource })
}
