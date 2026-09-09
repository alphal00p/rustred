use crate::foundry::completion::stratum::StratumRegistryLimits;

use super::super::{
    DirectShiftedSourceLimits, SpiredModularLimits, SpiredStructuralPreparationLimits,
};

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

impl SpiredStreamingLimits {
    /// Extract the exact case/source/role limits for a shared structural
    /// preparation. Probe-local Symbolica reducer limits remain separate.
    pub(crate) const fn structural_preparation(self) -> SpiredStructuralPreparationLimits {
        SpiredStructuralPreparationLimits {
            evaluation: self.evaluation,
            classification: self.classification,
            max_cached_shifts: self.max_cached_shifts,
            max_cached_shift_coordinate_cells: self.max_cached_shift_coordinate_cells,
            max_request_inspections: self.max_preparation_request_inspections,
            max_source_term_inspections: self.max_preparation_source_term_inspections,
            max_prepared_rows: self.modular.max_rows,
            max_prepared_term_roles: self.modular.max_structural_terms,
            max_prepared_request_offset_cells: self.modular.max_retained_request_shift_components,
        }
    }
}
