//! Structural and coefficient authentication for bound polynomials.

use super::FeynmanPolynomialContext;
use crate::family::symanzik::error::FeynmanPolynomialError;
use crate::family::symanzik::model::FeynmanPolynomial;
use crate::family::symanzik::work::check_limit;

impl FeynmanPolynomialContext {
    pub(in crate::family::symanzik) fn authenticate(
        &self,
        polynomial: &FeynmanPolynomial,
    ) -> Result<(), FeynmanPolynomialError> {
        if polynomial.context != self.family_fingerprint
            || polynomial.raw.variables.as_ref() != self.variables.as_ref()
            || polynomial.raw.ring != self.field
        {
            return Err(FeynmanPolynomialError::ForeignPolynomialContext);
        }
        let expected = polynomial
            .raw
            .coefficients
            .len()
            .checked_mul(self.parameter_count())
            .ok_or(FeynmanPolynomialError::ResourceCountOverflow {
                resource: "Feynman polynomial exponent layout",
            })?;
        check_limit(
            "Feynman polynomial exponent entries",
            expected,
            self.limits.max_exponent_entries,
        )?;
        if polynomial.raw.exponents.len() != expected {
            return Err(FeynmanPolynomialError::MalformedPolynomial {
                detail: format!(
                    "{} coefficients, {} exponents, {} variables",
                    polynomial.raw.coefficients.len(),
                    polynomial.raw.exponents.len(),
                    self.parameter_count()
                ),
            });
        }
        check_limit(
            "Feynman polynomial terms",
            polynomial.raw.nterms(),
            self.limits.max_polynomial_terms,
        )?;
        for (term, coefficient) in polynomial.raw.coefficients.iter().enumerate() {
            self.coefficients
                .validate_with_limits(coefficient, self.limits.exact_algebra)?;
            if coefficient.is_zero() {
                return Err(FeynmanPolynomialError::MalformedPolynomial {
                    detail: format!("explicit zero coefficient at term {term}"),
                });
            }
        }
        for (term, exponents) in polynomial.raw.exponents_iter().enumerate() {
            for (variable, &exponent) in exponents.iter().enumerate() {
                if exponent > self.limits.max_parameter_exponent {
                    return Err(FeynmanPolynomialError::ParameterExponentOverflow {
                        variable,
                        requested: u32::from(exponent),
                        limit: self.limits.max_parameter_exponent,
                    });
                }
            }
            if term > 0 && polynomial.raw.exponents(term - 1) >= exponents {
                return Err(FeynmanPolynomialError::MalformedPolynomial {
                    detail: format!("non-canonical monomial order at term {term}"),
                });
            }
        }
        Ok(())
    }
}
