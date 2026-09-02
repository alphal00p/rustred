//! Stable application boundary for the proof-retaining K6 wave driver.

use std::time::Instant;

use rustred::foundry::campaign::{
    FoundryCampaignItinerary, K6OrbitCampaignProgress, K6WaveCampaignErrorKind,
    K6WaveCampaignOutcome, K6WaveCampaignProgress, run_k6_full_rank_wave_campaign,
};
use serde::Serialize;

use super::super::{
    FOUNDRY_WAVE_CAMPAIGN_MEASUREMENTS_SCHEMA, FOUNDRY_WAVE_CAMPAIGN_REPORT_SCHEMA,
    FoundryWaveCampaignRunRequest, FoundryWaveCampaignRunResult, MAX_OUTPUT_BYTES,
};
use super::foundry::{
    FoundryCampaignSchedulerRejectionOutputV1, parse_config, require_itinerary,
    scheduler_rejection_output, serialize_bounded,
};
use crate::application::error::AppError;

const DIAGNOSTIC_PUBLICATION: &str = "diagnostic_only";
const MEASUREMENT_SCOPE: &str = "config_parse_full_rank_wave_run_and_report_serialization";

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
    sibling_worker_count: usize,
    outcome: &'static str,
    published_wave_count: usize,
    published_orbit_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocking_wave_ordinal: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocking_active_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    closed_sector_count_in_blocking_wave: Option<usize>,
    waves: Vec<WaveProgressV1>,
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
    let outcome =
        run_k6_full_rank_wave_campaign(&config, request.sibling_worker_count).map_err(|error| {
            match error.kind() {
                K6WaveCampaignErrorKind::Invariant => {
                    AppError::internal_invariant(error.to_string())
                }
                K6WaveCampaignErrorKind::Campaign
                | K6WaveCampaignErrorKind::ParallelExecution
                | K6WaveCampaignErrorKind::ProgressAggregation
                | K6WaveCampaignErrorKind::LedgerSeal
                | K6WaveCampaignErrorKind::WavePublication
                | K6WaveCampaignErrorKind::AllocationFailure => {
                    AppError::execution(error.to_string())
                }
            }
        })?;
    let report = render_report(&config, request.sibling_worker_count, &outcome)?;
    if let K6WaveCampaignOutcome::Published(published) = outcome {
        // Installation consumes only the exact published wave chain. Search
        // diagnostics and hint provenance never enter the artifact payload.
        let artifact = published
            .into_closed_artifact()
            .map_err(|error| AppError::execution(error.to_string()))?;
        drop(artifact);
    }
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
    Ok(FoundryWaveCampaignRunResult::new(report, measurements))
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
        publication: DIAGNOSTIC_PUBLICATION,
        artifact_installed: outcome_name.artifact_installed(),
        // Durable K6 encoding remains a separate typed unsupported boundary.
        durable_artifact_published: false,
        preset: config.preset().stable_id(),
        itinerary: config.itinerary().stable_id(),
        search_provenance: config.search_provenance().stable_id(),
        ordering_policy: config.ordering().stable_id().as_str().to_owned(),
        sibling_worker_count,
        outcome: outcome_name.stable_id(),
        published_wave_count,
        published_orbit_count,
        blocking_wave_ordinal,
        blocking_active_count,
        closed_sector_count_in_blocking_wave,
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
    fn bounded_full_wave_report_is_truthfully_incomplete() {
        let result = run_request(FoundryWaveCampaignRunRequest::new(WAVE_CONFIG, 1)).unwrap();
        let report = result.to_toml();
        assert!(report.contains("outcome = \"incomplete\""));
        assert!(report.contains("artifact_installed = false"));
        assert!(report.contains("durable_artifact_published = false"));
        assert!(report.contains("itinerary = \"full-rank-atomic-waves\""));
        assert!(report.contains("search_provenance = \"autonomous\""));
        assert!(report.contains("state = \"operationally-bounded\""));
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
