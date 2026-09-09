use crate::foundry::completion::SectorChart;
use crate::foundry::completion::source_discovery::{
    AdmittedExactRuleCandidate, CampaignModularProbe, CanonicalExactOwnerLedger,
    ExactExecutableOwnerProposal, ExactOwnerCoverDeltaKind, ExactRuleCellPromotionDisposition,
    try_compile_single_canonical_probe_executable_owner,
};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};

use super::super::{
    SpiredCase, SpiredCoordinateCaseObligation, SpiredCoordinateCaseWorklist, SpiredCoordinateFace,
    SpiredCoordinateGuardCaseOutcome, SpiredExecutionCase, SpiredFoundationError,
    SpiredStreamingError, SpiredTargetRunCensus, SpiredTargetRunErrorCause, SpiredTargetRunOutcome,
    SpiredTargetRunWorkspace, try_materialize_spired_coordinate_guard_cases,
};
use super::{
    SpiredEqualityStepCensus, SpiredEqualityStepCommitted, SpiredEqualityStepError,
    SpiredEqualityStepIncomplete, SpiredEqualityStepLimits, SpiredEqualityStepOutcome,
    SpiredEqualityStepProbeIncomplete, SpiredEqualityStepReport,
};

const PROBES: &str = "equality-step probes";
const SCHEDULED_REQUESTS: &str = "equality-step scheduled requests";
const STREAMED_ROWS: &str = "equality-step streamed rows";
const MODULAR_HITS: &str = "equality-step modular hits";
const EXACT_LIFT_ATTEMPTS: &str = "equality-step exact-lift attempts";

/// Advance exactly one highest-priority already-safe coordinate case.
///
/// The probe slice is its complete deterministic chronology. The same
/// structural workspace is reused, while every probe still receives fresh
/// modular reducer state. Only an exactly replayed, admitted candidate may
/// reach guard materialization or propose a live owner.
pub(crate) fn try_advance_spired_coordinate_case(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    probes: &[CampaignModularProbe],
    worklist: &mut SpiredCoordinateCaseWorklist,
    ledger: &mut CanonicalExactOwnerLedger,
    limits: SpiredEqualityStepLimits,
) -> Result<SpiredEqualityStepReport, SpiredEqualityStepError> {
    let mut census = SpiredEqualityStepCensus {
        probes_offered: probes.len(),
        ..Default::default()
    };
    if probes.len() > limits.max_probes {
        return Err(SpiredEqualityStepError::limit(
            census,
            PROBES,
            probes.len(),
            limits.max_probes,
        ));
    }
    let Some(current) = worklist.cloned_current() else {
        return Ok(incomplete(
            census,
            SpiredEqualityStepIncomplete::EmptyWorklist,
        ));
    };
    if probes.is_empty() {
        return Ok(incomplete(
            census,
            SpiredEqualityStepIncomplete::EmptyProbePortfolio,
        ));
    }
    if current.stratum().domain().sector() != ledger.sector() {
        return Err(SpiredEqualityStepError::wrong_ledger_sector(census));
    }

    let ledger_epoch = ledger.snapshot_identity();
    let target = IntegralShift::try_new(std::iter::repeat_n(0, current.stratum().domain().arity()))
        .map_err(|error| {
            SpiredEqualityStepError::foundation(census, SpiredFoundationError::from(error))
        })?;
    let execution = SpiredExecutionCase::try_new(
        SpiredCase::CoordinateFace(SpiredCoordinateFace::new(current.stratum().clone())),
        target,
        ledger.ordering(),
        ledger.predecessor_snapshot().clone(),
    )
    .map_err(|error| SpiredEqualityStepError::foundation(census, error))?;
    let mut workspace =
        SpiredTargetRunWorkspace::try_new(generator, completed, &execution, limits.target_run)
            .map_err(|error| SpiredEqualityStepError::target_run(census, 0, error))?;

    let mut last_incomplete = None;
    for (probe_ordinal, probe) in probes.iter().enumerate() {
        census.probes_attempted = checked_add(census, PROBES, census.probes_attempted, 1)?;
        let report = match workspace.try_run_probe(probe) {
            Ok(report) => report,
            Err(error) if is_retryable_singular_probe(&error) => {
                charge_target_census(&mut census, error.census(), limits)?;
                census.singular_probes_skipped = checked_add(
                    census,
                    "equality-step singular probes",
                    census.singular_probes_skipped,
                    1,
                )?;
                last_incomplete = Some(SpiredEqualityStepProbeIncomplete::SingularModularSample);
                continue;
            }
            Err(error) => {
                charge_target_census(&mut census, error.census(), limits)?;
                return Err(SpiredEqualityStepError::target_run(
                    census,
                    probe_ordinal,
                    error,
                ));
            }
        };
        charge_target_census(&mut census, report.census(), limits)?;
        match report.into_outcome() {
            SpiredTargetRunOutcome::RuleCell(ExactRuleCellPromotionDisposition::Admitted(
                admitted,
            )) => {
                census.admitted_candidates = checked_add(
                    census,
                    "equality-step admitted candidates",
                    census.admitted_candidates,
                    1,
                )?;
                return try_commit_admitted(
                    generator,
                    current,
                    admitted,
                    worklist,
                    ledger,
                    ledger_epoch,
                    limits,
                    census,
                );
            }
            SpiredTargetRunOutcome::RuleCell(
                ExactRuleCellPromotionDisposition::BlockedByKnownZero {
                    required_predicate_ordinal,
                    ..
                },
            ) => {
                last_incomplete = Some(SpiredEqualityStepProbeIncomplete::BlockedByKnownZero {
                    required_predicate_ordinal,
                });
            }
            SpiredTargetRunOutcome::RuleCell(
                ExactRuleCellPromotionDisposition::NeedsGuardedStratum { .. },
            ) => {
                last_incomplete = Some(SpiredEqualityStepProbeIncomplete::NeedsGuardedStratum);
            }
            SpiredTargetRunOutcome::RuleCell(
                ExactRuleCellPromotionDisposition::AnchorOnGuardWall { guard_ordinal, .. },
            ) => {
                last_incomplete =
                    Some(SpiredEqualityStepProbeIncomplete::AnchorOnGuardWall { guard_ordinal });
            }
            SpiredTargetRunOutcome::CompactInconclusive { exhaustion, .. } => {
                last_incomplete = Some(
                    SpiredEqualityStepProbeIncomplete::CompactReplayInconclusive { exhaustion },
                );
            }
            SpiredTargetRunOutcome::KnownZeroPromotionInconclusive { last, exhaustion } => {
                last_incomplete = Some(
                    SpiredEqualityStepProbeIncomplete::KnownZeroPromotionInconclusive {
                        required_predicate_ordinal: last.required_predicate_ordinal(),
                        exhaustion,
                    },
                );
            }
            SpiredTargetRunOutcome::SearchDepthExhausted => {
                last_incomplete = Some(SpiredEqualityStepProbeIncomplete::SearchDepthExhausted);
            }
        }
    }

    Ok(incomplete(
        census,
        SpiredEqualityStepIncomplete::ProbePortfolioExhausted {
            probes_attempted: census.probes_attempted,
            last: last_incomplete,
        },
    ))
}

#[allow(clippy::too_many_arguments)]
fn try_commit_admitted(
    generator: &ParametricIbpGenerator<'_>,
    current: SpiredCoordinateCaseObligation,
    admitted: AdmittedExactRuleCandidate,
    worklist: &mut SpiredCoordinateCaseWorklist,
    ledger: &mut CanonicalExactOwnerLedger,
    ledger_epoch: crate::foundry::completion::source_discovery::ExactOwnerLedgerSnapshotIdentity,
    limits: SpiredEqualityStepLimits,
    mut census: SpiredEqualityStepCensus,
) -> Result<SpiredEqualityStepReport, SpiredEqualityStepError> {
    // Children are exact equality consequences of the admitted circuit and
    // must exist before `admitted` is consumed by singleton owner compilation.
    let guard_cases = try_materialize_spired_coordinate_guard_cases(
        generator.context(),
        admitted.circuit(),
        admitted.cleared(),
        admitted.guard_refinement(),
        &current,
        limits.guard_cases,
    )
    .map_err(|error| SpiredEqualityStepError::guard_cases(census, error))?;
    let guard_cases = match guard_cases {
        SpiredCoordinateGuardCaseOutcome::Exact(cases) => cases,
        SpiredCoordinateGuardCaseOutcome::CandidateUnusable(rejection) => {
            return Ok(incomplete(
                census,
                SpiredEqualityStepIncomplete::GuardCaseCandidateUnusable(rejection),
            ));
        }
        SpiredCoordinateGuardCaseOutcome::Incomplete(reason) => {
            return Ok(incomplete(
                census,
                SpiredEqualityStepIncomplete::GuardCaseGeometry(reason),
            ));
        }
    };
    census.exact_guard_cases = guard_cases.cases().len();

    census.owner_compile_attempts = checked_add(
        census,
        "equality-step owner compilation attempts",
        census.owner_compile_attempts,
        1,
    )?;
    let proposal = try_compile_single_canonical_probe_executable_owner(
        generator.context(),
        admitted,
        limits.executable_owner,
    )
    .map_err(|error| SpiredEqualityStepError::owner_compilation(census, error))?;
    let owner = match proposal {
        ExactExecutableOwnerProposal::Compiled { owner, .. } => owner,
        ExactExecutableOwnerProposal::Incomplete(incomplete_owner) => {
            return Ok(incomplete(
                census,
                SpiredEqualityStepIncomplete::OwnerCompilationIncomplete {
                    obstructions: incomplete_owner.obstructions().len(),
                },
            ));
        }
    };

    // Rejoin the exact immutable ledger epoch captured before search. This is
    // redundant under the present exclusive caller, but makes the intended
    // worker/proposal boundary explicit and rejects future delayed results.
    ledger
        .try_require_current_snapshot(&ledger_epoch)
        .map_err(|error| SpiredEqualityStepError::owner_ledger(census, error))?;
    let prepared_owner = ledger
        .try_prepare_owner_mutation(owner)
        .map_err(|error| SpiredEqualityStepError::owner_ledger(census, error))?;
    let delta = prepared_owner.delta();
    // `Compiled(Closed)` is scoped to the ledger's retained closure carrier.
    // A diagnostic finite carrier may close while a logical equality child
    // still has canonical full free axes beyond it.  Such a bounded verdict
    // may retain the globally replayed owner, but it cannot retire global
    // discovery obligations.  Only the complete representable sector carrier
    // is allowed to discharge guard-zero children here.
    let full_carrier = SectorChart::new(ledger.sector().clone())
        .carrier_box()
        .map_err(|error| SpiredEqualityStepError::owner_ledger(census, error.into()))?;
    let discharged_by_compiler_cover =
        delta.updated().status().is_compiler_closed() && ledger.closure_carrier() == &full_carrier;
    match delta.kind() {
        ExactOwnerCoverDeltaKind::StrictGeometricShrink => {}
        ExactOwnerCoverDeltaKind::Duplicate
        | ExactOwnerCoverDeltaKind::ChangedWithoutGeometricShrink => {
            if !discharged_by_compiler_cover {
                return Ok(incomplete(
                    census,
                    SpiredEqualityStepIncomplete::OwnerMadeNoGeometricProgress {
                        kind: delta.kind(),
                        updated_status: delta.updated().status(),
                    },
                ));
            }
        }
    }

    let proposed_guard_children = if discharged_by_compiler_cover {
        0
    } else {
        guard_cases.cases().len()
    };
    let empty: &[SpiredCoordinateCaseObligation] = &[];
    let replacement_children = if discharged_by_compiler_cover {
        empty
    } else {
        guard_cases.cases()
    };
    let prepared_replacement = worklist
        .try_prepare_current_replacement(&current, replacement_children.iter().cloned())
        .map_err(|error| SpiredEqualityStepError::worklist(census, error))?;

    // Both fallible preparations are now complete. Bind both opaque epochs
    // before either install. The two remaining commits cannot fail, allocate,
    // invoke CAS, or observe user-controlled data.
    let validated_owner = ledger
        .try_validate_prepared_owner_mutation(prepared_owner)
        .map_err(|error| SpiredEqualityStepError::owner_ledger(census, error))?;
    let validated_replacement = worklist
        .try_validate_prepared_replacement(prepared_replacement)
        .map_err(|error| SpiredEqualityStepError::worklist(census, error))?;
    let committed_delta = validated_owner.commit();
    validated_replacement.commit();
    census.committed_steps = 1;
    debug_assert_eq!(committed_delta, delta);

    Ok(SpiredEqualityStepReport::new(
        census,
        SpiredEqualityStepOutcome::Committed(SpiredEqualityStepCommitted::new(
            committed_delta,
            proposed_guard_children,
            worklist.len(),
            discharged_by_compiler_cover,
        )),
    ))
}

fn is_retryable_singular_probe(error: &super::super::SpiredTargetRunError) -> bool {
    matches!(
        error.cause(),
        SpiredTargetRunErrorCause::Streaming(SpiredStreamingError::Evaluation(
            super::super::DirectShiftedSourceError::ConditionZero { .. }
                | super::super::DirectShiftedSourceError::TermDenominatorZero { .. }
        ))
    )
}

fn charge_target_census(
    aggregate: &mut SpiredEqualityStepCensus,
    run: SpiredTargetRunCensus,
    limits: SpiredEqualityStepLimits,
) -> Result<(), SpiredEqualityStepError> {
    aggregate.scheduled_requests = checked_bounded_add(
        *aggregate,
        SCHEDULED_REQUESTS,
        aggregate.scheduled_requests,
        run.scheduled_requests(),
        limits.max_total_scheduled_requests,
    )?;
    aggregate.streamed_rows = checked_bounded_add(
        *aggregate,
        STREAMED_ROWS,
        aggregate.streamed_rows,
        run.streamed_rows(),
        limits.max_total_streamed_rows,
    )?;
    aggregate.modular_hits = checked_bounded_add(
        *aggregate,
        MODULAR_HITS,
        aggregate.modular_hits,
        run.modular_hits_seen(),
        limits.max_total_modular_hits,
    )?;
    // `distinct_compact_lift_attempts` is the total; rooted attempts are its
    // diagnostic subset, not an additional class of work.
    let lift_attempts = run.distinct_compact_lift_attempts();
    aggregate.exact_lift_attempts = checked_bounded_add(
        *aggregate,
        EXACT_LIFT_ATTEMPTS,
        aggregate.exact_lift_attempts,
        lift_attempts,
        limits.max_total_exact_lift_attempts,
    )?;
    Ok(())
}

fn incomplete(
    census: SpiredEqualityStepCensus,
    reason: SpiredEqualityStepIncomplete,
) -> SpiredEqualityStepReport {
    SpiredEqualityStepReport::new(census, SpiredEqualityStepOutcome::Incomplete(reason))
}

fn checked_add(
    census: SpiredEqualityStepCensus,
    resource: &'static str,
    current: usize,
    increment: usize,
) -> Result<usize, SpiredEqualityStepError> {
    current
        .checked_add(increment)
        .ok_or_else(|| SpiredEqualityStepError::overflow(census, resource))
}

fn checked_bounded_add(
    census: SpiredEqualityStepCensus,
    resource: &'static str,
    current: usize,
    increment: usize,
    limit: usize,
) -> Result<usize, SpiredEqualityStepError> {
    let requested = checked_add(census, resource, current, increment)?;
    if requested > limit {
        Err(SpiredEqualityStepError::limit(
            census, resource, requested, limit,
        ))
    } else {
        Ok(requested)
    }
}
