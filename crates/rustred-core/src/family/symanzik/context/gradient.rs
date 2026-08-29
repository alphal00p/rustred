//! Resource-admitted differentiation in Symbolica's native polynomial ring.

use std::panic::{AssertUnwindSafe, catch_unwind};

use super::FeynmanPolynomialContext;
use crate::family::symanzik::error::FeynmanPolynomialError;
use crate::family::symanzik::model::FeynmanPolynomial;
use crate::family::symanzik::work::{FeynmanWorkBudget, check_limit, checked_mul};

impl FeynmanPolynomialContext {
    /// Differentiate an authenticated polynomial with respect to every
    /// Feynman parameter, retaining this context's full ordered variable map.
    pub fn try_gradient(
        &self,
        polynomial: &FeynmanPolynomial,
    ) -> Result<Vec<FeynmanPolynomial>, FeynmanPolynomialError> {
        self.authenticate(polynomial)?;
        let mut work = FeynmanWorkBudget::new(self.limits);
        let operations = checked_mul(
            polynomial.raw.nterms(),
            self.parameter_count(),
            "Feynman polynomial derivative terms",
        )?;
        check_limit(
            "Feynman polynomial derivative terms",
            operations,
            self.limits.max_term_operations,
        )?;
        work.charge_term_operations(operations)?;
        let output_entries = checked_mul(
            operations,
            self.parameter_count(),
            "Feynman gradient exponent entries",
        )?;
        check_limit(
            "Feynman gradient exponent entries",
            output_entries,
            self.limits.max_exponent_entries,
        )?;

        let mut gradient = Vec::new();
        gradient
            .try_reserve_exact(self.parameter_count())
            .map_err(|_| FeynmanPolynomialError::AllocationFailure {
                resource: "Feynman gradient polynomials",
                requested: self.parameter_count(),
            })?;
        for variable in 0..self.parameter_count() {
            // `MultivariatePolynomial::derivative` preserves the complete
            // variable map and delegates coefficient scaling to the native
            // coefficient ring. The native API has no fallible resource hook,
            // so the structural census precedes it and authentication follows.
            let raw = catch_unwind(AssertUnwindSafe(|| polynomial.raw.derivative(variable)))
                .map_err(|_| FeynmanPolynomialError::SymbolicaPanic)?;
            gradient.push(self.rebind_native_result(raw)?);
        }
        Ok(gradient)
    }
}
