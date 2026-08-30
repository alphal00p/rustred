use std::fmt;

use crate::algebra::IndexedAlgebraError;
use crate::foundry::completion::stratum::StratumRegistryError;

/// Typed failures while compiling one pulled-back guard into a coefficient-
/// ideal atom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CoefficientIdealGuardError {
    IdenticallyZeroGuard,
    TargetPullbackOverflow {
        index: usize,
        shift: i64,
    },
    IndexedAlgebra(IndexedAlgebraError),
    PredicateIdentity(StratumRegistryError),
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
}

impl fmt::Display for CoefficientIdealGuardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdenticallyZeroGuard => formatter.write_str(
                "an identically zero guard has empty applicability and cannot become an atom",
            ),
            Self::TargetPullbackOverflow { index, shift } => write!(
                formatter,
                "target pullback cannot negate index shift {shift} at position {index}"
            ),
            Self::IndexedAlgebra(error) => {
                write!(
                    formatter,
                    "generic-parameter coefficient split failed: {error}"
                )
            }
            Self::PredicateIdentity(error) => {
                write!(
                    formatter,
                    "coefficient-ideal generator identity failed: {error}"
                )
            }
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}"
            ),
        }
    }
}

impl std::error::Error for CoefficientIdealGuardError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IndexedAlgebra(error) => Some(error),
            Self::PredicateIdentity(error) => Some(error),
            _ => None,
        }
    }
}

impl From<IndexedAlgebraError> for CoefficientIdealGuardError {
    fn from(error: IndexedAlgebraError) -> Self {
        Self::IndexedAlgebra(error)
    }
}
