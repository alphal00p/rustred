//! Generic Symbolica-native Feynman-parameter polynomials.
//!
//! This module is the RustRed counterpart of LiteRed's `FeynParUF`.  It
//! constructs `U`, `F`, and `G = U + F` from an authenticated complete affine
//! [`IntegralFamily`].  The implementation contains no
//! loop-count or topology dispatch.

use std::collections::BTreeMap;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::*;

use crate::{
    FamilyDomain, IntegralFamily, ScalarProductCoordinate, algebra::Coefficient,
    algebra::CoefficientContext, algebra::ExactAlgebraError, algebra::ExactAlgebraLimits,
};

/// Sparse polynomials in Feynman parameters with coefficients in the
/// authenticated family field `K`.
pub type RawFeynmanPolynomial =
    MultivariatePolynomial<RationalPolynomialField<IntegerRing, u16>, u16>;

/// Symbolica's native polynomial-ring adapter for the natural `K[x]` domain.
type FeynmanPolynomialRing = PolynomialRing<RationalPolynomialField<IntegerRing, u16>, u16>;

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
    /// Aggregate RustRed-observable polynomial-term work for one public
    /// construction, differentiation, or face-restriction call.  Symbolica's
    /// native determinant does not expose its intermediate term census; its
    /// structural ring calls are bounded separately below and its retained
    /// result is authenticated against the polynomial representation limits.
    pub max_term_operations: usize,
    /// Maximum structural entries in one square matrix handed to Symbolica's
    /// native determinant implementation.  This is not an RSS bound: campaign
    /// admission must separately charge the resident caller input, RustRed's
    /// input clone, Symbolica's full Bareiss matrix clone for sizes at least
    /// four, intermediate polynomial/coefficient swell, exact-division and GCD
    /// temporaries, allocator/TLS scratch, and any adjugate-minor clones.
    pub max_determinant_matrix_entries: usize,
    /// Aggregate conservative count of structural arithmetic ring calls made
    /// by Symbolica determinants. Sizes two and three use the exact native
    /// formulas; larger sizes use the public fraction-free Bareiss structure.
    /// Pivot zero probes are excluded, and one counted polynomial operation can
    /// own substantial opaque native algebra and memory.
    pub max_determinant_ring_operations: usize,
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
            max_determinant_matrix_entries: 1_048_576,
            max_determinant_ring_operations: 16_000_000,
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
    /// The RustRed-owned outer allocation named by `resource` failed. Native
    /// Symbolica clones and arithmetic temporaries remain opaque to this error.
    AllocationFailure {
        resource: &'static str,
        requested: usize,
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
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} units for {resource}"
            ),
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
    determinant_ring_operations: usize,
    limits: FeynmanPolynomialLimits,
}

impl FeynmanWorkBudget {
    fn new(limits: FeynmanPolynomialLimits) -> Self {
        Self {
            term_operations: 0,
            determinant_ring_operations: 0,
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

    fn charge_determinant_ring_operations(
        &mut self,
        requested: usize,
    ) -> Result<(), FeynmanPolynomialError> {
        self.determinant_ring_operations = checked_add(
            self.determinant_ring_operations,
            requested,
            "aggregate Symbolica determinant ring operations",
        )?;
        check_limit(
            "aggregate Symbolica determinant ring operations",
            self.determinant_ring_operations,
            self.limits.max_determinant_ring_operations,
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

    /// Bind one native Symbolica result back to this context's ordered
    /// variable map.  `PolynomialRing::zero()` deliberately has an empty
    /// variable map, so an identically-zero native result must use the
    /// authenticated template zero instead.
    fn rebind_native_result(
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
        let half = context.coefficients.try_div(
            &context.coefficients.one(),
            &context.coefficients.integer(2),
            limits.exact_algebra,
        )?;

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

        let momentum_square = if externals == 0 {
            context.zero()
        } else {
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
            momentum_square
        };
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

    let matrix_entries = checked_mul(size, size, "Symbolica determinant matrix entries")?;
    check_limit(
        "Symbolica determinant matrix entries",
        matrix_entries,
        context.limits.max_determinant_matrix_entries,
    )?;
    work.charge_determinant_ring_operations(determinant_ring_operations(size)?)?;

    let native_size =
        u32::try_from(size).map_err(|_| FeynmanPolynomialError::ResourceCountOverflow {
            resource: "Symbolica determinant matrix dimension",
        })?;
    let native_matrix_entries = native_size.checked_mul(native_size).ok_or(
        FeynmanPolynomialError::ResourceCountOverflow {
            resource: "Symbolica determinant u32 matrix entries",
        },
    )?;
    if native_matrix_entries as usize != matrix_entries {
        return Err(FeynmanPolynomialError::InternalVerificationFailure {
            detail: "Symbolica determinant matrix dimensions failed checked replay".to_owned(),
        });
    }
    let mut entries = Vec::new();
    entries.try_reserve_exact(matrix_entries).map_err(|_| {
        FeynmanPolynomialError::AllocationFailure {
            resource: "Symbolica determinant input entries",
            requested: matrix_entries,
        }
    })?;
    for row in matrix {
        for entry in row {
            context.authenticate(entry)?;
            entries.push(entry.raw.clone());
        }
    }
    let ring = FeynmanPolynomialRing::from_poly(&context.template);
    let native = Matrix::from_linear(entries, native_size, native_size, ring).map_err(|_| {
        FeynmanPolynomialError::InternalVerificationFailure {
            detail: "Symbolica rejected a preflighted determinant matrix".to_owned(),
        }
    })?;
    let raw = catch_unwind(AssertUnwindSafe(|| native.det()))
        .map_err(|_| FeynmanPolynomialError::SymbolicaPanic)?
        .map_err(
            |error| FeynmanPolynomialError::InternalVerificationFailure {
                detail: native_determinant_error_detail(error).to_owned(),
            },
        )?;
    context.rebind_native_result(raw)
}

fn native_determinant_error_detail(
    error: symbolica::tensors::matrix::MatrixError<FeynmanPolynomialRing>,
) -> &'static str {
    use symbolica::tensors::matrix::MatrixError;

    match error {
        MatrixError::Underdetermined { .. } => {
            "Symbolica Matrix::det unexpectedly reported an underdetermined matrix"
        }
        MatrixError::Inconsistent => {
            "Symbolica Matrix::det unexpectedly reported an inconsistent matrix"
        }
        MatrixError::NotSquare => "Symbolica Matrix::det rejected a preflighted square K[x] matrix",
        MatrixError::Singular => {
            "Symbolica Matrix::det unexpectedly rejected a nonempty singular K[x] matrix"
        }
        MatrixError::ShapeMismatch => {
            "Symbolica Matrix::det unexpectedly reported a shape mismatch"
        }
        MatrixError::RightHandSideIsNotVector => {
            "Symbolica Matrix::det unexpectedly requested a vector right-hand side"
        }
        MatrixError::ResultNotInDomain => "Symbolica Matrix::det produced a result outside K[x]",
    }
}

/// Count the native determinant's structural ring operations without doing
/// any algebra in RustRed.  Symbolica uses direct formulas for sizes at most
/// three and fraction-free Bareiss elimination above that threshold.  Four
/// operations per trailing entry conservatively includes the first Bareiss
/// step, where Symbolica omits the exact division.
fn determinant_ring_operations(size: usize) -> Result<usize, FeynmanPolynomialError> {
    match size {
        0 | 1 => Ok(0),
        2 => Ok(3),
        3 => Ok(14),
        _ => {
            let mut sum_of_squares = 0_usize;
            for trailing_size in 1..size {
                let square = checked_mul(
                    trailing_size,
                    trailing_size,
                    "Symbolica Bareiss determinant ring operations",
                )?;
                sum_of_squares = checked_add(
                    sum_of_squares,
                    square,
                    "Symbolica Bareiss determinant ring operations",
                )?;
            }
            checked_mul(
                4,
                sum_of_squares,
                "Symbolica Bareiss determinant ring operations",
            )
        }
    }
}

fn checked_symbolica_neg(
    context: &FeynmanPolynomialContext,
    polynomial: &FeynmanPolynomial,
    work: &mut FeynmanWorkBudget,
) -> Result<FeynmanPolynomial, FeynmanPolynomialError> {
    context.authenticate(polynomial)?;
    work.charge_term_operations(polynomial.raw.nterms())?;
    let ring = FeynmanPolynomialRing::from_poly(&context.template);
    let raw = catch_unwind(AssertUnwindSafe(|| ring.neg(polynomial.raw())))
        .map_err(|_| FeynmanPolynomialError::SymbolicaPanic)?;
    context.rebind_native_result(raw)
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
                cofactor = checked_symbolica_neg(context, &cofactor, work)?;
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

    fn matrix_family(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "s"]);
        let denominators = (0..5)
            .map(|coordinate| {
                AffineDenominator::new(
                    coefficients.zero(),
                    (0..5)
                        .map(|candidate| {
                            if candidate == coordinate {
                                coefficients.one()
                            } else {
                                coefficients.zero()
                            }
                        })
                        .collect(),
                )
            })
            .collect();
        IntegralFamily::new(
            name,
            vec!["k0".into(), "k1".into()],
            vec!["p".into()],
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            denominators,
            vec![vec![coefficients.parameter("s").unwrap()]],
            vec![coefficients.zero(); 5],
        )
        .unwrap()
    }

    fn matrix_context(name: &str, limits: FeynmanPolynomialLimits) -> FeynmanPolynomialContext {
        FeynmanPolynomialContext::try_new(&matrix_family(name), limits).unwrap()
    }

    fn variable(context: &FeynmanPolynomialContext, parameter: usize) -> FeynmanPolynomial {
        context
            .parameter_monomial(parameter, &context.coefficients.one())
            .unwrap()
    }

    fn integer(context: &FeynmanPolynomialContext, value: i64) -> FeynmanPolynomial {
        context
            .constant(context.coefficients.integer(value))
            .unwrap()
    }

    fn symbolic_tridiagonal_four(
        context: &FeynmanPolynomialContext,
    ) -> Vec<Vec<FeynmanPolynomial>> {
        let zero = context.zero();
        let one = integer(context, 1);
        let x0 = variable(context, 0);
        let x1 = variable(context, 1);
        let x2 = variable(context, 2);
        let x3 = variable(context, 3);
        vec![
            vec![x0, one.clone(), zero.clone(), zero.clone()],
            vec![one.clone(), x1, one.clone(), zero.clone()],
            vec![zero.clone(), one.clone(), x2, one.clone()],
            vec![zero.clone(), zero, one, x3],
        ]
    }

    fn native_matrix(
        context: &FeynmanPolynomialContext,
        matrix: &[Vec<FeynmanPolynomial>],
    ) -> Matrix<FeynmanPolynomialRing> {
        Matrix::from_nested_vec(
            matrix
                .iter()
                .map(|row| row.iter().map(|entry| entry.raw.clone()).collect())
                .collect(),
            FeynmanPolynomialRing::from_poly(&context.template),
        )
        .unwrap()
    }

    #[test]
    fn empty_determinant_is_the_authenticated_multiplicative_identity() {
        let limits = FeynmanPolynomialLimits::default();
        let context = matrix_context("feynman-native-empty-determinant", limits);
        let mut work = FeynmanWorkBudget::new(limits);
        let determinant = checked_determinant(&context, &[], &mut work).unwrap();

        assert_eq!(determinant, integer(&context, 1));
        context.authenticate(&determinant).unwrap();
        assert_eq!(work.determinant_ring_operations, 0);
    }

    #[test]
    fn native_small_determinants_have_exact_structural_call_counts() {
        let limits = FeynmanPolynomialLimits::default();
        let context = matrix_context("feynman-native-small-counts", limits);
        let zero = context.zero();

        let two = vec![
            vec![variable(&context, 0), zero.clone()],
            vec![zero.clone(), variable(&context, 1)],
        ];
        let mut work = FeynmanWorkBudget::new(limits);
        checked_determinant(&context, &two, &mut work).unwrap();
        assert_eq!(work.determinant_ring_operations, 3);

        let three = vec![
            vec![variable(&context, 0), zero.clone(), zero.clone()],
            vec![zero.clone(), variable(&context, 1), zero.clone()],
            vec![zero.clone(), zero, variable(&context, 2)],
        ];
        let mut work = FeynmanWorkBudget::new(limits);
        checked_determinant(&context, &three, &mut work).unwrap();
        assert_eq!(work.determinant_ring_operations, 14);
    }

    #[test]
    fn ragged_determinant_is_rejected_before_native_construction() {
        let limits = FeynmanPolynomialLimits::default();
        let context = matrix_context("feynman-native-ragged", limits);
        let matrix = vec![
            vec![variable(&context, 0), variable(&context, 1)],
            vec![variable(&context, 2)],
        ];
        let mut work = FeynmanWorkBudget::new(limits);

        assert!(matches!(
            checked_determinant(&context, &matrix, &mut work),
            Err(FeynmanPolynomialError::InternalVerificationFailure { detail })
                if detail == "determinant received a non-square matrix"
        ));
        assert_eq!(work.determinant_ring_operations, 0);
    }

    #[test]
    fn symbolica_four_by_four_determinant_retains_symbolic_terms() {
        let limits = FeynmanPolynomialLimits::default();
        let context = matrix_context("feynman-native-symbolic-four", limits);
        let matrix = symbolic_tridiagonal_four(&context);
        let mut work = FeynmanWorkBudget::new(limits);
        let determinant = checked_determinant(&context, &matrix, &mut work).unwrap();
        let one = context.coefficients.one();
        let minus_one = context.coefficients.integer(-1);

        // det = x0*x1*x2*x3 - x2*x3 - x0*x3 - x0*x1 + 1.
        assert_eq!(determinant.term_count(), 5);
        assert_eq!(determinant.coefficient(&[1, 1, 1, 1, 0]), Some(&one));
        assert_eq!(determinant.coefficient(&[0, 0, 1, 1, 0]), Some(&minus_one));
        assert_eq!(determinant.coefficient(&[1, 0, 0, 1, 0]), Some(&minus_one));
        assert_eq!(determinant.coefficient(&[1, 1, 0, 0, 0]), Some(&minus_one));
        assert_eq!(determinant.coefficient(&[0, 0, 0, 0, 0]), Some(&one));
        assert_eq!(work.determinant_ring_operations, 56);
    }

    #[test]
    fn singular_native_four_by_four_zero_is_rebound_to_the_context_variable_map() {
        let limits = FeynmanPolynomialLimits::default();
        let context = matrix_context("feynman-native-singular-four", limits);
        let zero = context.zero();
        let matrix = vec![
            vec![
                zero.clone(),
                variable(&context, 0),
                zero.clone(),
                zero.clone(),
            ],
            vec![
                zero.clone(),
                zero.clone(),
                variable(&context, 1),
                zero.clone(),
            ],
            vec![
                zero.clone(),
                zero.clone(),
                zero.clone(),
                variable(&context, 2),
            ],
            vec![zero.clone(), zero.clone(), zero, variable(&context, 3)],
        ];
        let mut work = FeynmanWorkBudget::new(limits);
        let determinant = checked_determinant(&context, &matrix, &mut work).unwrap();

        assert!(determinant.is_zero());
        assert_eq!(determinant.raw.variables, context.variables);
        context.authenticate(&determinant).unwrap();
    }

    #[test]
    fn native_bareiss_row_swap_has_the_correct_sign() {
        let limits = FeynmanPolynomialLimits::default();
        let context = matrix_context("feynman-native-row-swap", limits);
        let zero = context.zero();
        let matrix = vec![
            vec![
                zero.clone(),
                variable(&context, 0),
                zero.clone(),
                zero.clone(),
            ],
            vec![
                variable(&context, 1),
                zero.clone(),
                zero.clone(),
                zero.clone(),
            ],
            vec![
                zero.clone(),
                zero.clone(),
                variable(&context, 2),
                zero.clone(),
            ],
            vec![zero.clone(), zero.clone(), zero, variable(&context, 3)],
        ];
        let mut work = FeynmanWorkBudget::new(limits);
        let determinant = checked_determinant(&context, &matrix, &mut work).unwrap();

        assert_eq!(determinant.term_count(), 1);
        assert_eq!(
            determinant.coefficient(&[1, 1, 1, 1, 0]),
            Some(&context.coefficients.integer(-1))
        );
    }

    #[test]
    fn native_constant_four_by_four_retains_the_authenticated_variable_map() {
        let limits = FeynmanPolynomialLimits::default();
        let context = matrix_context("feynman-native-constant-four", limits);
        let zero = context.zero();
        let matrix = vec![
            vec![
                integer(&context, 1),
                zero.clone(),
                zero.clone(),
                zero.clone(),
            ],
            vec![
                zero.clone(),
                integer(&context, 2),
                zero.clone(),
                zero.clone(),
            ],
            vec![
                zero.clone(),
                zero.clone(),
                integer(&context, 3),
                zero.clone(),
            ],
            vec![zero.clone(), zero.clone(), zero, integer(&context, 4)],
        ];
        let mut work = FeynmanWorkBudget::new(limits);
        let determinant = checked_determinant(&context, &matrix, &mut work).unwrap();

        assert_eq!(determinant, integer(&context, 24));
        assert_eq!(determinant.raw.variables, context.variables);
        context.authenticate(&determinant).unwrap();
    }

    #[test]
    fn one_by_one_adjugate_uses_the_empty_native_cofactor() {
        let limits = FeynmanPolynomialLimits::default();
        let context = matrix_context("feynman-native-one-adjugate", limits);
        let matrix = vec![vec![variable(&context, 0)]];
        let mut work = FeynmanWorkBudget::new(limits);
        let adjugate = checked_adjugate(&context, &matrix, &mut work).unwrap();

        assert_eq!(adjugate, vec![vec![integer(&context, 1)]]);
        assert_eq!(work.determinant_ring_operations, 0);
    }

    #[test]
    fn asymmetric_adjugate_replays_a_times_adjugate_with_symbolica_matrix_multiplication() {
        let limits = FeynmanPolynomialLimits::default();
        let context = matrix_context("feynman-native-asymmetric-adjugate", limits);
        let zero = context.zero();
        let one = integer(&context, 1);
        let matrix = vec![
            vec![variable(&context, 0), one.clone(), zero.clone()],
            vec![zero.clone(), variable(&context, 1), one.clone()],
            vec![one, zero, variable(&context, 2)],
        ];
        let mut work = FeynmanWorkBudget::new(limits);
        let determinant = checked_determinant(&context, &matrix, &mut work).unwrap();
        let adjugate = checked_adjugate(&context, &matrix, &mut work).unwrap();
        assert_eq!(work.determinant_ring_operations, 14 + 9 * 3);

        // Matrix multiplication, including every polynomial product and sum,
        // is performed by Symbolica's public K[x] matrix/ring API.
        let product = &native_matrix(&context, &matrix) * &native_matrix(&context, &adjugate);
        for row in 0..3_u32 {
            for column in 0..3_u32 {
                if row == column {
                    assert_eq!(&product[(row, column)], determinant.raw());
                } else {
                    assert!(product[(row, column)].is_zero());
                }
            }
        }
    }

    #[test]
    fn native_four_by_four_resource_preflight_has_exact_boundaries() {
        let below_operations = FeynmanPolynomialLimits {
            max_determinant_ring_operations: 55,
            ..FeynmanPolynomialLimits::default()
        };
        let context = matrix_context("feynman-native-four-below-operations", below_operations);
        let matrix = symbolic_tridiagonal_four(&context);
        let mut work = FeynmanWorkBudget::new(below_operations);
        assert!(matches!(
            checked_determinant(&context, &matrix, &mut work),
            Err(FeynmanPolynomialError::ResourceLimit {
                resource: "aggregate Symbolica determinant ring operations",
                requested: 56,
                limit: 55,
            })
        ));

        let exact = FeynmanPolynomialLimits {
            max_determinant_matrix_entries: 16,
            max_determinant_ring_operations: 56,
            ..FeynmanPolynomialLimits::default()
        };
        let context = matrix_context("feynman-native-four-exact", exact);
        let matrix = symbolic_tridiagonal_four(&context);
        let mut work = FeynmanWorkBudget::new(exact);
        checked_determinant(&context, &matrix, &mut work).unwrap();
        assert_eq!(work.determinant_ring_operations, 56);

        let below_entries = FeynmanPolynomialLimits {
            max_determinant_matrix_entries: 15,
            max_determinant_ring_operations: 56,
            ..FeynmanPolynomialLimits::default()
        };
        let context = matrix_context("feynman-native-four-below-entries", below_entries);
        let matrix = symbolic_tridiagonal_four(&context);
        let mut work = FeynmanWorkBudget::new(below_entries);
        assert!(matches!(
            checked_determinant(&context, &matrix, &mut work),
            Err(FeynmanPolynomialError::ResourceLimit {
                resource: "Symbolica determinant matrix entries",
                requested: 16,
                limit: 15,
            })
        ));
    }

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
