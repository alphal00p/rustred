//! Generic Symbolica-native Feynman-parameter polynomials.
//!
//! This module is the RustRed counterpart of LiteRed's `FeynParUF`.  It
//! constructs `U`, `F`, and `G = U + F` from an authenticated complete affine
//! [`IntegralFamily`](crate::IntegralFamily).  The implementation contains no
//! loop-count or topology dispatch.

use std::collections::BTreeMap;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::*;

use crate::{
    Coefficient, CoefficientContext, ExactAlgebraError, ExactAlgebraLimits, ExactRational,
    FamilyDomain, IntegralFamily, ScalarProductCoordinate, SectorMask,
};

/// Sparse polynomials in Feynman parameters with coefficients in the
/// authenticated family field `K`.
pub type RawFeynmanPolynomial =
    MultivariatePolynomial<RationalPolynomialField<IntegerRing, u16>, u16>;

/// Checked work and representation budgets for one `U/F/G` construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeynmanPolynomialLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_parameters: usize,
    pub max_parameter_exponent: u16,
    pub max_polynomial_terms: usize,
    /// Maximum dense Feynman-exponent entries retained or constructed by one
    /// polynomial operation.  Symbolica stores one exponent for every
    /// `(term, parameter)` pair even when most exponents are zero.
    pub max_exponent_entries: usize,
    /// Aggregate polynomial-term work for one public construction,
    /// differentiation, or face-restriction call.
    pub max_term_operations: usize,
    pub max_determinant_states: usize,
    pub max_determinant_operations: usize,
    pub max_adjugate_minors: usize,
}

impl Default for FeynmanPolynomialLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_parameters: 4_096,
            max_parameter_exponent: u16::MAX,
            max_polynomial_terms: 4_000_000,
            max_exponent_entries: 64_000_000,
            max_term_operations: 16_000_000,
            max_determinant_states: 1_048_576,
            max_determinant_operations: 16_000_000,
            max_adjugate_minors: 1_048_576,
        }
    }
}

/// Typed failures from checked Feynman-polynomial construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeynmanPolynomialError {
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ParameterExponentOverflow {
        variable: usize,
        requested: u32,
        limit: u16,
    },
    ForeignPolynomialContext,
    MalformedPolynomial {
        detail: String,
    },
    SymbolicaSymbol {
        parameter: usize,
        detail: String,
    },
    FeynmanBaseSymbolCollision {
        parameter: usize,
        base_parameter: String,
    },
    ExactAlgebra(ExactAlgebraError),
    InternalVerificationFailure {
        detail: String,
    },
    SymbolicaPanic,
}

impl fmt::Display for FeynmanPolynomialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ParameterExponentOverflow {
                variable,
                requested,
                limit,
            } => write!(
                formatter,
                "Feynman-parameter {variable} needs exponent {requested}, above limit {limit}"
            ),
            Self::ForeignPolynomialContext => {
                formatter.write_str("Feynman polynomial belongs to a foreign context")
            }
            Self::MalformedPolynomial { detail } => {
                write!(formatter, "malformed Feynman polynomial: {detail}")
            }
            Self::SymbolicaSymbol { parameter, detail } => write!(
                formatter,
                "could not construct Symbolica Feynman parameter {parameter}: {detail}"
            ),
            Self::FeynmanBaseSymbolCollision {
                parameter,
                base_parameter,
            } => write!(
                formatter,
                "Feynman parameter {parameter} aliases base-field parameter {base_parameter:?}"
            ),
            Self::ExactAlgebra(error) => {
                write!(formatter, "exact coefficient algebra failed: {error}")
            }
            Self::InternalVerificationFailure { detail } => {
                write!(formatter, "Feynman-polynomial replay failed: {detail}")
            }
            Self::SymbolicaPanic => formatter
                .write_str("Symbolica panicked while constructing checked Feynman polynomials"),
        }
    }
}

impl std::error::Error for FeynmanPolynomialError {}

impl From<ExactAlgebraError> for FeynmanPolynomialError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

/// Authenticated coefficient and variable map for `K[x_0,...,x_{N-1}]`.
#[derive(Clone, Debug)]
pub struct FeynmanPolynomialContext {
    family_fingerprint: Arc<str>,
    coefficients: CoefficientContext,
    family_domain: FamilyDomain,
    variables: Arc<Vec<PolyVariable>>,
    field: RationalPolynomialField<IntegerRing, u16>,
    template: RawFeynmanPolynomial,
    limits: FeynmanPolynomialLimits,
}

/// Aggregate counters shared by every checked algebra step in one public
/// operation.  A per-primitive preflight is not sufficient for an adjugate:
/// every minor may fit while their sum is prohibitively large.
#[derive(Clone, Copy, Debug)]
struct FeynmanWorkBudget {
    term_operations: usize,
    determinant_operations: usize,
    limits: FeynmanPolynomialLimits,
}

impl FeynmanWorkBudget {
    fn new(limits: FeynmanPolynomialLimits) -> Self {
        Self {
            term_operations: 0,
            determinant_operations: 0,
            limits,
        }
    }

    fn charge_term_operations(&mut self, requested: usize) -> Result<(), FeynmanPolynomialError> {
        self.term_operations = checked_add(
            self.term_operations,
            requested,
            "aggregate Feynman polynomial operations",
        )?;
        check_limit(
            "aggregate Feynman polynomial operations",
            self.term_operations,
            self.limits.max_term_operations,
        )
    }

    fn charge_determinant_operations(
        &mut self,
        requested: usize,
    ) -> Result<(), FeynmanPolynomialError> {
        self.determinant_operations = checked_add(
            self.determinant_operations,
            requested,
            "aggregate determinant operations",
        )?;
        check_limit(
            "aggregate determinant operations",
            self.determinant_operations,
            self.limits.max_determinant_operations,
        )
    }
}

impl FeynmanPolynomialContext {
    fn try_new(
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

    /// Set inactive parameters to zero without compressing the ordered
    /// variable map.  This is the exact face operation used by certificates.
    pub fn try_restrict_face(
        &self,
        polynomial: &FeynmanPolynomial,
        active: &SectorMask,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.authenticate(polynomial)?;
        let mut work = FeynmanWorkBudget::new(self.limits);
        if active.arity() != self.parameter_count() {
            return Err(FeynmanPolynomialError::MalformedPolynomial {
                detail: format!(
                    "face mask has arity {}, expected {}",
                    active.arity(),
                    self.parameter_count()
                ),
            });
        }
        check_limit(
            "Feynman polynomial face terms",
            polynomial.raw.nterms(),
            self.limits.max_term_operations,
        )?;
        work.charge_term_operations(polynomial.raw.nterms())?;
        let mut terms = BTreeMap::new();
        for (coefficient, exponents) in polynomial.terms() {
            if exponents
                .iter()
                .zip(active.active_bits())
                .any(|(&exponent, &is_active)| exponent > 0 && !is_active)
            {
                continue;
            }
            terms.insert(exponents.to_vec(), coefficient.clone());
        }
        self.from_terms(terms)
    }

    fn zero(&self) -> FeynmanPolynomial {
        self.wrap(self.template.zero())
    }

    fn one(&self) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.constant(self.coefficients.one())
    }

    fn constant(
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

    fn parameter_monomial(
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

    fn authenticate(&self, polynomial: &FeynmanPolynomial) -> Result<(), FeynmanPolynomialError> {
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

    fn add(
        &self,
        left: &FeynmanPolynomial,
        right: &FeynmanPolynomial,
        work: &mut FeynmanWorkBudget,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.combine(left, right, false, work)
    }

    fn sub(
        &self,
        left: &FeynmanPolynomial,
        right: &FeynmanPolynomial,
        work: &mut FeynmanWorkBudget,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.combine(left, right, true, work)
    }

    fn combine(
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

    fn mul(
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
                    let requested = u32::from(left) + u32::from(right);
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

    fn scale(
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

    fn neg(
        &self,
        polynomial: &FeynmanPolynomial,
        work: &mut FeynmanWorkBudget,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.scale(polynomial, &self.coefficients.integer(-1), work)
    }

    fn term_map(&self, polynomial: &FeynmanPolynomial) -> BTreeMap<Vec<u16>, Coefficient> {
        polynomial
            .raw
            .coefficients
            .iter()
            .cloned()
            .zip(polynomial.raw.exponents_iter().map(<[u16]>::to_vec))
            .map(|(coefficient, exponents)| (exponents, coefficient))
            .collect()
    }

    fn accumulate(
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

    fn from_terms(
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

    fn check_exponent_entries(
        &self,
        terms: usize,
        resource: &'static str,
    ) -> Result<usize, FeynmanPolynomialError> {
        let entries = checked_mul(terms, self.parameter_count(), resource)?;
        check_limit(resource, entries, self.limits.max_exponent_entries)?;
        Ok(entries)
    }

    fn wrap(&self, raw: RawFeynmanPolynomial) -> FeynmanPolynomial {
        FeynmanPolynomial {
            raw,
            context: self.family_fingerprint.clone(),
        }
    }
}

/// One polynomial authenticated as a member of a specific family's `K[x]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeynmanPolynomial {
    raw: RawFeynmanPolynomial,
    context: Arc<str>,
}

impl FeynmanPolynomial {
    pub fn raw(&self) -> &RawFeynmanPolynomial {
        &self.raw
    }

    pub fn is_zero(&self) -> bool {
        self.raw.is_zero()
    }

    pub fn term_count(&self) -> usize {
        self.raw.nterms()
    }

    pub fn terms(&self) -> impl Iterator<Item = (&Coefficient, &[u16])> {
        self.raw.coefficients.iter().zip(self.raw.exponents_iter())
    }

    pub fn coefficient(&self, exponents: &[u16]) -> Option<&Coefficient> {
        self.raw
            .exponents_iter()
            .position(|candidate| candidate == exponents)
            .map(|term| &self.raw.coefficients[term])
    }

    pub fn stable_string(&self) -> String {
        let mut output = format!("rustred-feynman-polynomial-v1|N={}", self.raw.nvars());
        for (coefficient, exponents) in self.terms() {
            let coefficient = coefficient.to_expression().to_canonical_string();
            output.push('|');
            output.push_str(
                &exponents
                    .iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
            output.push('=');
            output.push_str(&coefficient.len().to_string());
            output.push(':');
            output.push_str(&coefficient);
        }
        output
    }
}

/// Authenticated generic Symanzik data for one complete affine family.
#[derive(Clone, Debug)]
pub struct SymanzikPolynomials {
    context: FeynmanPolynomialContext,
    u: FeynmanPolynomial,
    f: FeynmanPolynomial,
    g: FeynmanPolynomial,
}

impl SymanzikPolynomials {
    pub fn try_from_family(family: &IntegralFamily) -> Result<Self, FeynmanPolynomialError> {
        Self::try_from_family_with_limits(family, FeynmanPolynomialLimits::default())
    }

    pub fn try_from_family_with_limits(
        family: &IntegralFamily,
        limits: FeynmanPolynomialLimits,
    ) -> Result<Self, FeynmanPolynomialError> {
        catch_unwind(AssertUnwindSafe(|| Self::build(family, limits)))
            .map_err(|_| FeynmanPolynomialError::SymbolicaPanic)?
    }

    fn build(
        family: &IntegralFamily,
        limits: FeynmanPolynomialLimits,
    ) -> Result<Self, FeynmanPolynomialError> {
        let context = FeynmanPolynomialContext::try_new(family, limits)?;
        let mut work = FeynmanWorkBudget::new(limits);
        let loops = family.loop_count();
        let externals = family.external_count();
        let assembly_columns = checked_add(
            family.denominator_count(),
            1,
            "Feynman polynomial assembly entries",
        )?;
        let assembly_entries = checked_mul(
            family.denominator_count(),
            assembly_columns,
            "Feynman polynomial assembly entries",
        )?;
        work.charge_term_operations(assembly_entries)?;
        let mut a = vec![vec![context.zero(); loops]; loops];
        let mut q = vec![vec![context.zero(); externals]; loops];
        let mut c = context.zero();
        let half = context.coefficients.rational(ExactRational::new(1, 2));

        for (denominator_index, denominator) in family.denominators().iter().enumerate() {
            let constant = context.parameter_monomial(denominator_index, denominator.constant())?;
            c = context.add(&c, &constant, &mut work)?;
            for (coordinate_index, coordinate) in family.coordinates().iter().enumerate() {
                let coefficient = &denominator.coefficients()[coordinate_index];
                if coefficient.is_zero() {
                    continue;
                }
                match *coordinate {
                    ScalarProductCoordinate::LoopLoop { left, right } => {
                        let coefficient = if left == right {
                            coefficient.clone()
                        } else {
                            context.coefficients.try_mul(
                                coefficient,
                                &half,
                                limits.exact_algebra,
                            )?
                        };
                        let monomial =
                            context.parameter_monomial(denominator_index, &coefficient)?;
                        a[left][right] = context.add(&a[left][right], &monomial, &mut work)?;
                        if left != right {
                            a[right][left] = context.add(&a[right][left], &monomial, &mut work)?;
                        }
                    }
                    ScalarProductCoordinate::LoopExternal {
                        loop_index,
                        external_index,
                    } => {
                        let coefficient = context.coefficients.try_mul(
                            coefficient,
                            &half,
                            limits.exact_algebra,
                        )?;
                        let monomial =
                            context.parameter_monomial(denominator_index, &coefficient)?;
                        q[loop_index][external_index] =
                            context.add(&q[loop_index][external_index], &monomial, &mut work)?;
                    }
                }
            }
        }

        let u = checked_determinant(&context, &a, &mut work)?;
        if u.is_zero() {
            let f = context.zero();
            let g = context.zero();
            return Ok(Self { context, u, f, g });
        }

        let adjugate = checked_adjugate(&context, &a, &mut work)?;
        let mut momentum_square = context.zero();
        let loop_external_entries =
            checked_mul(loops, externals, "Feynman Gram-contraction entries")?;
        let gram_contraction_entries = checked_mul(
            loop_external_entries,
            loop_external_entries,
            "Feynman Gram-contraction entries",
        )?;
        work.charge_term_operations(gram_contraction_entries)?;
        for loop_left in 0..loops {
            for loop_right in 0..loops {
                for external_left in 0..externals {
                    for external_right in 0..externals {
                        let gram = &family.external_gram()[external_left][external_right];
                        if gram.is_zero()
                            || q[loop_left][external_left].is_zero()
                            || adjugate[loop_left][loop_right].is_zero()
                            || q[loop_right][external_right].is_zero()
                        {
                            continue;
                        }
                        let product = context.mul(
                            &q[loop_left][external_left],
                            &adjugate[loop_left][loop_right],
                            &mut work,
                        )?;
                        let product =
                            context.mul(&product, &q[loop_right][external_right], &mut work)?;
                        let product = context.scale(&product, gram, &mut work)?;
                        momentum_square = context.add(&momentum_square, &product, &mut work)?;
                    }
                }
            }
        }
        let uc = context.mul(&u, &c, &mut work)?;
        let f = context.sub(&uc, &momentum_square, &mut work)?;
        let g = context.add(&u, &f, &mut work)?;
        verify_homogeneous(&u, loops, "U")?;
        verify_homogeneous(&f, loops + 1, "F")?;
        context.authenticate(&g)?;
        Ok(Self { context, u, f, g })
    }

    pub fn context(&self) -> &FeynmanPolynomialContext {
        &self.context
    }

    pub fn family_domain(&self) -> &FamilyDomain {
        self.context.family_domain()
    }

    pub fn u(&self) -> &FeynmanPolynomial {
        &self.u
    }

    pub fn f(&self) -> &FeynmanPolynomial {
        &self.f
    }

    pub fn g(&self) -> &FeynmanPolynomial {
        &self.g
    }

    /// Checked gradient of `G`, corresponding to LiteRed's cached
    /// `FeynParGdG` data.
    pub fn try_gradient(&self) -> Result<Vec<FeynmanPolynomial>, FeynmanPolynomialError> {
        self.context.try_gradient(&self.g)
    }

    /// Checked restriction of any polynomial from this family context to a
    /// sector face.  The result retains all parameter variables in family
    /// order.
    pub fn try_restrict_face(
        &self,
        polynomial: &FeynmanPolynomial,
        active: &SectorMask,
    ) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
        self.context.try_restrict_face(polynomial, active)
    }
}

fn checked_determinant(
    context: &FeynmanPolynomialContext,
    matrix: &[Vec<FeynmanPolynomial>],
    work: &mut FeynmanWorkBudget,
) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
    let size = matrix.len();
    if matrix.iter().any(|row| row.len() != size) {
        return Err(FeynmanPolynomialError::InternalVerificationFailure {
            detail: "determinant received a non-square matrix".to_owned(),
        });
    }
    if size == 0 {
        return context.one();
    }
    if size >= usize::BITS as usize {
        return Err(FeynmanPolynomialError::ResourceCountOverflow {
            resource: "determinant subset states",
        });
    }
    let states =
        1_usize
            .checked_shl(size as u32)
            .ok_or(FeynmanPolynomialError::ResourceCountOverflow {
                resource: "determinant subset states",
            })?;
    check_limit(
        "determinant subset states",
        states,
        context.limits.max_determinant_states,
    )?;
    let operations =
        size.checked_mul(states / 2)
            .ok_or(FeynmanPolynomialError::ResourceCountOverflow {
                resource: "determinant operations",
            })?;
    check_limit(
        "determinant operations",
        operations,
        context.limits.max_determinant_operations,
    )?;
    work.charge_determinant_operations(operations)?;
    let mut dp = vec![context.zero(); states];
    dp[0] = context.one()?;
    for mask in 0..states {
        let row = mask.count_ones() as usize;
        if row == size || dp[mask].is_zero() {
            continue;
        }
        for column in 0..size {
            let bit = 1_usize << column;
            if mask & bit != 0 || matrix[row][column].is_zero() {
                continue;
            }
            let mut contribution = context.mul(&dp[mask], &matrix[row][column], work)?;
            let greater = (mask >> (column + 1)).count_ones();
            if greater % 2 == 1 {
                contribution = context.neg(&contribution, work)?;
            }
            let next = mask | bit;
            dp[next] = context.add(&dp[next], &contribution, work)?;
        }
    }
    dp.into_iter()
        .last()
        .ok_or_else(|| FeynmanPolynomialError::InternalVerificationFailure {
            detail: "determinant subset table was unexpectedly empty".to_owned(),
        })
}

fn checked_adjugate(
    context: &FeynmanPolynomialContext,
    matrix: &[Vec<FeynmanPolynomial>],
    work: &mut FeynmanWorkBudget,
) -> Result<Vec<Vec<FeynmanPolynomial>>, FeynmanPolynomialError> {
    let size = matrix.len();
    let minors = size
        .checked_mul(size)
        .ok_or(FeynmanPolynomialError::ResourceCountOverflow {
            resource: "adjugate minors",
        })?;
    check_limit(
        "adjugate minors",
        minors,
        context.limits.max_adjugate_minors,
    )?;
    let mut adjugate = vec![vec![context.zero(); size]; size];
    for row in 0..size {
        for column in 0..size {
            // adj(A)[row,column] is the cofactor with row `column` and
            // column `row` deleted.
            let minor = matrix
                .iter()
                .enumerate()
                .filter(|(candidate, _)| *candidate != column)
                .map(|(_, values)| {
                    values
                        .iter()
                        .enumerate()
                        .filter(|(candidate, _)| *candidate != row)
                        .map(|(_, value)| value.clone())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut cofactor = checked_determinant(context, &minor, work)?;
            if (row + column) % 2 == 1 {
                cofactor = context.neg(&cofactor, work)?;
            }
            adjugate[row][column] = cofactor;
        }
    }
    Ok(adjugate)
}

fn verify_homogeneous(
    polynomial: &FeynmanPolynomial,
    expected: usize,
    name: &'static str,
) -> Result<(), FeynmanPolynomialError> {
    for (_, exponents) in polynomial.terms() {
        let degree = exponents.iter().try_fold(0_usize, |total, &exponent| {
            total.checked_add(usize::from(exponent))
        });
        if degree != Some(expected) {
            return Err(FeynmanPolynomialError::InternalVerificationFailure {
                detail: format!("{name} has a monomial of degree {degree:?}, expected {expected}"),
            });
        }
    }
    Ok(())
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FeynmanPolynomialError> {
    if requested > limit {
        Err(FeynmanPolynomialError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, FeynmanPolynomialError> {
    left.checked_add(right)
        .ok_or(FeynmanPolynomialError::ResourceCountOverflow { resource })
}

fn checked_mul(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, FeynmanPolynomialError> {
    left.checked_mul(right)
        .ok_or(FeynmanPolynomialError::ResourceCountOverflow { resource })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AffineDenominator;

    #[test]
    fn adjugate_uses_transposed_cofactor_indices() {
        let coefficients = CoefficientContext::new(["d"]);
        let family = IntegralFamily::new(
            "feynman-private-adjugate-indexing",
            vec!["k".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                coefficients.zero(),
                vec![coefficients.one()],
            )],
            Vec::new(),
            vec![coefficients.zero()],
        )
        .unwrap();
        let limits = FeynmanPolynomialLimits::default();
        let context = FeynmanPolynomialContext::try_new(&family, limits).unwrap();
        let entry = |value| context.constant(coefficients.integer(value)).unwrap();
        let matrix = vec![vec![entry(1), entry(2)], vec![entry(3), entry(4)]];
        let mut work = FeynmanWorkBudget::new(limits);
        let adjugate = checked_adjugate(&context, &matrix, &mut work).unwrap();

        assert_eq!(adjugate[0][0], entry(4));
        assert_eq!(adjugate[0][1], entry(-2));
        assert_eq!(adjugate[1][0], entry(-3));
        assert_eq!(adjugate[1][1], entry(1));
    }
}
