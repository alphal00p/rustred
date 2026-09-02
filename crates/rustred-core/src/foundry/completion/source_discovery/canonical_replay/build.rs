use std::cmp::Ordering;
use std::sync::Arc;

use crate::foundry::completion::frame::compare_exact_circuit_content;
use crate::foundry::completion::frame::exact::{
    ExactCircuitGuardOrigin, ExactCircuitLift, ExactTargetCircuit, try_lift_exact_circuit,
};
use crate::foundry::completion::frame::modular::{
    ModularKernelError, ModularRankDiagnostics, ModularTargetQuery,
};
use crate::foundry::completion::stratum::{CampaignStratumAnchor, ImmutableOwnerSnapshot};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::OrderingPolicy;

use super::super::scheduler::{ProbeLocalOutcome, ProbeLocalSchedulerReport};
use super::super::{
    AccumulatedSourceRequests, CampaignError, CampaignModularProbe, FreshTaskEpoch,
    GrowingTaskEpochState, OrdinarySourceIncidenceIndex,
};
use super::{
    CanonicalRebaseAttempt, CanonicalRebaseAttemptOutcome, CanonicalRebasedCandidate,
    CanonicalReplayBatch, CanonicalReplayDisposition, CanonicalReplayError, CanonicalReplayLimits,
    CanonicalReplayTelemetry,
};

const NOMINATIONS: &str = "replayed probe nominations";
const PROBE_COORDINATES: &str = "nomination probe coordinate cells";
const REQUEST_OCCURRENCES: &str = "submitted union request occurrences";
const REQUEST_COORDINATES: &str = "submitted union request coordinate cells";
const ATTEMPTS: &str = "common-plan rebase attempts";
const MODULAR_ENTRY_WORK: &str = "aggregate common-plan modular entry work";
const PARTITION_COLUMN_WORK: &str = "aggregate common-plan partition column work";
const RETAINED_DIAGNOSTIC_ENTRIES: &str = "retained common-plan diagnostic entries";
const RETAINED_EXACT_PAYLOAD_CELLS: &str = "retained common-plan exact payload cells";
const RETAINED_INTEGER_BITS: &str = "retained common-plan integer coefficient bits";
const SUCCESSFUL_LIFTS: &str = "successful common-plan exact lifts";
const UNIQUE_CANDIDATES: &str = "unique common-plan exact candidates";
const SUPPORTING_PROBES: &str = "supporting probe references";
const ANCHOR_COORDINATES: &str = "retained exact-anchor coordinate cells";
const CONTENT_SORT_COMPARISONS: &str = "exact-content sort work reservation";

#[derive(Debug)]
struct ReplayNomination {
    ordinal: usize,
    probe: CampaignModularProbe,
    requests: AccumulatedSourceRequests,
    final_domain: crate::sector::SectorMonotoneDomain,
}

#[derive(Debug)]
struct ProvisionalCandidate {
    circuit: Arc<ExactTargetCircuit>,
    anchor: Box<[i64]>,
    probe: CampaignModularProbe,
}

/// Rebase all exact probe-local nominations onto one freshly built common
/// physical plan.  Old circuits and plan tokens never cross this boundary.
#[allow(clippy::too_many_arguments)]
pub(crate) fn try_canonicalize_replayed_probes(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    target: IntegralShift,
    stratum_anchor: impl Into<CampaignStratumAnchor>,
    owners: ImmutableOwnerSnapshot,
    ordering: OrderingPolicy,
    report: &ProbeLocalSchedulerReport,
    limits: CanonicalReplayLimits,
) -> Result<CanonicalReplayDisposition, CanonicalReplayError> {
    let stratum_anchor = stratum_anchor.into();
    validate_task(generator, completed, &target, &stratum_anchor, &owners)?;
    let arity = target.len();
    let mut nominations = try_vec(NOMINATIONS, 0)?;
    let mut nomination_request_occurrences = 0usize;
    let mut probe_coordinate_cells = 0usize;

    for probe_report in report.probes() {
        let ProbeLocalOutcome::Replayed { epoch, circuit: _ } = probe_report.outcome() else {
            continue;
        };
        let nomination = nominations.len();
        let report_ordinal = probe_report.probe_ordinal();
        validate_nomination(
            report_ordinal,
            generator,
            &target,
            &stratum_anchor,
            &owners,
            ordering,
            epoch,
        )?;
        check_limit(
            NOMINATIONS,
            checked_add(NOMINATIONS, nomination, 1)?,
            limits.max_replayed_nominations,
        )?;
        nomination_request_occurrences = checked_add(
            REQUEST_OCCURRENCES,
            nomination_request_occurrences,
            epoch.requests().len(),
        )?;
        check_limit(
            REQUEST_OCCURRENCES,
            nomination_request_occurrences,
            limits.max_union_request_occurrences,
        )?;
        let total_request_cells =
            checked_mul(REQUEST_COORDINATES, nomination_request_occurrences, arity)?;
        check_limit(
            REQUEST_COORDINATES,
            total_request_cells,
            limits.max_union_request_coordinate_cells,
        )?;
        let probe_cells = checked_add(
            PROBE_COORDINATES,
            probe_report.probe().base_parameters().len(),
            probe_report.probe().chart_coordinates().len(),
        )?;
        probe_coordinate_cells =
            checked_add(PROBE_COORDINATES, probe_coordinate_cells, probe_cells)?;
        check_limit(
            PROBE_COORDINATES,
            probe_coordinate_cells,
            limits.max_nomination_probe_coordinate_cells,
        )?;
        nominations
            .try_reserve_exact(1)
            .map_err(|_| CanonicalReplayError::AllocationFailure {
                resource: NOMINATIONS,
                requested: nomination + 1,
            })?;
        nominations.push(ReplayNomination {
            ordinal: report_ordinal,
            probe: probe_report.probe().clone(),
            requests: epoch.requests().clone(),
            final_domain: epoch.fixed_stratum().domain().clone(),
        });
    }

    if nominations.is_empty() {
        return Ok(CanonicalReplayDisposition::NoReplayedNominations);
    }
    let attempts_requested = nominations.len();
    check_limit(ATTEMPTS, attempts_requested, limits.max_rebase_attempts)?;
    nominations.sort_unstable_by(|left, right| compare_probe(&left.probe, &right.probe));
    if nominations
        .windows(2)
        .any(|pair| compare_probe(&pair[0].probe, &pair[1].probe) == Ordering::Equal)
    {
        return Err(CanonicalReplayError::DuplicateProbeNomination);
    }

    let zero = IntegralShift::try_new_with_component_limit(
        std::iter::repeat_n(0, arity),
        limits.source_discovery.max_arity,
    )
    .map_err(CanonicalReplayError::Shift)?;
    let zero_sources = generator
        .translate_completed_source_rows(completed, [zero], limits.source_discovery.translation)
        .map_err(CanonicalReplayError::SourceTranslation)?;
    let incidence = OrdinarySourceIncidenceIndex::try_new(&zero_sources, limits.source_discovery)?;
    let bootstrap = incidence.try_nominate_target_unit(&target, limits.source_discovery)?;
    let bootstrap_requests = AccumulatedSourceRequests::try_new(
        arity,
        bootstrap.requests().iter().cloned(),
        limits.campaign,
    )?;
    let submitted_union_occurrences = checked_add(
        REQUEST_OCCURRENCES,
        bootstrap_requests.len(),
        nomination_request_occurrences,
    )?;
    check_limit(
        REQUEST_OCCURRENCES,
        submitted_union_occurrences,
        limits.max_union_request_occurrences,
    )?;
    let submitted_union_coordinate_cells =
        checked_mul(REQUEST_COORDINATES, submitted_union_occurrences, arity)?;
    check_limit(
        REQUEST_COORDINATES,
        submitted_union_coordinate_cells,
        limits.max_union_request_coordinate_cells,
    )?;

    let submitted_union = bootstrap_requests.requests().iter().cloned().chain(
        nominations
            .iter()
            .flat_map(|nomination| nomination.requests.requests().iter().cloned()),
    );
    let union = AccumulatedSourceRequests::try_new(arity, submitted_union, limits.campaign)?;

    let mut epochs = GrowingTaskEpochState::new(target, stratum_anchor, owners, ordering);
    let bootstrap_epoch = epochs.try_next(
        generator,
        completed,
        bootstrap_requests.clone(),
        limits.campaign,
    )?;
    let common_epoch = if union == bootstrap_requests {
        bootstrap_epoch
    } else {
        drop(bootstrap_epoch);
        epochs.try_next(generator, completed, union.clone(), limits.campaign)?
    };
    let common_epoch = Arc::new(common_epoch);
    for nomination in &nominations {
        if !domain_is_contained_by(
            common_epoch.fixed_stratum().domain(),
            &nomination.final_domain,
        ) {
            return Err(CanonicalReplayError::ReplayedNominationJoin {
                nomination: nomination.ordinal,
                detail: "common union domain is not contained in the contributing final domain",
            });
        }
    }
    let modular_work = checked_mul(
        MODULAR_ENTRY_WORK,
        attempts_requested,
        common_epoch.telemetry().physical_entries(),
    )?;
    check_limit(
        MODULAR_ENTRY_WORK,
        modular_work,
        limits.max_aggregate_modular_entry_work,
    )?;
    let partition_work = checked_mul(
        PARTITION_COLUMN_WORK,
        1,
        common_epoch.telemetry().physical_columns(),
    )?;
    check_limit(
        PARTITION_COLUMN_WORK,
        partition_work,
        limits.max_aggregate_partition_column_work,
    )?;
    let common_partition = common_epoch.try_partition(limits.campaign.stratum)?;

    let mut attempts = try_vec(ATTEMPTS, attempts_requested)?;
    let mut provisional = try_vec(
        SUCCESSFUL_LIFTS,
        attempts_requested.min(limits.max_successful_exact_lifts),
    )?;
    let mut anchor_coordinate_cells = 0usize;
    let mut retained_diagnostic_entries = 0usize;
    let mut retained_exact_payload_cells = 0usize;
    let mut retained_integer_coefficient_bits = 0usize;
    for nomination in nominations {
        let probe = nomination.probe;
        let query = match common_epoch.try_query_with_partition(
            generator.context(),
            &probe,
            &common_partition,
            limits.campaign,
        ) {
            Ok(query) => query,
            Err(error) if is_expected_probe_rejection(&error) => {
                attempts.push(CanonicalRebaseAttempt::new(
                    probe,
                    CanonicalRebaseAttemptOutcome::QueryRejected(error),
                ));
                continue;
            }
            Err(error) => return Err(CanonicalReplayError::Campaign(error)),
        };
        match query.query() {
            ModularTargetQuery::NoHitWithObstruction(_) => {
                let diagnostics = query.query().diagnostics();
                charge(
                    RETAINED_DIAGNOSTIC_ENTRIES,
                    &mut retained_diagnostic_entries,
                    diagnostic_entries(diagnostics)?,
                    limits.max_retained_diagnostic_entries,
                )?;
                let diagnostics = diagnostics.clone();
                drop(query);
                attempts.push(CanonicalRebaseAttempt::new(
                    probe,
                    CanonicalRebaseAttemptOutcome::NoModularHit { diagnostics },
                ));
            }
            ModularTargetQuery::Hit(hit) => {
                let lift = try_lift_exact_circuit(
                    generator.context(),
                    hit,
                    query.partition(),
                    limits.exact_circuit,
                )?;
                match lift {
                    ExactCircuitLift::ModularSupportDidNotLift(evidence) => {
                        let evidence_entries = checked_add(
                            RETAINED_DIAGNOSTIC_ENTRIES,
                            diagnostic_entries(evidence.modular_diagnostics())?,
                            checked_add(
                                RETAINED_DIAGNOSTIC_ENTRIES,
                                evidence.selected_source_instances().len(),
                                evidence.sample_fingerprint().point().len(),
                            )?,
                        )?;
                        charge(
                            RETAINED_DIAGNOSTIC_ENTRIES,
                            &mut retained_diagnostic_entries,
                            evidence_entries,
                            limits.max_retained_diagnostic_entries,
                        )?;
                        drop(query);
                        attempts.push(CanonicalRebaseAttempt::new(
                            probe,
                            CanonicalRebaseAttemptOutcome::SupportDidNotLift(evidence),
                        ));
                    }
                    ExactCircuitLift::Replayed(circuit) => {
                        if !Arc::ptr_eq(
                            circuit.sample_fingerprint(),
                            query.sampled().sample_fingerprint(),
                        ) {
                            return Err(CanonicalReplayError::Invariant {
                                detail: "fresh exact circuit lost the common query sample identity",
                            });
                        }
                        let anchor = common_epoch.try_anchor_for_probe(&probe)?;
                        validate_fresh_circuit(&common_epoch, &circuit, &anchor, &probe)?;
                        charge(
                            RETAINED_DIAGNOSTIC_ENTRIES,
                            &mut retained_diagnostic_entries,
                            diagnostic_entries(circuit.modular_diagnostics())?,
                            limits.max_retained_diagnostic_entries,
                        )?;
                        charge(
                            RETAINED_EXACT_PAYLOAD_CELLS,
                            &mut retained_exact_payload_cells,
                            exact_payload_cells(&circuit)?,
                            limits.max_retained_exact_payload_cells,
                        )?;
                        charge(
                            RETAINED_INTEGER_BITS,
                            &mut retained_integer_coefficient_bits,
                            exact_integer_bits(&circuit)?,
                            limits.max_retained_integer_coefficient_bits,
                        )?;
                        let successful = checked_add(SUCCESSFUL_LIFTS, provisional.len(), 1)?;
                        check_limit(
                            SUCCESSFUL_LIFTS,
                            successful,
                            limits.max_successful_exact_lifts,
                        )?;
                        anchor_coordinate_cells =
                            checked_add(ANCHOR_COORDINATES, anchor_coordinate_cells, anchor.len())?;
                        check_limit(
                            ANCHOR_COORDINATES,
                            anchor_coordinate_cells,
                            limits.max_anchor_coordinate_cells,
                        )?;
                        drop(query);
                        provisional.push(ProvisionalCandidate {
                            circuit: Arc::new(circuit),
                            anchor,
                            probe: probe.clone(),
                        });
                        attempts.push(CanonicalRebaseAttempt::new(
                            probe,
                            CanonicalRebaseAttemptOutcome::Replayed,
                        ));
                    }
                }
            }
        }
    }

    let successful_exact_lifts = provisional.len();
    let sort_reservation = checked_mul(
        CONTENT_SORT_COMPARISONS,
        successful_exact_lifts,
        ceil_log2(successful_exact_lifts).max(1),
    )?;
    check_limit(
        CONTENT_SORT_COMPARISONS,
        sort_reservation,
        limits.max_content_sort_comparisons,
    )?;
    provisional.sort_unstable_by(compare_provisional);
    let candidates = deduplicate_candidates(provisional, &common_epoch, limits)?;
    let unique_candidates = candidates.len();
    let duplicate_exact_lifts = successful_exact_lifts
        .checked_sub(unique_candidates)
        .ok_or(CanonicalReplayError::Invariant {
            detail: "unique candidate count exceeds successful exact lifts",
        })?;
    let telemetry = CanonicalReplayTelemetry::new(
        attempts_requested,
        nomination_request_occurrences,
        union.len(),
        common_epoch.telemetry().epoch_ordinal(),
        attempts.len(),
        successful_exact_lifts,
        unique_candidates,
        duplicate_exact_lifts,
        anchor_coordinate_cells,
        retained_diagnostic_entries,
        retained_exact_payload_cells,
        retained_integer_coefficient_bits,
    );
    if candidates.is_empty() {
        return Ok(CanonicalReplayDisposition::NoRebasedCircuits {
            epoch: common_epoch,
            attempts: attempts.into_boxed_slice(),
            telemetry,
        });
    }
    Ok(CanonicalReplayDisposition::Rebased(
        CanonicalReplayBatch::new(common_epoch, candidates, attempts, telemetry),
    ))
}

fn validate_task(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    target: &IntegralShift,
    anchor: &CampaignStratumAnchor,
    owners: &ImmutableOwnerSnapshot,
) -> Result<(), CanonicalReplayError> {
    if !completed.is_complete_ordinary() {
        return Err(CanonicalReplayError::WrongSourceLayout {
            actual: completed.layout_name(),
        });
    }
    let arity = generator.context().index_count();
    if target.len() != arity || anchor.arity() != arity || owners.arity() != arity {
        return Err(CanonicalReplayError::WrongTaskScope {
            detail: "target, campaign stratum, owner snapshot, and indexed context have different arities",
        });
    }
    if anchor.context_fingerprint() != generator.context().fingerprint()
        || owners.context_fingerprint() != generator.context().fingerprint()
        || owners.family_fingerprint() != anchor.family_fingerprint()
    {
        return Err(CanonicalReplayError::WrongTaskScope {
            detail: "generator, campaign stratum, and owner snapshot have different identities",
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_nomination(
    nomination: usize,
    generator: &ParametricIbpGenerator<'_>,
    target: &IntegralShift,
    anchor: &CampaignStratumAnchor,
    owners: &ImmutableOwnerSnapshot,
    ordering: OrderingPolicy,
    epoch: &FreshTaskEpoch,
) -> Result<(), CanonicalReplayError> {
    let reject = |detail| CanonicalReplayError::ReplayedNominationJoin { nomination, detail };
    if epoch.target_shift() != target {
        return Err(reject("target shift differs"));
    }
    if epoch.plan().family_fingerprint() != anchor.family_fingerprint()
        || epoch.plan().context_fingerprint() != generator.context().fingerprint()
    {
        return Err(reject("family or indexed-context identity differs"));
    }
    if epoch.fixed_stratum().domain().sector() != anchor.initial().domain().sector()
        || epoch.fixed_stratum().guards() != anchor.initial().guards()
        || !domain_is_contained_by(epoch.fixed_stratum().domain(), anchor.initial().domain())
    {
        return Err(reject(
            "final stratum is not a monotone restriction of the declared anchor",
        ));
    }
    if epoch.fixed_snapshot_id() != owners.id() {
        return Err(reject("immutable lower-owner snapshot differs"));
    }
    if epoch.fixed_ordering() != ordering {
        return Err(reject("ordering policy differs"));
    }
    Ok(())
}

fn domain_is_contained_by(
    inner: &crate::sector::SectorMonotoneDomain,
    outer: &crate::sector::SectorMonotoneDomain,
) -> bool {
    inner.sector() == outer.sector()
        && inner.bounds().len() == outer.bounds().len()
        && inner
            .bounds()
            .iter()
            .zip(outer.bounds())
            .all(|(&inner, &outer)| {
                outer.lower() <= inner.lower() && inner.upper() <= outer.upper()
            })
}

fn is_expected_probe_rejection(error: &CampaignError) -> bool {
    matches!(
        error,
        CampaignError::SampleOutsideFixedStratum { .. }
            | CampaignError::Modular(ModularKernelError::CoefficientDenominatorZero { .. })
            | CampaignError::Modular(ModularKernelError::SourceConditionZero { .. })
    )
}

fn validate_fresh_circuit(
    epoch: &FreshTaskEpoch,
    circuit: &ExactTargetCircuit,
    anchor: &[i64],
    probe: &CampaignModularProbe,
) -> Result<(), CanonicalReplayError> {
    if !circuit.is_bound_to(epoch.plan())
        || circuit.target_column() != epoch.target_column()
        || circuit.target_shift() != epoch.target_shift()
        || circuit.stratum_id() != epoch.fixed_stratum().id()
        || circuit.owner_snapshot_id() != epoch.fixed_snapshot_id()
    {
        return Err(CanonicalReplayError::Invariant {
            detail: "fresh exact circuit does not rejoin the common epoch",
        });
    }
    if circuit.sample_fingerprint().modulus() != probe.modulus() {
        return Err(CanonicalReplayError::Invariant {
            detail: "fresh exact circuit and raw probe have different moduli",
        });
    }
    match epoch.fixed_stratum().domain().contains(anchor) {
        Ok(true) => Ok(()),
        Ok(false) | Err(_) => Err(CanonicalReplayError::Invariant {
            detail: "retained exact replay anchor is outside the common epoch domain",
        }),
    }
}

fn compare_probe(left: &CampaignModularProbe, right: &CampaignModularProbe) -> Ordering {
    left.modulus()
        .cmp(&right.modulus())
        .then_with(|| left.base_parameters().cmp(right.base_parameters()))
        .then_with(|| left.chart_coordinates().cmp(right.chart_coordinates()))
}

fn compare_provisional(left: &ProvisionalCandidate, right: &ProvisionalCandidate) -> Ordering {
    compare_exact_circuit_content(&left.circuit, &right.circuit)
        .then_with(|| left.anchor.cmp(&right.anchor))
        .then_with(|| compare_probe(&left.probe, &right.probe))
}

fn deduplicate_candidates(
    provisional: Vec<ProvisionalCandidate>,
    epoch: &FreshTaskEpoch,
    limits: CanonicalReplayLimits,
) -> Result<Vec<CanonicalRebasedCandidate>, CanonicalReplayError> {
    let mut incoming = provisional.into_iter().peekable();
    let mut candidates = try_vec(UNIQUE_CANDIDATES, 0)?;
    let mut supporting_probe_references = 0usize;
    while let Some(primary) = incoming.next() {
        let mut supporting = try_vec(SUPPORTING_PROBES, 0)?;
        supporting_probe_references =
            checked_add(SUPPORTING_PROBES, supporting_probe_references, 1)?;
        check_limit(
            SUPPORTING_PROBES,
            supporting_probe_references,
            limits.max_supporting_probe_references,
        )?;
        supporting
            .try_reserve_exact(1)
            .map_err(|_| CanonicalReplayError::AllocationFailure {
                resource: SUPPORTING_PROBES,
                requested: supporting_probe_references,
            })?;
        supporting.push(primary.probe.clone());
        while incoming.peek().is_some_and(|next| {
            compare_exact_circuit_content(&primary.circuit, &next.circuit) == Ordering::Equal
        }) {
            let duplicate = incoming.next().ok_or(CanonicalReplayError::Invariant {
                detail: "peeked duplicate exact circuit disappeared",
            })?;
            supporting_probe_references =
                checked_add(SUPPORTING_PROBES, supporting_probe_references, 1)?;
            check_limit(
                SUPPORTING_PROBES,
                supporting_probe_references,
                limits.max_supporting_probe_references,
            )?;
            supporting.try_reserve_exact(1).map_err(|_| {
                CanonicalReplayError::AllocationFailure {
                    resource: SUPPORTING_PROBES,
                    requested: supporting_probe_references,
                }
            })?;
            supporting.push(duplicate.probe);
        }
        supporting.sort_unstable_by(compare_probe);
        if supporting
            .windows(2)
            .any(|pair| compare_probe(&pair[0], &pair[1]) == Ordering::Equal)
        {
            return Err(CanonicalReplayError::Invariant {
                detail: "one raw probe contributed duplicate exact supports",
            });
        }
        let requested = checked_add(UNIQUE_CANDIDATES, candidates.len(), 1)?;
        check_limit(UNIQUE_CANDIDATES, requested, limits.max_unique_candidates)?;
        candidates
            .try_reserve_exact(1)
            .map_err(|_| CanonicalReplayError::AllocationFailure {
                resource: UNIQUE_CANDIDATES,
                requested,
            })?;
        validate_fresh_circuit(epoch, &primary.circuit, &primary.anchor, &primary.probe)?;
        candidates.push(CanonicalRebasedCandidate::new(
            primary.circuit,
            primary.anchor,
            primary.probe,
            supporting,
        ));
    }
    Ok(candidates)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, CanonicalReplayError> {
    left.checked_add(right)
        .ok_or(CanonicalReplayError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, CanonicalReplayError> {
    left.checked_mul(right)
        .ok_or(CanonicalReplayError::ResourceCountOverflow { resource })
}

fn charge(
    resource: &'static str,
    total: &mut usize,
    additional: usize,
    limit: usize,
) -> Result<(), CanonicalReplayError> {
    let requested = checked_add(resource, *total, additional)?;
    check_limit(resource, requested, limit)?;
    *total = requested;
    Ok(())
}

fn diagnostic_entries(diagnostics: &ModularRankDiagnostics) -> Result<usize, CanonicalReplayError> {
    let mut total = diagnostics.forbidden_columns.len();
    for count in [
        diagnostics.forbidden_pivot_columns.len(),
        diagnostics.augmented_pivot_columns.len(),
        diagnostics.forbidden_independent_source_rows.len(),
        diagnostics.augmented_independent_source_rows.len(),
    ] {
        total = checked_add(RETAINED_DIAGNOSTIC_ENTRIES, total, count)?;
    }
    Ok(total)
}

fn exact_payload_cells(circuit: &ExactTargetCircuit) -> Result<usize, CanonicalReplayError> {
    let mut total = circuit.sample_fingerprint().point().len();
    total = checked_add(
        RETAINED_EXACT_PAYLOAD_CELLS,
        total,
        checked_mul(
            RETAINED_EXACT_PAYLOAD_CELLS,
            circuit.fixed_indices().len(),
            2,
        )?,
    )?;
    total = checked_add(
        RETAINED_EXACT_PAYLOAD_CELLS,
        total,
        circuit.target_shift().len(),
    )?;
    for term in circuit.residual_terms() {
        total = checked_add(RETAINED_EXACT_PAYLOAD_CELLS, total, 1)?;
        total = checked_add(RETAINED_EXACT_PAYLOAD_CELLS, total, term.shift().len())?;
        total = checked_add(
            RETAINED_EXACT_PAYLOAD_CELLS,
            total,
            term.proper_subsector_owners().len(),
        )?;
        total = checked_add(
            RETAINED_EXACT_PAYLOAD_CELLS,
            total,
            coefficient_cells(term.coefficient())?,
        )?;
    }
    for contribution in circuit.source_combination() {
        total = checked_add(RETAINED_EXACT_PAYLOAD_CELLS, total, 1)?;
        total = checked_add(
            RETAINED_EXACT_PAYLOAD_CELLS,
            total,
            coefficient_cells(contribution.coefficient())?,
        )?;
    }
    for pivot in circuit.pivot_guards() {
        total = checked_add(RETAINED_EXACT_PAYLOAD_CELLS, total, 1)?;
        total = checked_add(
            RETAINED_EXACT_PAYLOAD_CELLS,
            total,
            coefficient_cells(pivot.coefficient())?,
        )?;
        total = checked_add(
            RETAINED_EXACT_PAYLOAD_CELLS,
            total,
            polynomial_cells(pivot.nonzero_polynomial())?,
        )?;
    }
    for guard in circuit.nonzero_guards() {
        total = checked_add(RETAINED_EXACT_PAYLOAD_CELLS, total, 1)?;
        total = checked_add(
            RETAINED_EXACT_PAYLOAD_CELLS,
            total,
            polynomial_cells(guard.polynomial())?,
        )?;
        total = checked_add(RETAINED_EXACT_PAYLOAD_CELLS, total, guard.origins().len())?;
        for origin in guard.origins() {
            if let ExactCircuitGuardOrigin::SourceCondition {
                condition_sources, ..
            } = origin
            {
                total = checked_add(RETAINED_EXACT_PAYLOAD_CELLS, total, condition_sources.len())?;
            }
        }
    }
    Ok(total)
}

fn coefficient_cells(
    coefficient: &crate::algebra::IndexedCoefficient,
) -> Result<usize, CanonicalReplayError> {
    checked_add(
        RETAINED_EXACT_PAYLOAD_CELLS,
        polynomial_cells_raw(&coefficient.raw().numerator)?,
        polynomial_cells_raw(&coefficient.raw().denominator)?,
    )
}

fn polynomial_cells(
    polynomial: &crate::algebra::IndexedPolynomial,
) -> Result<usize, CanonicalReplayError> {
    polynomial_cells_raw(polynomial.raw())
}

fn polynomial_cells_raw(
    polynomial: &symbolica::prelude::MultivariatePolynomial<symbolica::prelude::IntegerRing, u16>,
) -> Result<usize, CanonicalReplayError> {
    checked_add(
        RETAINED_EXACT_PAYLOAD_CELLS,
        polynomial.coefficients.len(),
        polynomial.exponents.len(),
    )
}

fn exact_integer_bits(circuit: &ExactTargetCircuit) -> Result<usize, CanonicalReplayError> {
    let mut total = 0usize;
    for term in circuit.residual_terms() {
        total = checked_add(
            RETAINED_INTEGER_BITS,
            total,
            coefficient_integer_bits(term.coefficient())?,
        )?;
    }
    for contribution in circuit.source_combination() {
        total = checked_add(
            RETAINED_INTEGER_BITS,
            total,
            coefficient_integer_bits(contribution.coefficient())?,
        )?;
    }
    for pivot in circuit.pivot_guards() {
        total = checked_add(
            RETAINED_INTEGER_BITS,
            total,
            coefficient_integer_bits(pivot.coefficient())?,
        )?;
        total = checked_add(
            RETAINED_INTEGER_BITS,
            total,
            polynomial_integer_bits(pivot.nonzero_polynomial().raw())?,
        )?;
    }
    for guard in circuit.nonzero_guards() {
        total = checked_add(
            RETAINED_INTEGER_BITS,
            total,
            polynomial_integer_bits(guard.polynomial().raw())?,
        )?;
    }
    Ok(total)
}

fn coefficient_integer_bits(
    coefficient: &crate::algebra::IndexedCoefficient,
) -> Result<usize, CanonicalReplayError> {
    checked_add(
        RETAINED_INTEGER_BITS,
        polynomial_integer_bits(&coefficient.raw().numerator)?,
        polynomial_integer_bits(&coefficient.raw().denominator)?,
    )
}

fn polynomial_integer_bits(
    polynomial: &symbolica::prelude::MultivariatePolynomial<symbolica::prelude::IntegerRing, u16>,
) -> Result<usize, CanonicalReplayError> {
    let mut total = 0usize;
    for coefficient in &polynomial.coefficients {
        let bits = usize::try_from(integer_magnitude_bits(coefficient)).map_err(|_| {
            CanonicalReplayError::ResourceCountOverflow {
                resource: RETAINED_INTEGER_BITS,
            }
        })?;
        total = checked_add(RETAINED_INTEGER_BITS, total, bits)?;
    }
    Ok(total)
}

fn integer_magnitude_bits(value: &symbolica::prelude::Integer) -> u64 {
    match value {
        symbolica::prelude::Integer::Single(value) => {
            u64::from(i64::BITS - value.unsigned_abs().leading_zeros())
        }
        symbolica::prelude::Integer::Double(value) => {
            u64::from(i128::BITS - value.unsigned_abs().leading_zeros())
        }
        symbolica::prelude::Integer::Large(value) => u64::from(value.significant_bits()),
    }
}

fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        return 0;
    }
    usize::BITS as usize - (value - 1).leading_zeros() as usize
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), CanonicalReplayError> {
    if requested > limit {
        Err(CanonicalReplayError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn try_vec<T>(resource: &'static str, capacity: usize) -> Result<Vec<T>, CanonicalReplayError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| CanonicalReplayError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}
