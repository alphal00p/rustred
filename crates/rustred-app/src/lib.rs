mod application;
pub use application::{EntryPowerBudget, FiniteEntryDomain, entry_domain_plan};
#[cfg(feature = "cli")]
mod cli;

pub use application::{
    OWNER_DOMAIN_WALK_CHECKPOINT_FORMAT, OWNER_DOMAIN_WALK_CHECKPOINT_MANIFEST_MAX_BYTES,
    OWNER_DOMAIN_WALK_CHECKPOINT_SCHEMA, OWNER_DOMAIN_WALK_SEMANTICS_VERSION,
    OwnerDomainMatchRequest, OwnerDomainMatchResult, OwnerDomainScanRequest, OwnerDomainScanResult,
    OwnerDomainWalkApplySubdivision, OwnerDomainWalkCheckpointOptions,
    OwnerDomainWalkPublicationPolicy, OwnerDomainWalkRequest, OwnerDomainWalkResult,
    OwnerDomainWalkSchedulingPolicy, OwnerGuardedApplyRequest, OwnerGuardedApplyResult,
    RoutedCampaignRequest, RoutedCampaignResult, RoutedEntryWitnessLimits,
    RoutedEntryWitnessProposal, RoutedEntryWitnessRoundResult, RoutedEntryWitnessStats,
    RoutedFeedbackFixedResidualPolicy, RoutedFeedbackNomination, RoutedFeedbackOptions,
    RoutedFeedbackRoundResult, RoutedFeedbackSession, owner_domain_match_with_progress,
    owner_domain_scan_with_progress, owner_domain_walk_with_progress,
    owner_guarded_apply_with_progress, routed_campaign_with_progress,
};
pub use rustred::persistence::{BinaryIoLimits, equivalent_generated_programs};

pub use application::{
    AppError, AppErrorKind, ArtifactLoadLimits, CANDIDATE_BUNDLE_SCHEMA,
    CANDIDATE_CERTIFICATION_SCHEMA, CampaignPlanRequest, CampaignPlanResult,
    CampaignPreflightRequest, CampaignPreflightResult, CandidateBundleInspection,
    CandidateBundleLimits, CandidateBundleResult, CandidateCertificationRequest,
    CandidateCertificationResult, CandidateCheckpointOptions, CandidateExactBackend,
    CandidateOwnerBundle, CandidateOwnerLoadLimits, CaseIntersectionLimits,
    ClosingArtifactGenerateRequest, ClosingArtifactGenerateResult, ClosingArtifactInspectRequest,
    ClosingArtifactInspectResult, ClosingArtifactReduceRequest, ClosingArtifactReduceResult,
    ClosingFamilySelector, DeriveRequest, DeriveResult, ExactMasterCoefficient,
    FAMILY_CANDIDATES_SCHEMA, FAMILY_CLOSE_SCHEMA, FOUNDRY_CAMPAIGN_MEASUREMENTS_SCHEMA,
    FOUNDRY_WAVE_CAMPAIGN_MEASUREMENTS_SCHEMA, FOUNDRY_WAVE_CAMPAIGN_REPORT_SCHEMA,
    FamilyCandidatesRequest, FamilyCloseGenerationStage, FamilyCloseProgress, FamilyCloseRequest,
    FamilyCloseResult, FamilySolveRequest, FamilySolveResult, FiniteCaseLimits, FiniteCasePolicy,
    FoundryCampaignCensus, FoundryCampaignCoverageObstruction, FoundryCampaignCoverageStatus,
    FoundryCampaignNeedsRefinementReason, FoundryCampaignOperationalLimit, FoundryCampaignProgress,
    FoundryCampaignRunRequest, FoundryCampaignRunResult, FoundryCampaignSnapshot,
    FoundryCampaignStop, FoundryCampaignTaskLocation, FoundryCampaignTaskLocationKind,
    FoundryWaveCampaignRunRequest, FoundryWaveCampaignRunResult, InputFormat,
    K6OrbitCampaignProgress, K6OrbitCampaignState, K6WaveCampaignProgress, K6WaveCampaignState,
    MAX_CANDIDATE_BUNDLE_BYTES, MAX_CLOSING_ARTIFACT_BYTES, MAX_CLOSING_RULE_APPLICATIONS,
    MAX_FOUNDRY_CAMPAIGN_PROBES, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES,
    ParseClosingFamilySelectorError, ParseInputFormatError, ParseRelationSelectionError,
    RelationSelection, SourcePortLimits, campaign_plan, campaign_preflight, certify_candidates,
    certify_candidates_with_progress, closing_artifact_generate, closing_artifact_inspect,
    closing_artifact_reduce, derive, encode_generated_candidate_sector, family_candidates,
    family_candidates_with_progress, family_close, family_close_with_progress, family_solve,
    foundry_campaign_run, foundry_campaign_run_with_progress, foundry_wave_campaign_run,
    foundry_wave_campaign_run_with_progress, inspect_generated_candidate_bundle,
    load_generated_candidate_bundle, load_generated_candidate_checkpoint,
    load_generated_candidate_owners,
};

/// Run the command-line adapter and return its stable process exit code.
#[cfg(feature = "cli")]
pub fn cli_main_entry() -> i32 {
    cli::main_entry()
}
