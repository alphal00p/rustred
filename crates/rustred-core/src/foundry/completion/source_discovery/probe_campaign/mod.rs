//! One-task semantic adapter from frozen probe geometry to an exact owner-cover
//! delta.
//!
//! The direct one-task adapter consumes an immutable interior- or
//! boundary-simplex plan, completed ordinary source module, and canonical
//! sector ledger. This module validates the task against its originating planner
//! epoch and exact parent box, derives a fresh maximal anchor, streams the
//! scheduler result through exact replay, and transactionally submits any
//! compiled owner to the ledger. Only an exact compiler `Closed` status is
//! exposed as closure; every other result remains discovery or cover-delta
//! telemetry.

mod coordinator;
mod error;
mod limits;
mod model;
mod planned_task;
mod run;

#[allow(unused_imports)] // Production boundary campaign awaiting the Stage-1 driver.
pub(crate) use coordinator::{
    BoundaryProbeCoordinator, ProbeCoordinatorCensus, ProbeCoordinatorClass,
    ProbeCoordinatorClassSchedule, ProbeCoordinatorConfig, ProbeCoordinatorFailure,
    ProbeCoordinatorFailureStop, ProbeCoordinatorLimits, ProbeCoordinatorNeedsRefinement,
    ProbeCoordinatorNeedsRefinementReason, ProbeCoordinatorOperationalReason,
    ProbeCoordinatorOperationalStop, ProbeCoordinatorOwnerMutation,
    ProbeCoordinatorOwnerSetChanged, ProbeCoordinatorStop, ProbeCoordinatorTaskLocation,
    TaskRelativeModularProbe,
};
pub(crate) use error::ProbeCampaignError;
pub(crate) use limits::ProbeCampaignLimits;
pub(crate) use model::{
    ProbeCampaignAppliedOwner, ProbeCampaignBootstrapCensus, ProbeCampaignCensus,
    ProbeCampaignEvaluatedTask, ProbeCampaignNoProposal, ProbeCampaignOutcome,
    ProbeCampaignOwnerEffect, ProbeCampaignTaskBinding, ProbeCampaignTaskReport,
};
pub(crate) use planned_task::ProbeCampaignPlannedTask;
pub(crate) use run::ProbeCampaignAdapter;

#[cfg(test)]
mod tests;
