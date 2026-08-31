//! Immutable fresh-plan epochs for one fixed target-discovery task.
//!
//! Every successful request augmentation is materialized as a new selected
//! translated-source frame. Raw column ordinals, target partitions, modular
//! samples, hits, and obstructions are therefore frame-local and are rebuilt
//! from raw identities. One-shot callers may retain an intentionally fixed or
//! tightened stratum. Growing schedulers instead authenticate an initial
//! maximal stratum and permit later domains only to tighten as columns
//! accumulate. Guard identities, ordering policy, and the immutable lower-owner
//! snapshot remain fixed.
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
    FreshTaskEpoch, FreshTaskQuery, FreshTaskQueryTelemetry, GrowingTaskEpochState,
    ReusedTaskPartitionQuery,
};

#[cfg(test)]
mod tests;
