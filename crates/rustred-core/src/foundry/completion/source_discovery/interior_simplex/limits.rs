/// Aggregate retained-work envelope for one frozen interior-simplex plan.
///
/// No limit truncates the requested simplex.  The planner either returns the
/// complete design for every selected box or rejects the entire request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InteriorSimplexLimits {
    pub(crate) max_scopes: usize,
    pub(crate) max_aggregate_scope_key_bytes: usize,
    pub(crate) max_arity: usize,
    pub(crate) max_input_boxes: usize,
    pub(crate) max_input_box_coordinate_cells: usize,
    pub(crate) max_selected_boxes: usize,
    pub(crate) max_selected_box_coordinate_cells: usize,
    pub(crate) max_selected_free_axis_cells: usize,
    pub(crate) max_interior_margin: u64,
    pub(crate) max_polynomial_degree_ceiling: usize,
    pub(crate) max_simplex_samples: usize,
    pub(crate) max_simplex_coordinate_cells: usize,
    pub(crate) max_tasks: usize,
    /// Retained lattice-target plus target-shift coordinates in every task.
    pub(crate) max_task_coordinate_cells: usize,
}

impl Default for InteriorSimplexLimits {
    fn default() -> Self {
        Self {
            max_scopes: 4_096,
            max_aggregate_scope_key_bytes: 4_194_304,
            max_arity: 4_096,
            max_input_boxes: 1_048_576,
            max_input_box_coordinate_cells: 67_108_864,
            max_selected_boxes: 1_048_576,
            max_selected_box_coordinate_cells: 67_108_864,
            max_selected_free_axis_cells: 67_108_864,
            max_interior_margin: 1_048_576,
            max_polynomial_degree_ceiling: 1_024,
            max_simplex_samples: 2_097_152,
            max_simplex_coordinate_cells: 67_108_864,
            max_tasks: 4_194_304,
            max_task_coordinate_cells: 536_870_912,
        }
    }
}
