mod application;
#[cfg(feature = "cli")]
mod cli;

pub use rustred::persistence::{BinaryIoLimits, equivalent_generated_programs};

pub use application::{
    AppError, AppErrorKind, ArtifactLoadLimits, CANDIDATE_BUNDLE_SCHEMA,
    CANDIDATE_CERTIFICATION_SCHEMA, CampaignPlanRequest, CampaignPlanResult,
    CampaignPreflightRequest, CampaignPreflightResult, CandidateBundleInspection,
    CandidateBundleLimits, CandidateBundleResult, CandidateCertificationRequest,
    CandidateCertificationResult, CandidateExactBackend, ClosingArtifactGenerateRequest,
    ClosingArtifactGenerateResult, ClosingArtifactInspectRequest, ClosingArtifactInspectResult,
    ClosingArtifactReduceRequest, ClosingArtifactReduceResult, ClosingFamilySelector,
    DeriveRequest, DeriveResult, ExactMasterCoefficient, FAMILY_CANDIDATES_SCHEMA,
    FAMILY_CLOSE_SCHEMA, FOUNDRY_CAMPAIGN_MEASUREMENTS_SCHEMA,
    FOUNDRY_WAVE_CAMPAIGN_MEASUREMENTS_SCHEMA, FOUNDRY_WAVE_CAMPAIGN_REPORT_SCHEMA,
    FamilyCandidatesRequest, FamilyCloseGenerationStage, FamilyCloseProgress, FamilyCloseRequest,
    FamilyCloseResult, FamilySolveRequest, FamilySolveResult, FoundryCampaignCensus,
    FoundryCampaignCoverageObstruction, FoundryCampaignCoverageStatus,
    FoundryCampaignNeedsRefinementReason, FoundryCampaignOperationalLimit, FoundryCampaignProgress,
    FoundryCampaignRunRequest, FoundryCampaignRunResult, FoundryCampaignSnapshot,
    FoundryCampaignStop, FoundryCampaignTaskLocation, FoundryCampaignTaskLocationKind,
    FoundryWaveCampaignRunRequest, FoundryWaveCampaignRunResult, InputFormat,
    K6OrbitCampaignProgress, K6OrbitCampaignState, K6WaveCampaignProgress, K6WaveCampaignState,
    MAX_CANDIDATE_BUNDLE_BYTES, MAX_CLOSING_ARTIFACT_BYTES, MAX_CLOSING_RULE_APPLICATIONS,
    MAX_FOUNDRY_CAMPAIGN_PROBES, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES,
    ParseClosingFamilySelectorError, ParseInputFormatError, ParseRelationSelectionError,
    RelationSelection, SourcePortLimits, campaign_plan, campaign_preflight, certify_candidates,
    closing_artifact_generate, closing_artifact_inspect, closing_artifact_reduce, derive,
    family_candidates, family_candidates_with_progress, family_close, family_close_with_progress,
    family_solve, foundry_campaign_run, foundry_campaign_run_with_progress,
    foundry_wave_campaign_run, foundry_wave_campaign_run_with_progress,
    inspect_generated_candidate_bundle, load_generated_candidate_bundle,
};

/// Run the command-line adapter and return its stable process exit code.
#[cfg(feature = "cli")]
pub fn cli_main_entry() -> i32 {
    cli::main_entry()
}
