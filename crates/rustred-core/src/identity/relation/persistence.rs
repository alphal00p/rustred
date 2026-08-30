//! Test-only construction of a coherent forged persistence witness.

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};

#[cfg(test)]
use crate::identity::{IndexShift, RowId};

use super::{Builder, ParametricRelation, ParametricRelationError, RelationLimits};

impl ParametricRelation {
    /// Assemble a small exact relation for cross-boundary foundry tests.
    #[cfg(test)]
    pub(crate) fn from_terms_for_foundry_test(
        family_fingerprint: &str,
        row_id: RowId,
        context: &IndexedCoefficientContext,
        terms: impl IntoIterator<Item = (IndexShift, IndexedCoefficient)>,
    ) -> Result<Self, ParametricRelationError> {
        let mut builder = Builder::new(
            std::sync::Arc::new(family_fingerprint.to_owned()),
            row_id,
            context,
        );
        for (shift, coefficient) in terms {
            builder.add_term(context, shift, coefficient, RelationLimits::default())?;
        }
        Ok(builder.finish())
    }

    /// Produce a mathematically equivalent but semantically different source
    /// row for the artifact persistence forgery test. Production persistence
    /// never accepts relation bytes as an authority: it regenerates the tagged
    /// source plan instead.
    pub(crate) fn scaled_for_artifact_forgery_test(
        &self,
        context: &IndexedCoefficientContext,
        factor: &IndexedCoefficient,
    ) -> Result<Self, ParametricRelationError> {
        let mut builder = Builder::new(
            self.family_fingerprint_owner(),
            self.row_id().clone(),
            context,
        );
        builder.add_scaled(context, self, factor, RelationLimits::default())?;
        Ok(builder.finish())
    }
}
