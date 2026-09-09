use crate::foundry::completion::stratum::StratumRegistryLimits;

use super::super::DirectShiftedSourceLimits;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredStructuralPreparationLimits {
    pub(crate) evaluation: DirectShiftedSourceLimits,
    pub(crate) classification: StratumRegistryLimits,
    pub(crate) max_cached_shifts: usize,
    pub(crate) max_cached_shift_coordinate_cells: usize,
    pub(crate) max_request_inspections: usize,
    pub(crate) max_source_term_inspections: usize,
    pub(crate) max_prepared_rows: usize,
    pub(crate) max_prepared_term_roles: usize,
    pub(crate) max_prepared_request_offset_cells: usize,
}

impl Default for SpiredStructuralPreparationLimits {
    fn default() -> Self {
        Self {
            evaluation: DirectShiftedSourceLimits::default(),
            classification: StratumRegistryLimits::default(),
            max_cached_shifts: 1_048_576,
            max_cached_shift_coordinate_cells: 67_108_864,
            max_request_inspections: 268_435_456,
            max_source_term_inspections: 4_294_967_295,
            max_prepared_rows: 268_435_456,
            max_prepared_term_roles: 4_294_967_295,
            max_prepared_request_offset_cells: 4_294_967_295,
        }
    }
}
