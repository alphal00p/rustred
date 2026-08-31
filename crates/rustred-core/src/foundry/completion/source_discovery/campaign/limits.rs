use crate::foundry::completion::frame::PhysicalFrameLimits;
use crate::foundry::completion::frame::modular::ModularKernelLimits;
use crate::foundry::completion::stratum::StratumRegistryLimits;
use crate::identity::TranslatedSourceLimits;

/// Aggregate resource policy for one immutable selected-source campaign epoch.
///
/// The request caps cover Rust-owned scheduling metadata. Exact translation,
/// physical assembly, stratum classification, and modular reduction keep
/// their existing independent policies so a budget failure retains its
/// precise layer in [`super::CampaignBudgetExhaustion`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CampaignLimits {
    pub(crate) max_request_arity: usize,
    pub(crate) max_submitted_requests: usize,
    pub(crate) max_canonical_candidate_requests: usize,
    pub(crate) max_accumulated_requests: usize,
    pub(crate) max_request_coordinate_cells: usize,
    pub(crate) max_merge_comparisons: usize,
    pub(crate) max_retained_probe_coordinates: usize,
    pub(crate) translated_sources: TranslatedSourceLimits,
    pub(crate) physical_frame: PhysicalFrameLimits,
    pub(crate) stratum: StratumRegistryLimits,
    pub(crate) modular: ModularKernelLimits,
}

impl Default for CampaignLimits {
    fn default() -> Self {
        Self {
            max_request_arity: 4_096,
            max_submitted_requests: 16_000_000,
            max_canonical_candidate_requests: 16_000_000,
            max_accumulated_requests: 1_000_000,
            max_request_coordinate_cells: 64_000_000,
            max_merge_comparisons: 32_000_000,
            max_retained_probe_coordinates: 8_192,
            translated_sources: TranslatedSourceLimits::default(),
            physical_frame: PhysicalFrameLimits::default(),
            stratum: StratumRegistryLimits::default(),
            modular: ModularKernelLimits::default(),
        }
    }
}
