//! Deterministic semantic routing for already replayed exact target circuits.
//!
//! This crate-private layer binds exact circuit proof payloads to one verified
//! target partition and compiles every required guard in target coordinates.
//! A selected candidate is still only generic-context, pointwise evidence:
//! physical parameter quotients/fibres, rule ownership, and closure remain
//! separate obligations. In particular, [`ExactCircuitSemanticSelection::Incomplete`]
//! confers no negative or completion authority.

mod compile;
mod error;
mod limits;
mod model;
mod order;
mod validation;

pub(crate) use error::ExactCircuitSemanticError;
pub(crate) use limits::ExactCircuitSemanticLimits;
pub(crate) use model::{
    ExactCircuitSemanticCandidate, ExactCircuitSemanticDag, ExactCircuitSemanticSelection,
};
pub(crate) use order::{compare_exact_circuit_content, exact_circuit_content_equal};
