use crate::foundry::completion::source_discovery::leader_walk::LeaderWalkLimits;
use crate::sector::CoordinatePriority;

/// Cumulative work envelope for one live-ledger fixed-point drive.
///
/// These limits never reset after an owner mutation. Nested target runners
/// retain their own per-lane limits; the counters here prevent repeated
/// probes, exclusions, and replans from escaping an aggregate bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredFixedPointLimits {
    pub(crate) max_epochs: usize,
    pub(crate) max_plans: usize,
    pub(crate) max_planned_targets: usize,
    pub(crate) max_targets_inspected: usize,
    pub(crate) max_target_portfolios: usize,
    pub(crate) max_target_lane_runs: usize,
    pub(crate) max_probe_attempts: usize,
    pub(crate) max_scheduled_requests: usize,
    pub(crate) max_streamed_rows: usize,
    pub(crate) max_modular_hits: usize,
    pub(crate) max_exact_lift_attempts: usize,
    pub(crate) max_rejected_hits: usize,
    pub(crate) max_alternative_nodes: usize,
    pub(crate) max_excluded_requests: usize,
    pub(crate) max_excluded_request_coordinate_cells: usize,
    pub(crate) max_guard_predicates: usize,
    pub(crate) max_guard_obligations: usize,
    pub(crate) max_owner_compile_attempts: usize,
    pub(crate) max_owner_mutations: usize,
}

impl Default for SpiredFixedPointLimits {
    fn default() -> Self {
        Self {
            max_epochs: 1_048_576,
            max_plans: 1_048_576,
            max_planned_targets: 67_108_864,
            max_targets_inspected: 67_108_864,
            max_target_portfolios: 67_108_864,
            max_target_lane_runs: 268_435_456,
            max_probe_attempts: 268_435_456,
            max_scheduled_requests: 1_073_741_824,
            max_streamed_rows: 1_073_741_824,
            max_modular_hits: 268_435_456,
            max_exact_lift_attempts: 268_435_456,
            max_rejected_hits: 268_435_456,
            max_alternative_nodes: 67_108_864,
            max_excluded_requests: 1_073_741_824,
            max_excluded_request_coordinate_cells: 17_179_869_184,
            max_guard_predicates: 67_108_864,
            max_guard_obligations: 67_108_864,
            max_owner_compile_attempts: 268_435_456,
            max_owner_mutations: 16_777_216,
        }
    }
}

/// Remaining aggregate allowance handed to one target portfolio.
///
/// A real runner must apply its stricter per-lane policy as well and report
/// its complete work delta. Returning a delta outside this allowance is a
/// runner-contract violation, not a silently truncated campaign.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredTargetPortfolioBudget {
    pub(crate) target_lane_runs: usize,
    pub(crate) probe_attempts: usize,
    pub(crate) scheduled_requests: usize,
    pub(crate) streamed_rows: usize,
    pub(crate) modular_hits: usize,
    pub(crate) exact_lift_attempts: usize,
    pub(crate) rejected_hits: usize,
    pub(crate) alternative_nodes: usize,
    pub(crate) excluded_requests: usize,
    pub(crate) excluded_request_coordinate_cells: usize,
    pub(crate) guard_predicates: usize,
    pub(crate) guard_obligations: usize,
    pub(crate) owner_compile_attempts: usize,
}

/// Immutable policy for one fixed-point drive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredFixedPointConfig {
    pub(crate) leader_walk: LeaderWalkLimits,
    /// Optional proposal chronology only. It never changes exact ordering.
    pub(crate) discovery_coordinate_priority: Option<CoordinatePriority>,
    pub(crate) limits: SpiredFixedPointLimits,
}

impl Default for SpiredFixedPointConfig {
    fn default() -> Self {
        Self {
            leader_walk: LeaderWalkLimits::default(),
            discovery_coordinate_priority: None,
            limits: SpiredFixedPointLimits::default(),
        }
    }
}
