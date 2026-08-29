/// Structural limits for one lazy proper-subsector discovery plan.
///
/// These counts bound Rust-owned metadata and described work. They are not a
/// physical RSS estimate for Symbolica/GMP and do not authorize collecting
/// all described cells in memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParametricDependencyLimits {
    pub max_rule_terms: usize,
    pub max_partition_coordinate_cells: usize,
    pub max_described_target_sector_cells: usize,
    pub max_proper_subsector_obligations: usize,
    pub max_per_obligation_materialization_coordinate_cells: usize,
}

impl Default for ParametricDependencyLimits {
    fn default() -> Self {
        Self {
            max_rule_terms: 16_000_000,
            max_partition_coordinate_cells: 256_000_000,
            max_described_target_sector_cells: 256_000_000,
            max_proper_subsector_obligations: 256_000_000,
            max_per_obligation_materialization_coordinate_cells: 65_536,
        }
    }
}
