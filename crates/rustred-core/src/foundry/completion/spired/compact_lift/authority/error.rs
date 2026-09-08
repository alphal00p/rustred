use std::fmt;

use crate::foundry::completion::source_discovery::{CampaignError, ExactRuleCellPromotionError};

/// Failure while deriving the retained probe anchor or admitting one exact
/// compact replay as an executable rule candidate.
#[derive(Debug)]
pub(crate) enum SpiredRuleCellAuthorityError {
    Anchor(CampaignError),
    Promotion(ExactRuleCellPromotionError),
}

impl fmt::Display for SpiredRuleCellAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Anchor(error) => write!(
                formatter,
                "SpIRed compact replay anchor reconstruction failed: {error}"
            ),
            Self::Promotion(error) => {
                write!(formatter, "SpIRed compact replay promotion failed: {error}")
            }
        }
    }
}

impl std::error::Error for SpiredRuleCellAuthorityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Anchor(error) => Some(error),
            Self::Promotion(error) => Some(error),
        }
    }
}
