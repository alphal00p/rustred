use crate::foundry::completion::source_discovery::CampaignModularProbe;
use crate::foundry::completion::source_discovery::scheduler::{
    ProbeLocalIterationDisposition, ProbeLocalRunCensus, ProbeLocalStage,
};
use crate::identity::IntegralShift;

use super::super::InteriorSimplexTaskKey;

/// Exact structural counts from one complete target-unit bootstrap.
///
/// These counters describe nominated and translated support only. They are
/// not a rank result, no-relation certificate, or closure statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InteriorSimplexBootstrapTelemetry {
    raw_incidence_visits: usize,
    unique_nominated_requests: usize,
    excluded_existing_requests: usize,
    selected_sources: usize,
    physical_shift_occurrences: usize,
    distinct_physical_shifts: usize,
}

impl InteriorSimplexBootstrapTelemetry {
    pub(crate) const fn raw_incidence_visits(self) -> usize {
        self.raw_incidence_visits
    }

    pub(crate) const fn unique_nominated_requests(self) -> usize {
        self.unique_nominated_requests
    }

    pub(crate) const fn excluded_existing_requests(self) -> usize {
        self.excluded_existing_requests
    }

    pub(crate) const fn selected_sources(self) -> usize {
        self.selected_sources
    }

    pub(crate) const fn physical_shift_occurrences(self) -> usize {
        self.physical_shift_occurrences
    }

    pub(crate) const fn distinct_physical_shifts(self) -> usize {
        self.distinct_physical_shifts
    }

    pub(super) const fn new(
        raw_incidence_visits: usize,
        unique_nominated_requests: usize,
        excluded_existing_requests: usize,
        selected_sources: usize,
        physical_shift_occurrences: usize,
        distinct_physical_shifts: usize,
    ) -> Self {
        Self {
            raw_incidence_visits,
            unique_nominated_requests,
            excluded_existing_requests,
            selected_sources,
            physical_shift_occurrences,
            distinct_physical_shifts,
        }
    }
}

/// Ordinal-free scalar record of one fresh scheduler epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InteriorSimplexIterationTelemetry {
    epoch_ordinal: usize,
    request_count: usize,
    physical_rows: usize,
    physical_columns: usize,
    physical_entries: usize,
    allowed_columns: usize,
    forbidden_columns: usize,
    forbidden_rank: usize,
    augmented_rank: usize,
    disposition: ProbeLocalIterationDisposition,
}

impl InteriorSimplexIterationTelemetry {
    pub(crate) const fn epoch_ordinal(self) -> usize {
        self.epoch_ordinal
    }

    pub(crate) const fn request_count(self) -> usize {
        self.request_count
    }

    pub(crate) const fn physical_rows(self) -> usize {
        self.physical_rows
    }

    pub(crate) const fn physical_columns(self) -> usize {
        self.physical_columns
    }

    pub(crate) const fn physical_entries(self) -> usize {
        self.physical_entries
    }

    pub(crate) const fn allowed_columns(self) -> usize {
        self.allowed_columns
    }

    pub(crate) const fn forbidden_columns(self) -> usize {
        self.forbidden_columns
    }

    pub(crate) const fn forbidden_rank(self) -> usize {
        self.forbidden_rank
    }

    pub(crate) const fn augmented_rank(self) -> usize {
        self.augmented_rank
    }

    pub(crate) const fn disposition(self) -> ProbeLocalIterationDisposition {
        self.disposition
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) const fn new(
        epoch_ordinal: usize,
        request_count: usize,
        physical_rows: usize,
        physical_columns: usize,
        physical_entries: usize,
        allowed_columns: usize,
        forbidden_columns: usize,
        forbidden_rank: usize,
        augmented_rank: usize,
        disposition: ProbeLocalIterationDisposition,
    ) -> Self {
        Self {
            epoch_ordinal,
            request_count,
            physical_rows,
            physical_columns,
            physical_entries,
            allowed_columns,
            forbidden_columns,
            forbidden_rank,
            augmented_rank,
            disposition,
        }
    }
}

/// Compact typed outcome after dropping every frame/circuit/dual payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InteriorSimplexReplayRetention {
    /// `ExactTargetCircuit` retains physical-plan identity and cannot cross
    /// this boundary. The canonical-replay layer must detach it first.
    UnsupportedEpochBoundCircuit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InteriorSimplexOutcomeTelemetry {
    Replayed {
        exact_support: InteriorSimplexReplayRetention,
        final_requests: usize,
        selected_sources: usize,
        residual_terms: usize,
        pivot_guards: usize,
        nonzero_guards: usize,
    },
    SupportDidNotLift {
        final_requests: usize,
        selected_sources: usize,
        exact_forbidden_rank: usize,
        exact_augmented_rank: usize,
    },
    ExactLiftError {
        final_requests: usize,
    },
    SampledDual {
        final_requests: usize,
        obstruction_entries: usize,
        structurally_incident_rows: usize,
        evaluated_unseen_rows: usize,
        evaluated_source_terms: usize,
        paired_source_terms: usize,
    },
    BudgetStop {
        final_requests: Option<usize>,
        stage: ProbeLocalStage,
        resource: &'static str,
    },
    Rejected {
        final_requests: Option<usize>,
        stage: ProbeLocalStage,
    },
    Stalled {
        final_requests: usize,
        nonzero_residual_requests: usize,
    },
}

/// Conservative logical-cell census of compact retained payloads.
///
/// Object counts are exact. Coordinate and byte counts deliberately charge
/// each retaining task/probe even when its buffer is shared through `Arc`, so
/// they are deterministic upper bounds on physical allocations rather than a
/// heap-identity census. Exact circuits, sampled-dual obstructions, requests,
/// frames, and support entries are structurally zero because this boundary
/// retains only scalar summaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InteriorSimplexRetainedPayloadCensus {
    task_reports: usize,
    task_probe_reports: usize,
    task_key_coordinate_cells: usize,
    stable_scope_key_bytes: usize,
    probe_coordinate_cells: usize,
    iteration_records: usize,
}

impl InteriorSimplexRetainedPayloadCensus {
    pub(crate) const fn task_reports(self) -> usize {
        self.task_reports
    }

    pub(crate) const fn task_probe_reports(self) -> usize {
        self.task_probe_reports
    }

    pub(crate) const fn task_key_coordinate_cells(self) -> usize {
        self.task_key_coordinate_cells
    }

    pub(crate) const fn stable_scope_key_bytes(self) -> usize {
        self.stable_scope_key_bytes
    }

    pub(crate) const fn probe_coordinate_cells(self) -> usize {
        self.probe_coordinate_cells
    }

    pub(crate) const fn iteration_records(self) -> usize {
        self.iteration_records
    }

    pub(crate) const fn retained_exact_circuits(self) -> usize {
        0
    }

    pub(crate) const fn retained_support_entries(self) -> usize {
        0
    }

    pub(super) const fn new(
        task_reports: usize,
        task_probe_reports: usize,
        task_key_coordinate_cells: usize,
        stable_scope_key_bytes: usize,
        probe_coordinate_cells: usize,
        iteration_records: usize,
    ) -> Self {
        Self {
            task_reports,
            task_probe_reports,
            task_key_coordinate_cells,
            stable_scope_key_bytes,
            probe_coordinate_cells,
            iteration_records,
        }
    }

    pub(super) const fn with_iteration_records(self, iteration_records: usize) -> Self {
        Self {
            iteration_records,
            ..self
        }
    }
}

/// Compact result of one declared probe, retaining no physical frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InteriorSimplexProbeTelemetry {
    probe_ordinal: usize,
    probe: CampaignModularProbe,
    iterations: Vec<InteriorSimplexIterationTelemetry>,
    outcome: InteriorSimplexOutcomeTelemetry,
}

impl InteriorSimplexProbeTelemetry {
    pub(crate) const fn probe_ordinal(&self) -> usize {
        self.probe_ordinal
    }

    pub(crate) const fn probe(&self) -> &CampaignModularProbe {
        &self.probe
    }

    pub(crate) fn iterations(&self) -> &[InteriorSimplexIterationTelemetry] {
        &self.iterations
    }

    pub(crate) const fn outcome(&self) -> InteriorSimplexOutcomeTelemetry {
        self.outcome
    }

    pub(super) const fn new(
        probe_ordinal: usize,
        probe: CampaignModularProbe,
        iterations: Vec<InteriorSimplexIterationTelemetry>,
        outcome: InteriorSimplexOutcomeTelemetry,
    ) -> Self {
        Self {
            probe_ordinal,
            probe,
            iterations,
            outcome,
        }
    }
}

/// Canonically positioned compact result of one independent task scheduler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InteriorSimplexTaskExecutionReport {
    canonical_ordinal: usize,
    task_key: InteriorSimplexTaskKey,
    target_shift: IntegralShift,
    bootstrap: InteriorSimplexBootstrapTelemetry,
    probes: Vec<InteriorSimplexProbeTelemetry>,
    census: ProbeLocalRunCensus,
}

impl InteriorSimplexTaskExecutionReport {
    pub(crate) const fn canonical_ordinal(&self) -> usize {
        self.canonical_ordinal
    }

    pub(crate) const fn task_key(&self) -> &InteriorSimplexTaskKey {
        &self.task_key
    }

    pub(crate) const fn target_shift(&self) -> &IntegralShift {
        &self.target_shift
    }

    pub(crate) const fn bootstrap(&self) -> InteriorSimplexBootstrapTelemetry {
        self.bootstrap
    }

    pub(crate) fn probes(&self) -> &[InteriorSimplexProbeTelemetry] {
        &self.probes
    }

    pub(crate) const fn census(&self) -> ProbeLocalRunCensus {
        self.census
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) const fn new(
        canonical_ordinal: usize,
        task_key: InteriorSimplexTaskKey,
        target_shift: IntegralShift,
        bootstrap: InteriorSimplexBootstrapTelemetry,
        probes: Vec<InteriorSimplexProbeTelemetry>,
        census: ProbeLocalRunCensus,
    ) -> Self {
        Self {
            canonical_ordinal,
            task_key,
            target_shift,
            bootstrap,
            probes,
            census,
        }
    }
}

/// Complete compact serial result for one consumed planning epoch.
///
/// It deliberately carries no aggregate disposition such as complete,
/// exhausted, terminal, or closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InteriorSimplexExecutionReport {
    plan_epoch_ordinal: u64,
    interior_margin: u64,
    polynomial_degree_ceiling: usize,
    retained_payload: InteriorSimplexRetainedPayloadCensus,
    tasks: Vec<InteriorSimplexTaskExecutionReport>,
}

impl InteriorSimplexExecutionReport {
    pub(crate) const fn plan_epoch_ordinal(&self) -> u64 {
        self.plan_epoch_ordinal
    }

    pub(crate) const fn interior_margin(&self) -> u64 {
        self.interior_margin
    }

    pub(crate) const fn polynomial_degree_ceiling(&self) -> usize {
        self.polynomial_degree_ceiling
    }

    pub(crate) fn tasks(&self) -> &[InteriorSimplexTaskExecutionReport] {
        &self.tasks
    }

    pub(crate) const fn retained_payload(&self) -> InteriorSimplexRetainedPayloadCensus {
        self.retained_payload
    }

    pub(super) const fn new(
        plan_epoch_ordinal: u64,
        interior_margin: u64,
        polynomial_degree_ceiling: usize,
        retained_payload: InteriorSimplexRetainedPayloadCensus,
        tasks: Vec<InteriorSimplexTaskExecutionReport>,
    ) -> Self {
        Self {
            plan_epoch_ordinal,
            interior_margin,
            polynomial_degree_ceiling,
            retained_payload,
            tasks,
        }
    }
}
