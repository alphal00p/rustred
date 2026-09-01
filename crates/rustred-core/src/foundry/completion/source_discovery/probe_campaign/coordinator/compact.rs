use crate::foundry::completion::source_discovery::cover_delta::ExactOwnerCoverSnapshot;
use crate::foundry::completion::source_discovery::{
    ExactExecutableOwnerProposal, InteriorReplayRunDisposition,
};

use super::super::{
    ProbeCampaignEvaluatedTask, ProbeCampaignNoProposal, ProbeCampaignOutcome,
    ProbeCampaignTaskReport,
};
use super::{
    ProbeCoordinatorCensus, ProbeCoordinatorFailure, ProbeCoordinatorNeedsRefinementReason,
    ProbeCoordinatorOperationalReason, ProbeCoordinatorOwnerMutation,
};

const CENSUS: &str = "scalar census";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CompactProbeEvidence {
    pub(super) scheduler_budget_stops: usize,
    pub(super) scheduler_rejections: usize,
    pub(super) scheduler_stalls: usize,
    pub(super) scheduler_exact_lift_errors: usize,
    pub(super) canonical_replayed: usize,
    pub(super) canonical_no_modular_hit: usize,
    pub(super) canonical_query_rejections: usize,
    pub(super) canonical_support_did_not_lift: usize,
    pub(super) exact_obstructions: usize,
    pub(super) declared_probes: usize,
    pub(super) scheduler_replayed: usize,
    pub(super) scheduler_support_did_not_lift: usize,
    pub(super) scheduler_sampled_dual: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CompactTaskAction {
    NoProposal,
    Duplicate,
    IncompleteProposal,
    OwnerSetChanged {
        mutation: ProbeCoordinatorOwnerMutation,
        before_revision: u64,
        after_revision: u64,
    },
    CompilerClosed {
        exact: ExactOwnerCoverSnapshot,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CompactTaskResult {
    pub(super) action: CompactTaskAction,
    pub(super) evidence: CompactProbeEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompactTaskKind {
    NoProposal = 0,
    Duplicate = 1,
    IncompleteProposal = 2,
    ChangedWithoutGeometricShrink = 3,
    StrictGeometricShrink = 4,
    CompilerClosed = 5,
}

impl CompactTaskKind {
    const COUNT: usize = 6;

    const fn from_action(action: CompactTaskAction) -> Self {
        match action {
            CompactTaskAction::NoProposal => Self::NoProposal,
            CompactTaskAction::Duplicate => Self::Duplicate,
            CompactTaskAction::IncompleteProposal => Self::IncompleteProposal,
            CompactTaskAction::OwnerSetChanged {
                mutation: ProbeCoordinatorOwnerMutation::ChangedWithoutGeometricShrink,
                ..
            } => Self::ChangedWithoutGeometricShrink,
            CompactTaskAction::OwnerSetChanged {
                mutation: ProbeCoordinatorOwnerMutation::StrictGeometricShrink,
                ..
            } => Self::StrictGeometricShrink,
            CompactTaskAction::CompilerClosed { .. } => Self::CompilerClosed,
        }
    }
}

/// Every fallible compact-census update reserved before serial application.
///
/// A compiled proposal can still be duplicate, mutate by either exact cover
/// effect, or close the compiler. All four counter states are therefore
/// checked while the ledger is immutable. Finishing after application only
/// selects one already materialized state and cannot return `Failed`.
#[derive(Clone, Copy, Debug)]
pub(super) struct CompactTaskReservation {
    evidence: CompactProbeEvidence,
    updated: [Option<ProbeCoordinatorCensus>; CompactTaskKind::COUNT],
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CompactTaskCommit {
    pub(super) compact: CompactTaskResult,
    pub(super) census: ProbeCoordinatorCensus,
}

pub(super) fn try_reserve_evaluated_task(
    evaluated: &ProbeCampaignEvaluatedTask,
    baseline: ProbeCoordinatorCensus,
    requested_report: usize,
    invalidated_tickets: usize,
) -> Result<CompactTaskReservation, ProbeCoordinatorFailure> {
    let evidence = try_compact_census(evaluated.census())?;
    let possible: &[CompactTaskKind] = match evaluated.replay_disposition() {
        InteriorReplayRunDisposition::NoReplayedNominations
        | InteriorReplayRunDisposition::NoRebasedCircuits { .. } => &[CompactTaskKind::NoProposal],
        InteriorReplayRunDisposition::OwnerProposal {
            proposal: ExactExecutableOwnerProposal::Incomplete(_),
            ..
        } => &[CompactTaskKind::IncompleteProposal],
        InteriorReplayRunDisposition::OwnerProposal {
            proposal: ExactExecutableOwnerProposal::Compiled { .. },
            ..
        } => &[
            CompactTaskKind::Duplicate,
            CompactTaskKind::ChangedWithoutGeometricShrink,
            CompactTaskKind::StrictGeometricShrink,
            CompactTaskKind::CompilerClosed,
        ],
    };
    try_reserve_kinds(
        baseline,
        requested_report,
        invalidated_tickets,
        evidence,
        possible,
    )
}

/// Reserve a synthetic compact result before invoking a test executor.
pub(super) fn try_reserve_compact_result(
    baseline: ProbeCoordinatorCensus,
    requested_report: usize,
    invalidated_tickets: usize,
    compact: CompactTaskResult,
) -> Result<CompactTaskCommit, ProbeCoordinatorFailure> {
    let kind = CompactTaskKind::from_action(compact.action);
    let reservation = try_reserve_kinds(
        baseline,
        requested_report,
        invalidated_tickets,
        compact.evidence,
        &[kind],
    )?;
    Ok(reservation.finish_action(compact.action))
}

impl CompactTaskReservation {
    pub(super) const fn declared_probes(self) -> usize {
        self.evidence.declared_probes
    }

    /// Select the counter state reserved for the exact post-application
    /// outcome. This is intentionally infallible: a mismatch means the same
    /// evaluated replay changed disposition while being consumed.
    pub(super) fn finish_report(self, report: &ProbeCampaignTaskReport) -> CompactTaskCommit {
        debug_assert!(
            matches!(try_compact_census(report.census()), Ok(evidence) if evidence == self.evidence)
        );
        self.finish_action(compact_action(report))
    }

    fn finish_action(self, action: CompactTaskAction) -> CompactTaskCommit {
        let kind = CompactTaskKind::from_action(action);
        let census = self.updated[kind as usize]
            .expect("post-application action lacked its prevalidated compact reservation");
        CompactTaskCommit {
            compact: CompactTaskResult {
                action,
                evidence: self.evidence,
            },
            census,
        }
    }
}

fn try_compact_census(
    census: super::super::ProbeCampaignCensus,
) -> Result<CompactProbeEvidence, ProbeCoordinatorFailure> {
    let scheduler = census.scheduler_outcomes();
    let attempts = census.canonical_attempts();
    let canonical_attempt_count = attempts
        .replayed()
        .checked_add(attempts.no_modular_hit())
        .and_then(|count| count.checked_add(attempts.query_rejected()))
        .and_then(|count| count.checked_add(attempts.support_did_not_lift()))
        .ok_or(ProbeCoordinatorFailure::ResourceCountOverflow { resource: CENSUS })?;
    try_validate_canonical_join(
        scheduler.replayed(),
        canonical_attempt_count,
        census
            .replay()
            .map(|replay| (replay.rebase_attempts(), replay.replayed_nominations())),
    )?;

    let mut evidence = CompactProbeEvidence {
        scheduler_budget_stops: scheduler.budget_stop(),
        scheduler_rejections: scheduler.rejected(),
        scheduler_stalls: scheduler.stalled(),
        scheduler_exact_lift_errors: scheduler.exact_lift_error(),
        canonical_replayed: attempts.replayed(),
        canonical_no_modular_hit: attempts.no_modular_hit(),
        canonical_query_rejections: attempts.query_rejected(),
        canonical_support_did_not_lift: attempts.support_did_not_lift(),
        exact_obstructions: census.exact_obstructions(),
        declared_probes: 0,
        scheduler_replayed: scheduler.replayed(),
        scheduler_support_did_not_lift: scheduler.support_did_not_lift(),
        scheduler_sampled_dual: scheduler.sampled_dual(),
    };
    evidence.declared_probes = try_scheduler_outcome_total(evidence)?;
    Ok(evidence)
}

fn compact_action(report: &ProbeCampaignTaskReport) -> CompactTaskAction {
    match report.outcome() {
        ProbeCampaignOutcome::NoProposal(ProbeCampaignNoProposal::NoReplayedNominations)
        | ProbeCampaignOutcome::NoProposal(ProbeCampaignNoProposal::NoRebasedCircuits { .. }) => {
            CompactTaskAction::NoProposal
        }
        ProbeCampaignOutcome::IncompleteProposal(_) => CompactTaskAction::IncompleteProposal,
        ProbeCampaignOutcome::Duplicate(_) => CompactTaskAction::Duplicate,
        ProbeCampaignOutcome::ChangedWithoutGeometricShrink(applied) => {
            let delta = applied.delta();
            CompactTaskAction::OwnerSetChanged {
                mutation: ProbeCoordinatorOwnerMutation::ChangedWithoutGeometricShrink,
                before_revision: delta.baseline().revision().get(),
                after_revision: delta.updated().revision().get(),
            }
        }
        ProbeCampaignOutcome::StrictGeometricShrink(applied) => {
            let delta = applied.delta();
            CompactTaskAction::OwnerSetChanged {
                mutation: ProbeCoordinatorOwnerMutation::StrictGeometricShrink,
                before_revision: delta.baseline().revision().get(),
                after_revision: delta.updated().revision().get(),
            }
        }
        ProbeCampaignOutcome::Closed { applied, .. } => CompactTaskAction::CompilerClosed {
            exact: applied.delta().updated(),
        },
    }
}

pub(super) fn try_validate_canonical_join(
    scheduler_replayed: usize,
    canonical_attempt_count: usize,
    replay: Option<(usize, usize)>,
) -> Result<(), ProbeCoordinatorFailure> {
    if canonical_attempt_count != scheduler_replayed {
        return Err(ProbeCoordinatorFailure::Invariant {
            detail: "canonical attempt total differed from scheduler replayed outcomes",
        });
    }
    match replay {
        Some((rebase_attempts, replayed_nominations)) => {
            if rebase_attempts != canonical_attempt_count {
                return Err(ProbeCoordinatorFailure::Invariant {
                    detail: "canonical replay attempt census did not match replay telemetry",
                });
            }
            if replayed_nominations != scheduler_replayed {
                return Err(ProbeCoordinatorFailure::Invariant {
                    detail: "canonical replay nominations differed from scheduler replayed outcomes",
                });
            }
        }
        None if scheduler_replayed != 0 => {
            return Err(ProbeCoordinatorFailure::Invariant {
                detail: "scheduler replayed outcomes were retained without replay telemetry",
            });
        }
        None => {}
    }
    Ok(())
}

pub(super) fn try_scheduler_outcome_total(
    evidence: CompactProbeEvidence,
) -> Result<usize, ProbeCoordinatorFailure> {
    evidence
        .scheduler_replayed
        .checked_add(evidence.scheduler_support_did_not_lift)
        .and_then(|count| count.checked_add(evidence.scheduler_exact_lift_errors))
        .and_then(|count| count.checked_add(evidence.scheduler_sampled_dual))
        .and_then(|count| count.checked_add(evidence.scheduler_budget_stops))
        .and_then(|count| count.checked_add(evidence.scheduler_rejections))
        .and_then(|count| count.checked_add(evidence.scheduler_stalls))
        .ok_or(ProbeCoordinatorFailure::ResourceCountOverflow { resource: CENSUS })
}

pub(super) fn validate_live_effect(
    before: ExactOwnerCoverSnapshot,
    after: ExactOwnerCoverSnapshot,
    action: CompactTaskAction,
) -> Result<(), ProbeCoordinatorFailure> {
    match action {
        CompactTaskAction::NoProposal
        | CompactTaskAction::Duplicate
        | CompactTaskAction::IncompleteProposal => {
            if after != before {
                return Err(ProbeCoordinatorFailure::Invariant {
                    detail: "non-mutating task outcome changed the exact owner ledger",
                });
            }
        }
        CompactTaskAction::OwnerSetChanged {
            before_revision,
            after_revision,
            ..
        } => {
            if before.revision().get() != before_revision
                || after.revision().get() != after_revision
                || after_revision
                    != before_revision.checked_add(1).ok_or(
                        ProbeCoordinatorFailure::ResourceCountOverflow { resource: CENSUS },
                    )?
            {
                return Err(ProbeCoordinatorFailure::Invariant {
                    detail: "owner mutation did not advance the exact ledger once",
                });
            }
        }
        CompactTaskAction::CompilerClosed { exact } => {
            if after != exact || !exact.status().is_compiler_closed() {
                return Err(ProbeCoordinatorFailure::Invariant {
                    detail: "compiler-closed task did not retain the live exact closed snapshot",
                });
            }
        }
    }
    Ok(())
}

fn try_reserve_kinds(
    baseline: ProbeCoordinatorCensus,
    requested_report: usize,
    invalidated_tickets: usize,
    evidence: CompactProbeEvidence,
    possible: &[CompactTaskKind],
) -> Result<CompactTaskReservation, ProbeCoordinatorFailure> {
    let mut reserved = [None; CompactTaskKind::COUNT];
    for &kind in possible {
        reserved[kind as usize] = Some(try_updated_census(
            baseline,
            requested_report,
            invalidated_tickets,
            evidence,
            kind,
        )?);
    }
    Ok(CompactTaskReservation {
        evidence,
        updated: reserved,
    })
}

fn try_updated_census(
    baseline: ProbeCoordinatorCensus,
    requested_report: usize,
    invalidated_tickets: usize,
    evidence: CompactProbeEvidence,
    kind: CompactTaskKind,
) -> Result<ProbeCoordinatorCensus, ProbeCoordinatorFailure> {
    let mut updated = baseline;
    updated.task_reports = requested_report;
    match kind {
        CompactTaskKind::NoProposal => try_increment(&mut updated.no_proposal, CENSUS)?,
        CompactTaskKind::Duplicate => try_increment(&mut updated.duplicate, CENSUS)?,
        CompactTaskKind::IncompleteProposal => {
            try_increment(&mut updated.incomplete_proposal, CENSUS)?;
        }
        CompactTaskKind::ChangedWithoutGeometricShrink => {
            try_increment(&mut updated.changed_without_geometric_shrink, CENSUS)?;
            try_add(
                &mut updated.invalidated_tickets,
                invalidated_tickets,
                CENSUS,
            )?;
        }
        CompactTaskKind::StrictGeometricShrink => {
            try_increment(&mut updated.strict_geometric_shrink, CENSUS)?;
            try_add(
                &mut updated.invalidated_tickets,
                invalidated_tickets,
                CENSUS,
            )?;
        }
        CompactTaskKind::CompilerClosed => {
            try_increment(&mut updated.compiler_closed, CENSUS)?;
        }
    }
    try_add(
        &mut updated.scheduler_budget_stops,
        evidence.scheduler_budget_stops,
        CENSUS,
    )?;
    try_add(
        &mut updated.scheduler_rejections,
        evidence.scheduler_rejections,
        CENSUS,
    )?;
    try_add(
        &mut updated.scheduler_stalls,
        evidence.scheduler_stalls,
        CENSUS,
    )?;
    try_add(
        &mut updated.scheduler_exact_lift_errors,
        evidence.scheduler_exact_lift_errors,
        CENSUS,
    )?;
    try_add(
        &mut updated.canonical_replayed,
        evidence.canonical_replayed,
        CENSUS,
    )?;
    try_add(
        &mut updated.canonical_no_modular_hit,
        evidence.canonical_no_modular_hit,
        CENSUS,
    )?;
    try_add(
        &mut updated.canonical_query_rejections,
        evidence.canonical_query_rejections,
        CENSUS,
    )?;
    try_add(
        &mut updated.canonical_support_did_not_lift,
        evidence.canonical_support_did_not_lift,
        CENSUS,
    )?;
    try_add(
        &mut updated.exact_obstructions,
        evidence.exact_obstructions,
        CENSUS,
    )?;
    try_add(
        &mut updated.declared_probes,
        evidence.declared_probes,
        CENSUS,
    )?;
    try_add(
        &mut updated.scheduler_replayed,
        evidence.scheduler_replayed,
        CENSUS,
    )?;
    try_add(
        &mut updated.scheduler_support_did_not_lift,
        evidence.scheduler_support_did_not_lift,
        CENSUS,
    )?;
    try_add(
        &mut updated.scheduler_sampled_dual,
        evidence.scheduler_sampled_dual,
        CENSUS,
    )?;
    Ok(updated)
}

pub(super) fn operational_reason(
    compact: CompactTaskResult,
) -> Option<ProbeCoordinatorOperationalReason> {
    let evidence = compact.evidence;
    if evidence.scheduler_budget_stops != 0
        || evidence.scheduler_rejections != 0
        || evidence.scheduler_exact_lift_errors != 0
    {
        return Some(
            ProbeCoordinatorOperationalReason::IncompleteProbeExecution {
                scheduler_budget_stops: evidence.scheduler_budget_stops,
                scheduler_rejections: evidence.scheduler_rejections,
                scheduler_exact_lift_errors: evidence.scheduler_exact_lift_errors,
            },
        );
    }
    None
}

pub(super) fn search_refinement_reason(
    compact: CompactTaskResult,
) -> Option<ProbeCoordinatorNeedsRefinementReason> {
    let evidence = compact.evidence;
    if evidence.scheduler_stalls != 0 {
        return Some(ProbeCoordinatorNeedsRefinementReason::ProbeStalled {
            scheduler_stalls: evidence.scheduler_stalls,
        });
    }
    if evidence.canonical_query_rejections != 0 {
        return Some(
            ProbeCoordinatorNeedsRefinementReason::CanonicalQueryRejected {
                canonical_query_rejections: evidence.canonical_query_rejections,
            },
        );
    }
    // SupportDidNotLift is a completed outcome of the declared finite probe
    // program, not an operational failure or an instruction to change that
    // program. Both scheduler and canonical buckets remain in the exact
    // census. Their presence is compatible only with ExhaustedAtConfig's weak
    // finite-program meaning and can never produce CompilerClosed.
    None
}

pub(super) fn try_increment(
    counter: &mut usize,
    resource: &'static str,
) -> Result<(), ProbeCoordinatorFailure> {
    *counter = counter
        .checked_add(1)
        .ok_or(ProbeCoordinatorFailure::ResourceCountOverflow { resource })?;
    Ok(())
}

fn try_add(
    counter: &mut usize,
    amount: usize,
    resource: &'static str,
) -> Result<(), ProbeCoordinatorFailure> {
    *counter = counter
        .checked_add(amount)
        .ok_or(ProbeCoordinatorFailure::ResourceCountOverflow { resource })?;
    Ok(())
}
