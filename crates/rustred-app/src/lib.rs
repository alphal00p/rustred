mod application;
mod cli;

pub use application::{
    AppError, AppErrorKind, CampaignPlanRequest, CampaignPlanResult, CampaignPreflightRequest,
    CampaignPreflightResult, ClosingArtifactGenerateRequest, ClosingArtifactGenerateResult,
    ClosingArtifactInspectRequest, ClosingArtifactInspectResult, ClosingArtifactReduceRequest,
    ClosingArtifactReduceResult, ClosingFamilySelector, DeriveRequest, DeriveResult,
    ExactMasterCoefficient, FOUNDRY_CAMPAIGN_MEASUREMENTS_SCHEMA,
    FOUNDRY_WAVE_CAMPAIGN_MEASUREMENTS_SCHEMA, FOUNDRY_WAVE_CAMPAIGN_REPORT_SCHEMA,
    FoundryCampaignCensus, FoundryCampaignCoverageObstruction, FoundryCampaignCoverageStatus,
    FoundryCampaignNeedsRefinementReason, FoundryCampaignOperationalLimit, FoundryCampaignProgress,
    FoundryCampaignRunRequest, FoundryCampaignRunResult, FoundryCampaignSnapshot,
    FoundryCampaignStop, FoundryCampaignTaskLocation, FoundryWaveCampaignRunRequest,
    FoundryWaveCampaignRunResult, InputFormat, MAX_CLOSING_ARTIFACT_BYTES,
    MAX_CLOSING_RULE_APPLICATIONS, MAX_FOUNDRY_CAMPAIGN_PROBES, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES,
    ParseClosingFamilySelectorError, ParseInputFormatError, ParseRelationSelectionError,
    RelationSelection, campaign_plan, campaign_preflight, closing_artifact_generate,
    closing_artifact_inspect, closing_artifact_reduce, derive, foundry_campaign_run,
    foundry_campaign_run_with_progress, foundry_wave_campaign_run,
};

/// Run the command-line adapter and return its stable process exit code.
pub fn cli_main_entry() -> i32 {
    cli::main_entry()
}
