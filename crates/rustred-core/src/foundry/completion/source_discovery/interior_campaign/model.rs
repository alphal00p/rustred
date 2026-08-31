use crate::foundry::completion::source_discovery::cover_delta::ExactOwnerLedgerRevision;
use crate::foundry::completion::source_discovery::cover_delta::ExactOwnerLedgerSnapshotIdentity;
use crate::foundry::completion::source_discovery::interior_simplex::{
    InteriorSimplexPlan, InteriorSimplexTask,
};
use crate::foundry::completion::source_discovery::scheduler::ProbeLocalRunCensus;
use crate::foundry::completion::source_discovery::{
    CanonicalReplayTelemetry, ExactExecutableCandidateObstruction, ExactOwnerCoverDelta,
    InteriorReplayAttemptCensus, InteriorReplayRunDisposition,
    InteriorReplaySchedulerOutcomeCensus, InteriorReplaySupportCensus, InteriorReplayTaskReport,
    UnpublishedCanonicalOwnerProposal,
};

/// Opaque pairing of one checked plan task with the exact ledger revision and
/// uncovered box from which it was bound.
#[derive(Debug)]
pub(crate) struct InteriorCampaignTaskBinding<'plan> {
    pub(super) plan: &'plan InteriorSimplexPlan,
    pub(super) task: &'plan InteriorSimplexTask,
    pub(super) ledger_snapshot: ExactOwnerLedgerSnapshotIdentity,
}

impl InteriorCampaignTaskBinding<'_> {
    pub(crate) const fn task(&self) -> &InteriorSimplexTask {
        self.task
    }

    pub(crate) const fn planned_ledger_revision(&self) -> ExactOwnerLedgerRevision {
        self.ledger_snapshot.revision()
    }

    pub(super) const fn new<'plan>(
        plan: &'plan InteriorSimplexPlan,
        task: &'plan InteriorSimplexTask,
        ledger_snapshot: ExactOwnerLedgerSnapshotIdentity,
    ) -> InteriorCampaignTaskBinding<'plan> {
        InteriorCampaignTaskBinding {
            plan,
            task,
            ledger_snapshot,
        }
    }
}

/// Allocation-free census for the target-unit bootstrap used to derive one
/// task-specific maximal stratum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InteriorCampaignBootstrapCensus {
    raw_incidence_visits: usize,
    unique_before_existing_exclusion: usize,
    excluded_existing_requests: usize,
    requests: usize,
    selected_sources: usize,
    physical_shift_occurrences: usize,
    distinct_physical_shifts: usize,
    physical_shift_coordinate_cells: usize,
    physical_shift_sort_work: usize,
}

impl InteriorCampaignBootstrapCensus {
    pub(crate) const fn raw_incidence_visits(self) -> usize {
        self.raw_incidence_visits
    }

    pub(crate) const fn unique_before_existing_exclusion(self) -> usize {
        self.unique_before_existing_exclusion
    }

    pub(crate) const fn excluded_existing_requests(self) -> usize {
        self.excluded_existing_requests
    }

    pub(crate) const fn requests(self) -> usize {
        self.requests
    }

    pub(crate) const fn selected_sources(self) -> usize {
        self.selected_sources
    }

    pub(crate) const fn physical_shift_occurrences(self) -> usize {
        self.physical_shift_occurrences
    }

    pub(crate) const fn distinct_physical_shifts(self) -> usize {
        self.distinct_physical_shifts
    }

    pub(crate) const fn physical_shift_coordinate_cells(self) -> usize {
        self.physical_shift_coordinate_cells
    }

    pub(crate) const fn physical_shift_sort_work(self) -> usize {
        self.physical_shift_sort_work
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) const fn new(
        raw_incidence_visits: usize,
        unique_before_existing_exclusion: usize,
        excluded_existing_requests: usize,
        requests: usize,
        selected_sources: usize,
        physical_shift_occurrences: usize,
        distinct_physical_shifts: usize,
        physical_shift_coordinate_cells: usize,
        physical_shift_sort_work: usize,
    ) -> Self {
        Self {
            raw_incidence_visits,
            unique_before_existing_exclusion,
            excluded_existing_requests,
            requests,
            selected_sources,
            physical_shift_occurrences,
            distinct_physical_shifts,
            physical_shift_coordinate_cells,
            physical_shift_sort_work,
        }
    }
}

/// Compact, bounded scalar evidence retained beside one typed outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InteriorCampaignCensus {
    bootstrap: InteriorCampaignBootstrapCensus,
    scheduler: ProbeLocalRunCensus,
    scheduler_outcomes: InteriorReplaySchedulerOutcomeCensus,
    replay: Option<CanonicalReplayTelemetry>,
    support: Option<InteriorReplaySupportCensus>,
    exact_obstructions: usize,
}

impl InteriorCampaignCensus {
    pub(crate) const fn bootstrap(self) -> InteriorCampaignBootstrapCensus {
        self.bootstrap
    }

    pub(crate) const fn scheduler(self) -> ProbeLocalRunCensus {
        self.scheduler
    }

    pub(crate) const fn scheduler_outcomes(self) -> InteriorReplaySchedulerOutcomeCensus {
        self.scheduler_outcomes
    }

    pub(crate) const fn replay(self) -> Option<CanonicalReplayTelemetry> {
        self.replay
    }

    pub(crate) const fn support(self) -> Option<InteriorReplaySupportCensus> {
        self.support
    }

    pub(crate) const fn exact_obstructions(self) -> usize {
        self.exact_obstructions
    }

    pub(super) const fn new(
        bootstrap: InteriorCampaignBootstrapCensus,
        replay: &InteriorReplayTaskReport,
        exact_obstructions: usize,
    ) -> Self {
        let support = match replay.disposition() {
            InteriorReplayRunDisposition::OwnerProposal {
                support: Some(support),
                ..
            } => Some(support.census()),
            _ => None,
        };
        Self {
            bootstrap,
            scheduler: replay.scheduler(),
            scheduler_outcomes: replay.scheduler_outcomes(),
            replay: replay.replay(),
            support,
            exact_obstructions,
        }
    }
}

/// Why exact replay did not yield an owner proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InteriorCampaignNoProposal {
    NoReplayedNominations,
    NoRebasedCircuits {
        replay: CanonicalReplayTelemetry,
        attempts: InteriorReplayAttemptCensus,
    },
}

/// Geometric effect of an owner application before any closure status is
/// interpreted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InteriorCampaignOwnerEffect {
    Duplicate,
    ChangedWithoutGeometricShrink,
    StrictGeometricShrink,
}

/// Borrowed exact result of a compiled owner application. Candidate
/// obstructions remain available for later exact guard refinement.
#[derive(Clone, Copy, Debug)]
pub(crate) struct InteriorCampaignAppliedOwner<'a> {
    delta: ExactOwnerCoverDelta,
    obstructions: &'a [ExactExecutableCandidateObstruction],
}

impl<'a> InteriorCampaignAppliedOwner<'a> {
    pub(crate) const fn delta(self) -> ExactOwnerCoverDelta {
        self.delta
    }

    pub(crate) const fn obstructions(self) -> &'a [ExactExecutableCandidateObstruction] {
        self.obstructions
    }

    pub(super) const fn new(
        delta: ExactOwnerCoverDelta,
        obstructions: &'a [ExactExecutableCandidateObstruction],
    ) -> Self {
        Self {
            delta,
            obstructions,
        }
    }
}

/// Typed semantic outcome. `Closed` is emitted only when the ledger's exact
/// compiler status is closed; no scheduler or replay state can produce it.
#[derive(Clone, Copy, Debug)]
pub(crate) enum InteriorCampaignOutcome<'a> {
    NoProposal(InteriorCampaignNoProposal),
    IncompleteProposal(&'a UnpublishedCanonicalOwnerProposal),
    Duplicate(InteriorCampaignAppliedOwner<'a>),
    ChangedWithoutGeometricShrink(InteriorCampaignAppliedOwner<'a>),
    StrictGeometricShrink(InteriorCampaignAppliedOwner<'a>),
    Closed {
        effect: InteriorCampaignOwnerEffect,
        applied: InteriorCampaignAppliedOwner<'a>,
    },
}

/// One task report. The replay payload is retained under its existing exact
/// limits so incomplete proposals and compiled-candidate obstructions remain
/// available to the next refinement layer; the census is allocation-free.
#[derive(Debug)]
pub(crate) struct InteriorCampaignTaskReport {
    canonical_task_ordinal: usize,
    planned_ledger_revision: ExactOwnerLedgerRevision,
    census: InteriorCampaignCensus,
    replay: InteriorReplayTaskReport,
    delta: Option<ExactOwnerCoverDelta>,
}

impl InteriorCampaignTaskReport {
    pub(crate) const fn canonical_task_ordinal(&self) -> usize {
        self.canonical_task_ordinal
    }

    pub(crate) const fn planned_ledger_revision(&self) -> ExactOwnerLedgerRevision {
        self.planned_ledger_revision
    }

    pub(crate) const fn census(&self) -> InteriorCampaignCensus {
        self.census
    }

    pub(crate) fn outcome(&self) -> InteriorCampaignOutcome<'_> {
        match self.replay.disposition() {
            InteriorReplayRunDisposition::NoReplayedNominations => {
                InteriorCampaignOutcome::NoProposal(
                    InteriorCampaignNoProposal::NoReplayedNominations,
                )
            }
            InteriorReplayRunDisposition::NoRebasedCircuits { replay, attempts } => {
                InteriorCampaignOutcome::NoProposal(InteriorCampaignNoProposal::NoRebasedCircuits {
                    replay: *replay,
                    attempts: *attempts,
                })
            }
            InteriorReplayRunDisposition::OwnerProposal {
                proposal: super::super::ExactExecutableOwnerProposal::Incomplete(proposal),
                ..
            } => InteriorCampaignOutcome::IncompleteProposal(proposal),
            InteriorReplayRunDisposition::OwnerProposal {
                proposal: super::super::ExactExecutableOwnerProposal::Compiled { obstructions, .. },
                ..
            } => {
                let delta = self
                    .delta
                    .expect("compiled campaign owners are applied before report construction");
                let effect = match delta.kind() {
                    super::super::ExactOwnerCoverDeltaKind::Duplicate => {
                        InteriorCampaignOwnerEffect::Duplicate
                    }
                    super::super::ExactOwnerCoverDeltaKind::ChangedWithoutGeometricShrink => {
                        InteriorCampaignOwnerEffect::ChangedWithoutGeometricShrink
                    }
                    super::super::ExactOwnerCoverDeltaKind::StrictGeometricShrink => {
                        InteriorCampaignOwnerEffect::StrictGeometricShrink
                    }
                };
                let applied = InteriorCampaignAppliedOwner::new(delta, obstructions);
                if delta.updated().status().is_compiler_closed() {
                    InteriorCampaignOutcome::Closed { effect, applied }
                } else {
                    match effect {
                        InteriorCampaignOwnerEffect::Duplicate => {
                            InteriorCampaignOutcome::Duplicate(applied)
                        }
                        InteriorCampaignOwnerEffect::ChangedWithoutGeometricShrink => {
                            InteriorCampaignOutcome::ChangedWithoutGeometricShrink(applied)
                        }
                        InteriorCampaignOwnerEffect::StrictGeometricShrink => {
                            InteriorCampaignOutcome::StrictGeometricShrink(applied)
                        }
                    }
                }
            }
        }
    }

    pub(super) const fn new(
        canonical_task_ordinal: usize,
        planned_ledger_revision: ExactOwnerLedgerRevision,
        census: InteriorCampaignCensus,
        replay: InteriorReplayTaskReport,
        delta: Option<ExactOwnerCoverDelta>,
    ) -> Self {
        Self {
            canonical_task_ordinal,
            planned_ledger_revision,
            census,
            replay,
            delta,
        }
    }
}
