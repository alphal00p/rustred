use crate::foundry::completion::CompletionGeometryLimits;

use super::super::StagedSectorClosureLimits;

/// Aggregate cold-path envelope for staging and comparing one owner proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactOwnerCoverDeltaLimits {
    pub(crate) staged: StagedSectorClosureLimits,
    pub(crate) comparison_geometry: CompletionGeometryLimits,
    pub(crate) max_comparison_box_inputs: usize,
    pub(crate) max_comparison_coordinate_cells: usize,
    pub(crate) max_comparison_box_pair_probes: usize,
}

impl Default for ExactOwnerCoverDeltaLimits {
    fn default() -> Self {
        Self {
            staged: StagedSectorClosureLimits::default(),
            comparison_geometry: CompletionGeometryLimits::default(),
            max_comparison_box_inputs: 524_288,
            max_comparison_coordinate_cells: 16_777_216,
            max_comparison_box_pair_probes: 268_435_456,
        }
    }
}
