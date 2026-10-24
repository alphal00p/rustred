use super::CompletionGeometryError;

/// One nonrecursive, exhaustive finite-axis refinement. Shared by affine
/// box emptiness and predicate-cover consistency; never a sampling bound.
pub(crate) const MAX_BOUNDED_AXIS_FACES: usize = 8;

/// Hard resource limits for exact sector-lattice coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CompletionGeometryLimits {
    pub(crate) diagnostics_enabled: bool,
    pub(crate) max_arity: usize,
    pub(crate) max_requested_generators: usize,
    pub(crate) max_requested_generator_coordinate_cells: usize,
    pub(crate) max_minimal_generators: usize,
    pub(crate) max_requested_boxes: usize,
    pub(crate) max_requested_box_coordinate_cells: usize,
    pub(crate) max_uncovered_boxes: usize,
    pub(crate) max_uncovered_box_coordinate_cells: usize,
    pub(crate) max_split_operations: usize,
}

impl Default for CompletionGeometryLimits {
    fn default() -> Self {
        Self {
            diagnostics_enabled: false,
            max_arity: 4_096,
            max_requested_generators: 65_536,
            max_requested_generator_coordinate_cells: 1_048_576,
            max_minimal_generators: 65_536,
            max_requested_boxes: 65_536,
            max_requested_box_coordinate_cells: 1_048_576,
            max_uncovered_boxes: 262_144,
            max_uncovered_box_coordinate_cells: 4_194_304,
            max_split_operations: 16_777_216,
        }
    }
}

impl CompletionGeometryLimits {
    pub(crate) fn with_diagnostics(mut self) -> Self {
        self.diagnostics_enabled = true;
        self
    }
}

impl CompletionGeometryLimits {
    pub(super) fn check_count(
        self,
        resource: &'static str,
        requested: usize,
        limit: usize,
    ) -> Result<(), CompletionGeometryError> {
        symbolica::utils::resources::checkpoint()?;
        if !self.diagnostics_enabled || requested <= limit {
            Ok(())
        } else {
            Err(CompletionGeometryError::ResourceLimit {
                resource,
                requested,
                limit,
            })
        }
    }
}
