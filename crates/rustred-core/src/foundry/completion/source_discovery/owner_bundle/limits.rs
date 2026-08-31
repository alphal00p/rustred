use crate::foundry::completion::frame::admission::{
    ExactCircuitOwnerCoverLimits, ExactCircuitSemanticLimits,
};

use super::super::ExactRuleCellPromotionLimits;

/// Aggregate cold-path limits for pairing exact semantic owners with their
/// executable rule cells and rebuilding a complete owner cover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactExecutableOwnerLimits {
    pub(crate) max_candidates_per_owner: usize,
    pub(crate) max_owner_content_order_bytes: usize,
    pub(crate) max_promotion_attempts: usize,
    pub(crate) max_retry_supports: usize,
    pub(crate) max_retry_anchor_coordinate_cells: usize,
    pub(crate) max_owners: usize,
    pub(crate) max_pairing_probes: usize,
    pub(crate) promotion: ExactRuleCellPromotionLimits,
    pub(crate) semantic: ExactCircuitSemanticLimits,
    pub(crate) cover: ExactCircuitOwnerCoverLimits,
}

impl Default for ExactExecutableOwnerLimits {
    fn default() -> Self {
        Self {
            max_candidates_per_owner: 4_096,
            max_owner_content_order_bytes: 67_108_864,
            max_promotion_attempts: 65_536,
            max_retry_supports: 4_096,
            max_retry_anchor_coordinate_cells: 4_194_304,
            max_owners: 4_096,
            max_pairing_probes: 16_777_216,
            promotion: ExactRuleCellPromotionLimits::default(),
            semantic: ExactCircuitSemanticLimits::default(),
            cover: ExactCircuitOwnerCoverLimits::default(),
        }
    }
}
