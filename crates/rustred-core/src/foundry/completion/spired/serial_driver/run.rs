use crate::family::{IntegralKey, IntegralKeyError};
use crate::foundry::completion::source_discovery::{
    CanonicalExactOwnerLedger, ExactTerminalCoverDeltaKind,
};
use crate::foundry::completion::stratum::DecoratedStratum;
use crate::foundry::completion::{
    CompletionGeometryError, LatticeCardinality, LatticePoint, SectorChart, UncoveredPartition,
};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};

use super::super::{
    SpiredCoordinateCaseObligation, SpiredCoordinateCaseWorklist,
    SpiredCoordinateCaseWorklistLimits, SpiredEqualityStepCensus, SpiredEqualityStepOutcome,
    try_advance_spired_coordinate_case,
};
use super::error::SpiredSerialDriverErrorCause;
use super::probe::try_build_case_probes;
use super::{
    SpiredExistingTerminalAuthority, SpiredSerialCaseIncomplete, SpiredSerialCaseOutcome,
    SpiredSerialCaseReport, SpiredSerialDriverCensus, SpiredSerialDriverConfig,
    SpiredSerialDriverError, SpiredSerialDriverReport, SpiredSerialDriverStop,
    SpiredSerialFiniteResidualReason, SpiredSerialProbeCensus, SpiredSerialProbePortfolio,
    SpiredSerialResumePolicy,
};

const CASE_ATTEMPTS: &str = "case attempts";
const CASE_REPORTS: &str = "retained case reports";
const GENERATED_PROBES: &str = "generated probes";
const GENERATED_PROBE_CELLS: &str = "generated probe coordinate cells";
const PROBE_ATTEMPTS: &str = "probe attempts";
const SCHEDULED_REQUESTS: &str = "scheduled requests";
const STREAMED_ROWS: &str = "streamed rows";
const MODULAR_HITS: &str = "modular hits";
const EXACT_LIFT_ATTEMPTS: &str = "exact-lift attempts";
const TERMINAL_RETIREMENTS: &str = "existing terminal retirements";

/// Atomically initialize the generic-first exact equality worklist.
pub(crate) fn try_initialize_spired_serial_worklist(
    roots: impl IntoIterator<Item = SpiredCoordinateCaseObligation>,
    limits: SpiredCoordinateCaseWorklistLimits,
) -> Result<SpiredCoordinateCaseWorklist, super::super::SpiredCoordinateCaseWorklistError> {
    let mut worklist = SpiredCoordinateCaseWorklist::new(limits);
    let prepared = worklist.try_prepare_enqueue_batch(roots)?;
    worklist.try_commit_prepared_enqueue_batch(prepared)?;
    Ok(worklist)
}

/// Drive exact equality cases serially in the worklist's generic-first order.
///
/// A normal incomplete result leaves the current case in place.  This first
/// production slice intentionally rebuilds its diagonal/source search from
/// the beginning on the next call; the report makes that policy explicit.
pub(crate) fn try_drive_spired_serial_coordinate_cases(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    portfolio: &SpiredSerialProbePortfolio,
    worklist: &mut SpiredCoordinateCaseWorklist,
    ledger: &mut CanonicalExactOwnerLedger,
    config: SpiredSerialDriverConfig,
) -> Result<SpiredSerialDriverReport, SpiredSerialDriverError> {
    let mut census = SpiredSerialDriverCensus::default();
    let mut reports = Vec::new();
    let mut retained_declared_carrier: Option<DecoratedStratum> = None;
    let full_sector_carrier = SectorChart::new(ledger.sector().clone())
        .carrier_box()
        .map_err(|error| {
            fail(
                census,
                Vec::new(),
                SpiredSerialDriverErrorCause::CompletionGeometry(error),
            )
        })?;
    let uses_full_sector_carrier = ledger.closure_carrier() == &full_sector_carrier;

    loop {
        if ledger.snapshot().status().is_compiler_closed() && worklist.is_empty() {
            let kind = if uses_full_sector_carrier {
                StopKind::CompilerClosed { pending_cases: 0 }
            } else {
                StopKind::BoundedCarrierClosed
            };
            return Ok(finish(ledger, census, reports, kind));
        }
        let Some(current) = worklist.cloned_current() else {
            let partition = match ledger.try_clone_uncovered_partition() {
                Ok(partition) => partition,
                Err(error) => {
                    return Err(fail(
                        census,
                        reports,
                        SpiredSerialDriverErrorCause::OwnerLedger(error),
                    ));
                }
            };
            let residual = match try_finite_residual_integrals(
                ledger,
                &partition,
                config.limits.max_finite_residual_points,
                config.limits.max_finite_residual_coordinate_cells,
            ) {
                Ok(residual) => residual,
                Err(CompletionGeometryError::ResourceLimit {
                    resource,
                    requested,
                    limit,
                }) => {
                    return Ok(resource_stop(
                        ledger, census, reports, None, resource, requested, limit,
                    ));
                }
                Err(error) => {
                    return Err(fail(
                        census,
                        reports,
                        SpiredSerialDriverErrorCause::CompletionGeometry(error),
                    ));
                }
            };
            let Some(integrals) = residual else {
                return Ok(finish(ledger, census, reports, StopKind::WorklistExhausted));
            };
            census.finite_residual_reconciliations = match checked_add(
                "finite residual reconciliations",
                census.finite_residual_reconciliations,
                1,
            ) {
                Ok(value) => value,
                Err(cause) => return Err(fail(census, reports, cause)),
            };
            census.finite_residual_points_reified = match checked_add(
                "finite residual points reified",
                census.finite_residual_points_reified,
                integrals.len(),
            ) {
                Ok(value) => value,
                Err(cause) => return Err(fail(census, reports, cause)),
            };
            let reason = match retained_declared_carrier.as_ref() {
                None => SpiredSerialFiniteResidualReason::NoRetainedDeclaredCarrier,
                Some(carrier)
                    if integrals.iter().all(|integral| {
                        carrier
                            .domain()
                            .bounds()
                            .iter()
                            .zip(integral.powers())
                            .all(|(&bounds, &power)| bounds.contains(power))
                    }) =>
                {
                    SpiredSerialFiniteResidualReason::NeedsDeclaredTerminalAuthority
                }
                Some(_) => SpiredSerialFiniteResidualReason::OutsideRetainedDeclaredCarrier,
            };
            return Ok(finite_residual_stop(
                ledger, census, reports, integrals, reason,
            ));
        };
        retained_declared_carrier = Some(current.declared_carrier().clone());
        let case_id = current.stratum().id().clone();
        if census.case_attempts >= config.limits.max_case_attempts {
            return Ok(resource_stop(
                ledger,
                census,
                reports,
                Some(case_id),
                CASE_ATTEMPTS,
                census.case_attempts.saturating_add(1),
                config.limits.max_case_attempts,
            ));
        }
        if reports.len() >= config.limits.max_case_reports {
            let requested = reports.len().saturating_add(1);
            return Ok(resource_stop(
                ledger,
                census,
                reports,
                Some(case_id),
                CASE_REPORTS,
                requested,
                config.limits.max_case_reports,
            ));
        }
        if let Err(_) = reports.try_reserve(1) {
            return Err(SpiredSerialDriverError::new(
                census,
                reports,
                SpiredSerialDriverErrorCause::AllocationFailure {
                    resource: CASE_REPORTS,
                    requested: census.retained_case_reports.saturating_add(1),
                },
            ));
        }
        if current.stratum().family_fingerprint()
            != ledger.predecessor_snapshot().family_fingerprint()
            || current.stratum().family_fingerprint() != completed.family_fingerprint()
        {
            return Err(fail(
                census,
                reports,
                SpiredSerialDriverErrorCause::WrongCaseFamily,
            ));
        }
        if current.stratum().context_fingerprint()
            != ledger.predecessor_snapshot().context_fingerprint()
            || current.stratum().context_fingerprint() != generator.context().fingerprint()
        {
            return Err(fail(
                census,
                reports,
                SpiredSerialDriverErrorCause::WrongCaseContext,
            ));
        }
        if current.stratum().domain().sector() != ledger.sector() {
            return Err(fail(
                census,
                reports,
                SpiredSerialDriverErrorCause::WrongCaseSector,
            ));
        }
        census.case_attempts = match checked_add(CASE_ATTEMPTS, census.case_attempts, 1) {
            Ok(value) => value,
            Err(cause) => return Err(fail(census, reports, cause)),
        };
        let free_dimension = current.free_dimension();
        if free_dimension == 0 {
            census.zero_dimensional_case_attempts = match checked_add(
                "zero-dimensional case attempts",
                census.zero_dimensional_case_attempts,
                1,
            ) {
                Ok(value) => value,
                Err(cause) => return Err(fail(census, reports, cause)),
            };
            let integral = match singleton_integral(&current) {
                Ok(integral) => integral,
                Err(error) => {
                    return Err(fail(
                        census,
                        reports,
                        SpiredSerialDriverErrorCause::IntegralKey(error),
                    ));
                }
            };
            let authority = if ledger.has_explicit_terminal(&integral) {
                Some(SpiredExistingTerminalAuthority::AlreadyRetainedByLedger)
            } else {
                match ledger
                    .predecessor_snapshot()
                    .authenticates_explicit_terminal(&integral)
                {
                    Ok(true) => Some(SpiredExistingTerminalAuthority::AuthenticatedByPredecessor),
                    Ok(false) => None,
                    Err(error) => {
                        return Err(fail(
                            census,
                            reports,
                            SpiredSerialDriverErrorCause::TerminalAuthority(error),
                        ));
                    }
                }
            };
            let Some(authority) = authority else {
                census.incomplete_case_attempts += 1;
                census.retained_case_reports += 1;
                let reason = SpiredSerialCaseIncomplete::ZeroDimensionalCaseNeedsDeclaredTerminal {
                    integral,
                };
                reports.push(case_report(
                    &current,
                    free_dimension,
                    SpiredSerialProbeCensus::default(),
                    None,
                    SpiredSerialCaseOutcome::Incomplete(reason.clone()),
                ));
                return Ok(finish(
                    ledger,
                    census,
                    reports,
                    StopKind::Incomplete { case_id, reason },
                ));
            };
            if census.existing_terminal_retirements
                >= config.limits.max_existing_terminal_retirements
            {
                return Ok(resource_stop(
                    ledger,
                    census,
                    reports,
                    Some(case_id),
                    TERMINAL_RETIREMENTS,
                    census.existing_terminal_retirements.saturating_add(1),
                    config.limits.max_existing_terminal_retirements,
                ));
            }
            let prepared = match worklist.try_prepare_current_replacement(&current, []) {
                Ok(prepared) => prepared,
                Err(error) => {
                    return Err(fail(
                        census,
                        reports,
                        SpiredSerialDriverErrorCause::Worklist(error),
                    ));
                }
            };
            let validated = match worklist.try_validate_prepared_replacement(prepared) {
                Ok(validated) => validated,
                Err(error) => {
                    return Err(fail(
                        census,
                        reports,
                        SpiredSerialDriverErrorCause::Worklist(error),
                    ));
                }
            };
            let delta = match ledger.try_apply_explicit_terminal(integral.clone()) {
                Ok(delta) => delta,
                Err(error) => {
                    return Err(fail(
                        census,
                        reports,
                        SpiredSerialDriverErrorCause::OwnerLedger(error),
                    ));
                }
            };
            validated.commit();
            census.existing_terminal_retirements += 1;
            if matches!(
                authority,
                SpiredExistingTerminalAuthority::AuthenticatedByPredecessor
            ) && matches!(delta.kind(), ExactTerminalCoverDeltaKind::Inserted)
            {
                census.predecessor_terminals_retained += 1;
            }
            census.retained_case_reports += 1;
            reports.push(case_report(
                &current,
                free_dimension,
                SpiredSerialProbeCensus::default(),
                None,
                SpiredSerialCaseOutcome::ExistingTerminalRetired {
                    integral,
                    authority,
                    delta,
                },
            ));
            continue;
        }

        census.positive_dimensional_case_attempts += 1;
        let remaining_probes = config
            .limits
            .max_generated_probes
            .saturating_sub(census.generated_probes);
        let remaining_cells = config
            .limits
            .max_generated_probe_coordinate_cells
            .saturating_sub(census.generated_probe_coordinate_cells);
        if portfolio.probe_template_count() > remaining_probes {
            return Ok(resource_stop(
                ledger,
                census,
                reports,
                Some(case_id),
                GENERATED_PROBES,
                census
                    .generated_probes
                    .saturating_add(portfolio.probe_template_count()),
                config.limits.max_generated_probes,
            ));
        }
        let mut step_limits = config.equality_step;
        step_limits.max_total_scheduled_requests = step_limits.max_total_scheduled_requests.min(
            config
                .limits
                .max_scheduled_requests
                .saturating_sub(census.scheduled_requests),
        );
        step_limits.max_total_streamed_rows = step_limits.max_total_streamed_rows.min(
            config
                .limits
                .max_streamed_rows
                .saturating_sub(census.streamed_rows),
        );
        step_limits.max_total_modular_hits = step_limits.max_total_modular_hits.min(
            config
                .limits
                .max_modular_hits
                .saturating_sub(census.modular_hits),
        );
        step_limits.max_total_exact_lift_attempts = step_limits.max_total_exact_lift_attempts.min(
            config
                .limits
                .max_exact_lift_attempts
                .saturating_sub(census.exact_lift_attempts),
        );
        let allowed_probe_attempts = config
            .limits
            .max_probe_attempts
            .saturating_sub(census.probe_attempts);
        step_limits.max_probes = step_limits.max_probes.min(allowed_probe_attempts);
        if portfolio.probe_template_count() > step_limits.max_probes {
            return Ok(resource_stop(
                ledger,
                census,
                reports,
                Some(case_id),
                PROBE_ATTEMPTS,
                census
                    .probe_attempts
                    .saturating_add(portfolio.probe_template_count()),
                config.limits.max_probe_attempts.min(
                    census
                        .probe_attempts
                        .saturating_add(config.equality_step.max_probes),
                ),
            ));
        }
        let (probes, probe_census) = match try_build_case_probes(
            &current,
            generator.context().base().parameter_names().len(),
            portfolio,
            config.probe_campaign,
            remaining_probes,
            remaining_cells,
        ) {
            Ok(built) => built,
            Err(error) => {
                return Err(fail(
                    census,
                    reports,
                    SpiredSerialDriverErrorCause::ProbeBuild(error),
                ));
            }
        };
        census.generated_probes += probe_census.probes_generated();
        census.generated_probe_coordinate_cells += probe_census.retained_coordinate_cells();
        let step = match try_advance_spired_coordinate_case(
            generator,
            completed,
            &probes,
            worklist,
            ledger,
            step_limits,
        ) {
            Ok(step) => step,
            Err(error) => {
                if let Err(cause) = charge_step(&mut census, error.census()) {
                    return Err(fail(census, reports, cause));
                }
                return Err(fail(
                    census,
                    reports,
                    SpiredSerialDriverErrorCause::EqualityStep(error),
                ));
            }
        };
        let step_census = step.census();
        if let Err(cause) = charge_step(&mut census, step_census) {
            return Err(fail(census, reports, cause));
        }
        match step.into_outcome() {
            SpiredEqualityStepOutcome::Committed(committed) => {
                census.rule_commits += 1;
                census.retained_case_reports += 1;
                reports.push(case_report(
                    &current,
                    free_dimension,
                    probe_census,
                    Some(step_census),
                    SpiredSerialCaseOutcome::RuleCommitted(committed),
                ));
            }
            SpiredEqualityStepOutcome::Incomplete(incomplete) => {
                census.incomplete_case_attempts += 1;
                census.retained_case_reports += 1;
                let reason = SpiredSerialCaseIncomplete::EqualityStep(incomplete);
                reports.push(case_report(
                    &current,
                    free_dimension,
                    probe_census,
                    Some(step_census),
                    SpiredSerialCaseOutcome::Incomplete(reason.clone()),
                ));
                return Ok(finish(
                    ledger,
                    census,
                    reports,
                    StopKind::Incomplete { case_id, reason },
                ));
            }
        }
    }
}

fn singleton_integral(
    current: &SpiredCoordinateCaseObligation,
) -> Result<IntegralKey, IntegralKeyError> {
    IntegralKey::try_new(
        current
            .stratum()
            .domain()
            .bounds()
            .iter()
            .map(|bounds| bounds.lower()),
    )
}

fn case_report(
    current: &SpiredCoordinateCaseObligation,
    free_dimension: usize,
    probe_census: SpiredSerialProbeCensus,
    equality_census: Option<SpiredEqualityStepCensus>,
    outcome: SpiredSerialCaseOutcome,
) -> SpiredSerialCaseReport {
    SpiredSerialCaseReport::new(
        current.declared_carrier().id().clone(),
        current.stratum().id().clone(),
        free_dimension,
        probe_census,
        equality_census,
        outcome,
    )
}

fn charge_step(
    cumulative: &mut SpiredSerialDriverCensus,
    step: SpiredEqualityStepCensus,
) -> Result<(), SpiredSerialDriverErrorCause> {
    cumulative.probe_attempts = checked_add(
        PROBE_ATTEMPTS,
        cumulative.probe_attempts,
        step.probes_attempted(),
    )?;
    cumulative.singular_probes_skipped = checked_add(
        "singular probes skipped",
        cumulative.singular_probes_skipped,
        step.singular_probes_skipped(),
    )?;
    cumulative.scheduled_requests = checked_add(
        SCHEDULED_REQUESTS,
        cumulative.scheduled_requests,
        step.scheduled_requests(),
    )?;
    cumulative.streamed_rows = checked_add(
        STREAMED_ROWS,
        cumulative.streamed_rows,
        step.streamed_rows(),
    )?;
    cumulative.modular_hits =
        checked_add(MODULAR_HITS, cumulative.modular_hits, step.modular_hits())?;
    cumulative.exact_lift_attempts = checked_add(
        EXACT_LIFT_ATTEMPTS,
        cumulative.exact_lift_attempts,
        step.exact_lift_attempts(),
    )?;
    cumulative.admitted_candidates = checked_add(
        "admitted candidates",
        cumulative.admitted_candidates,
        step.admitted_candidates(),
    )?;
    cumulative.exact_guard_cases = checked_add(
        "exact guard cases",
        cumulative.exact_guard_cases,
        step.exact_guard_cases(),
    )?;
    cumulative.owner_compile_attempts = checked_add(
        "owner compilation attempts",
        cumulative.owner_compile_attempts,
        step.owner_compile_attempts(),
    )?;
    Ok(())
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredSerialDriverErrorCause> {
    left.checked_add(right)
        .ok_or(SpiredSerialDriverErrorCause::ResourceCountOverflow { resource })
}

/// Materialize an exact finite compiler residual for diagnosis.
///
/// This is deliberately not a terminal factory: callers may use the returned
/// keys only to reconcile already-existing terminal authority or to retain a
/// typed incomplete obligation.
fn try_finite_residual_integrals(
    ledger: &CanonicalExactOwnerLedger,
    partition: &UncoveredPartition,
    max_points: usize,
    max_coordinate_cells: usize,
) -> Result<Option<Vec<IntegralKey>>, CompletionGeometryError> {
    if partition.is_empty() || !partition.is_finite() {
        return Ok(None);
    }
    let count = match partition.try_cardinality(max_points)? {
        LatticeCardinality::Finite(count) => count,
        LatticeCardinality::Infinite => {
            return Err(CompletionGeometryError::Invariant {
                detail: "a finite residual reported infinite cardinality",
            });
        }
    };
    let coordinate_cells = count.checked_mul(ledger.sector().arity()).ok_or(
        CompletionGeometryError::ResourceCountOverflow {
            resource: "finite residual coordinate cells",
        },
    )?;
    if coordinate_cells > max_coordinate_cells {
        return Err(CompletionGeometryError::ResourceLimit {
            resource: "finite residual coordinate cells",
            requested: coordinate_cells,
            limit: max_coordinate_cells,
        });
    }

    let mut integrals = Vec::new();
    integrals
        .try_reserve_exact(count)
        .map_err(|_| CompletionGeometryError::AllocationFailure {
            resource: "finite residual integrals",
            requested: count,
        })?;
    let chart = SectorChart::new(ledger.sector().clone());
    for cell in partition.boxes() {
        let mut coordinates = Vec::new();
        coordinates.try_reserve_exact(cell.arity()).map_err(|_| {
            CompletionGeometryError::AllocationFailure {
                resource: "finite residual point coordinates",
                requested: cell.arity(),
            }
        })?;
        coordinates.extend_from_slice(cell.lower());
        let mut upper = Vec::new();
        upper.try_reserve_exact(cell.arity()).map_err(|_| {
            CompletionGeometryError::AllocationFailure {
                resource: "finite residual upper coordinates",
                requested: cell.arity(),
            }
        })?;
        for &endpoint in cell.upper() {
            upper.push(endpoint.ok_or(CompletionGeometryError::Invariant {
                detail: "a finite residual retained an unbounded box",
            })?);
        }

        'cell_points: loop {
            let point = LatticePoint::try_new(coordinates.iter().copied())?;
            integrals.push(chart.to_integral(&point)?);
            for position in (0..coordinates.len()).rev() {
                if coordinates[position] < upper[position] {
                    coordinates[position] = coordinates[position].checked_add(1).ok_or(
                        CompletionGeometryError::ResourceCountOverflow {
                            resource: "finite residual lattice coordinate",
                        },
                    )?;
                    for reset in position + 1..coordinates.len() {
                        coordinates[reset] = cell.lower()[reset];
                    }
                    continue 'cell_points;
                }
            }
            break;
        }
    }
    debug_assert_eq!(integrals.len(), count);
    Ok(Some(integrals))
}

fn fail(
    census: SpiredSerialDriverCensus,
    reports: Vec<SpiredSerialCaseReport>,
    cause: SpiredSerialDriverErrorCause,
) -> SpiredSerialDriverError {
    SpiredSerialDriverError::new(census, reports, cause)
}

enum StopKind {
    CompilerClosed {
        pending_cases: usize,
    },
    BoundedCarrierClosed,
    WorklistExhausted,
    Incomplete {
        case_id: crate::foundry::completion::stratum::DecoratedStratumId,
        reason: SpiredSerialCaseIncomplete,
    },
}

fn finish(
    ledger: &CanonicalExactOwnerLedger,
    census: SpiredSerialDriverCensus,
    reports: Vec<SpiredSerialCaseReport>,
    kind: StopKind,
) -> SpiredSerialDriverReport {
    let authority = ledger.snapshot_identity();
    let snapshot = ledger.snapshot();
    let stop = match kind {
        StopKind::CompilerClosed { pending_cases } => SpiredSerialDriverStop::CompilerClosed {
            authority,
            snapshot,
            pending_cases,
        },
        StopKind::BoundedCarrierClosed => SpiredSerialDriverStop::BoundedCarrierClosed {
            authority,
            snapshot,
        },
        StopKind::WorklistExhausted => SpiredSerialDriverStop::WorklistExhausted {
            authority,
            snapshot,
        },
        StopKind::Incomplete { case_id, reason } => SpiredSerialDriverStop::CurrentCaseIncomplete {
            authority,
            snapshot,
            case_id,
            reason,
            resume:
                SpiredSerialResumePolicy::RestartCurrentCaseFromFirstConfiguredDiagonalAndDepthZero,
        },
    };
    SpiredSerialDriverReport::new(census, reports, stop)
}

fn finite_residual_stop(
    ledger: &CanonicalExactOwnerLedger,
    census: SpiredSerialDriverCensus,
    reports: Vec<SpiredSerialCaseReport>,
    integrals: Vec<IntegralKey>,
    reason: SpiredSerialFiniteResidualReason,
) -> SpiredSerialDriverReport {
    let stop = SpiredSerialDriverStop::FiniteResidualIncomplete {
        authority: ledger.snapshot_identity(),
        snapshot: ledger.snapshot(),
        integrals: integrals.into_boxed_slice(),
        reason,
        resume: SpiredSerialResumePolicy::RestartCurrentCaseFromFirstConfiguredDiagonalAndDepthZero,
    };
    SpiredSerialDriverReport::new(census, reports, stop)
}

fn resource_stop(
    ledger: &CanonicalExactOwnerLedger,
    census: SpiredSerialDriverCensus,
    reports: Vec<SpiredSerialCaseReport>,
    current_case_id: Option<crate::foundry::completion::stratum::DecoratedStratumId>,
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> SpiredSerialDriverReport {
    let stop = SpiredSerialDriverStop::ResourceLimit {
        authority: ledger.snapshot_identity(),
        snapshot: ledger.snapshot(),
        current_case_id,
        resource,
        requested,
        limit,
        resume: SpiredSerialResumePolicy::RestartCurrentCaseFromFirstConfiguredDiagonalAndDepthZero,
    };
    SpiredSerialDriverReport::new(census, reports, stop)
}
