use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::cell::{
    FixedIndexRestriction, RuleCell, RuleCellError, try_single_guard_domain_split,
};
use crate::foundry::completion::frame::admission::{
    ExactGuardRefinementOutcome, try_refine_cleared_exact_circuit_guards,
};
use crate::foundry::completion::frame::exact::{
    ClearedExactCircuit, ExactCircuitLoweringError, ExactTargetCircuit, try_clear_exact_circuit,
    try_lower_cleared_exact_circuit,
};
use crate::foundry::completion::source_discovery::FreshTaskEpoch;
use crate::foundry::completion::stratum::TargetColumnPartition;
use crate::foundry::parametric::ParametricRuleError;

use super::{
    AdmittedExactRuleCandidate, ExactRuleCellGuardObstruction, ExactRuleCellPromotionDisposition,
    ExactRuleCellPromotionError, ExactRuleCellPromotionLimits,
};

/// Rejoin and promote one exact scheduler result without trusting any stale
/// row/column ordinal.  No cover is mutated by this function.
pub(crate) fn try_promote_replayed_rule_cell(
    context: &IndexedCoefficientContext,
    epoch: Arc<FreshTaskEpoch>,
    circuit: Arc<ExactTargetCircuit>,
    anchor: &[i64],
    limits: ExactRuleCellPromotionLimits,
) -> Result<ExactRuleCellPromotionDisposition, ExactRuleCellPromotionError> {
    let partition = epoch.try_partition(limits.partition)?;
    try_promote_replayed_rule_cell_on_partition(
        context,
        epoch.clone(),
        circuit,
        anchor,
        &partition,
        limits,
    )
}

/// Promote one candidate against a partition already rebuilt and authenticated
/// for its canonical batch. This avoids repeating the cold owner/stratum join
/// for every exact candidate while retaining the same hard checks.
pub(crate) fn try_promote_replayed_rule_cell_on_partition(
    context: &IndexedCoefficientContext,
    epoch: Arc<FreshTaskEpoch>,
    circuit: Arc<ExactTargetCircuit>,
    anchor: &[i64],
    partition: &TargetColumnPartition<'_>,
    limits: ExactRuleCellPromotionLimits,
) -> Result<ExactRuleCellPromotionDisposition, ExactRuleCellPromotionError> {
    if context.fingerprint() != epoch.plan().context_fingerprint() {
        return Err(ExactRuleCellPromotionError::WrongContext);
    }
    if !circuit.is_bound_to(epoch.plan()) {
        return Err(ExactRuleCellPromotionError::WrongPhysicalPlan);
    }
    if circuit.target_column() != epoch.target_column()
        || circuit.target_shift() != epoch.target_shift()
    {
        return Err(ExactRuleCellPromotionError::TargetMismatch);
    }
    if circuit.stratum_id() != epoch.fixed_stratum().id() {
        return Err(ExactRuleCellPromotionError::StratumMismatch);
    }
    if !circuit
        .fixed_indices()
        .iter()
        .copied()
        .eq(epoch.fixed_stratum().singleton_index_assignments())
    {
        return Err(ExactRuleCellPromotionError::StratumMismatch);
    }
    if circuit.owner_snapshot_id() != epoch.fixed_snapshot_id() {
        return Err(ExactRuleCellPromotionError::OwnerSnapshotMismatch);
    }
    if !std::ptr::eq(partition.frame(), epoch.plan()) {
        return Err(ExactRuleCellPromotionError::WrongPhysicalPlan);
    }
    if partition.target_column() != epoch.target_column() {
        return Err(ExactRuleCellPromotionError::TargetMismatch);
    }
    if partition.stratum_id() != epoch.fixed_stratum().id() {
        return Err(ExactRuleCellPromotionError::StratumMismatch);
    }
    if partition.snapshot_id() != epoch.fixed_snapshot_id() {
        return Err(ExactRuleCellPromotionError::OwnerSnapshotMismatch);
    }
    if partition.ordering() != epoch.fixed_ordering() {
        return Err(ExactRuleCellPromotionError::OrderingMismatch);
    }

    validate_exact_residual_owners(&epoch, &circuit, partition)?;
    let cleared = Arc::new(try_clear_exact_circuit(
        context,
        epoch.plan(),
        &circuit,
        limits.clearing,
    )?);
    let refinement = match try_refine_cleared_exact_circuit_guards(
        context,
        &circuit,
        &cleared,
        partition,
        limits.guard_refinement,
    )? {
        ExactGuardRefinementOutcome::Admitted(refinement) => refinement,
        ExactGuardRefinementOutcome::BlockedByKnownZero {
            required_predicate_ordinal,
            first_circuit_guard_ordinal,
            zero_branch,
        } => {
            return Ok(ExactRuleCellPromotionDisposition::BlockedByKnownZero {
                epoch,
                circuit,
                cleared,
                required_predicate_ordinal,
                first_circuit_guard_ordinal,
                zero_branch,
            });
        }
    };

    let lowered = match try_lower_cleared_exact_circuit(
        context,
        epoch.plan(),
        &circuit,
        &cleared,
        anchor,
        limits.lowering,
    ) {
        Ok(lowered) => lowered,
        Err(ExactCircuitLoweringError::Parametric(
            ParametricRuleError::GuardVanishedAtAnchor { guard_ordinal },
        )) => {
            return Ok(ExactRuleCellPromotionDisposition::AnchorOnGuardWall {
                epoch,
                circuit,
                cleared,
                refinement,
                guard_ordinal,
            });
        }
        Err(error) => return Err(ExactRuleCellPromotionError::Lowering(error)),
    };
    validate_lowered_join(&epoch, &circuit, &cleared, &lowered)?;
    let (rule, sources) = lowered.into_parts();
    let fixed = circuit
        .fixed_indices()
        .iter()
        .map(|&(position, value)| FixedIndexRestriction::new(position, value))
        .collect::<Vec<_>>();
    let guard_domain_split = try_single_guard_domain_split(
        context,
        &rule,
        epoch.fixed_stratum().domain(),
        &fixed,
        limits.cell,
    )
    .map_err(ExactRuleCellPromotionError::Cell)?;
    let application_domain = guard_domain_split
        .as_ref()
        .map_or_else(
            || epoch.fixed_stratum().domain(),
            |split| split.admitted_domain(),
        )
        .clone();
    let cell = match RuleCell::try_refined(
        context,
        rule,
        sources,
        application_domain,
        fixed,
        [],
        limits.cell,
    ) {
        Ok(cell) => cell,
        Err(RuleCellError::GuardVanishesInApplicationDomain {
            ordinal,
            position,
            value,
        }) => {
            return Ok(ExactRuleCellPromotionDisposition::NeedsGuardedStratum {
                epoch,
                circuit,
                cleared,
                refinement,
                obstruction: ExactRuleCellGuardObstruction::IntegerRoot {
                    guard_ordinal: ordinal,
                    position,
                    value,
                },
            });
        }
        Err(RuleCellError::UnsupportedMultivariateGuardLocus { ordinal }) => {
            return Ok(ExactRuleCellPromotionDisposition::NeedsGuardedStratum {
                epoch,
                circuit,
                cleared,
                refinement,
                obstruction: ExactRuleCellGuardObstruction::UnsupportedMultivariate {
                    guard_ordinal: ordinal,
                },
            });
        }
        Err(error) => return Err(ExactRuleCellPromotionError::Cell(error)),
    };
    Ok(ExactRuleCellPromotionDisposition::Admitted(
        AdmittedExactRuleCandidate::new(
            epoch,
            circuit,
            cleared,
            Arc::new(cell),
            refinement,
            guard_domain_split,
        ),
    ))
}

fn validate_exact_residual_owners(
    epoch: &FreshTaskEpoch,
    circuit: &ExactTargetCircuit,
    partition: &crate::foundry::completion::stratum::TargetColumnPartition<'_>,
) -> Result<(), ExactRuleCellPromotionError> {
    for (ordinal, term) in circuit.residual_terms().iter().enumerate() {
        if term.descent().policy() != epoch.fixed_ordering() {
            return Err(ExactRuleCellPromotionError::OrderingMismatch);
        }
        if term.descent().domain() != epoch.fixed_stratum().domain() {
            return Err(ExactRuleCellPromotionError::ResidualJoin {
                ordinal,
                detail: "descent domain differs from the retained epoch stratum",
            });
        }
        let descriptor = partition.allowed_descriptor(term.physical_column()).ok_or(
            ExactRuleCellPromotionError::ResidualJoin {
                ordinal,
                detail: "physical residual is no longer an allowed column",
            },
        )?;
        if descriptor.descent() != term.descent() {
            return Err(ExactRuleCellPromotionError::ResidualJoin {
                ordinal,
                detail: "descent witness differs from the rebuilt partition",
            });
        }
        if descriptor.proper_subsector_owners() != term.proper_subsector_owners() {
            return Err(ExactRuleCellPromotionError::ResidualJoin {
                ordinal,
                detail: "lower-sector witnesses differ from the rebuilt owner snapshot",
            });
        }
    }
    Ok(())
}

fn validate_lowered_join(
    epoch: &FreshTaskEpoch,
    circuit: &ExactTargetCircuit,
    cleared: &ClearedExactCircuit,
    lowered: &crate::foundry::completion::frame::exact::LoweredExactCircuit,
) -> Result<(), ExactRuleCellPromotionError> {
    let rule = lowered.rule();
    if rule.ordering() != epoch.fixed_ordering() {
        return Err(ExactRuleCellPromotionError::OrderingMismatch);
    }
    let admission =
        rule.sector_monotone_admission()
            .ok_or(ExactRuleCellPromotionError::LoweredJoin {
                detail: "lowered rule lost its sector-monotone admission",
            })?;
    if admission.domain() != epoch.fixed_stratum().domain()
        || admission.pivot().values() != circuit.target_shift().values()
        || admission.dependencies().len() != circuit.residual_terms().len()
    {
        return Err(ExactRuleCellPromotionError::LoweredJoin {
            detail: "lowered admission domain, pivot, or dependency count changed",
        });
    }
    for (ordinal, (dependency, term)) in admission
        .dependencies()
        .iter()
        .zip(circuit.residual_terms())
        .enumerate()
    {
        if dependency.right_hand_side_ordinal() != ordinal
            || dependency.pivot_shift().values() != circuit.target_shift().values()
            || dependency.shift().values() != term.shift().values()
            || dependency.descent() != term.descent()
        {
            return Err(ExactRuleCellPromotionError::LoweredJoin {
                detail: "lowered dependency chronology or descent changed",
            });
        }
    }
    if rule.nonzero_guards().len() != cleared.semantic_guards().len()
        || rule
            .nonzero_guards()
            .iter()
            .zip(cleared.semantic_guards())
            .any(|(lowered, exact)| lowered.polynomial() != exact.polynomial())
    {
        return Err(ExactRuleCellPromotionError::LoweredJoin {
            detail: "lowered semantic guard content or chronology changed",
        });
    }
    Ok(())
}
