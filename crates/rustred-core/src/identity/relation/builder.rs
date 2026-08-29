use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};

use super::super::condition::{
    IdentityConditionSource, ParametricNonZeroCondition, insert_borrowed_parametric_condition,
    insert_parametric_condition,
};
use super::super::row::RowId;
use super::error::ParametricRelationError;
use super::index::IndexShift;
use super::limits::RelationLimits;
use super::model::ParametricRelation;

/// Unpublished owner used while one relation is assembled.
///
/// A partially mutated value cannot cross the identity boundary: callers
/// propagate every construction error and only [`Self::finish`] exposes the
/// completed relation. This avoids cloning the complete growing row before
/// each fallible insertion.
pub(in crate::identity) struct Builder {
    relation: ParametricRelation,
}

#[derive(Clone, Copy)]
enum CoefficientIngress {
    /// A caller-provided coefficient crosses the relation boundary and is
    /// authenticated exactly once under the current limits.
    Authenticate,
    /// A coefficient was produced by checked indexed arithmetic in this call
    /// or is already sealed inside a compatible relation.
    Sealed,
}

impl Builder {
    pub(in crate::identity) fn new(
        family_fingerprint: impl Into<std::sync::Arc<String>>,
        row_id: RowId,
        context: &IndexedCoefficientContext,
    ) -> Self {
        Self {
            relation: ParametricRelation::new(family_fingerprint, row_id, context),
        }
    }

    pub(in crate::identity) fn finish(self) -> ParametricRelation {
        self.relation
    }

    pub(in crate::identity) fn add_nonzero_condition(
        &mut self,
        context: &IndexedCoefficientContext,
        condition: ParametricNonZeroCondition,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        self.relation.validate_context(context)?;
        context.validate_polynomial_with_limits(
            condition.polynomial(),
            limits.arithmetic.exact_algebra,
        )?;
        self.attach_nonzero_condition(condition, limits)
    }

    fn add_authenticated_nonzero_condition(
        &mut self,
        context: &IndexedCoefficientContext,
        condition: ParametricNonZeroCondition,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        self.relation.validate_context(context)?;
        context.validate_polynomial_context(condition.polynomial())?;
        self.attach_nonzero_condition(condition, limits)
    }

    fn attach_nonzero_condition(
        &mut self,
        mut condition: ParametricNonZeroCondition,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        if condition.polynomial().is_zero() {
            return Err(ParametricRelationError::UnsatisfiableDomain);
        }
        if condition.polynomial().is_nonzero_constant() {
            return Ok(());
        }
        condition.add_source(
            IdentityConditionSource::RelationConditionAttached {
                row: self.relation.row_id.clone(),
            },
            limits.identity_conditions,
        )?;
        insert_parametric_condition(
            &mut self.relation.nonzero_conditions,
            condition,
            limits.identity_conditions,
        )?;
        Ok(())
    }

    /// Re-admit a borrowed condition under the current arithmetic policy, then
    /// preflight its complete target provenance footprint before copying it.
    /// Compatibility of both relations has already been checked by
    /// [`Self::add_scaled`].
    fn copy_nonzero_condition_with_readmission(
        &mut self,
        context: &IndexedCoefficientContext,
        condition: &ParametricNonZeroCondition,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
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
        insert_borrowed_parametric_condition(
            &mut self.relation.nonzero_conditions,
            condition,
            IdentityConditionSource::RelationConditionAttached {
                row: self.relation.row_id.clone(),
            },
            limits.identity_conditions,
        )?;
        Ok(())
    }

    pub(in crate::identity) fn add_term(
        &mut self,
        context: &IndexedCoefficientContext,
        shift: IndexShift,
        coefficient: IndexedCoefficient,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        self.add_term_with_ingress(
            context,
            shift,
            coefficient,
            limits,
            CoefficientIngress::Authenticate,
        )
    }

    fn add_term_with_ingress(
        &mut self,
        context: &IndexedCoefficientContext,
        shift: IndexShift,
        coefficient: IndexedCoefficient,
        limits: RelationLimits,
        ingress: CoefficientIngress,
    ) -> Result<(), ParametricRelationError> {
        self.relation.validate_context(context)?;
        self.relation.validate_shift(&shift)?;

        let coefficient_bound = match ingress {
            CoefficientIngress::Authenticate => context.authenticate_coefficient_with_limits(
                &coefficient,
                limits.arithmetic.exact_algebra,
            )?,
            CoefficientIngress::Sealed => context.bind_sealed(&coefficient)?,
        };

        // Inspect the incoming fraction before testing whether its numerator
        // is zero. This preserves an intentionally unnormalized `0 / p` as a
        // domain-bearing zero term. The ingress step above is the sole full
        // scan; denominator extraction consumes its context-bound proof.
        let denominator = context.denominator_condition_from_bound(coefficient_bound)?;
        let condition = ParametricNonZeroCondition::from_authenticated_with_limits(
            denominator,
            [IdentityConditionSource::RelationInputTermDenominator {
                row: self.relation.row_id.clone(),
                shift: shift.values().to_vec().into_boxed_slice(),
            }],
            limits.identity_conditions,
        )?;
        self.add_authenticated_nonzero_condition(context, condition, limits)?;
        if coefficient.is_zero() {
            return Ok(());
        }
        if let Some(current) = self.relation.terms.get(&shift) {
            let current = context.bind_sealed(current)?;
            let sum = context.add_bound_with_limits(
                current,
                coefficient_bound,
                limits.arithmetic.exact_algebra,
            )?;
            if sum.is_zero() {
                self.relation.terms.remove(&shift);
            } else {
                let sum_bound = context.bind_sealed(&sum)?;
                let denominator = context.denominator_condition_from_bound(sum_bound)?;
                let condition = ParametricNonZeroCondition::from_authenticated_with_limits(
                    denominator,
                    [IdentityConditionSource::RelationCollectedTermDenominator {
                        row: self.relation.row_id.clone(),
                        shift: shift.values().to_vec().into_boxed_slice(),
                    }],
                    limits.identity_conditions,
                )?;
                self.add_authenticated_nonzero_condition(context, condition, limits)?;
                self.relation.terms.insert(shift, sum);
            }
        } else {
            self.relation.terms.insert(shift, coefficient);
        }
        Ok(())
    }

    pub(in crate::identity) fn add_scaled(
        &mut self,
        context: &IndexedCoefficientContext,
        other: &ParametricRelation,
        factor: &IndexedCoefficient,
        limits: RelationLimits,
    ) -> Result<(), ParametricRelationError> {
        self.relation.validate_compatible(other, context)?;
        // Extracting the denominator authenticates the complete factor. Do
        // this before mutating the target so wrong-context and arithmetic
        // admission errors retain their original precedence.
        let factor = context
            .authenticate_coefficient_with_limits(factor, limits.arithmetic.exact_algebra)?;
        let factor_denominator = context.denominator_condition_from_bound(factor)?;
        for condition in &other.nonzero_conditions {
            // Compatibility seals the source relation's identity, but it may
            // have been constructed under looser arithmetic limits. Re-admit
            // each borrowed condition exactly once under the current policy,
            // before copying its polynomial or provenance storage.
            self.copy_nonzero_condition_with_readmission(context, condition, limits)?;
        }
        let factor_condition = ParametricNonZeroCondition::from_authenticated_with_limits(
            factor_denominator,
            [IdentityConditionSource::RelationScaleFactorDenominator {
                target_row: self.relation.row_id.clone(),
                source_row: other.row_id.clone(),
            }],
            limits.identity_conditions,
        )?;
        self.add_authenticated_nonzero_condition(context, factor_condition, limits)?;
        for (shift, coefficient) in &other.terms {
            let coefficient = context.bind_sealed(coefficient)?;
            let scaled = context.mul_bound_with_limits(
                coefficient,
                factor,
                limits.arithmetic.exact_algebra,
            )?;
            self.add_term_with_ingress(
                context,
                shift.clone(),
                scaled,
                limits,
                CoefficientIngress::Sealed,
            )?;
        }
        Ok(())
    }
}
