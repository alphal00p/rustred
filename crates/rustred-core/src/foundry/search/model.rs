use crate::family::IntegralKey;
use crate::identity::IntegralShift;

/// Complete bounded L1 offset diamond that remains in one exact sector.
///
/// Offsets are immutable, unique, and ordered lexicographically. Every
/// retained shifted point is representable by `i64` and has the same active
/// (`n >= 1`) slots as the owned anchor.
#[derive(Debug, PartialEq, Eq)]
pub struct SectorSearchDiamond {
    pub(super) anchor: IntegralKey,
    pub(super) depth: usize,
    pub(super) offsets: Box<[IntegralShift]>,
}

impl SectorSearchDiamond {
    pub fn anchor(&self) -> &IntegralKey {
        &self.anchor
    }

    pub const fn depth(&self) -> usize {
        self.depth
    }

    pub fn offsets(&self) -> &[IntegralShift] {
        &self.offsets
    }

    pub fn offset_count(&self) -> usize {
        self.offsets.len()
    }
}
