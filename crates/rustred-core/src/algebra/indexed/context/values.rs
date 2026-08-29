use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::*;

use crate::algebra::{
    Coefficient, CoefficientPolynomial, ExactAlgebraLimits, validate_coefficient_on_map,
    validate_polynomial_on_map,
};

use super::super::error::IndexedAlgebraError;
use super::super::value::{IndexedCoefficient, IndexedPolynomial};
use super::{BoundIndexedCoefficient, IndexedCoefficientContext};

impl IndexedCoefficientContext {
    pub fn zero(&self) -> IndexedCoefficient {
        self.wrap_sealed(self.template.numerator.zero().into())
    }

    pub fn one(&self) -> IndexedCoefficient {
        self.wrap_sealed(self.template.numerator.one().into())
    }

    pub fn integer(&self, value: i64) -> IndexedCoefficient {
        self.wrap_sealed(
            self.template
                .numerator
                .constant(Integer::from(value))
                .into(),
        )
    }

    pub fn index(&self, position: usize) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        let variable =
            self.index_variables
                .get(position)
                .ok_or(IndexedAlgebraError::WrongIndexArity {
                    expected: self.index_count(),
                    actual: position.saturating_add(1),
                })?;
        let polynomial = self
            .template
            .numerator
            .variable(variable)
            .map_err(IndexedAlgebraError::Symbolica)?;
        Ok(self.wrap_sealed(polynomial.into()))
    }

    pub fn lift(&self, value: &Coefficient) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.base
            .validate_with_limits(value, ExactAlgebraLimits::default())
            .map_err(|_| IndexedAlgebraError::WrongContext)?;
        let numerator = self.extend_authenticated_base_polynomial(&value.numerator);
        let denominator = self.extend_authenticated_base_polynomial(&value.denominator);
        if denominator.is_zero() {
            return Err(IndexedAlgebraError::ZeroDenominator);
        }
        let raw = <Coefficient as FromNumeratorAndDenominator<
            IntegerRing,
            IntegerRing,
            u16,
        >>::from_num_den(numerator, denominator, &Z, true);
        self.wrap_checked(raw)
    }

    pub fn lift_base_polynomial(
        &self,
        value: &CoefficientPolynomial,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        let raw = self.extend_base_polynomial(value)?;
        Ok(IndexedPolynomial {
            raw,
            context: self.fingerprint.clone(),
        })
    }

    pub fn denominator_condition_with_limits(
        &self,
        value: &IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        let value = self.authenticate_coefficient_with_limits(value, limits)?;
        self.denominator_condition_from_bound(value)
    }

    pub fn numerator_condition_with_limits(
        &self,
        value: &IndexedCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        let value = self.authenticate_coefficient_with_limits(value, limits)?;
        self.numerator_condition_from_bound(value)
    }

    pub(crate) fn numerator_condition_from_bound(
        &self,
        value: BoundIndexedCoefficient<'_, '_>,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        self.validate_bound(value)?;
        Ok(IndexedPolynomial {
            raw: value.value.raw.numerator.clone(),
            context: self.fingerprint.clone(),
        })
    }

    /// Extract a denominator from a coefficient already authenticated or
    /// sealed to this exact context.
    pub(crate) fn denominator_condition_from_bound(
        &self,
        value: BoundIndexedCoefficient<'_, '_>,
    ) -> Result<IndexedPolynomial, IndexedAlgebraError> {
        self.validate_bound(value)?;
        Ok(IndexedPolynomial {
            raw: value.value.raw.denominator.clone(),
            context: self.fingerprint.clone(),
        })
    }

    pub(super) fn wrap_sealed(&self, raw: Coefficient) -> IndexedCoefficient {
        debug_assert!(self.raw_has_sealed_shape(&raw));
        IndexedCoefficient {
            raw,
            context: self.fingerprint.clone(),
        }
    }

    fn raw_has_sealed_shape(&self, raw: &Coefficient) -> bool {
        let polynomial_has_shape = |polynomial: &CoefficientPolynomial| {
            polynomial.variables.as_ref() == self.variables.as_ref()
                && polynomial
                    .coefficients
                    .len()
                    .checked_mul(self.variables.len())
                    == Some(polynomial.exponents.len())
        };
        polynomial_has_shape(&raw.numerator)
            && polynomial_has_shape(&raw.denominator)
            && !raw.denominator.coefficients.is_empty()
    }

    fn wrap_checked(&self, raw: Coefficient) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.wrap_checked_with_limits(raw, ExactAlgebraLimits::default())
    }

    pub(in crate::algebra::indexed) fn wrap_checked_with_limits(
        &self,
        raw: Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        validate_coefficient_on_map(&raw, &self.variables, limits)?;
        self.record_authenticated_native_result();
        Ok(self.wrap_sealed(raw))
    }

    /// Admit one raw rational function returned by a native Symbolica
    /// algorithm which consumed values from this exact indexed context.
    ///
    /// Native coefficient fields do not carry RustRed's variable-map
    /// identity. This crate-private seam performs the complete map, layout,
    /// exponent, and resource authentication exactly once before sealing the
    /// result for trusted downstream arithmetic.
    pub(crate) fn admit_native_result_with_limits(
        &self,
        raw: Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<IndexedCoefficient, IndexedAlgebraError> {
        self.wrap_checked_with_limits(raw, limits)
    }

    fn extend_base_polynomial(
        &self,
        source: &CoefficientPolynomial,
    ) -> Result<CoefficientPolynomial, IndexedAlgebraError> {
        validate_polynomial_on_map(
            source,
            self.base.variables(),
            crate::algebra::CoefficientPolynomialPart::Numerator,
            ExactAlgebraLimits::default(),
        )?;
        Ok(self.extend_authenticated_base_polynomial(source))
    }

    fn extend_authenticated_base_polynomial(
        &self,
        source: &CoefficientPolynomial,
    ) -> CoefficientPolynomial {
        let mut result = self
            .template
            .numerator
            .zero_with_capacity(source.coefficients.len());
        let mut exponents = vec![0_u16; self.variables.len()];
        for (coefficient, source_exponents) in
            source.coefficients.iter().zip(source.exponents_iter())
        {
            exponents.fill(0);
            exponents[..self.base.variables().len()].copy_from_slice(source_exponents);
            result.append_monomial(coefficient.clone(), &exponents);
        }
        result
    }
}
