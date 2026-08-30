use crate::algebra::ExactAlgebraLimits;
use crate::foundry::completion::stratum::StratumRegistryLimits;

/// Aggregate resource policy for one exact guard refinement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactGuardRefinementLimits {
    pub(crate) max_circuit_guards: usize,
    pub(crate) max_circuit_guard_identity_bytes: usize,
    pub(crate) max_unique_predicates: usize,
    pub(crate) max_guard_ordinal_references: usize,
    pub(crate) max_exceptional_strata: usize,
    pub(crate) max_result_guard_branch_references: usize,
    pub(crate) max_result_stratum_identity_bytes: usize,
    pub(crate) exact_algebra: ExactAlgebraLimits,
    pub(crate) strata: StratumRegistryLimits,
}

impl Default for ExactGuardRefinementLimits {
    fn default() -> Self {
        Self {
            max_circuit_guards: 4_096,
            max_circuit_guard_identity_bytes: 67_108_864,
            max_unique_predicates: 4_096,
            max_guard_ordinal_references: 4_096,
            max_exceptional_strata: 4_096,
            max_result_guard_branch_references: 16_777_216,
            max_result_stratum_identity_bytes: 67_108_864,
            exact_algebra: ExactAlgebraLimits::default(),
            strata: StratumRegistryLimits::default(),
        }
    }
}
