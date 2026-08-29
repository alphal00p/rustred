use std::fmt;

use crate::algebra::{ExactAlgebraError, IndexedAlgebraError};
use crate::family::IntegralKeyError;
use crate::sector;

/// Typed failure while applying a sealed closing artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReductionError {
    WrongArity {
        expected: usize,
        actual: usize,
    },
    IndexOverflow {
        position: usize,
    },
    UncoveredIntegral {
        target: crate::family::IntegralKey,
    },
    CycleDetected {
        target: crate::family::IntegralKey,
    },
    RuleApplicationLimit {
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    CacheLimit {
        requested: usize,
        limit: usize,
    },
    CacheCoefficientTermLimit {
        requested: usize,
        limit: usize,
    },
    CacheCoefficientByteLimit {
        requested: usize,
        limit: usize,
    },
    CacheResourceCountOverflow {
        resource: &'static str,
    },
    PendingFrameLimit {
        requested: usize,
        limit: usize,
    },
    ReducerInvariant {
        detail: &'static str,
    },
    UnexpectedCoefficientGuard,
    ZeroCommonMass,
    MissingCommonMassHomogeneityProof,
    CommonMassPowerOverflow,
    IntegralKey(IntegralKeyError),
    IndexedAlgebra(IndexedAlgebraError),
    ExactAlgebra(ExactAlgebraError),
    Ordering(sector::Error),
}

impl fmt::Display for ReductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongArity { expected, actual } => {
                write!(formatter, "integral arity is {actual}, expected {expected}")
            }
            Self::IndexOverflow { position } => {
                write!(formatter, "integral-index arithmetic overflowed at position {position}")
            }
            Self::UncoveredIntegral { target } => write!(
                formatter,
                "the closed artifact has no rule or terminal for integral {:?}",
                target.powers()
            ),
            Self::CycleDetected { target } => write!(
                formatter,
                "artifact application encountered a dependency cycle at integral {:?}",
                target.powers()
            ),
            Self::RuleApplicationLimit { requested, limit } => write!(
                formatter,
                "reduction needs at least {requested} rule applications, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}"
            ),
            Self::CacheLimit { requested, limit } => write!(
                formatter,
                "reduction cache needs {requested} integral entries, exceeding the configured limit {limit}"
            ),
            Self::CacheCoefficientTermLimit { requested, limit } => write!(
                formatter,
                "reduction cache needs {requested} retained coefficient terms, exceeding the configured limit {limit}"
            ),
            Self::CacheCoefficientByteLimit { requested, limit } => write!(
                formatter,
                "reduction cache needs {requested} retained coefficient bytes, exceeding the configured limit {limit}"
            ),
            Self::CacheResourceCountOverflow { resource } => write!(
                formatter,
                "reduction cache resource census overflowed while counting {resource}"
            ),
            Self::PendingFrameLimit { requested, limit } => write!(
                formatter,
                "reduction needs {requested} pending frames, exceeding the configured limit {limit}"
            ),
            Self::ReducerInvariant { detail } => {
                write!(formatter, "sealed reducer invariant failed: {detail}")
            }
            Self::UnexpectedCoefficientGuard => formatter.write_str(
                "a sealed universally applicable rule produced an unexpected base-parameter denominator guard",
            ),
            Self::ZeroCommonMass => formatter
                .write_str("common-mass restoration requires nonzero mass squared"),
            Self::MissingCommonMassHomogeneityProof => formatter.write_str(
                "the closed artifact has no proof for common-mass homogeneity restoration",
            ),
            Self::CommonMassPowerOverflow => formatter
                .write_str("the common-mass homogeneity exponent cannot be represented"),
            Self::IntegralKey(error) => error.fmt(formatter),
            Self::IndexedAlgebra(error) => error.fmt(formatter),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::Ordering(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ReductionError {}

impl From<IntegralKeyError> for ReductionError {
    fn from(value: IntegralKeyError) -> Self {
        Self::IntegralKey(value)
    }
}

impl From<IndexedAlgebraError> for ReductionError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::IndexedAlgebra(value)
    }
}

impl From<ExactAlgebraError> for ReductionError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

impl From<sector::Error> for ReductionError {
    fn from(value: sector::Error) -> Self {
        Self::Ordering(value)
    }
}
