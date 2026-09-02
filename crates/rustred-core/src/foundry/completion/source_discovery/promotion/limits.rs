use crate::foundry::cell::RuleCellLimits;
use crate::foundry::completion::frame::admission::ExactGuardRefinementLimits;
use crate::foundry::completion::frame::exact::{ClearedCircuitLimits, ExactCircuitLoweringLimits};
use crate::foundry::completion::stratum::StratumRegistryLimits;

/// Aggregate cold-path policy for one exact circuit-to-cell promotion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactRuleCellPromotionLimits {
    pub(crate) partition: StratumRegistryLimits,
    pub(crate) clearing: ClearedCircuitLimits,
    pub(crate) guard_refinement: ExactGuardRefinementLimits,
    pub(crate) lowering: ExactCircuitLoweringLimits,
    pub(crate) cell: RuleCellLimits,
}

impl Default for ExactRuleCellPromotionLimits {
    fn default() -> Self {
        Self {
            partition: StratumRegistryLimits::default(),
            clearing: ClearedCircuitLimits::default(),
            guard_refinement: ExactGuardRefinementLimits::default(),
            lowering: ExactCircuitLoweringLimits::default(),
            cell: RuleCellLimits::default(),
        }
    }
}
