use crate::algebra::IndexedAlgebraLimits;
use crate::foundry::anchored::AnchoredRuleLimits;

/// Resource policy for deriving and authenticating one parametric rule.
///
/// These limits cover every caller-sized Rust container, every checked exact
/// replay operation, and the visible `U`/`L` output of Symbolica's sparse
/// reducer. Symbolica does not expose a scratch-memory census for that native
/// algorithm, so this is not a hard process-RSS limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParametricRuleLimits {
    pub indexed_algebra: IndexedAlgebraLimits,
    pub anchored: AnchoredRuleLimits,
    pub max_source_rows: usize,
    pub max_shift_columns: usize,
    pub max_augmented_columns: usize,
    pub max_input_nonzero_entries: usize,
    /// Maximum non-provenance `i64` index-coordinate cells retained by the
    /// foundry boundary.
    ///
    /// Every value-distinct source shift is canonicalized to one retained Arc
    /// buffer and counted once, together with the copied agreement anchor.
    /// Cloning that canonical `IndexShift` into columns, pivots, RHS terms,
    /// pivot guards, and guard origins only adds Arc handles, so it never
    /// counts the same coordinate buffer again. Domain endpoints have their
    /// own limit.
    pub max_index_coordinate_cells: usize,
    /// Maximum retained `i128` coordinates in structural ordering keys.
    pub max_ordering_key_coordinate_cells: usize,
    /// Maximum retained lower/upper `i64` endpoint cells in the domain.
    pub max_domain_bound_endpoint_cells: usize,
    /// Maximum retained boolean cells in the sector mask.
    pub max_sector_mask_cells: usize,
    /// Maximum foundry-owned provenance coordinate cells deep-cloned from
    /// nested identity-condition shifts and offsets. Guard origins carrying
    /// `IndexShift` use shared handles and add no coordinate buffer.
    pub max_guard_provenance_index_cells: usize,
    pub max_native_decomposition_nonzero_entries: usize,
    pub max_rule_guards: usize,
    pub max_guard_origins: usize,
    pub max_guard_provenance_sources: usize,
    pub max_elimination_pivots: usize,
    /// Maximum aggregate dependency ordinals retained across every native
    /// reducer row, rather than only the depth of one selected chain.
    pub max_elimination_pivot_dependency_entries: usize,
    pub max_source_combination_terms: usize,
    pub max_replay_exact_operations: usize,
    /// Maximum `i64` power cells simultaneously retained by the concrete
    /// anchor comparison outside the independently limited anchored rule.
    pub max_anchor_bridge_integral_key_power_cells: usize,
}

impl Default for ParametricRuleLimits {
    fn default() -> Self {
        Self {
            indexed_algebra: IndexedAlgebraLimits::default(),
            anchored: AnchoredRuleLimits::default(),
            max_source_rows: 65_536,
            max_shift_columns: 1_000_000,
            max_augmented_columns: 2_000_000,
            max_input_nonzero_entries: 16_000_000,
            max_index_coordinate_cells: 64_000_000,
            max_ordering_key_coordinate_cells: 128_000_000,
            max_domain_bound_endpoint_cells: 8_192,
            max_sector_mask_cells: 4_096,
            max_guard_provenance_index_cells: 64_000_000,
            max_native_decomposition_nonzero_entries: 64_000_000,
            max_rule_guards: 1_000_000,
            max_guard_origins: 4_000_000,
            max_guard_provenance_sources: 4_000_000,
            max_elimination_pivots: 65_536,
            max_elimination_pivot_dependency_entries: 64_000_000,
            max_source_combination_terms: 65_536,
            max_replay_exact_operations: 100_000_000,
            max_anchor_bridge_integral_key_power_cells: 64_000_000,
        }
    }
}
