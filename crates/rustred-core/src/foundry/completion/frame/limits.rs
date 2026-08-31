use crate::identity::TranslatedSourceLimits;

/// Resource limits shared by physical-frame construction shells.
///
/// The one-sided chart shell uses the embedded translation and chart policy
/// before common assembly. A selected shell consumes a batch whose exact
/// Symbolica work was already bounded at translation time, ignores the three
/// chart-only caps, and applies every common frame-owned Rust metadata cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PhysicalFrameLimits {
    pub(crate) translated_sources: TranslatedSourceLimits,
    pub(crate) max_arity: usize,
    pub(crate) max_degree: usize,
    pub(crate) max_offsets: usize,
    pub(crate) max_offset_coordinate_cells: usize,
    pub(crate) max_source_instances: usize,
    pub(crate) max_physical_columns: usize,
    pub(crate) max_physical_column_coordinate_cells: usize,
    pub(crate) max_physical_entries: usize,
    pub(crate) max_csr_row_offsets: usize,
}

impl Default for PhysicalFrameLimits {
    fn default() -> Self {
        Self {
            translated_sources: TranslatedSourceLimits::default(),
            max_arity: 4_096,
            max_degree: 64,
            max_offsets: 65_536,
            max_offset_coordinate_cells: 1_048_576,
            max_source_instances: 1_000_000,
            max_physical_columns: 4_000_000,
            max_physical_column_coordinate_cells: 64_000_000,
            max_physical_entries: 16_000_000,
            max_csr_row_offsets: 1_000_001,
        }
    }
}
