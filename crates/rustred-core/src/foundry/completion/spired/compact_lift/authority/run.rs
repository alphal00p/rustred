use crate::algebra::IndexedCoefficientContext;
use crate::foundry::completion::source_discovery::{
    ExactRuleCellPromotionDisposition, ExactRuleCellPromotionLimits, try_promote_replayed_rule_cell,
};

use super::super::SpiredReplayedCompactLift;
use super::SpiredRuleCellAuthorityError;

/// Promote one exactly replayed compact support without publication.
///
/// The raw probe is sealed inside `replayed` by the fresh modular-query
/// boundary. Its exact index anchor is reconstructed against the retained
/// epoch stratum, then the established promotion pipeline rechecks physical
/// plan identity, replay authority, owner witnesses, and strict descent. The
/// supplied limits bound every clearing, guard, lowering, and `RuleCell`
/// operation; the anchor arity was already bounded when the epoch was built.
pub(crate) fn try_promote_spired_replayed_rule_cell(
    context: &IndexedCoefficientContext,
    replayed: SpiredReplayedCompactLift,
    limits: ExactRuleCellPromotionLimits,
) -> Result<ExactRuleCellPromotionDisposition, SpiredRuleCellAuthorityError> {
    let (epoch, circuit, probe) = replayed.into_parts();
    let anchor = epoch
        .try_anchor_for_probe(&probe)
        .map_err(SpiredRuleCellAuthorityError::Anchor)?;
    try_promote_replayed_rule_cell(context, epoch, circuit, &anchor, limits)
        .map_err(SpiredRuleCellAuthorityError::Promotion)
}
