//! Checked storage adapter for Symbolica's native factorized denominator.
//!
//! No factorization, cancellation or polynomial arithmetic is implemented here.
//! The private value remains bound to one exact base-variable context. Resource
//! envelopes are conservative, and do not bound all native CAS scratch work.

mod admission;
#[cfg(test)]
mod tests;

use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::factorized_rational_polynomial::{
    FactorizedRationalPolynomial, FromNumeratorAndFactorizedDenominator,
};
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::{IntegerRing, RationalPolynomial, Z};

use super::validation::{
    integer_clone_owned_heap_byte_bound, polynomial_clone_owned_heap_byte_bound,
};
use super::{Coefficient, CoefficientContext, ExactAlgebraError, ExactAlgebraLimits};

type Native = FactorizedRationalPolynomial<IntegerRing, u16>;

#[derive(Clone, Debug)]
/// Immutable native value whose complete layout and factor maps were admitted.
/// Each use still checks its caller's context and resource limits. There is no
/// unchecked constructor or mutable native access outside this private module.
pub(crate) struct FactorizedCoefficient {
    value: Native,
}

impl FactorizedCoefficient {
    pub(crate) fn from_coefficient(
        context: &CoefficientContext,
        coefficient: Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<Self, ExactAlgebraError> {
        context.validate_with_limits(&coefficient, limits)?;
        let value = native("factoring an admitted coefficient denominator", || {
            Native::from_num_den(
                coefficient.numerator,
                vec![(coefficient.denominator, 1)],
                &Z,
                true,
            )
        })?;
        Self::admit(context, value, limits)
    }

    fn admit(
        context: &CoefficientContext,
        value: Native,
        limits: ExactAlgebraLimits,
    ) -> Result<Self, ExactAlgebraError> {
        admission::validate(context, &value, limits)?;
        Ok(Self { value })
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.value.is_zero()
    }

    pub(crate) fn try_mul(
        &self,
        other: &Self,
        context: &CoefficientContext,
        limits: ExactAlgebraLimits,
    ) -> Result<Self, ExactAlgebraError> {
        admission::preflight(self, other, context, limits, false)?;
        let value = native("multiplying factorized coefficients", || {
            &self.value * &other.value
        })?;
        Self::admit(context, value, limits)
    }

    pub(crate) fn try_add(
        &self,
        other: &Self,
        context: &CoefficientContext,
        limits: ExactAlgebraLimits,
    ) -> Result<Self, ExactAlgebraError> {
        admission::preflight(self, other, context, limits, true)?;
        let value = native("adding factorized coefficients", || {
            &self.value + &other.value
        })?;
        Self::admit(context, value, limits)
    }

    pub(crate) fn materialize(
        &self,
        context: &CoefficientContext,
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, ExactAlgebraError> {
        admission::preflight_materialization(self, context, limits)?;
        // Same composition of native calls as Symbolica's constructor tests.
        // Keeping `do_gcd=true` reestablishes the public Coefficient invariant.
        let result = native("materializing a factorized coefficient", || {
            let numerator = self
                .value
                .numerator
                .clone()
                .mul_coeff(self.value.numer_coeff.clone());
            let denominator = self.value.denominators.iter().fold(
                numerator.constant(self.value.denom_coeff.clone()),
                |product, (base, power)| product * &base.pow(*power),
            );
            RationalPolynomial::from_num_den(numerator, denominator, &Z, true)
        })?;
        context.validate_with_limits(&result, limits)?;
        Ok(result)
    }

    /// Charged term envelope and clone-owned payload bytes, not process RSS.
    pub(crate) fn cache_weight(&self) -> Result<(usize, usize), ExactAlgebraError> {
        let value = &self.value;
        let stored_terms = value.denominators.iter().try_fold(
            value
                .numerator
                .nterms()
                .checked_add(2)
                .ok_or_else(admission::overflow)?,
            |sum, (factor, _)| {
                sum.checked_add(factor.nterms())
                    .ok_or_else(admission::overflow)
            },
        )?;
        let expanded_terms = value
            .numerator
            .nterms()
            .checked_add(admission::denominator_shape(value)?.terms)
            .ok_or_else(admission::overflow)?;
        let mut bytes = size_of::<Native>()
            .checked_add(
                value
                    .denominators
                    .capacity()
                    .checked_mul(size_of::<(super::CoefficientPolynomial, usize)>())
                    .ok_or_else(admission::overflow)?,
            )
            .and_then(|n| n.checked_add(polynomial_clone_owned_heap_byte_bound(&value.numerator)?))
            .and_then(|n| n.checked_add(integer_clone_owned_heap_byte_bound(&value.numer_coeff)?))
            .and_then(|n| n.checked_add(integer_clone_owned_heap_byte_bound(&value.denom_coeff)?))
            .ok_or_else(admission::overflow)?;
        for (factor, _) in &value.denominators {
            bytes = bytes
                .checked_add(
                    polynomial_clone_owned_heap_byte_bound(factor)
                        .ok_or_else(admission::overflow)?,
                )
                .ok_or_else(admission::overflow)?;
        }
        Ok((stored_terms.max(expanded_terms), bytes))
    }
}

fn native<T>(operation: &'static str, f: impl FnOnce() -> T) -> Result<T, ExactAlgebraError> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|_| ExactAlgebraError::NativePanic { operation })
}
