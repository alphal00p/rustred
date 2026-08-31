//! Exact guard-stratum refinement before one circuit may become a rule cell.
//!
//! The all-nonzero child is the only child associated with the circuit.
//! Exceptional children are explicit uncovered obligations and carry neither
//! a target partition nor an owner. This module proves a finite Boolean
//! partition of one already-decorated parent; it does not prove that any child
//! is nonempty and it does not confer closure authority.

mod error;
mod limits;
mod model;
mod refine;
mod semantic;

pub(crate) use error::ExactGuardRefinementError;
pub(crate) use limits::ExactGuardRefinementLimits;
pub(crate) use model::{
    ExactGuardRefinement, ExactGuardRefinementOutcome, ExceptionalGuardStratum,
    RequiredGuardPredicate,
};
pub(crate) use refine::try_refine_exact_circuit_guards;
pub(crate) use semantic::{
    ExactCircuitSemanticDag, ExactCircuitSemanticError, ExactCircuitSemanticLimits,
    ExactCircuitSemanticSelection,
};

#[cfg(test)]
mod tests;
