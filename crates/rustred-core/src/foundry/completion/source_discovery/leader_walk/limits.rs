/// Global retained-work envelope for one frozen maximal-orthant leader walk.
///
/// Every limit is aggregate over all supplied scopes and both waves.  The
/// planner never truncates to a cap: it either returns the complete selected
/// census or a typed error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LeaderWalkLimits {
    pub(crate) max_scopes: usize,
    pub(crate) max_aggregate_scope_key_bytes: usize,
    pub(crate) max_arity: usize,
    pub(crate) max_input_boxes: usize,
    pub(crate) max_input_box_coordinate_cells: usize,
    pub(crate) max_selected_boxes: usize,
    pub(crate) max_selected_box_coordinate_cells: usize,
    /// One retained position for every unbounded axis of a selected box.
    pub(crate) max_selected_free_axis_cells: usize,
    /// Aggregate over the lower-corner and depth-one waves.
    pub(crate) max_tasks: usize,
    /// Retained leader plus target-shift coordinates across both waves.
    pub(crate) max_task_coordinate_cells: usize,
}

impl Default for LeaderWalkLimits {
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
            max_tasks: 2_097_152,
            max_task_coordinate_cells: 268_435_456,
        }
    }
}
