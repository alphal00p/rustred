use crate::foundry::completion::source_discovery::ExactRuleCellPromotionDisposition;
use crate::foundry::completion::stratum::GuardBranchIdentity;

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
    search_branches_started: usize,
    search_branches_exhausted: usize,
    alternative_branches_enqueued: usize,
    excluded_requests_skipped: usize,
    modular_hits_seen: usize,
    distinct_compact_lift_attempts: usize,
    known_zero_promotions_rejected: usize,
    duplicate_rejected_support_hits: usize,
    alternative_streamed_rows: usize,
    retained_exclusion_requests: usize,
    retained_exclusion_coordinate_cells: usize,
    retained_rejected_support_requests: usize,
    retained_rejected_support_coordinate_cells: usize,
    post_hit_rows_streamed: usize,
    post_hit_candidates_seen: usize,
    rooted_compact_lift_attempts: usize,
    viable_candidates_compared: usize,
    best_candidate_replacements: usize,
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

    pub(crate) const fn search_branches_started(self) -> usize {
        self.search_branches_started
    }

    pub(crate) const fn search_branches_exhausted(self) -> usize {
        self.search_branches_exhausted
    }

    pub(crate) const fn alternative_branches_enqueued(self) -> usize {
        self.alternative_branches_enqueued
    }

    pub(crate) const fn excluded_requests_skipped(self) -> usize {
        self.excluded_requests_skipped
    }

    pub(crate) const fn modular_hits_seen(self) -> usize {
        self.modular_hits_seen
    }

    pub(crate) const fn distinct_compact_lift_attempts(self) -> usize {
        self.distinct_compact_lift_attempts
    }

    /// Exactly replayed candidates whose required denominator predicate was
    /// already known zero on this case. Such a candidate is unusable, so it
    /// is retained only as a rejected support for the bounded alternative
    /// portfolio; it never ends or closes the case by itself.
    pub(crate) const fn known_zero_promotions_rejected(self) -> usize {
        self.known_zero_promotions_rejected
    }

    pub(crate) const fn duplicate_rejected_support_hits(self) -> usize {
        self.duplicate_rejected_support_hits
    }

    pub(crate) const fn alternative_streamed_rows(self) -> usize {
        self.alternative_streamed_rows
    }

    pub(crate) const fn retained_exclusion_requests(self) -> usize {
        self.retained_exclusion_requests
    }

    pub(crate) const fn retained_exclusion_coordinate_cells(self) -> usize {
        self.retained_exclusion_coordinate_cells
    }

    pub(crate) const fn retained_rejected_support_requests(self) -> usize {
        self.retained_rejected_support_requests
    }

    pub(crate) const fn retained_rejected_support_coordinate_cells(self) -> usize {
        self.retained_rejected_support_coordinate_cells
    }

    /// Rows admitted after a branch's immutable first target pivot.
    pub(crate) const fn post_hit_rows_streamed(self) -> usize {
        self.post_hit_rows_streamed
    }

    /// Later dependency roots nominated by the modular GPLU trace.
    pub(crate) const fn post_hit_candidates_seen(self) -> usize {
        self.post_hit_candidates_seen
    }

    /// Post-hit nominations which reached fresh-frame rooted exact lifting.
    pub(crate) const fn rooted_compact_lift_attempts(self) -> usize {
        self.rooted_compact_lift_attempts
    }

    /// Exact non-blocked candidates admitted to deterministic comparison.
    pub(crate) const fn viable_candidates_compared(self) -> usize {
        self.viable_candidates_compared
    }

    /// Strict improvements over an already retained viable candidate.
    pub(crate) const fn best_candidate_replacements(self) -> usize {
        self.best_candidate_replacements
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

    pub(super) fn set_search_branches_started(&mut self, value: usize) {
        self.search_branches_started = value;
    }

    pub(super) fn set_search_branches_exhausted(&mut self, value: usize) {
        self.search_branches_exhausted = value;
    }

    pub(super) fn set_alternative_branches_enqueued(&mut self, value: usize) {
        self.alternative_branches_enqueued = value;
    }

    pub(super) fn set_excluded_requests_skipped(&mut self, value: usize) {
        self.excluded_requests_skipped = value;
    }

    pub(super) fn set_modular_hits_seen(&mut self, value: usize) {
        self.modular_hits_seen = value;
    }

    pub(super) fn set_distinct_compact_lift_attempts(&mut self, value: usize) {
        self.distinct_compact_lift_attempts = value;
    }

    pub(super) fn set_known_zero_promotions_rejected(&mut self, value: usize) {
        self.known_zero_promotions_rejected = value;
    }

    pub(super) fn set_duplicate_rejected_support_hits(&mut self, value: usize) {
        self.duplicate_rejected_support_hits = value;
    }

    pub(super) fn set_alternative_streamed_rows(&mut self, value: usize) {
        self.alternative_streamed_rows = value;
    }

    pub(super) fn set_retained_exclusion_requests(&mut self, value: usize) {
        self.retained_exclusion_requests = value;
    }

    pub(super) fn set_retained_exclusion_coordinate_cells(&mut self, value: usize) {
        self.retained_exclusion_coordinate_cells = value;
    }

    pub(super) fn set_retained_rejected_support_requests(&mut self, value: usize) {
        self.retained_rejected_support_requests = value;
    }

    pub(super) fn set_retained_rejected_support_coordinate_cells(&mut self, value: usize) {
        self.retained_rejected_support_coordinate_cells = value;
    }

    pub(super) fn set_post_hit_rows_streamed(&mut self, value: usize) {
        self.post_hit_rows_streamed = value;
    }

    pub(super) fn set_post_hit_candidates_seen(&mut self, value: usize) {
        self.post_hit_candidates_seen = value;
    }

    pub(super) fn set_rooted_compact_lift_attempts(&mut self, value: usize) {
        self.rooted_compact_lift_attempts = value;
    }

    pub(super) fn set_viable_candidates_compared(&mut self, value: usize) {
        self.viable_candidates_compared = value;
    }

    pub(super) fn set_best_candidate_replacements(&mut self, value: usize) {
        self.best_candidate_replacements = value;
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

/// Why the bounded canonical source-exclusion search stopped after at least
/// one modular support failed exact replay.  None of these reasons carries
/// no-relation, terminal, owner, or closure authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpiredAlternativeExhaustion {
    SearchDepth,
    DistinctCompactLiftAttempts,
    SearchBranches,
    ExcludedRequestsPerBranch,
    RetainedExclusionRequests,
    RetainedExclusionCoordinateCells,
    RetainedRejectedSupportRequests,
    RetainedRejectedSupportCoordinateCells,
    AlternativeStreamedRows,
}

/// Minimal exact diagnostic retained when a replayed circuit is unusable
/// because one of its required predicates is already zero on the case.
///
/// The heavy epoch/circuit payload is deliberately not retained after the
/// support has been rejected. This value conveys neither an owner nor a
/// terminal; it only explains the last bounded-search rejection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredKnownZeroPromotionRejection {
    required_predicate_ordinal: usize,
    first_circuit_guard_ordinal: usize,
    zero_branch: GuardBranchIdentity,
}

impl SpiredKnownZeroPromotionRejection {
    pub(crate) const fn required_predicate_ordinal(&self) -> usize {
        self.required_predicate_ordinal
    }

    pub(crate) const fn first_circuit_guard_ordinal(&self) -> usize {
        self.first_circuit_guard_ordinal
    }

    pub(crate) const fn zero_branch(&self) -> &GuardBranchIdentity {
        &self.zero_branch
    }

    pub(super) const fn new(
        required_predicate_ordinal: usize,
        first_circuit_guard_ordinal: usize,
        zero_branch: GuardBranchIdentity,
    ) -> Self {
        Self {
            required_predicate_ordinal,
            first_circuit_guard_ordinal,
            zero_branch,
        }
    }
}

/// Normal ending of one finite target/probe lane.
#[derive(Debug)]
pub(crate) enum SpiredTargetRunOutcome {
    /// The existing promotion boundary decided whether the exact replay is an
    /// admitted cell or needs a guard-specific retry.
    RuleCell(ExactRuleCellPromotionDisposition),
    /// At least one streamed support did not reproduce an exact replay and
    /// the bounded canonical source-exclusion alternatives were exhausted.
    /// The retained value is the last distinct exact-lift attempt, not
    /// publication evidence.
    CompactInconclusive {
        last: SpiredCompactLift,
        exhaustion: SpiredAlternativeExhaustion,
    },
    /// At least one exactly replayed support was unusable because a required
    /// predicate was already zero, and the bounded alternative portfolio did
    /// not find an admissible later circuit. This is explicitly not closure,
    /// a terminal declaration, or evidence that no relation exists.
    KnownZeroPromotionInconclusive {
        last: SpiredKnownZeroPromotionRejection,
        exhaustion: SpiredAlternativeExhaustion,
    },
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
