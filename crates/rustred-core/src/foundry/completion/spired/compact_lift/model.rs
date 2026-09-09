use std::sync::Arc;

use crate::foundry::completion::frame::exact::{
    ExactCircuitSupportDidNotLift, ExactTargetCircuit, RootedExactCircuitMiss,
};
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
/// Variants reached after exact-frame construction retain the fresh epoch. A
/// support-budget rejection is deliberately returned before an epoch or any
/// translated exact source is built. No inconclusive variant is evidence that
/// the complete translated module has no target relation.
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
    /// The modular predecessor trace is larger than the configured exact
    /// materialization budget.
    ///
    /// This is a candidate-local, fail-closed rejection rather than an
    /// infrastructure error: target search may compare a later compact trace
    /// or explore a canonical source-exclusion branch. It is emitted before
    /// any exact epoch or translated polynomial source is constructed.
    ExactSupportBudgetExceeded {
        probe: CampaignModularProbe,
        requested_rows: usize,
        limit: usize,
    },
    /// The designated later row did not produce an exact target relation over
    /// the compact predecessor trace. This rejects only that bounded rooted
    /// proposal; it is not evidence that the case has no relation.
    RootedExactDidNotLift {
        epoch: Arc<FreshTaskEpoch>,
        probe: CampaignModularProbe,
        miss: RootedExactCircuitMiss,
    },
}

impl SpiredCompactLift {
    pub(crate) fn epoch(&self) -> Option<&Arc<FreshTaskEpoch>> {
        match self {
            Self::Replayed(replayed) => Some(replayed.epoch()),
            Self::FreshFrameDidNotHit { epoch, .. }
            | Self::ExactSupportDidNotLift { epoch, .. }
            | Self::RootedExactDidNotLift { epoch, .. } => Some(epoch),
            Self::ExactSupportBudgetExceeded { .. } => None,
        }
    }

    pub(crate) fn circuit(&self) -> Option<&ExactTargetCircuit> {
        match self {
            Self::Replayed(replayed) => Some(replayed.circuit().as_ref()),
            Self::FreshFrameDidNotHit { .. }
            | Self::ExactSupportDidNotLift { .. }
            | Self::ExactSupportBudgetExceeded { .. }
            | Self::RootedExactDidNotLift { .. } => None,
        }
    }

    pub(crate) const fn probe(&self) -> &CampaignModularProbe {
        match self {
            Self::Replayed(replayed) => replayed.probe(),
            Self::FreshFrameDidNotHit { probe, .. }
            | Self::ExactSupportDidNotLift { probe, .. }
            | Self::ExactSupportBudgetExceeded { probe, .. }
            | Self::RootedExactDidNotLift { probe, .. } => probe,
        }
    }
}
