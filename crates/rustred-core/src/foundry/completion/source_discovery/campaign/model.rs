use std::sync::Arc;

use crate::foundry::completion::frame::modular::{
    ModularPhysicalFrame, ModularRightObstruction, ModularTargetQuery,
};
use crate::foundry::completion::frame::{PhysicalFramePlan, SelectedSourceFrame};
use crate::foundry::completion::stratum::{
    CampaignStratumAnchor, CampaignStratumSequence, DecoratedStratum, ImmutableOwnerSnapshot,
    ImmutableOwnerSnapshotId, TargetColumnPartition,
};
use crate::identity::{IntegralShift, TranslatedSourceRequest};
use crate::sector::OrderingPolicy;

/// Immutable canonical request set accumulated across obstruction epochs.
///
/// Requests are strictly ordered by their exact signed offset and stable
/// source ordinal. This value contains no modular validity or residual claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AccumulatedSourceRequests {
    arity: usize,
    requests: Arc<[TranslatedSourceRequest]>,
}

impl AccumulatedSourceRequests {
    pub(crate) const fn arity(&self) -> usize {
        self.arity
    }

    pub(crate) fn requests(&self) -> &[TranslatedSourceRequest] {
        &self.requests
    }

    pub(crate) fn len(&self) -> usize {
        self.requests.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.requests, &other.requests)
    }

    pub(super) fn from_canonical(arity: usize, requests: Vec<TranslatedSourceRequest>) -> Self {
        debug_assert!(requests.windows(2).all(|pair| pair[0] < pair[1]));
        Self {
            arity,
            requests: Arc::from(requests),
        }
    }
}

/// Exact counters from one stable canonical request merge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CampaignRequestMergeTelemetry {
    submitted_candidates: usize,
    canonical_candidates: usize,
    duplicate_candidates: usize,
    already_accumulated: usize,
    added_requests: usize,
    merged_request_count: usize,
    merge_comparisons: usize,
}

impl CampaignRequestMergeTelemetry {
    pub(crate) const fn submitted_candidates(&self) -> usize {
        self.submitted_candidates
    }

    pub(crate) const fn canonical_candidates(&self) -> usize {
        self.canonical_candidates
    }

    pub(crate) const fn duplicate_candidates(&self) -> usize {
        self.duplicate_candidates
    }

    pub(crate) const fn already_accumulated(&self) -> usize {
        self.already_accumulated
    }

    pub(crate) const fn added_requests(&self) -> usize {
        self.added_requests
    }

    pub(crate) const fn merged_request_count(&self) -> usize {
        self.merged_request_count
    }

    pub(crate) const fn merge_comparisons(&self) -> usize {
        self.merge_comparisons
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) const fn new(
        submitted_candidates: usize,
        canonical_candidates: usize,
        duplicate_candidates: usize,
        already_accumulated: usize,
        added_requests: usize,
        merged_request_count: usize,
        merge_comparisons: usize,
    ) -> Self {
        Self {
            submitted_candidates,
            canonical_candidates,
            duplicate_candidates,
            already_accumulated,
            added_requests,
            merged_request_count,
            merge_comparisons,
        }
    }
}

/// A finite candidate batch contained no request absent from the accumulator.
///
/// This telemetry does **not** certify exhaustive residual evaluation, a
/// sampled module dual, or a terminal. It is intentionally incapable of
/// entering any owner API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CandidateBatchExhaustionTelemetry {
    merge: CampaignRequestMergeTelemetry,
}

impl CandidateBatchExhaustionTelemetry {
    pub(crate) const fn merge(&self) -> CampaignRequestMergeTelemetry {
        self.merge
    }

    pub(super) const fn new(merge: CampaignRequestMergeTelemetry) -> Self {
        Self { merge }
    }
}

/// Result of a pure immutable request-set augmentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CampaignRequestMerge {
    Augmented {
        requests: AccumulatedSourceRequests,
        telemetry: CampaignRequestMergeTelemetry,
    },
    CandidateBatchExhausted(CandidateBatchExhaustionTelemetry),
}

/// Stateful materialization boundary for one growing target task.
///
/// The target, maximal or exactly restricted stratum proof sequence, immutable
/// owners, and ordering are captured once. Epoch ordinals are derived
/// internally, and every later request set must be a strict canonical
/// superset of the preceding one.
#[derive(Debug)]
pub(crate) struct GrowingTaskEpochState {
    target_shift: IntegralShift,
    strata: CampaignStratumSequence,
    owners: ImmutableOwnerSnapshot,
    ordering: OrderingPolicy,
    previous_requests: Option<AccumulatedSourceRequests>,
    next_epoch_ordinal: usize,
}

impl GrowingTaskEpochState {
    pub(crate) fn new(
        target_shift: IntegralShift,
        stratum: impl Into<CampaignStratumAnchor>,
        owners: ImmutableOwnerSnapshot,
        ordering: OrderingPolicy,
    ) -> Self {
        Self {
            target_shift,
            strata: stratum.into().into_sequence(),
            owners,
            ordering,
            previous_requests: None,
            next_epoch_ordinal: 0,
        }
    }

    pub(super) const fn target_shift(&self) -> &IntegralShift {
        &self.target_shift
    }

    pub(super) const fn strata_mut(&mut self) -> &mut CampaignStratumSequence {
        &mut self.strata
    }

    pub(super) const fn owners(&self) -> &ImmutableOwnerSnapshot {
        &self.owners
    }

    pub(super) const fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub(super) const fn previous_requests(&self) -> Option<&AccumulatedSourceRequests> {
        self.previous_requests.as_ref()
    }

    pub(crate) const fn next_epoch_ordinal(&self) -> usize {
        self.next_epoch_ordinal
    }

    pub(super) fn commit(
        &mut self,
        requests: AccumulatedSourceRequests,
        next_epoch_ordinal: usize,
    ) {
        self.previous_requests = Some(requests);
        self.next_epoch_ordinal = next_epoch_ordinal;
    }
}

/// Original integer inputs for one deterministic modular probe.
///
/// These values are retained verbatim. A later plan rebuild resamples from
/// them and never attempts to recover integer inputs from finite-field
/// residues, which would be ambiguous across representatives and moduli.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CampaignModularProbe {
    modulus: u64,
    base_parameters: Arc<[i64]>,
    chart_coordinates: Arc<[u64]>,
}

impl CampaignModularProbe {
    pub(crate) const fn modulus(&self) -> u64 {
        self.modulus
    }

    pub(crate) fn base_parameters(&self) -> &[i64] {
        &self.base_parameters
    }

    pub(crate) fn chart_coordinates(&self) -> &[u64] {
        &self.chart_coordinates
    }

    pub(super) fn from_parts(
        modulus: u64,
        base_parameters: Vec<i64>,
        chart_coordinates: Vec<u64>,
    ) -> Self {
        Self {
            modulus,
            base_parameters: Arc::from(base_parameters),
            chart_coordinates: Arc::from(chart_coordinates),
        }
    }
}

impl CampaignRequestMerge {
    pub(crate) const fn telemetry(&self) -> CampaignRequestMergeTelemetry {
        match self {
            Self::Augmented { telemetry, .. } => *telemetry,
            Self::CandidateBatchExhausted(exhaustion) => exhaustion.merge(),
        }
    }

    pub(crate) const fn augmented_requests(&self) -> Option<&AccumulatedSourceRequests> {
        match self {
            Self::Augmented { requests, .. } => Some(requests),
            Self::CandidateBatchExhausted(_) => None,
        }
    }
}

/// Deterministic structural telemetry for one freshly materialized plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FreshTaskBuildTelemetry {
    epoch_ordinal: usize,
    request_count: usize,
    physical_rows: usize,
    physical_columns: usize,
    physical_entries: usize,
    target_column: usize,
}

impl FreshTaskBuildTelemetry {
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

    pub(crate) const fn target_column(&self) -> usize {
        self.target_column
    }

    pub(super) const fn new(
        epoch_ordinal: usize,
        request_count: usize,
        physical_rows: usize,
        physical_columns: usize,
        physical_entries: usize,
        target_column: usize,
    ) -> Self {
        Self {
            epoch_ordinal,
            request_count,
            physical_rows,
            physical_columns,
            physical_entries,
            target_column,
        }
    }
}

/// One immutable materialization of a fixed target task.
///
/// Rebuilding this value always creates a fresh [`PhysicalFramePlan`]. A
/// growing scheduler also materializes the maximal decorated stratum for that
/// exact plan; a one-shot caller may deliberately retain a fixed tightened
/// stratum. Ordering and the immutable owner snapshot remain fixed inputs.
#[derive(Debug)]
pub(crate) struct FreshTaskEpoch {
    requests: AccumulatedSourceRequests,
    frame: SelectedSourceFrame,
    target_shift: IntegralShift,
    target_column: usize,
    stratum: DecoratedStratum,
    owners: ImmutableOwnerSnapshot,
    ordering: OrderingPolicy,
    telemetry: FreshTaskBuildTelemetry,
}

impl FreshTaskEpoch {
    pub(crate) const fn plan(&self) -> &PhysicalFramePlan {
        self.frame.plan()
    }

    pub(crate) const fn requests(&self) -> &AccumulatedSourceRequests {
        &self.requests
    }

    pub(crate) const fn target_shift(&self) -> &IntegralShift {
        &self.target_shift
    }

    pub(crate) const fn target_column(&self) -> usize {
        self.target_column
    }

    pub(crate) const fn fixed_stratum(&self) -> &DecoratedStratum {
        &self.stratum
    }

    pub(crate) const fn fixed_snapshot_id(&self) -> &ImmutableOwnerSnapshotId {
        self.owners.id()
    }

    /// The exact immutable lower-sector authority used to build every frame,
    /// partition, and replay in this epoch.  Closure sealing clones this
    /// already-owned snapshot; it must never attempt to recover authority from
    /// [`Self::fixed_snapshot_id`] alone.
    pub(in crate::foundry::completion::source_discovery) const fn predecessor_snapshot(
        &self,
    ) -> &ImmutableOwnerSnapshot {
        &self.owners
    }

    pub(crate) const fn fixed_ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub(super) const fn owners(&self) -> &ImmutableOwnerSnapshot {
        &self.owners
    }

    pub(super) const fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub(crate) const fn telemetry(&self) -> FreshTaskBuildTelemetry {
        self.telemetry
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_parts(
        requests: AccumulatedSourceRequests,
        frame: SelectedSourceFrame,
        target_shift: IntegralShift,
        target_column: usize,
        stratum: DecoratedStratum,
        owners: ImmutableOwnerSnapshot,
        ordering: OrderingPolicy,
        telemetry: FreshTaskBuildTelemetry,
    ) -> Self {
        Self {
            requests,
            frame,
            target_shift,
            target_column,
            stratum,
            owners,
            ordering,
            telemetry,
        }
    }
}

/// Deterministic counters from one fresh partition and modular target query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FreshTaskQueryTelemetry {
    allowed_columns: usize,
    forbidden_columns: usize,
    forbidden_rank: usize,
    augmented_rank: usize,
}

impl FreshTaskQueryTelemetry {
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

    pub(super) const fn new(
        allowed_columns: usize,
        forbidden_columns: usize,
        forbidden_rank: usize,
        augmented_rank: usize,
    ) -> Self {
        Self {
            allowed_columns,
            forbidden_columns,
            forbidden_rank,
            augmented_rank,
        }
    }
}

/// Fresh plan-bound target evidence together with its rebuilt exact partition.
///
/// The query is either an actual modular hit or an explicitly checked,
/// target-normalized right obstruction. Both borrow the same fresh plan as
/// `partition`; neither survives a later plan augmentation by construction.
#[derive(Debug)]
pub(crate) struct FreshTaskQuery<'epoch> {
    partition: TargetColumnPartition<'epoch>,
    sampled: ModularPhysicalFrame<'epoch>,
    query: ModularTargetQuery<'epoch>,
    probe: CampaignModularProbe,
    telemetry: FreshTaskQueryTelemetry,
}

/// One modular query borrowing a caller-retained, already authenticated
/// partition for the same immutable epoch.
///
/// This is the multi-probe path: the expensive exact partition and lower-owner
/// census is built once, while every probe still receives a fresh modular
/// sample and target query. The cheap scope joins are repeated per probe.
#[derive(Debug)]
pub(crate) struct ReusedTaskPartitionQuery<'partition, 'epoch> {
    partition: &'partition TargetColumnPartition<'epoch>,
    sampled: ModularPhysicalFrame<'epoch>,
    query: ModularTargetQuery<'epoch>,
    probe: CampaignModularProbe,
    telemetry: FreshTaskQueryTelemetry,
}

impl<'epoch> FreshTaskQuery<'epoch> {
    pub(crate) const fn partition(&self) -> &TargetColumnPartition<'epoch> {
        &self.partition
    }

    pub(crate) const fn query(&self) -> &ModularTargetQuery<'epoch> {
        &self.query
    }

    pub(crate) const fn sampled(&self) -> &ModularPhysicalFrame<'epoch> {
        &self.sampled
    }

    pub(crate) const fn obstruction(&self) -> Option<&ModularRightObstruction<'epoch>> {
        self.query.obstruction()
    }

    pub(crate) const fn probe(&self) -> &CampaignModularProbe {
        &self.probe
    }

    pub(crate) const fn telemetry(&self) -> FreshTaskQueryTelemetry {
        self.telemetry
    }

    pub(super) const fn new(
        partition: TargetColumnPartition<'epoch>,
        sampled: ModularPhysicalFrame<'epoch>,
        query: ModularTargetQuery<'epoch>,
        probe: CampaignModularProbe,
        telemetry: FreshTaskQueryTelemetry,
    ) -> Self {
        Self {
            partition,
            sampled,
            query,
            probe,
            telemetry,
        }
    }
}

impl<'partition, 'epoch> ReusedTaskPartitionQuery<'partition, 'epoch> {
    pub(crate) const fn partition(&self) -> &'partition TargetColumnPartition<'epoch> {
        self.partition
    }

    pub(crate) const fn query(&self) -> &ModularTargetQuery<'epoch> {
        &self.query
    }

    pub(crate) const fn sampled(&self) -> &ModularPhysicalFrame<'epoch> {
        &self.sampled
    }

    pub(crate) const fn probe(&self) -> &CampaignModularProbe {
        &self.probe
    }

    pub(crate) const fn telemetry(&self) -> FreshTaskQueryTelemetry {
        self.telemetry
    }

    pub(super) const fn new(
        partition: &'partition TargetColumnPartition<'epoch>,
        sampled: ModularPhysicalFrame<'epoch>,
        query: ModularTargetQuery<'epoch>,
        probe: CampaignModularProbe,
        telemetry: FreshTaskQueryTelemetry,
    ) -> Self {
        Self {
            partition,
            sampled,
            query,
            probe,
            telemetry,
        }
    }
}
