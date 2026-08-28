use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};

use super::super::condition::{
    IdentityConditionSource, ParametricNonZeroCondition, insert_parametric_condition,
};
use super::super::row::RowId;
use super::error::ParametricRelationError;
use super::index::IndexShift;
use super::limits::RelationLimits;
use super::model::ParametricRelation;

impl ParametricRelation {
    pub(in crate::identity) fn add_nonzero_condition_with_limits(
        &mut self,
        context: &IndexedCoefficientContext,
        mut condition: ParametricNonZeroCondition,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        self.validate_context(context)?;
        context.validate_polynomial_with_limits(
            condition.polynomial(),
            limits.arithmetic.exact_algebra,
        )?;
        if condition.polynomial().is_zero() {
            return Err(ParametricRelationError::UnsatisfiableDomain);
        }
        if condition.polynomial().is_nonzero_constant() {
            return Ok(());
        }
        condition.add_source(
            IdentityConditionSource::RelationConditionAttached {
                row: self.row_id.clone(),
            },
            limits.identity_conditions,
        )?;
        insert_parametric_condition(
            &mut self.nonzero_conditions,
            condition,
            limits.identity_conditions,
        )?;
        Ok(())
    }

    pub(in crate::identity) fn add_term_with_limits(
        &mut self,
        context: &IndexedCoefficientContext,
        shift: IndexShift,
        coefficient: IndexedCoefficient,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        let mut staged = self.clone();
        staged.add_term_in_place(context, shift, coefficient, limits)?;
        *self = staged;
        Ok(())
    }

    /// Apply one term insertion to an isolated relation snapshot.
    ///
    /// The transactional entry point clones before calling this helper because the
    /// input-denominator condition is discovered before coefficient collection.
    /// A later exact-arithmetic failure must not leave that condition committed to
    /// an otherwise unchanged relation.
    fn add_term_in_place(
        &mut self,
        context: &IndexedCoefficientContext,
        shift: IndexShift,
        coefficient: IndexedCoefficient,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        self.validate_context(context)?;
        self.validate_shift(&shift)?;
        context.validate_with_limits(&coefficient, limits.arithmetic.exact_algebra)?;

        // Inspect the incoming fraction before testing whether its numerator
        // is zero. This preserves a deliberately unnormalized `0 / p` as a
        // domain-bearing zero term.
        let denominator = context
            .denominator_condition_with_limits(&coefficient, limits.arithmetic.exact_algebra)?;
        let condition = ParametricNonZeroCondition::try_new_with_limits(
            context,
            denominator,
            [IdentityConditionSource::RelationInputTermDenominator {
                row: self.row_id.clone(),
                shift: shift.values().to_vec().into_boxed_slice(),
            }],
            limits.arithmetic.exact_algebra,
            limits.identity_conditions,
        )?;
        self.add_nonzero_condition_with_limits(context, condition, limits)?;
        if coefficient.is_zero() {
            return Ok(());
        }
        if let Some(current) = self.terms.get(&shift) {
            let sum =
                context.add_with_limits(current, &coefficient, limits.arithmetic.exact_algebra)?;
            if sum.is_zero() {
                self.terms.remove(&shift);
            } else {
                let denominator = context
                    .denominator_condition_with_limits(&sum, limits.arithmetic.exact_algebra)?;
                let condition = ParametricNonZeroCondition::try_new_with_limits(
                    context,
                    denominator,
                    [IdentityConditionSource::RelationCollectedTermDenominator {
                        row: self.row_id.clone(),
                        shift: shift.values().to_vec().into_boxed_slice(),
                    }],
                    limits.arithmetic.exact_algebra,
                    limits.identity_conditions,
                )?;
                self.add_nonzero_condition_with_limits(context, condition, limits)?;
                self.terms.insert(shift, sum);
            }
        } else {
            self.terms.insert(shift, coefficient);
        }
        Ok(())
    }

    pub(in crate::identity) fn add_scaled_with_limits(
        &mut self,
        context: &IndexedCoefficientContext,
        other: &Self,
        factor: &IndexedCoefficient,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        let mut staged = self.clone();
        staged.add_scaled_in_place(context, other, factor, limits)?;
        *self = staged;
        Ok(())
    }

    /// Apply a scaled addition to an isolated relation snapshot.
    fn add_scaled_in_place(
        &mut self,
        context: &IndexedCoefficientContext,
        other: &Self,
        factor: &IndexedCoefficient,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        self.validate_compatible(other, context)?;
        context.validate_with_limits(factor, limits.arithmetic.exact_algebra)?;
        for condition in &other.nonzero_conditions {
            self.add_nonzero_condition_with_limits(context, condition.clone(), limits)?;
        }
        let factor_denominator =
            context.denominator_condition_with_limits(factor, limits.arithmetic.exact_algebra)?;
        let factor_condition = ParametricNonZeroCondition::try_new_with_limits(
            context,
            factor_denominator,
            [IdentityConditionSource::RelationScaleFactorDenominator {
                target_row: self.row_id.clone(),
                source_row: other.row_id.clone(),
            }],
            limits.arithmetic.exact_algebra,
            limits.identity_conditions,
        )?;
        self.add_nonzero_condition_with_limits(context, factor_condition, limits)?;
        for (shift, coefficient) in &other.terms {
            let scaled =
                context.mul_with_limits(coefficient, factor, limits.arithmetic.exact_algebra)?;
            self.add_term_in_place(context, shift.clone(), scaled, limits)?;
        }
        Ok(())
    }

    pub(in crate::identity) fn translated(
        &self,
        context: &IndexedCoefficientContext,
        translation: &IndexShift,
        row_id: RowId,
        limits: RelationLimits,
    ) -> Result<Self, ParametricRelationError> {
        self.validate_context(context)?;
        self.validate_shift(translation)?;
        let target_row = row_id.clone();
        let source_row = self.row_id.clone();
        let mut result = Self::new(self.family_fingerprint.clone(), row_id, context);
        for condition in &self.nonzero_conditions {
            let mut translated = condition.translated(
                context,
                translation.values(),
                limits.arithmetic,
                limits.identity_conditions,
            )?;
            translated.add_source(
                IdentityConditionSource::RelationTranslation {
                    source_row: source_row.clone(),
                    target_row: target_row.clone(),
                    offset: translation.values().to_vec().into_boxed_slice(),
                },
                limits.identity_conditions,
            )?;
            result.add_nonzero_condition_with_limits(context, translated, limits)?;
        }
        for (shift, coefficient) in &self.terms {
            let translated_shift = shift.checked_add(translation)?;
            let translated_coefficient =
                context.translate(coefficient, translation.values(), limits.arithmetic)?;
            // `result` is an isolated, not-yet-published relation. Use the
            // transactional helper directly so translating many terms does
            // not deep-clone every previously retained condition and source on
            // each insertion; any error still drops the complete local row.
            result.add_term_in_place(context, translated_shift, translated_coefficient, limits)?;
        }
        Ok(result)
    }
}
