use std::sync::Arc;

use crate::foundry::completion::frame::exact::{ExactCircuitLift, RootedExactCircuitLift};
use crate::foundry::completion::frame::modular::ModularTargetQuery;
use crate::foundry::completion::source_discovery::{
    AccumulatedSourceRequests, CampaignModularProbe, FreshTaskEpoch, GrowingTaskEpochState,
};
use crate::foundry::completion::stratum::CampaignStratumAnchor;
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};

use super::super::{
    PrunedExactMaterializer, SpiredExecutionCase, SpiredModularHit, SpiredPostHitCandidate,
    TargetRuleMaterializer,
};
use super::{
    SpiredCompactLift, SpiredCompactLiftError, SpiredCompactLiftLimits, SpiredReplayedCompactLift,
};

/// Rebuild and exactly replay only the translated-source support nominated by
/// one streaming modular hit.
///
/// The streaming hit contributes no finite-field coefficient or frame
/// authority here. Its canonical support becomes a new immutable request set,
/// while its checked dependency-topological projection supplies only the
/// physical row chronology. A fresh selected-source frame then repeats the
/// modular query from the original integer probe before the existing exact
/// materializer may run. There is deliberately no complete-frame fallback in
/// this boundary.
pub(crate) fn try_lift_spired_compact_support(
    case: &SpiredExecutionCase,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    streaming_hit: &SpiredModularHit,
    probe: &CampaignModularProbe,
    limits: SpiredCompactLiftLimits,
) -> Result<SpiredCompactLift, SpiredCompactLiftError> {
    // This check intentionally precedes request retention, translated-source
    // materialization, fresh-epoch construction, and Symbolica. Large modular
    // traces are discovery evidence only and must not force an unbounded exact
    // polynomial reduction merely to discover the configured row ceiling.
    if let Some((requested_rows, limit)) =
        exact_support_budget_exceeded(streaming_hit.dependency_order().len(), limits)
    {
        return Ok(SpiredCompactLift::ExactSupportBudgetExceeded {
            probe: probe.clone(),
            requested_rows,
            limit,
        });
    }
    try_lift_with_materializer(
        case,
        generator,
        completed,
        streaming_hit,
        probe,
        limits,
        &PrunedExactMaterializer,
    )
}

/// Rebuild and exactly replay the compact ancestor DAG of one later GPLU row.
///
/// The dependency trace controls only physical row chronology. A fresh frame
/// repeats the original modular target query, then the exact materializer
/// independently forces the designated final row to cancel every forbidden
/// column over its predecessors. No residue or modular multiplier crosses the
/// boundary.
pub(crate) fn try_lift_spired_rooted_compact_support(
    case: &SpiredExecutionCase,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    candidate: &SpiredPostHitCandidate,
    probe: &CampaignModularProbe,
    limits: SpiredCompactLiftLimits,
) -> Result<SpiredCompactLift, SpiredCompactLiftError> {
    try_lift_rooted_with_materializer(
        case,
        generator,
        completed,
        candidate,
        probe,
        limits,
        &PrunedExactMaterializer,
    )
}

#[allow(clippy::too_many_arguments)]
fn try_lift_rooted_with_materializer(
    case: &SpiredExecutionCase,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    candidate: &SpiredPostHitCandidate,
    probe: &CampaignModularProbe,
    limits: SpiredCompactLiftLimits,
    materializer: &impl TargetRuleMaterializer,
) -> Result<SpiredCompactLift, SpiredCompactLiftError> {
    let trace = candidate.dependency_trace();
    if trace.nodes().is_empty() || trace.root() + 1 != trace.nodes().len() {
        return Err(SpiredCompactLiftError::Invariant {
            detail: "a post-hit dependency trace is empty or its root is not last",
        });
    }
    // As above, fail before cloning requests or constructing an exact epoch.
    if let Some((requested_rows, limit)) =
        exact_support_budget_exceeded(trace.nodes().len(), limits)
    {
        return Ok(SpiredCompactLift::ExactSupportBudgetExceeded {
            probe: probe.clone(),
            requested_rows,
            limit,
        });
    }
    let mut dependency_order = Vec::new();
    dependency_order
        .try_reserve_exact(trace.nodes().len())
        .map_err(|_| SpiredCompactLiftError::AllocationFailure {
            resource: "rooted compact dependency order",
            requested: trace.nodes().len(),
        })?;
    dependency_order.extend(trace.nodes().iter().map(|node| node.source().clone()));
    let requests = AccumulatedSourceRequests::try_new(
        case.arity(),
        dependency_order.iter().cloned(),
        limits.campaign,
    )?;
    let anchor =
        CampaignStratumAnchor::try_restricted(case.stratum().clone(), limits.campaign.stratum)?;
    let mut epochs = GrowingTaskEpochState::new(
        case.target_shift().clone(),
        anchor,
        case.owner_snapshot().clone(),
        case.ordering(),
    );
    let epoch = Arc::new(epochs.try_next_with_request_order(
        generator,
        completed,
        requests,
        &dependency_order,
        limits.campaign,
    )?);
    debug_assert_eq!(epoch.telemetry().epoch_ordinal(), 0);

    if epoch.plan().source_instances().len() != dependency_order.len()
        || epoch
            .plan()
            .source_instances()
            .iter()
            .zip(&dependency_order)
            .any(|(instance, request)| {
                instance.provenance().source_ordinal() != request.source_ordinal()
                    || instance.provenance().offset() != request.offset()
            })
    {
        return Err(SpiredCompactLiftError::Invariant {
            detail: "fresh rooted frame did not preserve dependency-trace chronology",
        });
    }

    enum FreshDisposition {
        Exact(RootedExactCircuitLift),
        DidNotHit(crate::foundry::completion::frame::modular::ModularRankDiagnostics),
    }
    let disposition = {
        let query = epoch.try_query(generator.context(), probe, limits.campaign)?;
        match query.query() {
            ModularTargetQuery::Hit(hit) => {
                let root_frame_row = trace.root();
                let mut predecessor_rows = Vec::new();
                predecessor_rows
                    .try_reserve_exact(root_frame_row)
                    .map_err(|_| SpiredCompactLiftError::AllocationFailure {
                        resource: "rooted compact predecessor rows",
                        requested: root_frame_row,
                    })?;
                predecessor_rows.extend(0..root_frame_row);
                FreshDisposition::Exact(materializer.try_materialize_rooted(
                    generator.context(),
                    hit,
                    query.partition(),
                    &predecessor_rows,
                    root_frame_row,
                    limits.exact,
                )?)
            }
            ModularTargetQuery::NoHitWithObstruction(_) => {
                FreshDisposition::DidNotHit(query.query().diagnostics().clone())
            }
        }
    };

    Ok(match disposition {
        FreshDisposition::Exact(RootedExactCircuitLift::Replayed(circuit)) => {
            SpiredCompactLift::Replayed(SpiredReplayedCompactLift::new(
                epoch,
                circuit,
                probe.clone(),
            ))
        }
        FreshDisposition::Exact(RootedExactCircuitLift::Inconclusive(miss)) => {
            SpiredCompactLift::RootedExactDidNotLift {
                epoch,
                probe: probe.clone(),
                miss,
            }
        }
        FreshDisposition::DidNotHit(diagnostics) => SpiredCompactLift::FreshFrameDidNotHit {
            epoch,
            probe: probe.clone(),
            diagnostics,
        },
    })
}

fn exact_support_budget_exceeded(
    requested: usize,
    limits: SpiredCompactLiftLimits,
) -> Option<(usize, usize)> {
    let limit = limits.exact.max_selected_rows;
    if requested > limit {
        Some((requested, limit))
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn try_lift_with_materializer(
    case: &SpiredExecutionCase,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    streaming_hit: &SpiredModularHit,
    probe: &CampaignModularProbe,
    limits: SpiredCompactLiftLimits,
    materializer: &impl TargetRuleMaterializer,
) -> Result<SpiredCompactLift, SpiredCompactLiftError> {
    let requests = AccumulatedSourceRequests::try_new(
        case.arity(),
        streaming_hit.support().iter().cloned(),
        limits.campaign,
    )?;
    let anchor =
        CampaignStratumAnchor::try_restricted(case.stratum().clone(), limits.campaign.stratum)?;
    let mut epochs = GrowingTaskEpochState::new(
        case.target_shift().clone(),
        anchor,
        case.owner_snapshot().clone(),
        case.ordering(),
    );
    let epoch = Arc::new(epochs.try_next_with_request_order(
        generator,
        completed,
        requests,
        streaming_hit.dependency_order(),
        limits.campaign,
    )?);
    debug_assert_eq!(epoch.telemetry().epoch_ordinal(), 0);

    enum FreshDisposition {
        Exact(ExactCircuitLift),
        DidNotHit(crate::foundry::completion::frame::modular::ModularRankDiagnostics),
    }

    let disposition = {
        let query = epoch.try_query(generator.context(), probe, limits.campaign)?;
        match query.query() {
            ModularTargetQuery::Hit(hit) => FreshDisposition::Exact(materializer.try_materialize(
                generator.context(),
                hit,
                query.partition(),
                limits.exact,
            )?),
            ModularTargetQuery::NoHitWithObstruction(_) => {
                FreshDisposition::DidNotHit(query.query().diagnostics().clone())
            }
        }
    };

    Ok(match disposition {
        FreshDisposition::Exact(ExactCircuitLift::Replayed(circuit)) => {
            SpiredCompactLift::Replayed(SpiredReplayedCompactLift::new(
                epoch,
                circuit,
                probe.clone(),
            ))
        }
        FreshDisposition::Exact(ExactCircuitLift::ModularSupportDidNotLift(miss)) => {
            SpiredCompactLift::ExactSupportDidNotLift {
                epoch,
                probe: probe.clone(),
                miss,
            }
        }
        FreshDisposition::DidNotHit(diagnostics) => SpiredCompactLift::FreshFrameDidNotHit {
            epoch,
            probe: probe.clone(),
            diagnostics,
        },
    })
}
