//! Owned, non-authoritative probe outcomes and error classification.

mod classify;

use super::super::super::super::{
    CampaignError, CampaignModularProbe, FreshTaskEpoch, FreshTaskQuery,
    SampledDeclaredModuleDualError, SourceDiscoveryError,
};
use super::super::super::{
    ProbeLocalBudgetCause, ProbeLocalBudgetStop, ProbeLocalIterationDisposition,
    ProbeLocalIterationRecord, ProbeLocalOutcome, ProbeLocalProbeReport, ProbeLocalRejection,
    ProbeLocalStage, ProbeLocalStopContext,
};
use classify::{campaign_budget_cause, sampled_dual_budget_cause, source_budget_cause};

pub(super) fn iteration_record(
    epoch: &FreshTaskEpoch,
    query: &FreshTaskQuery<'_>,
    disposition: ProbeLocalIterationDisposition,
) -> ProbeLocalIterationRecord {
    let build = epoch.telemetry();
    let queried = query.telemetry();
    ProbeLocalIterationRecord::new(
        build.epoch_ordinal(),
        build.request_count(),
        build.physical_rows(),
        build.physical_columns(),
        build.physical_entries(),
        queried.allowed_columns(),
        queried.forbidden_columns(),
        queried.forbidden_rank(),
        queried.augmented_rank(),
        query.sampled().sample_fingerprint().clone(),
        disposition,
    )
}

pub(super) fn finish_probe(
    probe_ordinal: usize,
    probe: CampaignModularProbe,
    records: Vec<ProbeLocalIterationRecord>,
    outcome: ProbeLocalOutcome,
) -> ProbeLocalProbeReport {
    ProbeLocalProbeReport::new(probe_ordinal, probe, records, outcome)
}

pub(super) fn unexecuted_suffix_report(
    probe_ordinal: usize,
    probe: CampaignModularProbe,
    triggering_probe_ordinal: usize,
    resource: &'static str,
) -> ProbeLocalProbeReport {
    let stop = ProbeLocalBudgetStop::new(
        probe_ordinal,
        0,
        ProbeLocalStage::UnexecutedAggregateSuffix,
        ProbeLocalBudgetCause::UnexecutedAggregateSuffix {
            triggering_probe_ordinal,
            resource,
        },
    );
    finish_probe(
        probe_ordinal,
        probe,
        Vec::new(),
        ProbeLocalOutcome::BudgetStop {
            context: ProbeLocalStopContext::BeforeBootstrap,
            stop,
        },
    )
}

pub(super) fn campaign_stop_or_rejection(
    probe_ordinal: usize,
    epoch_ordinal: usize,
    stage: ProbeLocalStage,
    context: ProbeLocalStopContext,
    error: CampaignError,
) -> ProbeLocalOutcome {
    match campaign_budget_cause(&error) {
        Some(cause) => ProbeLocalOutcome::BudgetStop {
            context,
            stop: ProbeLocalBudgetStop::new(probe_ordinal, epoch_ordinal, stage, cause),
        },
        None => ProbeLocalOutcome::Rejected {
            context,
            stage,
            error: ProbeLocalRejection::Campaign(error),
        },
    }
}

pub(super) fn source_stop_or_rejection(
    probe_ordinal: usize,
    epoch_ordinal: usize,
    stage: ProbeLocalStage,
    context: ProbeLocalStopContext,
    error: SourceDiscoveryError,
) -> ProbeLocalOutcome {
    match source_budget_cause(&error) {
        Some(cause) => ProbeLocalOutcome::BudgetStop {
            context,
            stop: ProbeLocalBudgetStop::new(probe_ordinal, epoch_ordinal, stage, cause),
        },
        None => ProbeLocalOutcome::Rejected {
            context,
            stage,
            error: ProbeLocalRejection::SourceDiscovery(error),
        },
    }
}

pub(super) fn sampled_dual_stop_or_rejection(
    probe_ordinal: usize,
    epoch_ordinal: usize,
    context: ProbeLocalStopContext,
    error: SampledDeclaredModuleDualError,
) -> ProbeLocalOutcome {
    match sampled_dual_budget_cause(&error) {
        Some(cause) => ProbeLocalOutcome::BudgetStop {
            context,
            stop: ProbeLocalBudgetStop::new(
                probe_ordinal,
                epoch_ordinal,
                ProbeLocalStage::SampledDualAdmission,
                cause,
            ),
        },
        None => ProbeLocalOutcome::Rejected {
            context,
            stage: ProbeLocalStage::SampledDualAdmission,
            error: ProbeLocalRejection::SampledDual(error),
        },
    }
}
