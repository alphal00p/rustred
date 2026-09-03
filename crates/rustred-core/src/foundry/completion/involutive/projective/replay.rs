use crate::algebra::IndexedCoefficientContext;
use crate::sector::ShiftComplexityKey;

use super::super::{ForwardShift, OreOrderingAdapter};
use super::error::ProjectiveError;
use super::limits::{
    ProjectiveLimits, ProjectiveNormalizationPolicy, ProjectiveWorkBudget, ProjectiveWorkCensus,
};
use super::model::{PrimitiveOreConsequence, ValidatedProjectiveConsequence};

/// Transactional ordering witness for a proposal-only projective replay.
///
/// Janet normal form reduces the greatest *reducible* term, which need not be
/// the physical row leader.  The cursor therefore constrains only successive
/// scheduler selections: after one successful step, the next selected target
/// must be strictly lower in the exact frozen Ore order.  Failed attempts
/// never mutate the consequence or selection witness, although attempted
/// polynomial work remains charged in the caller-owned budget.
pub(super) struct ProjectiveReplayCursor<'budget> {
    consequence: PrimitiveOreConsequence,
    previous_selection: Option<ShiftComplexityKey>,
    budget: &'budget mut ProjectiveWorkBudget,
}

impl<'budget> ProjectiveReplayCursor<'budget> {
    pub(super) fn try_new(
        consequence: PrimitiveOreConsequence,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        budget: &'budget mut ProjectiveWorkBudget,
        limits: ProjectiveLimits,
    ) -> Result<Self, ProjectiveError> {
        budget.require_limits(limits)?;
        consequence.try_validate(ordering, context, limits)?;
        Ok(Self {
            consequence,
            previous_selection: None,
            budget,
        })
    }

    pub(super) const fn consequence(&self) -> &PrimitiveOreConsequence {
        &self.consequence
    }

    pub(super) const fn work_census(&self) -> ProjectiveWorkCensus {
        self.budget.census()
    }

    pub(super) fn try_into_fully_normalized(
        self,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ProjectiveLimits,
    ) -> Result<PrimitiveOreConsequence, ProjectiveError> {
        self.consequence
            .try_full_normalize_for_admission(ordering, context, self.budget, limits)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn try_pseudo_reduce_next(
        &mut self,
        target: &ForwardShift,
        operator_shift: &ForwardShift,
        divisor: &ValidatedProjectiveConsequence<'_>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        normalization_policy: ProjectiveNormalizationPolicy,
        limits: ProjectiveLimits,
    ) -> Result<(), ProjectiveError> {
        ordering.require_action(&self.consequence.action)?;
        self.consequence.require_context(context)?;
        divisor.consequence().require_context(context)?;
        divisor.require_limits(limits)?;
        let target_key = ordering.try_key(target)?;
        if self
            .previous_selection
            .as_ref()
            .is_some_and(|previous| target_key >= *previous)
        {
            return Err(ProjectiveError::TargetExceedsPreviousSelection);
        }
        let next = self.consequence.try_pseudo_reduce_sealed(
            target,
            operator_shift,
            divisor.consequence(),
            ordering,
            context,
            normalization_policy,
            self.budget,
            limits,
        )?;
        self.consequence = next;
        self.previous_selection = Some(target_key);
        Ok(())
    }
}
