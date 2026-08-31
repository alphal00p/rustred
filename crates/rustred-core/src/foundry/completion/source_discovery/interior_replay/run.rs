use crate::foundry::completion::source_discovery::scheduler::{
    ProbeLocalObstructionScheduler, ProbeLocalOutcomeKind,
};
use crate::foundry::completion::stratum::{ImmutableOwnerSnapshot, MaximalStratumAnchor};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::OrderingPolicy;

use super::super::{
    CampaignModularProbe, CanonicalRebaseAttemptOutcome, CanonicalReplayDisposition,
    ExactExecutableOwnerProposal, try_canonicalize_replayed_probes,
    try_compile_canonical_executable_owner,
};
use super::support::try_extract_support;
use super::{
    InteriorReplayAttemptCensus, InteriorReplayRunDisposition, InteriorReplayRunError,
    InteriorReplayRunLimits, InteriorReplaySchedulerOutcomeCensus, InteriorReplayTaskReport,
};

/// Run one independent target through scheduler, common-plan replay, and
/// exact owner compilation before any epoch-bound scheduler payload is
/// dropped. The returned value cannot mutate or certify an owner cover.
#[allow(clippy::too_many_arguments)]
pub(crate) fn try_run_interior_replay_task(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    target: IntegralShift,
    maximal_anchor: MaximalStratumAnchor,
    owners: ImmutableOwnerSnapshot,
    ordering: OrderingPolicy,
    probes: impl IntoIterator<Item = CampaignModularProbe>,
    limits: InteriorReplayRunLimits,
) -> Result<InteriorReplayTaskReport, InteriorReplayRunError> {
    let scheduler_report = ProbeLocalObstructionScheduler::try_new(
        generator,
        completed,
        target.clone(),
        maximal_anchor.clone(),
        owners.clone(),
        ordering,
        probes,
        limits.scheduler,
    )?
    .run()?;
    let scheduler = scheduler_report.census();
    let scheduler_outcomes = scheduler_outcome_census(&scheduler_report);

    // This borrow is the critical streaming boundary: canonical replay
    // reconstructs a fresh common epoch before `scheduler_report` is dropped.
    let canonical = try_canonicalize_replayed_probes(
        generator,
        completed,
        target.clone(),
        maximal_anchor,
        owners,
        ordering,
        &scheduler_report,
        limits.canonical_replay,
    )?;

    let (replay, disposition) = match canonical {
        CanonicalReplayDisposition::NoReplayedNominations => {
            (None, InteriorReplayRunDisposition::NoReplayedNominations)
        }
        CanonicalReplayDisposition::NoRebasedCircuits {
            epoch: _,
            attempts,
            telemetry,
        } => {
            let attempt_census = attempt_census(&attempts);
            (
                Some(telemetry),
                InteriorReplayRunDisposition::NoRebasedCircuits {
                    replay: telemetry,
                    attempts: attempt_census,
                },
            )
        }
        CanonicalReplayDisposition::Rebased(batch) => {
            let telemetry = batch.telemetry();
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
        replay,
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
