use crate::foundry::completion::source_discovery::scheduler::{
    ProbeLocalObstructionScheduler, ProbeLocalOutcomeKind,
};
use crate::foundry::completion::stratum::{CampaignStratumAnchor, ImmutableOwnerSnapshot};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::OrderingPolicy;

use super::super::{
    CampaignModularProbe, CanonicalRebaseAttemptOutcome, CanonicalReplayDisposition,
    ExactExecutableOwnerProposal, InitialParentSourceProposal, try_canonicalize_replayed_probes,
    try_compile_canonical_executable_owner,
};
use super::support::try_extract_support;
use super::{
    InteriorReplayAttemptCensus, InteriorReplayBudgetStopSummary, InteriorReplayRunDisposition,
    InteriorReplayRunError, InteriorReplayRunLimits, InteriorReplaySchedulerOutcomeCensus,
    InteriorReplayTaskReport,
};

/// Run one independent target through scheduler, common-plan replay, and
/// exact owner compilation before any epoch-bound scheduler payload is
/// dropped. The returned value cannot mutate or certify an owner cover.
#[allow(clippy::too_many_arguments)]
pub(crate) fn try_run_interior_replay_task(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    target: IntegralShift,
    stratum_anchor: impl Into<CampaignStratumAnchor>,
    owners: ImmutableOwnerSnapshot,
    ordering: OrderingPolicy,
    probes: impl IntoIterator<Item = CampaignModularProbe>,
    limits: InteriorReplayRunLimits,
) -> Result<InteriorReplayTaskReport, InteriorReplayRunError> {
    try_run_interior_replay_task_internal(
        generator,
        completed,
        target,
        stratum_anchor.into(),
        owners,
        ordering,
        None,
        probes,
        limits,
    )
}

/// As [`try_run_interior_replay_task`], with one authority-minimal parent
/// request proposal injected independently into each probe's epoch zero.
#[allow(clippy::too_many_arguments)]
pub(crate) fn try_run_interior_replay_task_with_initial_parent_proposal(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    target: IntegralShift,
    stratum_anchor: impl Into<CampaignStratumAnchor>,
    owners: ImmutableOwnerSnapshot,
    ordering: OrderingPolicy,
    initial_parent_proposal: InitialParentSourceProposal,
    probes: impl IntoIterator<Item = CampaignModularProbe>,
    limits: InteriorReplayRunLimits,
) -> Result<InteriorReplayTaskReport, InteriorReplayRunError> {
    try_run_interior_replay_task_internal(
        generator,
        completed,
        target,
        stratum_anchor.into(),
        owners,
        ordering,
        Some(initial_parent_proposal),
        probes,
        limits,
    )
}

#[allow(clippy::too_many_arguments)]
fn try_run_interior_replay_task_internal(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    target: IntegralShift,
    stratum_anchor: CampaignStratumAnchor,
    owners: ImmutableOwnerSnapshot,
    ordering: OrderingPolicy,
    initial_parent_proposal: Option<InitialParentSourceProposal>,
    probes: impl IntoIterator<Item = CampaignModularProbe>,
    limits: InteriorReplayRunLimits,
) -> Result<InteriorReplayTaskReport, InteriorReplayRunError> {
    let scheduler = match initial_parent_proposal {
        None => ProbeLocalObstructionScheduler::try_new(
            generator,
            completed,
            target.clone(),
            stratum_anchor.clone(),
            owners.clone(),
            ordering,
            probes,
            limits.scheduler,
        ),
        Some(proposal) => ProbeLocalObstructionScheduler::try_new_with_initial_parent_proposal(
            generator,
            completed,
            target.clone(),
            stratum_anchor.clone(),
            owners.clone(),
            ordering,
            proposal,
            probes,
            limits.scheduler,
        ),
    }?;
    let scheduler_report = scheduler.run()?;
    let scheduler = scheduler_report.census();
    let scheduler_outcomes = scheduler_outcome_census(&scheduler_report);
    let budget_stops = scheduler_report
        .probes()
        .iter()
        .filter_map(|probe| probe.outcome().budget_stop())
        .map(InteriorReplayBudgetStopSummary::from_stop)
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let first_scheduler_rejection = scheduler_report
        .probes()
        .iter()
        .find_map(|probe| probe.outcome().rejection_summary());

    // This borrow is the critical streaming boundary: canonical replay
    // reconstructs a fresh common epoch before `scheduler_report` is dropped.
    let canonical = try_canonicalize_replayed_probes(
        generator,
        completed,
        target.clone(),
        stratum_anchor,
        owners,
        ordering,
        &scheduler_report,
        limits.canonical_replay,
    )?;

    let (replay, canonical_attempts, disposition) = match canonical {
        CanonicalReplayDisposition::NoReplayedNominations => (
            None,
            InteriorReplayAttemptCensus::default(),
            InteriorReplayRunDisposition::NoReplayedNominations,
        ),
        CanonicalReplayDisposition::NoRebasedCircuits {
            epoch: _,
            attempts,
            telemetry,
        } => {
            let attempt_census = attempt_census(&attempts);
            (
                Some(telemetry),
                attempt_census,
                InteriorReplayRunDisposition::NoRebasedCircuits {
                    replay: telemetry,
                    attempts: attempt_census,
                },
            )
        }
        CanonicalReplayDisposition::Rebased(batch) => {
            let telemetry = batch.telemetry();
            let attempt_census = attempt_census(batch.attempts());
            let proposal =
                try_compile_canonical_executable_owner(generator.context(), batch, limits.owner)?;
            let support = match &proposal {
                ExactExecutableOwnerProposal::Compiled { owner, .. } => {
                    Some(try_extract_support(owner, &target, limits)?)
                }
                ExactExecutableOwnerProposal::Incomplete(_) => None,
            };
            (
                Some(telemetry),
                attempt_census,
                InteriorReplayRunDisposition::OwnerProposal { proposal, support },
            )
        }
    };

    // All old probe-local epochs/circuits die here. A compiled or incomplete
    // owner proposal independently owns only the fresh common replay epoch.
    drop(scheduler_report);
    Ok(InteriorReplayTaskReport::new(
        scheduler,
        scheduler_outcomes,
        budget_stops,
        first_scheduler_rejection,
        replay,
        canonical_attempts,
        disposition,
    ))
}

fn scheduler_outcome_census(
    report: &crate::foundry::completion::source_discovery::scheduler::ProbeLocalSchedulerReport,
) -> InteriorReplaySchedulerOutcomeCensus {
    let mut census = InteriorReplaySchedulerOutcomeCensus::default();
    for probe in report.probes() {
        match probe.outcome().kind() {
            ProbeLocalOutcomeKind::Replayed => census.increment_replayed(),
            ProbeLocalOutcomeKind::SupportDidNotLift => census.increment_support_did_not_lift(),
            ProbeLocalOutcomeKind::ExactLiftError => census.increment_exact_lift_error(),
            ProbeLocalOutcomeKind::SampledDual => census.increment_sampled_dual(),
            ProbeLocalOutcomeKind::BudgetStop => census.increment_budget_stop(),
            ProbeLocalOutcomeKind::Rejected => census.increment_rejected(),
            ProbeLocalOutcomeKind::Stalled => census.increment_stalled(),
        }
    }
    census
}

fn attempt_census(
    attempts: &[super::super::CanonicalRebaseAttempt],
) -> InteriorReplayAttemptCensus {
    let mut census = InteriorReplayAttemptCensus::default();
    for attempt in attempts {
        match attempt.outcome() {
            CanonicalRebaseAttemptOutcome::Replayed => census.increment_replayed(),
            CanonicalRebaseAttemptOutcome::NoModularHit { .. } => census.increment_no_modular_hit(),
            CanonicalRebaseAttemptOutcome::QueryRejected(_) => census.increment_query_rejected(),
            CanonicalRebaseAttemptOutcome::SupportDidNotLift(_) => {
                census.increment_support_did_not_lift()
            }
        }
    }
    census
}
