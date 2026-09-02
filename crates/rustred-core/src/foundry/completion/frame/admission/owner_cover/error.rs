//! Typed failures at the guarded exact-circuit owner-cover boundary.

use std::fmt;

use crate::algebra::IndexedAlgebraError;
use crate::foundry::completion::CompletionGeometryError;

use super::super::ExactCircuitSemanticError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExactCircuitOwnerCoverError {
    EmptyOwnerInputs,
    WrongContext,
    OwnerJoin {
        owner: usize,
        detail: &'static str,
    },
    MixedOwnerScope {
        owner: usize,
        detail: &'static str,
    },
    GuardLocus {
        owner: usize,
        candidate: usize,
        guard: usize,
        error: IndexedAlgebraError,
    },
    DuplicateOwnerContent,
    DuplicateTerminal,
    TerminalOutsideClosureCarrier {
        terminal: usize,
    },
    TerminalOverlapsDescendingOwner {
        terminal: usize,
        owner: usize,
    },
    Geometry(CompletionGeometryError),
    SemanticSelection {
        owner: usize,
        error: ExactCircuitSemanticError,
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
    Invariant(&'static str),
}

impl fmt::Display for ExactCircuitOwnerCoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyOwnerInputs => {
                formatter.write_str("an exact owner cover needs at least one semantic owner input")
            }
            Self::WrongContext => formatter
                .write_str("exact owner-cover compilation uses another coefficient context"),
            Self::OwnerJoin { owner, detail } => {
                write!(
                    formatter,
                    "exact owner input {owner} failed its join: {detail}"
                )
            }
            Self::MixedOwnerScope { owner, detail } => write!(
                formatter,
                "exact owner input {owner} differs from the common cover scope: {detail}"
            ),
            Self::GuardLocus {
                owner,
                candidate,
                guard,
                error,
            } => write!(
                formatter,
                "exact owner input {owner}, candidate {candidate}, guard {guard} failed exceptional-locus analysis: {error}"
            ),
            Self::DuplicateOwnerContent => {
                formatter.write_str("duplicate exact semantic owner content is not admissible")
            }
            Self::DuplicateTerminal => {
                formatter.write_str("duplicate explicit finite terminal is not admissible")
            }
            Self::TerminalOutsideClosureCarrier { terminal } => write!(
                formatter,
                "explicit terminal {terminal} lies outside the exact closure carrier"
            ),
            Self::TerminalOverlapsDescendingOwner { terminal, owner } => write!(
                formatter,
                "explicit terminal {terminal} overlaps descending owner {owner}"
            ),
            Self::Geometry(error) => error.fmt(formatter),
            Self::SemanticSelection { owner, error } => {
                write!(
                    formatter,
                    "exact owner {owner} guard selection failed: {error}"
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
            Self::Invariant(detail) => {
                write!(formatter, "exact owner-cover invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for ExactCircuitOwnerCoverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Geometry(error) => Some(error),
            Self::GuardLocus { error, .. } => Some(error),
            Self::SemanticSelection { error, .. } => Some(error),
            _ => None,
        }
    }
}

impl From<CompletionGeometryError> for ExactCircuitOwnerCoverError {
    fn from(value: CompletionGeometryError) -> Self {
        Self::Geometry(value)
    }
}
