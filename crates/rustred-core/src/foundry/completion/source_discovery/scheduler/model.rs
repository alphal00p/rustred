use std::sync::Arc;

use crate::foundry::completion::frame::exact::{
    ExactCircuitError, ExactCircuitSupportDidNotLift, ExactTargetCircuit,
};
use crate::foundry::completion::frame::modular::ModularSampleFingerprint;
use crate::identity::TranslatedSourceRequest;

use super::super::{
    AccumulatedSourceRequests, CampaignBudgetExhaustion, CampaignError, CampaignModularProbe,
    CandidateBatchExhaustionTelemetry, FreshTaskEpoch, SampledDeclaredModuleDual,
    SampledDeclaredModuleDualError, SourceDiscoveryError,
};

/// Exact scheduler boundary at which a probe-local campaign stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeLocalStage {
    UnexecutedAggregateSuffix,
    BootstrapNomination,
    BootstrapAccumulation,
    EpochAdmission,
    EpochBuild,
    ModularQuery,
    ObstructionNomination,
    ResidualEvaluation,
    RequestMerge,
    SampledDualAdmission,
    ExactLift,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeLocalBudgetScope {
    Probe,
    Aggregate,
}

/// Resource cause for a resumable probe-local stop.
///
/// A budget stop is telemetry only and can never be interpreted as a
/// no-relation result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProbeLocalBudgetCause {
    Outer {
        scope: ProbeLocalBudgetScope,
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    CountOverflow {
        scope: ProbeLocalBudgetScope,
        resource: &'static str,
    },
    AllocationFailure {
        scope: ProbeLocalBudgetScope,
        resource: &'static str,
        requested: usize,
    },
    Campaign(CampaignBudgetExhaustion),
    SourceDiscovery {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    UnexecutedAggregateSuffix {
        triggering_probe_ordinal: usize,
        resource: &'static str,
    },
}

impl ProbeLocalBudgetCause {
    pub(crate) const fn resource(&self) -> &'static str {
        match self {
            Self::Outer { resource, .. }
            | Self::CountOverflow { resource, .. }
            | Self::AllocationFailure { resource, .. }
            | Self::SourceDiscovery { resource, .. }
            | Self::UnexecutedAggregateSuffix { resource, .. } => resource,
            Self::Campaign(exhaustion) => exhaustion.resource(),
        }
    }

    pub(crate) const fn scope(&self) -> ProbeLocalBudgetScope {
        match self {
            Self::Outer { scope, .. }
            | Self::CountOverflow { scope, .. }
            | Self::AllocationFailure { scope, .. } => *scope,
            Self::UnexecutedAggregateSuffix { .. } => ProbeLocalBudgetScope::Aggregate,
            Self::Campaign(_) | Self::SourceDiscovery { .. } => ProbeLocalBudgetScope::Probe,
        }
    }
}

/// Typed resumable stop, including the exact declared probe and local epoch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProbeLocalBudgetStop {
    probe_ordinal: usize,
    epoch_ordinal: usize,
    stage: ProbeLocalStage,
    cause: ProbeLocalBudgetCause,
}

impl ProbeLocalBudgetStop {
    pub(crate) const fn probe_ordinal(&self) -> usize {
        self.probe_ordinal
    }

    pub(crate) const fn epoch_ordinal(&self) -> usize {
        self.epoch_ordinal
    }

    pub(crate) const fn stage(&self) -> ProbeLocalStage {
        self.stage
    }

    pub(crate) const fn cause(&self) -> &ProbeLocalBudgetCause {
        &self.cause
    }

    pub(super) const fn new(
        probe_ordinal: usize,
        epoch_ordinal: usize,
        stage: ProbeLocalStage,
        cause: ProbeLocalBudgetCause,
    ) -> Self {
        Self {
            probe_ordinal,
            epoch_ordinal,
            stage,
            cause,
        }
    }
}

/// Owned context retained by a non-authoritative stop.
///
/// Any diagnostic containing physical ordinals is paired with the exact epoch
/// that gives those ordinals meaning. Stops before a plan exists retain only
/// the probe-local request accumulator (or the pre-bootstrap state).
#[derive(Debug)]
pub(crate) enum ProbeLocalStopContext {
    BeforeBootstrap,
    Requests(AccumulatedSourceRequests),
    Epoch(FreshTaskEpoch),
}

impl ProbeLocalStopContext {
    pub(crate) const fn epoch(&self) -> Option<&FreshTaskEpoch> {
        match self {
            Self::Epoch(epoch) => Some(epoch),
            Self::BeforeBootstrap | Self::Requests(_) => None,
        }
    }

    pub(crate) fn requests(&self) -> Option<&AccumulatedSourceRequests> {
        match self {
            Self::BeforeBootstrap => None,
            Self::Requests(requests) => Some(requests),
            Self::Epoch(epoch) => Some(epoch.requests()),
        }
    }
}

/// Non-resource failure local to one declared probe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProbeLocalRejection {
    Campaign(CampaignError),
    SourceDiscovery(SourceDiscoveryError),
    SampledDual(SampledDeclaredModuleDualError),
}

/// A nonzero exhaustive residual batch contained no novel request.
///
/// This is an internal campaign stall, never sampled-dual or closure evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProbeLocalStall {
    probe_ordinal: usize,
    epoch_ordinal: usize,
    nonzero_residual_requests: usize,
    exhaustion: CandidateBatchExhaustionTelemetry,
}

impl ProbeLocalStall {
    pub(crate) const fn probe_ordinal(&self) -> usize {
        self.probe_ordinal
    }

    pub(crate) const fn epoch_ordinal(&self) -> usize {
        self.epoch_ordinal
    }

    pub(crate) const fn nonzero_residual_requests(&self) -> usize {
        self.nonzero_residual_requests
    }

    pub(crate) const fn exhaustion(&self) -> CandidateBatchExhaustionTelemetry {
        self.exhaustion
    }

    pub(super) const fn new(
        probe_ordinal: usize,
        epoch_ordinal: usize,
        nonzero_residual_requests: usize,
        exhaustion: CandidateBatchExhaustionTelemetry,
    ) -> Self {
        Self {
            probe_ordinal,
            epoch_ordinal,
            nonzero_residual_requests,
            exhaustion,
        }
    }
}

/// Ordinal-free result of one successfully sampled fresh epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeLocalIterationDisposition {
    ModularHit,
    NoHitAugmented {
        nominated_requests: usize,
        nonzero_residual_requests: usize,
        added_requests: usize,
    },
    NoHitEmptyResidual {
        nominated_requests: usize,
    },
    NoHitStalled {
        nominated_requests: usize,
        nonzero_residual_requests: usize,
    },
    NoHitStopped {
        stage: ProbeLocalStage,
    },
}

/// Owned scalar/sample telemetry for one plan-local query.
///
/// Physical row and column ordinals are deliberately absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProbeLocalIterationRecord {
    epoch_ordinal: usize,
    request_count: usize,
    physical_rows: usize,
    physical_columns: usize,
    physical_entries: usize,
    allowed_columns: usize,
    forbidden_columns: usize,
    forbidden_rank: usize,
    augmented_rank: usize,
    sample: Arc<ModularSampleFingerprint>,
    disposition: ProbeLocalIterationDisposition,
}

impl ProbeLocalIterationRecord {
    pub(crate) const fn epoch_ordinal(&self) -> usize {
        self.epoch_ordinal
    }

    pub(crate) const fn request_count(&self) -> usize {
        self.request_count
    }

    pub(crate) const fn physical_rows(&self) -> usize {
        self.physical_rows
    }

    pub(crate) const fn physical_columns(&self) -> usize {
        self.physical_columns
    }

    pub(crate) const fn physical_entries(&self) -> usize {
        self.physical_entries
    }

    pub(crate) const fn allowed_columns(&self) -> usize {
        self.allowed_columns
    }

    pub(crate) const fn forbidden_columns(&self) -> usize {
        self.forbidden_columns
    }

    pub(crate) const fn forbidden_rank(&self) -> usize {
        self.forbidden_rank
    }

    pub(crate) const fn augmented_rank(&self) -> usize {
        self.augmented_rank
    }

    pub(crate) const fn sample_fingerprint(&self) -> &Arc<ModularSampleFingerprint> {
        &self.sample
    }

    pub(crate) const fn disposition(&self) -> ProbeLocalIterationDisposition {
        self.disposition
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) const fn new(
        epoch_ordinal: usize,
        request_count: usize,
        physical_rows: usize,
        physical_columns: usize,
        physical_entries: usize,
        allowed_columns: usize,
        forbidden_columns: usize,
        forbidden_rank: usize,
        augmented_rank: usize,
        sample: Arc<ModularSampleFingerprint>,
        disposition: ProbeLocalIterationDisposition,
    ) -> Self {
        Self {
            epoch_ordinal,
            request_count,
            physical_rows,
            physical_columns,
            physical_entries,
            allowed_columns,
            forbidden_columns,
            forbidden_rank,
            augmented_rank,
            sample,
            disposition,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeLocalOutcomeKind {
    Replayed,
    SupportDidNotLift,
    ExactLiftError,
    SampledDual,
    BudgetStop,
    Rejected,
    Stalled,
}

/// One terminal outcome for a single independent probe-local campaign.
///
/// Replayed circuits remain proposals only. Sampled duals remain sampled
/// evidence only. This enum has no conversion into completion authority.
#[derive(Debug)]
pub(crate) enum ProbeLocalOutcome {
    Replayed {
        epoch: FreshTaskEpoch,
        circuit: ExactTargetCircuit,
    },
    SupportDidNotLift {
        epoch: FreshTaskEpoch,
        inconclusive: ExactCircuitSupportDidNotLift,
    },
    ExactLiftError {
        epoch: FreshTaskEpoch,
        error: ExactCircuitError,
    },
    SampledDual(SampledDeclaredModuleDual),
    BudgetStop {
        context: ProbeLocalStopContext,
        stop: ProbeLocalBudgetStop,
    },
    Rejected {
        context: ProbeLocalStopContext,
        stage: ProbeLocalStage,
        error: ProbeLocalRejection,
    },
    Stalled {
        epoch: FreshTaskEpoch,
        stall: ProbeLocalStall,
    },
}

impl ProbeLocalOutcome {
    pub(crate) const fn kind(&self) -> ProbeLocalOutcomeKind {
        match self {
            Self::Replayed { .. } => ProbeLocalOutcomeKind::Replayed,
            Self::SupportDidNotLift { .. } => ProbeLocalOutcomeKind::SupportDidNotLift,
            Self::ExactLiftError { .. } => ProbeLocalOutcomeKind::ExactLiftError,
            Self::SampledDual(_) => ProbeLocalOutcomeKind::SampledDual,
            Self::BudgetStop { .. } => ProbeLocalOutcomeKind::BudgetStop,
            Self::Rejected { .. } => ProbeLocalOutcomeKind::Rejected,
            Self::Stalled { .. } => ProbeLocalOutcomeKind::Stalled,
        }
    }

    pub(crate) const fn epoch(&self) -> Option<&FreshTaskEpoch> {
        match self {
            Self::Replayed { epoch, .. }
            | Self::SupportDidNotLift { epoch, .. }
            | Self::ExactLiftError { epoch, .. }
            | Self::Stalled { epoch, .. } => Some(epoch),
            Self::BudgetStop { context, .. } | Self::Rejected { context, .. } => context.epoch(),
            Self::SampledDual(_) => None,
        }
    }

    pub(crate) fn final_requests(&self) -> Option<&[TranslatedSourceRequest]> {
        match self {
            Self::Replayed { epoch, .. }
            | Self::SupportDidNotLift { epoch, .. }
            | Self::ExactLiftError { epoch, .. }
            | Self::Stalled { epoch, .. } => Some(epoch.requests().requests()),
            Self::SampledDual(evidence) => Some(evidence.final_requests()),
            Self::BudgetStop { context, .. } | Self::Rejected { context, .. } => {
                context.requests().map(AccumulatedSourceRequests::requests)
            }
        }
    }

    pub(crate) const fn replayed(&self) -> Option<&ExactTargetCircuit> {
        match self {
            Self::Replayed { circuit, .. } => Some(circuit),
            _ => None,
        }
    }

    pub(crate) const fn sampled_dual(&self) -> Option<&SampledDeclaredModuleDual> {
        match self {
            Self::SampledDual(evidence) => Some(evidence),
            _ => None,
        }
    }

    pub(crate) const fn budget_stop(&self) -> Option<&ProbeLocalBudgetStop> {
        match self {
            Self::BudgetStop { stop, .. } => Some(stop),
            _ => None,
        }
    }
}

/// Ordered result for one declared modular probe.
#[derive(Debug)]
pub(crate) struct ProbeLocalProbeReport {
    probe_ordinal: usize,
    probe: CampaignModularProbe,
    iterations: Box<[ProbeLocalIterationRecord]>,
    outcome: ProbeLocalOutcome,
}

impl ProbeLocalProbeReport {
    pub(crate) const fn probe_ordinal(&self) -> usize {
        self.probe_ordinal
    }

    pub(crate) const fn modulus(&self) -> u64 {
        self.probe.modulus()
    }

    pub(crate) fn base_parameters(&self) -> &[i64] {
        self.probe.base_parameters()
    }

    pub(crate) fn chart_coordinates(&self) -> &[u64] {
        self.probe.chart_coordinates()
    }

    pub(crate) fn iterations(&self) -> &[ProbeLocalIterationRecord] {
        &self.iterations
    }

    pub(crate) const fn outcome(&self) -> &ProbeLocalOutcome {
        &self.outcome
    }

    pub(super) fn new(
        probe_ordinal: usize,
        probe: CampaignModularProbe,
        iterations: Vec<ProbeLocalIterationRecord>,
        outcome: ProbeLocalOutcome,
    ) -> Self {
        Self {
            probe_ordinal,
            probe,
            iterations: iterations.into_boxed_slice(),
            outcome,
        }
    }
}

/// Aggregate scalar work census. It contains no probe-local identities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProbeLocalRunCensus {
    epochs: usize,
    epoch_request_work: usize,
    materialized_source_terms: usize,
    modular_entry_work: usize,
    merge_request_work: usize,
    retained_iteration_records: usize,
    exact_lift_attempts: usize,
}

impl ProbeLocalRunCensus {
    pub(crate) const fn epochs(self) -> usize {
        self.epochs
    }

    pub(crate) const fn epoch_request_work(self) -> usize {
        self.epoch_request_work
    }

    pub(crate) const fn retained_iteration_records(self) -> usize {
        self.retained_iteration_records
    }

    pub(crate) const fn materialized_source_terms(self) -> usize {
        self.materialized_source_terms
    }

    pub(crate) const fn modular_entry_work(self) -> usize {
        self.modular_entry_work
    }

    pub(crate) const fn merge_request_work(self) -> usize {
        self.merge_request_work
    }

    pub(crate) const fn exact_lift_attempts(self) -> usize {
        self.exact_lift_attempts
    }

    pub(super) const fn new(
        epochs: usize,
        epoch_request_work: usize,
        materialized_source_terms: usize,
        modular_entry_work: usize,
        merge_request_work: usize,
        retained_iteration_records: usize,
        exact_lift_attempts: usize,
    ) -> Self {
        Self {
            epochs,
            epoch_request_work,
            materialized_source_terms,
            modular_entry_work,
            merge_request_work,
            retained_iteration_records,
            exact_lift_attempts,
        }
    }
}

/// Complete ordered outer-scheduler result.
#[derive(Debug)]
pub(crate) struct ProbeLocalSchedulerReport {
    probes: Box<[ProbeLocalProbeReport]>,
    census: ProbeLocalRunCensus,
}

impl ProbeLocalSchedulerReport {
    pub(crate) fn probes(&self) -> &[ProbeLocalProbeReport] {
        &self.probes
    }

    pub(crate) const fn census(&self) -> ProbeLocalRunCensus {
        self.census
    }

    pub(super) fn new(probes: Vec<ProbeLocalProbeReport>, census: ProbeLocalRunCensus) -> Self {
        Self {
            probes: probes.into_boxed_slice(),
            census,
        }
    }
}
