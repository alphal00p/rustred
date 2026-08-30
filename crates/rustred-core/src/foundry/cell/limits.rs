use crate::algebra::IndexedAlgebraLimits;
use crate::identity::RelationLimits;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleCellLimits {
    pub indexed_algebra: IndexedAlgebraLimits,
    pub relation: RelationLimits,
    pub max_source_views: usize,
    pub max_fixed_restrictions: usize,
    pub max_pruned_terms: usize,
    pub max_retained_terms: usize,
    pub max_guards: usize,
    pub max_projected_source_terms: usize,
    pub max_projection_group_routes: usize,
    pub max_projection_zero_sectors: usize,
}

impl Default for RuleCellLimits {
    fn default() -> Self {
        Self {
            indexed_algebra: IndexedAlgebraLimits::default(),
            relation: RelationLimits::default(),
            max_source_views: 1_000_000,
            max_fixed_restrictions: 4_096,
            max_pruned_terms: 1_000_000,
            max_retained_terms: 1_000_000,
            max_guards: 1_000_000,
            max_projected_source_terms: 16_000_000,
            max_projection_group_routes: 16_000_000,
            max_projection_zero_sectors: 1_000_000,
        }
    }
}
