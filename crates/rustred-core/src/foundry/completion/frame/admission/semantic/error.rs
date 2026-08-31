//! Typed failures at the exact-circuit semantic boundary.

use std::fmt;

use crate::algebra::IndexedAlgebraError;
use crate::foundry::completion::guard::CoefficientIdealGuardError;
use crate::foundry::completion::guard::decision::GuardDecisionDagError;
use crate::foundry::completion::stratum::StratumRegistryError;

/// Typed failure at the exact-circuit/partition semantic boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExactCircuitSemanticError {
    WrongContext,
    PartitionVerification(StratumRegistryError),
    PartitionInvariant(&'static str),
    CandidateJoin {
        candidate: usize,
        detail: &'static str,
    },
    IndexedAlgebra {
        candidate: usize,
        error: IndexedAlgebraError,
    },
    GuardAtom {
        candidate: usize,
        guard: usize,
        error: CoefficientIdealGuardError,
    },
    DuplicateExactContent,
    GuardDag(GuardDecisionDagError),
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
    Invariant(&'static str),
}

impl fmt::Display for ExactCircuitSemanticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongContext => formatter.write_str(
                "semantic exact-circuit compilation uses another indexed coefficient context",
            ),
            Self::PartitionVerification(error) => {
                write!(formatter, "target partition verification failed: {error}")
            }
            Self::PartitionInvariant(detail) => {
                write!(formatter, "target partition invariant failed: {detail}")
            }
            Self::CandidateJoin { candidate, detail } => {
                write!(
                    formatter,
                    "exact candidate {candidate} failed its join: {detail}"
                )
            }
            Self::IndexedAlgebra { candidate, error } => write!(
                formatter,
                "exact candidate {candidate} failed algebra authentication: {error}"
            ),
            Self::GuardAtom {
                candidate,
                guard,
                error,
            } => write!(
                formatter,
                "exact candidate {candidate} guard {guard} failed semantic compilation: {error}"
            ),
            Self::DuplicateExactContent => {
                formatter.write_str("duplicate exact circuit proof content is not admissible")
            }
            Self::GuardDag(error) => write!(formatter, "semantic guard DAG failed: {error}"),
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
            Self::Invariant(detail) => {
                write!(
                    formatter,
                    "semantic exact-circuit invariant failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for ExactCircuitSemanticError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PartitionVerification(error) => Some(error),
            Self::IndexedAlgebra { error, .. } => Some(error),
            Self::GuardAtom { error, .. } => Some(error),
            Self::GuardDag(error) => Some(error),
            _ => None,
        }
    }
}
