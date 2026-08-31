use crate::foundry::completion::source_discovery::scheduler::ProbeLocalRunCensus;
use crate::foundry::completion::source_discovery::{
    CanonicalReplayTelemetry, ExactExecutableOwnerProposal,
};
use crate::identity::RowId;

/// Scalar census of terminal scheduler outcomes. No epoch or ordinal-bearing
/// proof payload is retained.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct InteriorReplaySchedulerOutcomeCensus {
    replayed: usize,
    support_did_not_lift: usize,
    exact_lift_error: usize,
    sampled_dual: usize,
    budget_stop: usize,
    rejected: usize,
    stalled: usize,
}

impl InteriorReplaySchedulerOutcomeCensus {
    pub(crate) const fn replayed(self) -> usize {
        self.replayed
    }
    pub(crate) const fn support_did_not_lift(self) -> usize {
        self.support_did_not_lift
    }
    pub(crate) const fn exact_lift_error(self) -> usize {
        self.exact_lift_error
    }
    pub(crate) const fn sampled_dual(self) -> usize {
        self.sampled_dual
    }
    pub(crate) const fn budget_stop(self) -> usize {
        self.budget_stop
    }
    pub(crate) const fn rejected(self) -> usize {
        self.rejected
    }
    pub(crate) const fn stalled(self) -> usize {
        self.stalled
    }

    pub(super) fn increment_replayed(&mut self) {
        self.replayed += 1;
    }
    pub(super) fn increment_support_did_not_lift(&mut self) {
        self.support_did_not_lift += 1;
    }
    pub(super) fn increment_exact_lift_error(&mut self) {
        self.exact_lift_error += 1;
    }
    pub(super) fn increment_sampled_dual(&mut self) {
        self.sampled_dual += 1;
    }
    pub(super) fn increment_budget_stop(&mut self) {
        self.budget_stop += 1;
    }
    pub(super) fn increment_rejected(&mut self) {
        self.rejected += 1;
    }
    pub(super) fn increment_stalled(&mut self) {
        self.stalled += 1;
    }
}

/// Ordinal-free summary of canonical common-plan attempts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct InteriorReplayAttemptCensus {
    replayed: usize,
    no_modular_hit: usize,
    query_rejected: usize,
    support_did_not_lift: usize,
}

impl InteriorReplayAttemptCensus {
    pub(crate) const fn replayed(self) -> usize {
        self.replayed
    }
    pub(crate) const fn no_modular_hit(self) -> usize {
        self.no_modular_hit
    }
    pub(crate) const fn query_rejected(self) -> usize {
        self.query_rejected
    }
    pub(crate) const fn support_did_not_lift(self) -> usize {
        self.support_did_not_lift
    }

    pub(super) fn increment_replayed(&mut self) {
        self.replayed += 1;
    }
    pub(super) fn increment_no_modular_hit(&mut self) {
        self.no_modular_hit += 1;
    }
    pub(super) fn increment_query_rejected(&mut self) {
        self.query_rejected += 1;
    }
    pub(super) fn increment_support_did_not_lift(&mut self) {
        self.support_did_not_lift += 1;
    }
}

/// One translated ordinary source with its offset relative to the target.
/// Source ordinals and row identities are stable module provenance, not
/// physical frame-row ordinals.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct InteriorReplayRelativeSource {
    source_ordinal: usize,
    source_row: RowId,
    relative_offset: Box<[i64]>,
}

impl InteriorReplayRelativeSource {
    pub(crate) const fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }
    pub(crate) const fn source_row(&self) -> &RowId {
        &self.source_row
    }
    pub(crate) fn relative_offset(&self) -> &[i64] {
        &self.relative_offset
    }

    pub(super) fn new(source_ordinal: usize, source_row: RowId, relative_offset: Vec<i64>) -> Self {
        Self {
            source_ordinal,
            source_row,
            relative_offset: relative_offset.into_boxed_slice(),
        }
    }
}

/// One exact residual shift relative to the target, with no physical column.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct InteriorReplayRelativeResidual {
    relative_shift: Box<[i64]>,
}

impl InteriorReplayRelativeResidual {
    pub(crate) fn relative_shift(&self) -> &[i64] {
        &self.relative_shift
    }

    pub(super) fn new(relative_shift: Vec<i64>) -> Self {
        Self {
            relative_shift: relative_shift.into_boxed_slice(),
        }
    }
}

/// Exact ordinal-free row/residual support shape of one compiled candidate.
///
/// Guard cardinalities are diagnostics. Guard polynomials and coefficients
/// are deliberately absent, so equality of this value is never exact-rule
/// equality or admission evidence.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct InteriorReplayCandidateSupport {
    sources: Box<[InteriorReplayRelativeSource]>,
    residuals: Box<[InteriorReplayRelativeResidual]>,
    pivot_guard_count: usize,
    nonzero_guard_count: usize,
}

impl InteriorReplayCandidateSupport {
    pub(crate) fn sources(&self) -> &[InteriorReplayRelativeSource] {
        &self.sources
    }
    pub(crate) fn residuals(&self) -> &[InteriorReplayRelativeResidual] {
        &self.residuals
    }
    pub(crate) const fn pivot_guard_count(&self) -> usize {
        self.pivot_guard_count
    }
    pub(crate) const fn nonzero_guard_count(&self) -> usize {
        self.nonzero_guard_count
    }

    pub(super) fn new(
        sources: Vec<InteriorReplayRelativeSource>,
        residuals: Vec<InteriorReplayRelativeResidual>,
        pivot_guard_count: usize,
        nonzero_guard_count: usize,
    ) -> Self {
        Self {
            sources: sources.into_boxed_slice(),
            residuals: residuals.into_boxed_slice(),
            pivot_guard_count,
            nonzero_guard_count,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InteriorReplaySupportCensus {
    candidates: usize,
    relative_sources: usize,
    relative_residuals: usize,
    relative_coordinate_cells: usize,
    sort_work_reservation: usize,
}

impl InteriorReplaySupportCensus {
    pub(crate) const fn candidates(self) -> usize {
        self.candidates
    }
    pub(crate) const fn relative_sources(self) -> usize {
        self.relative_sources
    }
    pub(crate) const fn relative_residuals(self) -> usize {
        self.relative_residuals
    }
    pub(crate) const fn relative_coordinate_cells(self) -> usize {
        self.relative_coordinate_cells
    }
    pub(crate) const fn sort_work_reservation(self) -> usize {
        self.sort_work_reservation
    }

    pub(super) const fn new(
        candidates: usize,
        relative_sources: usize,
        relative_residuals: usize,
        relative_coordinate_cells: usize,
        sort_work_reservation: usize,
    ) -> Self {
        Self {
            candidates,
            relative_sources,
            relative_residuals,
            relative_coordinate_cells,
            sort_work_reservation,
        }
    }
}

/// Comparable exact row/residual support-shape telemetry for one compiled
/// owner. It does not retain coefficient or guard-polynomial content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InteriorReplaySupportSet {
    candidates: Box<[InteriorReplayCandidateSupport]>,
    census: InteriorReplaySupportCensus,
}

impl InteriorReplaySupportSet {
    pub(crate) fn candidates(&self) -> &[InteriorReplayCandidateSupport] {
        &self.candidates
    }
    pub(crate) const fn census(&self) -> InteriorReplaySupportCensus {
        self.census
    }

    pub(super) fn new(
        candidates: Vec<InteriorReplayCandidateSupport>,
        census: InteriorReplaySupportCensus,
    ) -> Self {
        Self {
            candidates: candidates.into_boxed_slice(),
            census,
        }
    }
}

/// Compact end state after the old scheduler report has been dropped.
#[derive(Debug)]
pub(crate) enum InteriorReplayRunDisposition {
    NoReplayedNominations,
    NoRebasedCircuits {
        replay: CanonicalReplayTelemetry,
        attempts: InteriorReplayAttemptCensus,
    },
    OwnerProposal {
        proposal: ExactExecutableOwnerProposal,
        support: Option<InteriorReplaySupportSet>,
    },
}

/// Complete one-task report. It deliberately has no cover, terminal,
/// exhaustion, publication, or closure field.
#[derive(Debug)]
pub(crate) struct InteriorReplayTaskReport {
    scheduler: ProbeLocalRunCensus,
    scheduler_outcomes: InteriorReplaySchedulerOutcomeCensus,
    replay: Option<CanonicalReplayTelemetry>,
    disposition: InteriorReplayRunDisposition,
}

impl InteriorReplayTaskReport {
    pub(crate) const fn scheduler(&self) -> ProbeLocalRunCensus {
        self.scheduler
    }
    pub(crate) const fn scheduler_outcomes(&self) -> InteriorReplaySchedulerOutcomeCensus {
        self.scheduler_outcomes
    }
    pub(crate) const fn replay(&self) -> Option<CanonicalReplayTelemetry> {
        self.replay
    }
    pub(crate) const fn disposition(&self) -> &InteriorReplayRunDisposition {
        &self.disposition
    }

    pub(super) const fn new(
        scheduler: ProbeLocalRunCensus,
        scheduler_outcomes: InteriorReplaySchedulerOutcomeCensus,
        replay: Option<CanonicalReplayTelemetry>,
        disposition: InteriorReplayRunDisposition,
    ) -> Self {
        Self {
            scheduler,
            scheduler_outcomes,
            replay,
            disposition,
        }
    }
}
