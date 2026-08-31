//! Deterministic finite-field probes of one exact physical frame.
//!
//! This bounded A0 discovery lane is crate-private production code with its
//! parent completion module. A modular hit is evidence for a later exact
//! lift and replay; a checked
//! [`ModularTargetQuery::NoHitWithObstruction`] is still inconclusive.

mod error;
mod limits;
mod model;
mod obstruction;
mod rank;
mod sample;

pub(crate) use error::ModularKernelError;
pub(crate) use limits::ModularKernelLimits;
pub(crate) use model::{
    ModularHit, ModularObstructionEntry, ModularPhysicalFrame, ModularRankDiagnostics,
    ModularRightObstruction, ModularRightObstructionIdentity, ModularSampleFingerprint,
    ModularTargetQuery,
};
pub(crate) use sample::ModularSourceEvaluationError;

#[cfg(test)]
mod tests;
