//! Errors raised by authenticated indexed coefficient algebra.

use std::fmt;

use crate::algebra::ExactAlgebraError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexedAlgebraError {
    EmptyIndexSpace,
    InvalidScope(String),
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
    ExactAlgebra(ExactAlgebraError),
    Symbolica(String),
}

impl fmt::Display for IndexedAlgebraError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIndexSpace => {
                formatter.write_str("an indexed coefficient context needs at least one index")
            }
            Self::InvalidScope(scope) => {
                write!(
                    formatter,
                    "invalid indexed coefficient context scope {scope:?}"
                )
            }
            Self::IndexSymbolCollision { position } => write!(
                formatter,
                "generated indexed coefficient symbol {position} collides with a base variable"
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
