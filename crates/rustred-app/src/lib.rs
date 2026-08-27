mod application;
mod cli;

pub use application::{
    AppError, AppErrorKind, CampaignPlanRequest, CampaignPlanResult, CampaignPreflightRequest,
    CampaignPreflightResult, DeriveRequest, DeriveResult, InputFormat, MAX_INPUT_BYTES,
    MAX_OUTPUT_BYTES, ParseInputFormatError, ParseRelationSelectionError, RelationSelection,
    campaign_plan, campaign_preflight, derive,
};

/// Run the command-line adapter and return its stable process exit code.
pub fn cli_main_entry() -> i32 {
    cli::main_entry()
}
