use crate::algebra::{Coefficient, IndexedCoefficientContext};
use crate::family::IntegralKey;
use crate::foundry::anchored::{
    derive_strictly_descending_rule, derive_strictly_descending_rule_for_target,
};
use crate::identity::{IndexShift, ParametricRelation};
use crate::sector::OrderingPolicy;

use super::error::ParametricRuleError;
use super::limits::ParametricRuleLimits;
use super::model::{
    AnchorAgreement, ParametricNonZeroGuard, ParametricRuleTerm, ParametricSourceRowContribution,
};
use super::prepare::{check_cell_limit, checked_add, try_vec};

#[derive(Clone, Copy)]
pub(super) enum AnchorSelection {
    FirstDescending,
    Targeted,
}

pub(super) fn verify_anchor_agreement(
    context: &IndexedCoefficientContext,
    relations: &[ParametricRelation],
    anchor: &[i64],
    ordering: OrderingPolicy,
    pivot: &IndexShift,
    right_hand_side: &[ParametricRuleTerm],
    guards: &[ParametricNonZeroGuard],
    source_combination: &[ParametricSourceRowContribution],
    limits: ParametricRuleLimits,
    selection: AnchorSelection,
) -> Result<AnchorAgreement, ParametricRuleError> {
    for (guard_ordinal, guard) in guards.iter().enumerate() {
        let specialized =
            context.specialize_polynomial(guard.polynomial(), anchor, limits.indexed_algebra)?;
        if specialized.is_zero() {
            return Err(ParametricRuleError::GuardVanishedAtAnchor { guard_ordinal });
        }
    }

    let parametric_pivot = shifted_key(anchor, pivot)?;
    let anchored = match selection {
        AnchorSelection::FirstDescending => {
            derive_strictly_descending_rule(context, relations, anchor, ordering, limits.anchored)?
        }
        AnchorSelection::Targeted => derive_strictly_descending_rule_for_target(
            context,
            relations,
            anchor,
            parametric_pivot.powers(),
            ordering,
            limits.anchored,
        )?,
    };
    if anchored.pivot() != &parametric_pivot {
        return Err(ParametricRuleError::AnchorPivotMismatch);
    }
    // The temporary parametric pivot key has served its sole comparison
    // purpose. Drop it explicitly so the census below starts with exactly the
    // anchored rule's retained anchor and pivot keys.
    drop(parametric_pivot);

    let bridge_keys = checked_add(
        "parametric anchor-bridge integral keys",
        checked_add(
            "parametric anchor-bridge integral keys",
            2,
            anchored.right_hand_side().len(),
        )?,
        right_hand_side.len(),
    )?;
    check_cell_limit(
        "parametric anchor-bridge integral-key power cells",
        bridge_keys,
        anchor.len(),
        limits.max_anchor_bridge_integral_key_power_cells,
    )?;

    let mut specialized_rhs = try_vec(
        "specialized parametric right-hand side",
        right_hand_side.len(),
    )?;
    for term in right_hand_side {
        let (coefficient, _) =
            context.specialize(term.coefficient(), anchor, limits.indexed_algebra)?;
        if coefficient.is_zero() {
            continue;
        }
        specialized_rhs.push((shifted_key(anchor, term.shift())?, coefficient));
    }
    specialized_rhs.sort_unstable_by(|left, right| left.0.cmp(&right.0));

    let mut anchored_rhs = try_vec(
        "independently anchored right-hand side",
        anchored.right_hand_side().len(),
    )?;
    for term in anchored.right_hand_side() {
        anchored_rhs.push((term.integral(), term.coefficient()));
    }
    anchored_rhs.sort_unstable_by(|left, right| left.0.cmp(right.0));
    if specialized_rhs.len() != anchored_rhs.len()
        || specialized_rhs.iter().zip(&anchored_rhs).any(
            |((left_key, left_coefficient), (right_key, right_coefficient))| {
                left_key != *right_key || !coefficient_equal(left_coefficient, right_coefficient)
            },
        )
    {
        return Err(ParametricRuleError::AnchorRightHandSideMismatch);
    }

    let mut specialized_sources = try_vec(
        "specialized parametric source combination",
        source_combination.len(),
    )?;
    for parametric in source_combination {
        let (specialized, _) =
            context.specialize(parametric.coefficient(), anchor, limits.indexed_algebra)?;
        if specialized.is_zero() {
            continue;
        }
        specialized_sources.push((
            parametric.source_ordinal(),
            parametric.row_id(),
            specialized,
        ));
    }
    if specialized_sources.len() != anchored.source_combination().len() {
        return Err(ParametricRuleError::AnchorSourceCombinationMismatch);
    }
    for ((source_ordinal, row_id, specialized), concrete) in specialized_sources
        .iter()
        .zip(anchored.source_combination())
    {
        if *source_ordinal != concrete.source_ordinal() || *row_id != concrete.row_id() {
            return Err(ParametricRuleError::AnchorSourceCombinationMismatch);
        }
        if !coefficient_equal(specialized, concrete.coefficient()) {
            return Err(ParametricRuleError::AnchorSourceCombinationMismatch);
        }
    }

    Ok(AnchorAgreement::new(
        anchored,
        specialized_rhs.len(),
        specialized_sources.len(),
        guards.len(),
    ))
}

fn shifted_key(anchor: &[i64], shift: &IndexShift) -> Result<IntegralKey, ParametricRuleError> {
    let mut powers = try_vec("parametric anchor specialization key", anchor.len())?;
    for (position, (&base, &offset)) in anchor.iter().zip(shift.values()).enumerate() {
        powers.push(
            base.checked_add(offset)
                .ok_or(ParametricRuleError::AnchorIndexOverflow { position })?,
        );
    }
    Ok(IntegralKey::try_from_preallocated(powers)?)
}

fn coefficient_equal(left: &Coefficient, right: &Coefficient) -> bool {
    left == right
}
