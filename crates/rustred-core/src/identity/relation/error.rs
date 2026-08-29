use std::fmt;

use crate::algebra::IndexedAlgebraError;

use super::super::condition::IdentityConditionError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricRelationError {
    EmptyIndexSpace,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    IndexOutOfRange {
        position: usize,
        arity: usize,
    },
    IndexOverflow {
        position: usize,
    },
    WrongContext,
    WrongFamily,
    UnsatisfiableDomain,
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    IdentityCondition(IdentityConditionError),
    Coefficient(IndexedAlgebraError),
}

impl fmt::Display for ParametricRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIndexSpace => formatter.write_str("an integral index space cannot be empty"),
            Self::WrongArity { expected, actual } => {
                write!(formatter, "index arity is {actual}, expected {expected}")
            }
            Self::IndexOutOfRange { position, arity } => {
                write!(
                    formatter,
                    "index position {position} is outside arity {arity}"
                )
            }
            Self::IndexOverflow { position } => {
                write!(formatter, "integer index overflow at position {position}")
            }
            Self::WrongContext => formatter.write_str("relation and coefficient contexts differ"),
            Self::WrongFamily => formatter.write_str("relations belong to different families"),
            Self::UnsatisfiableDomain => formatter
                .write_str("relation domain contains an identically zero nonzero condition"),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} units for {resource}"
            ),
            Self::IdentityCondition(error) => error.fmt(formatter),
            Self::Coefficient(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ParametricRelationError {}

impl From<IndexedAlgebraError> for ParametricRelationError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::Coefficient(value)
    }
}

impl From<IdentityConditionError> for ParametricRelationError {
    fn from(value: IdentityConditionError) -> Self {
        Self::IdentityCondition(value)
    }
}
