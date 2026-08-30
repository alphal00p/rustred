use crate::algebra::IndexedAlgebraLimits;

/// Resource policy for one bounded concrete reachability discovery.
///
/// The count limits bound retained Rust metadata and exact Symbolica
/// specializations separately. They do not turn a finite census into a proof
/// about keys outside the submitted roots' reachable concrete graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReachabilityLimits {
    pub indexed_algebra: IndexedAlgebraLimits,
    pub max_rule_cells: usize,
    pub max_roots: usize,
    pub max_discovered_nodes: usize,
    pub max_pending_nodes: usize,
    /// Aggregate retained `i64`/`u64` lattice coordinates in canonical roots,
    /// scheduled/visited keys and complexity keys, concrete assignments, and
    /// raw-plus-canonical dependency children.
    pub max_retained_lattice_coordinate_cells: usize,
    pub max_dependency_edges: usize,
    pub max_rule_cell_probes: usize,
    pub max_guard_specializations: usize,
    pub max_coefficient_specializations: usize,
}

impl Default for ReachabilityLimits {
    fn default() -> Self {
        Self {
            indexed_algebra: IndexedAlgebraLimits::default(),
            max_rule_cells: 100_000,
            max_roots: 1_000_000,
            max_discovered_nodes: 1_000_000,
            max_pending_nodes: 1_000_000,
            max_retained_lattice_coordinate_cells: 128_000_000,
            max_dependency_edges: 16_000_000,
            max_rule_cell_probes: 16_000_000,
            max_guard_specializations: 64_000_000,
            max_coefficient_specializations: 64_000_000,
        }
    }
}
