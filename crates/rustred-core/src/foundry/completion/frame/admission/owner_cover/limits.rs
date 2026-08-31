//! Resource policy for guarded exact-circuit owner-cover compilation.

use crate::algebra::IndexedGuardLimits;
use crate::foundry::completion::CompletionGeometryLimits;
use crate::foundry::completion::guard::decision::GuardDecisionEvaluationLimits;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactCircuitOwnerCoverLimits {
    pub(crate) max_owner_inputs: usize,
    pub(crate) max_owner_coordinate_cells: usize,
    pub(crate) max_explicit_terminals: usize,
    pub(crate) max_terminal_coordinate_cells: usize,
    pub(crate) max_finite_complement_points: usize,
    pub(crate) max_finite_complement_coordinate_cells: usize,
    pub(crate) max_point_owner_probes: usize,
    pub(crate) geometry: CompletionGeometryLimits,
    pub(crate) guard_locus: IndexedGuardLimits,
    pub(crate) guard_evaluation: GuardDecisionEvaluationLimits,
}

impl Default for ExactCircuitOwnerCoverLimits {
    fn default() -> Self {
        Self {
            max_owner_inputs: 4_096,
            max_owner_coordinate_cells: 16_777_216,
            max_explicit_terminals: 1_048_576,
            max_terminal_coordinate_cells: 16_777_216,
            max_finite_complement_points: 1_048_576,
            max_finite_complement_coordinate_cells: 16_777_216,
            max_point_owner_probes: 268_435_456,
            geometry: CompletionGeometryLimits::default(),
            guard_locus: IndexedGuardLimits::default(),
            guard_evaluation: GuardDecisionEvaluationLimits::default(),
        }
    }
}
