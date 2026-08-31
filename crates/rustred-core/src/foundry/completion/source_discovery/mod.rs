//! Bounded inverse-incidence discovery of exact translated-source requests.
//!
//! This layer only nominates rows from a sealed ordinary-source module.  A
//! nomination is neither a modular hit nor an exact relation, and no result
//! here can authorize a rule, owner, terminal, artifact, or closure claim.

mod campaign;
mod canonical_replay;
mod cover_delta;
mod dual;
mod error;
mod incidence;
mod interior_campaign;
mod interior_replay;
pub(crate) mod interior_simplex;
pub(crate) mod leader_walk;
mod limits;
mod model;
mod nominate;
mod obstruction_block;
mod owner_bundle;
mod promotion;
mod residual;
pub(crate) mod scheduler;
mod sector_closure;
mod triangular_support;

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
#[allow(unused_imports)] // Consumed by the staged interior-replay campaign.
pub(crate) use cover_delta::{
    CanonicalExactOwnerLedger, ExactOwnerCoverDelta, ExactOwnerCoverDeltaError,
    ExactOwnerCoverDeltaKind, ExactOwnerCoverDeltaLimits, ExactOwnerCoverSnapshot,
    ExactOwnerLedgerCoverStatus,
};
#[allow(unused_imports)]
pub(crate) use dual::{
    SampledDeclaredModuleDual, SampledDeclaredModuleDualCensus, SampledDeclaredModuleDualError,
    SampledDeclaredModuleDualObstructionEntry, SampledDeclaredModuleDualRankCensus,
};
pub(crate) use error::SourceDiscoveryError;
pub(crate) use incidence::OrdinarySourceIncidenceIndex;
#[allow(unused_imports)] // Consumed by the staged blind-sector campaign.
pub(crate) use interior_campaign::{
    InteriorCampaignAdapter, InteriorCampaignAppliedOwner, InteriorCampaignBootstrapCensus,
    InteriorCampaignCensus, InteriorCampaignError, InteriorCampaignLimits,
    InteriorCampaignNoProposal, InteriorCampaignOutcome, InteriorCampaignOwnerEffect,
    InteriorCampaignTaskBinding, InteriorCampaignTaskReport,
};
#[allow(unused_imports)] // Consumed by the staged interior-simplex orchestrator.
pub(crate) use interior_replay::{
    InteriorReplayAttemptCensus, InteriorReplayCandidateSupport, InteriorReplayRelativeResidual,
    InteriorReplayRelativeSource, InteriorReplayRunDisposition, InteriorReplayRunError,
    InteriorReplayRunLimits, InteriorReplaySchedulerOutcomeCensus, InteriorReplaySupportCensus,
    InteriorReplaySupportSet, InteriorReplayTaskReport, support_shapes_match,
    try_run_interior_replay_task,
};
pub(crate) use limits::SourceDiscoveryLimits;
pub(crate) use model::{
    IncidentTranslationNominations, NonzeroIncidentTranslationResiduals, ResidualProposalScore,
};
#[allow(unused_imports)] // Consumed by the proposal-only width-four research lane.
pub(crate) use obstruction_block::{
    ObstructionBlockNominationPlan, ObstructionBlockNominationUpperBound,
    ObstructionBlockNominations, ObstructionBlockProposalBatch, ObstructionBlockProposalCandidate,
    ObstructionBlockProposalScore, ObstructionBlockProposalTelemetry, ProbeRowEvaluationCache,
    ProbeRowEvaluationCacheTelemetry, UnionObstructionSupportEntry, UnionSupportNominations,
    try_select_obstruction_block_proposals,
};
#[allow(unused_imports)] // Consumed by the staged sector-layer orchestrator.
pub(crate) use owner_bundle::{
    ClosedExactExecutableOwnerCover, ClosedSectorLayer, ClosedSectorLayerContentId,
    ExactExecutableCandidateObstruction, ExactExecutableOwnerCover, ExactExecutableOwnerError,
    ExactExecutableOwnerLimits, ExactExecutableOwnerObstruction, ExactExecutableOwnerProposal,
    ExactExecutableOwnerSelection, ExactSemanticExecutableOwner, UnpublishedCanonicalOwnerProposal,
    compare_exact_owner_group_content, compare_exact_owner_proof_content,
    try_compile_canonical_executable_owner,
};
#[allow(unused_imports)] // Consumed by the staged sector-layer orchestrator.
pub(crate) use promotion::{
    AdmittedExactRuleCandidate, ExactRuleCellGuardObstruction, ExactRuleCellPromotionDisposition,
    ExactRuleCellPromotionError, ExactRuleCellPromotionLimits, try_promote_replayed_rule_cell,
    try_promote_replayed_rule_cell_on_partition,
};
#[allow(unused_imports)] // Used by the staged K6 rank-three publication wave.
pub(crate) use sector_closure::{
    ClosedSectorClosureWave, StagedSectorClosureCoordinator, StagedSectorClosureError,
    StagedSectorClosureLimits, StagedSectorClosureOutcome, StagedSectorClosureStop,
    StagedSectorClosureStopEvidence,
};
#[allow(unused_imports)] // Generic seed frame for the staged K6 discovery portfolio.
pub(crate) use triangular_support::{TriangularSupportError, try_build_triangular_support_frame};
#[cfg(test)]
mod tests;
