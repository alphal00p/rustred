use super::super::{
    CanonicalReplayLimits, ExactExecutableOwnerLimits, scheduler::ProbeLocalSchedulerLimits,
};

/// Complete resource envelope for one streaming scheduler, canonical replay,
/// owner compilation, and ordinal-free support extraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InteriorReplayRunLimits {
    pub(crate) scheduler: ProbeLocalSchedulerLimits,
    pub(crate) canonical_replay: CanonicalReplayLimits,
    pub(crate) owner: ExactExecutableOwnerLimits,
    pub(crate) max_support_candidates: usize,
    pub(crate) max_relative_source_supports: usize,
    pub(crate) max_relative_residual_supports: usize,
    pub(crate) max_relative_coordinate_cells: usize,
    pub(crate) max_support_sort_work: usize,
}

impl Default for InteriorReplayRunLimits {
    fn default() -> Self {
        Self {
            scheduler: ProbeLocalSchedulerLimits::default(),
            canonical_replay: CanonicalReplayLimits::default(),
            owner: ExactExecutableOwnerLimits::default(),
            max_support_candidates: 4_096,
            max_relative_source_supports: 1_048_576,
            max_relative_residual_supports: 1_048_576,
            max_relative_coordinate_cells: 67_108_864,
            max_support_sort_work: 16_777_216,
        }
    }
}
