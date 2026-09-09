use crate::foundry::completion::source_discovery::ExactExecutableOwnerLimits;

use super::super::{SpiredCoordinateGuardCaseLimits, SpiredTargetRunLimits};

/// Complete bounded policy for one immutable equality-case probe portfolio.
///
/// The aggregate counters matter even though every target run is separately
/// bounded: retrying many unlucky or inconclusive probes must not multiply a
/// nominal per-probe allowance without an explicit outer budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredEqualityStepLimits {
    pub(crate) target_run: SpiredTargetRunLimits,
    pub(crate) guard_cases: SpiredCoordinateGuardCaseLimits,
    pub(crate) executable_owner: ExactExecutableOwnerLimits,
    pub(crate) max_probes: usize,
    pub(crate) max_total_scheduled_requests: usize,
    pub(crate) max_total_streamed_rows: usize,
    pub(crate) max_total_modular_hits: usize,
    pub(crate) max_total_exact_lift_attempts: usize,
}

impl Default for SpiredEqualityStepLimits {
    fn default() -> Self {
        Self {
            target_run: SpiredTargetRunLimits::default(),
            guard_cases: SpiredCoordinateGuardCaseLimits::default(),
            executable_owner: ExactExecutableOwnerLimits::default(),
            max_probes: 64,
            max_total_scheduled_requests: 268_435_456,
            max_total_streamed_rows: 268_435_456,
            max_total_modular_hits: 262_144,
            max_total_exact_lift_attempts: 262_144,
        }
    }
}
