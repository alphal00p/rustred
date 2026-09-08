use crate::foundry::completion::frame::exact::ExactCircuitLimits;
use crate::foundry::completion::source_discovery::CampaignLimits;

/// Independent resource policies for fresh-frame reconstruction and exact
/// support lift.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredCompactLiftLimits {
    pub(crate) campaign: CampaignLimits,
    pub(crate) exact: ExactCircuitLimits,
}

impl Default for SpiredCompactLiftLimits {
    fn default() -> Self {
        Self {
            campaign: CampaignLimits::default(),
            exact: ExactCircuitLimits::default(),
        }
    }
}
