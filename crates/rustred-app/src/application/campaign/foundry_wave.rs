//! Stable application boundary for the proof-retaining K6 wave driver.

use std::time::Instant;

use rustred::foundry::artifact::ClosedArtifact;
use rustred::foundry::campaign::{
    FoundryCampaignItinerary, K6OrbitCampaignProgress, K6WaveCampaignErrorKind,
    K6WaveCampaignOutcome, K6WaveCampaignProgress, K6WaveCampaignRunError,
    run_k6_full_rank_wave_campaign_with_progress,
};
use serde::Serialize;

use super::super::{
    FOUNDRY_WAVE_CAMPAIGN_MEASUREMENTS_SCHEMA, FOUNDRY_WAVE_CAMPAIGN_REPORT_SCHEMA,
    FoundryWaveCampaignRunRequest, FoundryWaveCampaignRunResult, MAX_OUTPUT_BYTES,
};
use super::foundry::{
    FoundryAutonomousSelectionOutputV1, FoundryCampaignResidualOutputV1,
    FoundryCampaignSchedulerRejectionOutputV1, autonomous_selection_output, map_core_error,
    parse_config, require_itinerary, residual_output, scheduler_rejection_output,
    serialize_bounded,
};
use crate::application::error::AppError;

const DIAGNOSTIC_PUBLICATION: &str = "diagnostic_only";
const DURABLE_PUBLICATION: &str = "canonical_closing_artifact";
const MEASUREMENT_SCOPE: &str = "application=config-parse-through-validated-result;core=search-resolution-through-validated-result;validated-result=report-serialization-plus-artifact-install-encode-cold-replay";

#[derive(Debug, Serialize)]
struct WaveCampaignReportV1 {
    schema: &'static str,
    status: &'static str,
    publication: &'static str,
    artifact_installed: bool,
    durable_artifact_published: bool,
    preset: &'static str,
    itinerary: &'static str,
    search_provenance: &'static str,
    ordering_policy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    discovery_coordinate_priority: Option<Vec<usize>>,
    probes: Vec<WaveCampaignProbeV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    autonomous_selection: Option<FoundryAutonomousSelectionOutputV1>,
    sibling_worker_count: usize,
    max_task_reports: usize,
    max_reported_uncovered_boxes: usize,
    outcome: &'static str,
    published_wave_count: usize,
    published_orbit_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocking_wave_ordinal: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocking_active_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    closed_sector_count_in_blocking_wave: Option<usize>,
    /// Exact detached stop and residual geometry for every sibling which
    /// prevented atomic publication of the blocking wave.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    incomplete_orbits: Vec<IncompleteOrbitResidualV1>,
    waves: Vec<WaveProgressV1>,
}

#[derive(Debug, Serialize)]
struct IncompleteOrbitResidualV1 {
    orbit_ordinal: usize,
    residual: FoundryCampaignResidualOutputV1,
}

#[derive(Debug, Serialize)]
struct WaveCampaignProbeV1 {
    modulus: u64,
    base_parameters: Vec<i64>,
    chart_offsets: Vec<u64>,
}

#[derive(Debug, Serialize)]
struct WaveProgressV1 {
    wave_ordinal: usize,
    active_count: usize,
    state: &'static str,
    closed_orbit_count: usize,
    orbits: Vec<OrbitProgressV1>,
}

#[derive(Debug, Serialize)]
struct OrbitProgressV1 {
    orbit_ordinal: usize,
    representative: Vec<i64>,
    active_count: usize,
    state: &'static str,
    ledger_revision: u64,
    owner_count: usize,
    uncovered_box_count: usize,
    task_reports: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_scheduler_rejection: Option<FoundryCampaignSchedulerRejectionOutputV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    terminal_scheduler_rejection: Option<FoundryCampaignSchedulerRejectionOutputV1>,
}

#[derive(Debug, Serialize)]
struct WaveCampaignMeasurementsV1 {
    schema: &'static str,
    status: &'static str,
    semantic_report_schema: &'static str,
    clock: &'static str,
    scope: &'static str,
    duration_encoding: &'static str,
    application_elapsed_nanoseconds: String,
    core_elapsed_nanoseconds: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WaveReportOutcome {
    Incomplete,
    FullRankWavesPublished,
}

impl WaveReportOutcome {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Incomplete => "incomplete",
            Self::FullRankWavesPublished => "full-rank-waves-published",
        }
    }

    const fn artifact_installed(self) -> bool {
        matches!(self, Self::FullRankWavesPublished)
    }
}

pub(crate) fn run_request(
    request: FoundryWaveCampaignRunRequest,
) -> Result<FoundryWaveCampaignRunResult, AppError> {
    run_request_with_progress(request, |_| {})
}

pub(crate) fn run_request_with_progress(
    request: FoundryWaveCampaignRunRequest,
    observe: impl FnMut(K6WaveCampaignProgress),
) -> Result<FoundryWaveCampaignRunResult, AppError> {
    let application_started = Instant::now();
    let config = parse_config(
        &request.config,
        FoundryCampaignItinerary::FullRankAtomicWaves,
    )?;
    require_itinerary(
        &config,
        FoundryCampaignItinerary::FullRankAtomicWaves,
        "foundry_wave_campaign_run",
    )?;
    let core_started = Instant::now();
    let config = config
        .try_resolve_search_program()
        .map_err(map_core_error)?;
    let outcome = run_k6_full_rank_wave_campaign_with_progress(
        &config,
        request.sibling_worker_count,
        observe,
    )
    .map_err(map_wave_core_error)?;
    let report = render_report(&config, request.sibling_worker_count, &outcome)?;
    let artifact = if let K6WaveCampaignOutcome::Published(published) = outcome {
        // Installation consumes only the exact published wave chain. Search
        // diagnostics and hint provenance never enter the artifact payload.
        let artifact = published
            .into_closed_artifact()
            .map_err(|error| AppError::execution(error.to_string()))?;
        let durable = artifact.encode_durable().map_err(|error| {
            AppError::execution(format!("cannot encode installed K6 artifact: {error}"))
        })?;
        let cold = ClosedArtifact::decode_durable(&durable).map_err(|error| {
            AppError::execution(format!("cannot cold-reload installed K6 artifact: {error}"))
        })?;
        let canonical = cold.encode_durable().map_err(|error| {
            AppError::execution(format!("cannot re-encode cold-loaded K6 artifact: {error}"))
        })?;
        if canonical != durable {
            return Err(AppError::internal_invariant(
                "cold-loaded K6 artifact did not preserve canonical durable bytes",
            ));
        }
        Some(durable)
    } else {
        None
    };
    let core_elapsed = core_started.elapsed();
    let application_elapsed = application_started.elapsed();
    let measurements = serialize_bounded(
        &WaveCampaignMeasurementsV1 {
            schema: FOUNDRY_WAVE_CAMPAIGN_MEASUREMENTS_SCHEMA,
            status: "measured",
            semantic_report_schema: FOUNDRY_WAVE_CAMPAIGN_REPORT_SCHEMA,
            clock: "std.time.Instant",
            scope: MEASUREMENT_SCOPE,
            duration_encoding: "unsigned-decimal-nanoseconds",
            application_elapsed_nanoseconds: application_elapsed.as_nanos().to_string(),
            core_elapsed_nanoseconds: core_elapsed.as_nanos().to_string(),
        },
        "foundry wave campaign measurement sidecar",
        MAX_OUTPUT_BYTES,
    )?;
    Ok(FoundryWaveCampaignRunResult::new(
        report,
        measurements,
        artifact,
    ))
}

fn map_wave_core_error(error: K6WaveCampaignRunError) -> AppError {
    map_wave_core_error_kind(error.kind(), error.to_string())
}

fn map_wave_core_error_kind(kind: K6WaveCampaignErrorKind, message: String) -> AppError {
    match kind {
        K6WaveCampaignErrorKind::ResourceLimit => AppError::limit(message),
        K6WaveCampaignErrorKind::Invariant => AppError::internal_invariant(message),
        K6WaveCampaignErrorKind::Campaign
        | K6WaveCampaignErrorKind::ParallelExecution
        | K6WaveCampaignErrorKind::ProgressAggregation
        | K6WaveCampaignErrorKind::LedgerSeal
        | K6WaveCampaignErrorKind::WavePublication
        | K6WaveCampaignErrorKind::AllocationFailure => AppError::execution(message),
    }
}

fn render_report(
    config: &rustred::foundry::campaign::FoundryCampaignConfig,
    sibling_worker_count: usize,
    outcome: &K6WaveCampaignOutcome,
) -> Result<String, AppError> {
    let (
        outcome_name,
        published_wave_count,
        published_orbit_count,
        blocking_wave_ordinal,
        blocking_active_count,
        closed_sector_count_in_blocking_wave,
        progress,
    ) = match outcome {
        K6WaveCampaignOutcome::Incomplete(incomplete) => (
            WaveReportOutcome::Incomplete,
            incomplete.published_wave_count(),
            incomplete
                .progress()
                .iter()
                .flat_map(K6WaveCampaignProgress::orbits)
                .filter(|orbit| orbit.state().stable_id() == "published")
                .count(),
            Some(incomplete.wave_ordinal()),
            Some(incomplete.active_count()),
            Some(incomplete.closed_sector_count()),
            incomplete.progress(),
        ),
        K6WaveCampaignOutcome::Published(published) => (
            WaveReportOutcome::FullRankWavesPublished,
            published.published_wave_count(),
            published.published_orbit_count(),
            None,
            None,
            None,
            published.progress(),
        ),
    };
    let output = WaveCampaignReportV1 {
        schema: FOUNDRY_WAVE_CAMPAIGN_REPORT_SCHEMA,
        status: "completed",
        publication: if matches!(outcome_name, WaveReportOutcome::FullRankWavesPublished) {
            DURABLE_PUBLICATION
        } else {
            DIAGNOSTIC_PUBLICATION
        },
        artifact_installed: outcome_name.artifact_installed(),
        durable_artifact_published: matches!(
            outcome_name,
            WaveReportOutcome::FullRankWavesPublished
        ),
        preset: config.preset().stable_id(),
        itinerary: config.itinerary().stable_id(),
        search_provenance: config.search_provenance().stable_id(),
        ordering_policy: config.ordering().stable_id().as_str().to_owned(),
        discovery_coordinate_priority: (!config.discovery_coordinate_priority().is_natural()).then(
            || {
                config
                    .discovery_coordinate_priority()
                    .rank_by_slot()
                    .to_vec()
            },
        ),
        probes: config
            .probes()
            .iter()
            .map(|probe| WaveCampaignProbeV1 {
                modulus: probe.modulus(),
                base_parameters: probe.base_parameters().to_vec(),
                chart_offsets: probe.chart_offsets().to_vec(),
            })
            .collect(),
        autonomous_selection: config
            .autonomous_selection()
            .map(autonomous_selection_output),
        sibling_worker_count,
        max_task_reports: config.max_task_reports(),
        max_reported_uncovered_boxes: config.max_reported_uncovered_boxes(),
        outcome: outcome_name.stable_id(),
        published_wave_count,
        published_orbit_count,
        blocking_wave_ordinal,
        blocking_active_count,
        closed_sector_count_in_blocking_wave,
        incomplete_orbits: match outcome {
            K6WaveCampaignOutcome::Incomplete(incomplete) => incomplete
                .incomplete_orbits()
                .iter()
                .map(|orbit| IncompleteOrbitResidualV1 {
                    orbit_ordinal: orbit.orbit_ordinal(),
                    residual: residual_output(orbit.report()),
                })
                .collect(),
            K6WaveCampaignOutcome::Published(_) => Vec::new(),
        },
        waves: progress.iter().map(wave_output).collect(),
    };
    serialize_bounded(
        &output,
        "foundry wave campaign diagnostic report",
        MAX_OUTPUT_BYTES,
    )
}

fn wave_output(progress: &K6WaveCampaignProgress) -> WaveProgressV1 {
    WaveProgressV1 {
        wave_ordinal: progress.wave_ordinal(),
        active_count: progress.active_count(),
        state: progress.state().stable_id(),
        closed_orbit_count: progress.closed_orbit_count(),
        orbits: progress.orbits().iter().map(orbit_output).collect(),
    }
}

fn orbit_output(progress: &K6OrbitCampaignProgress) -> OrbitProgressV1 {
    OrbitProgressV1 {
        orbit_ordinal: progress.orbit_ordinal(),
        representative: progress.representative().to_vec(),
        active_count: progress.active_count(),
        state: progress.state().stable_id(),
        ledger_revision: progress.ledger_revision(),
        owner_count: progress.owner_count(),
        uncovered_box_count: progress.uncovered_box_count(),
        task_reports: progress.task_reports(),
        first_scheduler_rejection: progress
            .first_scheduler_rejection()
            .map(scheduler_rejection_output),
        terminal_scheduler_rejection: progress
            .terminal_scheduler_rejection()
            .map(scheduler_rejection_output),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppErrorKind;

    const WAVE_CONFIG: &str = r#"schema = "rustred.foundry-campaign-config.toml.v2"
preset = "three-loop-unit-mass-vacuum-k6-orbit-0"
mode = "autonomous"
max_task_reports = 1
max_reported_uncovered_boxes = 1
"#;

    const HINTED_WAVE_CONFIG: &str = r#"schema = "rustred.foundry-campaign-config.toml.v2"
preset = "three-loop-unit-mass-vacuum-k6-orbit-0"
mode = "external-hints-only"
max_task_reports = 1
max_reported_uncovered_boxes = 1

[hints]
itinerary = "full-rank-atomic-waves"
interior_margin = 2
polynomial_degree_ceiling = 0
ordering_policy = "rustred.unshifted-sector-order.v1"

[[hints.probes]]
modulus = 1000000007
base_parameters = [37]
chart_offsets = [0, 0, 0, 0, 0, 0]
"#;

    #[test]
    fn report_outcome_cannot_contradict_artifact_installation() {
        assert_eq!(WaveReportOutcome::Incomplete.stable_id(), "incomplete");
        assert!(!WaveReportOutcome::Incomplete.artifact_installed());
        assert_eq!(
            WaveReportOutcome::FullRankWavesPublished.stable_id(),
            "full-rank-waves-published"
        );
        assert!(WaveReportOutcome::FullRankWavesPublished.artifact_installed());
    }

    #[test]
    fn wave_error_kinds_keep_resource_and_invariant_application_categories() {
        assert_eq!(
            map_wave_core_error_kind(
                K6WaveCampaignErrorKind::ResourceLimit,
                "bounded resource".to_owned(),
            )
            .kind(),
            AppErrorKind::Limit,
        );
        assert_eq!(
            map_wave_core_error_kind(
                K6WaveCampaignErrorKind::Invariant,
                "broken invariant".to_owned(),
            )
            .kind(),
            AppErrorKind::InternalInvariant,
        );
    }

    #[test]
    fn bounded_full_wave_report_is_truthfully_incomplete() {
        let mut progress = Vec::new();
        let result = run_request_with_progress(
            FoundryWaveCampaignRunRequest::new(WAVE_CONFIG, 1),
            |snapshot| progress.push(snapshot),
        )
        .unwrap();
        let report = result.to_toml();
        assert!(report.contains("outcome = \"incomplete\""));
        assert!(report.contains("artifact_installed = false"));
        assert!(report.contains("durable_artifact_published = false"));
        assert!(report.contains("itinerary = \"full-rank-atomic-waves\""));
        assert!(report.contains("search_provenance = \"autonomous\""));
        assert!(report.contains("algorithm = \"rustred.autonomous-k6-selector.v1\""));
        assert!(report.contains("selected_probe_count = 6"));
        assert!(report.contains("max_task_reports = 1"));
        assert!(report.contains("max_reported_uncovered_boxes = 1"));
        assert!(report.contains("[[probes]]"));
        assert!(report.contains("state = \"operationally-bounded\""));
        assert!(report.contains("[[incomplete_orbits]]"));
        assert!(report.contains("[incomplete_orbits.residual.stop]"));
        assert!(report.contains("[incomplete_orbits.residual.uncovered_partition]"));
        assert!(report.contains("[[incomplete_orbits.residual.uncovered_boxes]]"));
        let document: toml::Value = toml::from_str(report).expect("valid wave report TOML");
        let incomplete_orbits = document["incomplete_orbits"]
            .as_array()
            .expect("incomplete orbit array");
        assert_eq!(
            incomplete_orbits.len(),
            rustred::foundry::campaign::K6_FULL_RANK_WAVE_WIDTHS[0],
            "the report must retain every sibling which blocked atomic publication",
        );
        for (expected_ordinal, orbit) in incomplete_orbits.iter().enumerate() {
            assert_eq!(
                orbit["orbit_ordinal"].as_integer(),
                Some(expected_ordinal as i64)
            );
            let residual = &orbit["residual"];
            assert_eq!(
                residual["stop"]["kind"].as_str(),
                Some("operationally_bounded")
            );
            assert_eq!(
                residual["stop"]["operational_limit"]["kind"].as_str(),
                Some("task_report")
            );
            assert_eq!(
                residual["uncovered_partition"]["reported_box_count"].as_integer(),
                Some(1),
            );
            let total = residual["uncovered_partition"]["total_box_count"]
                .as_integer()
                .expect("total residual box count");
            assert!(total >= 1);
            assert_eq!(
                residual["uncovered_partition"]["truncated"].as_bool(),
                Some(total > 1),
            );
            assert_eq!(
                residual["uncovered_boxes"]
                    .as_array()
                    .expect("reported residual boxes")
                    .len(),
                1,
            );
        }
        assert!(result.artifact_bytes().is_none());
        assert!(!progress.is_empty());
        assert_eq!(
            progress.last().map(K6WaveCampaignProgress::state),
            Some(rustred::foundry::campaign::K6WaveCampaignState::Incomplete)
        );
    }

    #[test]
    fn itinerary_entry_point_mismatch_is_input_error() {
        let single = HINTED_WAVE_CONFIG.replace(
            "itinerary = \"full-rank-atomic-waves\"\n",
            "itinerary = \"single-sector-fixed-point\"\n",
        );
        assert_eq!(
            run_request(FoundryWaveCampaignRunRequest::new(single, 1))
                .unwrap_err()
                .kind(),
            AppErrorKind::Input
        );
    }

    #[test]
    fn unknown_fields_and_itineraries_are_rejected_before_core_work() {
        let unknown_field = format!("{WAVE_CONFIG}\nlegacy_wave_mode = true\n");
        assert_eq!(
            run_request(FoundryWaveCampaignRunRequest::new(unknown_field, 1))
                .unwrap_err()
                .kind(),
            AppErrorKind::Input
        );
        let unknown_itinerary =
            HINTED_WAVE_CONFIG.replace("full-rank-atomic-waves", "form-guided-full-rank-waves");
        assert_eq!(
            run_request(FoundryWaveCampaignRunRequest::new(unknown_itinerary, 1))
                .unwrap_err()
                .kind(),
            AppErrorKind::Input
        );
    }
}
