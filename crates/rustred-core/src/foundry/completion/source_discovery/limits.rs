/// Resource policy for one ordinary-source incidence index and nomination.
///
/// Exhausting any limit is a typed research-budget result.  It never means
/// that the declared translated module has no rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SourceDiscoveryLimits {
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
}

impl Default for SourceDiscoveryLimits {
    fn default() -> Self {
        Self {
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
        }
    }
}
