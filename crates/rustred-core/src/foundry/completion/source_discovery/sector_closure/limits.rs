use super::super::ExactExecutableOwnerLimits;
use crate::foundry::completion::stratum::StratumRegistryLimits;

/// Aggregate resource envelope for one staged same-rank publication wave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StagedSectorClosureLimits {
    pub(crate) max_sectors: usize,
    pub(crate) max_frontier_coordinate_cells: usize,
    pub(crate) max_staged_owners: usize,
    pub(crate) max_staged_owner_coordinate_cells: usize,
    pub(crate) max_staged_owner_candidate_slots: usize,
    pub(crate) max_staged_owner_content_key_bytes: usize,
    pub(crate) max_owner_order_comparisons: usize,
    pub(crate) max_staged_terminals: usize,
    pub(crate) max_staged_terminal_coordinate_cells: usize,
    pub(crate) max_compiled_pairing_probes: usize,
    pub(crate) max_compiled_finite_complement_points: usize,
    pub(crate) max_compiled_finite_complement_coordinate_cells: usize,
    pub(crate) max_compiled_point_owner_probes: usize,
    pub(crate) max_compiled_uncovered_boxes: usize,
    pub(crate) max_compiled_uncovered_box_coordinate_cells: usize,
    pub(crate) max_compiled_split_operations: usize,
    pub(crate) executable: ExactExecutableOwnerLimits,
    pub(crate) registry: StratumRegistryLimits,
}

impl Default for StagedSectorClosureLimits {
    fn default() -> Self {
        Self {
            max_sectors: 4_096,
            max_frontier_coordinate_cells: 16_777_216,
            max_staged_owners: 65_536,
            max_staged_owner_coordinate_cells: 16_777_216,
            max_staged_owner_candidate_slots: 4_194_304,
            max_staged_owner_content_key_bytes: 4_194_304,
            max_owner_order_comparisons: 16_777_216,
            max_staged_terminals: 1_048_576,
            max_staged_terminal_coordinate_cells: 16_777_216,
            max_compiled_pairing_probes: 16_777_216,
            max_compiled_finite_complement_points: 1_048_576,
            max_compiled_finite_complement_coordinate_cells: 16_777_216,
            max_compiled_point_owner_probes: 268_435_456,
            max_compiled_uncovered_boxes: 262_144,
            max_compiled_uncovered_box_coordinate_cells: 4_194_304,
            max_compiled_split_operations: 16_777_216,
            executable: ExactExecutableOwnerLimits::default(),
            registry: StratumRegistryLimits::default(),
        }
    }
}
