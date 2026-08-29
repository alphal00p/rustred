//! Deterministic resource preflight and execution ownership for RustRed
//! campaigns.
//!
//! This boundary owns calibrated resource metadata, width selection, and
//! bounded ordered execution. Application-owned root planning composes these
//! primitives without entering the algebraic core.

mod execution;
mod execution_width;
mod resource_profile;
mod resources;
pub use execution::{ParallelExecution, ParallelExecutionError};
pub use execution_width::{
    CAMPAIGN_EXECUTION_WIDTH_PLAN_V1_SCHEMA, CampaignExecutionFixedMemory,
    CampaignExecutionWidthError, CampaignExecutionWidthPause, CampaignExecutionWidthPlan,
    CampaignExecutionWidthPlanner, CampaignExecutionWidthPlanningOutcome,
    CampaignExecutionWidthRequest,
};
pub use resource_profile::{
    CAMPAIGN_EXECUTION_RESOURCE_PROFILE_V1_SCHEMA, CampaignExecutionResourceProfile,
    CampaignExecutionResourceProfileError,
};
pub use resources::{
    CampaignBaselineMemory, CampaignBytes, CampaignEstimatorRevision, CampaignMemoryEstimate,
    CampaignResourceError, CampaignTaskMemoryEnvelope, CampaignTaskResourceEstimate,
};
