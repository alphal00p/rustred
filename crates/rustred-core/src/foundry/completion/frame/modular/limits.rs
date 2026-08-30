/// Resource policy for one bounded modular sample and target query.
///
/// `max_reducer_dense_cells` admits the dense work rectangle
/// `rows * projected_columns`.  The separate total-fill bound admits the
/// sharper worst case `r * (rows + projected_columns)`, with
/// `r = min(rows, projected_columns)`, for Symbolica's coefficient-free L
/// pattern plus coefficient-valued U.  The fill multiple is the experimental
/// `(nnz(L) + nnz(U)) / nnz(input)` kill gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ModularKernelLimits {
    pub(crate) max_point_coordinates: usize,
    pub(crate) max_matrix_rows: usize,
    pub(crate) max_matrix_columns: usize,
    pub(crate) max_source_conditions: usize,
    pub(crate) max_structural_entries: usize,
    pub(crate) max_retained_entries: usize,
    pub(crate) max_csr_row_offsets: usize,
    pub(crate) max_projected_columns: usize,
    pub(crate) max_projected_entries: usize,
    pub(crate) max_reducer_dense_cells: usize,
    pub(crate) max_reducer_total_fill_entries: usize,
    pub(crate) max_reducer_fill_multiple: usize,
}

impl Default for ModularKernelLimits {
    fn default() -> Self {
        Self {
            max_point_coordinates: 8_192,
            max_matrix_rows: 1_000_000,
            max_matrix_columns: 4_000_000,
            max_source_conditions: 16_000_000,
            max_structural_entries: 16_000_000,
            max_retained_entries: 16_000_000,
            max_csr_row_offsets: 1_000_001,
            max_projected_columns: 1_000_000,
            max_projected_entries: 16_000_000,
            max_reducer_dense_cells: 64_000_000,
            max_reducer_total_fill_entries: 128_000_000,
            max_reducer_fill_multiple: 20,
        }
    }
}
