//! Test-only construction of a coherent forged persistence witness.

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};

use super::{Builder, ParametricRelation, ParametricRelationError, RelationLimits};

impl ParametricRelation {
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
