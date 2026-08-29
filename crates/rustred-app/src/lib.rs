mod application;
mod cli;

pub use application::{
    AppError, AppErrorKind, CampaignPlanRequest, CampaignPlanResult, CampaignPreflightRequest,
    CampaignPreflightResult, ClosingArtifactGenerateRequest, ClosingArtifactGenerateResult,
    ClosingArtifactInspectRequest, ClosingArtifactInspectResult, ClosingArtifactReduceRequest,
    ClosingArtifactReduceResult, ClosingFamilySelector, DeriveRequest, DeriveResult,
    ExactMasterCoefficient, InputFormat, MAX_CLOSING_ARTIFACT_BYTES, MAX_CLOSING_RULE_APPLICATIONS,
    MAX_INPUT_BYTES, MAX_OUTPUT_BYTES, ParseClosingFamilySelectorError, ParseInputFormatError,
    ParseRelationSelectionError, RelationSelection, campaign_plan, campaign_preflight,
    closing_artifact_generate, closing_artifact_inspect, closing_artifact_reduce, derive,
};

/// Run the command-line adapter and return its stable process exit code.
pub fn cli_main_entry() -> i32 {
    cli::main_entry()
}
