use crate::foundry::completion::source_discovery::cover_delta::ExactOwnerCoverSnapshot;

use super::super::{ProbeCampaignNoProposal, ProbeCampaignOutcome, ProbeCampaignTaskReport};
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

pub(super) fn try_compact_report(
    report: &ProbeCampaignTaskReport,
) -> Result<CompactTaskResult, ProbeCoordinatorFailure> {
    let census = report.census();
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

    let action = match report.outcome() {
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
    };
    let mut compact = CompactTaskResult {
        action,
        evidence: CompactProbeEvidence {
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
        },
    };
    compact.evidence.declared_probes = try_scheduler_outcome_total(compact.evidence)?;
    Ok(compact)
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

pub(super) fn try_record_compact(
    census: &mut ProbeCoordinatorCensus,
    requested_report: usize,
    compact: CompactTaskResult,
) -> Result<(), ProbeCoordinatorFailure> {
    let mut updated = *census;
    updated.task_reports = requested_report;
    match compact.action {
        CompactTaskAction::NoProposal => try_increment(&mut updated.no_proposal, CENSUS)?,
        CompactTaskAction::Duplicate => try_increment(&mut updated.duplicate, CENSUS)?,
        CompactTaskAction::IncompleteProposal => {
            try_increment(&mut updated.incomplete_proposal, CENSUS)?;
        }
        CompactTaskAction::OwnerSetChanged {
            mutation: ProbeCoordinatorOwnerMutation::ChangedWithoutGeometricShrink,
            ..
        } => try_increment(&mut updated.changed_without_geometric_shrink, CENSUS)?,
        CompactTaskAction::OwnerSetChanged {
            mutation: ProbeCoordinatorOwnerMutation::StrictGeometricShrink,
            ..
        } => try_increment(&mut updated.strict_geometric_shrink, CENSUS)?,
        CompactTaskAction::CompilerClosed { .. } => {
            try_increment(&mut updated.compiler_closed, CENSUS)?;
        }
    }
    try_add(
        &mut updated.scheduler_budget_stops,
        compact.evidence.scheduler_budget_stops,
        CENSUS,
    )?;
    try_add(
        &mut updated.scheduler_rejections,
        compact.evidence.scheduler_rejections,
        CENSUS,
    )?;
    try_add(
        &mut updated.scheduler_stalls,
        compact.evidence.scheduler_stalls,
        CENSUS,
    )?;
    try_add(
        &mut updated.scheduler_exact_lift_errors,
        compact.evidence.scheduler_exact_lift_errors,
        CENSUS,
    )?;
    try_add(
        &mut updated.canonical_replayed,
        compact.evidence.canonical_replayed,
        CENSUS,
    )?;
    try_add(
        &mut updated.canonical_no_modular_hit,
        compact.evidence.canonical_no_modular_hit,
        CENSUS,
    )?;
    try_add(
        &mut updated.canonical_query_rejections,
        compact.evidence.canonical_query_rejections,
        CENSUS,
    )?;
    try_add(
        &mut updated.canonical_support_did_not_lift,
        compact.evidence.canonical_support_did_not_lift,
        CENSUS,
    )?;
    try_add(
        &mut updated.exact_obstructions,
        compact.evidence.exact_obstructions,
        CENSUS,
    )?;
    try_add(
        &mut updated.declared_probes,
        compact.evidence.declared_probes,
        CENSUS,
    )?;
    try_add(
        &mut updated.scheduler_replayed,
        compact.evidence.scheduler_replayed,
        CENSUS,
    )?;
    try_add(
        &mut updated.scheduler_support_did_not_lift,
        compact.evidence.scheduler_support_did_not_lift,
        CENSUS,
    )?;
    try_add(
        &mut updated.scheduler_sampled_dual,
        compact.evidence.scheduler_sampled_dual,
        CENSUS,
    )?;
    *census = updated;
    Ok(())
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
    if evidence.scheduler_support_did_not_lift != 0 || evidence.canonical_support_did_not_lift != 0
    {
        return Some(ProbeCoordinatorNeedsRefinementReason::SupportDidNotLift {
            scheduler_support_did_not_lift: evidence.scheduler_support_did_not_lift,
            canonical_support_did_not_lift: evidence.canonical_support_did_not_lift,
        });
    }
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
