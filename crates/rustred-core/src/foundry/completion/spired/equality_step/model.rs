use crate::foundry::completion::source_discovery::{
    ExactOwnerCoverDelta, ExactOwnerCoverDeltaKind, ExactOwnerLedgerCoverStatus,
};

use super::super::{
    SpiredAlternativeExhaustion, SpiredCoordinateGuardCaseIncomplete,
    SpiredCoordinateGuardCaseRejection,
};

/// Compact reason why one exact or modular probe did not yield an admitted
/// candidate. These values are search telemetry only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpiredEqualityStepProbeIncomplete {
    SearchDepthExhausted,
    CompactReplayInconclusive {
        exhaustion: SpiredAlternativeExhaustion,
    },
    KnownZeroPromotionInconclusive {
        required_predicate_ordinal: usize,
        exhaustion: SpiredAlternativeExhaustion,
    },
    BlockedByKnownZero {
        required_predicate_ordinal: usize,
    },
    NeedsGuardedStratum,
    AnchorOnGuardWall {
        guard_ordinal: usize,
    },
    SingularModularSample,
}

/// Normal fail-closed stop of one bounded equality-case step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredEqualityStepIncomplete {
    EmptyWorklist,
    EmptyProbePortfolio,
    ProbePortfolioExhausted {
        probes_attempted: usize,
        last: Option<SpiredEqualityStepProbeIncomplete>,
    },
    GuardCaseCandidateUnusable(SpiredCoordinateGuardCaseRejection),
    GuardCaseGeometry(SpiredCoordinateGuardCaseIncomplete),
    OwnerCompilationIncomplete {
        obstructions: usize,
    },
    OwnerMadeNoGeometricProgress {
        kind: ExactOwnerCoverDeltaKind,
        updated_status: ExactOwnerLedgerCoverStatus,
    },
}

/// Aggregate work retained for one explicit deterministic probe portfolio.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SpiredEqualityStepCensus {
    pub(super) probes_offered: usize,
    pub(super) probes_attempted: usize,
    pub(super) singular_probes_skipped: usize,
    pub(super) scheduled_requests: usize,
    pub(super) streamed_rows: usize,
    pub(super) modular_hits: usize,
    pub(super) exact_lift_attempts: usize,
    pub(super) admitted_candidates: usize,
    pub(super) exact_guard_cases: usize,
    pub(super) owner_compile_attempts: usize,
    pub(super) committed_steps: usize,
}

impl SpiredEqualityStepCensus {
    pub(crate) const fn probes_offered(self) -> usize {
        self.probes_offered
    }

    pub(crate) const fn probes_attempted(self) -> usize {
        self.probes_attempted
    }

    pub(crate) const fn singular_probes_skipped(self) -> usize {
        self.singular_probes_skipped
    }

    pub(crate) const fn scheduled_requests(self) -> usize {
        self.scheduled_requests
    }

    pub(crate) const fn streamed_rows(self) -> usize {
        self.streamed_rows
    }

    pub(crate) const fn modular_hits(self) -> usize {
        self.modular_hits
    }

    pub(crate) const fn exact_lift_attempts(self) -> usize {
        self.exact_lift_attempts
    }

    pub(crate) const fn admitted_candidates(self) -> usize {
        self.admitted_candidates
    }

    pub(crate) const fn exact_guard_cases(self) -> usize {
        self.exact_guard_cases
    }

    pub(crate) const fn owner_compile_attempts(self) -> usize {
        self.owner_compile_attempts
    }

    pub(crate) const fn committed_steps(self) -> usize {
        self.committed_steps
    }
}

/// Exact live effect of one successful two-object transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredEqualityStepCommitted {
    owner_delta: ExactOwnerCoverDelta,
    proposed_guard_children: usize,
    retained_pending_cases: usize,
    discharged_by_compiler_cover: bool,
}

impl SpiredEqualityStepCommitted {
    pub(crate) const fn owner_delta(self) -> ExactOwnerCoverDelta {
        self.owner_delta
    }

    pub(crate) const fn proposed_guard_children(self) -> usize {
        self.proposed_guard_children
    }

    pub(crate) const fn retained_pending_cases(self) -> usize {
        self.retained_pending_cases
    }

    /// Whether the exact post-mutation compiler cover discharged the entire
    /// ledger, allowing all otherwise-proposed guard children to be omitted.
    pub(crate) const fn discharged_by_compiler_cover(self) -> bool {
        self.discharged_by_compiler_cover
    }

    pub(super) const fn new(
        owner_delta: ExactOwnerCoverDelta,
        proposed_guard_children: usize,
        retained_pending_cases: usize,
        discharged_by_compiler_cover: bool,
    ) -> Self {
        Self {
            owner_delta,
            proposed_guard_children,
            retained_pending_cases,
            discharged_by_compiler_cover,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredEqualityStepOutcome {
    Committed(SpiredEqualityStepCommitted),
    Incomplete(SpiredEqualityStepIncomplete),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredEqualityStepReport {
    census: SpiredEqualityStepCensus,
    outcome: SpiredEqualityStepOutcome,
}

impl SpiredEqualityStepReport {
    pub(crate) const fn census(&self) -> SpiredEqualityStepCensus {
        self.census
    }

    pub(crate) const fn outcome(&self) -> &SpiredEqualityStepOutcome {
        &self.outcome
    }

    pub(crate) fn into_outcome(self) -> SpiredEqualityStepOutcome {
        self.outcome
    }

    pub(super) const fn new(
        census: SpiredEqualityStepCensus,
        outcome: SpiredEqualityStepOutcome,
    ) -> Self {
        Self { census, outcome }
    }
}
