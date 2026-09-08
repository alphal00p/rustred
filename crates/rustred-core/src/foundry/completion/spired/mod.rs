//! Targeted depth-wise completion over exact guarded index cases.
//!
//! This private foundation deliberately separates the geometry a completion
//! request names from the geometry the current executor can prove it handles.
//! Axis-aligned coordinate faces reuse an authenticated
//! [`crate::foundry::completion::stratum::DecoratedStratum`]. Coupled affine
//! lattices have a typed request boundary but fail closed until integer-lattice
//! parameterization, congruences, and exact replay all gain matching support.
//!
//! Source scheduling is likewise proposal-only. A complete signed-L1 depth
//! shell carries no closure or publication authority; later execution must
//! still pass through modular discovery, exact replay, and ordinary promotion.

mod case;
mod compact_lift;
mod discovery;
mod error;
mod evaluate;
mod materializer;
mod modular;
mod schedule;
mod target_run;

pub(crate) use case::{
    SpiredAffineLatticeCase, SpiredCase, SpiredCoordinateFace, SpiredExecutionCase,
};
pub(crate) use compact_lift::{
    SpiredCompactLift, SpiredCompactLiftError, SpiredCompactLiftLimits, SpiredReplayedCompactLift,
    SpiredRuleCellAuthorityError, try_lift_spired_compact_support,
    try_promote_spired_replayed_rule_cell,
};
pub(crate) use discovery::{SpiredStreamingDiscovery, SpiredStreamingError, SpiredStreamingLimits};
pub(crate) use error::SpiredFoundationError;
pub(crate) use evaluate::{
    DirectShiftedSourceError, DirectShiftedSourceEvaluator, DirectShiftedSourceLimits,
    ShiftedModularSourceBuffer, ShiftedModularTerm, ShiftedModularTerms,
};
pub(crate) use materializer::{PrunedExactMaterializer, TargetRuleMaterializer};
pub(crate) use modular::{
    SpiredForbiddenTerm, SpiredModularError, SpiredModularHit, SpiredModularKernel,
    SpiredModularLimits, SpiredModularRow, SpiredValidatedPrime,
};
pub(crate) use schedule::{
    SignedL1DepthShell, SignedL1RequestChunk, SignedL1ScheduleLimits, SignedL1ShellScheduler,
};
pub(crate) use target_run::{
    SpiredTargetRunCensus, SpiredTargetRunError, SpiredTargetRunErrorCause, SpiredTargetRunLimits,
    SpiredTargetRunOutcome, SpiredTargetRunReport, SpiredTargetRunStage,
    try_run_spired_guarded_target, try_run_spired_target,
};

#[cfg(test)]
mod tests;
