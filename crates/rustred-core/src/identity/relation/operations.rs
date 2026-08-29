use crate::algebra::IndexedCoefficientContext;

use super::super::condition::{IdentityConditionError, IdentityConditionSource};
use super::super::row::RowId;
use super::builder::Builder;
use super::error::ParametricRelationError;
use super::index::IndexShift;
use super::limits::RelationLimits;
use super::model::ParametricRelation;

impl ParametricRelation {
    /// Re-admit a sealed relation under the caller's current relation policy,
    /// then copy it without exposing a public raw `Clone` boundary.
    ///
    /// This is the zero-translation path: each retained coefficient and
    /// condition polynomial is scanned exactly once to enforce a possibly
    /// tighter current algebra policy, while provenance cardinality is checked
    /// without replaying or rebuilding the relation.
    pub(in crate::identity) fn cloned_with_limits(
        &self,
        context: &IndexedCoefficientContext,
        limits: RelationLimits,
    ) -> Result<Self, ParametricRelationError> {
        self.validate_context(context)?;
        for coefficient in self.terms.values() {
            context.validate_with_limits(coefficient, limits.arithmetic.exact_algebra)?;
        }
        for condition in &self.nonzero_conditions {
            context.validate_polynomial_with_limits(
                condition.polynomial(),
                limits.arithmetic.exact_algebra,
            )?;
            let requested = condition.sources().len();
            if requested > limits.identity_conditions.max_sources {
                return Err(ParametricRelationError::IdentityCondition(
                    IdentityConditionError::ResourceLimit {
                        resource: "identity condition sources",
                        requested,
                        limit: limits.identity_conditions.max_sources,
                    },
                ));
            }
        }
        Ok(self.clone_sealed())
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
            result.add_sealed_nonzero_condition(context, translated, limits)?;
        }
        for (shift, coefficient) in &self.terms {
            let translated_shift = shift.checked_add(translation)?;
            let translated_coefficient =
                context.translate_sealed(coefficient, translation.values(), limits.arithmetic)?;
            result.add_sealed_term(context, translated_shift, translated_coefficient, limits)?;
        }
        Ok(result.finish())
    }
}
