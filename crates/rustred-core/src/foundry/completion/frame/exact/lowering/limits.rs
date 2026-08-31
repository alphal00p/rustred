use crate::foundry::parametric::ParametricRuleLimits;
use crate::identity::RelationLimits;

/// Aggregate resource policy for lowering one replayed exact circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactCircuitLoweringLimits {
    pub(crate) relation: RelationLimits,
    pub(crate) parametric: ParametricRuleLimits,
    pub(crate) max_selected_source_rows: usize,
    pub(crate) max_selected_source_terms: usize,
    pub(crate) max_guard_origins: usize,
    pub(crate) max_guard_condition_sources: usize,
    pub(crate) max_guard_provenance_coordinate_cells: usize,
}

impl Default for ExactCircuitLoweringLimits {
    fn default() -> Self {
        Self {
            relation: RelationLimits::default(),
            parametric: ParametricRuleLimits::default(),
            max_selected_source_rows: 65_536,
            max_selected_source_terms: 16_000_000,
            max_guard_origins: 4_000_000,
            max_guard_condition_sources: 4_000_000,
            max_guard_provenance_coordinate_cells: 64_000_000,
        }
    }
}
