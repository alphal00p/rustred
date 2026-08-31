use crate::foundry::completion::source_discovery::scheduler::ProbeLocalSchedulerLimits;

/// Aggregate outer envelope for one serial simplex execution.
///
/// Nested scheduler limits remain authoritative per task. These limits bound
/// the number of independent schedulers and every variable-size compact
/// telemetry payload retained across the complete plan. Physical frames,
/// request accumulators, duals, and circuits are never retained here. No cap
/// truncates a task or returns a partial report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InteriorSimplexExecutionLimits {
    pub(crate) scheduler: ProbeLocalSchedulerLimits,
    pub(crate) max_tasks: usize,
    pub(crate) max_retained_task_reports: usize,
    pub(crate) max_task_probe_runs: usize,
    pub(crate) max_retained_probe_coordinate_cells: usize,
    pub(crate) max_retained_task_key_coordinate_cells: usize,
    pub(crate) max_retained_stable_scope_key_bytes: usize,
    pub(crate) max_retained_iteration_records: usize,
    pub(crate) max_aggregate_bootstrap_requests: usize,
    pub(crate) max_bootstrap_physical_shifts_per_task: usize,
    pub(crate) max_aggregate_bootstrap_physical_shifts: usize,
    pub(crate) max_aggregate_bootstrap_shift_coordinate_cells: usize,
}

impl Default for InteriorSimplexExecutionLimits {
    fn default() -> Self {
        Self {
            scheduler: ProbeLocalSchedulerLimits::default(),
            max_tasks: 4_096,
            max_retained_task_reports: 4_096,
            max_task_probe_runs: 16_384,
            max_retained_probe_coordinate_cells: 16_777_216,
            max_retained_task_key_coordinate_cells: 67_108_864,
            max_retained_stable_scope_key_bytes: 16_777_216,
            max_retained_iteration_records: 16_777_216,
            max_aggregate_bootstrap_requests: 16_777_216,
            max_bootstrap_physical_shifts_per_task: 4_194_304,
            max_aggregate_bootstrap_physical_shifts: 67_108_864,
            max_aggregate_bootstrap_shift_coordinate_cells: 1_073_741_824,
        }
    }
}
