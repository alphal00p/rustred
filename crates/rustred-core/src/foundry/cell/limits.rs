use crate::algebra::{IndexedAlgebraLimits, IndexedGuardLimits};
use crate::identity::RelationLimits;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleCellLimits {
    /// Enforce operational count templates only for explicit diagnostics.
    pub diagnostics_enabled: bool,
    pub indexed_algebra: IndexedAlgebraLimits,
    pub guard_algebra: IndexedGuardLimits,
    pub relation: RelationLimits,
    pub max_source_views: usize,
    pub max_fixed_restrictions: usize,
    pub max_pruned_terms: usize,
    pub max_retained_terms: usize,
    pub max_guards: usize,
    pub max_guard_split_coordinate_cells: usize,
    pub max_projected_source_terms: usize,
    pub max_projection_group_routes: usize,
    pub max_projection_zero_sectors: usize,
}

impl Default for RuleCellLimits {
    fn default() -> Self {
        Self {
            diagnostics_enabled: false,
            indexed_algebra: IndexedAlgebraLimits::default(),
            guard_algebra: IndexedGuardLimits::default(),
            relation: RelationLimits::default(),
            max_source_views: 1_000_000,
            max_fixed_restrictions: 4_096,
            max_pruned_terms: 1_000_000,
            max_retained_terms: 1_000_000,
            max_guards: 1_000_000,
            max_guard_split_coordinate_cells: 12_288,
            max_projected_source_terms: 16_000_000,
            max_projection_group_routes: 16_000_000,
            max_projection_zero_sectors: 1_000_000,
        }
    }
}

impl RuleCellLimits {
    pub fn with_diagnostics(mut self) -> Self {
        self.diagnostics_enabled = true;
        self.indexed_algebra = self.indexed_algebra.with_diagnostics();
        self.guard_algebra = self.guard_algebra.with_diagnostics();
        self.relation.arithmetic = self.relation.arithmetic.with_diagnostics();
        self.relation.identity_conditions = self.relation.identity_conditions.with_diagnostics();
        self
    }
}
