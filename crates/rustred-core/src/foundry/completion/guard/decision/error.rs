use std::fmt;

use crate::algebra::IndexedAlgebraError;

/// Typed failures at the bounded semantic guard-DAG boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GuardDecisionDagError {
    WrongAtomContext {
        candidate: usize,
        atom: usize,
    },
    WrongEvaluationContext,
    DuplicateCandidate {
        candidate: usize,
    },
    NonCanonicalCandidateOrder {
        previous: usize,
        current: usize,
    },
    BranchArity {
        expected: usize,
        actual: usize,
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
    IndexedAlgebra(IndexedAlgebraError),
    InternalInvariant(&'static str),
}

impl fmt::Display for GuardDecisionDagError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongAtomContext { candidate, atom } => write!(
                formatter,
                "candidate {candidate} guard atom {atom} belongs to another indexed context"
            ),
            Self::WrongEvaluationContext => {
                formatter.write_str("semantic guard DAG evaluation used another indexed context")
            }
            Self::DuplicateCandidate { candidate } => {
                write!(
                    formatter,
                    "guard candidate identity {candidate} is duplicated"
                )
            }
            Self::NonCanonicalCandidateOrder { previous, current } => write!(
                formatter,
                "guard candidates must be supplied in strictly increasing identity order; \
                 identity {current} follows {previous}"
            ),
            Self::BranchArity { expected, actual } => write!(
                formatter,
                "guard decision requires {expected} atom branches, received {actual}"
            ),
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
            Self::IndexedAlgebra(error) => {
                write!(
                    formatter,
                    "semantic guard predicate evaluation failed: {error}"
                )
            }
            Self::InternalInvariant(message) => {
                write!(formatter, "semantic guard DAG invariant failed: {message}")
            }
        }
    }
}

impl std::error::Error for GuardDecisionDagError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IndexedAlgebra(error) => Some(error),
            _ => None,
        }
    }
}

impl From<IndexedAlgebraError> for GuardDecisionDagError {
    fn from(error: IndexedAlgebraError) -> Self {
        Self::IndexedAlgebra(error)
    }
}
