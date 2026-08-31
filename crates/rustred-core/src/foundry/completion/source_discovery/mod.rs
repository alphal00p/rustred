//! Bounded inverse-incidence discovery of exact translated-source requests.
//!
//! This layer only nominates rows from a sealed ordinary-source module.  A
//! nomination is neither a modular hit nor an exact relation, and no result
//! here can authorize a rule, owner, terminal, artifact, or closure claim.

mod campaign;
mod canonical_replay;
mod dual;
mod error;
mod incidence;
mod limits;
mod model;
mod nominate;
mod owner_bundle;
mod promotion;
mod residual;
pub(crate) mod scheduler;

// The probe-local scheduler consumes this sealed campaign boundary. A few
// evidence and telemetry types remain reexported for sibling admission tests,
// so keep the seam explicit without widening it outside completion.
#[allow(unused_imports)]
pub(crate) use campaign::{
    AccumulatedSourceRequests, CampaignBudgetExhaustion, CampaignError, CampaignLimits,
    CampaignModularProbe, CampaignRequestMerge, CampaignRequestMergeTelemetry,
    CampaignResourceStage, CandidateBatchExhaustionTelemetry, FreshTaskBuildTelemetry,
    FreshTaskEpoch, FreshTaskQuery, FreshTaskQueryTelemetry, GrowingTaskEpochState,
    ReusedTaskPartitionQuery,
};
#[allow(unused_imports)] // Consumed by the staged sector-layer orchestrator.
pub(crate) use canonical_replay::{
    CanonicalRebaseAttempt, CanonicalRebaseAttemptOutcome, CanonicalRebasedCandidate,
    CanonicalReplayBatch, CanonicalReplayDisposition, CanonicalReplayError, CanonicalReplayLimits,
    CanonicalReplayTelemetry, try_canonicalize_replayed_probes,
};
#[allow(unused_imports)]
pub(crate) use dual::{
    SampledDeclaredModuleDual, SampledDeclaredModuleDualCensus, SampledDeclaredModuleDualError,
    SampledDeclaredModuleDualObstructionEntry, SampledDeclaredModuleDualRankCensus,
};
pub(crate) use error::SourceDiscoveryError;
pub(crate) use incidence::OrdinarySourceIncidenceIndex;
pub(crate) use limits::SourceDiscoveryLimits;
pub(crate) use model::{
    IncidentTranslationNominations, NonzeroIncidentTranslationResiduals, ResidualProposalScore,
};
#[allow(unused_imports)] // Consumed by the staged sector-layer orchestrator.
pub(crate) use owner_bundle::{
    ExactExecutableCandidateObstruction, ExactExecutableOwnerCover, ExactExecutableOwnerError,
    ExactExecutableOwnerLimits, ExactExecutableOwnerObstruction, ExactExecutableOwnerProposal,
    ExactExecutableOwnerSelection, ExactSemanticExecutableOwner, UnpublishedCanonicalOwnerProposal,
    try_compile_canonical_executable_owner,
};
#[allow(unused_imports)] // Consumed by the staged sector-layer orchestrator.
pub(crate) use promotion::{
    AdmittedExactRuleCandidate, ExactRuleCellGuardObstruction, ExactRuleCellPromotionDisposition,
    ExactRuleCellPromotionError, ExactRuleCellPromotionLimits, try_promote_replayed_rule_cell,
    try_promote_replayed_rule_cell_on_partition,
};
#[cfg(test)]
mod tests;
