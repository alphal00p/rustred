//! One-task semantic adapter from frozen probe geometry to an exact owner-cover
//! delta.
//!
//! The caller owns an immutable interior- or boundary-simplex plan, completed
//! ordinary source module, zero-source incidence index, and canonical sector
//! ledger. This module validates the task against its originating planner
//! epoch and exact parent box, derives a fresh maximal anchor, streams the
//! scheduler result through exact replay, and transactionally submits any
//! compiled owner to the ledger. Only an exact compiler `Closed` status is
//! exposed as closure; every other result remains discovery or cover-delta
//! telemetry.

mod error;
mod limits;
mod model;
mod planned_task;
mod run;

pub(crate) use error::ProbeCampaignError;
pub(crate) use limits::ProbeCampaignLimits;
pub(crate) use model::{
    ProbeCampaignAppliedOwner, ProbeCampaignBootstrapCensus, ProbeCampaignCensus,
    ProbeCampaignNoProposal, ProbeCampaignOutcome, ProbeCampaignOwnerEffect,
    ProbeCampaignTaskBinding, ProbeCampaignTaskReport,
};
pub(crate) use planned_task::ProbeCampaignPlannedTask;
pub(crate) use run::ProbeCampaignAdapter;

#[cfg(test)]
mod tests;
