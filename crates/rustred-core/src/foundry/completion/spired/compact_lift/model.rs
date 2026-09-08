use std::sync::Arc;

use crate::foundry::completion::frame::exact::{ExactCircuitSupportDidNotLift, ExactTargetCircuit};
use crate::foundry::completion::frame::modular::ModularRankDiagnostics;
use crate::foundry::completion::source_discovery::{CampaignModularProbe, FreshTaskEpoch};

/// Replayed compact support sealed to the exact epoch and raw modular probe
/// that produced it.
///
/// Construction stays inside this module so later authority admission cannot
/// accidentally substitute a different anchor point or detach the circuit
/// from its live physical plan.
#[derive(Debug)]
pub(crate) struct SpiredReplayedCompactLift {
    epoch: Arc<FreshTaskEpoch>,
    circuit: Arc<ExactTargetCircuit>,
    probe: CampaignModularProbe,
}

impl SpiredReplayedCompactLift {
    pub(crate) const fn epoch(&self) -> &Arc<FreshTaskEpoch> {
        &self.epoch
    }

    pub(crate) const fn circuit(&self) -> &Arc<ExactTargetCircuit> {
        &self.circuit
    }

    pub(crate) const fn probe(&self) -> &CampaignModularProbe {
        &self.probe
    }

    pub(super) fn new(
        epoch: Arc<FreshTaskEpoch>,
        circuit: ExactTargetCircuit,
        probe: CampaignModularProbe,
    ) -> Self {
        Self {
            epoch,
            circuit: Arc::new(circuit),
            probe,
        }
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        Arc<FreshTaskEpoch>,
        Arc<ExactTargetCircuit>,
        CampaignModularProbe,
    ) {
        (self.epoch, self.circuit, self.probe)
    }
}

/// Outcome of independently rebuilding and lifting a streaming support.
///
/// Every variant retains the fresh epoch. In the replayed case this keeps the
/// exact circuit's physical-plan admission identity alive. Neither
/// inconclusive variant is evidence that the complete translated module has
/// no target relation.
#[derive(Debug)]
pub(crate) enum SpiredCompactLift {
    Replayed(SpiredReplayedCompactLift),
    FreshFrameDidNotHit {
        epoch: Arc<FreshTaskEpoch>,
        probe: CampaignModularProbe,
        diagnostics: ModularRankDiagnostics,
    },
    ExactSupportDidNotLift {
        epoch: Arc<FreshTaskEpoch>,
        probe: CampaignModularProbe,
        miss: ExactCircuitSupportDidNotLift,
    },
}

impl SpiredCompactLift {
    pub(crate) fn epoch(&self) -> &Arc<FreshTaskEpoch> {
        match self {
            Self::Replayed(replayed) => replayed.epoch(),
            Self::FreshFrameDidNotHit { epoch, .. }
            | Self::ExactSupportDidNotLift { epoch, .. } => epoch,
        }
    }

    pub(crate) fn circuit(&self) -> Option<&ExactTargetCircuit> {
        match self {
            Self::Replayed(replayed) => Some(replayed.circuit().as_ref()),
            Self::FreshFrameDidNotHit { .. } | Self::ExactSupportDidNotLift { .. } => None,
        }
    }

    pub(crate) const fn probe(&self) -> &CampaignModularProbe {
        match self {
            Self::Replayed(replayed) => replayed.probe(),
            Self::FreshFrameDidNotHit { probe, .. }
            | Self::ExactSupportDidNotLift { probe, .. } => probe,
        }
    }
}
