/// Structural resource policy for one exact-sector search diamond.
///
/// These limits bound Rust-owned search metadata. They are not a physical RSS
/// estimate for later identity generation or Symbolica elimination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectorSearchLimits {
    pub max_depth: usize,
    pub max_offsets: usize,
    pub max_offset_coordinate_cells: usize,
}

impl Default for SectorSearchLimits {
    fn default() -> Self {
        Self {
            max_depth: 64,
            max_offsets: 1_000_000,
            max_offset_coordinate_cells: 16_000_000,
        }
    }
}
