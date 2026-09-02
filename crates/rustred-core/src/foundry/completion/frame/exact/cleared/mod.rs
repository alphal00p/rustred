//! Bounded fraction-free exact-circuit replay and semantic guard separation.
//!
//! This layer proves a polynomial source consequence before promotion. It
//! separates guards intrinsic to the ordinary sources and final target
//! coefficient from guards introduced by one field-elimination path.

mod budget;
mod compile;
mod model;
#[cfg(test)]
mod tests;

pub(crate) use compile::try_clear_exact_circuit;
#[cfg(test)]
pub(crate) use compile::try_compile_final_target_guard;
pub(crate) use model::{
    ClearedCircuitError, ClearedCircuitLimits, ClearedExactCircuit, ClearedSemanticGuardOrigin,
};
