use crate::algebra::IndexedAlgebraLimits;

/// Resource policy for one selected-minor exact lift and full physical replay.
///
/// Symbolica does not expose a scratch-memory census for sparse row
/// reduction. `max_native_decomposition_nonzero_entries` therefore admits
/// the conservative retained `U + L` envelope `R * (P + 2R)` before the
/// native reducer is constructed, where `R` is the selected-row count and
/// `P = |F| + 1` is the projected physical-column count. Likewise,
/// `indexed_algebra` authenticates every input and admitted output and bounds
/// the independent replay, but Symbolica 2.2.0 offers no per-operation limit
/// hook inside `SparseRowReducer`. These policies are therefore not a hard
/// bound on native intermediate coefficient growth or RSS; promotion requires
/// an isolated worker-level memory/time envelope in addition to these limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactCircuitLimits {
    pub(crate) indexed_algebra: IndexedAlgebraLimits,
    pub(crate) max_physical_columns: usize,
    pub(crate) max_selected_rows: usize,
    pub(crate) max_projected_physical_columns: usize,
    pub(crate) max_augmented_columns: usize,
    pub(crate) max_projected_input_nonzero_entries: usize,
    pub(crate) max_native_decomposition_nonzero_entries: usize,
    pub(crate) max_pivot_dependency_entries: usize,
    pub(crate) max_source_combination_terms: usize,
    pub(crate) max_replay_source_terms: usize,
    pub(crate) max_replay_exact_operations: usize,
    pub(crate) max_circuit_terms: usize,
    pub(crate) max_dependency_owner_witnesses: usize,
    pub(crate) max_guards: usize,
    pub(crate) max_guard_origins: usize,
    pub(crate) max_condition_source_entries: usize,
}

impl Default for ExactCircuitLimits {
    fn default() -> Self {
        Self {
            indexed_algebra: IndexedAlgebraLimits::default(),
            max_physical_columns: 4_000_000,
            max_selected_rows: 65_536,
            max_projected_physical_columns: 1_000_000,
            max_augmented_columns: 1_100_000,
            max_projected_input_nonzero_entries: 16_000_000,
            max_native_decomposition_nonzero_entries: 64_000_000,
            max_pivot_dependency_entries: 64_000_000,
            max_source_combination_terms: 65_536,
            max_replay_source_terms: 16_000_000,
            max_replay_exact_operations: 100_000_000,
            max_circuit_terms: 4_000_000,
            max_dependency_owner_witnesses: 16_000_000,
            max_guards: 1_000_000,
            max_guard_origins: 4_000_000,
            max_condition_source_entries: 4_000_000,
        }
    }
}
