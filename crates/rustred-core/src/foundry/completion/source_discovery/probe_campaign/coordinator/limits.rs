use super::super::super::boundary_simplex::BoundarySimplexLimits;

/// Aggregate retained-work envelope for the window-one coordinator.
///
/// Planner limits apply independently to each materialized class plan. Epoch,
/// plan, and report limits are cumulative over one coordinator instance, so
/// owner-triggered replans cannot reset the outer work budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProbeCoordinatorLimits {
    pub(crate) boundary_plan: BoundarySimplexLimits,
    pub(crate) max_present_dimensions: usize,
    pub(crate) max_classes_per_epoch: usize,
    pub(crate) max_epochs: usize,
    pub(crate) max_plans: usize,
    pub(crate) max_task_reports: usize,
    pub(crate) max_probes_per_task: usize,
}

impl Default for ProbeCoordinatorLimits {
    fn default() -> Self {
        Self {
            boundary_plan: BoundarySimplexLimits::default(),
            max_present_dimensions: 4_097,
            max_classes_per_epoch: 8_396_801,
            max_epochs: 1_048_576,
            max_plans: 8_396_801,
            max_task_reports: 67_108_864,
            max_probes_per_task: 4_096,
        }
    }
}
