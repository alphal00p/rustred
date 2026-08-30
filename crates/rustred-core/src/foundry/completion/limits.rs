/// Hard resource limits for exact sector-lattice coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CompletionGeometryLimits {
    pub(crate) max_arity: usize,
    pub(crate) max_requested_generators: usize,
    pub(crate) max_requested_generator_coordinate_cells: usize,
    pub(crate) max_minimal_generators: usize,
    pub(crate) max_uncovered_boxes: usize,
    pub(crate) max_uncovered_box_coordinate_cells: usize,
    pub(crate) max_split_operations: usize,
}

impl Default for CompletionGeometryLimits {
    fn default() -> Self {
        Self {
            max_arity: 4_096,
            max_requested_generators: 65_536,
            max_requested_generator_coordinate_cells: 1_048_576,
            max_minimal_generators: 65_536,
            max_uncovered_boxes: 262_144,
            max_uncovered_box_coordinate_cells: 4_194_304,
            max_split_operations: 16_777_216,
        }
    }
}
