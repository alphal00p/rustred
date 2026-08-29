//! Errors raised by authenticated indexed coefficient algebra.

use std::fmt;

use crate::algebra::ExactAlgebraError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexedAlgebraError {
    EmptyIndexSpace,
    InvalidScope,
    IndexSymbolRegistrationFailure {
        position: usize,
    },
    IndexSymbolCollision {
        position: usize,
    },
    WrongContext,
    WrongIndexArity {
        expected: usize,
        actual: usize,
    },
    ZeroDenominator,
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
    Symbolica(String),
}

impl fmt::Display for IndexedAlgebraError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIndexSpace => {
                formatter.write_str("an indexed coefficient context needs at least one index")
            }
            Self::InvalidScope => formatter.write_str("invalid indexed coefficient context scope"),
            Self::IndexSymbolRegistrationFailure { position } => write!(
                formatter,
                "Symbolica rejected generated indexed coefficient symbol {position}"
            ),
            Self::IndexSymbolCollision { position } => write!(
                formatter,
                "private indexed coefficient symbol {position} has a conflicting registration"
            ),
            Self::WrongContext => formatter.write_str(
                "coefficient or polynomial belongs to a different authenticated context",
            ),
            Self::WrongIndexArity { expected, actual } => write!(
                formatter,
                "index vector has arity {actual}, expected {expected}"
            ),
            Self::ZeroDenominator => {
                formatter.write_str("rational coefficient has a zero denominator")
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
                "could not reserve {requested} units for {resource}"
            ),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::Symbolica(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for IndexedAlgebraError {}

impl From<ExactAlgebraError> for IndexedAlgebraError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}
