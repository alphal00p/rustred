use rustred::{
    CAMPAIGN_EXECUTION_RESOURCE_PROFILE_V1_SCHEMA, CampaignBytes, CampaignEstimatorRevision,
    CampaignExecutionFixedMemory, CampaignExecutionResourceProfile, CampaignExecutionWidthPlanner,
    CampaignExecutionWidthPlanningOutcome, CampaignMemoryEstimate, CampaignTaskMemoryEnvelope,
    CampaignTaskResourceEstimate,
};
use serde::{Deserialize, Serialize, Serializer};

use crate::cli::args::parse_memory_bytes;
use crate::cli::error::AppError;
use crate::{CampaignPreflightRequest, CampaignPreflightResult, MAX_OUTPUT_BYTES};

pub(crate) const CAMPAIGN_PREFLIGHT_OUTPUT_SCHEMA: &str =
    "rustred.campaign-execution-preflight-output.toml.v1";
const UNSIGNED_DECIMAL_STRING_ENCODING: &str = "unsigned-decimal-string";

fn serialize_u64_as_unsigned_decimal_string<S>(
    value: &u64,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.collect_str(value)
}

fn serialize_usize_as_unsigned_decimal_string<S>(
    value: &usize,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.collect_str(value)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CampaignExecutionResourceProfileDocumentV1 {
    schema: String,
    estimator_revision: u64,
    enclosing_memory_limit: String,
    fixed_memory: FixedMemoryDocumentV1,
    minimum_runnable_task: MinimumRunnableTaskDocumentV1,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixedMemoryDocumentV1 {
    process_runtime_and_shared_catalogs: String,
    coordinator_stack_tls_workspace: String,
    per_worker_stack_tls_workspace: String,
    explicitly_admitted_inner_threads: String,
    hydrated_retained_lanes: String,
    staged_results: String,
    checkpoint_and_output_buffers: String,
    safety_reserve: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MinimumRunnableTaskDocumentV1 {
    retained_output: MemoryEstimateDocumentV1,
    transient_excluding_output: MemoryEstimateDocumentV1,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MemoryEstimateDocumentV1 {
    visible_logical: String,
    opaque_native_reserve: String,
}

#[derive(Debug, Serialize)]
struct CampaignPreflightOutputV1 {
    schema: &'static str,
    status: &'static str,
    frontier: &'static str,
    profile_schema: &'static str,
    unsigned_integer_encoding: &'static str,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    estimator_revision: u64,
    #[serde(serialize_with = "serialize_usize_as_unsigned_decimal_string")]
    requested_core_ceiling: usize,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    enclosing_memory_limit_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    operational_memory_limit_bytes: u64,
    fixed_memory: FixedMemoryOutputV1,
    minimum_runnable_task: MinimumRunnableTaskOutputV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    ready: Option<ReadyOutputV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pause: Option<PauseOutputV1>,
}

#[derive(Debug, Serialize)]
struct FixedMemoryOutputV1 {
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    process_runtime_and_shared_catalogs_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    coordinator_stack_tls_workspace_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    per_worker_stack_tls_workspace_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    explicitly_admitted_inner_threads_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    hydrated_retained_lanes_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    staged_results_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    checkpoint_and_output_buffers_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    safety_reserve_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    non_worker_total_bytes: u64,
}

#[derive(Debug, Serialize)]
struct MinimumRunnableTaskOutputV1 {
    #[serde(serialize_with = "serialize_usize_as_unsigned_decimal_string")]
    cores: usize,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    retained_output_visible_logical_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    retained_output_opaque_native_reserve_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    retained_output_total_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    transient_excluding_output_visible_logical_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    transient_excluding_output_opaque_native_reserve_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    transient_excluding_output_total_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    peak_additional_bytes: u64,
}

#[derive(Debug, Serialize)]
struct ReadyOutputV1 {
    #[serde(serialize_with = "serialize_usize_as_unsigned_decimal_string")]
    effective_width: usize,
    #[serde(serialize_with = "serialize_usize_as_unsigned_decimal_string")]
    worker_thread_count: usize,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    selected_fixed_memory_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    minimum_required_memory_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    operational_headroom_after_minimum_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    enclosing_headroom_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    admission_fixed_and_shared_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    admission_hydrated_retained_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    admission_staged_results_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    admission_total_bytes: u64,
}

#[derive(Debug, Serialize)]
struct PauseOutputV1 {
    kind: &'static str,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    inline_fixed_memory_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    inline_minimum_required_memory_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_unsigned_decimal_string")]
    memory_shortfall_bytes: u64,
}

pub(crate) fn preflight_request(
    request: CampaignPreflightRequest,
) -> Result<CampaignPreflightResult, AppError> {
    // This command intentionally stops at the pure width planner. In
    // particular, it never consumes a Ready plan and therefore does not
    // construct the campaign executor or its worker pool.
    let document: CampaignExecutionResourceProfileDocumentV1 = toml::from_str(&request.profile)
        .map_err(|error| {
            AppError::Input(format!(
                "invalid RustRed campaign execution resource profile TOML: {error}"
            ))
        })?;
    let (profile, enclosing_memory_limit) = prepare_profile(document)?;
    let estimator_revision = profile.estimator_revision();
    let fixed_memory = profile.fixed_memory();
    let minimum_runnable_task = profile.minimum_runnable_task();
    let requested_core_ceiling = request.n_cores;
    let operational_memory_limit_bytes = request.max_memory_bytes;
    let request = profile
        .try_into_width_request(
            requested_core_ceiling,
            enclosing_memory_limit,
            CampaignBytes::new(operational_memory_limit_bytes),
        )
        .map_err(|error| AppError::Input(format!("invalid campaign execution limits: {error}")))?;
    let outcome = CampaignExecutionWidthPlanner::try_plan(request).map_err(|error| {
        AppError::Input(format!("cannot plan campaign execution width: {error}"))
    })?;
    let mut output = CampaignPreflightOutputV1 {
        schema: CAMPAIGN_PREFLIGHT_OUTPUT_SCHEMA,
        status: "ready",
        frontier: "not_started",
        profile_schema: CAMPAIGN_EXECUTION_RESOURCE_PROFILE_V1_SCHEMA,
        unsigned_integer_encoding: UNSIGNED_DECIMAL_STRING_ENCODING,
        estimator_revision: estimator_revision.get(),
        requested_core_ceiling,
        enclosing_memory_limit_bytes: enclosing_memory_limit.get(),
        operational_memory_limit_bytes,
        fixed_memory: fixed_memory_output(fixed_memory),
        minimum_runnable_task: minimum_task_output(minimum_runnable_task),
        ready: None,
        pause: None,
    };
    match outcome {
        CampaignExecutionWidthPlanningOutcome::Ready(plan) => {
            let baseline = plan.admission_baseline();
            output.ready = Some(ReadyOutputV1 {
                effective_width: plan.effective_width(),
                worker_thread_count: plan.worker_thread_count(),
                selected_fixed_memory_bytes: plan.selected_fixed_memory().get(),
                minimum_required_memory_bytes: plan.minimum_required_memory().get(),
                operational_headroom_after_minimum_bytes: plan
                    .operational_headroom_after_minimum()
                    .get(),
                enclosing_headroom_bytes: plan.enclosing_headroom().get(),
                admission_fixed_and_shared_bytes: baseline.fixed_and_shared().get(),
                admission_hydrated_retained_bytes: baseline.hydrated_retained().get(),
                admission_staged_results_bytes: baseline.staged_results().get(),
                admission_total_bytes: baseline.total().get(),
            });
        }
        CampaignExecutionWidthPlanningOutcome::PausedForMemoryCapacity(pause) => {
            output.status = "paused_for_memory_capacity";
            output.pause = Some(PauseOutputV1 {
                kind: "memory_capacity",
                inline_fixed_memory_bytes: pause.inline_fixed_memory().get(),
                inline_minimum_required_memory_bytes: pause.inline_minimum_required_memory().get(),
                memory_shortfall_bytes: pause.memory_shortfall().get(),
            });
        }
    }
    let status = output.status;
    let serialized = serialize_preflight_output(&output)?;
    Ok(CampaignPreflightResult::new(
        CAMPAIGN_PREFLIGHT_OUTPUT_SCHEMA,
        status,
        serialized,
    ))
}

fn prepare_profile(
    document: CampaignExecutionResourceProfileDocumentV1,
) -> Result<(CampaignExecutionResourceProfile, CampaignBytes), AppError> {
    if document.schema != CAMPAIGN_EXECUTION_RESOURCE_PROFILE_V1_SCHEMA {
        return Err(AppError::Input(format!(
            "unsupported campaign execution resource profile schema {:?}; expected {:?}",
            document.schema, CAMPAIGN_EXECUTION_RESOURCE_PROFILE_V1_SCHEMA
        )));
    }
    let revision = CampaignEstimatorRevision::try_new(document.estimator_revision)
        .map_err(|error| AppError::Input(format!("invalid estimator_revision: {error}")))?;
    let enclosing_memory_limit = profile_memory(
        "enclosing_memory_limit",
        &document.enclosing_memory_limit,
        false,
    )?;
    let fixed = document.fixed_memory;
    let fixed_memory = CampaignExecutionFixedMemory::try_new(
        profile_memory(
            "fixed_memory.process_runtime_and_shared_catalogs",
            &fixed.process_runtime_and_shared_catalogs,
            true,
        )?,
        profile_memory(
            "fixed_memory.coordinator_stack_tls_workspace",
            &fixed.coordinator_stack_tls_workspace,
            true,
        )?,
        profile_memory(
            "fixed_memory.per_worker_stack_tls_workspace",
            &fixed.per_worker_stack_tls_workspace,
            true,
        )?,
        profile_memory(
            "fixed_memory.explicitly_admitted_inner_threads",
            &fixed.explicitly_admitted_inner_threads,
            true,
        )?,
        profile_memory(
            "fixed_memory.hydrated_retained_lanes",
            &fixed.hydrated_retained_lanes,
            true,
        )?,
        profile_memory("fixed_memory.staged_results", &fixed.staged_results, true)?,
        profile_memory(
            "fixed_memory.checkpoint_and_output_buffers",
            &fixed.checkpoint_and_output_buffers,
            true,
        )?,
        profile_memory("fixed_memory.safety_reserve", &fixed.safety_reserve, true)?,
    )
    .map_err(|error| AppError::Input(format!("invalid fixed_memory: {error}")))?;
    let retained = prepare_memory_estimate(
        "minimum_runnable_task.retained_output",
        document.minimum_runnable_task.retained_output,
    )?;
    let transient = prepare_memory_estimate(
        "minimum_runnable_task.transient_excluding_output",
        document.minimum_runnable_task.transient_excluding_output,
    )?;
    let envelope = CampaignTaskMemoryEnvelope::try_new(retained, transient).map_err(|error| {
        AppError::Input(format!(
            "invalid minimum_runnable_task memory envelope: {error}"
        ))
    })?;
    let task = CampaignTaskResourceEstimate::try_new(revision, 1, envelope).map_err(|error| {
        AppError::Input(format!(
            "invalid minimum_runnable_task resource estimate: {error}"
        ))
    })?;
    let profile = CampaignExecutionResourceProfile::try_new(revision, fixed_memory, task)
        .map_err(|error| AppError::Input(format!("invalid execution resource profile: {error}")))?;
    Ok((profile, enclosing_memory_limit))
}

fn prepare_memory_estimate(
    field: &'static str,
    document: MemoryEstimateDocumentV1,
) -> Result<CampaignMemoryEstimate, AppError> {
    CampaignMemoryEstimate::try_new(
        profile_memory(
            &format!("{field}.visible_logical"),
            &document.visible_logical,
            true,
        )?,
        profile_memory(
            &format!("{field}.opaque_native_reserve"),
            &document.opaque_native_reserve,
            true,
        )?,
    )
    .map_err(|error| AppError::Input(format!("invalid {field}: {error}")))
}

fn profile_memory(field: &str, value: &str, allow_zero: bool) -> Result<CampaignBytes, AppError> {
    let parsed = parse_memory_bytes(value)
        .filter(|bytes| allow_zero || *bytes > 0)
        .ok_or_else(|| {
            let quantity = if allow_zero { "an integer" } else { "a positive integer" };
            AppError::Input(format!(
                "invalid {field} value {value:?}; expected {quantity} followed by B, KiB, MiB, GiB, or TiB"
            ))
        })?;
    Ok(CampaignBytes::new(parsed))
}

fn fixed_memory_output(fixed: CampaignExecutionFixedMemory) -> FixedMemoryOutputV1 {
    FixedMemoryOutputV1 {
        process_runtime_and_shared_catalogs_bytes: fixed
            .process_runtime_and_shared_catalogs()
            .get(),
        coordinator_stack_tls_workspace_bytes: fixed.coordinator_stack_tls_workspace().get(),
        per_worker_stack_tls_workspace_bytes: fixed.per_worker_stack_tls_workspace().get(),
        explicitly_admitted_inner_threads_bytes: fixed.explicitly_admitted_inner_threads().get(),
        hydrated_retained_lanes_bytes: fixed.hydrated_retained_lanes().get(),
        staged_results_bytes: fixed.staged_results().get(),
        checkpoint_and_output_buffers_bytes: fixed.checkpoint_and_output_buffers().get(),
        safety_reserve_bytes: fixed.safety_reserve().get(),
        non_worker_total_bytes: fixed.non_worker_total().get(),
    }
}

fn minimum_task_output(task: CampaignTaskResourceEstimate) -> MinimumRunnableTaskOutputV1 {
    let retained = task.memory().retained_output();
    let transient = task.memory().transient_excluding_output();
    MinimumRunnableTaskOutputV1 {
        cores: task.cores(),
        retained_output_visible_logical_bytes: retained.visible_logical().get(),
        retained_output_opaque_native_reserve_bytes: retained.opaque_native_reserve().get(),
        retained_output_total_bytes: retained.total().get(),
        transient_excluding_output_visible_logical_bytes: transient.visible_logical().get(),
        transient_excluding_output_opaque_native_reserve_bytes: transient
            .opaque_native_reserve()
            .get(),
        transient_excluding_output_total_bytes: transient.total().get(),
        peak_additional_bytes: task.memory().peak_additional().get(),
    }
}

fn serialize_preflight_output(output: &CampaignPreflightOutputV1) -> Result<String, AppError> {
    let mut serialized = toml::to_string_pretty(output).map_err(|error| {
        AppError::Serialization(format!(
            "cannot serialize campaign execution preflight TOML: {error}"
        ))
    })?;
    if !serialized.ends_with('\n') {
        serialized.push('\n');
    }
    if serialized.len() > MAX_OUTPUT_BYTES {
        return Err(AppError::Serialization(format!(
            "campaign execution preflight TOML needs {} bytes, exceeding the {MAX_OUTPUT_BYTES}-byte CLI limit",
            serialized.len()
        )));
    }
    Ok(serialized)
}
