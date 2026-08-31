//! Bounded inverse-incidence discovery of exact translated-source requests.
//!
//! This layer only nominates rows from a sealed ordinary-source module.  A
//! nomination is neither a modular hit nor an exact relation, and no result
//! here can authorize a rule, owner, terminal, artifact, or closure claim.

mod campaign;
mod error;
mod incidence;
mod limits;
mod model;
mod nominate;
mod residual;

// This is the staged integration seam for the next fixed-task driver. The
// executable campaign is regression-tested in its child before that driver
// consumes every reexport.
#[allow(unused_imports)]
pub(crate) use campaign::{
    AccumulatedSourceRequests, CampaignBudgetExhaustion, CampaignError, CampaignLimits,
    CampaignModularProbe, CampaignRequestMerge, CampaignRequestMergeTelemetry,
    CampaignResourceStage, CandidateBatchExhaustionTelemetry, FreshTaskBuildTelemetry,
    FreshTaskEpoch, FreshTaskQuery, FreshTaskQueryTelemetry,
};
pub(crate) use error::SourceDiscoveryError;
pub(crate) use incidence::OrdinarySourceIncidenceIndex;
pub(crate) use limits::SourceDiscoveryLimits;
pub(crate) use model::{IncidentTranslationNominations, NonzeroIncidentTranslationResiduals};

#[cfg(test)]
mod tests;
