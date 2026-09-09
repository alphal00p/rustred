//! Exact pending-work ownership for axis-aligned SpIReD equality cases.
//!
//! The queue stores guard-blind coordinate geometry rather than sampled
//! lattice points or owner-routing cells. Boolean nonzero predicates and
//! first-zero chronology are rejected at the typed obligation boundary.
//! Queue exhaustion carries no closure, terminal, or no-relation authority.

mod error;
mod limits;
mod model;
mod queue;

pub(crate) use error::SpiredCoordinateCaseWorklistError;
pub(crate) use limits::SpiredCoordinateCaseWorklistLimits;
pub(crate) use model::{
    SpiredCoordinateCaseEnqueueOutcome, SpiredCoordinateCaseObligation,
    SpiredCoordinateCasePopOutcome, SpiredCoordinateCaseWorklistCensus,
};
pub(crate) use queue::{
    SpiredCoordinateCasePreparedEnqueueBatch, SpiredCoordinateCasePreparedReplacement,
    SpiredCoordinateCaseValidatedEnqueueBatch, SpiredCoordinateCaseValidatedReplacement,
    SpiredCoordinateCaseWorklist,
};

#[cfg(test)]
mod tests;
