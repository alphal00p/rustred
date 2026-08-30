use std::collections::{BTreeMap, btree_map::Entry};

use crate::algebra::{
    Coefficient, IndexedAlgebraError, IndexedCoefficientContext,
    coefficient_clone_owned_retained_byte_bound,
};
use crate::family::IntegralKey;
use crate::identity::{IndexShift, ParametricRelation};

use super::error::ParametricRuleError;
use super::limits::ParametricRuleLimits;
#[cfg(test)]
use super::model::ParametricRule;
use super::model::{
    ConcreteSpecializationReplayWitness, ParametricNonZeroGuard, ParametricRuleTerm,
    ParametricSourceRowContribution,
};
use super::prepare::{check_cell_limit, check_limit, checked_add, try_vec};

/// Crate-internal held-out replay seam for artifact discovery and tests.
///
/// Derivation invokes the same implementation at its declared anchor. This
/// wrapper lets a foundry owner probe additional concrete points without
/// introducing a second elimination or mutating the retained rule witness.
/// It is a derivation/test boundary, never a reducer-hot-path operation.
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn replay_rule_at_concrete_assignment(
    context: &IndexedCoefficientContext,
    relations: &[ParametricRelation],
    rule: &ParametricRule,
    assignment: &[i64],
    limits: ParametricRuleLimits,
) -> Result<ConcreteSpecializationReplayWitness, ParametricRuleError> {
    verify_concrete_specialization_replay(
        context,
        relations,
        assignment,
        rule.pivot(),
        rule.right_hand_side(),
        rule.nonzero_guards(),
        rule.source_combination(),
        limits,
    )
}

/// Replay the retained indexed source combination directly in the exact base
/// field at one concrete anchor.
///
/// This compares the mathematical relation, not the normal form chosen by a
/// second elimination. Concrete boundary sectors can reorder columns after a
/// pinch, so equality with an independently reduced row would be stronger
/// than — and unrelated to — exact membership in the retained source span.
pub(super) fn verify_concrete_specialization_replay(
    context: &IndexedCoefficientContext,
    relations: &[ParametricRelation],
    anchor: &[i64],
    pivot: &IndexShift,
    right_hand_side: &[ParametricRuleTerm],
    guards: &[ParametricNonZeroGuard],
    source_combination: &[ParametricSourceRowContribution],
    limits: ParametricRuleLimits,
) -> Result<ConcreteSpecializationReplayWitness, ParametricRuleError> {
    let mut budget = ReplayBudget::new(limits.max_concrete_replay_exact_operations);

    // Guards are pre-cancellation domain facts. Check every retained guard
    // before specializing any coefficient which relies on it.
    for (guard_ordinal, guard) in guards.iter().enumerate() {
        budget.charge("concrete specialization replay exact operations")?;
        let specialized = context.specialize_polynomial_sealed(
            guard.polynomial(),
            anchor,
            limits.indexed_algebra,
        )?;
        if specialized.is_zero() {
            return Err(ParametricRuleError::GuardVanishedAtAnchor { guard_ordinal });
        }
    }

    let mut source_terms_checked = 0usize;
    for contribution in source_combination {
        let source = relations.get(contribution.source_ordinal()).ok_or(
            ParametricRuleError::ConcreteReplaySourceOrdinalOutOfRange {
                source_ordinal: contribution.source_ordinal(),
            },
        )?;
        if source.row_id() != contribution.row_id() {
            return Err(ParametricRuleError::ConcreteReplaySourceIdentityMismatch {
                source_ordinal: contribution.source_ordinal(),
            });
        }
        source_terms_checked = checked_add(
            "concrete specialization replay source terms",
            source_terms_checked,
            source.terms().len(),
        )?;
    }
    let integral_keys_checked = checked_add(
        "concrete specialization replay terms",
        checked_add(
            "concrete specialization replay terms",
            source_terms_checked,
            right_hand_side.len(),
        )?,
        1,
    )?;
    check_limit(
        "concrete specialization replay terms",
        integral_keys_checked,
        limits.max_concrete_replay_terms,
    )?;
    let replay_key_buffers = checked_add(
        "concrete specialization replay integral-key buffers",
        integral_keys_checked,
        1,
    )?;
    check_cell_limit(
        "concrete specialization replay integral-key power cells",
        replay_key_buffers,
        anchor.len(),
        limits.max_concrete_replay_integral_key_power_cells,
    )?;

    let mut accumulated = BTreeMap::<IntegralKey, AccumulatedCoefficient>::new();
    let mut retained = RetainedCoefficientCensus::new(
        limits.max_concrete_replay_retained_coefficient_terms,
        limits.max_concrete_replay_retained_coefficient_clone_owned_bytes,
    );
    for contribution in source_combination {
        let source = &relations[contribution.source_ordinal()];
        budget.charge("concrete specialization replay exact operations")?;
        let (source_weight, _) = context.specialize_sealed(
            contribution.coefficient(),
            anchor,
            limits.indexed_algebra,
        )?;
        for (shift, coefficient) in source.terms() {
            // Construct every referenced key, including terms whose weight or
            // coefficient specializes to zero. This keeps overflow handling
            // and the witness census independent of accidental zeros.
            let key = shifted_key(anchor, shift)?;
            budget.charge("concrete specialization replay exact operations")?;
            let (source_coefficient, _) =
                context.specialize_sealed(coefficient, anchor, limits.indexed_algebra)?;
            budget.charge("concrete specialization replay exact operations")?;
            let product = context
                .base()
                .try_mul(
                    &source_weight,
                    &source_coefficient,
                    limits.indexed_algebra.exact_algebra,
                )
                .map_err(IndexedAlgebraError::from)?;
            accumulate(
                context,
                &mut accumulated,
                key,
                product,
                &mut retained,
                &mut budget,
                limits,
            )?;
        }
    }

    let pivot_key = shifted_key(anchor, pivot)?;
    let pivot_coefficient =
        remove_accumulated(context, &mut accumulated, &pivot_key, &mut retained)?;
    budget.charge("concrete specialization replay exact operations")?;
    if pivot_coefficient != context.base().one() {
        return Err(ParametricRuleError::ConcreteReplayPivotMismatch);
    }

    for (right_hand_side_ordinal, term) in right_hand_side.iter().enumerate() {
        let key = shifted_key(anchor, term.shift())?;
        budget.charge("concrete specialization replay exact operations")?;
        let (coefficient, _) =
            context.specialize_sealed(term.coefficient(), anchor, limits.indexed_algebra)?;
        budget.charge("concrete specialization replay exact operations")?;
        let expected_relation_coefficient = context
            .base()
            .try_neg(&coefficient, limits.indexed_algebra.exact_algebra)
            .map_err(IndexedAlgebraError::from)?;
        let actual = remove_accumulated(context, &mut accumulated, &key, &mut retained)?;
        budget.charge("concrete specialization replay exact operations")?;
        if actual != expected_relation_coefficient {
            return Err(ParametricRuleError::ConcreteReplayRightHandSideMismatch {
                right_hand_side_ordinal,
            });
        }
    }
    if !accumulated.is_empty() {
        return Err(ParametricRuleError::ConcreteReplayUnexpectedIntegral);
    }
    retained.verify_empty()?;

    Ok(ConcreteSpecializationReplayWitness::new(
        IntegralKey::try_from_preallocated(copy_anchor(anchor)?)?,
        source_combination.len(),
        source_terms_checked,
        right_hand_side.len(),
        integral_keys_checked,
        guards.len(),
        budget.used,
        retained.peak_terms,
    ))
}

fn accumulate(
    context: &IndexedCoefficientContext,
    accumulated: &mut BTreeMap<IntegralKey, AccumulatedCoefficient>,
    key: IntegralKey,
    value: Coefficient,
    retained: &mut RetainedCoefficientCensus,
    budget: &mut ReplayBudget,
    limits: ParametricRuleLimits,
) -> Result<(), ParametricRuleError> {
    if value.is_zero() {
        return Ok(());
    }
    let retained_keys = accumulated.len();
    match accumulated.entry(key) {
        Entry::Occupied(mut entry) => {
            let old_weight = entry.get().weight;
            budget.charge("concrete specialization replay exact operations")?;
            let sum = context
                .base()
                .try_add(
                    &entry.get().coefficient,
                    &value,
                    limits.indexed_algebra.exact_algebra,
                )
                .map_err(IndexedAlgebraError::from)?;
            let new_weight = if sum.is_zero() {
                RetainedCoefficientWeight::default()
            } else {
                coefficient_weight(&sum)?
            };
            // Replacement and cancellation are one map transition: remove
            // the old retained payload before adding the normalized result.
            // The transient exact-operation operands are bounded separately
            // by `ExactAlgebraLimits` and are not part of the BTreeMap census.
            retained.replace(old_weight, new_weight)?;
            if sum.is_zero() {
                entry.remove();
            } else {
                *entry.get_mut() = AccumulatedCoefficient {
                    coefficient: sum,
                    weight: new_weight,
                };
            }
        }
        Entry::Vacant(entry) => {
            let requested = checked_add(
                "concrete specialization replay distinct integral keys",
                retained_keys,
                1,
            )?;
            check_limit(
                "concrete specialization replay distinct integral keys",
                requested,
                limits.max_concrete_replay_integral_keys,
            )?;
            let weight = coefficient_weight(&value)?;
            retained.replace(RetainedCoefficientWeight::default(), weight)?;
            entry.insert(AccumulatedCoefficient {
                coefficient: value,
                weight,
            });
        }
    }
    Ok(())
}

fn remove_accumulated(
    context: &IndexedCoefficientContext,
    accumulated: &mut BTreeMap<IntegralKey, AccumulatedCoefficient>,
    key: &IntegralKey,
    retained: &mut RetainedCoefficientCensus,
) -> Result<Coefficient, ParametricRuleError> {
    let Some(value) = accumulated.remove(key) else {
        return Ok(context.base().zero());
    };
    retained.replace(value.weight, RetainedCoefficientWeight::default())?;
    Ok(value.coefficient)
}

struct AccumulatedCoefficient {
    coefficient: Coefficient,
    weight: RetainedCoefficientWeight,
}

#[derive(Clone, Copy, Default)]
struct RetainedCoefficientWeight {
    terms: usize,
    clone_owned_bytes: usize,
}

fn coefficient_weight(
    coefficient: &Coefficient,
) -> Result<RetainedCoefficientWeight, ParametricRuleError> {
    let terms = checked_add(
        "concrete specialization replay retained coefficient terms",
        coefficient.numerator.nterms(),
        coefficient.denominator.nterms(),
    )?;
    let clone_owned_bytes = coefficient_clone_owned_retained_byte_bound(coefficient).ok_or(
        ParametricRuleError::ResourceCountOverflow {
            resource: "concrete specialization replay retained coefficient clone-owned bytes",
        },
    )?;
    Ok(RetainedCoefficientWeight {
        terms,
        clone_owned_bytes,
    })
}

struct RetainedCoefficientCensus {
    current_terms: usize,
    current_clone_owned_bytes: usize,
    peak_terms: usize,
    max_terms: usize,
    max_clone_owned_bytes: usize,
}

impl RetainedCoefficientCensus {
    fn new(max_terms: usize, max_clone_owned_bytes: usize) -> Self {
        Self {
            current_terms: 0,
            current_clone_owned_bytes: 0,
            peak_terms: 0,
            max_terms,
            max_clone_owned_bytes,
        }
    }

    fn replace(
        &mut self,
        old: RetainedCoefficientWeight,
        new: RetainedCoefficientWeight,
    ) -> Result<(), ParametricRuleError> {
        let terms = self.current_terms.checked_sub(old.terms).ok_or(
            ParametricRuleError::ReducerInvariant {
                detail: "concrete replay retained coefficient-term census underflowed",
            },
        )?;
        let terms = checked_add(
            "concrete specialization replay retained coefficient terms",
            terms,
            new.terms,
        )?;
        check_limit(
            "concrete specialization replay retained coefficient terms",
            terms,
            self.max_terms,
        )?;
        let clone_owned_bytes = self
            .current_clone_owned_bytes
            .checked_sub(old.clone_owned_bytes)
            .ok_or(ParametricRuleError::ReducerInvariant {
                detail: "concrete replay retained coefficient-byte census underflowed",
            })?;
        let clone_owned_bytes = checked_add(
            "concrete specialization replay retained coefficient clone-owned bytes",
            clone_owned_bytes,
            new.clone_owned_bytes,
        )?;
        check_limit(
            "concrete specialization replay retained coefficient clone-owned bytes",
            clone_owned_bytes,
            self.max_clone_owned_bytes,
        )?;
        self.current_terms = terms;
        self.current_clone_owned_bytes = clone_owned_bytes;
        self.peak_terms = self.peak_terms.max(terms);
        Ok(())
    }

    fn verify_empty(&self) -> Result<(), ParametricRuleError> {
        if self.current_terms != 0 {
            return Err(ParametricRuleError::ReducerInvariant {
                detail: "empty concrete replay map retained a nonzero coefficient-term census",
            });
        }
        if self.current_clone_owned_bytes != 0 {
            return Err(ParametricRuleError::ReducerInvariant {
                detail: "empty concrete replay map retained a nonzero coefficient-byte census",
            });
        }
        Ok(())
    }
}

fn shifted_key(anchor: &[i64], shift: &IndexShift) -> Result<IntegralKey, ParametricRuleError> {
    let mut powers = try_vec("concrete specialization replay integral key", anchor.len())?;
    for (position, (&base, &offset)) in anchor.iter().zip(shift.values()).enumerate() {
        powers.push(
            base.checked_add(offset)
                .ok_or(ParametricRuleError::AnchorIndexOverflow { position })?,
        );
    }
    Ok(IntegralKey::try_from_preallocated(powers)?)
}

fn copy_anchor(anchor: &[i64]) -> Result<Vec<i64>, ParametricRuleError> {
    let mut copy = try_vec("concrete specialization replay anchor", anchor.len())?;
    copy.extend_from_slice(anchor);
    Ok(copy)
}

struct ReplayBudget {
    used: usize,
    limit: usize,
}

impl ReplayBudget {
    fn new(limit: usize) -> Self {
        Self { used: 0, limit }
    }

    fn charge(&mut self, resource: &'static str) -> Result<(), ParametricRuleError> {
        self.used = checked_add(resource, self.used, 1)?;
        check_limit(resource, self.used, self.limit)
    }
}
