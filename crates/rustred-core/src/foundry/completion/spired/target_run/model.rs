use crate::foundry::completion::source_discovery::ExactRuleCellPromotionDisposition;

use super::super::SpiredCompactLift;

/// Exact progress census for one target/probe lane.
///
/// Scheduled requests count every request materialized in an admitted chunk;
/// streamed rows count only requests actually consumed before a hit or error.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SpiredTargetRunCensus {
    shells_opened: usize,
    shells_completed: usize,
    chunks_materialized: usize,
    scheduled_requests: usize,
    scheduled_request_coordinate_cells: usize,
    streamed_rows: usize,
    current_depth: Option<usize>,
    hit_depth: Option<usize>,
    hit_chunk_ordinal: Option<usize>,
    hit_shell_request_ordinal: Option<usize>,
    hit_request_ordinal_in_chunk: Option<usize>,
    compact_support_requests: usize,
}

impl SpiredTargetRunCensus {
    pub(crate) const fn shells_opened(self) -> usize {
        self.shells_opened
    }

    pub(crate) const fn shells_completed(self) -> usize {
        self.shells_completed
    }

    pub(crate) const fn chunks_materialized(self) -> usize {
        self.chunks_materialized
    }

    pub(crate) const fn scheduled_requests(self) -> usize {
        self.scheduled_requests
    }

    pub(crate) const fn scheduled_request_coordinate_cells(self) -> usize {
        self.scheduled_request_coordinate_cells
    }

    pub(crate) const fn streamed_rows(self) -> usize {
        self.streamed_rows
    }

    pub(crate) const fn current_depth(self) -> Option<usize> {
        self.current_depth
    }

    pub(crate) const fn hit_depth(self) -> Option<usize> {
        self.hit_depth
    }

    pub(crate) const fn hit_chunk_ordinal(self) -> Option<usize> {
        self.hit_chunk_ordinal
    }

    pub(crate) const fn hit_shell_request_ordinal(self) -> Option<usize> {
        self.hit_shell_request_ordinal
    }

    pub(crate) const fn hit_request_ordinal_in_chunk(self) -> Option<usize> {
        self.hit_request_ordinal_in_chunk
    }

    pub(crate) const fn compact_support_requests(self) -> usize {
        self.compact_support_requests
    }

    pub(super) fn set_current_depth(&mut self, depth: usize) {
        self.current_depth = Some(depth);
    }

    pub(super) fn set_shells_opened(&mut self, value: usize) {
        self.shells_opened = value;
    }

    pub(super) fn set_shells_completed(&mut self, value: usize) {
        self.shells_completed = value;
    }

    pub(super) fn set_chunks_materialized(&mut self, value: usize) {
        self.chunks_materialized = value;
    }

    pub(super) fn set_scheduled_requests(&mut self, value: usize) {
        self.scheduled_requests = value;
    }

    pub(super) fn set_scheduled_request_coordinate_cells(&mut self, value: usize) {
        self.scheduled_request_coordinate_cells = value;
    }

    pub(super) fn set_streamed_rows(&mut self, value: usize) {
        self.streamed_rows = value;
    }

    pub(super) fn record_hit(
        &mut self,
        depth: usize,
        chunk_ordinal: usize,
        shell_request_ordinal: usize,
        request_ordinal_in_chunk: usize,
        compact_support_requests: usize,
    ) {
        self.hit_depth = Some(depth);
        self.hit_chunk_ordinal = Some(chunk_ordinal);
        self.hit_shell_request_ordinal = Some(shell_request_ordinal);
        self.hit_request_ordinal_in_chunk = Some(request_ordinal_in_chunk);
        self.compact_support_requests = compact_support_requests;
    }
}

/// Normal ending of one finite target/probe lane.
#[derive(Debug)]
pub(crate) enum SpiredTargetRunOutcome {
    /// The existing promotion boundary decided whether the exact replay is an
    /// admitted cell or needs a guard-specific retry.
    RuleCell(ExactRuleCellPromotionDisposition),
    /// The first streamed modular support did not reproduce an exact replay
    /// on the fresh frame. A later independent probe may retry it.
    CompactInconclusive(SpiredCompactLift),
    /// Every scheduled request through the configured depth was consumed
    /// without a hit. This is bounded-search telemetry, never no-relation or
    /// closure evidence.
    SearchDepthExhausted,
}

#[derive(Debug)]
pub(crate) struct SpiredTargetRunReport {
    census: SpiredTargetRunCensus,
    outcome: SpiredTargetRunOutcome,
}

impl SpiredTargetRunReport {
    pub(crate) const fn census(&self) -> SpiredTargetRunCensus {
        self.census
    }

    pub(crate) const fn outcome(&self) -> &SpiredTargetRunOutcome {
        &self.outcome
    }

    pub(crate) fn into_outcome(self) -> SpiredTargetRunOutcome {
        self.outcome
    }

    pub(super) const fn new(
        census: SpiredTargetRunCensus,
        outcome: SpiredTargetRunOutcome,
    ) -> Self {
        Self { census, outcome }
    }
}
