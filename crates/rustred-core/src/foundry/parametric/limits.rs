use crate::algebra::IndexedAlgebraLimits;

/// Resource policy for deriving and authenticating one parametric rule.
///
/// These limits cover every caller-sized Rust container, every checked exact
/// replay operation, and the visible `U`/`L` output of Symbolica's sparse
/// reducer. Symbolica does not expose a scratch-memory census for that native
/// algorithm, so this is not a hard process-RSS limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParametricRuleLimits {
    pub indexed_algebra: IndexedAlgebraLimits,
    pub max_source_rows: usize,
    pub max_shift_columns: usize,
    pub max_augmented_columns: usize,
    pub max_input_nonzero_entries: usize,
    /// Maximum non-provenance `i64` index-coordinate cells retained by the
    /// foundry boundary.
    ///
    /// Every value-distinct source shift is canonicalized to one retained Arc
    /// buffer and counted once, together with the copied replay anchor.
    /// Cloning that canonical `IndexShift` into columns, pivots, RHS terms,
    /// pivot guards, and guard origins only adds Arc handles, so it never
    /// counts the same coordinate buffer again. Domain endpoints have their
    /// own limit.
    pub max_index_coordinate_cells: usize,
    /// Maximum retained `i128` coordinates in structural ordering keys.
    pub max_ordering_key_coordinate_cells: usize,
    /// Maximum retained lower/upper `i64` endpoint cells across the prepared
    /// fixed-sector interior, a sector-monotone parent box, and every
    /// term-local same-sector cell.
    pub max_domain_bound_endpoint_cells: usize,
    /// Maximum active-coordinate thresholds retained across the compact
    /// term-local pinch partitions of one sector-monotone rule.
    pub max_sector_monotone_thresholds: usize,
    /// Maximum retained boolean cells in the sector mask.
    pub max_sector_mask_cells: usize,
    /// Maximum foundry-owned provenance coordinate cells deep-cloned from
    /// nested identity-condition shifts and offsets. Guard origins carrying
    /// `IndexShift` use shared handles and add no coordinate buffer.
    pub max_guard_provenance_index_cells: usize,
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
    /// Maximum aggregate dependency ordinals retained across every native
    /// reducer row, rather than only the depth of one selected chain.
    pub max_elimination_pivot_dependency_entries: usize,
    pub max_source_combination_terms: usize,
    pub max_replay_exact_operations: usize,
    /// Maximum source and rule terms specialized by the concrete base-field
    /// replay. This bounds work before any integral-key allocation starts.
    pub max_concrete_replay_terms: usize,
    /// Maximum distinct integral keys retained by the concrete replay's exact
    /// base-field accumulator.
    pub max_concrete_replay_integral_keys: usize,
    /// Maximum aggregate numerator-plus-denominator term count retained by
    /// base-field coefficients in the concrete replay accumulator.
    pub max_concrete_replay_retained_coefficient_terms: usize,
    /// Maximum aggregate clone-owned bytes retained by base-field
    /// coefficients in the concrete replay accumulator. This is deliberately
    /// runtime-only policy state: native capacities and `size_of` are not
    /// cross-platform semantic evidence and are never persisted in a rule.
    pub max_concrete_replay_retained_coefficient_clone_owned_bytes: usize,
    /// Maximum `i64` power cells constructed while specializing source terms,
    /// the pivot, and right-hand-side terms, plus the retained replay anchor.
    pub max_concrete_replay_integral_key_power_cells: usize,
    /// Maximum exact specializations, products, sums, negations, and
    /// coefficient comparisons performed by the concrete replay.
    pub max_concrete_replay_exact_operations: usize,
}

impl Default for ParametricRuleLimits {
    fn default() -> Self {
        Self {
            indexed_algebra: IndexedAlgebraLimits::default(),
            max_source_rows: 65_536,
            max_shift_columns: 1_000_000,
            max_augmented_columns: 2_000_000,
            max_input_nonzero_entries: 16_000_000,
            max_index_coordinate_cells: 64_000_000,
            max_ordering_key_coordinate_cells: 128_000_000,
            max_domain_bound_endpoint_cells: 8_192,
            max_sector_monotone_thresholds: 4_000_000,
            max_sector_mask_cells: 4_096,
            max_guard_provenance_index_cells: 64_000_000,
            max_native_decomposition_nonzero_entries: 64_000_000,
            max_back_substitution_output_nonzero_entries: 64_000_000,
            max_back_substitution_live_nonzero_entries: 192_000_000,
            max_rule_guards: 1_000_000,
            max_guard_origins: 4_000_000,
            max_guard_provenance_sources: 4_000_000,
            max_elimination_pivots: 65_536,
            max_elimination_pivot_dependency_entries: 64_000_000,
            max_source_combination_terms: 65_536,
            max_replay_exact_operations: 100_000_000,
            max_concrete_replay_terms: 64_000_000,
            max_concrete_replay_integral_keys: 16_000_000,
            max_concrete_replay_retained_coefficient_terms: 64_000_000,
            max_concrete_replay_retained_coefficient_clone_owned_bytes: 1024 * 1024 * 1024,
            max_concrete_replay_integral_key_power_cells: 64_000_000,
            max_concrete_replay_exact_operations: 200_000_000,
        }
    }
}
