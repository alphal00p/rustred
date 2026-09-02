//! Exact recovery and full physical replay of one modularly discovered target
//! circuit.
//!
//! The caller supplies an exhaustive, frame-bound target/forbidden/allowed
//! partition. This module proves only the guarded exact source-span relation;
//! it never promotes the supplied roles into completion authority.

mod error;
mod limits;
mod lowering;
mod model;
mod reduce;
mod replay;

mod cleared;

pub(crate) use cleared::{
    ClearedCircuitError, ClearedCircuitLimits, ClearedExactCircuit, ClearedSemanticGuardOrigin,
    try_clear_exact_circuit,
};

pub(crate) use error::ExactCircuitError;
pub(crate) use limits::ExactCircuitLimits;
#[cfg(test)]
pub(crate) use lowering::try_lower_exact_circuit;
pub(crate) use lowering::{
    ExactCircuitLoweringError, ExactCircuitLoweringLimits, ExactCircuitLoweringSeal,
    LoweredExactCircuit, try_lower_cleared_exact_circuit,
};
pub(crate) use model::{
    ExactCircuitGuard, ExactCircuitGuardOrigin, ExactCircuitLift, ExactCircuitPivotGuard,
    ExactCircuitReplayWitness, ExactCircuitSupportDidNotLift, ExactCircuitTerm,
    ExactFrameSourceContribution, ExactTargetCircuit, ExactTargetCircuitIdentity,
};
pub(crate) use reduce::{try_lift_exact_circuit, try_lift_exact_circuit_over_complete_frame};

#[cfg(test)]
mod tests;
