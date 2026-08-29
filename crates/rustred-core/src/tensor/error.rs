//! Typed tensor-service failures.

use std::fmt;

use symbolica::atom::Atom;

use crate::algebra::ExactAlgebraError;
use crate::family::presentation::SingleScaleVacuumIneligibility;
use crate::family::{IntegralFamilyError, IntegralKeyError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TensorHeadKind {
    LoopVector,
    ExternalVector,
    Metric,
    Dot,
}

impl fmt::Display for TensorHeadKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LoopVector => "loop-vector",
            Self::ExternalVector => "external-vector",
            Self::Metric => "metric",
            Self::Dot => "dot",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TensorHeadViolation {
    Wildcard,
    BuiltIn,
    CustomBehavior,
    Aliases,
    Tags,
    UserData,
    Attributes,
}

impl fmt::Display for TensorHeadViolation {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TensorHeadError {
    Duplicate {
        first: TensorHeadKind,
        second: TensorHeadKind,
    },
    Invalid {
        kind: TensorHeadKind,
        violation: TensorHeadViolation,
    },
}

impl fmt::Display for TensorHeadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate { first, second } => {
                write!(formatter, "tensor heads {first} and {second} are identical")
            }
            Self::Invalid { kind, violation } => {
                write!(formatter, "invalid {kind} tensor head: {violation}")
            }
        }
    }
}

impl std::error::Error for TensorHeadError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MomentumKind {
    Loop,
    External,
}

impl fmt::Display for MomentumKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Loop => "loop",
            Self::External => "external",
        })
    }
}

#[derive(Debug)]
pub enum TensorError {
    Head(TensorHeadError),
    UnsupportedGenericKinematics,
    AutomaticLaneUnavailable(SingleScaleVacuumIneligibility),
    SingleScaleVacuumIneligible(SingleScaleVacuumIneligibility),
    WrongLoopMomentumCount {
        expected: usize,
        actual: usize,
    },
    DuplicateMomentum {
        first: MomentumKind,
        second: MomentumKind,
    },
    ReservedHeadInMomentum {
        kind: TensorHeadKind,
    },
    MalformedReservedHead {
        head: TensorHeadKind,
        expected_arity: usize,
        actual_arity: Option<usize>,
    },
    ReservedHeadInUnsupportedPosition {
        head: TensorHeadKind,
    },
    UnknownMomentum {
        momentum: Atom,
    },
    LoopMomentumInOpaqueScalar {
        momentum: Atom,
    },
    UnsupportedNestedTensorSum,
    UnsupportedLorentzIndexContraction {
        index: Atom,
    },
    UnsupportedEvenRank {
        rank: usize,
        supported: usize,
    },
    SingularDimension,
    WrongIntegralKeyArity {
        expected: usize,
        actual: usize,
    },
    UnsupportedAuxiliaryIntegral {
        denominator: usize,
        power: i64,
    },
    UnsupportedAuxiliaryPowerShift {
        denominator: usize,
    },
    IntegralPowerUnderflow {
        denominator: usize,
        power: i64,
    },
    ProjectionFamilyMismatch,
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
    Family(IntegralFamilyError),
    IntegralKey(IntegralKeyError),
}

impl fmt::Display for TensorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Head(error) => error.fmt(formatter),
            Self::UnsupportedGenericKinematics => formatter
                .write_str("generic external-kinematics tensor reduction is not implemented"),
            Self::AutomaticLaneUnavailable(reason) => {
                write!(
                    formatter,
                    "no automatic tensor lane is admissible: {reason}"
                )
            }
            Self::SingleScaleVacuumIneligible(reason) => {
                write!(
                    formatter,
                    "single-scale vacuum lane is ineligible: {reason}"
                )
            }
            Self::WrongLoopMomentumCount { expected, actual } => write!(
                formatter,
                "tensor momentum map has {actual} loop labels, expected {expected}"
            ),
            Self::DuplicateMomentum { first, second } => write!(
                formatter,
                "a {first} momentum label duplicates a {second} momentum label"
            ),
            Self::ReservedHeadInMomentum { kind } => {
                write!(formatter, "momentum label contains reserved {kind} head")
            }
            Self::MalformedReservedHead {
                head,
                expected_arity,
                actual_arity,
            } => match actual_arity {
                Some(actual) => write!(
                    formatter,
                    "reserved {head} head has arity {actual}, expected {expected_arity}"
                ),
                None => write!(formatter, "reserved {head} head is not a function"),
            },
            Self::ReservedHeadInUnsupportedPosition { head } => write!(
                formatter,
                "reserved {head} head occurs outside the bounded tensor-factor grammar"
            ),
            Self::UnknownMomentum { momentum } => {
                write!(formatter, "unknown tensor momentum label {momentum}")
            }
            Self::LoopMomentumInOpaqueScalar { momentum } => write!(
                formatter,
                "loop-momentum label {momentum} occurs inside an opaque scalar factor"
            ),
            Self::UnsupportedNestedTensorSum => formatter
                .write_str("the initial tensor grammar accepts sums only at the numerator root"),
            Self::UnsupportedLorentzIndexContraction { index } => write!(
                formatter,
                "Lorentz index {index} contracts a free loop vector with another retained tensor factor; this bounded projector does not yet canonicalize that contraction"
            ),
            Self::UnsupportedEvenRank { rank, supported } => write!(
                formatter,
                "even internal tensor rank {rank} is not implemented; supported through rank {supported}"
            ),
            Self::SingularDimension => {
                formatter.write_str("rank-two projection requires nonzero dimension")
            }
            Self::WrongIntegralKeyArity { expected, actual } => write!(
                formatter,
                "integral key has {actual} powers, expected {expected}"
            ),
            Self::UnsupportedAuxiliaryIntegral { denominator, power } => write!(
                formatter,
                "auxiliary denominator {denominator} has base power {power}; the bounded tensor grammar requires zero auxiliary powers"
            ),
            Self::UnsupportedAuxiliaryPowerShift { denominator } => write!(
                formatter,
                "auxiliary denominator {denominator} has a nonzero family power shift, which the bounded tensor grammar cannot inspect"
            ),
            Self::IntegralPowerUnderflow { denominator, power } => write!(
                formatter,
                "cancelling denominator {denominator} from integral power {power} underflowed i64"
            ),
            Self::ProjectionFamilyMismatch => {
                formatter.write_str("the tensor projection was minted for a different family owner")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "tensor resource limit exceeded for {resource}: requested {requested}, limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "tensor resource count overflowed for {resource}")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} cells for {resource}"
            ),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::Family(error) => error.fmt(formatter),
            Self::IntegralKey(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TensorError {}

impl From<TensorHeadError> for TensorError {
    fn from(value: TensorHeadError) -> Self {
        Self::Head(value)
    }
}

impl From<ExactAlgebraError> for TensorError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

impl From<IntegralFamilyError> for TensorError {
    fn from(value: IntegralFamilyError) -> Self {
        Self::Family(value)
    }
}

impl From<IntegralKeyError> for TensorError {
    fn from(value: IntegralKeyError) -> Self {
        Self::IntegralKey(value)
    }
}

pub(crate) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), TensorError> {
    if requested > limit {
        Err(TensorError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(crate) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, TensorError> {
    left.checked_add(right)
        .ok_or(TensorError::ResourceCountOverflow { resource })
}

pub(crate) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, TensorError> {
    left.checked_mul(right)
        .ok_or(TensorError::ResourceCountOverflow { resource })
}
