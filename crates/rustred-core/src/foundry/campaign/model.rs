use crate::sector::OrderingPolicy;

use super::{
    FOUNDRY_CAMPAIGN_REPORT_SCHEMA, FoundryCampaignPreset, FoundryCampaignSchedulerRejection,
};

/// Exact compiler obstruction retained as detached scalar state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundryCampaignCoverageObstruction {
    NonFinite,
    GuardIncomplete,
    FiniteTerminalOwnership,
}

/// Exact compiler state at the end of the bounded run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundryCampaignCoverageStatus {
    OwnerFree,
    Closed,
    Incomplete(FoundryCampaignCoverageObstruction),
}

/// Detached final exact-ledger census.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundryCampaignSnapshot {
    revision: u64,
    coverage: FoundryCampaignCoverageStatus,
    owner_count: usize,
    terminal_count: usize,
    uncovered_box_count: usize,
    uncovered_is_finite: bool,
    missing_terminal_count: usize,
    guard_incomplete_owner_count: usize,
}

impl FoundryCampaignSnapshot {
    pub(crate) const fn new(
        revision: u64,
        coverage: FoundryCampaignCoverageStatus,
        owner_count: usize,
        terminal_count: usize,
        uncovered_box_count: usize,
        uncovered_is_finite: bool,
        missing_terminal_count: usize,
        guard_incomplete_owner_count: usize,
    ) -> Self {
        Self {
            revision,
            coverage,
            owner_count,
            terminal_count,
            uncovered_box_count,
            uncovered_is_finite,
            missing_terminal_count,
            guard_incomplete_owner_count,
        }
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }
    pub const fn coverage(&self) -> FoundryCampaignCoverageStatus {
        self.coverage
    }
    pub const fn owner_count(&self) -> usize {
        self.owner_count
    }
    pub const fn terminal_count(&self) -> usize {
        self.terminal_count
    }
    pub const fn uncovered_box_count(&self) -> usize {
        self.uncovered_box_count
    }
    pub const fn uncovered_is_finite(&self) -> bool {
        self.uncovered_is_finite
    }
    pub const fn missing_terminal_count(&self) -> usize {
        self.missing_terminal_count
    }
    pub const fn guard_incomplete_owner_count(&self) -> usize {
        self.guard_incomplete_owner_count
    }
}

/// Allocation-free progress emitted after one exact owner-set mutation.
///
/// Every field is detached scalar telemetry. It exposes the exact compiler
/// census and task coordinates at that committed ledger revision, but carries
/// no live ledger nonce, owner payload, or publication authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundryCampaignProgress {
    snapshot: FoundryCampaignSnapshot,
    census: FoundryCampaignCensus,
    location: Option<FoundryCampaignTaskLocation>,
    maximum_dimension: usize,
    task_report_ceiling: usize,
}

impl FoundryCampaignProgress {
    pub(crate) const fn new(
        snapshot: FoundryCampaignSnapshot,
        census: FoundryCampaignCensus,
        location: Option<FoundryCampaignTaskLocation>,
        maximum_dimension: usize,
        task_report_ceiling: usize,
    ) -> Self {
        Self {
            snapshot,
            census,
            location,
            maximum_dimension,
            task_report_ceiling,
        }
    }

    pub const fn revision(&self) -> u64 {
        self.snapshot.revision()
    }

    pub const fn snapshot(&self) -> &FoundryCampaignSnapshot {
        &self.snapshot
    }

    pub const fn census(&self) -> FoundryCampaignCensus {
        self.census
    }

    /// Canonical task that committed this owner, when the coordinator stop
    /// retains one. A closing mutation may deliberately report no location.
    pub const fn location(&self) -> Option<FoundryCampaignTaskLocation> {
        self.location
    }

    /// Maximum chart dimension of the exact campaign ledger.
    pub const fn maximum_dimension(&self) -> usize {
        self.maximum_dimension
    }

    /// Configured operational ceiling for cumulative task reports.
    ///
    /// This is useful for reporting progress toward a bounded run. It is not
    /// an estimate of mathematical closure.
    pub const fn task_report_ceiling(&self) -> usize {
        self.task_report_ceiling
    }
}

/// One exact disjoint box in the final uncovered chart partition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundryCampaignUncoveredBox {
    lower: Box<[u64]>,
    upper: Box<[Option<u64>]>,
    free_dimension: usize,
}

impl FoundryCampaignUncoveredBox {
    pub(crate) fn new(
        lower: impl Into<Box<[u64]>>,
        upper: impl Into<Box<[Option<u64>]>>,
        free_dimension: usize,
    ) -> Self {
        Self {
            lower: lower.into(),
            upper: upper.into(),
            free_dimension,
        }
    }

    pub fn lower(&self) -> &[u64] {
        &self.lower
    }
    pub fn upper(&self) -> &[Option<u64>] {
        &self.upper
    }
    pub const fn free_dimension(&self) -> usize {
        self.free_dimension
    }
}

/// Semantic origin of one detached campaign task location.
///
/// A requested-domain ordinal is proposal chronology only; it never denotes
/// a boundary service class and carries no exhaustion or closure authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundryCampaignTaskLocationKind {
    BoundarySimplex,
    RequestedDomain { requested_ordinal: usize },
}

impl FoundryCampaignTaskLocationKind {
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::BoundarySimplex => "boundary_simplex",
            Self::RequestedDomain { .. } => "requested_domain",
        }
    }
}

/// Canonical task location for a bounded or refinement stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FoundryCampaignTaskLocation {
    kind: FoundryCampaignTaskLocationKind,
    ledger_revision: u64,
    class_ordinal: usize,
    effective_dimension: usize,
    parent_free_dimension: usize,
    boundary_codimension: usize,
    task_ordinal: usize,
}

impl FoundryCampaignTaskLocation {
    pub(crate) const fn new(
        kind: FoundryCampaignTaskLocationKind,
        ledger_revision: u64,
        class_ordinal: usize,
        effective_dimension: usize,
        parent_free_dimension: usize,
        boundary_codimension: usize,
        task_ordinal: usize,
    ) -> Self {
        Self {
            kind,
            ledger_revision,
            class_ordinal,
            effective_dimension,
            parent_free_dimension,
            boundary_codimension,
            task_ordinal,
        }
    }

    pub const fn kind(self) -> FoundryCampaignTaskLocationKind {
        self.kind
    }
    pub const fn ledger_revision(self) -> u64 {
        self.ledger_revision
    }
    pub const fn class_ordinal(self) -> usize {
        self.class_ordinal
    }
    pub const fn effective_dimension(self) -> usize {
        self.effective_dimension
    }
    pub const fn parent_free_dimension(self) -> usize {
        self.parent_free_dimension
    }
    pub const fn boundary_codimension(self) -> usize {
        self.boundary_codimension
    }
    pub const fn task_ordinal(self) -> usize {
        self.task_ordinal
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundryCampaignOperationalLimit {
    Epoch {
        requested: usize,
        limit: usize,
    },
    Plan {
        requested: usize,
        limit: usize,
    },
    TaskReport {
        requested: usize,
        limit: usize,
    },
    IncompleteProbeExecution {
        scheduler_budget_stops: usize,
        scheduler_rejections: usize,
        scheduler_exact_lift_errors: usize,
        terminal_scheduler_rejection: Option<FoundryCampaignSchedulerRejection>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundryCampaignNeedsRefinementReason {
    IncompleteProposal {
        exact_obstructions: usize,
    },
    ProbeStalled {
        scheduler_stalls: usize,
    },
    CanonicalQueryRejected {
        canonical_query_rejections: usize,
    },
    DiagnosticExactObstructions {
        count: usize,
    },
    ExactCompilerState {
        coverage: FoundryCampaignCoverageStatus,
        uncovered_is_finite: bool,
        missing_terminal_count: usize,
        guard_incomplete_owner_count: usize,
    },
}

/// Why the bounded deterministic driver returned control to its caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundryCampaignStop {
    CompilerClosed,
    NeedsRefinement {
        location: Option<FoundryCampaignTaskLocation>,
        reason: FoundryCampaignNeedsRefinementReason,
    },
    OperationallyBounded {
        location: Option<FoundryCampaignTaskLocation>,
        limit: FoundryCampaignOperationalLimit,
    },
    ExhaustedAtConfig {
        ledger_revision: u64,
        completed_classes: usize,
        completed_tasks: usize,
    },
}

/// Allocation-free cumulative scheduler and replay census.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FoundryCampaignCensus {
    pub(crate) epochs_started: usize,
    pub(crate) plans_built: usize,
    pub(crate) classes_completed: usize,
    pub(crate) task_reports: usize,
    pub(crate) no_proposal: usize,
    pub(crate) duplicate: usize,
    pub(crate) incomplete_proposal: usize,
    pub(crate) changed_without_geometric_shrink: usize,
    pub(crate) strict_geometric_shrink: usize,
    pub(crate) compiler_closed: usize,
    pub(crate) invalidated_tickets: usize,
    pub(crate) scheduler_budget_stops: usize,
    pub(crate) scheduler_rejections: usize,
    pub(crate) first_scheduler_rejection: Option<FoundryCampaignSchedulerRejection>,
    pub(crate) scheduler_stalls: usize,
    pub(crate) scheduler_exact_lift_errors: usize,
    pub(crate) canonical_replayed: usize,
    pub(crate) canonical_no_modular_hit: usize,
    pub(crate) canonical_query_rejections: usize,
    pub(crate) canonical_support_did_not_lift: usize,
    pub(crate) exact_obstructions: usize,
    pub(crate) declared_probes: usize,
    pub(crate) scheduler_replayed: usize,
    pub(crate) scheduler_support_did_not_lift: usize,
    pub(crate) scheduler_sampled_dual: usize,
}

macro_rules! census_accessors {
    ($($name:ident),* $(,)?) => {$(
        pub const fn $name(self) -> usize { self.$name }
    )*};
}

impl FoundryCampaignCensus {
    census_accessors!(
        epochs_started,
        plans_built,
        classes_completed,
        task_reports,
        no_proposal,
        duplicate,
        incomplete_proposal,
        changed_without_geometric_shrink,
        strict_geometric_shrink,
        compiler_closed,
        invalidated_tickets,
        scheduler_budget_stops,
        scheduler_rejections,
        scheduler_stalls,
        scheduler_exact_lift_errors,
        canonical_replayed,
        canonical_no_modular_hit,
        canonical_query_rejections,
        canonical_support_did_not_lift,
        exact_obstructions,
        declared_probes,
        scheduler_replayed,
        scheduler_support_did_not_lift,
        scheduler_sampled_dual,
    );

    /// First rejection encountered in deterministic campaign chronology.
    /// No physical probe or epoch ordinal is retained.
    pub const fn first_scheduler_rejection(self) -> Option<FoundryCampaignSchedulerRejection> {
        self.first_scheduler_rejection
    }
}

/// Deterministic, detached report. Durations and host measurements are
/// deliberately absent from this semantic payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundryCampaignReport {
    preset: FoundryCampaignPreset,
    ordering: OrderingPolicy,
    family_fingerprint: String,
    context_fingerprint: String,
    sector_active: Box<[bool]>,
    stop: FoundryCampaignStop,
    census: FoundryCampaignCensus,
    snapshot: FoundryCampaignSnapshot,
    uncovered_boxes: Box<[FoundryCampaignUncoveredBox]>,
}

impl FoundryCampaignReport {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        preset: FoundryCampaignPreset,
        ordering: OrderingPolicy,
        family_fingerprint: String,
        context_fingerprint: String,
        sector_active: Box<[bool]>,
        stop: FoundryCampaignStop,
        census: FoundryCampaignCensus,
        snapshot: FoundryCampaignSnapshot,
        uncovered_boxes: Box<[FoundryCampaignUncoveredBox]>,
    ) -> Self {
        Self {
            preset,
            ordering,
            family_fingerprint,
            context_fingerprint,
            sector_active,
            stop,
            census,
            snapshot,
            uncovered_boxes,
        }
    }

    pub const fn schema(&self) -> &'static str {
        FOUNDRY_CAMPAIGN_REPORT_SCHEMA
    }
    pub const fn preset(&self) -> FoundryCampaignPreset {
        self.preset
    }
    pub const fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }
    pub fn sector_active(&self) -> &[bool] {
        &self.sector_active
    }
    pub const fn stop(&self) -> FoundryCampaignStop {
        self.stop
    }
    pub const fn census(&self) -> FoundryCampaignCensus {
        self.census
    }
    pub const fn snapshot(&self) -> &FoundryCampaignSnapshot {
        &self.snapshot
    }
    pub const fn total_uncovered_box_count(&self) -> usize {
        self.snapshot.uncovered_box_count()
    }
    pub fn reported_uncovered_box_count(&self) -> usize {
        self.uncovered_boxes.len()
    }
    pub fn uncovered_boxes_truncated(&self) -> bool {
        self.uncovered_boxes.len() < self.snapshot.uncovered_box_count()
    }
    pub fn uncovered_boxes(&self) -> &[FoundryCampaignUncoveredBox] {
        &self.uncovered_boxes
    }
}

/// Result owner reserved for future nonsemantic measurement sidecars. The
/// deterministic report itself remains independently comparable and stable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundryCampaignRun {
    report: FoundryCampaignReport,
}

impl FoundryCampaignRun {
    pub(crate) const fn new(report: FoundryCampaignReport) -> Self {
        Self { report }
    }

    pub const fn report(&self) -> &FoundryCampaignReport {
        &self.report
    }

    pub fn into_report(self) -> FoundryCampaignReport {
        self.report
    }
}
