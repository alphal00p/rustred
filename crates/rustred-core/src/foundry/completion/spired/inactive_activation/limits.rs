use crate::algebra::IndexedAlgebraLimits;

/// Resource envelope for one exact inactive-line activation decomposition.
///
/// The decomposition is structural scheduling evidence only. In particular,
/// these limits do not authorize a column, rule, owner, or closure claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredInactiveActivationLimits {
    pub(crate) max_arity: usize,
    pub(crate) max_affected_axes: usize,
    pub(crate) max_activation_slices: usize,
    pub(crate) max_retained_domains: usize,
    pub(crate) max_retained_domain_bound_cells: usize,
}

impl Default for SpiredInactiveActivationLimits {
    fn default() -> Self {
        Self {
            max_arity: 4_096,
            max_affected_axes: 4_096,
            max_activation_slices: 1_048_576,
            max_retained_domains: 1_048_577,
            max_retained_domain_bound_cells: 16_777_216,
        }
    }
}

/// Aggregate resource envelope for joining exact replay coefficients to
/// inactive-line activation geometry.
///
/// `geometry` applies independently to each retained physical term.  The
/// remaining limits bound aggregate work and retained output for the complete
/// candidate so a long replay cannot evade the same policy through many small
/// terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredInactiveActivationAnalysisLimits {
    pub(crate) geometry: SpiredInactiveActivationLimits,
    pub(crate) indexed_algebra: IndexedAlgebraLimits,
    pub(crate) max_candidate_terms: usize,
    pub(crate) max_face_specializations: usize,
    pub(crate) max_surviving_face_sources: usize,
    pub(crate) max_unique_surviving_faces: usize,
    pub(crate) max_partition_face_values: usize,
    pub(crate) max_application_cell_product_states: usize,
    pub(crate) max_application_cells: usize,
    pub(crate) max_application_cell_bound_cells: usize,
    pub(crate) max_pruned_physical_column_references: usize,
}

impl Default for SpiredInactiveActivationAnalysisLimits {
    fn default() -> Self {
        Self {
            geometry: SpiredInactiveActivationLimits::default(),
            indexed_algebra: IndexedAlgebraLimits::default(),
            max_candidate_terms: 4_000_000,
            max_face_specializations: 16_000_000,
            max_surviving_face_sources: 16_000_000,
            max_unique_surviving_faces: 1_048_576,
            max_partition_face_values: 1_048_576,
            max_application_cell_product_states: 67_108_864,
            max_application_cells: 1_048_576,
            max_application_cell_bound_cells: 33_554_432,
            max_pruned_physical_column_references: 67_108_864,
        }
    }
}
