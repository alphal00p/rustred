mod api;
mod cli;

pub use api::{
    CampaignPlanRequest, CampaignPlanResult, CampaignPreflightRequest, CampaignPreflightResult,
    DeriveRequest, DeriveResult, InputFormat, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES, RelationSelection,
    campaign_plan, campaign_preflight, derive,
};
pub use cli::args::ArgError;
pub use cli::error::AppError;

/// Run the command-line adapter and return its stable process exit code.
pub fn cli_main_entry() -> i32 {
    cli::main_entry()
}
