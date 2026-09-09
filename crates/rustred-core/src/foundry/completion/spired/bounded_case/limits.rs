use crate::foundry::completion::stratum::StratumRegistryLimits;

/// Resource envelope for one finite-depth source-safe case partition.
///
/// These ceilings bound structural inspection and retained proposal geometry.
/// Raising them grants no rule, owner, terminal, or closure authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredBoundedCaseEnvelopeLimits {
    pub(crate) strata: StratumRegistryLimits,
    pub(crate) max_arity: usize,
    pub(crate) max_source_rows: usize,
    pub(crate) max_source_terms: usize,
    pub(crate) max_source_coordinate_cells: usize,
    pub(crate) max_source_depth: usize,
    pub(crate) max_boundary_values_per_axis: usize,
    pub(crate) max_equality_faces: usize,
    pub(crate) max_retained_domains: usize,
    pub(crate) max_retained_domain_bound_cells: usize,
}

impl Default for SpiredBoundedCaseEnvelopeLimits {
    fn default() -> Self {
        Self {
            strata: StratumRegistryLimits::default(),
            max_arity: 4_096,
            max_source_rows: 65_536,
            max_source_terms: 4_194_304,
            max_source_coordinate_cells: 67_108_864,
            max_source_depth: 1_048_576,
            max_boundary_values_per_axis: 1_048_576,
            max_equality_faces: 4_194_304,
            max_retained_domains: 4_194_305,
            max_retained_domain_bound_cells: 67_108_864,
        }
    }
}
