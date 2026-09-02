use std::time::{Duration, Instant};

use rustred::foundry::campaign::{
    FoundryCampaignCensus, FoundryCampaignStop, FoundryCampaignTaskLocation,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CampaignPhase {
    Starting,
    Discovering,
    Closing,
    Closed,
    Bounded,
    Refinement,
    Exhausted,
    Failed,
}

impl CampaignPhase {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Discovering => "discovering",
            Self::Closing => "closing",
            Self::Closed => "closed",
            Self::Bounded => "bounded",
            Self::Refinement => "needs refinement",
            Self::Exhausted => "exhausted",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct DashboardState {
    pub(super) phase: CampaignPhase,
    pub(super) elapsed: Duration,
    pub(super) revision: u64,
    pub(super) owner_count: usize,
    pub(super) task_report_ceiling: Option<usize>,
    pub(super) uncovered_box_count: Option<usize>,
    pub(super) effective_dimension: Option<usize>,
    pub(super) maximum_dimension: Option<usize>,
    pub(super) strict_shrink: usize,
    pub(super) no_proposal: usize,
    pub(super) duplicate: usize,
    pub(super) errors: usize,
    pub(super) task_reports: usize,
    pub(super) owner_rate: Option<f64>,
    pub(super) cap_eta: Option<Duration>,
    pub(super) rss_bytes: Option<u64>,
}

impl Default for DashboardState {
    fn default() -> Self {
        Self {
            phase: CampaignPhase::Starting,
            elapsed: Duration::ZERO,
            revision: 0,
            owner_count: 0,
            task_report_ceiling: None,
            uncovered_box_count: None,
            effective_dimension: None,
            maximum_dimension: None,
            strict_shrink: 0,
            no_proposal: 0,
            duplicate: 0,
            errors: 0,
            task_reports: 0,
            owner_rate: None,
            cap_eta: None,
            rss_bytes: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RateSample {
    pub(super) at: Instant,
    pub(super) owners: usize,
}

pub(super) fn phase_for_stop(stop: FoundryCampaignStop) -> CampaignPhase {
    match stop {
        FoundryCampaignStop::CompilerClosed => CampaignPhase::Closed,
        FoundryCampaignStop::NeedsRefinement { .. } => CampaignPhase::Refinement,
        FoundryCampaignStop::OperationallyBounded { .. } => CampaignPhase::Bounded,
        FoundryCampaignStop::ExhaustedAtConfig { .. } => CampaignPhase::Exhausted,
    }
}

pub(super) fn stop_location(stop: FoundryCampaignStop) -> Option<FoundryCampaignTaskLocation> {
    match stop {
        FoundryCampaignStop::NeedsRefinement { location, .. }
        | FoundryCampaignStop::OperationallyBounded { location, .. } => location,
        FoundryCampaignStop::CompilerClosed | FoundryCampaignStop::ExhaustedAtConfig { .. } => None,
    }
}

pub(super) fn error_count(census: FoundryCampaignCensus) -> usize {
    // Count disjoint failed task/outcome categories. Exact obstruction and
    // support-miss tallies are diagnostic details of these outcomes and are
    // deliberately not added again here.
    [
        census.incomplete_proposal(),
        census.changed_without_geometric_shrink(),
        census.scheduler_budget_stops(),
        census.scheduler_rejections(),
        census.scheduler_stalls(),
        census.scheduler_exact_lift_errors(),
        census.canonical_query_rejections(),
    ]
    .into_iter()
    .fold(0, usize::saturating_add)
}

pub(super) fn owner_progress_is_one_to_one(owners: usize, census: FoundryCampaignCensus) -> bool {
    owners == census.task_reports()
        && census.task_reports() == census.strict_geometric_shrink()
        && census.no_proposal() == 0
        && census.duplicate() == 0
        && census.incomplete_proposal() == 0
        && census.changed_without_geometric_shrink() == 0
        && error_count(census) == 0
}

pub(super) fn defensible_cap_eta(
    owners: usize,
    task_report_ceiling: usize,
    one_to_one: bool,
    owner_rate: Option<f64>,
) -> Option<Duration> {
    let rate = owner_rate.filter(|rate| rate.is_finite() && *rate > 0.0)?;
    if !one_to_one {
        return None;
    }
    let remaining = task_report_ceiling.saturating_sub(owners);
    Duration::try_from_secs_f64(remaining as f64 / rate).ok()
}
