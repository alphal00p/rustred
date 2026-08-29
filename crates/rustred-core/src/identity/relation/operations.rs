use crate::algebra::IndexedCoefficientContext;

use super::super::condition::IdentityConditionSource;
use super::super::row::RowId;
use super::builder::Builder;
use super::error::ParametricRelationError;
use super::index::IndexShift;
use super::limits::RelationLimits;
use super::model::ParametricRelation;

impl ParametricRelation {
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
        let mut result = Builder::new(self.family_fingerprint.clone(), row_id, context);
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
            result.add_nonzero_condition(context, translated, limits)?;
        }
        for (shift, coefficient) in &self.terms {
            let translated_shift = shift.checked_add(translation)?;
            let translated_coefficient =
                context.translate(coefficient, translation.values(), limits.arithmetic)?;
            result.add_term(context, translated_shift, translated_coefficient, limits)?;
        }
        Ok(result.finish())
    }
}
