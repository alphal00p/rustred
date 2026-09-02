//! Bounded Symbolica-native polynomial arithmetic for cleared replay.

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::algebra::{
    ExactAlgebraError, IndexedAlgebraError, IndexedCoefficient, IndexedCoefficientContext,
    IndexedPolynomial,
};

use super::model::{ClearedCircuitError, ClearedCircuitLimits};

pub(super) const SOURCE_CONTRIBUTIONS: &str = "cleared-circuit source contributions";
pub(super) const SOURCE_TERMS: &str = "cleared-circuit source terms";
pub(super) const PHYSICAL_COLUMNS: &str = "cleared-circuit physical columns";
const POLYNOMIAL_OPERATIONS: &str = "cleared-circuit polynomial operations";
const GCD_TERM_PAIRS: &str = "cleared-circuit GCD term-pair work";
const RETAINED_POLYNOMIAL_TERMS: &str = "cleared-circuit retained polynomial terms";
pub(super) const GUARDS: &str = "cleared-circuit semantic guards";
pub(super) const GUARD_ORIGINS: &str = "cleared-circuit semantic guard origins";
pub(super) const CONDITION_SOURCES: &str = "cleared-circuit condition-source entries";

pub(super) struct PolynomialBudget<'context> {
    context: &'context IndexedCoefficientContext,
    limits: ClearedCircuitLimits,
    pub(super) operations: usize,
    pub(super) gcd_term_pairs: usize,
    pub(super) retained_terms: usize,
}

impl<'context> PolynomialBudget<'context> {
    pub(super) const fn new(
        context: &'context IndexedCoefficientContext,
        limits: ClearedCircuitLimits,
    ) -> Self {
        Self {
            context,
            limits,
            operations: 0,
            gcd_term_pairs: 0,
            retained_terms: 0,
        }
    }

    pub(super) fn charge_operation(&mut self) -> Result<(), ClearedCircuitError> {
        self.operations = checked_add(POLYNOMIAL_OPERATIONS, self.operations, 1)?;
        check_limit(
            POLYNOMIAL_OPERATIONS,
            self.operations,
            self.limits.max_polynomial_operations,
        )
    }

    pub(super) fn retain(
        &mut self,
        polynomial: &IndexedPolynomial,
    ) -> Result<(), ClearedCircuitError> {
        self.retained_terms = checked_add(
            RETAINED_POLYNOMIAL_TERMS,
            self.retained_terms,
            polynomial.raw().nterms(),
        )?;
        check_limit(
            RETAINED_POLYNOMIAL_TERMS,
            self.retained_terms,
            self.limits.max_retained_polynomial_terms,
        )
    }

    pub(super) fn one_polynomial(&mut self) -> Result<IndexedPolynomial, ClearedCircuitError> {
        self.charge_operation()?;
        let one = self.context.one();
        let polynomial = self
            .context
            .numerator_condition_with_limits(&one, self.limits.exact_algebra)?;
        self.retain(&polynomial)?;
        Ok(polynomial)
    }

    pub(super) fn as_coefficient(
        &mut self,
        polynomial: &IndexedPolynomial,
    ) -> Result<IndexedCoefficient, ClearedCircuitError> {
        self.charge_operation()?;
        self.context
            .validate_polynomial_with_limits(polynomial, self.limits.exact_algebra)?;
        Ok(self.context.admit_native_result_with_limits(
            polynomial.raw().clone().into(),
            self.limits.exact_algebra,
        )?)
    }

    pub(super) fn require_polynomial(
        &mut self,
        coefficient: &IndexedCoefficient,
    ) -> Result<IndexedPolynomial, ClearedCircuitError> {
        self.charge_operation()?;
        self.context
            .validate_with_limits(coefficient, self.limits.exact_algebra)?;
        if !coefficient.raw().denominator.is_one() {
            return Err(ClearedCircuitError::RationalCoefficientSurvivedClearing);
        }
        let polynomial = self
            .context
            .numerator_condition_with_limits(coefficient, self.limits.exact_algebra)?;
        self.retain(&polynomial)?;
        Ok(polynomial)
    }

    pub(super) fn add(
        &mut self,
        left: &IndexedCoefficient,
        right: &IndexedCoefficient,
    ) -> Result<IndexedCoefficient, ClearedCircuitError> {
        self.charge_operation()?;
        Ok(self
            .context
            .add_with_limits(left, right, self.limits.exact_algebra)?)
    }

    pub(super) fn mul(
        &mut self,
        left: &IndexedCoefficient,
        right: &IndexedCoefficient,
    ) -> Result<IndexedCoefficient, ClearedCircuitError> {
        self.charge_operation()?;
        Ok(self
            .context
            .mul_with_limits(left, right, self.limits.exact_algebra)?)
    }

    pub(super) fn div(
        &mut self,
        numerator: &IndexedCoefficient,
        denominator: &IndexedCoefficient,
    ) -> Result<IndexedCoefficient, ClearedCircuitError> {
        self.charge_operation()?;
        Ok(self
            .context
            .div_with_limits(numerator, denominator, self.limits.exact_algebra)?)
    }

    pub(super) fn polynomial_gcd(
        &mut self,
        left: &IndexedPolynomial,
        right: &IndexedPolynomial,
    ) -> Result<IndexedPolynomial, ClearedCircuitError> {
        self.context
            .validate_polynomial_with_limits(left, self.limits.exact_algebra)?;
        self.context
            .validate_polynomial_with_limits(right, self.limits.exact_algebra)?;
        self.charge_operation()?;
        let work = checked_mul(GCD_TERM_PAIRS, left.raw().nterms(), right.raw().nterms())?;
        self.gcd_term_pairs = checked_add(GCD_TERM_PAIRS, self.gcd_term_pairs, work)?;
        check_limit(
            GCD_TERM_PAIRS,
            self.gcd_term_pairs,
            self.limits.max_gcd_term_pairs,
        )?;
        let raw = catch_unwind(AssertUnwindSafe(|| left.raw().gcd(right.raw()))).map_err(|_| {
            ClearedCircuitError::NativePanic {
                operation: "computing a cleared-circuit polynomial GCD",
            }
        })?;
        let coefficient = self
            .context
            .admit_native_result_with_limits(raw.into(), self.limits.exact_algebra)?;
        self.require_polynomial(&coefficient)
    }

    pub(super) fn polynomial_lcm(
        &mut self,
        left: &IndexedPolynomial,
        right: &IndexedPolynomial,
    ) -> Result<IndexedPolynomial, ClearedCircuitError> {
        if left.is_zero() || right.is_zero() {
            return Err(ClearedCircuitError::ZeroFinalTargetCoefficient);
        }
        if left.raw().is_one() {
            return Ok(right.clone());
        }
        if right.raw().is_one() {
            return Ok(left.clone());
        }
        let gcd = self.polynomial_gcd(left, right)?;
        let quotient = self.exact_polynomial_division(left, &gcd)?;
        let quotient = self.as_coefficient(&quotient)?;
        let right = self.as_coefficient(right)?;
        let product = self.mul(&quotient, &right)?;
        self.require_polynomial(&product)
    }

    pub(super) fn exact_polynomial_division(
        &mut self,
        numerator: &IndexedPolynomial,
        denominator: &IndexedPolynomial,
    ) -> Result<IndexedPolynomial, ClearedCircuitError> {
        self.context
            .validate_polynomial_with_limits(numerator, self.limits.exact_algebra)?;
        self.context
            .validate_polynomial_with_limits(denominator, self.limits.exact_algebra)?;
        if denominator.is_zero() {
            return Err(ClearedCircuitError::IndexedAlgebra(
                IndexedAlgebraError::ExactAlgebra(ExactAlgebraError::DivisionByZero),
            ));
        }

        // Symbolica exposes no quotient scratch-work census. Charge the one
        // logical operation and authenticate both inputs and the exact output
        // instead of presenting an operand-term count as an internal bound.
        self.charge_operation()?;
        let raw = catch_unwind(AssertUnwindSafe(|| {
            numerator.raw().try_div(denominator.raw())
        }))
        .map_err(|_| ClearedCircuitError::NativePanic {
            operation: "performing cleared-circuit exact polynomial division",
        })?
        .ok_or(ClearedCircuitError::NonExactPolynomialDivision)?;
        let quotient = self
            .context
            .admit_native_polynomial_result_with_limits(raw, self.limits.exact_algebra)?;
        self.retain(&quotient)?;
        Ok(quotient)
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ClearedCircuitError> {
    left.checked_add(right)
        .ok_or(ClearedCircuitError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ClearedCircuitError> {
    left.checked_mul(right)
        .ok_or(ClearedCircuitError::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ClearedCircuitError> {
    if requested > limit {
        Err(ClearedCircuitError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, ClearedCircuitError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| ClearedCircuitError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}
