//! Authenticated coefficient/parameter context and checked polynomial operations.

use std::collections::BTreeMap;
use std::sync::Arc;

use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::*;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{FamilyDomain, IntegralFamily};

use super::error::FeynmanPolynomialError;
use super::model::{FeynmanPolynomial, FeynmanPolynomialLimits, RawFeynmanPolynomial};
use super::work::{FeynmanWorkBudget, check_limit, checked_add, checked_mul};

/// Authenticated coefficient and variable map for `K[x_0,...,x_{N-1}]`.
#[derive(Clone, Debug)]
pub struct FeynmanPolynomialContext {
    pub(super) family_fingerprint: Arc<str>,
    pub(super) coefficients: CoefficientContext,
    pub(super) family_domain: FamilyDomain,
    pub(super) variables: Arc<Vec<PolyVariable>>,
    pub(super) field: RationalPolynomialField<IntegerRing, u16>,
    pub(super) template: RawFeynmanPolynomial,
    pub(super) limits: FeynmanPolynomialLimits,
}

impl FeynmanPolynomialContext {
    pub(super) fn try_new(
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
            family_fingerprint: Arc::from(family.fingerprint()),
            coefficients: family.coefficient_context().clone(),
            family_domain: family.domain().clone(),
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

    /// Canonical generic-locus conditions inherited from the authenticated
    /// family.  They remain necessary even when rational simplification makes
    /// a factor disappear from the visible coefficients of `U`, `F`, or `G`.
    pub fn family_domain(&self) -> &FamilyDomain {
        &self.family_domain
    }

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
        let mut gradient = Vec::with_capacity(self.parameter_count());
        for variable in 0..self.parameter_count() {
            let mut terms = BTreeMap::new();
            for (coefficient, exponents) in polynomial.terms() {
                let exponent = exponents[variable];
                if exponent == 0 {
                    continue;
                }
                let coefficient = self.coefficients.try_mul(
                    coefficient,
                    &self.coefficients.integer(i64::from(exponent)),
                    self.limits.exact_algebra,
                )?;
                let mut derivative_exponents = exponents.to_vec();
                derivative_exponents[variable] -= 1;
                self.accumulate(&mut terms, derivative_exponents, coefficient)?;
            }
            gradient.push(self.from_terms(terms)?);
        }
        Ok(gradient)
    }

    pub(super) fn zero(&self) -> FeynmanPolynomial {
        self.wrap(self.template.zero())
    }

    pub(super) fn one(&self) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.constant(self.coefficients.one())
    }

    pub(super) fn constant(
        &self,
        coefficient: Coefficient,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.coefficients
            .validate_with_limits(&coefficient, self.limits.exact_algebra)?;
        if coefficient.is_zero() {
            return Ok(self.zero());
        }
        self.check_exponent_entries(1, "Feynman polynomial constant exponent entries")?;
        self.from_terms(BTreeMap::from([(
            vec![0_u16; self.parameter_count()],
            coefficient,
        )]))
    }

    pub(super) fn parameter_monomial(
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
        self.check_exponent_entries(1, "Feynman parameter monomial exponent entries")?;
        let mut exponents = vec![0_u16; self.parameter_count()];
        exponents[parameter] = 1;
        self.from_terms(BTreeMap::from([(exponents, coefficient.clone())]))
    }

    pub(super) fn authenticate(
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

    pub(super) fn add(
        &self,
        left: &FeynmanPolynomial,
        right: &FeynmanPolynomial,
        work: &mut FeynmanWorkBudget,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.combine(left, right, false, work)
    }

    pub(super) fn sub(
        &self,
        left: &FeynmanPolynomial,
        right: &FeynmanPolynomial,
        work: &mut FeynmanWorkBudget,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.combine(left, right, true, work)
    }

    pub(super) fn combine(
        &self,
        left: &FeynmanPolynomial,
        right: &FeynmanPolynomial,
        subtract: bool,
        work: &mut FeynmanWorkBudget,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.authenticate(left)?;
        self.authenticate(right)?;
        let operations = checked_add(
            left.raw.nterms(),
            right.raw.nterms(),
            "Feynman polynomial additions",
        )?;
        check_limit(
            "Feynman polynomial additions",
            operations,
            self.limits.max_term_operations,
        )?;
        work.charge_term_operations(operations)?;
        self.check_exponent_entries(operations, "Feynman polynomial addition exponent entries")?;
        let mut terms = self.term_map(left);
        for (coefficient, exponents) in right
            .raw
            .coefficients
            .iter()
            .zip(right.raw.exponents_iter())
        {
            let incoming = if subtract {
                self.coefficients
                    .try_neg(coefficient, self.limits.exact_algebra)?
            } else {
                coefficient.clone()
            };
            self.accumulate(&mut terms, exponents.to_vec(), incoming)?;
        }
        self.from_terms(terms)
    }

    pub(super) fn mul(
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
            "Feynman polynomial term products",
        )?;
        let operations = checked_mul(products, 2, "Feynman polynomial term operations")?;
        check_limit(
            "Feynman polynomial term operations",
            operations,
            self.limits.max_term_operations,
        )?;
        work.charge_term_operations(operations)?;
        self.check_exponent_entries(products, "Feynman polynomial product exponent entries")?;
        let mut terms = BTreeMap::new();
        for (left_coefficient, left_exponents) in
            left.raw.coefficients.iter().zip(left.raw.exponents_iter())
        {
            for (right_coefficient, right_exponents) in right
                .raw
                .coefficients
                .iter()
                .zip(right.raw.exponents_iter())
            {
                let mut exponents = Vec::with_capacity(self.parameter_count());
                for (variable, (&left, &right)) in
                    left_exponents.iter().zip(right_exponents).enumerate()
                {
                    let requested = u32::from(left).checked_add(u32::from(right)).ok_or(
                        FeynmanPolynomialError::ResourceCountOverflow {
                            resource: "prospective Feynman-parameter exponent",
                        },
                    )?;
                    if requested > u32::from(self.limits.max_parameter_exponent) {
                        return Err(FeynmanPolynomialError::ParameterExponentOverflow {
                            variable,
                            requested,
                            limit: self.limits.max_parameter_exponent,
                        });
                    }
                    exponents.push(requested as u16);
                }
                let coefficient = self.coefficients.try_mul(
                    left_coefficient,
                    right_coefficient,
                    self.limits.exact_algebra,
                )?;
                self.accumulate(&mut terms, exponents, coefficient)?;
            }
        }
        self.from_terms(terms)
    }

    pub(super) fn scale(
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
        let mut terms = BTreeMap::new();
        for (value, exponents) in polynomial
            .raw
            .coefficients
            .iter()
            .zip(polynomial.raw.exponents_iter())
        {
            let value = self
                .coefficients
                .try_mul(value, coefficient, self.limits.exact_algebra)?;
            if !value.is_zero() {
                terms.insert(exponents.to_vec(), value);
            }
        }
        self.from_terms(terms)
    }

    pub(super) fn term_map(
        &self,
        polynomial: &FeynmanPolynomial,
    ) -> BTreeMap<Vec<u16>, Coefficient> {
        polynomial
            .raw
            .coefficients
            .iter()
            .cloned()
            .zip(polynomial.raw.exponents_iter().map(<[u16]>::to_vec))
            .map(|(coefficient, exponents)| (exponents, coefficient))
            .collect()
    }

    pub(super) fn accumulate(
        &self,
        terms: &mut BTreeMap<Vec<u16>, Coefficient>,
        exponents: Vec<u16>,
        coefficient: Coefficient,
    ) -> Result<(), FeynmanPolynomialError> {
        if coefficient.is_zero() {
            return Ok(());
        }
        if let Some(previous) = terms.remove(&exponents) {
            let sum =
                self.coefficients
                    .try_add(&previous, &coefficient, self.limits.exact_algebra)?;
            if !sum.is_zero() {
                terms.insert(exponents, sum);
            }
        } else {
            terms.insert(exponents, coefficient);
        }
        check_limit(
            "Feynman polynomial terms",
            terms.len(),
            self.limits.max_polynomial_terms,
        )
    }

    pub(super) fn from_terms(
        &self,
        terms: BTreeMap<Vec<u16>, Coefficient>,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        check_limit(
            "Feynman polynomial terms",
            terms.len(),
            self.limits.max_polynomial_terms,
        )?;
        self.check_exponent_entries(terms.len(), "Feynman polynomial exponent entries")?;
        let mut raw = self.template.zero_with_capacity(terms.len());
        for (exponents, coefficient) in terms {
            if exponents.len() != self.parameter_count() {
                return Err(FeynmanPolynomialError::MalformedPolynomial {
                    detail: format!(
                        "term has {} exponents, expected {}",
                        exponents.len(),
                        self.parameter_count()
                    ),
                });
            }
            self.coefficients
                .validate_with_limits(&coefficient, self.limits.exact_algebra)?;
            if coefficient.is_zero() {
                continue;
            }
            raw.append_monomial_back(coefficient, &exponents);
        }
        let polynomial = self.wrap(raw);
        self.authenticate(&polynomial)?;
        Ok(polynomial)
    }

    pub(super) fn check_exponent_entries(
        &self,
        terms: usize,
        resource: &'static str,
    ) -> Result<usize, FeynmanPolynomialError> {
        let entries = checked_mul(terms, self.parameter_count(), resource)?;
        check_limit(resource, entries, self.limits.max_exponent_entries)?;
        Ok(entries)
    }

    pub(super) fn wrap(&self, raw: RawFeynmanPolynomial) -> FeynmanPolynomial {
        FeynmanPolynomial {
            raw,
            context: self.family_fingerprint.clone(),
        }
    }

    /// Bind one native Symbolica result back to this context's ordered
    /// variable map.  `PolynomialRing::zero()` deliberately has an empty
    /// variable map, so an identically-zero native result must use the
    /// authenticated template zero instead.
    pub(super) fn rebind_native_result(
        &self,
        raw: RawFeynmanPolynomial,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        if raw.is_zero() {
            return Ok(self.zero());
        }
        let polynomial = self.wrap(raw);
        self.authenticate(&polynomial)?;
        Ok(polynomial)
    }
}
