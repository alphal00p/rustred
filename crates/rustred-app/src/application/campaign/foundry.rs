use std::time::Instant;

use rustred::foundry::campaign::{
    FOUNDRY_CAMPAIGN_CONFIG_SCHEMA, FOUNDRY_CAMPAIGN_REPORT_SCHEMA, FoundryCampaignCensus,
    FoundryCampaignConfig, FoundryCampaignCoverageObstruction, FoundryCampaignCoverageStatus,
    FoundryCampaignError, FoundryCampaignExternalHints, FoundryCampaignItinerary,
    FoundryCampaignNeedsRefinementReason, FoundryCampaignOperationalLimit, FoundryCampaignPreset,
    FoundryCampaignProbe, FoundryCampaignProgress, FoundryCampaignReport,
    FoundryCampaignSchedulerRejection, FoundryCampaignSnapshot, FoundryCampaignStop,
    FoundryCampaignTaskLocation, FoundryCampaignUncoveredBox, FoundrySearchProvenance,
    run_foundry_campaign_with_progress,
};
use rustred::sector::{CoordinatePriority, CoordinatePriorityLimits, OrderingPolicy};
use serde::{Deserialize, Serialize};

use super::super::{
    FOUNDRY_CAMPAIGN_MEASUREMENTS_SCHEMA, FoundryCampaignRunRequest, FoundryCampaignRunResult,
    MAX_FOUNDRY_CAMPAIGN_PROBES, MAX_OUTPUT_BYTES,
};
use crate::application::error::AppError;

const DIAGNOSTIC_PUBLICATION: &str = "diagnostic_only";
const MEASUREMENT_SCOPE: &str = "config_parse_core_run_and_report_serialization";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FoundryCampaignConfigDocumentV2 {
    schema: String,
    preset: String,
    mode: String,
    max_task_reports: u64,
    max_reported_uncovered_boxes: u64,
    #[serde(default)]
    hints: Option<FoundryCampaignExternalHintsDocumentV2>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FoundryCampaignExternalHintsDocumentV2 {
    itinerary: String,
    probes: Vec<FoundryCampaignProbeDocumentV2>,
    interior_margin: u64,
    polynomial_degree_ceiling: u64,
    ordering_policy: String,
    #[serde(default)]
    discovery_coordinate_priority: Option<Vec<u64>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FoundryCampaignProbeDocumentV2 {
    modulus: u64,
    base_parameters: Vec<i64>,
    chart_offsets: Vec<u64>,
}

#[derive(Debug, Serialize)]
struct FoundryCampaignReportDocumentV1 {
    schema: &'static str,
    status: &'static str,
    publication: &'static str,
    artifact_published: bool,
    preset: &'static str,
    family_fingerprint: String,
    context_fingerprint: String,
    sector_active: Vec<bool>,
    closure_status: &'static str,
    configuration: FoundryCampaignConfigurationOutputV1,
    stop: FoundryCampaignStopOutputV1,
    census: FoundryCampaignCensusOutputV1,
    snapshot: FoundryCampaignSnapshotOutputV1,
    uncovered_partition: FoundryCampaignUncoveredPartitionOutputV1,
    uncovered_boxes: Vec<FoundryCampaignUncoveredBoxOutputV1>,
}

#[derive(Debug, Serialize)]
struct FoundryCampaignConfigurationOutputV1 {
    itinerary: &'static str,
    search_provenance: &'static str,
    interior_margin: u64,
    polynomial_degree_ceiling: usize,
    ordering_policy: String,
    /// Omitted for the natural chronology so legacy/default report bytes stay
    /// unchanged. This field never names the persisted proof ordering.
    #[serde(skip_serializing_if = "Option::is_none")]
    discovery_coordinate_priority: Option<Vec<usize>>,
    max_task_reports: usize,
    max_reported_uncovered_boxes: usize,
    probes: Vec<FoundryCampaignProbeOutputV1>,
}

#[derive(Debug, Serialize)]
struct FoundryCampaignProbeOutputV1 {
    modulus: u64,
    base_parameters: Vec<i64>,
    chart_offsets: Vec<u64>,
}

#[derive(Debug, Serialize)]
struct FoundryCampaignStopOutputV1 {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<FoundryCampaignTaskLocationOutputV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refinement: Option<FoundryCampaignRefinementOutputV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    operational_limit: Option<FoundryCampaignOperationalLimitOutputV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ledger_revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed_classes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed_tasks: Option<usize>,
}

#[derive(Debug, Serialize)]
struct FoundryCampaignTaskLocationOutputV1 {
    ledger_revision: u64,
    class_ordinal: usize,
    effective_dimension: usize,
    parent_free_dimension: usize,
    boundary_codimension: usize,
    task_ordinal: usize,
}

#[derive(Debug, Serialize)]
struct FoundryCampaignRefinementOutputV1 {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    exact_obstructions: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduler_stalls: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    canonical_query_rejections: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    coverage_status: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    coverage_obstruction: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    uncovered_is_finite: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_terminal_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    guard_incomplete_owner_count: Option<usize>,
}

#[derive(Debug, Serialize)]
struct FoundryCampaignOperationalLimitOutputV1 {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    requested: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduler_budget_stops: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduler_rejections: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduler_exact_lift_errors: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    terminal_scheduler_rejection: Option<FoundryCampaignSchedulerRejectionOutputV1>,
}

#[derive(Debug, Serialize)]
pub(super) struct FoundryCampaignSchedulerRejectionOutputV1 {
    category: &'static str,
    stage: &'static str,
    subkind: &'static str,
}

#[derive(Debug, Serialize)]
struct FoundryCampaignCensusOutputV1 {
    epochs_started: usize,
    plans_built: usize,
    classes_completed: usize,
    task_reports: usize,
    no_proposal: usize,
    duplicate: usize,
    incomplete_proposal: usize,
    changed_without_geometric_shrink: usize,
    strict_geometric_shrink: usize,
    compiler_closed: usize,
    invalidated_tickets: usize,
    scheduler_budget_stops: usize,
    scheduler_rejections: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_scheduler_rejection: Option<FoundryCampaignSchedulerRejectionOutputV1>,
    scheduler_stalls: usize,
    scheduler_exact_lift_errors: usize,
    canonical_replayed: usize,
    canonical_no_modular_hit: usize,
    canonical_query_rejections: usize,
    canonical_support_did_not_lift: usize,
    exact_obstructions: usize,
    declared_probes: usize,
    scheduler_replayed: usize,
    scheduler_support_did_not_lift: usize,
    scheduler_sampled_dual: usize,
}

#[derive(Debug, Serialize)]
struct FoundryCampaignSnapshotOutputV1 {
    revision: u64,
    coverage_status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    coverage_obstruction: Option<&'static str>,
    owner_count: usize,
    terminal_count: usize,
    uncovered_box_count: usize,
    uncovered_is_finite: bool,
    missing_terminal_count: usize,
    guard_incomplete_owner_count: usize,
}

#[derive(Debug, Serialize)]
struct FoundryCampaignUncoveredPartitionOutputV1 {
    total_box_count: usize,
    reported_box_count: usize,
    truncated: bool,
}

#[derive(Debug, Serialize)]
struct FoundryCampaignUncoveredBoxOutputV1 {
    lower: Vec<u64>,
    /// Decimal coordinates, with `unbounded` denoting an open upper ray.
    upper: Vec<String>,
    free_dimension: usize,
}

#[derive(Debug, Serialize)]
struct FoundryCampaignMeasurementsDocumentV1 {
    schema: &'static str,
    status: &'static str,
    semantic_report_schema: &'static str,
    clock: &'static str,
    scope: &'static str,
    duration_encoding: &'static str,
    application_elapsed_nanoseconds: String,
    core_elapsed_nanoseconds: String,
}

pub(crate) fn run_request(
    request: FoundryCampaignRunRequest,
) -> Result<FoundryCampaignRunResult, AppError> {
    run_request_with_progress(request, |_| {})
}

pub(crate) fn run_request_with_progress(
    request: FoundryCampaignRunRequest,
    observe: impl FnMut(FoundryCampaignProgress),
) -> Result<FoundryCampaignRunResult, AppError> {
    let application_started = Instant::now();
    let config = parse_config(
        &request.config,
        FoundryCampaignItinerary::SingleSectorFixedPoint,
    )?;
    require_itinerary(
        &config,
        FoundryCampaignItinerary::SingleSectorFixedPoint,
        "foundry_campaign_run",
    )?;
    let core_started = Instant::now();
    let run = run_foundry_campaign_with_progress(&config, observe).map_err(map_core_error)?;
    let core_elapsed = core_started.elapsed();
    let core_report = run.report();
    let report = render_report(&config, core_report)?;
    let application_elapsed = application_started.elapsed();
    let measurements =
        render_measurements(application_elapsed.as_nanos(), core_elapsed.as_nanos())?;
    Ok(FoundryCampaignRunResult::new(
        report,
        measurements,
        core_report.stop(),
        core_report.census(),
        core_report.snapshot().clone(),
        core_report.sector_active().len(),
        config.max_task_reports(),
    ))
}

pub(super) fn parse_config(
    source: &str,
    expected_itinerary: FoundryCampaignItinerary,
) -> Result<FoundryCampaignConfig, AppError> {
    let document: FoundryCampaignConfigDocumentV2 = toml::from_str(source).map_err(|error| {
        AppError::input(format!(
            "invalid RustRed foundry campaign configuration TOML: {error}"
        ))
    })?;
    if document.schema != FOUNDRY_CAMPAIGN_CONFIG_SCHEMA {
        return Err(AppError::schema(format!(
            "unsupported foundry campaign configuration schema {:?}; expected {:?}",
            document.schema, FOUNDRY_CAMPAIGN_CONFIG_SCHEMA
        )));
    }
    let preset = FoundryCampaignPreset::from_stable_id(&document.preset).ok_or_else(|| {
        AppError::input(format!(
            "unknown foundry campaign preset {:?}; expected {:?}",
            document.preset,
            FoundryCampaignPreset::THREE_LOOP_UNIT_MASS_VACUUM_K6_ORBIT_0_ID
        ))
    })?;
    let max_task_reports = checked_usize("max_task_reports", document.max_task_reports)?;
    let max_reported_uncovered_boxes = checked_usize(
        "max_reported_uncovered_boxes",
        document.max_reported_uncovered_boxes,
    )?;
    if document.mode == FoundrySearchProvenance::AUTONOMOUS_ID {
        if document.hints.is_some() {
            return Err(AppError::input(
                "autonomous foundry campaigns cannot contain an external hints object",
            ));
        }
        return match expected_itinerary {
            FoundryCampaignItinerary::SingleSectorFixedPoint => {
                FoundryCampaignConfig::try_autonomous_single_sector(
                    preset,
                    max_task_reports,
                    max_reported_uncovered_boxes,
                )
            }
            FoundryCampaignItinerary::FullRankAtomicWaves => {
                FoundryCampaignConfig::try_autonomous_full_rank_waves(
                    preset,
                    max_task_reports,
                    max_reported_uncovered_boxes,
                )
            }
        }
        .map_err(|error| AppError::input(format!("invalid autonomous foundry campaign: {error}")));
    }
    if document.mode != FoundrySearchProvenance::EXTERNAL_HINTS_ONLY_ID {
        return Err(AppError::input(format!(
            "unknown foundry campaign mode {:?}; expected {:?} or {:?}",
            document.mode,
            FoundrySearchProvenance::AUTONOMOUS_ID,
            FoundrySearchProvenance::EXTERNAL_HINTS_ONLY_ID,
        )));
    }
    let hints = document.hints.ok_or_else(|| {
        AppError::input("external-hints-only foundry campaigns require a reviewed hints object")
    })?;
    if hints.probes.len() > MAX_FOUNDRY_CAMPAIGN_PROBES {
        return Err(AppError::limit(format!(
            "foundry campaign declares {} hinted probes, exceeding the {MAX_FOUNDRY_CAMPAIGN_PROBES}-probe application limit",
            hints.probes.len()
        )));
    }
    let itinerary =
        FoundryCampaignItinerary::from_stable_id(&hints.itinerary).ok_or_else(|| {
            AppError::input(format!(
                "unknown hinted foundry campaign itinerary {:?}; expected {:?} or {:?}",
                hints.itinerary,
                FoundryCampaignItinerary::SINGLE_SECTOR_FIXED_POINT_ID,
                FoundryCampaignItinerary::FULL_RANK_ATOMIC_WAVES_ID,
            ))
        })?;
    if itinerary != expected_itinerary {
        return Err(AppError::input(format!(
            "hinted foundry campaign itinerary {:?} is incompatible with this entry point; expected {:?}",
            itinerary.stable_id(),
            expected_itinerary.stable_id(),
        )));
    }
    let ordering = OrderingPolicy::try_from_stable_id(&hints.ordering_policy).map_err(|error| {
        AppError::input(format!(
            "invalid hinted foundry campaign proof ordering {:?}: {error}",
            hints.ordering_policy
        ))
    })?;
    let discovery_coordinate_priority = match hints.discovery_coordinate_priority {
        None => None,
        Some(rank_by_slot) => Some(parse_coordinate_priority(rank_by_slot, 6)?),
    };
    let probes = hints.probes.into_iter().map(|probe| {
        FoundryCampaignProbe::new(probe.modulus, probe.base_parameters, probe.chart_offsets)
    });
    let hints = FoundryCampaignExternalHints::try_new(
        itinerary,
        probes,
        hints.interior_margin,
        checked_usize(
            "hints.polynomial_degree_ceiling",
            hints.polynomial_degree_ceiling,
        )?,
        ordering,
        discovery_coordinate_priority,
    )
    .map_err(|error| AppError::input(format!("invalid foundry campaign hints: {error}")))?;
    FoundryCampaignConfig::try_external_hints(
        preset,
        hints,
        max_task_reports,
        max_reported_uncovered_boxes,
    )
    .map_err(|error| AppError::input(format!("invalid hinted foundry campaign: {error}")))
}

fn parse_coordinate_priority(
    rank_by_slot: Vec<u64>,
    arity: usize,
) -> Result<CoordinatePriority, AppError> {
    let rank_by_slot = rank_by_slot
        .into_iter()
        .map(|rank| checked_usize("discovery_coordinate_priority", rank))
        .collect::<Result<Vec<_>, _>>()?;
    let priority =
        CoordinatePriority::try_new(arity, &rank_by_slot, CoordinatePriorityLimits::default())
            .map_err(|error| {
                AppError::input(format!(
                    "invalid foundry campaign discovery coordinate priority: {error}"
                ))
            })?;
    Ok(priority)
}

pub(super) fn require_itinerary(
    config: &FoundryCampaignConfig,
    expected: FoundryCampaignItinerary,
    entry_point: &'static str,
) -> Result<(), AppError> {
    if config.itinerary() == expected {
        Ok(())
    } else {
        Err(AppError::input(format!(
            "foundry campaign itinerary {:?} is incompatible with {entry_point}; expected {:?}",
            config.itinerary().stable_id(),
            expected.stable_id(),
        )))
    }
}

fn checked_usize(field: &'static str, value: u64) -> Result<usize, AppError> {
    usize::try_from(value).map_err(|_| {
        AppError::limit(format!(
            "foundry campaign {field} value {value} does not fit this platform"
        ))
    })
}

fn map_core_error(error: FoundryCampaignError) -> AppError {
    match error {
        FoundryCampaignError::Setup { .. } | FoundryCampaignError::Execution { .. } => {
            AppError::execution(error.to_string())
        }
        FoundryCampaignError::Invariant { .. } => AppError::internal_invariant(error.to_string()),
    }
}

fn render_report(
    config: &FoundryCampaignConfig,
    report: &FoundryCampaignReport,
) -> Result<String, AppError> {
    let closure_status = coverage_name(report.snapshot().coverage()).0;
    let output = FoundryCampaignReportDocumentV1 {
        schema: FOUNDRY_CAMPAIGN_REPORT_SCHEMA,
        status: "completed",
        publication: DIAGNOSTIC_PUBLICATION,
        artifact_published: false,
        preset: report.preset().stable_id(),
        family_fingerprint: report.family_fingerprint().to_owned(),
        context_fingerprint: report.context_fingerprint().to_owned(),
        sector_active: report.sector_active().to_vec(),
        closure_status,
        configuration: configuration_output(config),
        stop: stop_output(report.stop()),
        census: census_output(report.census()),
        snapshot: snapshot_output(report.snapshot()),
        uncovered_partition: FoundryCampaignUncoveredPartitionOutputV1 {
            total_box_count: report.total_uncovered_box_count(),
            reported_box_count: report.reported_uncovered_box_count(),
            truncated: report.uncovered_boxes_truncated(),
        },
        uncovered_boxes: report
            .uncovered_boxes()
            .iter()
            .map(uncovered_box_output)
            .collect(),
    };
    serialize_bounded(
        &output,
        "foundry campaign diagnostic report",
        MAX_OUTPUT_BYTES,
    )
}

fn configuration_output(config: &FoundryCampaignConfig) -> FoundryCampaignConfigurationOutputV1 {
    FoundryCampaignConfigurationOutputV1 {
        itinerary: config.itinerary().stable_id(),
        search_provenance: config.search_provenance().stable_id(),
        interior_margin: config.interior_margin(),
        polynomial_degree_ceiling: config.polynomial_degree_ceiling(),
        ordering_policy: config.ordering().stable_id().as_str().to_owned(),
        discovery_coordinate_priority: (!config.discovery_coordinate_priority().is_natural()).then(
            || {
                config
                    .discovery_coordinate_priority()
                    .rank_by_slot()
                    .to_vec()
            },
        ),
        max_task_reports: config.max_task_reports(),
        max_reported_uncovered_boxes: config.max_reported_uncovered_boxes(),
        probes: config
            .probes()
            .iter()
            .map(|probe| FoundryCampaignProbeOutputV1 {
                modulus: probe.modulus(),
                base_parameters: probe.base_parameters().to_vec(),
                chart_offsets: probe.chart_offsets().to_vec(),
            })
            .collect(),
    }
}

fn stop_output(stop: FoundryCampaignStop) -> FoundryCampaignStopOutputV1 {
    match stop {
        FoundryCampaignStop::CompilerClosed => FoundryCampaignStopOutputV1 {
            kind: "compiler_closed",
            location: None,
            refinement: None,
            operational_limit: None,
            ledger_revision: None,
            completed_classes: None,
            completed_tasks: None,
        },
        FoundryCampaignStop::NeedsRefinement { location, reason } => FoundryCampaignStopOutputV1 {
            kind: "needs_refinement",
            location: location.map(location_output),
            refinement: Some(refinement_output(reason)),
            operational_limit: None,
            ledger_revision: None,
            completed_classes: None,
            completed_tasks: None,
        },
        FoundryCampaignStop::OperationallyBounded { location, limit } => {
            FoundryCampaignStopOutputV1 {
                kind: "operationally_bounded",
                location: location.map(location_output),
                refinement: None,
                operational_limit: Some(operational_limit_output(limit)),
                ledger_revision: None,
                completed_classes: None,
                completed_tasks: None,
            }
        }
        FoundryCampaignStop::ExhaustedAtConfig {
            ledger_revision,
            completed_classes,
            completed_tasks,
        } => FoundryCampaignStopOutputV1 {
            kind: "exhausted_at_config",
            location: None,
            refinement: None,
            operational_limit: None,
            ledger_revision: Some(ledger_revision),
            completed_classes: Some(completed_classes),
            completed_tasks: Some(completed_tasks),
        },
    }
}

fn location_output(location: FoundryCampaignTaskLocation) -> FoundryCampaignTaskLocationOutputV1 {
    FoundryCampaignTaskLocationOutputV1 {
        ledger_revision: location.ledger_revision(),
        class_ordinal: location.class_ordinal(),
        effective_dimension: location.effective_dimension(),
        parent_free_dimension: location.parent_free_dimension(),
        boundary_codimension: location.boundary_codimension(),
        task_ordinal: location.task_ordinal(),
    }
}

fn refinement_output(
    reason: FoundryCampaignNeedsRefinementReason,
) -> FoundryCampaignRefinementOutputV1 {
    let mut output = FoundryCampaignRefinementOutputV1 {
        kind: "diagnostic_exact_obstructions",
        exact_obstructions: None,
        scheduler_stalls: None,
        canonical_query_rejections: None,
        coverage_status: None,
        coverage_obstruction: None,
        uncovered_is_finite: None,
        missing_terminal_count: None,
        guard_incomplete_owner_count: None,
    };
    match reason {
        FoundryCampaignNeedsRefinementReason::IncompleteProposal { exact_obstructions } => {
            output.kind = "incomplete_proposal";
            output.exact_obstructions = Some(exact_obstructions);
        }
        FoundryCampaignNeedsRefinementReason::ProbeStalled { scheduler_stalls } => {
            output.kind = "probe_stalled";
            output.scheduler_stalls = Some(scheduler_stalls);
        }
        FoundryCampaignNeedsRefinementReason::CanonicalQueryRejected {
            canonical_query_rejections,
        } => {
            output.kind = "canonical_query_rejected";
            output.canonical_query_rejections = Some(canonical_query_rejections);
        }
        FoundryCampaignNeedsRefinementReason::DiagnosticExactObstructions { count } => {
            output.exact_obstructions = Some(count);
        }
        FoundryCampaignNeedsRefinementReason::ExactCompilerState {
            coverage,
            uncovered_is_finite,
            missing_terminal_count,
            guard_incomplete_owner_count,
        } => {
            let (status, obstruction) = coverage_name(coverage);
            output.kind = "exact_compiler_state";
            output.coverage_status = Some(status);
            output.coverage_obstruction = obstruction;
            output.uncovered_is_finite = Some(uncovered_is_finite);
            output.missing_terminal_count = Some(missing_terminal_count);
            output.guard_incomplete_owner_count = Some(guard_incomplete_owner_count);
        }
    }
    output
}

fn operational_limit_output(
    limit: FoundryCampaignOperationalLimit,
) -> FoundryCampaignOperationalLimitOutputV1 {
    let mut output = FoundryCampaignOperationalLimitOutputV1 {
        kind: "incomplete_probe_execution",
        requested: None,
        limit: None,
        scheduler_budget_stops: None,
        scheduler_rejections: None,
        scheduler_exact_lift_errors: None,
        terminal_scheduler_rejection: None,
    };
    match limit {
        FoundryCampaignOperationalLimit::Epoch { requested, limit } => {
            output.kind = "epoch";
            output.requested = Some(requested);
            output.limit = Some(limit);
        }
        FoundryCampaignOperationalLimit::Plan { requested, limit } => {
            output.kind = "plan";
            output.requested = Some(requested);
            output.limit = Some(limit);
        }
        FoundryCampaignOperationalLimit::TaskReport { requested, limit } => {
            output.kind = "task_report";
            output.requested = Some(requested);
            output.limit = Some(limit);
        }
        FoundryCampaignOperationalLimit::IncompleteProbeExecution {
            scheduler_budget_stops,
            scheduler_rejections,
            scheduler_exact_lift_errors,
            terminal_scheduler_rejection,
        } => {
            output.scheduler_budget_stops = Some(scheduler_budget_stops);
            output.scheduler_rejections = Some(scheduler_rejections);
            output.scheduler_exact_lift_errors = Some(scheduler_exact_lift_errors);
            output.terminal_scheduler_rejection =
                terminal_scheduler_rejection.map(scheduler_rejection_output);
        }
    }
    output
}

fn census_output(census: FoundryCampaignCensus) -> FoundryCampaignCensusOutputV1 {
    FoundryCampaignCensusOutputV1 {
        epochs_started: census.epochs_started(),
        plans_built: census.plans_built(),
        classes_completed: census.classes_completed(),
        task_reports: census.task_reports(),
        no_proposal: census.no_proposal(),
        duplicate: census.duplicate(),
        incomplete_proposal: census.incomplete_proposal(),
        changed_without_geometric_shrink: census.changed_without_geometric_shrink(),
        strict_geometric_shrink: census.strict_geometric_shrink(),
        compiler_closed: census.compiler_closed(),
        invalidated_tickets: census.invalidated_tickets(),
        scheduler_budget_stops: census.scheduler_budget_stops(),
        scheduler_rejections: census.scheduler_rejections(),
        first_scheduler_rejection: census
            .first_scheduler_rejection()
            .map(scheduler_rejection_output),
        scheduler_stalls: census.scheduler_stalls(),
        scheduler_exact_lift_errors: census.scheduler_exact_lift_errors(),
        canonical_replayed: census.canonical_replayed(),
        canonical_no_modular_hit: census.canonical_no_modular_hit(),
        canonical_query_rejections: census.canonical_query_rejections(),
        canonical_support_did_not_lift: census.canonical_support_did_not_lift(),
        exact_obstructions: census.exact_obstructions(),
        declared_probes: census.declared_probes(),
        scheduler_replayed: census.scheduler_replayed(),
        scheduler_support_did_not_lift: census.scheduler_support_did_not_lift(),
        scheduler_sampled_dual: census.scheduler_sampled_dual(),
    }
}

pub(super) const fn scheduler_rejection_output(
    rejection: FoundryCampaignSchedulerRejection,
) -> FoundryCampaignSchedulerRejectionOutputV1 {
    FoundryCampaignSchedulerRejectionOutputV1 {
        category: rejection.category().stable_id(),
        stage: rejection.stage().stable_id(),
        subkind: rejection.stable_subkind(),
    }
}

fn snapshot_output(snapshot: &FoundryCampaignSnapshot) -> FoundryCampaignSnapshotOutputV1 {
    let (coverage_status, coverage_obstruction) = coverage_name(snapshot.coverage());
    FoundryCampaignSnapshotOutputV1 {
        revision: snapshot.revision(),
        coverage_status,
        coverage_obstruction,
        owner_count: snapshot.owner_count(),
        terminal_count: snapshot.terminal_count(),
        uncovered_box_count: snapshot.uncovered_box_count(),
        uncovered_is_finite: snapshot.uncovered_is_finite(),
        missing_terminal_count: snapshot.missing_terminal_count(),
        guard_incomplete_owner_count: snapshot.guard_incomplete_owner_count(),
    }
}

fn coverage_name(coverage: FoundryCampaignCoverageStatus) -> (&'static str, Option<&'static str>) {
    match coverage {
        FoundryCampaignCoverageStatus::OwnerFree => ("owner_free", None),
        FoundryCampaignCoverageStatus::Closed => ("closed", None),
        FoundryCampaignCoverageStatus::Incomplete(obstruction) => (
            "incomplete",
            Some(match obstruction {
                FoundryCampaignCoverageObstruction::NonFinite => "non_finite",
                FoundryCampaignCoverageObstruction::GuardIncomplete => "guard_incomplete",
                FoundryCampaignCoverageObstruction::FiniteTerminalOwnership => {
                    "finite_terminal_ownership"
                }
            }),
        ),
    }
}

fn uncovered_box_output(
    lattice_box: &FoundryCampaignUncoveredBox,
) -> FoundryCampaignUncoveredBoxOutputV1 {
    FoundryCampaignUncoveredBoxOutputV1 {
        lower: lattice_box.lower().to_vec(),
        upper: lattice_box
            .upper()
            .iter()
            .map(|coordinate| match coordinate {
                Some(value) => value.to_string(),
                None => "unbounded".to_owned(),
            })
            .collect(),
        free_dimension: lattice_box.free_dimension(),
    }
}

fn render_measurements(
    application_elapsed_nanoseconds: u128,
    core_elapsed_nanoseconds: u128,
) -> Result<String, AppError> {
    let output = FoundryCampaignMeasurementsDocumentV1 {
        schema: FOUNDRY_CAMPAIGN_MEASUREMENTS_SCHEMA,
        status: "measured",
        semantic_report_schema: FOUNDRY_CAMPAIGN_REPORT_SCHEMA,
        clock: "std.time.Instant",
        scope: MEASUREMENT_SCOPE,
        duration_encoding: "unsigned-decimal-nanoseconds",
        application_elapsed_nanoseconds: application_elapsed_nanoseconds.to_string(),
        core_elapsed_nanoseconds: core_elapsed_nanoseconds.to_string(),
    };
    serialize_bounded(
        &output,
        "foundry campaign measurement sidecar",
        MAX_OUTPUT_BYTES,
    )
}

pub(super) fn serialize_bounded(
    value: &impl Serialize,
    label: &'static str,
    max_bytes: usize,
) -> Result<String, AppError> {
    let mut serialized = toml::to_string_pretty(value)
        .map_err(|error| AppError::serialization(format!("cannot serialize {label}: {error}")))?;
    if !serialized.ends_with('\n') {
        serialized.push('\n');
    }
    if serialized.len() > max_bytes {
        return Err(AppError::output_limit(format!(
            "{label} needs {} bytes, exceeding the {max_bytes}-byte application limit",
            serialized.len()
        )));
    }
    Ok(serialized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppErrorKind;

    const AUTONOMOUS_CONFIG: &str = r#"schema = "rustred.foundry-campaign-config.toml.v2"
preset = "three-loop-unit-mass-vacuum-k6-orbit-0"
mode = "autonomous"
max_task_reports = 1
max_reported_uncovered_boxes = 2
"#;

    const EXTERNAL_CONFIG: &str = r#"schema = "rustred.foundry-campaign-config.toml.v2"
preset = "three-loop-unit-mass-vacuum-k6-orbit-0"
mode = "external-hints-only"
max_task_reports = 1
max_reported_uncovered_boxes = 2

[hints]
itinerary = "single-sector-fixed-point"
interior_margin = 2
polynomial_degree_ceiling = 0
ordering_policy = "rustred.unshifted-sector-order.v1"

[[hints.probes]]
modulus = 1000000007
base_parameters = [37]
chart_offsets = [0, 0, 0, 0, 0, 0]
"#;

    #[test]
    fn strict_v2_parser_owns_the_version_and_autonomous_program() {
        let config = parse_config(
            AUTONOMOUS_CONFIG,
            FoundryCampaignItinerary::SingleSectorFixedPoint,
        )
        .unwrap();
        assert_eq!(config.schema(), FOUNDRY_CAMPAIGN_CONFIG_SCHEMA);
        assert_eq!(config.probes().len(), 1);
        assert_eq!(config.max_task_reports(), 1);
        assert_eq!(config.max_reported_uncovered_boxes(), 2);
        assert_eq!(
            config.search_provenance(),
            FoundrySearchProvenance::Autonomous
        );

        let wrong_schema = AUTONOMOUS_CONFIG.replace(".v2", ".v1");
        assert_eq!(
            parse_config(
                &wrong_schema,
                FoundryCampaignItinerary::SingleSectorFixedPoint
            )
            .unwrap_err()
            .kind(),
            AppErrorKind::Schema
        );
        let unknown = format!("{AUTONOMOUS_CONFIG}\nlegacy_field = true\n");
        assert_eq!(
            parse_config(&unknown, FoundryCampaignItinerary::SingleSectorFixedPoint)
                .unwrap_err()
                .kind(),
            AppErrorKind::Input
        );
    }

    #[test]
    fn claim_bearing_modes_have_disjoint_typed_inputs() {
        let relabeled =
            EXTERNAL_CONFIG.replace("mode = \"external-hints-only\"", "mode = \"autonomous\"");
        assert_eq!(
            parse_config(&relabeled, FoundryCampaignItinerary::SingleSectorFixedPoint)
                .unwrap_err()
                .kind(),
            AppErrorKind::Input
        );

        let missing_hints =
            AUTONOMOUS_CONFIG.replace("mode = \"autonomous\"", "mode = \"external-hints-only\"");
        assert_eq!(
            parse_config(
                &missing_hints,
                FoundryCampaignItinerary::SingleSectorFixedPoint
            )
            .unwrap_err()
            .kind(),
            AppErrorKind::Input
        );

        let flat_search_knob = AUTONOMOUS_CONFIG.replace(
            "max_task_reports = 1\n",
            "max_task_reports = 1\nordering_policy = \"rustred.unshifted-sector-order.v1\"\n",
        );
        assert_eq!(
            parse_config(
                &flat_search_knob,
                FoundryCampaignItinerary::SingleSectorFixedPoint
            )
            .unwrap_err()
            .kind(),
            AppErrorKind::Input,
            "autonomous callers must not smuggle in a proof ordering"
        );

        for forbidden_key in [
            "rhs",
            "coefficient",
            "support",
            "source_rows",
            "rule",
            "form_line",
            "topology",
        ] {
            let forbidden_payload = EXTERNAL_CONFIG.replace(
                "ordering_policy = \"rustred.unshifted-sector-order.v1\"\n",
                &format!(
                    "ordering_policy = \"rustred.unshifted-sector-order.v1\"\n{forbidden_key} = \"forbidden\"\n"
                ),
            );
            assert_eq!(
                parse_config(
                    &forbidden_payload,
                    FoundryCampaignItinerary::SingleSectorFixedPoint
                )
                .unwrap_err()
                .kind(),
                AppErrorKind::Input,
                "external hints must reject the {forbidden_key} payload"
            );
        }
    }

    #[test]
    fn autonomous_and_external_reports_derive_their_provenance() {
        let autonomous = run_request(FoundryCampaignRunRequest::new(AUTONOMOUS_CONFIG))
            .unwrap()
            .to_toml()
            .to_owned();
        assert!(autonomous.contains("search_provenance = \"autonomous\""));
        assert!(!autonomous.contains("discovery_coordinate_priority"));

        let external = run_request(FoundryCampaignRunRequest::new(EXTERNAL_CONFIG))
            .unwrap()
            .to_toml()
            .to_owned();
        assert!(external.contains("search_provenance = \"external-hints-only\""));
        assert!(!external.contains("rhs"));
        assert!(!external.contains("recurrence"));

        let nonnatural = EXTERNAL_CONFIG.replace(
            "ordering_policy = \"rustred.unshifted-sector-order.v1\"\n",
            "ordering_policy = \"rustred.unshifted-sector-order.v1\"\n\
             discovery_coordinate_priority = [5, 3, 4, 2, 0, 1]\n",
        );
        let report = run_request(FoundryCampaignRunRequest::new(nonnatural))
            .unwrap()
            .into_toml();
        assert!(report.contains("discovery_coordinate_priority = ["));
    }

    #[test]
    fn external_itinerary_is_checked_against_the_entry_point() {
        let wrong_entry_point = EXTERNAL_CONFIG.replace(
            "itinerary = \"single-sector-fixed-point\"",
            "itinerary = \"full-rank-atomic-waves\"",
        );
        assert_eq!(
            run_request(FoundryCampaignRunRequest::new(wrong_entry_point))
                .unwrap_err()
                .kind(),
            AppErrorKind::Input
        );

        let unknown_itinerary = EXTERNAL_CONFIG.replace(
            "itinerary = \"single-sector-fixed-point\"",
            "itinerary = \"greedy-oracle-walk\"",
        );
        assert_eq!(
            parse_config(
                &unknown_itinerary,
                FoundryCampaignItinerary::SingleSectorFixedPoint
            )
            .unwrap_err()
            .kind(),
            AppErrorKind::Input
        );
    }

    #[test]
    fn persisted_external_proof_ordering_round_trips_and_rejects_wrong_arity() {
        let winner_id = "rustred.unshifted-sector-order.v1;priority=rustred.coordinate-priority.v1;k=6;rank-by-slot=5,3,4,2,0,1";
        let winner = EXTERNAL_CONFIG.replace(
            "ordering_policy = \"rustred.unshifted-sector-order.v1\"",
            &format!("ordering_policy = {winner_id:?}"),
        );
        let parsed =
            parse_config(&winner, FoundryCampaignItinerary::SingleSectorFixedPoint).unwrap();
        assert_eq!(parsed.ordering().stable_id().as_str(), winner_id);
        assert_eq!(
            parsed.discovery_coordinate_priority().rank_by_slot(),
            [5, 3, 4, 2, 0, 1]
        );
        let report = run_request(FoundryCampaignRunRequest::new(winner))
            .unwrap()
            .into_toml();
        assert!(report.contains(&format!("ordering_policy = {winner_id:?}")));
        assert!(report.contains("discovery_coordinate_priority = ["));

        let wrong_arity = EXTERNAL_CONFIG.replace(
            "ordering_policy = \"rustred.unshifted-sector-order.v1\"",
            "ordering_policy = \"rustred.unshifted-sector-order.v1;priority=rustred.coordinate-priority.v1;k=5;rank-by-slot=0,1,2,3,4\"",
        );
        assert_eq!(
            parse_config(
                &wrong_arity,
                FoundryCampaignItinerary::SingleSectorFixedPoint
            )
            .unwrap_err()
            .kind(),
            AppErrorKind::Input
        );

        let malformed_discovery = EXTERNAL_CONFIG.replace(
            "ordering_policy = \"rustred.unshifted-sector-order.v1\"\n",
            "ordering_policy = \"rustred.unshifted-sector-order.v1\"\ndiscovery_coordinate_priority = [0, 1]\n",
        );
        assert_eq!(
            parse_config(
                &malformed_discovery,
                FoundryCampaignItinerary::SingleSectorFixedPoint
            )
            .unwrap_err()
            .kind(),
            AppErrorKind::Input
        );
    }

    #[test]
    fn core_failures_map_without_message_parsing() {
        let execution = map_core_error(FoundryCampaignError::Execution {
            message: "bounded failure".to_owned(),
        });
        assert_eq!(execution.kind(), AppErrorKind::Execution);
        let invariant = map_core_error(FoundryCampaignError::Invariant {
            detail: "broken invariant",
        });
        assert_eq!(invariant.kind(), AppErrorKind::InternalInvariant);
    }
}
