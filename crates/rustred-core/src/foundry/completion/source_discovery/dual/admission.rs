use super::*;

impl SampledDeclaredModuleDual {
    /// Construct sampled-dual evidence only from an empty *complete* residual
    /// census for the obstruction carried by this exact fresh query.
    ///
    /// The inverse-incidence nomination is independently repeated under
    /// `limits`.  Equality with the sealed evaluated nominations proves that
    /// every structurally incident row was either evaluated in this census or
    /// excluded because that exact translated-source request was already a
    /// row of the immutable plan.  Any cap, singular evaluation, stale join,
    /// incomplete projection, or nonzero residual returns an error.
    pub(crate) fn try_new(
        incidence: &OrdinarySourceIncidenceIndex<'_>,
        epoch: &FreshTaskEpoch,
        query: &FreshTaskQuery<'_>,
        nominations: &IncidentTranslationNominations,
        residuals: &NonzeroIncidentTranslationResiduals,
        limits: SourceDiscoveryLimits,
    ) -> Result<Self, SampledDeclaredModuleDualError> {
        incidence
            .try_verify_limits(limits)
            .map_err(SampledDeclaredModuleDualError::IncidenceVerification)?;
        validate_incidence_task_scope(incidence, epoch)?;
        if !epoch.fixed_stratum().guards().is_empty() {
            return Err(
                SampledDeclaredModuleDualError::GuardedStratumRequiresSampleWitness {
                    guard_count: epoch.fixed_stratum().guards().len(),
                },
            );
        }
        let partition = query.partition();
        if !partition
            .try_verify()
            .map_err(SampledDeclaredModuleDualError::PartitionVerification)?
        {
            return Err(SampledDeclaredModuleDualError::PartitionNotVerified);
        }
        if !std::ptr::eq(partition.frame(), epoch.plan()) {
            return Err(SampledDeclaredModuleDualError::PartitionPlanMismatch);
        }
        if !std::ptr::eq(query.sampled().plan(), epoch.plan()) {
            return Err(SampledDeclaredModuleDualError::SamplePlanMismatch);
        }
        if partition.stratum() != epoch.fixed_stratum() {
            return Err(SampledDeclaredModuleDualError::FixedStratumMismatch);
        }
        if partition.ordering() != epoch.fixed_ordering() {
            return Err(SampledDeclaredModuleDualError::FixedOrderingMismatch);
        }
        if partition.snapshot_id() != epoch.fixed_snapshot_id() {
            return Err(SampledDeclaredModuleDualError::FixedOwnerSnapshotMismatch);
        }
        validate_target_join(epoch, query)?;
        validate_materialized_rows(incidence, epoch)?;

        let obstruction = match query.query() {
            ModularTargetQuery::Hit(_) => {
                return Err(SampledDeclaredModuleDualError::QueryIsModularHit);
            }
            ModularTargetQuery::NoHitWithObstruction(obstruction) => obstruction,
        };
        if !std::ptr::eq(obstruction.plan(), epoch.plan()) {
            return Err(SampledDeclaredModuleDualError::ObstructionPlanMismatch);
        }
        if !Arc::ptr_eq(
            obstruction.sample_fingerprint(),
            query.sampled().sample_fingerprint(),
        ) {
            return Err(SampledDeclaredModuleDualError::ObstructionSampleMismatch);
        }
        if obstruction.logical_forbidden_columns() != partition.forbidden_columns()
            || obstruction.target_physical_column() != partition.target_column()
        {
            return Err(SampledDeclaredModuleDualError::ObstructionPartitionMismatch);
        }

        if !incidence.owns_identity(nominations.incidence_identity()) {
            return Err(SampledDeclaredModuleDualError::NominationIncidenceMismatch);
        }
        match nominations.origin() {
            IncidentNominationOrigin::TargetUnit => {
                return Err(SampledDeclaredModuleDualError::NominationIsTargetUnit);
            }
            IncidentNominationOrigin::CheckedObstruction(identity)
                if !identity.belongs_to(obstruction) =>
            {
                return Err(SampledDeclaredModuleDualError::NominationObstructionMismatch);
            }
            IncidentNominationOrigin::CheckedObstruction(_) => {}
        }

        let census = residuals.census();
        if !census.belongs_to_nominations(nominations) {
            return Err(SampledDeclaredModuleDualError::ResidualNominationMismatch);
        }
        let incidence_identity = incidence.identity_owner();
        if !census.belongs_to_incidence(&incidence_identity) {
            return Err(SampledDeclaredModuleDualError::ResidualIncidenceMismatch);
        }
        if !census.belongs_to_plan(epoch.plan()) {
            return Err(SampledDeclaredModuleDualError::ResidualPlanMismatch);
        }
        if !census.belongs_to_obstruction(obstruction) {
            return Err(SampledDeclaredModuleDualError::ResidualObstructionMismatch);
        }
        if !census.belongs_to_sample(query.sampled().sample_fingerprint()) {
            return Err(SampledDeclaredModuleDualError::ResidualSampleMismatch);
        }

        // Re-enumeration is the completeness boundary.  It uses the exact
        // incidence support and this obstruction's checked nonzero support,
        // then excludes only sealed rows already present in this plan.
        let expected = incidence
            .try_nominate_obstruction(obstruction, limits)
            .map_err(SampledDeclaredModuleDualError::NominationVerification)?;
        if expected != *nominations {
            return Err(SampledDeclaredModuleDualError::IncompleteNominationCensus);
        }
        validate_residual_telemetry(incidence, obstruction, residuals, &expected, limits)?;
        if !residuals.requests().is_empty() {
            return Err(SampledDeclaredModuleDualError::CuttingResiduals {
                count: residuals.requests().len(),
            });
        }

        let final_requests = try_clone_final_requests(epoch, limits)?;
        let raw_obstruction = try_clone_raw_obstruction(epoch, query, obstruction, limits)?;
        check_limit(
            DUAL_SAMPLE_COORDINATES,
            query.sampled().sample_fingerprint().point().len(),
            limits.max_sampled_dual_sample_coordinates,
        )
        .map_err(SampledDeclaredModuleDualError::Retention)?;
        let rank_census = try_summarize_rank_diagnostics(epoch, obstruction, limits)?;
        let census = SampledDeclaredModuleDualCensus {
            declared_source_rows: incidence.source_count(),
            final_request_count: final_requests.len(),
            raw_incidence_visits: expected.raw_incidence_visits(),
            structurally_incident_rows: expected.unique_before_existing_exclusion(),
            evaluated_unseen_rows: expected.requests().len(),
            already_materialized_incident_rows: expected.excluded_existing_requests(),
            evaluated_source_terms: residuals.evaluated_source_terms(),
            paired_source_terms: residuals.paired_source_terms(),
        };

        Ok(Self {
            _plan_identity: epoch.plan().identity_owner(),
            sample: query.sampled().sample_fingerprint().clone(),
            _obstruction_identity: obstruction.identity_owner(),
            _incidence_identity: incidence_identity,
            target_shift: epoch.target_shift().clone(),
            stratum_id: epoch.fixed_stratum().id().clone(),
            ordering: epoch.fixed_ordering(),
            snapshot_id: epoch.fixed_snapshot_id().clone(),
            final_requests,
            obstruction: raw_obstruction,
            rank_census,
            census,
        })
    }

    pub(crate) const fn sample_fingerprint(&self) -> &Arc<ModularSampleFingerprint> {
        &self.sample
    }

    pub(crate) const fn target_shift(&self) -> &IntegralShift {
        &self.target_shift
    }

    pub(crate) const fn stratum_id(&self) -> &DecoratedStratumId {
        &self.stratum_id
    }

    pub(crate) const fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub(crate) const fn snapshot_id(&self) -> &ImmutableOwnerSnapshotId {
        &self.snapshot_id
    }

    pub(crate) fn final_requests(&self) -> &[TranslatedSourceRequest] {
        &self.final_requests
    }

    pub(crate) fn obstruction(&self) -> &[SampledDeclaredModuleDualObstructionEntry] {
        &self.obstruction
    }

    pub(crate) const fn rank_census(&self) -> SampledDeclaredModuleDualRankCensus {
        self.rank_census
    }

    pub(crate) const fn census(&self) -> SampledDeclaredModuleDualCensus {
        self.census
    }
}

fn validate_incidence_task_scope(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    epoch: &FreshTaskEpoch,
) -> Result<(), SampledDeclaredModuleDualError> {
    if incidence.family_fingerprint() != epoch.plan().family_fingerprint() {
        return Err(SampledDeclaredModuleDualError::IncidenceTaskScopeMismatch {
            detail: "ordinary-source incidence and fresh plan belong to different families",
        });
    }
    if incidence.context_fingerprint() != epoch.plan().context_fingerprint() {
        return Err(SampledDeclaredModuleDualError::IncidenceTaskScopeMismatch {
            detail: "ordinary-source incidence and fresh plan use different coefficient contexts",
        });
    }
    if incidence.arity() != epoch.plan().sector().arity()
        || incidence.arity() != epoch.requests().arity()
        || incidence.arity() != epoch.target_shift().len()
        || incidence.arity() != epoch.fixed_stratum().domain().arity()
    {
        return Err(SampledDeclaredModuleDualError::IncidenceTaskScopeMismatch {
            detail: "ordinary-source incidence and fresh task have different arities",
        });
    }
    Ok(())
}

fn try_clone_final_requests(
    epoch: &FreshTaskEpoch,
    limits: SourceDiscoveryLimits,
) -> Result<Box<[TranslatedSourceRequest]>, SampledDeclaredModuleDualError> {
    let requests = epoch.requests().requests();
    check_limit(
        DUAL_REQUESTS,
        requests.len(),
        limits.max_sampled_dual_requests,
    )
    .map_err(SampledDeclaredModuleDualError::Retention)?;
    let coordinate_cells = requests
        .len()
        .checked_mul(epoch.requests().arity())
        .ok_or_else(|| {
            SampledDeclaredModuleDualError::Retention(SourceDiscoveryError::ResourceCountOverflow {
                resource: DUAL_REQUEST_COORDINATES,
            })
        })?;
    check_limit(
        DUAL_REQUEST_COORDINATES,
        coordinate_cells,
        limits.max_sampled_dual_request_coordinate_cells,
    )
    .map_err(SampledDeclaredModuleDualError::Retention)?;
    if requests.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(SampledDeclaredModuleDualError::MaterializedSourceChronologyMismatch);
    }
    let mut retained = try_vec(DUAL_REQUESTS, requests.len())
        .map_err(SampledDeclaredModuleDualError::Retention)?;
    retained.extend_from_slice(requests);
    Ok(retained.into_boxed_slice())
}

fn try_clone_raw_obstruction(
    epoch: &FreshTaskEpoch,
    query: &FreshTaskQuery<'_>,
    obstruction: &ModularRightObstruction<'_>,
    limits: SourceDiscoveryLimits,
) -> Result<Box<[SampledDeclaredModuleDualObstructionEntry]>, SampledDeclaredModuleDualError> {
    check_limit(
        DUAL_OBSTRUCTION_ENTRIES,
        obstruction.entries().len(),
        limits.max_sampled_dual_obstruction_entries,
    )
    .map_err(SampledDeclaredModuleDualError::Retention)?;
    let coordinate_cells = obstruction
        .entries()
        .len()
        .checked_mul(epoch.target_shift().len())
        .ok_or_else(|| {
            SampledDeclaredModuleDualError::Retention(SourceDiscoveryError::ResourceCountOverflow {
                resource: DUAL_OBSTRUCTION_COORDINATES,
            })
        })?;
    check_limit(
        DUAL_OBSTRUCTION_COORDINATES,
        coordinate_cells,
        limits.max_sampled_dual_obstruction_coordinate_cells,
    )
    .map_err(SampledDeclaredModuleDualError::Retention)?;

    let mut retained = try_vec(DUAL_OBSTRUCTION_ENTRIES, obstruction.entries().len())
        .map_err(SampledDeclaredModuleDualError::Retention)?;
    let mut target_entries = 0usize;
    let mut previous_logical = None;
    for entry in obstruction.entries() {
        if previous_logical.is_some_and(|previous| previous >= entry.logical_column()) {
            return Err(SampledDeclaredModuleDualError::RawObstructionMismatch);
        }
        previous_logical = Some(entry.logical_column());
        let physical = *obstruction
            .logical_physical_columns()
            .get(entry.logical_column())
            .ok_or(SampledDeclaredModuleDualError::RawObstructionMismatch)?;
        let shift = epoch
            .plan()
            .columns()
            .get(physical)
            .ok_or(SampledDeclaredModuleDualError::RawObstructionMismatch)?;
        let target = entry.logical_column() == obstruction.target_logical_column();
        if target {
            target_entries = target_entries.checked_add(1).ok_or_else(|| {
                SampledDeclaredModuleDualError::Retention(
                    SourceDiscoveryError::ResourceCountOverflow {
                        resource: DUAL_OBSTRUCTION_ENTRIES,
                    },
                )
            })?;
            if shift != epoch.target_shift()
                || entry.coefficient() != &query.sampled().field().one()
            {
                return Err(SampledDeclaredModuleDualError::RawObstructionMismatch);
            }
        }
        retained.push(SampledDeclaredModuleDualObstructionEntry {
            shift: shift.clone(),
            coefficient: entry.coefficient().clone(),
            target,
        });
    }
    if target_entries != 1 || retained.is_empty() {
        return Err(SampledDeclaredModuleDualError::RawObstructionMismatch);
    }
    Ok(retained.into_boxed_slice())
}

fn try_summarize_rank_diagnostics(
    epoch: &FreshTaskEpoch,
    obstruction: &ModularRightObstruction<'_>,
    limits: SourceDiscoveryLimits,
) -> Result<SampledDeclaredModuleDualRankCensus, SampledDeclaredModuleDualError> {
    let diagnostics = obstruction.diagnostics();
    let slices = [
        diagnostics.forbidden_columns.as_ref(),
        diagnostics.forbidden_pivot_columns.as_ref(),
        diagnostics.augmented_pivot_columns.as_ref(),
        diagnostics.forbidden_independent_source_rows.as_ref(),
        diagnostics.augmented_independent_source_rows.as_ref(),
    ];
    let mut ordinal_count = 0usize;
    for values in slices {
        ordinal_count = checked_add(DUAL_DIAGNOSTIC_ORDINALS, ordinal_count, values.len())
            .map_err(SampledDeclaredModuleDualError::Retention)?;
        check_limit(
            DUAL_DIAGNOSTIC_ORDINALS,
            ordinal_count,
            limits.max_sampled_dual_diagnostic_ordinals,
        )
        .map_err(SampledDeclaredModuleDualError::Retention)?;
    }

    if diagnostics.target_column != epoch.target_column()
        || diagnostics.forbidden_columns.as_ref() != obstruction.logical_forbidden_columns()
        || diagnostics.forbidden_rank > diagnostics.augmented_rank
        || diagnostics.augmented_rank > diagnostics.forbidden_rank.saturating_add(1)
        || diagnostics.forbidden_pivot_columns.len() != diagnostics.forbidden_rank
        || diagnostics.augmented_pivot_columns.len() != diagnostics.augmented_rank
        || diagnostics.forbidden_independent_source_rows.len() != diagnostics.forbidden_rank
        || diagnostics.augmented_independent_source_rows.len() != diagnostics.augmented_rank
        || diagnostics.forbidden_rank > diagnostics.forbidden_columns.len()
        || diagnostics.augmented_rank > obstruction.logical_physical_columns().len()
    {
        return Err(SampledDeclaredModuleDualError::RankDiagnosticsMismatch {
            detail: "rank, pivot, row, target, or forbidden-column census is inconsistent",
        });
    }
    validate_plan_ordinals(
        "forbidden columns",
        &diagnostics.forbidden_columns,
        epoch.plan().columns().len(),
    )?;
    validate_plan_ordinals(
        "forbidden pivot columns",
        &diagnostics.forbidden_pivot_columns,
        epoch.plan().columns().len(),
    )?;
    validate_plan_ordinals(
        "augmented pivot columns",
        &diagnostics.augmented_pivot_columns,
        epoch.plan().columns().len(),
    )?;
    if diagnostics
        .forbidden_pivot_columns
        .iter()
        .any(|pivot| diagnostics.forbidden_columns.binary_search(pivot).is_err())
    {
        return Err(SampledDeclaredModuleDualError::RankDiagnosticsMismatch {
            detail: "forbidden pivot is outside the physical forbidden projection",
        });
    }
    if diagnostics.augmented_pivot_columns.iter().any(|pivot| {
        *pivot != obstruction.target_physical_column()
            && diagnostics.forbidden_columns.binary_search(pivot).is_err()
    }) {
        return Err(SampledDeclaredModuleDualError::RankDiagnosticsMismatch {
            detail: "augmented pivot is outside the physical target projection",
        });
    }
    validate_plan_ordinals(
        "forbidden independent source rows",
        &diagnostics.forbidden_independent_source_rows,
        epoch.plan().row_count(),
    )?;
    validate_plan_ordinals(
        "augmented independent source rows",
        &diagnostics.augmented_independent_source_rows,
        epoch.plan().row_count(),
    )?;
    let forbidden_fill = diagnostics
        .forbidden_lower_pattern_nonzeros
        .checked_add(diagnostics.forbidden_upper_nonzeros)
        .ok_or(SampledDeclaredModuleDualError::RankDiagnosticsMismatch {
            detail: "forbidden fill census overflowed",
        })?;
    let augmented_fill = diagnostics
        .augmented_lower_pattern_nonzeros
        .checked_add(diagnostics.augmented_upper_nonzeros)
        .ok_or(SampledDeclaredModuleDualError::RankDiagnosticsMismatch {
            detail: "augmented fill census overflowed",
        })?;
    if forbidden_fill != diagnostics.forbidden_total_fill_nonzeros
        || augmented_fill != diagnostics.augmented_total_fill_nonzeros
    {
        return Err(SampledDeclaredModuleDualError::RankDiagnosticsMismatch {
            detail: "rank diagnostic fill totals do not match their exact components",
        });
    }

    Ok(SampledDeclaredModuleDualRankCensus {
        forbidden_columns: diagnostics.forbidden_columns.len(),
        forbidden_rank: diagnostics.forbidden_rank,
        augmented_rank: diagnostics.augmented_rank,
        forbidden_pivot_columns: diagnostics.forbidden_pivot_columns.len(),
        augmented_pivot_columns: diagnostics.augmented_pivot_columns.len(),
        forbidden_independent_source_rows: diagnostics.forbidden_independent_source_rows.len(),
        augmented_independent_source_rows: diagnostics.augmented_independent_source_rows.len(),
        forbidden_input_nonzeros: diagnostics.forbidden_input_nonzeros,
        augmented_input_nonzeros: diagnostics.augmented_input_nonzeros,
        forbidden_lower_pattern_nonzeros: diagnostics.forbidden_lower_pattern_nonzeros,
        augmented_lower_pattern_nonzeros: diagnostics.augmented_lower_pattern_nonzeros,
        forbidden_upper_nonzeros: diagnostics.forbidden_upper_nonzeros,
        augmented_upper_nonzeros: diagnostics.augmented_upper_nonzeros,
        forbidden_total_fill_nonzeros: diagnostics.forbidden_total_fill_nonzeros,
        augmented_total_fill_nonzeros: diagnostics.augmented_total_fill_nonzeros,
    })
}

fn validate_plan_ordinals(
    object: &'static str,
    ordinals: &[usize],
    upper: usize,
) -> Result<(), SampledDeclaredModuleDualError> {
    if ordinals.iter().any(|&ordinal| ordinal >= upper)
        || ordinals.windows(2).any(|pair| pair[0] >= pair[1])
    {
        Err(SampledDeclaredModuleDualError::RankDiagnosticsMismatch { detail: object })
    } else {
        Ok(())
    }
}

fn validate_target_join(
    epoch: &FreshTaskEpoch,
    query: &FreshTaskQuery<'_>,
) -> Result<(), SampledDeclaredModuleDualError> {
    if query.partition().target_column() != epoch.target_column() {
        return Err(SampledDeclaredModuleDualError::TargetColumnMismatch);
    }
    let Some(raw_target) = epoch.plan().columns().get(epoch.target_column()) else {
        return Err(SampledDeclaredModuleDualError::TargetColumnOutOfRange);
    };
    if raw_target != epoch.target_shift() {
        return Err(SampledDeclaredModuleDualError::TargetShiftMismatch);
    }
    Ok(())
}

fn validate_materialized_rows(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    epoch: &FreshTaskEpoch,
) -> Result<(), SampledDeclaredModuleDualError> {
    if epoch.requests().arity() != incidence.arity()
        || epoch.requests().len() != epoch.plan().source_instances().len()
    {
        return Err(SampledDeclaredModuleDualError::MaterializedSourceChronologyMismatch);
    }
    // Selected frames deliberately reorder their physical rows by translation
    // radius and sector-oriented offset. Reconstruct the canonical request
    // chronology from that sealed physical permutation instead of zipping it
    // positionally with the independently offset-major accumulator.
    let mut materialized = try_vec(DUAL_REQUESTS, epoch.plan().source_instances().len())
        .map_err(SampledDeclaredModuleDualError::Retention)?;
    for (row, instance) in epoch.plan().source_instances().iter().enumerate() {
        let provenance = instance.provenance();
        let Some(source) = incidence.sources().get(provenance.source_ordinal()) else {
            return Err(SampledDeclaredModuleDualError::MaterializedSourceChronologyMismatch);
        };
        if provenance.source_row() != source.row_id()
            || epoch
                .plan()
                .source_for_row(row)
                .map(|source| source.provenance())
                != Some(provenance)
        {
            return Err(SampledDeclaredModuleDualError::MaterializedSourceChronologyMismatch);
        }
        materialized.push(TranslatedSourceRequest::new(
            provenance.source_ordinal(),
            provenance.offset().clone(),
        ));
    }
    materialized.sort_unstable();
    if materialized.as_slice() != epoch.requests().requests() {
        return Err(SampledDeclaredModuleDualError::MaterializedSourceChronologyMismatch);
    }
    Ok(())
}

fn validate_residual_telemetry(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    obstruction: &ModularRightObstruction<'_>,
    residuals: &NonzeroIncidentTranslationResiduals,
    nominations: &IncidentTranslationNominations,
    limits: SourceDiscoveryLimits,
) -> Result<(), SampledDeclaredModuleDualError> {
    let obstruction_support_entries = obstruction.entries().len();
    let candidate_count = nominations.requests().len();
    check_limit(
        RESIDUAL_CANDIDATES,
        candidate_count,
        limits.max_residual_candidates,
    )
    .map_err(SampledDeclaredModuleDualError::NominationVerification)?;
    let support_coordinates = obstruction_support_entries
        .checked_mul(incidence.arity())
        .ok_or_else(|| {
            SampledDeclaredModuleDualError::NominationVerification(
                SourceDiscoveryError::ResourceCountOverflow {
                    resource: RESIDUAL_SUPPORT_COORDINATES,
                },
            )
        })?;
    check_limit(
        RESIDUAL_SUPPORT_COORDINATES,
        support_coordinates,
        limits.max_residual_support_coordinate_cells,
    )
    .map_err(SampledDeclaredModuleDualError::NominationVerification)?;

    let mut source_terms = 0usize;
    for request in nominations.requests() {
        let source = incidence
            .sources()
            .get(request.source_ordinal())
            .ok_or(SampledDeclaredModuleDualError::ResidualTelemetryMismatch)?;
        source_terms = checked_add(RESIDUAL_SOURCE_TERMS, source_terms, source.terms().len())
            .map_err(SampledDeclaredModuleDualError::NominationVerification)?;
        check_limit(
            RESIDUAL_SOURCE_TERMS,
            source_terms,
            limits.max_residual_source_terms,
        )
        .map_err(SampledDeclaredModuleDualError::NominationVerification)?;
    }

    let expected_paired_source_terms =
        exact_paired_source_term_census(incidence, obstruction, nominations, source_terms, limits)?;
    if residuals.evaluated_candidates() != candidate_count
        || residuals.evaluated_source_terms() != source_terms
        || residuals.paired_source_terms() != expected_paired_source_terms
        || residuals.obstruction_support_entries() != obstruction_support_entries
    {
        return Err(SampledDeclaredModuleDualError::ResidualTelemetryMismatch);
    }
    Ok(())
}

fn exact_paired_source_term_census(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    obstruction: &ModularRightObstruction<'_>,
    nominations: &IncidentTranslationNominations,
    source_terms: usize,
    limits: SourceDiscoveryLimits,
) -> Result<usize, SampledDeclaredModuleDualError> {
    let coordinate_cells = source_terms.checked_mul(incidence.arity()).ok_or_else(|| {
        SampledDeclaredModuleDualError::NominationVerification(
            SourceDiscoveryError::ResourceCountOverflow {
                resource: "sampled-dual exact pairing coordinate cells",
            },
        )
    })?;
    check_limit(
        "sampled-dual exact pairing coordinate cells",
        coordinate_cells,
        limits.max_sampled_dual_pairing_coordinate_cells,
    )
    .map_err(SampledDeclaredModuleDualError::NominationVerification)?;

    let mut support = try_vec(DUAL_OBSTRUCTION_ENTRIES, obstruction.entries().len())
        .map_err(SampledDeclaredModuleDualError::NominationVerification)?;
    for entry in obstruction.entries() {
        let physical = *obstruction
            .logical_physical_columns()
            .get(entry.logical_column())
            .ok_or(SampledDeclaredModuleDualError::RawObstructionMismatch)?;
        let shift = obstruction
            .plan()
            .columns()
            .get(physical)
            .ok_or(SampledDeclaredModuleDualError::RawObstructionMismatch)?;
        support.push(shift);
    }
    support.sort_unstable_by(|left, right| left.values().cmp(right.values()));
    if support.is_empty()
        || support
            .windows(2)
            .any(|pair| pair[0].values() >= pair[1].values())
    {
        return Err(SampledDeclaredModuleDualError::RawObstructionMismatch);
    }

    let mut translated = try_vec(
        "sampled-dual exact pairing scratch coordinates",
        incidence.arity(),
    )
    .map_err(SampledDeclaredModuleDualError::NominationVerification)?;
    let mut paired = 0usize;
    for (candidate_ordinal, request) in nominations.requests().iter().enumerate() {
        let source = incidence
            .sources()
            .get(request.source_ordinal())
            .ok_or(SampledDeclaredModuleDualError::ResidualTelemetryMismatch)?;
        for (term_ordinal, source_shift) in source.terms().keys().enumerate() {
            translated.clear();
            for (position, (&offset, &shift)) in request
                .offset()
                .values()
                .iter()
                .zip(source_shift.values())
                .enumerate()
            {
                translated.push(offset.checked_add(shift).ok_or(
                    SampledDeclaredModuleDualError::ResidualPairingShiftOverflow {
                        candidate_ordinal,
                        term_ordinal,
                        position,
                        offset,
                        source_shift: shift,
                    },
                )?);
            }
            if translated.len() != incidence.arity() {
                return Err(SampledDeclaredModuleDualError::ResidualTelemetryMismatch);
            }
            if support
                .binary_search_by(|entry| entry.values().cmp(translated.as_slice()))
                .is_ok()
            {
                paired = checked_add(RESIDUAL_SOURCE_TERMS, paired, 1)
                    .map_err(SampledDeclaredModuleDualError::NominationVerification)?;
            }
        }
    }
    Ok(paired)
}
