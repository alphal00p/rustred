//! Immutable fresh-plan epochs for one fixed target-discovery task.
//!
//! Every successful request augmentation is materialized as a new selected
//! translated-source frame. Raw column ordinals, target partitions, modular
//! samples, hits, and obstructions are therefore frame-local and are rebuilt
//! from raw identities. The decorated stratum, ordering policy, and immutable
//! lower-owner snapshot are fixed inputs and are never widened to fit a new
//! frame.
//!
//! This module owns bounded scheduling and evidence plumbing only. A query
//! hit still requires exact lift and replay. An unchanged candidate batch is
//! finite-input telemetry, not `SampledDeclaredModuleDual`, a terminal, or a
//! closure result. The latter requires the separate complete residual census.

mod build;
mod error;
mod limits;
mod model;

pub(crate) use error::{CampaignBudgetExhaustion, CampaignError, CampaignResourceStage};
pub(crate) use limits::CampaignLimits;
pub(crate) use model::{
    AccumulatedSourceRequests, CampaignModularProbe, CampaignRequestMerge,
    CampaignRequestMergeTelemetry, CandidateBatchExhaustionTelemetry, FreshTaskBuildTelemetry,
    FreshTaskEpoch, FreshTaskQuery, FreshTaskQueryTelemetry,
};

#[cfg(test)]
mod tests;
