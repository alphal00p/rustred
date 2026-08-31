use super::super::InteriorReplayRunLimits;

/// Complete one-task resource envelope for bootstrap construction, streaming
/// replay, and bounded retention of exact refinement obstructions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InteriorCampaignLimits {
    pub(crate) replay: InteriorReplayRunLimits,
    pub(crate) max_bootstrap_physical_shift_occurrences: usize,
    pub(crate) max_bootstrap_physical_shift_coordinate_cells: usize,
    /// Logical `n * ceil(log2(max(n, 2)))` work reserved before sorting the
    /// task-local physical-shift views.
    pub(crate) max_bootstrap_physical_shift_sort_work: usize,
    pub(crate) max_bootstrap_distinct_physical_shifts: usize,
    /// Post-compilation obstruction count retained in the returned report.
    /// Transient exact-candidate allocation inside replay is independently
    /// bounded by `replay.owner.max_candidates_per_owner`.
    pub(crate) max_retained_exact_obstructions: usize,
}

impl Default for InteriorCampaignLimits {
    fn default() -> Self {
        Self {
            replay: InteriorReplayRunLimits::default(),
            max_bootstrap_physical_shift_occurrences: 4_194_304,
            max_bootstrap_physical_shift_coordinate_cells: 67_108_864,
            max_bootstrap_physical_shift_sort_work: 92_274_688,
            max_bootstrap_distinct_physical_shifts: 4_194_304,
            max_retained_exact_obstructions: 4_096,
        }
    }
}
