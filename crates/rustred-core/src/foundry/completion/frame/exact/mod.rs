//! Exact recovery and full physical replay of one modularly discovered target
//! circuit.
//!
//! The caller supplies an exhaustive, frame-bound target/forbidden/allowed
//! partition. This module proves only the guarded exact source-span relation;
//! it never promotes the supplied roles into completion authority.

mod error;
mod limits;
mod model;
mod reduce;
mod replay;

pub(crate) use error::ExactCircuitError;
pub(crate) use limits::ExactCircuitLimits;
pub(crate) use model::{
    ExactCircuitGuard, ExactCircuitGuardOrigin, ExactCircuitLift, ExactCircuitPivotGuard,
    ExactCircuitReplayWitness, ExactCircuitSupportDidNotLift, ExactCircuitTerm,
    ExactFrameSourceContribution, ExactTargetCircuit,
};
pub(crate) use reduce::try_lift_exact_circuit;

#[cfg(test)]
mod tests;
