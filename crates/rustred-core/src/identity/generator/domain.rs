use crate::family::CoefficientLocation;

use super::super::condition::{IdentityConditionSource, ParametricNonZeroCondition};
use super::super::relation::Builder;
use super::super::row::RowId;
use super::error::ParametricIbpError;
use super::model::ParametricIbpGenerator;

impl ParametricIbpGenerator<'_> {
    pub(super) fn empty_relation(&self, row_id: RowId) -> Result<Builder, ParametricIbpError> {
        let mut relation = Builder::new(
            self.source_scope.family_fingerprint.clone(),
            row_id,
            &self.context,
        );
        // Preserve the complete family domain before any fraction-field
        // cancellation. Tautological nonzero constants are intentionally
        // omitted by ParametricRelation.
        for condition in self.family.domain().conditions() {
            let lifted = self.context.lift_base_polynomial(condition.polynomial())?;
            let sources = condition.sources().iter().cloned().map(|location| {
                if location == CoefficientLocation::BasisDeterminantNumerator {
                    IdentityConditionSource::FamilyBasisDeterminantNumerator
                } else {
                    IdentityConditionSource::FamilyInputCoefficientDenominator { location }
                }
            });
            let lifted = ParametricNonZeroCondition::try_new_with_limits(
                &self.context,
                lifted,
                sources,
                self.config.relation_limits.arithmetic.exact_algebra,
                self.config.relation_limits.identity_conditions,
            )?;
            relation.add_nonzero_condition(&self.context, lifted, self.config.relation_limits)?;
        }
        Ok(relation)
    }
}
