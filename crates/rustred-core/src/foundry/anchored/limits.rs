use crate::algebra::IndexedAlgebraLimits;

/// Resource policy for deriving and replaying one concrete anchored rule.
///
/// The structural limits cover every caller-sized Rust container retained by
/// this boundary and the visible `U`/`L` output of Symbolica's sparse reducer.
/// Symbolica 2.2.0 does not expose a scratch-memory census or cancellation
/// hook for `SparseRowReducer`, so these limits are not a hard RSS bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnchoredRuleLimits {
    pub indexed_algebra: IndexedAlgebraLimits,
    pub max_source_rows: usize,
    pub max_integral_columns: usize,
    pub max_augmented_columns: usize,
    pub max_input_nonzero_entries: usize,
    /// Maximum live `i64` power slots owned by integral keys while preparing
    /// source rows or assembling the returned rule.
    pub max_integral_key_power_cells: usize,
    /// Maximum `i64` coordinate slots cloned into foundry-owned guard
    /// provenance, including nested identity-condition shifts and offsets.
    pub max_guard_provenance_index_cells: usize,
    /// Maximum live coordinate slots in ordering keys. Every
    /// [`crate::sector::ComplexityKey`] owns one sector-bit buffer and one
    /// index-excess buffer, so each key contributes twice its arity.
    pub max_ordering_key_coordinate_cells: usize,
    pub max_native_decomposition_nonzero_entries: usize,
    /// Maximum conservative nonzero-entry capacity admitted for the complete
    /// serial Symbolica back-substitution output. This is charged only by the
    /// target-directed API.
    pub max_back_substitution_output_nonzero_entries: usize,
    /// Maximum conservative live nonzero-entry capacity while the original
    /// forward `U`/`L`, the copied physical-pivot rows of `U`, and the
    /// prospective serial back-substitution output coexist. Column-bounded
    /// pivot-map and native scratch storage are not counted in these units.
    /// This is charged only by the target-directed API.
    pub max_back_substitution_live_nonzero_entries: usize,
    pub max_rule_guards: usize,
    pub max_guard_origins: usize,
    pub max_guard_provenance_sources: usize,
    pub max_elimination_pivots: usize,
    pub max_source_combination_terms: usize,
    pub max_replay_exact_operations: usize,
}

impl Default for AnchoredRuleLimits {
    fn default() -> Self {
        Self {
            indexed_algebra: IndexedAlgebraLimits::default(),
            max_source_rows: 65_536,
            max_integral_columns: 1_000_000,
            max_augmented_columns: 2_000_000,
            max_input_nonzero_entries: 16_000_000,
            max_integral_key_power_cells: 64_000_000,
            max_guard_provenance_index_cells: 64_000_000,
            max_ordering_key_coordinate_cells: 128_000_000,
            max_native_decomposition_nonzero_entries: 64_000_000,
            max_back_substitution_output_nonzero_entries: 64_000_000,
            max_back_substitution_live_nonzero_entries: 192_000_000,
            max_rule_guards: 1_000_000,
            max_guard_origins: 4_000_000,
            max_guard_provenance_sources: 4_000_000,
            max_elimination_pivots: 65_536,
            max_source_combination_terms: 65_536,
            max_replay_exact_operations: 100_000_000,
        }
    }
}
