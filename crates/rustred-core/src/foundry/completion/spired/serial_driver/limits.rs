use crate::foundry::completion::source_discovery::CampaignLimits;

use super::super::{SpiredCoordinateCaseWorklistLimits, SpiredEqualityStepLimits};

/// Retention envelope for one explicit deterministic modular-probe portfolio.
///
/// The Cartesian product is intentional and bounded at construction.  Its
/// chronology is chart-rank point, then base-parameter point, then modulus; no hash
/// iteration or topology-specific ordering can perturb it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredSerialProbePortfolioLimits {
    pub(crate) max_moduli: usize,
    pub(crate) max_base_parameter_points: usize,
    pub(crate) max_base_parameter_cells: usize,
    pub(crate) max_chart_rank_points: usize,
    pub(crate) max_chart_rank_cells: usize,
    pub(crate) max_probe_templates: usize,
}

impl Default for SpiredSerialProbePortfolioLimits {
    fn default() -> Self {
        Self {
            max_moduli: 256,
            max_base_parameter_points: 4_096,
            max_base_parameter_cells: 1_048_576,
            max_chart_rank_points: 65_536,
            max_chart_rank_cells: 4_194_304,
            max_probe_templates: 4_194_304,
        }
    }
}

/// Aggregate resource policy for one resumable serial drive.
///
/// Per-case search retains [`SpiredEqualityStepLimits`].  The duplicate
/// counters here prevent a sequence of exact guard children from multiplying
/// that nominal allowance without an outer campaign bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredSerialDriverLimits {
    pub(crate) max_case_attempts: usize,
    pub(crate) max_case_reports: usize,
    pub(crate) max_generated_probes: usize,
    pub(crate) max_generated_probe_coordinate_cells: usize,
    pub(crate) max_probe_attempts: usize,
    pub(crate) max_scheduled_requests: usize,
    pub(crate) max_streamed_rows: usize,
    pub(crate) max_modular_hits: usize,
    pub(crate) max_exact_lift_attempts: usize,
    pub(crate) max_existing_terminal_retirements: usize,
    pub(crate) max_finite_residual_points: usize,
    pub(crate) max_finite_residual_coordinate_cells: usize,
}

impl Default for SpiredSerialDriverLimits {
    fn default() -> Self {
        Self {
            max_case_attempts: 1_048_576,
            max_case_reports: 1_048_576,
            max_generated_probes: 268_435_456,
            max_generated_probe_coordinate_cells: 17_179_869_184,
            max_probe_attempts: 268_435_456,
            max_scheduled_requests: 1_073_741_824,
            max_streamed_rows: 1_073_741_824,
            max_modular_hits: 268_435_456,
            max_exact_lift_attempts: 268_435_456,
            max_existing_terminal_retirements: 67_108_864,
            max_finite_residual_points: 1_048_576,
            max_finite_residual_coordinate_cells: 67_108_864,
        }
    }
}

/// Complete policy for the current serial implementation slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredSerialDriverConfig {
    pub(crate) worklist: SpiredCoordinateCaseWorklistLimits,
    pub(crate) probe_campaign: CampaignLimits,
    pub(crate) equality_step: SpiredEqualityStepLimits,
    pub(crate) limits: SpiredSerialDriverLimits,
}

impl Default for SpiredSerialDriverConfig {
    fn default() -> Self {
        Self {
            worklist: SpiredCoordinateCaseWorklistLimits::default(),
            probe_campaign: CampaignLimits::default(),
            equality_step: SpiredEqualityStepLimits::default(),
            limits: SpiredSerialDriverLimits::default(),
        }
    }
}
