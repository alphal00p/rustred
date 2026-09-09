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
    /// Immutable structural row plans retained for replay by independent
    /// probes and source-exclusion branches. This is intentionally smaller
    /// than the theoretical scheduler universe: reaching it is a resumable
    /// search pause, never a closure decision.
    pub(crate) max_retained_prepared_rows: usize,
    /// Exact number of structural term roles retained in the flat target tape.
    /// This cap is checked together with the row cap before either flat arena
    /// reserves storage or the row is structurally classified.
    pub(crate) max_retained_prepared_term_roles: usize,
    /// Aggregate successfully streamed rows across the root lane and every
    /// source-exclusion alternative.  Unlike the nested modular row cap this
    /// cannot be reset by opening a fresh alternative reducer.
    pub(crate) max_total_streamed_rows: usize,
    /// Exact-lift attempts on distinct modular supports.  A modular hit is
    /// proposal evidence only, so reaching this bound returns an explicitly
    /// inconclusive alternative-search outcome.
    pub(crate) max_distinct_compact_lift_attempts: usize,
    /// Root plus canonical source-exclusion branches retained by the bounded
    /// alternative search.
    pub(crate) max_search_branches: usize,
    /// Maximum exclusion-set cardinality for one alternative branch.
    pub(crate) max_excluded_requests_per_branch: usize,
    /// Aggregate request entries retained across all canonical exclusion
    /// sets.  Branches are intentionally few, but their supports need not be.
    pub(crate) max_retained_exclusion_requests: usize,
    /// Aggregate retained shift-coordinate cells across all exclusion sets.
    pub(crate) max_retained_exclusion_coordinate_cells: usize,
    /// Aggregate request entries retained only to deduplicate already
    /// rejected compact supports across sibling exclusion branches.
    pub(crate) max_retained_rejected_support_requests: usize,
    /// Aggregate shift-coordinate cells retained by rejected supports.
    pub(crate) max_retained_rejected_support_coordinate_cells: usize,
    /// Aggregate rows streamed by non-root canonical source-exclusion
    /// branches after an earlier support proved exactly unusable.
    pub(crate) max_alternative_streamed_rows: usize,
    /// Rows inspected in one source-exclusion branch's canonical chronology
    /// after its first modular target pivot. The live reducer is retained
    /// across this window so later GPLU roots can be compared without replaying
    /// the prefix. Zero preserves strict first-hit selection.
    pub(crate) max_post_hit_streamed_rows: usize,
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
            max_retained_prepared_rows: 4_194_304,
            max_retained_prepared_term_roles: 67_108_864,
            max_total_streamed_rows: 67_108_864,
            max_distinct_compact_lift_attempts: 4,
            max_search_branches: 16,
            max_excluded_requests_per_branch: 64,
            max_retained_exclusion_requests: 512,
            max_retained_exclusion_coordinate_cells: 32_768,
            max_retained_rejected_support_requests: 256,
            max_retained_rejected_support_coordinate_cells: 16_384,
            max_alternative_streamed_rows: 4_096,
            max_post_hit_streamed_rows: 64,
        }
    }
}
