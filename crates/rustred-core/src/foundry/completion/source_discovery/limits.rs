use crate::identity::TranslatedSourceLimits;

/// Resource policy for one ordinary-source incidence index and nomination.
///
/// Exhausting any limit is a typed research-budget result.  It never means
/// that the declared translated module has no rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SourceDiscoveryLimits {
    /// Exact selected-source translation policy used by residual pairing.
    pub(crate) translation: TranslatedSourceLimits,
    pub(crate) max_arity: usize,
    pub(crate) max_source_rows: usize,
    pub(crate) max_source_term_occurrences: usize,
    pub(crate) max_distinct_source_shifts: usize,
    pub(crate) max_obstruction_support: usize,
    /// Raw integral shifts in the union support of one proposal-only bounded
    /// obstruction block.
    pub(crate) max_union_block_entries: usize,
    pub(crate) max_union_support_entries: usize,
    pub(crate) max_union_support_coordinate_cells: usize,
    /// Dense finite-field direction coefficients retained across that union.
    pub(crate) max_union_support_coefficient_cells: usize,
    pub(crate) max_union_incidence_visits: usize,
    pub(crate) max_union_raw_requests: usize,
    pub(crate) max_union_unique_requests: usize,
    pub(crate) max_union_request_coordinate_cells: usize,
    pub(crate) max_union_subset_comparisons: usize,
    /// Conservative comparator-coordinate work for support, request, and
    /// existing-request canonicalization plus exclusion searches.
    /// Logical comparison-coordinate reservation derived from input sizes.
    /// This is not an exact comparator-call census for the standard-library
    /// unstable sort implementation.
    pub(crate) max_union_canonicalization_logical_work_reservation: usize,
    /// Probe-local complete modular row-evaluation cache. Rows are keyed by
    /// exact translated-source requests; value cells include explicit zeros.
    pub(crate) max_row_cache_rows: usize,
    pub(crate) max_row_cache_value_cells: usize,
    pub(crate) max_row_cache_request_coordinate_cells: usize,
    pub(crate) max_row_cache_lookup_comparisons: usize,
    pub(crate) max_row_cache_physical_evaluations: usize,
    /// Existing sorted entries shifted by cache insertions.
    pub(crate) max_row_cache_insertion_moves: usize,
    /// Proposal-only block signature construction and selection work.
    pub(crate) max_block_signature_candidates: usize,
    pub(crate) max_block_signature_cells: usize,
    pub(crate) max_block_signature_pairing_operations: usize,
    pub(crate) max_block_candidate_classifications: usize,
    pub(crate) max_block_primary_crosscheck_comparisons: usize,
    pub(crate) max_block_selection_candidates: usize,
    pub(crate) max_block_selection_comparisons: usize,
    pub(crate) max_block_signature_rank_operations: usize,
    pub(crate) max_block_signature_rank_cells: usize,
    pub(crate) max_block_selected_requests: usize,
    pub(crate) max_incidence_visits: usize,
    pub(crate) max_candidate_coordinate_cells: usize,
    pub(crate) max_raw_requests: usize,
    pub(crate) max_unique_requests: usize,
    pub(crate) max_existing_requests: usize,
    pub(crate) max_residual_candidates: usize,
    pub(crate) max_residual_source_terms: usize,
    pub(crate) max_residual_support_coordinate_cells: usize,
    /// Nonzero-residual rows admitted to non-authoritative proposal scoring.
    /// Empty-census authority is established before this limit is consulted.
    pub(crate) max_residual_classifications: usize,
    pub(crate) max_nonzero_residual_requests: usize,
    /// Owned request census retained by one admitted sampled dual.
    pub(crate) max_sampled_dual_requests: usize,
    pub(crate) max_sampled_dual_request_coordinate_cells: usize,
    /// Sparse checked obstruction copied from plan-local columns to raw keys.
    pub(crate) max_sampled_dual_obstruction_entries: usize,
    pub(crate) max_sampled_dual_obstruction_coordinate_cells: usize,
    /// Finite-field coordinates kept alive by the retained sample owner.
    pub(crate) max_sampled_dual_sample_coordinates: usize,
    /// Coordinate additions used to recompute the exact translated-term /
    /// raw-obstruction intersection census at admission.
    pub(crate) max_sampled_dual_pairing_coordinate_cells: usize,
    /// Aggregate ordinals in all copied modular-rank diagnostic sidecars.
    pub(crate) max_sampled_dual_diagnostic_ordinals: usize,
}

impl Default for SourceDiscoveryLimits {
    fn default() -> Self {
        Self {
            translation: TranslatedSourceLimits::default(),
            max_arity: 4_096,
            max_source_rows: 65_536,
            max_source_term_occurrences: 1_000_000,
            max_distinct_source_shifts: 1_000_000,
            max_obstruction_support: 1_000_000,
            max_union_block_entries: 4_000_000,
            max_union_support_entries: 4_000_000,
            max_union_support_coordinate_cells: 64_000_000,
            max_union_support_coefficient_cells: 16_000_000,
            max_union_incidence_visits: 64_000_000,
            max_union_raw_requests: 64_000_000,
            max_union_unique_requests: 16_000_000,
            max_union_request_coordinate_cells: 64_000_000,
            max_union_subset_comparisons: 32_000_000,
            max_union_canonicalization_logical_work_reservation: 1_000_000_000,
            max_row_cache_rows: 1_000_000,
            max_row_cache_value_cells: 16_000_000,
            max_row_cache_request_coordinate_cells: 64_000_000,
            max_row_cache_lookup_comparisons: 64_000_000,
            max_row_cache_physical_evaluations: 16_000_000,
            max_row_cache_insertion_moves: 256_000_000,
            max_block_signature_candidates: 16_000_000,
            max_block_signature_cells: 64_000_000,
            max_block_signature_pairing_operations: 64_000_000,
            max_block_candidate_classifications: 16_000_000,
            max_block_primary_crosscheck_comparisons: 32_000_000,
            max_block_selection_candidates: 16_000_000,
            max_block_selection_comparisons: 64_000_000,
            max_block_signature_rank_operations: 64_000_000,
            max_block_signature_rank_cells: 256_000_000,
            max_block_selected_requests: 32,
            max_incidence_visits: 16_000_000,
            max_candidate_coordinate_cells: 64_000_000,
            max_raw_requests: 16_000_000,
            max_unique_requests: 16_000_000,
            max_existing_requests: 1_000_000,
            max_residual_candidates: 1_000_000,
            max_residual_source_terms: 16_000_000,
            max_residual_support_coordinate_cells: 64_000_000,
            max_residual_classifications: 1_000_000,
            max_nonzero_residual_requests: 1_000_000,
            max_sampled_dual_requests: 1_000_000,
            max_sampled_dual_request_coordinate_cells: 64_000_000,
            max_sampled_dual_obstruction_entries: 1_000_000,
            max_sampled_dual_obstruction_coordinate_cells: 64_000_000,
            max_sampled_dual_sample_coordinates: 65_536,
            max_sampled_dual_pairing_coordinate_cells: 64_000_000,
            max_sampled_dual_diagnostic_ordinals: 8_000_000,
        }
    }
}
