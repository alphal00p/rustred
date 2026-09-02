use std::cmp::Ordering;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::completion::frame::SelectedSourceFrame;
use crate::foundry::completion::stratum::{
    DecoratedStratum, StratumRegistryLimits, TargetColumnPartition,
};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, TranslatedSourceRequest,
};

use super::error::{frame_error, modular_error, stratum_error, translated_error};
use super::{
    AccumulatedSourceRequests, CampaignBudgetExhaustion, CampaignError, CampaignLimits,
    CampaignModularProbe, CampaignRequestMerge, CampaignRequestMergeTelemetry,
    CampaignResourceStage, CandidateBatchExhaustionTelemetry, FreshTaskBuildTelemetry,
    FreshTaskEpoch, FreshTaskQuery, FreshTaskQueryTelemetry, GrowingTaskEpochState,
    ReusedTaskPartitionQuery,
};

const SUBMITTED_REQUESTS: &str = "campaign submitted source requests";
const CANONICAL_REQUESTS: &str = "campaign canonical candidate requests";
const ACCUMULATED_REQUESTS: &str = "campaign accumulated source requests";
const REQUEST_COORDINATES: &str = "campaign retained request coordinate cells";
const MERGE_COMPARISONS: &str = "campaign stable request merge comparisons";
const RETAINED_PROBE_COORDINATES: &str = "campaign retained raw probe coordinates";
const EXACT_PROBE_ANCHOR_COORDINATES: &str = "campaign exact probe anchor coordinates";
const GROWING_EPOCH_ORDINAL: &str = "growing campaign epoch ordinal";

impl AccumulatedSourceRequests {
    /// Canonicalize one bounded finite request collection.
    ///
    /// Empty state is representable for the target-unit bootstrap boundary,
    /// but [`FreshTaskEpoch::try_new`] rejects it because a physical frame
    /// cannot be built without exact rows.
    pub(crate) fn try_new(
        arity: usize,
        requests: impl IntoIterator<Item = TranslatedSourceRequest>,
        limits: CampaignLimits,
    ) -> Result<Self, CampaignError> {
        validate_arity(arity, limits)?;
        let (canonical, _, _) = canonicalize_candidates(arity, requests, limits)?;
        check_budget(
            CampaignResourceStage::RequestAccumulation,
            ACCUMULATED_REQUESTS,
            canonical.len(),
            limits.max_accumulated_requests,
        )?;
        check_coordinate_budget(arity, canonical.len(), limits)?;
        Ok(Self::from_canonical(arity, canonical))
    }

    /// Stable-merge one finite candidate batch into a new immutable state.
    ///
    /// Candidate order and duplicates do not affect the resulting state. An
    /// unchanged result reports only exhaustion of this supplied batch; it
    /// cannot certify residual or translated-module exhaustion.
    pub(crate) fn try_merge_candidates(
        &self,
        candidates: impl IntoIterator<Item = TranslatedSourceRequest>,
        limits: CampaignLimits,
    ) -> Result<CampaignRequestMerge, CampaignError> {
        validate_arity(self.arity(), limits)?;
        if self.requests().windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(CampaignError::Invariant {
                detail: "accumulated campaign requests are not canonical and unique",
            });
        }
        check_budget(
            CampaignResourceStage::RequestAccumulation,
            ACCUMULATED_REQUESTS,
            self.len(),
            limits.max_accumulated_requests,
        )?;
        check_coordinate_budget(self.arity(), self.len(), limits)?;

        let (canonical, submitted, duplicate_candidates) =
            canonicalize_candidates(self.arity(), candidates, limits)?;
        let mut existing_ordinal = 0usize;
        let mut candidate_ordinal = 0usize;
        let mut comparisons = 0usize;
        let mut already_accumulated = 0usize;
        let mut added_requests = 0usize;
        while existing_ordinal < self.len() && candidate_ordinal < canonical.len() {
            comparisons = checked_add(MERGE_COMPARISONS, comparisons, 1)?;
            check_budget(
                CampaignResourceStage::RequestAccumulation,
                MERGE_COMPARISONS,
                comparisons,
                limits.max_merge_comparisons,
            )?;
            match self.requests()[existing_ordinal].cmp(&canonical[candidate_ordinal]) {
                Ordering::Less => existing_ordinal += 1,
                Ordering::Equal => {
                    existing_ordinal += 1;
                    candidate_ordinal += 1;
                    already_accumulated =
                        checked_add(ACCUMULATED_REQUESTS, already_accumulated, 1)?;
                }
                Ordering::Greater => {
                    candidate_ordinal += 1;
                    added_requests = checked_add(ACCUMULATED_REQUESTS, added_requests, 1)?;
                }
            }
        }
        added_requests = checked_add(
            ACCUMULATED_REQUESTS,
            added_requests,
            canonical.len() - candidate_ordinal,
        )?;
        let merged_count = checked_add(ACCUMULATED_REQUESTS, self.len(), added_requests)?;
        check_budget(
            CampaignResourceStage::RequestAccumulation,
            ACCUMULATED_REQUESTS,
            merged_count,
            limits.max_accumulated_requests,
        )?;
        check_coordinate_budget(self.arity(), merged_count, limits)?;
        let telemetry = CampaignRequestMergeTelemetry::new(
            submitted,
            canonical.len(),
            duplicate_candidates,
            already_accumulated,
            added_requests,
            merged_count,
            comparisons,
        );
        if added_requests == 0 {
            return Ok(CampaignRequestMerge::CandidateBatchExhausted(
                CandidateBatchExhaustionTelemetry::new(telemetry),
            ));
        }

        let mut merged = try_vec(ACCUMULATED_REQUESTS, merged_count)?;
        let mut existing_ordinal = 0usize;
        let mut candidate_ordinal = 0usize;
        while existing_ordinal < self.len() && candidate_ordinal < canonical.len() {
            match self.requests()[existing_ordinal].cmp(&canonical[candidate_ordinal]) {
                Ordering::Less => {
                    merged.push(self.requests()[existing_ordinal].clone());
                    existing_ordinal += 1;
                }
                Ordering::Equal => {
                    merged.push(self.requests()[existing_ordinal].clone());
                    existing_ordinal += 1;
                    candidate_ordinal += 1;
                }
                Ordering::Greater => {
                    merged.push(canonical[candidate_ordinal].clone());
                    candidate_ordinal += 1;
                }
            }
        }
        merged.extend_from_slice(&self.requests()[existing_ordinal..]);
        merged.extend_from_slice(&canonical[candidate_ordinal..]);
        if merged.len() != merged_count || merged.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(CampaignError::Invariant {
                detail: "stable campaign request merge violated its exact census or order",
            });
        }
        Ok(CampaignRequestMerge::Augmented {
            requests: Self::from_canonical(self.arity(), merged),
            telemetry,
        })
    }
}

impl CampaignModularProbe {
    /// Retain the original integer probe inputs under one aggregate cap.
    pub(crate) fn try_new(
        modulus: u64,
        base_parameters: impl IntoIterator<Item = i64>,
        chart_coordinates: impl IntoIterator<Item = u64>,
        limits: CampaignLimits,
    ) -> Result<Self, CampaignError> {
        let mut base = Vec::new();
        let mut chart = Vec::new();
        for value in base_parameters {
            let requested = checked_add(RETAINED_PROBE_COORDINATES, base.len(), 1)?;
            check_budget(
                CampaignResourceStage::ModularQuery,
                RETAINED_PROBE_COORDINATES,
                requested,
                limits.max_retained_probe_coordinates,
            )?;
            base.try_reserve(1)
                .map_err(|_| CampaignError::AllocationFailure {
                    resource: RETAINED_PROBE_COORDINATES,
                    requested,
                })?;
            base.push(value);
        }
        for value in chart_coordinates {
            let requested = checked_add(
                RETAINED_PROBE_COORDINATES,
                base.len(),
                checked_add(RETAINED_PROBE_COORDINATES, chart.len(), 1)?,
            )?;
            check_budget(
                CampaignResourceStage::ModularQuery,
                RETAINED_PROBE_COORDINATES,
                requested,
                limits.max_retained_probe_coordinates,
            )?;
            chart
                .try_reserve(1)
                .map_err(|_| CampaignError::AllocationFailure {
                    resource: RETAINED_PROBE_COORDINATES,
                    requested,
                })?;
            chart.push(value);
        }
        Ok(Self::from_parts(modulus, base, chart))
    }
}

impl FreshTaskEpoch {
    /// Materialize a new immutable physical plan from the complete accumulated
    /// request set on one intentionally fixed decorated stratum.
    ///
    /// This one-shot boundary never widens or refreshes a caller-selected
    /// tightened domain. Growing campaigns use [`GrowingTaskEpochState`].
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        epoch_ordinal: usize,
        generator: &ParametricIbpGenerator<'_>,
        completed: &CompletedIbpSourceRows,
        requests: AccumulatedSourceRequests,
        target_shift: IntegralShift,
        stratum: DecoratedStratum,
        owners: crate::foundry::completion::stratum::ImmutableOwnerSnapshot,
        ordering: crate::sector::OrderingPolicy,
        limits: CampaignLimits,
    ) -> Result<Self, CampaignError> {
        Self::try_new_with_stratum(
            epoch_ordinal,
            generator,
            completed,
            requests,
            target_shift,
            EpochStratumInput::Fixed(stratum),
            owners,
            ordering,
            limits,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn try_new_with_stratum(
        epoch_ordinal: usize,
        generator: &ParametricIbpGenerator<'_>,
        completed: &CompletedIbpSourceRows,
        requests: AccumulatedSourceRequests,
        target_shift: IntegralShift,
        stratum_input: EpochStratumInput<'_>,
        owners: crate::foundry::completion::stratum::ImmutableOwnerSnapshot,
        ordering: crate::sector::OrderingPolicy,
        limits: CampaignLimits,
    ) -> Result<Self, CampaignError> {
        if !completed.is_complete_ordinary() {
            return Err(CampaignError::WrongSourceLayout {
                actual: completed.layout_name(),
            });
        }
        if requests.is_empty() {
            return Err(CampaignError::EmptyAccumulatedRequests);
        }
        validate_arity(requests.arity(), limits)?;
        check_budget(
            CampaignResourceStage::RequestAccumulation,
            ACCUMULATED_REQUESTS,
            requests.len(),
            limits.max_accumulated_requests,
        )?;
        check_coordinate_budget(requests.arity(), requests.len(), limits)?;
        if target_shift.len() != requests.arity() {
            return Err(CampaignError::WrongTargetArity {
                expected: requests.arity(),
                actual: target_shift.len(),
            });
        }
        if stratum_input.arity() != requests.arity() {
            return Err(CampaignError::WrongTargetArity {
                expected: requests.arity(),
                actual: stratum_input.arity(),
            });
        }

        let selected = generator
            .translate_selected_completed_source_rows(
                completed,
                requests.requests().iter().cloned(),
                limits.translated_sources,
            )
            .map_err(translated_error)?;
        if !selected.is_complete_ordinary() {
            return Err(CampaignError::WrongSourceLayout {
                actual: selected.source_layout_name(),
            });
        }
        validate_fixed_scope(&selected, &requests, stratum_input.scope(), &owners)?;
        let frame = SelectedSourceFrame::try_new(
            selected,
            stratum_input.scope().domain().sector().clone(),
            limits.physical_frame,
        )
        .map_err(frame_error)?;
        let plan = frame.plan();
        let target_column = plan
            .columns()
            .binary_search(&target_shift)
            .map_err(|_| CampaignError::TargetColumnAbsent)?;
        if plan.columns()[target_column] != target_shift {
            return Err(CampaignError::Invariant {
                detail: "fresh campaign target lookup did not recover the raw target shift",
            });
        }
        let stratum = stratum_input
            .try_materialize(plan, target_column, limits.stratum)
            .map_err(stratum_error)?;
        let telemetry = FreshTaskBuildTelemetry::new(
            epoch_ordinal,
            requests.len(),
            plan.row_count(),
            plan.columns().len(),
            plan.entry_count(),
            target_column,
        );
        Ok(Self::from_parts(
            requests,
            frame,
            target_shift,
            target_column,
            stratum,
            owners,
            ordering,
            telemetry,
        ))
    }

    /// Rebuild the exhaustive target partition on this exact immutable epoch.
    /// Promotion uses the same boundary after a modular query has gone out of
    /// scope; no physical ordinal is trusted without this reconstruction.
    pub(crate) fn try_partition(
        &self,
        limits: StratumRegistryLimits,
    ) -> Result<TargetColumnPartition<'_>, CampaignError> {
        // The owner snapshot was installed once through a checked immutable
        // constructor. Reuse that exact Arc-backed ID/route authority for
        // every partition in this epoch; only an explicit cold audit should
        // reconstruct the complete snapshot payload again.
        TargetColumnPartition::try_new_with_verified_snapshot(
            self.plan(),
            self.target_column(),
            self.fixed_stratum().clone(),
            self.owners().verified_clone(),
            self.ordering(),
            limits,
        )
        .map_err(stratum_error)
    }

    /// Recover the exact integral-index anchor represented by one retained
    /// raw probe, authenticated against this epoch's fixed stratum.
    pub(crate) fn try_anchor_for_probe(
        &self,
        probe: &CampaignModularProbe,
    ) -> Result<Box<[i64]>, CampaignError> {
        validate_probe_in_fixed_stratum(self.fixed_stratum(), probe)?;
        let mut anchor = try_vec(
            EXACT_PROBE_ANCHOR_COORDINATES,
            self.fixed_stratum().domain().arity(),
        )?;
        for (position, (&coordinate, &active)) in probe
            .chart_coordinates()
            .iter()
            .zip(self.fixed_stratum().domain().sector().active_bits())
            .enumerate()
        {
            anchor.push(try_exact_probe_index(position, active, coordinate)?);
        }
        Ok(anchor.into_boxed_slice())
    }

    /// Rebuild the exact target partition on this plan, resample from the
    /// retained original integer probe inputs, and query the target span.
    pub(crate) fn try_query<'epoch>(
        &'epoch self,
        context: &IndexedCoefficientContext,
        probe: &CampaignModularProbe,
        limits: CampaignLimits,
    ) -> Result<FreshTaskQuery<'epoch>, CampaignError> {
        self.try_query_with_obstruction_rotation(context, probe, 0, limits)
    }

    /// As [`Self::try_query`], with a deterministic proposal-only rotation of
    /// auxiliary right-kernel directions. The primary q0 obstruction and all
    /// sampled-dual authority are independent of this scheduling input.
    pub(crate) fn try_query_with_obstruction_rotation<'epoch>(
        &'epoch self,
        context: &IndexedCoefficientContext,
        probe: &CampaignModularProbe,
        obstruction_rotation: usize,
        limits: CampaignLimits,
    ) -> Result<FreshTaskQuery<'epoch>, CampaignError> {
        validate_probe_in_fixed_stratum(self.fixed_stratum(), probe)?;
        let partition = self.try_partition(limits.stratum)?;
        let sampled = self
            .plan()
            .try_modular_sample(
                context,
                probe.modulus(),
                probe.base_parameters(),
                probe.chart_coordinates(),
                limits.modular,
            )
            .map_err(modular_error)?;
        let query = sampled
            .query_target_with_obstruction_rotation(
                partition.target_column(),
                partition.forbidden_columns(),
                obstruction_rotation,
                limits.modular,
            )
            .map_err(modular_error)?;
        let diagnostics = query.diagnostics();
        let telemetry = FreshTaskQueryTelemetry::new(
            partition.allowed_columns().len(),
            partition.forbidden_columns().len(),
            diagnostics.forbidden_rank,
            diagnostics.augmented_rank,
        );
        Ok(FreshTaskQuery::new(
            partition,
            sampled,
            query,
            probe.clone(),
            telemetry,
        ))
    }

    /// Reuse one already authenticated exact partition across independent
    /// modular probes of this immutable epoch.
    ///
    /// Only cheap pointer and identity joins are repeated. Each call still
    /// constructs a fresh sample and target query, so no modular values or
    /// obstructions are shared between probes.
    pub(crate) fn try_query_with_partition<'partition, 'epoch>(
        &'epoch self,
        context: &IndexedCoefficientContext,
        probe: &CampaignModularProbe,
        partition: &'partition TargetColumnPartition<'epoch>,
        limits: CampaignLimits,
    ) -> Result<ReusedTaskPartitionQuery<'partition, 'epoch>, CampaignError> {
        if context.fingerprint() != self.plan().context_fingerprint()
            || !std::ptr::eq(partition.frame(), self.plan())
            || partition.target_column() != self.target_column()
            || partition.stratum_id() != self.fixed_stratum().id()
            || partition.snapshot_id() != self.fixed_snapshot_id()
            || partition.ordering() != self.fixed_ordering()
        {
            return Err(CampaignError::FixedTaskScopeMismatch {
                detail: "reused target partition differs from its immutable campaign epoch",
            });
        }
        validate_probe_in_fixed_stratum(self.fixed_stratum(), probe)?;
        let sampled = self
            .plan()
            .try_modular_sample(
                context,
                probe.modulus(),
                probe.base_parameters(),
                probe.chart_coordinates(),
                limits.modular,
            )
            .map_err(modular_error)?;
        let query = sampled
            .query_target(
                partition.target_column(),
                partition.forbidden_columns(),
                limits.modular,
            )
            .map_err(modular_error)?;
        let diagnostics = query.diagnostics();
        let telemetry = FreshTaskQueryTelemetry::new(
            partition.allowed_columns().len(),
            partition.forbidden_columns().len(),
            diagnostics.forbidden_rank,
            diagnostics.augmented_rank,
        );
        Ok(ReusedTaskPartitionQuery::new(
            partition,
            sampled,
            query,
            probe.clone(),
            telemetry,
        ))
    }

    /// Build an intentionally caller-selected projection for adversarial
    /// evidence-boundary tests. Production queries always use [`Self::try_query`].
    #[cfg(test)]
    pub(crate) fn projected_query_for_test<'epoch>(
        &'epoch self,
        context: &IndexedCoefficientContext,
        probe: &CampaignModularProbe,
        projected_target: usize,
        projected_forbidden: &[usize],
        limits: CampaignLimits,
    ) -> FreshTaskQuery<'epoch> {
        let partition = TargetColumnPartition::try_new(
            self.plan(),
            self.target_column(),
            self.fixed_stratum().clone(),
            self.owners().clone(),
            self.ordering(),
            limits.stratum,
        )
        .unwrap();
        let sampled = self
            .plan()
            .try_modular_sample(
                context,
                probe.modulus(),
                probe.base_parameters(),
                probe.chart_coordinates(),
                limits.modular,
            )
            .unwrap();
        let query = sampled
            .query_target(projected_target, projected_forbidden, limits.modular)
            .unwrap();
        let diagnostics = query.diagnostics();
        let telemetry = FreshTaskQueryTelemetry::new(
            partition.allowed_columns().len(),
            partition.forbidden_columns().len(),
            diagnostics.forbidden_rank,
            diagnostics.augmented_rank,
        );
        FreshTaskQuery::new(partition, sampled, query, probe.clone(), telemetry)
    }
}

impl GrowingTaskEpochState {
    /// Materialize the next growing epoch and advance the proof sequence only
    /// after the complete fresh frame has been accepted.
    pub(crate) fn try_next(
        &mut self,
        generator: &ParametricIbpGenerator<'_>,
        completed: &CompletedIbpSourceRows,
        requests: AccumulatedSourceRequests,
        limits: CampaignLimits,
    ) -> Result<FreshTaskEpoch, CampaignError> {
        validate_growing_request_chronology(self.previous_requests(), &requests)?;
        let epoch_ordinal = self.next_epoch_ordinal();
        let next_epoch_ordinal =
            epoch_ordinal
                .checked_add(1)
                .ok_or(CampaignError::ResourceCountOverflow {
                    resource: GROWING_EPOCH_ORDINAL,
                })?;
        let retained_requests = requests.clone();
        let target_shift = self.target_shift().clone();
        let owners = self.owners().clone();
        let ordering = self.ordering();
        let epoch = FreshTaskEpoch::try_new_with_stratum(
            epoch_ordinal,
            generator,
            completed,
            requests,
            target_shift,
            EpochStratumInput::Growing(self.strata_mut()),
            owners,
            ordering,
            limits,
        )?;
        self.commit(retained_requests, next_epoch_ordinal);
        Ok(epoch)
    }
}

enum EpochStratumInput<'state> {
    Fixed(DecoratedStratum),
    Growing(&'state mut crate::foundry::completion::stratum::CampaignStratumSequence),
}

impl EpochStratumInput<'_> {
    fn scope(&self) -> &DecoratedStratum {
        match self {
            Self::Fixed(stratum) => stratum,
            Self::Growing(sequence) => sequence.scope(),
        }
    }

    fn arity(&self) -> usize {
        self.scope().domain().arity()
    }

    fn try_materialize(
        self,
        frame: &crate::foundry::completion::frame::PhysicalFramePlan,
        target_column: usize,
        limits: crate::foundry::completion::stratum::StratumRegistryLimits,
    ) -> Result<DecoratedStratum, crate::foundry::completion::stratum::StratumRegistryError> {
        match self {
            Self::Fixed(stratum) => Ok(stratum),
            Self::Growing(sequence) => sequence.try_materialize(frame, target_column, limits),
        }
    }
}

fn validate_growing_request_chronology(
    previous: Option<&AccumulatedSourceRequests>,
    current: &AccumulatedSourceRequests,
) -> Result<(), CampaignError> {
    let Some(previous) = previous else {
        return Ok(());
    };
    if current.arity() != previous.arity() || current.len() <= previous.len() {
        return Err(CampaignError::NonMonotoneGrowingRequests {
            previous: previous.len(),
            current: current.len(),
        });
    }

    let mut current_ordinal = 0usize;
    for old in previous.requests() {
        while current_ordinal < current.len() && current.requests()[current_ordinal] < *old {
            current_ordinal += 1;
        }
        if current.requests().get(current_ordinal) != Some(old) {
            return Err(CampaignError::NonMonotoneGrowingRequests {
                previous: previous.len(),
                current: current.len(),
            });
        }
        current_ordinal += 1;
    }
    Ok(())
}

fn validate_fixed_scope(
    selected: &crate::identity::SelectedTranslatedSourceBatch,
    requests: &AccumulatedSourceRequests,
    stratum: &DecoratedStratum,
    owners: &crate::foundry::completion::stratum::ImmutableOwnerSnapshot,
) -> Result<(), CampaignError> {
    if selected.family_fingerprint() != stratum.family_fingerprint() {
        return Err(CampaignError::FixedTaskScopeMismatch {
            detail: "selected sources and decorated stratum belong to different families",
        });
    }
    if selected.context_fingerprint() != stratum.context_fingerprint() {
        return Err(CampaignError::FixedTaskScopeMismatch {
            detail: "selected sources and decorated stratum use different coefficient contexts",
        });
    }
    if owners.family_fingerprint() != stratum.family_fingerprint() {
        return Err(CampaignError::FixedTaskScopeMismatch {
            detail: "immutable owners and decorated stratum belong to different families",
        });
    }
    if owners.context_fingerprint() != stratum.context_fingerprint() {
        return Err(CampaignError::FixedTaskScopeMismatch {
            detail: "immutable owners and decorated stratum use different coefficient contexts",
        });
    }
    if owners.arity() != requests.arity() {
        return Err(CampaignError::FixedTaskScopeMismatch {
            detail: "immutable owner snapshot has the wrong task arity",
        });
    }
    if selected.completed_source_row_count() == 0
        || selected.requests() != requests.requests()
        || selected.sources().len() != requests.len()
        || selected
            .requests()
            .iter()
            .any(|request| request.source_ordinal() >= selected.completed_source_row_count())
        || selected
            .sources()
            .iter()
            .zip(selected.requests())
            .any(|(source, request)| {
                source.provenance().source_ordinal() != request.source_ordinal()
                    || source.provenance().offset() != request.offset()
            })
    {
        return Err(CampaignError::SourceChronologyMismatch);
    }
    Ok(())
}

fn canonicalize_candidates(
    arity: usize,
    candidates: impl IntoIterator<Item = TranslatedSourceRequest>,
    limits: CampaignLimits,
) -> Result<(Vec<TranslatedSourceRequest>, usize, usize), CampaignError> {
    let mut canonical = Vec::new();
    let mut submitted = 0usize;
    for candidate in candidates {
        let request_ordinal = submitted;
        submitted = checked_add(SUBMITTED_REQUESTS, submitted, 1)?;
        check_budget(
            CampaignResourceStage::RequestAccumulation,
            SUBMITTED_REQUESTS,
            submitted,
            limits.max_submitted_requests,
        )?;
        if candidate.offset().len() != arity {
            return Err(CampaignError::WrongRequestArity {
                request_ordinal,
                expected: arity,
                actual: candidate.offset().len(),
            });
        }
        canonical
            .try_reserve(1)
            .map_err(|_| CampaignError::AllocationFailure {
                resource: CANONICAL_REQUESTS,
                requested: submitted,
            })?;
        canonical.push(candidate);
    }
    canonical.sort_unstable();
    canonical.dedup();
    check_budget(
        CampaignResourceStage::RequestAccumulation,
        CANONICAL_REQUESTS,
        canonical.len(),
        limits.max_canonical_candidate_requests,
    )?;
    let duplicate_candidates = submitted - canonical.len();
    Ok((canonical, submitted, duplicate_candidates))
}

fn validate_probe_in_fixed_stratum(
    stratum: &DecoratedStratum,
    probe: &CampaignModularProbe,
) -> Result<(), CampaignError> {
    let expected = stratum.domain().arity();
    if probe.chart_coordinates().len() != expected {
        return Err(CampaignError::WrongProbeChartArity {
            expected,
            actual: probe.chart_coordinates().len(),
        });
    }
    for (position, ((&coordinate, &active), &bounds)) in probe
        .chart_coordinates()
        .iter()
        .zip(stratum.domain().sector().active_bits())
        .zip(stratum.domain().bounds())
        .enumerate()
    {
        let index = try_exact_probe_index(position, active, coordinate)?;
        if !bounds.contains(index) {
            return Err(CampaignError::SampleOutsideFixedStratum {
                position,
                index,
                lower: bounds.lower(),
                upper: bounds.upper(),
            });
        }
    }
    Ok(())
}

fn try_exact_probe_index(
    position: usize,
    active: bool,
    coordinate: u64,
) -> Result<i64, CampaignError> {
    if active {
        let coordinate = i64::try_from(coordinate).map_err(|_| {
            CampaignError::SampleCoordinateNotRepresentable {
                position,
                active,
                coordinate,
            }
        })?;
        coordinate
            .checked_add(1)
            .ok_or(CampaignError::SampleCoordinateNotRepresentable {
                position,
                active,
                coordinate: coordinate as u64,
            })
    } else if coordinate == (i64::MAX as u64) + 1 {
        Ok(i64::MIN)
    } else {
        let coordinate = i64::try_from(coordinate).map_err(|_| {
            CampaignError::SampleCoordinateNotRepresentable {
                position,
                active,
                coordinate,
            }
        })?;
        Ok(-coordinate)
    }
}

fn validate_arity(arity: usize, limits: CampaignLimits) -> Result<(), CampaignError> {
    if arity == 0 {
        return Err(CampaignError::EmptyRequestArity);
    }
    check_budget(
        CampaignResourceStage::RequestAccumulation,
        "campaign request arity",
        arity,
        limits.max_request_arity,
    )
}

fn check_coordinate_budget(
    arity: usize,
    request_count: usize,
    limits: CampaignLimits,
) -> Result<(), CampaignError> {
    let coordinate_cells =
        arity
            .checked_mul(request_count)
            .ok_or(CampaignError::ResourceCountOverflow {
                resource: REQUEST_COORDINATES,
            })?;
    check_budget(
        CampaignResourceStage::RequestAccumulation,
        REQUEST_COORDINATES,
        coordinate_cells,
        limits.max_request_coordinate_cells,
    )
}

fn check_budget(
    stage: CampaignResourceStage,
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), CampaignError> {
    if requested > limit {
        Err(CampaignError::BudgetExhausted(
            CampaignBudgetExhaustion::new(stage, resource, requested, limit),
        ))
    } else {
        Ok(())
    }
}

fn checked_add(resource: &'static str, left: usize, right: usize) -> Result<usize, CampaignError> {
    left.checked_add(right)
        .ok_or(CampaignError::ResourceCountOverflow { resource })
}

fn try_vec<T>(resource: &'static str, capacity: usize) -> Result<Vec<T>, CampaignError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| CampaignError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}
