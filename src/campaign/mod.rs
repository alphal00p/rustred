//! Deterministic planning, resource admission, and execution ownership for
//! topology-neutral RustRed campaigns.
//!
//! This boundary owns campaign identity, work scheduling, calibrated resource
//! policy, width selection, and move-only runtime admission. It performs no
//! symbolic algebra and does not own family derivation or solver semantics.

mod admission;
mod execution;
mod execution_width;
mod plan;
mod resource_profile;
mod resources;
mod work;

pub use admission::{
    CAMPAIGN_ADMISSION_V1_SCHEMA, CampaignAdmissionController, CampaignAdmissionError,
    CampaignAdmissionSnapshot, CampaignAdmissionUsage, CampaignAdmittedTask, CampaignCommitFailure,
    CampaignResident, CampaignResidentToken, CampaignResidentTransformBatchAdmissionFailure,
    CampaignResidentTransformBindFailure, CampaignResidentTransformBuildFailure,
    CampaignResidentTransformExecution, CampaignResidentTransformFailure,
    CampaignResidentTransformPanic, CampaignResidentTransformTask, CampaignTaskContext,
    CampaignTaskExecution, CampaignTaskFailure, CampaignTaskPanic, CampaignTaskReservation,
    CampaignWaveExecutionAdmissionFailure, CampaignWaveReservation,
};
pub use execution::{ParallelExecution, ParallelExecutionError};
pub use execution_width::{
    CAMPAIGN_EXECUTION_WIDTH_PLAN_V1_SCHEMA, CampaignExecutionFixedMemory,
    CampaignExecutionWidthError, CampaignExecutionWidthPause, CampaignExecutionWidthPlan,
    CampaignExecutionWidthPlanner, CampaignExecutionWidthPlanningOutcome,
    CampaignExecutionWidthRequest,
};
pub use plan::{
    CAMPAIGN_PLAN_V1_SCHEMA, CampaignDependencyInsertion, CampaignFamilyId, CampaignFamilyRecord,
    CampaignJobKey, CampaignPlan, CampaignPlanError, CampaignPlanLimits, CampaignPlanStats,
    CampaignRootId, CampaignRootInsertion, CampaignRootRecord, CampaignRootSpec,
    PlannedCampaignJob, ProperSubsectorWitness,
};
pub use resource_profile::{
    CAMPAIGN_EXECUTION_RESOURCE_PROFILE_V1_SCHEMA, CampaignExecutionResourceProfile,
    CampaignExecutionResourceProfileError,
};
pub use resources::{
    CAMPAIGN_RESOURCE_POLICY_V1_SCHEMA, CampaignBaselineMemory, CampaignBytes,
    CampaignEstimatorRevision, CampaignMemoryEstimate, CampaignResourceError,
    CampaignResourcePolicy, CampaignResourceWavePlan, CampaignTaskMemoryEnvelope,
    CampaignTaskResourceEstimate, CampaignWavePlanner,
};
pub use work::{CampaignWorkKey, CampaignWorkUnitKey};

pub(crate) use admission::CampaignFixedComponentReservation;
