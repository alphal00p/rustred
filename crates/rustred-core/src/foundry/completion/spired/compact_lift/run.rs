use std::sync::Arc;

use crate::foundry::completion::frame::exact::ExactCircuitLift;
use crate::foundry::completion::frame::modular::ModularTargetQuery;
use crate::foundry::completion::source_discovery::{
    AccumulatedSourceRequests, CampaignModularProbe, FreshTaskEpoch, GrowingTaskEpochState,
};
use crate::foundry::completion::stratum::CampaignStratumAnchor;
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};

use super::super::{
    PrunedExactMaterializer, SpiredExecutionCase, SpiredModularHit, TargetRuleMaterializer,
};
use super::{
    SpiredCompactLift, SpiredCompactLiftError, SpiredCompactLiftLimits, SpiredReplayedCompactLift,
};

/// Rebuild and exactly replay only the translated-source support nominated by
/// one streaming modular hit.
///
/// The streaming hit contributes no row ordinal, finite-field coefficient, or
/// frame authority here. Its support is canonicalized into a new immutable
/// request set. A fresh selected-source frame then repeats the modular query
/// from the original integer probe before the existing exact materializer may
/// run. There is deliberately no complete-frame fallback in this boundary.
pub(crate) fn try_lift_spired_compact_support(
    case: &SpiredExecutionCase,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    streaming_hit: &SpiredModularHit,
    probe: &CampaignModularProbe,
    limits: SpiredCompactLiftLimits,
) -> Result<SpiredCompactLift, SpiredCompactLiftError> {
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
    let epoch = Arc::new(epochs.try_next(generator, completed, requests, limits.campaign)?);
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
