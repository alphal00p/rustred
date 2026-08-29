//! Memory-admitted execution-width planning before any worker pool exists.
//!
//! This module is deliberately algebra-free and host-independent. It turns
//! the invocation-wide core ceiling and a calibrated fixed-memory breakdown
//! into the largest feasible effective width. It never inspects topology data
//! or constructs a worker pool.

mod arithmetic;
mod error;
mod model;
mod planner;

pub use error::CampaignExecutionWidthError;
pub use model::{
    CampaignExecutionFixedMemory, CampaignExecutionWidthPause, CampaignExecutionWidthPlan,
    CampaignExecutionWidthPlanningOutcome, CampaignExecutionWidthRequest,
};
pub use planner::CampaignExecutionWidthPlanner;

#[cfg(test)]
mod tests;
