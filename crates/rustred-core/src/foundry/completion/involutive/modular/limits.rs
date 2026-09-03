/// Retained-shape and cumulative-work envelope for modular coefficient
/// guidance.  These limits are deliberately independent of exact Janet work:
/// exhausting a guide budget rejects that probe and leaves exact authority
/// unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ModularGuideLimits {
    pub(super) max_nodes: usize,
    pub(super) max_exact_leaves: usize,
    pub(super) max_physical_deltas: usize,
    pub(super) max_physical_delta_coordinate_cells: usize,
    pub(super) max_absolute_physical_translation: u64,
    pub(super) max_probe_point_coordinates: usize,
    pub(super) max_probe_retained_point_coordinate_cells: usize,
    pub(super) max_probe_accumulated_deltas: usize,
    pub(super) max_probe_accumulated_delta_coordinate_cells: usize,
    pub(super) max_probe_translated_points: usize,
    pub(super) max_probe_translated_point_coordinate_cells: usize,
    pub(super) max_probe_cached_values: usize,
    pub(super) max_probe_batch_images: usize,
    pub(super) max_probe_queries: usize,
    pub(super) max_probe_delta_compositions: usize,
    pub(super) max_probe_delta_coordinate_operations: usize,
    pub(super) max_probe_evaluation_steps: usize,
    pub(super) max_probe_evaluation_depth: usize,
    pub(super) max_probe_exact_leaf_evaluations: usize,
    pub(super) max_probe_exact_leaf_terms_evaluated: usize,
    pub(super) max_probe_exact_leaf_exponent_cells_evaluated: usize,
}

impl Default for ModularGuideLimits {
    fn default() -> Self {
        Self {
            max_nodes: 1_000_000,
            max_exact_leaves: 1_000_000,
            max_physical_deltas: 1_000_000,
            max_physical_delta_coordinate_cells: 64_000_000,
            max_absolute_physical_translation: i64::MAX as u64,
            max_probe_point_coordinates: 8_192,
            max_probe_retained_point_coordinate_cells: 24_576,
            max_probe_accumulated_deltas: 1_000_000,
            max_probe_accumulated_delta_coordinate_cells: 64_000_000,
            max_probe_translated_points: 1_000_000,
            max_probe_translated_point_coordinate_cells: 8_192_000_000,
            max_probe_cached_values: 16_000_000,
            max_probe_batch_images: 16_000_000,
            max_probe_queries: 100_000_000,
            max_probe_delta_compositions: 100_000_000,
            max_probe_delta_coordinate_operations: 8_000_000_000,
            max_probe_evaluation_steps: 100_000_000,
            // Evaluation is recursive over an acyclic, earlier-node-only DAG.
            // This hard cap rejects adversarial depth before the Rust stack is
            // exposed to an unbounded caller-shaped chain.
            max_probe_evaluation_depth: 256,
            max_probe_exact_leaf_evaluations: 16_000_000,
            max_probe_exact_leaf_terms_evaluated: 1_000_000_000,
            max_probe_exact_leaf_exponent_cells_evaluated: 16_000_000_000,
        }
    }
}
