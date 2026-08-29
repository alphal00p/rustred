//! Resource-admitted arithmetic in Symbolica's native polynomial ring.

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::algebra::Coefficient;

use super::FeynmanPolynomialContext;
use crate::family::symanzik::error::FeynmanPolynomialError;
use crate::family::symanzik::model::FeynmanPolynomial;
use crate::family::symanzik::work::{FeynmanWorkBudget, check_limit, checked_add, checked_mul};

impl FeynmanPolynomialContext {
    pub(in crate::family::symanzik) fn add(
        &self,
        left: &FeynmanPolynomial,
        right: &FeynmanPolynomial,
        work: &mut FeynmanWorkBudget,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.admit_sum(left, right, work)?;
        let raw = catch_unwind(AssertUnwindSafe(|| left.raw() + right.raw()))
            .map_err(|_| FeynmanPolynomialError::SymbolicaPanic)?;
        self.rebind_native_result(raw)
    }

    pub(in crate::family::symanzik) fn sub(
        &self,
        left: &FeynmanPolynomial,
        right: &FeynmanPolynomial,
        work: &mut FeynmanWorkBudget,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.admit_sum(left, right, work)?;
        let raw = catch_unwind(AssertUnwindSafe(|| left.raw() - right.raw()))
            .map_err(|_| FeynmanPolynomialError::SymbolicaPanic)?;
        self.rebind_native_result(raw)
    }

    fn admit_sum(
        &self,
        left: &FeynmanPolynomial,
        right: &FeynmanPolynomial,
        work: &mut FeynmanWorkBudget,
    ) -> Result<(), FeynmanPolynomialError> {
        self.authenticate(left)?;
        self.authenticate(right)?;
        let term_slots = checked_add(
            left.raw.nterms(),
            right.raw.nterms(),
            "prospective Symbolica polynomial sum terms",
        )?;
        check_limit(
            "prospective Symbolica polynomial sum terms",
            term_slots,
            self.limits.max_polynomial_terms,
        )?;
        check_limit(
            "Feynman polynomial additions",
            term_slots,
            self.limits.max_term_operations,
        )?;
        work.charge_term_operations(term_slots)?;
        self.check_exponent_entries(
            term_slots,
            "prospective Symbolica polynomial sum exponent entries",
        )?;
        Ok(())
    }

    pub(in crate::family::symanzik) fn mul(
        &self,
        left: &FeynmanPolynomial,
        right: &FeynmanPolynomial,
        work: &mut FeynmanWorkBudget,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.authenticate(left)?;
        self.authenticate(right)?;
        if left.is_zero() || right.is_zero() {
            return Ok(self.zero());
        }

        let products = checked_mul(
            left.raw.nterms(),
            right.raw.nterms(),
            "prospective Symbolica polynomial term products",
        )?;
        check_limit(
            "prospective Symbolica polynomial product terms",
            products,
            self.limits.max_polynomial_terms,
        )?;
        let operations = checked_mul(products, 2, "Feynman polynomial term operations")?;
        check_limit(
            "Feynman polynomial term operations",
            operations,
            self.limits.max_term_operations,
        )?;
        work.charge_term_operations(operations)?;
        self.check_exponent_entries(
            products,
            "prospective Symbolica polynomial product exponent entries",
        )?;
        self.admit_product_exponents(left, right)?;

        // Symbolica chooses its native dense or heap multiplication lane. Its
        // coefficient-ring intermediates are not exposed for per-step census;
        // authenticated inputs and the prospective outer-ring bounds above
        // therefore precede the call, and the retained result is checked below.
        let raw = catch_unwind(AssertUnwindSafe(|| left.raw() * right.raw()))
            .map_err(|_| FeynmanPolynomialError::SymbolicaPanic)?;
        self.rebind_native_result(raw)
    }

    fn admit_product_exponents(
        &self,
        left: &FeynmanPolynomial,
        right: &FeynmanPolynomial,
    ) -> Result<(), FeynmanPolynomialError> {
        for variable in 0..self.parameter_count() {
            let requested = u32::from(left.raw.degree(variable))
                .checked_add(u32::from(right.raw.degree(variable)))
                .ok_or(FeynmanPolynomialError::ResourceCountOverflow {
                    resource: "prospective Feynman-parameter exponent",
                })?;
            if requested > u32::from(self.limits.max_parameter_exponent) {
                return Err(FeynmanPolynomialError::ParameterExponentOverflow {
                    variable,
                    requested,
                    limit: self.limits.max_parameter_exponent,
                });
            }
        }
        Ok(())
    }

    pub(in crate::family::symanzik) fn scale(
        &self,
        polynomial: &FeynmanPolynomial,
        coefficient: &Coefficient,
        work: &mut FeynmanWorkBudget,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.authenticate(polynomial)?;
        self.coefficients
            .validate_with_limits(coefficient, self.limits.exact_algebra)?;
        if polynomial.is_zero() || coefficient.is_zero() {
            return Ok(self.zero());
        }
        check_limit(
            "Feynman polynomial coefficient products",
            polynomial.raw.nterms(),
            self.limits.max_term_operations,
        )?;
        work.charge_term_operations(polynomial.raw.nterms())?;

        let raw = catch_unwind(AssertUnwindSafe(|| {
            polynomial.raw.clone().mul_coeff(coefficient.clone())
        }))
        .map_err(|_| FeynmanPolynomialError::SymbolicaPanic)?;
        self.rebind_native_result(raw)
    }
}
