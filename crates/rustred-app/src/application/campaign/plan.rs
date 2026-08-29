mod decode;
mod intern;
mod render;

use crate::application::error::AppError;
use crate::application::{CampaignPlanRequest, CampaignPlanResult};

pub(crate) const CAMPAIGN_OUTPUT_SCHEMA: &str = "rustred.campaign-plan-output.toml.v1";

pub(crate) fn plan_request(request: CampaignPlanRequest) -> Result<CampaignPlanResult, AppError> {
    let roots =
        decode::prepare_campaign_roots(&request.source, request.input_format, request.root_id)?;
    let output = intern::compile_roots_only_output(roots)?;
    let serialized = render::serialize_campaign_output(&output)?;
    Ok(CampaignPlanResult::new(
        CAMPAIGN_OUTPUT_SCHEMA,
        "ok",
        serialized,
    ))
}
