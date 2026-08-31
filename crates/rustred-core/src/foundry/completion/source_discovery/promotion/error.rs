use std::fmt;

use crate::foundry::cell::RuleCellError;
use crate::foundry::completion::frame::admission::ExactGuardRefinementError;
use crate::foundry::completion::frame::exact::ExactCircuitLoweringError;
use crate::foundry::completion::source_discovery::CampaignError;

/// Hard rejection or bounded cold-path failure during exact promotion.
#[derive(Debug)]
pub(crate) enum ExactRuleCellPromotionError {
    WrongContext,
    WrongPhysicalPlan,
    TargetMismatch,
    StratumMismatch,
    OwnerSnapshotMismatch,
    OrderingMismatch,
    ResidualJoin {
        ordinal: usize,
        detail: &'static str,
    },
    LoweredJoin {
        detail: &'static str,
    },
    Partition(CampaignError),
    GuardRefinement(ExactGuardRefinementError),
    Lowering(ExactCircuitLoweringError),
    Cell(RuleCellError),
}

impl fmt::Display for ExactRuleCellPromotionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongContext => formatter.write_str("promotion uses another indexed context"),
            Self::WrongPhysicalPlan => {
                formatter.write_str("exact circuit is not sealed to the retained epoch plan")
            }
            Self::TargetMismatch => {
                formatter.write_str("exact circuit and retained epoch select different targets")
            }
            Self::StratumMismatch => {
                formatter.write_str("exact circuit and retained epoch bind different strata")
            }
            Self::OwnerSnapshotMismatch => formatter
                .write_str("exact circuit and retained epoch bind different lower-owner snapshots"),
            Self::OrderingMismatch => {
                formatter.write_str("exact circuit descent uses another ordering policy")
            }
            Self::ResidualJoin { ordinal, detail } => {
                write!(
                    formatter,
                    "exact residual {ordinal} failed promotion join: {detail}"
                )
            }
            Self::LoweredJoin { detail } => {
                write!(
                    formatter,
                    "lowered exact rule failed promotion join: {detail}"
                )
            }
            Self::Partition(error) => write!(formatter, "epoch partition rebuild failed: {error}"),
            Self::GuardRefinement(error) => error.fmt(formatter),
            Self::Lowering(error) => error.fmt(formatter),
            Self::Cell(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ExactRuleCellPromotionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Partition(error) => Some(error),
            Self::GuardRefinement(error) => Some(error),
            Self::Lowering(error) => Some(error),
            Self::Cell(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CampaignError> for ExactRuleCellPromotionError {
    fn from(value: CampaignError) -> Self {
        Self::Partition(value)
    }
}

impl From<ExactGuardRefinementError> for ExactRuleCellPromotionError {
    fn from(value: ExactGuardRefinementError) -> Self {
        Self::GuardRefinement(value)
    }
}

impl From<ExactCircuitLoweringError> for ExactRuleCellPromotionError {
    fn from(value: ExactCircuitLoweringError) -> Self {
        Self::Lowering(value)
    }
}
