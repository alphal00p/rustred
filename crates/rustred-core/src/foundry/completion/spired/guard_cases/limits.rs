use crate::algebra::{IndexedAlgebraLimits, IndexedGuardLimits};
use crate::foundry::completion::stratum::StratumRegistryLimits;

/// Aggregate resource envelope for one exact guard-to-coordinate-case pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredCoordinateGuardCaseLimits {
    pub(crate) indexed_algebra: IndexedAlgebraLimits,
    pub(crate) guard_algebra: IndexedGuardLimits,
    pub(crate) strata: StratumRegistryLimits,
    pub(crate) max_required_predicates: usize,
    pub(crate) max_guard_ordinal_references: usize,
    pub(crate) max_exact_hyperplanes: usize,
    pub(crate) max_output_cases: usize,
    pub(crate) max_output_coordinate_cells: usize,
    pub(crate) max_output_identity_bytes: usize,
}

impl Default for SpiredCoordinateGuardCaseLimits {
    fn default() -> Self {
        Self {
            indexed_algebra: IndexedAlgebraLimits::default(),
            guard_algebra: IndexedGuardLimits::default(),
            strata: StratumRegistryLimits::default(),
            max_required_predicates: 4_096,
            max_guard_ordinal_references: 4_096,
            max_exact_hyperplanes: 65_536,
            max_output_cases: 1_048_576,
            max_output_coordinate_cells: 33_554_432,
            max_output_identity_bytes: 67_108_864,
        }
    }
}
