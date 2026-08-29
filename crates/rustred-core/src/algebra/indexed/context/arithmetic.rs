use crate::algebra::coefficient::{
    trusted_coefficient_add_on_map, trusted_coefficient_mul_on_map, trusted_coefficient_neg_on_map,
    trusted_coefficient_sub_on_map,
};
use crate::algebra::{ExactAlgebraLimits, IndexedCoefficientContext};

use super::super::error::IndexedAlgebraError;
use super::super::value::IndexedCoefficient;
use super::BoundIndexedCoefficient;

impl IndexedCoefficientContext {
    pub fn add(
        &self,
        left: &IndexedCoefficient,
        right: &IndexedCoefficient,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.add_with_limits(left, right, ExactAlgebraLimits::default())
    }

    pub fn add_with_limits(
        &self,
        left: &IndexedCoefficient,
        right: &IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        let left = self.authenticate_coefficient_with_limits(left, limits)?;
        let right = self.authenticate_coefficient_with_limits(right, limits)?;
        self.add_bound_with_limits(left, right, limits)
    }

    pub(crate) fn add_bound_with_limits(
        &self,
        left: BoundIndexedCoefficient<'_, '_>,
        right: BoundIndexedCoefficient<'_, '_>,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.validate_bound(left)?;
        self.validate_bound(right)?;
        let raw = trusted_coefficient_add_on_map(
            &left.value.raw,
            &right.value.raw,
            &self.variables,
            limits,
        )?;
        self.record_authenticated_native_result();
        Ok(self.wrap_sealed(raw))
    }

    pub fn sub(
        &self,
        left: &IndexedCoefficient,
        right: &IndexedCoefficient,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.sub_with_limits(left, right, ExactAlgebraLimits::default())
    }

    pub fn sub_with_limits(
        &self,
        left: &IndexedCoefficient,
        right: &IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        let left = self.authenticate_coefficient_with_limits(left, limits)?;
        let right = self.authenticate_coefficient_with_limits(right, limits)?;
        self.sub_bound_with_limits(left, right, limits)
    }

    pub(crate) fn sub_bound_with_limits(
        &self,
        left: BoundIndexedCoefficient<'_, '_>,
        right: BoundIndexedCoefficient<'_, '_>,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.validate_bound(left)?;
        self.validate_bound(right)?;
        let raw = trusted_coefficient_sub_on_map(
            &left.value.raw,
            &right.value.raw,
            &self.variables,
            limits,
        )?;
        self.record_authenticated_native_result();
        Ok(self.wrap_sealed(raw))
    }

    pub fn mul(
        &self,
        left: &IndexedCoefficient,
        right: &IndexedCoefficient,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.mul_with_limits(left, right, ExactAlgebraLimits::default())
    }

    pub fn mul_with_limits(
        &self,
        left: &IndexedCoefficient,
        right: &IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        let left = self.authenticate_coefficient_with_limits(left, limits)?;
        let right = self.authenticate_coefficient_with_limits(right, limits)?;
        self.mul_bound_with_limits(left, right, limits)
    }

    pub(crate) fn mul_bound_with_limits(
        &self,
        left: BoundIndexedCoefficient<'_, '_>,
        right: BoundIndexedCoefficient<'_, '_>,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.validate_bound(left)?;
        self.validate_bound(right)?;
        let raw = trusted_coefficient_mul_on_map(
            &left.value.raw,
            &right.value.raw,
            &self.variables,
            limits,
        )?;
        self.record_authenticated_native_result();
        Ok(self.wrap_sealed(raw))
    }

    pub fn neg_with_limits(
        &self,
        value: &IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        let value = self.authenticate_coefficient_with_limits(value, limits)?;
        self.neg_bound_with_limits(value, limits)
    }

    pub(crate) fn neg_bound_with_limits(
        &self,
        value: BoundIndexedCoefficient<'_, '_>,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.validate_bound(value)?;
        let raw = trusted_coefficient_neg_on_map(&value.value.raw, &self.variables, limits)?;
        self.record_authenticated_native_result();
        Ok(self.wrap_sealed(raw))
    }
}
