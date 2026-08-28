use std::{cmp::Ordering, fmt, mem::size_of, sync::Arc};

use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::prelude::*;

const RUSTRED_NAMESPACE: &str = "rustred";

/// Exact rational functions in the kinematic parameters.
pub type Coefficient = RationalPolynomial<IntegerRing, u16>;

/// Largest exponent representable by RustRed's Symbolica coefficient domain.
///
/// Symbolica's polynomial arithmetic panics when an operation would overflow
/// its exponent type.  Analytic reducers use this ceiling to preflight their
/// caller-controlled formula degrees before constructing coefficients.
pub const SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT: u16 = u16::MAX;

/// Resource limits for exact rational-polynomial arithmetic.
///
/// These limits are checked before entering Symbolica operations that add
/// polynomial exponents.  This is essential because Symbolica deliberately
/// panics when its `u16` exponent representation overflows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactAlgebraLimits {
    /// Largest exponent admitted by RustRed's checked representation boundary.
    pub max_exponent: u16,
    /// Largest authenticated retained sparse part.
    ///
    /// A conservative native-output envelope is a separate operation-local
    /// limit.  In particular, direct polynomial multiplication may have a
    /// support envelope larger than its actual canonical result.
    pub max_polynomial_terms: usize,
    /// Sparse input-pair/sum admission bound for one checked operation.
    ///
    /// This is not a complete bound on Symbolica's internal GCD, quotient, or
    /// dense-multiplication scratch work. The vendored polynomial multiplier
    /// may scan a dense degree box (internally capped at `2^24` slots) even
    /// when the sparse Cartesian input has few pairs.
    pub max_term_operations: usize,
}

impl Default for ExactAlgebraLimits {
    fn default() -> Self {
        Self {
            max_exponent: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            max_polynomial_terms: 4_000_000,
            max_term_operations: 16_000_000,
        }
    }
}

/// One checked rational-polynomial operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExactAlgebraOperation {
    Authenticate,
    Add,
    Subtract,
    Multiply,
    Divide,
    Negate,
    Power,
}

impl fmt::Display for ExactAlgebraOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authenticate => formatter.write_str("authenticate"),
            Self::Add => formatter.write_str("add"),
            Self::Subtract => formatter.write_str("subtract"),
            Self::Multiply => formatter.write_str("multiply"),
            Self::Divide => formatter.write_str("divide"),
            Self::Negate => formatter.write_str("negate"),
            Self::Power => formatter.write_str("power"),
        }
    }
}

/// Typed failures produced before panic-prone Symbolica arithmetic is called.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExactAlgebraError {
    VariableMapMismatch {
        part: CoefficientPolynomialPart,
    },
    MalformedExponentLayout {
        part: CoefficientPolynomialPart,
        coefficients: usize,
        exponents: usize,
        variables: usize,
    },
    ZeroCoefficient {
        part: CoefficientPolynomialPart,
        term: usize,
    },
    NonCanonicalMonomialOrder {
        part: CoefficientPolynomialPart,
        term: usize,
    },
    ZeroDenominator,
    DivisionByZero,
    ExponentLimit {
        operation: ExactAlgebraOperation,
        variable: usize,
        requested: u64,
        limit: u16,
    },
    ExponentArithmeticOverflow {
        operation: ExactAlgebraOperation,
        variable: usize,
        width: u8,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
}

impl fmt::Display for ExactAlgebraError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VariableMapMismatch { part } => {
                write!(formatter, "coefficient {part} uses a foreign variable map")
            }
            Self::MalformedExponentLayout {
                part,
                coefficients,
                exponents,
                variables,
            } => write!(
                formatter,
                "coefficient {part} has {coefficients} terms, {exponents} exponents, and {variables} variables"
            ),
            Self::ZeroCoefficient { part, term } => write!(
                formatter,
                "coefficient {part} contains an explicit zero coefficient at term {term}"
            ),
            Self::NonCanonicalMonomialOrder { part, term } => write!(
                formatter,
                "coefficient {part} is not in strict lexicographic monomial order at term {term}"
            ),
            Self::ZeroDenominator => {
                formatter.write_str("rational polynomial has a zero denominator")
            }
            Self::DivisionByZero => {
                formatter.write_str("attempted to divide by an identically zero coefficient")
            }
            Self::ExponentLimit {
                operation,
                variable,
                requested,
                limit,
            } => write!(
                formatter,
                "exact {operation} needs exponent {requested} in variable {variable}, above limit {limit}"
            ),
            Self::ExponentArithmeticOverflow {
                operation,
                variable,
                width,
            } => write!(
                formatter,
                "exact {operation} exponent arithmetic overflowed u{width} in variable {variable}"
            ),
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
        }
    }
}

impl std::error::Error for ExactAlgebraError {}

pub(crate) fn validate_coefficient_on_map(
    coefficient: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    validate_polynomial_on_map(
        &coefficient.numerator,
        variables,
        CoefficientPolynomialPart::Numerator,
        limits,
    )?;
    validate_polynomial_on_map(
        &coefficient.denominator,
        variables,
        CoefficientPolynomialPart::Denominator,
        limits,
    )?;
    if coefficient.denominator.coefficients.is_empty() {
        return Err(ExactAlgebraError::ZeroDenominator);
    }
    Ok(())
}

pub(crate) fn checked_coefficient_add_on_map(
    left: &Coefficient,
    right: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, ExactAlgebraError> {
    checked_coefficient_sum_on_map(
        left,
        right,
        variables,
        limits,
        ExactAlgebraOperation::Add,
        false,
    )
}

pub(crate) fn checked_coefficient_sub_on_map(
    left: &Coefficient,
    right: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, ExactAlgebraError> {
    checked_coefficient_sum_on_map(
        left,
        right,
        variables,
        limits,
        ExactAlgebraOperation::Subtract,
        true,
    )
}

pub(crate) fn checked_coefficient_mul_on_map(
    left: &Coefficient,
    right: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, ExactAlgebraError> {
    validate_binary_inputs(left, right, variables, limits)?;
    preflight_product_degrees(
        &left.numerator,
        &right.numerator,
        ExactAlgebraOperation::Multiply,
        limits,
    )?;
    preflight_product_degrees(
        &left.denominator,
        &right.denominator,
        ExactAlgebraOperation::Multiply,
        limits,
    )?;
    preflight_product_terms(
        left.numerator.nterms(),
        right.numerator.nterms(),
        "exact multiplication numerator terms",
        limits,
    )?;
    preflight_product_terms(
        left.denominator.nterms(),
        right.denominator.nterms(),
        "exact multiplication denominator terms",
        limits,
    )?;
    let result = left * right;
    validate_coefficient_on_map(&result, variables, limits)?;
    Ok(result)
}

pub(crate) fn checked_coefficient_div_on_map(
    numerator: &Coefficient,
    denominator: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, ExactAlgebraError> {
    validate_binary_inputs(numerator, denominator, variables, limits)?;
    if denominator.numerator.coefficients.is_empty() {
        return Err(ExactAlgebraError::DivisionByZero);
    }
    preflight_product_degrees(
        &numerator.numerator,
        &denominator.denominator,
        ExactAlgebraOperation::Divide,
        limits,
    )?;
    preflight_product_degrees(
        &numerator.denominator,
        &denominator.numerator,
        ExactAlgebraOperation::Divide,
        limits,
    )?;
    preflight_product_terms(
        numerator.numerator.nterms(),
        denominator.denominator.nterms(),
        "exact division numerator terms",
        limits,
    )?;
    preflight_product_terms(
        numerator.denominator.nterms(),
        denominator.numerator.nterms(),
        "exact division denominator terms",
        limits,
    )?;
    let result = numerator / denominator;
    validate_coefficient_on_map(&result, variables, limits)?;
    Ok(result)
}

pub(crate) fn checked_coefficient_neg_on_map(
    value: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, ExactAlgebraError> {
    validate_coefficient_on_map(value, variables, limits)?;
    let result = -value.clone();
    validate_coefficient_on_map(&result, variables, limits)?;
    Ok(result)
}

fn checked_coefficient_sum_on_map(
    left: &Coefficient,
    right: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
    operation: ExactAlgebraOperation,
    subtract: bool,
) -> Result<Coefficient, ExactAlgebraError> {
    validate_binary_inputs(left, right, variables, limits)?;
    if left.denominator == right.denominator {
        preflight_sum_terms(
            left.numerator.nterms(),
            right.numerator.nterms(),
            "exact equal-denominator numerator terms",
            limits,
        )?;
    } else {
        preflight_cross_sum_degrees(left, right, operation, limits)?;
        let left_terms = checked_term_product(
            left.numerator.nterms(),
            right.denominator.nterms(),
            "exact addition numerator terms",
        )?;
        let right_terms = checked_term_product(
            right.numerator.nterms(),
            left.denominator.nterms(),
            "exact addition numerator terms",
        )?;
        preflight_sum_terms(
            left_terms,
            right_terms,
            "exact addition numerator terms",
            limits,
        )?;
        preflight_product_terms(
            left.denominator.nterms(),
            right.denominator.nterms(),
            "exact addition denominator terms",
            limits,
        )?;
    }
    let result = if subtract { left - right } else { left + right };
    validate_coefficient_on_map(&result, variables, limits)?;
    Ok(result)
}

fn validate_binary_inputs(
    left: &Coefficient,
    right: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    validate_coefficient_on_map(left, variables, limits)?;
    validate_coefficient_on_map(right, variables, limits)
}

pub(crate) fn validate_polynomial_on_map(
    polynomial: &MultivariatePolynomial<IntegerRing, u16>,
    variables: &Arc<Vec<PolyVariable>>,
    part: CoefficientPolynomialPart,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    if polynomial.variables.as_ref() != variables.as_ref() {
        return Err(ExactAlgebraError::VariableMapMismatch { part });
    }
    let expected = polynomial
        .coefficients
        .len()
        .checked_mul(variables.len())
        .ok_or(ExactAlgebraError::ResourceCountOverflow {
            resource: "polynomial exponent layout",
        })?;
    if polynomial.exponents.len() != expected {
        return Err(ExactAlgebraError::MalformedExponentLayout {
            part,
            coefficients: polynomial.coefficients.len(),
            exponents: polynomial.exponents.len(),
            variables: variables.len(),
        });
    }
    check_exact_resource_limit(
        "authenticated polynomial terms",
        polynomial.coefficients.len(),
        limits.max_polynomial_terms,
    )?;
    for (term, coefficient) in polynomial.coefficients.iter().enumerate() {
        // Symbolica's public `Integer` representation can retain a numeric zero
        // in a noncanonical backend variant, whereas `IntegerRing::is_zero`
        // recognizes only the canonical small zero.  Authentication is a
        // numeric boundary, so reject every representation of exact zero.
        if coefficient.cmp(&Integer::Single(0)) == Ordering::Equal {
            return Err(ExactAlgebraError::ZeroCoefficient { part, term });
        }
    }
    if variables.is_empty() {
        if polynomial.coefficients.len() > 1 {
            return Err(ExactAlgebraError::NonCanonicalMonomialOrder { part, term: 1 });
        }
        return Ok(());
    }
    for (term, exponents) in polynomial
        .exponents
        .chunks_exact(variables.len())
        .enumerate()
    {
        for (variable, &exponent) in exponents.iter().enumerate() {
            if exponent > limits.max_exponent {
                return Err(ExactAlgebraError::ExponentLimit {
                    operation: ExactAlgebraOperation::Authenticate,
                    variable,
                    requested: u64::from(exponent),
                    limit: limits.max_exponent,
                });
            }
        }
        if term > 0 {
            let previous_start = (term - 1) * variables.len();
            let previous = &polynomial.exponents[previous_start..previous_start + variables.len()];
            if previous.cmp(exponents) != Ordering::Less {
                return Err(ExactAlgebraError::NonCanonicalMonomialOrder { part, term });
            }
        }
    }
    Ok(())
}

fn preflight_cross_sum_degrees(
    left: &Coefficient,
    right: &Coefficient,
    operation: ExactAlgebraOperation,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    for variable in 0..left.numerator.variables.len() {
        let left_numerator = left.numerator.degree(variable);
        let left_denominator = left.denominator.degree(variable);
        let right_numerator = right.numerator.degree(variable);
        let right_denominator = right.denominator.degree(variable);
        let requested =
            checked_pairwise_exponent_sum(left_numerator, right_denominator, operation, variable)?
                .max(checked_pairwise_exponent_sum(
                    right_numerator,
                    left_denominator,
                    operation,
                    variable,
                )?)
                .max(checked_pairwise_exponent_sum(
                    left_denominator,
                    right_denominator,
                    operation,
                    variable,
                )?);
        check_exact_exponent(operation, variable, u64::from(requested), limits)?;
    }
    Ok(())
}

fn preflight_product_degrees(
    left: &MultivariatePolynomial<IntegerRing, u16>,
    right: &MultivariatePolynomial<IntegerRing, u16>,
    operation: ExactAlgebraOperation,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    for variable in 0..left.variables.len() {
        let requested = checked_pairwise_exponent_sum(
            left.degree(variable),
            right.degree(variable),
            operation,
            variable,
        )?;
        check_exact_exponent(operation, variable, u64::from(requested), limits)?;
    }
    Ok(())
}

fn checked_pairwise_exponent_sum(
    left: u16,
    right: u16,
    operation: ExactAlgebraOperation,
    variable: usize,
) -> Result<u32, ExactAlgebraError> {
    u32::from(left).checked_add(u32::from(right)).ok_or(
        ExactAlgebraError::ExponentArithmeticOverflow {
            operation,
            variable,
            width: 32,
        },
    )
}

fn preflight_power_degrees(
    polynomial: &MultivariatePolynomial<IntegerRing, u16>,
    exponent: u64,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    for variable in 0..polynomial.variables.len() {
        let requested = u64::from(polynomial.degree(variable))
            .checked_mul(exponent)
            .ok_or(ExactAlgebraError::ExponentArithmeticOverflow {
                operation: ExactAlgebraOperation::Power,
                variable,
                width: 64,
            })?;
        check_exact_exponent(ExactAlgebraOperation::Power, variable, requested, limits)?;
    }
    Ok(())
}

fn check_exact_exponent(
    operation: ExactAlgebraOperation,
    variable: usize,
    requested: u64,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    if requested > u64::from(limits.max_exponent) {
        Err(ExactAlgebraError::ExponentLimit {
            operation,
            variable,
            requested,
            limit: limits.max_exponent,
        })
    } else {
        Ok(())
    }
}

fn checked_term_product(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, ExactAlgebraError> {
    left.checked_mul(right)
        .ok_or(ExactAlgebraError::ResourceCountOverflow { resource })
}

fn preflight_product_terms(
    left: usize,
    right: usize,
    resource: &'static str,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    let requested = checked_term_product(left, right, resource)?;
    check_exact_resource_limit(resource, requested, limits.max_term_operations)?;
    check_exact_resource_limit(resource, requested, limits.max_polynomial_terms)
}

fn preflight_sum_terms(
    left: usize,
    right: usize,
    resource: &'static str,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    let requested = left
        .checked_add(right)
        .ok_or(ExactAlgebraError::ResourceCountOverflow { resource })?;
    check_exact_resource_limit(resource, requested, limits.max_term_operations)?;
    check_exact_resource_limit(resource, requested, limits.max_polynomial_terms)
}

fn check_exact_resource_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ExactAlgebraError> {
    if requested > limit {
        Err(ExactAlgebraError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

/// A shared Symbolica rational-polynomial coefficient domain.
#[derive(Clone, Debug)]
pub struct CoefficientContext {
    names: Arc<Vec<String>>,
    variables: Arc<Vec<PolyVariable>>,
    template: Coefficient,
}

pub(crate) fn coefficient_clone_owned_retained_byte_bound(
    coefficient: &Coefficient,
) -> Option<usize> {
    let polynomial_bytes = |polynomial: &MultivariatePolynomial<IntegerRing, u16>| {
        let mut bytes = polynomial
            .coefficients
            .capacity()
            .checked_mul(size_of::<Integer>())?
            .checked_add(
                polynomial
                    .exponents
                    .capacity()
                    .checked_mul(size_of::<u16>())?,
            )?;
        for coefficient in &polynomial.coefficients {
            if let Integer::Large(value) = coefficient {
                let capacity_bits = usize::try_from(value.capacity()).ok()?;
                bytes = bytes.checked_add(capacity_bits.checked_add(7)?.checked_div(8)?)?;
            }
        }
        Some(bytes)
    };
    size_of::<Coefficient>()
        .checked_add(polynomial_bytes(&coefficient.numerator)?)?
        .checked_add(polynomial_bytes(&coefficient.denominator)?)
}

/// Typed failures produced before constructing a Symbolica polynomial map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoefficientContextError {
    DuplicateParameter(String),
    InvalidParameter { name: String, reason: String },
}

/// The numerator or denominator of an exact coefficient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoefficientPolynomialPart {
    Numerator,
    Denominator,
}

impl fmt::Display for CoefficientPolynomialPart {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Numerator => formatter.write_str("numerator"),
            Self::Denominator => formatter.write_str("denominator"),
        }
    }
}

impl fmt::Display for CoefficientContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateParameter(name) => {
                write!(formatter, "coefficient parameter {name:?} is repeated")
            }
            Self::InvalidParameter { name, reason } => {
                write!(
                    formatter,
                    "invalid coefficient parameter {name:?}: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for CoefficientContextError {}

impl CoefficientContext {
    /// Construct a validated Symbolica variable map without allowing malformed
    /// or duplicate caller labels to reach polynomial construction.
    pub fn try_new(
        parameter_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, CoefficientContextError> {
        let names: Vec<String> = parameter_names.into_iter().map(Into::into).collect();
        for (index, name) in names.iter().enumerate() {
            if names[..index].contains(name) {
                return Err(CoefficientContextError::DuplicateParameter(name.clone()));
            }
        }
        let mut variables = Vec::with_capacity(names.len());
        for name in &names {
            let qualified = format!("{RUSTRED_NAMESPACE}::{name}");
            let namespaced = NamespacedSymbol::try_parse(&qualified).ok_or_else(|| {
                CoefficientContextError::InvalidParameter {
                    name: name.clone(),
                    reason: "could not form a namespaced Symbolica symbol".to_owned(),
                }
            })?;
            let symbol = SymbolBuilder::new(namespaced).build().map_err(|reason| {
                CoefficientContextError::InvalidParameter {
                    name: name.clone(),
                    reason: reason.to_string(),
                }
            })?;
            variables.push(PolyVariable::Symbol(symbol));
        }
        let variables = Arc::new(variables);
        let template = RationalPolynomial::new(&Z, variables.clone());
        Ok(Self {
            names: Arc::new(names),
            variables,
            template,
        })
    }

    #[cfg(test)]
    pub(crate) fn new(parameter_names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::try_new(parameter_names).expect("test coefficient labels must be valid")
    }

    pub fn parameter_names(&self) -> &[String] {
        self.names.as_slice()
    }

    /// Whether two contexts use the exact same ordered Symbolica polynomial
    /// variable map and RustRed parameter labels.
    ///
    /// Matching names alone are not sufficient for safe coefficient
    /// composition.  Higher-loop component services use this check before
    /// sharing a lower-loop reduction cache across authenticated families.
    pub fn has_same_variable_map(&self, other: &Self) -> bool {
        self.names == other.names && self.template.get_variables() == other.template.get_variables()
    }

    /// Whether `coefficient` uses exactly this context's ordered variable map
    /// in both polynomial parts.
    ///
    /// Symbolica normally unifies differing maps during arithmetic.  Generic
    /// RustRed code uses this check before every proof-bearing composition so
    /// that an undeclared variable cannot be appended implicitly.
    pub fn contains(&self, coefficient: &Coefficient) -> bool {
        self.validate_with_limits(coefficient, ExactAlgebraLimits::default())
            .is_ok()
    }

    pub fn validate_with_limits(
        &self,
        coefficient: &Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<(), ExactAlgebraError> {
        validate_coefficient_on_map(coefficient, &self.variables, limits)
    }

    pub(crate) fn preflight_power_with_limits(
        &self,
        coefficient: &Coefficient,
        exponent: u64,
        limits: ExactAlgebraLimits,
    ) -> Result<(), ExactAlgebraError> {
        validate_coefficient_on_map(coefficient, &self.variables, limits)?;
        preflight_power_degrees(&coefficient.numerator, exponent, limits)?;
        preflight_power_degrees(&coefficient.denominator, exponent, limits)
    }

    pub(crate) fn variables(&self) -> &Arc<Vec<PolyVariable>> {
        &self.variables
    }

    pub(crate) fn template(&self) -> &Coefficient {
        &self.template
    }

    pub fn zero(&self) -> Coefficient {
        self.integer(0)
    }

    pub fn one(&self) -> Coefficient {
        self.integer(1)
    }

    pub fn integer(&self, value: i64) -> Coefficient {
        self.template
            .numerator
            .constant(Integer::from(value))
            .into()
    }

    pub fn parameter(&self, name: &str) -> Option<Coefficient> {
        self.names
            .iter()
            .position(|candidate| candidate == name)
            .map(|position| self.parameter_at(position))
    }

    pub fn parameter_at(&self, position: usize) -> Coefficient {
        self.template
            .numerator
            .variable(&self.variables[position])
            .expect("coefficient parameter is present in its own variable map")
            .into()
    }

    /// Checked exact addition for proof-bearing code.
    pub fn try_add(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, ExactAlgebraError> {
        checked_coefficient_add_on_map(left, right, &self.variables, limits)
    }

    /// Checked exact subtraction for proof-bearing code.
    pub fn try_sub(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, ExactAlgebraError> {
        checked_coefficient_sub_on_map(left, right, &self.variables, limits)
    }

    /// Checked exact multiplication for proof-bearing code.
    pub fn try_mul(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, ExactAlgebraError> {
        checked_coefficient_mul_on_map(left, right, &self.variables, limits)
    }

    /// Checked exact division for proof-bearing code.
    pub fn try_div(
        &self,
        numerator: &Coefficient,
        denominator: &Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, ExactAlgebraError> {
        checked_coefficient_div_on_map(numerator, denominator, &self.variables, limits)
    }

    /// Checked exact negation for proof-bearing code.
    pub fn try_neg(
        &self,
        value: &Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, ExactAlgebraError> {
        checked_coefficient_neg_on_map(value, &self.variables, limits)
    }

    #[cfg(test)]
    pub(crate) fn coefficient_fixture(&self, expression: &str) -> Coefficient {
        let atom = try_parse!(expression, default_namespace = RUSTRED_NAMESPACE)
            .expect("test coefficient must parse");
        let coefficient = atom
            .as_view()
            .try_to_rational_polynomial(&Q, &Z, Some(self.variables.clone()))
            .expect("test coefficient must be rational-polynomial");
        self.validate_with_limits(&coefficient, ExactAlgebraLimits::default())
            .expect("test coefficient must use the declared context");
        coefficient
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_authentication_rejects_malformed_sparse_polynomials_without_panicking() {
        let context = CoefficientContext::new(["x"]);

        let mut malformed_layout = context.one();
        malformed_layout.numerator.exponents.push(0);
        assert!(matches!(
            context.validate_with_limits(&malformed_layout, ExactAlgebraLimits::default()),
            Err(ExactAlgebraError::MalformedExponentLayout {
                part: CoefficientPolynomialPart::Numerator,
                ..
            })
        ));
        assert!(!context.contains(&malformed_layout));

        let mut explicit_zero = context.one();
        explicit_zero.numerator.coefficients[0] = Integer::from(0);
        assert!(matches!(
            context.validate_with_limits(&explicit_zero, ExactAlgebraLimits::default()),
            Err(ExactAlgebraError::ZeroCoefficient {
                part: CoefficientPolynomialPart::Numerator,
                term: 0,
            })
        ));

        let mut wrong_order = context.one();
        wrong_order.numerator.coefficients = vec![Integer::from(1), Integer::from(1)];
        wrong_order.numerator.exponents = vec![1, 0];
        assert!(matches!(
            context.validate_with_limits(&wrong_order, ExactAlgebraLimits::default()),
            Err(ExactAlgebraError::NonCanonicalMonomialOrder {
                part: CoefficientPolynomialPart::Numerator,
                term: 1,
            })
        ));
    }

    #[test]
    fn exact_authentication_rejects_every_backend_representation_of_numeric_zero() {
        let context = CoefficientContext::new(["x"]);
        for (part, zero) in [
            (CoefficientPolynomialPart::Numerator, Integer::Double(0)),
            (
                CoefficientPolynomialPart::Numerator,
                Integer::Large(0.into()),
            ),
            (CoefficientPolynomialPart::Denominator, Integer::Double(0)),
            (
                CoefficientPolynomialPart::Denominator,
                Integer::Large(0.into()),
            ),
        ] {
            let mut malformed = context.one();
            match part {
                CoefficientPolynomialPart::Numerator => {
                    malformed.numerator.coefficients[0] = zero;
                }
                CoefficientPolynomialPart::Denominator => {
                    malformed.denominator.coefficients[0] = zero;
                }
            }
            assert_eq!(
                context.validate_with_limits(&malformed, ExactAlgebraLimits::default()),
                Err(ExactAlgebraError::ZeroCoefficient { part, term: 0 })
            );
            assert!(!context.contains(&malformed));
        }
    }

    #[test]
    fn checked_exact_multiplication_reports_u16_exponent_overflow() {
        let context = CoefficientContext::new(["x"]);
        let maximal = context.coefficient_fixture("x^65535");
        let x = context.parameter("x").unwrap();
        assert!(matches!(
            context.try_mul(&maximal, &x, ExactAlgebraLimits::default()),
            Err(ExactAlgebraError::ExponentLimit {
                operation: ExactAlgebraOperation::Multiply,
                variable: 0,
                requested: 65_536,
                limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            })
        ));
    }

    #[test]
    fn checked_power_preflight_uses_u64_before_native_arithmetic() {
        let context = CoefficientContext::new(["x"]);
        let x_squared = context.coefficient_fixture("x^2");
        assert!(matches!(
            context.preflight_power_with_limits(
                &x_squared,
                u64::MAX,
                ExactAlgebraLimits::default(),
            ),
            Err(ExactAlgebraError::ExponentArithmeticOverflow {
                operation: ExactAlgebraOperation::Power,
                variable: 0,
                width: 64,
            })
        ));

        let x_40_000 = context.coefficient_fixture("x^40000");
        assert!(matches!(
            context.preflight_power_with_limits(&x_40_000, 2, ExactAlgebraLimits::default()),
            Err(ExactAlgebraError::ExponentLimit {
                operation: ExactAlgebraOperation::Power,
                variable: 0,
                requested: 80_000,
                limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            })
        ));
    }

    #[test]
    fn rational_normalization_can_densify_beyond_input_pair_counts() {
        let context = CoefficientContext::new(["x"]);
        let geometric_numerator = context.coefficient_fixture("x^8-1");
        let linear = context.coefficient_fixture("x-1");
        let reciprocal_linear = context.coefficient_fixture("1/(x-1)");

        let division = context
            .try_div(&geometric_numerator, &linear, ExactAlgebraLimits::default())
            .unwrap();
        let multiplication = context
            .try_mul(
                &geometric_numerator,
                &reciprocal_linear,
                ExactAlgebraLimits::default(),
            )
            .unwrap();
        assert_eq!(division.numerator.nterms(), 8);
        assert_eq!(multiplication.numerator.nterms(), 8);
        assert_eq!(division.denominator.nterms(), 1);
        assert_eq!(multiplication, division);
        assert!(
            division.numerator.nterms()
                > geometric_numerator.numerator.nterms() * linear.denominator.nterms()
        );

        let left = context.coefficient_fixture("1/(x-1)");
        let right = context.coefficient_fixture("(x^8-2)/(x-1)");
        let addition = context
            .try_add(&left, &right, ExactAlgebraLimits::default())
            .unwrap();
        assert_eq!(addition.numerator.nterms(), 8);
        assert_eq!(addition.denominator.nterms(), 1);
        assert!(addition.numerator.nterms() > left.numerator.nterms() + right.numerator.nterms());

        // These one-step input counts are not sound retained-output bounds for
        // rational arithmetic. The checked path must still reject the dense
        // normalized result during post-authentication.
        for error in [
            context
                .try_mul(
                    &geometric_numerator,
                    &reciprocal_linear,
                    ExactAlgebraLimits {
                        max_polynomial_terms: 2,
                        ..ExactAlgebraLimits::default()
                    },
                )
                .unwrap_err(),
            context
                .try_div(
                    &geometric_numerator,
                    &linear,
                    ExactAlgebraLimits {
                        max_polynomial_terms: 2,
                        ..ExactAlgebraLimits::default()
                    },
                )
                .unwrap_err(),
            context
                .try_add(
                    &left,
                    &right,
                    ExactAlgebraLimits {
                        max_polynomial_terms: 3,
                        ..ExactAlgebraLimits::default()
                    },
                )
                .unwrap_err(),
        ] {
            assert!(matches!(
                error,
                ExactAlgebraError::ResourceLimit {
                    resource: "authenticated polynomial terms",
                    requested: 8,
                    ..
                }
            ));
        }
    }
}
