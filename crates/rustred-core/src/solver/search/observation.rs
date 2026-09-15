//! Cheap search milestones, with no formatting or algebra in the observer API.

use super::{DiscoveryStats, Integral};

/// Diagnostic phase boundaries. No event is emitted per matrix row, and the
/// no-op observer adds no clock reads, synchronization or expression copies.
/// Discovery includes source instantiation, modular evaluation and GPLU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchEvent<const N: usize> {
    /// Emitted at each new depth and geometrically spaced seed milestones.
    DiscoveryProgress {
        depth: u32,
        seeds: usize,
        rows: usize,
        discovery: Option<DiscoveryStats>,
    },
    ExactStarted {
        pivot: Integral<N>,
        trace_rows: usize,
        discovery: DiscoveryStats,
    },
    CanonicalizationStarted {
        terms: usize,
        direct_hit: bool,
    },
}
