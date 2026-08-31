//! One-task semantic adapter from frozen blind-sector geometry to an exact
//! owner-cover delta.
//!
//! The caller owns the immutable simplex plan, completed ordinary source
//! module, zero-source incidence index, and canonical sector ledger. This
//! module derives a fresh maximal anchor for one checked task, streams the
//! scheduler result through exact replay, and transactionally submits any
//! compiled owner to the ledger. Only an exact compiler `Closed` status is
//! exposed as closure; every other result remains discovery or cover-delta
//! telemetry.

mod error;
mod limits;
mod model;
mod run;

pub(crate) use error::InteriorCampaignError;
pub(crate) use limits::InteriorCampaignLimits;
pub(crate) use model::{
    InteriorCampaignAppliedOwner, InteriorCampaignBootstrapCensus, InteriorCampaignCensus,
    InteriorCampaignNoProposal, InteriorCampaignOutcome, InteriorCampaignOwnerEffect,
    InteriorCampaignTaskBinding, InteriorCampaignTaskReport,
};
pub(crate) use run::InteriorCampaignAdapter;

#[cfg(test)]
mod tests;
