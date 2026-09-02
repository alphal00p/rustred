//! Streaming scheduler-to-owner bridge for one interior target.
//!
//! The probe-local scheduler report remains alive only long enough for
//! canonical replay to reconstruct every nominated relation on one fresh
//! common epoch.  Exact owner compilation then either retains the fresh
//! epoch/circuit/cell authority in its existing typed proposal or this module
//! drops all physical plans and returns scalar telemetry.
//!
//! Relative support is extracted only from a compiled owner and contains no
//! frame row or column ordinal.  Equality of that support across targets is
//! proposal telemetry, not a polynomial-lift proof, owner-cover delta,
//! exhaustion result, terminal declaration, artifact, or closure claim.

mod error;
mod limits;
mod model;
mod run;
mod support;

pub(crate) use error::InteriorReplayRunError;
pub(crate) use limits::InteriorReplayRunLimits;
pub(crate) use model::{
    InteriorReplayAttemptCensus, InteriorReplayBudgetStopSummary, InteriorReplayCandidateSupport,
    InteriorReplayRelativeResidual, InteriorReplayRelativeSource, InteriorReplayRunDisposition,
    InteriorReplaySchedulerOutcomeCensus, InteriorReplaySupportCensus, InteriorReplaySupportSet,
    InteriorReplayTaskReport,
};
pub(crate) use run::{
    try_run_interior_replay_task, try_run_interior_replay_task_with_initial_parent_proposal,
};
pub(crate) use support::support_shapes_match;

#[cfg(test)]
mod tests;
