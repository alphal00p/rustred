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
    pub(crate) max_incidence_visits: usize,
    pub(crate) max_candidate_coordinate_cells: usize,
    pub(crate) max_raw_requests: usize,
    pub(crate) max_unique_requests: usize,
    pub(crate) max_existing_requests: usize,
    pub(crate) max_residual_candidates: usize,
    pub(crate) max_residual_source_terms: usize,
    pub(crate) max_residual_support_coordinate_cells: usize,
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
