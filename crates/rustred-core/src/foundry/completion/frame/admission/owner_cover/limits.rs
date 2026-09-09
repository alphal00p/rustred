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
    /// Preflight bound on semantic candidates inspected while constructing
    /// exact pointwise-guard coverage.
    pub(crate) max_guard_cover_candidates: usize,
    /// Preflight bound on guard atoms inspected across all candidates.
    pub(crate) max_guard_cover_atoms: usize,
    /// Exact coordinate hyperplanes retained across all guard atoms.
    pub(crate) max_guard_cover_hyperplanes: usize,
    /// Endpoint-coordinate cells retained by guard hyperplane boxes before
    /// they enter the generic box-cover compiler.
    pub(crate) max_guard_cover_hyperplane_coordinate_cells: usize,
    /// Candidate-applicability boxes retained before their final union.
    pub(crate) max_guard_cover_boxes: usize,
    /// Endpoint cells retained by candidate-applicability boxes.
    pub(crate) max_guard_cover_box_coordinate_cells: usize,
    /// Aggregate structural subtraction work used to remove guard walls.
    pub(crate) max_guard_cover_split_operations: usize,
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
            max_guard_cover_candidates: 1_048_576,
            max_guard_cover_atoms: 16_777_216,
            max_guard_cover_hyperplanes: 16_777_216,
            max_guard_cover_hyperplane_coordinate_cells: 268_435_456,
            max_guard_cover_boxes: 16_777_216,
            max_guard_cover_box_coordinate_cells: 268_435_456,
            max_guard_cover_split_operations: 268_435_456,
            geometry: CompletionGeometryLimits::default(),
            guard_locus: IndexedGuardLimits::default(),
            guard_evaluation: GuardDecisionEvaluationLimits::default(),
        }
    }
}
