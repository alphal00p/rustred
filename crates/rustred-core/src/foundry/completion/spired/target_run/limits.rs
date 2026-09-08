use crate::foundry::completion::source_discovery::ExactRuleCellPromotionLimits;

use super::super::{SignedL1ScheduleLimits, SpiredCompactLiftLimits, SpiredStreamingLimits};

/// Aggregate policy for one scheduler-driven target/probe lane.
///
/// Nested policies continue to bound their own allocations and native work.
/// The additional counters cap work accumulated across depth shells and
/// materialized chunks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredTargetRunLimits {
    pub(crate) schedule: SignedL1ScheduleLimits,
    pub(crate) streaming: SpiredStreamingLimits,
    pub(crate) compact_lift: SpiredCompactLiftLimits,
    pub(crate) promotion: ExactRuleCellPromotionLimits,
    pub(crate) max_depth_inclusive: usize,
    pub(crate) request_chunk_size: usize,
    pub(crate) max_chunks: usize,
    pub(crate) max_total_scheduled_requests: usize,
    pub(crate) max_total_request_coordinate_cells: usize,
}

impl Default for SpiredTargetRunLimits {
    fn default() -> Self {
        Self {
            schedule: SignedL1ScheduleLimits::default(),
            streaming: SpiredStreamingLimits::default(),
            compact_lift: SpiredCompactLiftLimits::default(),
            promotion: ExactRuleCellPromotionLimits::default(),
            max_depth_inclusive: 64,
            request_chunk_size: 4_096,
            max_chunks: 1_048_576,
            max_total_scheduled_requests: 67_108_864,
            max_total_request_coordinate_cells: 1_073_741_824,
        }
    }
}
