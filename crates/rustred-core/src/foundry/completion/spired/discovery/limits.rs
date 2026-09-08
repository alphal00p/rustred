use crate::foundry::completion::stratum::StratumRegistryLimits;

use super::super::{DirectShiftedSourceLimits, SpiredModularLimits};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredStreamingLimits {
    pub(crate) evaluation: DirectShiftedSourceLimits,
    pub(crate) classification: StratumRegistryLimits,
    pub(crate) modular: SpiredModularLimits,
    pub(crate) max_cached_shifts: usize,
    pub(crate) max_cached_shift_coordinate_cells: usize,
    /// Cumulative request entries structurally inspected by chunk preparation.
    pub(crate) max_preparation_request_inspections: usize,
    /// Cumulative exact source terms visited by chunk preparation.
    pub(crate) max_preparation_source_term_inspections: usize,
}

impl Default for SpiredStreamingLimits {
    fn default() -> Self {
        Self {
            evaluation: DirectShiftedSourceLimits::default(),
            classification: StratumRegistryLimits::default(),
            modular: SpiredModularLimits::default(),
            max_cached_shifts: 1_048_576,
            max_cached_shift_coordinate_cells: 67_108_864,
            max_preparation_request_inspections: 268_435_456,
            max_preparation_source_term_inspections: 4_294_967_295,
        }
    }
}
