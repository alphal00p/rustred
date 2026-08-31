use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::cell::{RuleCell, RuleCellError};
use crate::foundry::completion::frame::admission::{
    ExactGuardRefinementOutcome, try_refine_exact_circuit_guards,
};
use crate::foundry::completion::frame::exact::{
    ExactCircuitLoweringError, ExactTargetCircuit, try_lower_exact_circuit,
};
use crate::foundry::completion::source_discovery::FreshTaskEpoch;
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
    if circuit.owner_snapshot_id() != epoch.fixed_snapshot_id() {
        return Err(ExactRuleCellPromotionError::OwnerSnapshotMismatch);
    }

    let partition = epoch.try_partition(limits.partition)?;
    validate_exact_residual_owners(&epoch, &circuit, &partition)?;
    let refinement = match try_refine_exact_circuit_guards(
        context,
        &circuit,
        &partition,
        limits.guard_refinement,
    )? {
        ExactGuardRefinementOutcome::Admitted(refinement) => refinement,
        ExactGuardRefinementOutcome::BlockedByKnownZero {
            required_predicate_ordinal,
            first_circuit_guard_ordinal,
            zero_branch,
        } => {
            drop(partition);
            return Ok(ExactRuleCellPromotionDisposition::BlockedByKnownZero {
                epoch,
                circuit,
                required_predicate_ordinal,
                first_circuit_guard_ordinal,
                zero_branch,
            });
        }
    };

    let lowered =
        match try_lower_exact_circuit(context, epoch.plan(), &circuit, anchor, limits.lowering) {
            Ok(lowered) => lowered,
            Err(ExactCircuitLoweringError::Parametric(
                ParametricRuleError::GuardVanishedAtAnchor { guard_ordinal },
            )) => {
                drop(partition);
                return Ok(ExactRuleCellPromotionDisposition::AnchorOnGuardWall {
                    epoch,
                    circuit,
                    refinement,
                    guard_ordinal,
                });
            }
            Err(error) => return Err(ExactRuleCellPromotionError::Lowering(error)),
        };
    validate_lowered_join(&epoch, &circuit, &lowered)?;
    let (rule, sources) = lowered.into_parts();
    let cell = match RuleCell::try_refined(
        context,
        rule,
        sources,
        epoch.fixed_stratum().domain().clone(),
        [],
        [],
        limits.cell,
    ) {
        Ok(cell) => cell,
        Err(RuleCellError::GuardVanishesInApplicationDomain {
            ordinal,
            position,
            value,
        }) => {
            drop(partition);
            return Ok(ExactRuleCellPromotionDisposition::NeedsGuardedStratum {
                epoch,
                circuit,
                refinement,
                obstruction: ExactRuleCellGuardObstruction::IntegerRoot {
                    guard_ordinal: ordinal,
                    position,
                    value,
                },
            });
        }
        Err(RuleCellError::UnsupportedMultivariateGuardLocus { ordinal }) => {
            drop(partition);
            return Ok(ExactRuleCellPromotionDisposition::NeedsGuardedStratum {
                epoch,
                circuit,
                refinement,
                obstruction: ExactRuleCellGuardObstruction::UnsupportedMultivariate {
                    guard_ordinal: ordinal,
                },
            });
        }
        Err(error) => return Err(ExactRuleCellPromotionError::Cell(error)),
    };
    drop(partition);
    Ok(ExactRuleCellPromotionDisposition::Admitted(
        AdmittedExactRuleCandidate::new(epoch, circuit, cell, refinement),
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
    if rule.nonzero_guards().len() != circuit.nonzero_guards().len()
        || rule
            .nonzero_guards()
            .iter()
            .zip(circuit.nonzero_guards())
            .any(|(lowered, exact)| lowered.polynomial() != exact.polynomial())
    {
        return Err(ExactRuleCellPromotionError::LoweredJoin {
            detail: "lowered semantic guard content or chronology changed",
        });
    }
    Ok(())
}
