//! Context construction, polynomial binding, and basic state access.

use std::sync::Arc;

use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::*;

use crate::algebra::{Coefficient, CoefficientContext, is_exact_plain_symbol};
use crate::family::IntegralFamily;

use super::FeynmanPolynomialContext;
use crate::family::symanzik::error::FeynmanPolynomialError;
use crate::family::symanzik::model::{
    FeynmanPolynomial, FeynmanPolynomialLimits, RawFeynmanPolynomial,
};
use crate::family::symanzik::work::{check_limit, checked_mul};

/// Reject process-global metadata that would change the algebraic or printed
/// meaning of RustRed's positional Feynman parameters.
pub(in crate::family::symanzik) fn authenticate_feynman_symbol(
    symbol: Symbol,
    qualified: &str,
    parameter: usize,
) -> Result<(), FeynmanPolynomialError> {
    if is_exact_plain_symbol(symbol, qualified) {
        Ok(())
    } else {
        Err(FeynmanPolynomialError::FeynmanParameterSymbolCollision { parameter })
    }
}

impl FeynmanPolynomialContext {
    pub(in crate::family::symanzik) fn try_new(
        family: &IntegralFamily,
        limits: FeynmanPolynomialLimits,
    ) -> Result<Self, FeynmanPolynomialError> {
        check_limit(
            "Feynman parameters",
            family.denominator_count(),
            limits.max_parameters,
        )?;
        let field = RationalPolynomialField::new(Z);
        let mut variables = Vec::with_capacity(family.denominator_count());
        for parameter in 0..family.denominator_count() {
            let name = format!("rustred::feynman_x_{parameter}");
            let namespaced = NamespacedSymbol::try_parse(&name).ok_or_else(|| {
                FeynmanPolynomialError::SymbolicaSymbol {
                    parameter,
                    detail: "invalid namespaced symbol".to_owned(),
                }
            })?;
            let symbol = SymbolBuilder::new(namespaced).build().map_err(|error| {
                FeynmanPolynomialError::SymbolicaSymbol {
                    parameter,
                    detail: error.to_string(),
                }
            })?;
            authenticate_feynman_symbol(symbol, &name, parameter)?;
            let variable = PolyVariable::Symbol(symbol);
            if let Some(base_parameter) = family
                .coefficient_context()
                .variables()
                .iter()
                .position(|candidate| candidate == &variable)
            {
                return Err(FeynmanPolynomialError::FeynmanBaseSymbolCollision {
                    parameter,
                    base_parameter: family.coefficient_context().parameter_names()[base_parameter]
                        .clone(),
                });
            }
            variables.push(variable);
        }
        let variables = Arc::new(variables);
        let template = MultivariatePolynomial::new(&field, None, variables.clone());
        Ok(Self {
            family_fingerprint: family.fingerprint_owner(),
            coefficients: family.coefficient_context().clone(),
            variables,
            field,
            template,
            limits,
        })
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn parameter_count(&self) -> usize {
        self.variables.len()
    }

    pub fn limits(&self) -> FeynmanPolynomialLimits {
        self.limits
    }

    pub fn coefficient_context(&self) -> &CoefficientContext {
        &self.coefficients
    }

    pub(in crate::family::symanzik) fn zero(&self) -> FeynmanPolynomial {
        self.wrap(self.template.zero())
    }

    pub(in crate::family::symanzik) fn one(
        &self,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.constant(self.coefficients.one())
    }

    pub(in crate::family::symanzik) fn constant(
        &self,
        coefficient: Coefficient,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.coefficients
            .validate_with_limits(&coefficient, self.limits.exact_algebra)?;
        if coefficient.is_zero() {
            return Ok(self.zero());
        }
        check_limit(
            "Feynman polynomial terms",
            1,
            self.limits.max_polynomial_terms,
        )?;
        self.check_exponent_entries(1, "Feynman polynomial constant exponent entries")?;
        self.rebind_native_result(self.template.constant(coefficient))
    }

    pub(in crate::family::symanzik) fn parameter_monomial(
        &self,
        parameter: usize,
        coefficient: &Coefficient,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.coefficients
            .validate_with_limits(coefficient, self.limits.exact_algebra)?;
        if coefficient.is_zero() {
            return Ok(self.zero());
        }
        if parameter >= self.parameter_count() {
            return Err(FeynmanPolynomialError::InternalVerificationFailure {
                detail: format!("parameter {parameter} is out of range"),
            });
        }
        check_limit(
            "Feynman polynomial terms",
            1,
            self.limits.max_polynomial_terms,
        )?;
        self.check_exponent_entries(1, "Feynman parameter monomial exponent entries")?;
        let mut exponents = vec![0_u16; self.parameter_count()];
        exponents[parameter] = 1;
        self.rebind_native_result(self.template.monomial(coefficient.clone(), exponents))
    }

    pub(in crate::family::symanzik) fn check_exponent_entries(
        &self,
        terms: usize,
        resource: &'static str,
    ) -> Result<usize, FeynmanPolynomialError> {
        let entries = checked_mul(terms, self.parameter_count(), resource)?;
        check_limit(resource, entries, self.limits.max_exponent_entries)?;
        Ok(entries)
    }

    pub(in crate::family::symanzik) fn wrap(&self, raw: RawFeynmanPolynomial) -> FeynmanPolynomial {
        FeynmanPolynomial {
            raw,
            context: self.family_fingerprint.clone(),
        }
    }

    /// Bind one native Symbolica result back to this context's ordered
    /// variable map.  `PolynomialRing::zero()` deliberately has an empty
    /// variable map, so an identically-zero native result must use the
    /// authenticated template zero instead.
    pub(in crate::family::symanzik) fn rebind_native_result(
        &self,
        mut raw: RawFeynmanPolynomial,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        if raw.is_zero() {
            return Ok(self.zero());
        }
        let candidate = self.wrap(raw);
        self.authenticate(&candidate)?;
        raw = candidate.raw;
        raw.variables = self.variables.clone();
        Ok(self.wrap(raw))
    }
}
