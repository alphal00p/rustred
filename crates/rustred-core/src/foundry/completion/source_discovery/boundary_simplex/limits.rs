/// Aggregate retained-work envelope for one frozen boundary-face plan.
///
/// No limit truncates a combinatorial design. The planner either returns all
/// requested parent boxes, faces, finite assignments, and simplex offsets or
/// rejects the entire request before task construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BoundarySimplexLimits {
    pub(crate) max_scopes: usize,
    pub(crate) max_aggregate_scope_key_bytes: usize,
    pub(crate) max_arity: usize,
    pub(crate) max_input_boxes: usize,
    pub(crate) max_input_box_coordinate_cells: usize,
    /// Zero for `n<=1`; otherwise deterministic `n*ceil(log2(n))` work.
    pub(crate) max_canonical_sort_work: usize,
    pub(crate) max_selected_parent_boxes: usize,
    pub(crate) max_selected_parent_coordinate_cells: usize,
    pub(crate) max_faces_per_parent: usize,
    pub(crate) max_boundary_faces: usize,
    /// Pinned plus remaining ambient-axis ordinals retained by every face.
    pub(crate) max_boundary_face_axis_cells: usize,
    /// Conservative checked work envelope for lexicographic subset unranking.
    pub(crate) max_subset_unrank_work: usize,
    pub(crate) max_finite_assignments_per_parent: usize,
    /// Sum of finite assignments over parents before face multiplication.
    pub(crate) max_parent_finite_assignments: usize,
    /// Sum of face × finite-assignment pairs before simplex multiplication.
    pub(crate) max_face_finite_assignments: usize,
    /// Peak entries retained by parent flattening, face storage, and the two
    /// ordered active-assignment frontiers.
    pub(crate) max_scheduler_workspace_entries: usize,
    /// Exact canonical parent/face/frontier visits performed by construction.
    pub(crate) max_scheduler_visits: usize,
    pub(crate) max_interior_margin: u64,
    pub(crate) max_polynomial_degree_ceiling: usize,
    pub(crate) max_simplex_samples: usize,
    pub(crate) max_simplex_coordinate_cells: usize,
    pub(crate) max_tasks: usize,
    /// Retained lattice-target plus target-shift coordinates in every task.
    pub(crate) max_task_coordinate_cells: usize,
}

impl Default for BoundarySimplexLimits {
    fn default() -> Self {
        Self {
            max_scopes: 4_096,
            max_aggregate_scope_key_bytes: 4_194_304,
            max_arity: 4_096,
            max_input_boxes: 1_048_576,
            max_input_box_coordinate_cells: 67_108_864,
            max_canonical_sort_work: 134_217_728,
            max_selected_parent_boxes: 1_048_576,
            max_selected_parent_coordinate_cells: 67_108_864,
            max_faces_per_parent: 4_194_304,
            max_boundary_faces: 4_194_304,
            max_boundary_face_axis_cells: 134_217_728,
            max_subset_unrank_work: 536_870_912,
            max_finite_assignments_per_parent: 4_194_304,
            max_parent_finite_assignments: 4_194_304,
            max_face_finite_assignments: 4_194_304,
            max_scheduler_workspace_entries: 12_582_912,
            max_scheduler_visits: 67_108_864,
            max_interior_margin: 1_048_576,
            max_polynomial_degree_ceiling: 1_024,
            max_simplex_samples: 2_097_152,
            max_simplex_coordinate_cells: 67_108_864,
            max_tasks: 4_194_304,
            max_task_coordinate_cells: 536_870_912,
        }
    }
}
