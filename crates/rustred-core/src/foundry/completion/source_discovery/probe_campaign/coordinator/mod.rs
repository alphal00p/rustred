//! Deterministic fair window-one execution over exact boundary-simplex classes.
//!
//! One epoch is bound to an opaque exact-ledger snapshot. Classes are visited
//! by descending effective dimension and then descending present parent
//! dimension. Live memory is bounded by the cloned partition, class schedule,
//! largest materialized class plan, and one evaluated task; completed task
//! reports are compacted to scalar counters rather than retained. Any owner-set
//! mutation ends the epoch, drops every remaining materialized ticket, and
//! requires the next call to replan from the new ledger snapshot. A retained
//! scalar service cursor starts that fresh plan at the ordinal successor of
//! the mutating task. This prevents repeated task-zero service on a stable
//! plan without retaining stale task authority. It is deliberately not a
//! semantic itinerary: callers that must visit a fixed named target set across
//! geometry changes need a cursor keyed by those stable targets, because a new
//! partition may renumber this plan's dynamic task ordinals.
//!
//! ExhaustedAtConfig means only that the coordinator-owned probe program completed on
//! one unchanged snapshot. It carries no closure API. Only the exact compiler
//! status can produce CompilerClosed.

mod chronology;
mod compact;
mod error;
mod limits;
mod model;
mod run;
mod schedule;

pub(crate) use error::ProbeCoordinatorFailure;
pub(crate) use limits::ProbeCoordinatorLimits;
pub(super) use model::ProbeCoordinatorFairCursor;
pub(crate) use model::{
    BoundaryProbeCoordinator, ProbeCoordinatorCensus, ProbeCoordinatorClass,
    ProbeCoordinatorClassSchedule, ProbeCoordinatorConfig, ProbeCoordinatorFailureStop,
    ProbeCoordinatorNeedsRefinement, ProbeCoordinatorNeedsRefinementReason,
    ProbeCoordinatorOperationalReason, ProbeCoordinatorOperationalStop,
    ProbeCoordinatorOwnerMutation, ProbeCoordinatorOwnerSetChanged, ProbeCoordinatorStop,
    ProbeCoordinatorTaskLocation, ProbeCoordinatorTaskLocationKind, RequestedProbeCoordinatorStop,
    TaskRelativeModularProbe,
};

#[cfg(test)]
mod tests;
