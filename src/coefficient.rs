use std::{borrow::Borrow, cmp::Ordering, fmt, mem::size_of, sync::Arc};

use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::*;

use crate::ExactRational;

const RUSTRED_NAMESPACE: &str = "rustred";

/// Exact rational functions in the kinematic parameters.
pub type Coefficient = RationalPolynomial<IntegerRing, u16>;

/// Largest exponent representable by RustRed's Symbolica coefficient domain.
///
/// Symbolica's polynomial arithmetic panics when an operation would overflow
/// its exponent type.  Analytic reducers use this ceiling to preflight their
/// caller-controlled formula degrees before constructing coefficients.
pub const SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT: u128 = u16::MAX as u128;

/// Resource limits for exact rational-polynomial arithmetic.
///
/// These limits are checked before entering Symbolica operations that add
/// polynomial exponents.  This is essential because Symbolica deliberately
/// panics when its `u16` exponent representation overflows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactAlgebraLimits {
    /// Largest exponent admitted by RustRed's checked representation boundary.
    pub max_exponent: u128,
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
        }
    }
}

/// Typed failures produced before panic-prone Symbolica arithmetic is called.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExactAlgebraError {
    ConfiguredExponentLimit {
        requested: u128,
        representation_limit: u128,
    },
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
        requested: u128,
        limit: u128,
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
            Self::ConfiguredExponentLimit {
                requested,
                representation_limit,
            } => write!(
                formatter,
                "configured exponent limit {requested} exceeds the Symbolica representation limit {representation_limit}"
            ),
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

/// Per-variable numerator and denominator degrees of an existing coefficient.
///
/// Existing coefficients are already representable.  Reducers use these
/// bounds to account for nontrivial rational mass parameters before starting
/// a caller-controlled repeated product.
pub(crate) fn coefficient_variable_degrees(coefficient: &Coefficient) -> Vec<(u128, u128)> {
    (0..coefficient.numerator.variables.len())
        .map(|variable| {
            (
                u128::from(coefficient.numerator.degree(variable)),
                u128::from(coefficient.denominator.degree(variable)),
            )
        })
        .collect()
}

pub(crate) fn symbolica_coefficient_degree_is_representable(requested: u128) -> bool {
    requested <= SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT
}

/// Conservative per-variable exponent bound for `left * right`.
///
/// Symbolica cancels cross gcds before multiplying rational polynomials, so
/// the raw numerator/denominator degree sums can overestimate the result but
/// can never underestimate an intermediate multiplication.  Coefficients
/// from different variable maps are rejected conservatively.
pub(crate) fn coefficient_product_degree_bound(left: &Coefficient, right: &Coefficient) -> u128 {
    if left.get_variables() != right.get_variables() {
        return u128::MAX;
    }
    coefficient_variable_degrees(left)
        .into_iter()
        .zip(coefficient_variable_degrees(right))
        .map(
            |((left_numerator, left_denominator), (right_numerator, right_denominator))| {
                left_numerator
                    .saturating_add(right_numerator)
                    .max(left_denominator.saturating_add(right_denominator))
            },
        )
        .max()
        .unwrap_or(0)
}

/// Conservative per-variable exponent bound for `left + right`.
///
/// The bound mirrors cross multiplication over the two denominators.  The gcd
/// optimization used by Symbolica can only lower these degrees.
pub(crate) fn coefficient_sum_degree_bound(left: &Coefficient, right: &Coefficient) -> u128 {
    if left.get_variables() != right.get_variables() {
        return u128::MAX;
    }
    coefficient_variable_degrees(left)
        .into_iter()
        .zip(coefficient_variable_degrees(right))
        .map(
            |((left_numerator, left_denominator), (right_numerator, right_denominator))| {
                left_numerator
                    .saturating_add(right_denominator)
                    .max(right_numerator.saturating_add(left_denominator))
                    .max(left_denominator.saturating_add(right_denominator))
            },
        )
        .max()
        .unwrap_or(0)
}

pub(crate) fn validate_coefficient_on_map(
    coefficient: &Coefficient,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    validate_exact_limits(limits)?;
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

/// Multiply two already-canonical integer polynomials without routing the
/// operation through rational-function GCD normalization.
///
/// A polynomial product visits `left.nterms() * right.nterms()` Cartesian
/// pairs, but its retained sparse support is bounded independently by the
/// componentwise degree box.  Keeping those two bounds separate is sound for
/// direct polynomial multiplication: unlike a normalized rational operation,
/// no exact quotient can densify the result beyond the input-pair count.
pub(crate) fn checked_polynomial_mul_on_map(
    left: &MultivariatePolynomial<IntegerRing, u16>,
    right: &MultivariatePolynomial<IntegerRing, u16>,
    variables: &Arc<Vec<PolyVariable>>,
    limits: ExactAlgebraLimits,
    max_native_output_term_bound: usize,
) -> Result<MultivariatePolynomial<IntegerRing, u16>, ExactAlgebraError> {
    validate_polynomial_on_map(
        left,
        variables,
        CoefficientPolynomialPart::Numerator,
        limits,
    )?;
    validate_polynomial_on_map(
        right,
        variables,
        CoefficientPolynomialPart::Numerator,
        limits,
    )?;
    preflight_product_degrees(left, right, ExactAlgebraOperation::Multiply, limits)?;

    let term_pairs = checked_term_product(
        left.nterms(),
        right.nterms(),
        "exact polynomial multiplication term pairs",
    )?;
    check_exact_resource_limit(
        "exact polynomial multiplication term pairs",
        term_pairs,
        limits.max_term_operations,
    )?;
    let output_term_bound = polynomial_product_output_term_bound(left, right, term_pairs)?;
    check_exact_resource_limit(
        "exact polynomial multiplication output terms",
        output_term_bound,
        max_native_output_term_bound,
    )?;

    let result = left * right;
    check_exact_resource_limit(
        "exact polynomial multiplication output bound",
        result.nterms(),
        output_term_bound,
    )?;
    validate_polynomial_on_map(
        &result,
        variables,
        CoefficientPolynomialPart::Numerator,
        limits,
    )?;
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

fn validate_exact_limits(limits: ExactAlgebraLimits) -> Result<(), ExactAlgebraError> {
    if limits.max_exponent > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
        Err(ExactAlgebraError::ConfiguredExponentLimit {
            requested: limits.max_exponent,
            representation_limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
        })
    } else {
        Ok(())
    }
}

pub(crate) fn validate_polynomial_on_map(
    polynomial: &MultivariatePolynomial<IntegerRing, u16>,
    variables: &Arc<Vec<PolyVariable>>,
    part: CoefficientPolynomialPart,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    validate_exact_limits(limits)?;
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
        if polynomial.ring.is_zero(coefficient) {
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
            let requested = u128::from(exponent);
            if requested > limits.max_exponent {
                return Err(ExactAlgebraError::ExponentLimit {
                    operation: ExactAlgebraOperation::Authenticate,
                    variable,
                    requested,
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
        let left_numerator = u128::from(left.numerator.degree(variable));
        let left_denominator = u128::from(left.denominator.degree(variable));
        let right_numerator = u128::from(right.numerator.degree(variable));
        let right_denominator = u128::from(right.denominator.degree(variable));
        let requested = left_numerator
            .saturating_add(right_denominator)
            .max(right_numerator.saturating_add(left_denominator))
            .max(left_denominator.saturating_add(right_denominator));
        check_exact_exponent(operation, variable, requested, limits)?;
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
        let requested =
            u128::from(left.degree(variable)).saturating_add(u128::from(right.degree(variable)));
        check_exact_exponent(operation, variable, requested, limits)?;
    }
    Ok(())
}

fn check_exact_exponent(
    operation: ExactAlgebraOperation,
    variable: usize,
    requested: u128,
    limits: ExactAlgebraLimits,
) -> Result<(), ExactAlgebraError> {
    if requested > limits.max_exponent {
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

fn polynomial_product_output_term_bound(
    left: &MultivariatePolynomial<IntegerRing, u16>,
    right: &MultivariatePolynomial<IntegerRing, u16>,
    term_pairs: usize,
) -> Result<usize, ExactAlgebraError> {
    if term_pairs == 0 {
        return Ok(0);
    }

    // Cap every partial product at the Cartesian-pair count. This computes
    // min(term_pairs, product_i(deg_i(left) + deg_i(right) + 1)) without an
    // overflowing intermediate and is exact as an upper bound on support.
    let mut degree_box = 1usize;
    for variable in 0..left.variables.len() {
        let width = usize::from(left.degree(variable))
            .checked_add(usize::from(right.degree(variable)))
            .and_then(|degree| degree.checked_add(1))
            .ok_or(ExactAlgebraError::ResourceCountOverflow {
                resource: "exact polynomial multiplication degree box",
            })?;
        degree_box = degree_box.saturating_mul(width).min(term_pairs);
    }
    Ok(degree_box)
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

fn coefficient_clone_owned_retained_byte_bound(coefficient: &Coefficient) -> Option<usize> {
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
    /// Retained for source compatibility with the pre-parametric API.
    /// Empty contexts are now valid and represent the exact field `Q`.
    Empty,
    DuplicateParameter(String),
    InvalidParameter {
        name: String,
        reason: String,
    },
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

/// Typed failures from exact projection into a coefficient context with one
/// named parameter removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoefficientProjectionError {
    DroppedParameterNotFound(String),
    TargetParameterCount {
        expected: usize,
        actual: usize,
    },
    TargetParameterMismatch {
        position: usize,
        expected: String,
        actual: String,
    },
    TargetVariableMapMismatch {
        position: usize,
        parameter: String,
    },
    TargetTemplateVariableMapMismatch {
        part: CoefficientPolynomialPart,
    },
    SourceVariableMapMismatch {
        part: CoefficientPolynomialPart,
    },
    MalformedExponentLayout {
        part: CoefficientPolynomialPart,
        coefficients: usize,
        exponents: usize,
        variables: usize,
    },
    DroppedParameterDependence {
        parameter: String,
        part: CoefficientPolynomialPart,
        term: usize,
        exponent: u16,
    },
    ExponentOutOfRange {
        parameter: String,
        part: CoefficientPolynomialPart,
        term: usize,
        exponent: u128,
        limit: u128,
    },
    ZeroDenominator,
}

impl fmt::Display for CoefficientProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DroppedParameterNotFound(parameter) => {
                write!(
                    formatter,
                    "coefficient parameter {parameter:?} is not present"
                )
            }
            Self::TargetParameterCount { expected, actual } => write!(
                formatter,
                "coefficient projection needs {expected} target parameters, found {actual}"
            ),
            Self::TargetParameterMismatch {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "coefficient projection target parameter {position} is {actual:?}, expected {expected:?}"
            ),
            Self::TargetVariableMapMismatch {
                position,
                parameter,
            } => write!(
                formatter,
                "coefficient projection target parameter {position} ({parameter:?}) has a different Symbolica variable"
            ),
            Self::TargetTemplateVariableMapMismatch { part } => write!(
                formatter,
                "coefficient projection target template {part} does not use the target context variable map"
            ),
            Self::SourceVariableMapMismatch { part } => write!(
                formatter,
                "coefficient {part} does not use the source context variable map"
            ),
            Self::MalformedExponentLayout {
                part,
                coefficients,
                exponents,
                variables,
            } => write!(
                formatter,
                "coefficient {part} has {coefficients} terms, {exponents} exponents, and {variables} variables"
            ),
            Self::DroppedParameterDependence {
                parameter,
                part,
                term,
                exponent,
            } => write!(
                formatter,
                "coefficient {part} term {term} retains {parameter:?} with exponent {exponent}"
            ),
            Self::ExponentOutOfRange {
                parameter,
                part,
                term,
                exponent,
                limit,
            } => write!(
                formatter,
                "coefficient {part} term {term} has exponent {exponent} for {parameter:?}, above the Symbolica limit {limit}"
            ),
            Self::ZeroDenominator => {
                formatter.write_str("coefficient projection received a zero denominator")
            }
        }
    }
}

impl std::error::Error for CoefficientProjectionError {}

impl fmt::Display for CoefficientContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => {
                formatter.write_str("a coefficient context needs at least one parameter")
            }
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
    /// Conservative bytes owned by a deep clone of this context. The names
    /// and variable maps remain shared `Arc` payloads; only the inline context
    /// and the sparse/GMP payload of its private template are charged.
    pub(crate) fn clone_owned_retained_byte_bound(&self) -> Option<usize> {
        size_of::<Self>().checked_add(coefficient_clone_owned_retained_byte_bound(&self.template)?)
    }

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

    /// Compatibility constructor for trusted static parameter labels.
    /// Caller-controlled labels should use [`Self::try_new`].
    pub fn new(parameter_names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::try_new(parameter_names)
            .unwrap_or_else(|error| panic!("invalid coefficient context: {error}"))
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
        self.validate(coefficient).is_ok()
    }

    /// Authenticate the exact variable map and sparse polynomial structure.
    pub fn validate(&self, coefficient: &Coefficient) -> Result<(), ExactAlgebraError> {
        self.validate_with_limits(coefficient, ExactAlgebraLimits::default())
    }

    pub fn validate_with_limits(
        &self,
        coefficient: &Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<(), ExactAlgebraError> {
        validate_coefficient_on_map(coefficient, &self.variables, limits)
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

    pub fn rational(&self, value: impl Borrow<ExactRational>) -> Coefficient {
        let value = value.borrow();
        let numerator: Coefficient = self
            .template
            .numerator
            .constant(value.numerator().clone())
            .into();
        let denominator: Coefficient = self
            .template
            .denominator
            .constant(value.denominator().clone())
            .into();
        &numerator / &denominator
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

    pub fn parse(&self, expression: &str) -> Result<Coefficient, String> {
        let atom = try_parse!(expression, default_namespace = RUSTRED_NAMESPACE)
            .map_err(|error| error.to_string())?;
        self.parse_atom(atom.as_view())
    }

    /// Convert an already parsed Symbolica expression into this exact base
    /// field without formatting and reparsing it.
    ///
    /// Symbolica's polynomial converter may append variables which were not
    /// present in the supplied map.  The validation after conversion is
    /// therefore part of this boundary: an undeclared symbol, function, or
    /// non-rational power is rejected instead of silently extending `K`.
    pub fn parse_atom(&self, expression: AtomView<'_>) -> Result<Coefficient, String> {
        self.parse_atom_with_limits(expression, ExactAlgebraLimits::default())
    }

    /// Convert and authenticate an Atom under explicit retained-output limits.
    ///
    /// These limits validate the resulting sparse rational polynomial.  They
    /// do **not** by themselves bound Symbolica's conversion, expansion, or
    /// GCD workspace.  Callers accepting untrusted expressions must first
    /// enforce their own AST/degree/term-work preflight and protect the native
    /// conversion boundary against panics.  Keeping that policy outside this
    /// base-field type lets each expression grammar account for its own exact
    /// expansion envelope while this method remains the single no-format/
    /// no-reparse authentication seam.
    pub fn parse_atom_with_limits(
        &self,
        expression: AtomView<'_>,
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, String> {
        let coefficient = expression
            .try_to_rational_polynomial(&Q, &Z, Some(self.variables.clone()))
            .map_err(|error| error.to_string())?;
        if let Err(error) = self.validate_with_limits(&coefficient, limits) {
            return Err(format!(
                "coefficient is outside the declared context {:?}: {error}",
                self.names,
            ));
        }
        Ok(coefficient)
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

    /// Project an exact coefficient into `target` after proving that
    /// `dropped_parameter` is absent from every numerator and denominator
    /// monomial.
    ///
    /// The target parameters must be exactly the source parameters, in the
    /// same order, with the named parameter removed.  Both the labels and the
    /// underlying Symbolica variables are authenticated.  Integer
    /// coefficients and checked exponents are copied directly into the target
    /// polynomial map; this path never formats or parses the coefficient.
    pub fn project_parameter_free(
        &self,
        coefficient: &Coefficient,
        dropped_parameter: &str,
        target: &CoefficientContext,
    ) -> Result<Coefficient, CoefficientProjectionError> {
        let dropped_position = self
            .names
            .iter()
            .position(|name| name == dropped_parameter)
            .ok_or_else(|| {
                CoefficientProjectionError::DroppedParameterNotFound(dropped_parameter.to_owned())
            })?;
        let expected_target_count = self.names.len().saturating_sub(1);
        if target.names.len() != expected_target_count {
            return Err(CoefficientProjectionError::TargetParameterCount {
                expected: expected_target_count,
                actual: target.names.len(),
            });
        }
        if target.template.numerator.variables.as_ref() != target.variables.as_ref() {
            return Err(
                CoefficientProjectionError::TargetTemplateVariableMapMismatch {
                    part: CoefficientPolynomialPart::Numerator,
                },
            );
        }
        if target.template.denominator.variables.as_ref() != target.variables.as_ref() {
            return Err(
                CoefficientProjectionError::TargetTemplateVariableMapMismatch {
                    part: CoefficientPolynomialPart::Denominator,
                },
            );
        }

        let retained_positions: Vec<usize> = (0..self.names.len())
            .filter(|position| *position != dropped_position)
            .collect();
        for (target_position, source_position) in retained_positions.iter().copied().enumerate() {
            let expected = &self.names[source_position];
            let actual = &target.names[target_position];
            if actual != expected {
                return Err(CoefficientProjectionError::TargetParameterMismatch {
                    position: target_position,
                    expected: expected.clone(),
                    actual: actual.clone(),
                });
            }
            if target.variables[target_position] != self.variables[source_position] {
                return Err(CoefficientProjectionError::TargetVariableMapMismatch {
                    position: target_position,
                    parameter: expected.clone(),
                });
            }
        }

        if coefficient.numerator.variables.as_ref() != self.variables.as_ref() {
            return Err(CoefficientProjectionError::SourceVariableMapMismatch {
                part: CoefficientPolynomialPart::Numerator,
            });
        }
        if coefficient.denominator.variables.as_ref() != self.variables.as_ref() {
            return Err(CoefficientProjectionError::SourceVariableMapMismatch {
                part: CoefficientPolynomialPart::Denominator,
            });
        }
        if coefficient.denominator.is_zero() {
            return Err(CoefficientProjectionError::ZeroDenominator);
        }

        let numerator = project_polynomial_parameter_free(
            &coefficient.numerator,
            &target.template.numerator,
            &self.names,
            &retained_positions,
            dropped_position,
            dropped_parameter,
            CoefficientPolynomialPart::Numerator,
        )?;
        let denominator = project_polynomial_parameter_free(
            &coefficient.denominator,
            &target.template.denominator,
            &self.names,
            &retained_positions,
            dropped_position,
            dropped_parameter,
            CoefficientPolynomialPart::Denominator,
        )?;
        if denominator.is_zero() {
            return Err(CoefficientProjectionError::ZeroDenominator);
        }

        Ok(<Coefficient as FromNumeratorAndDenominator<
            IntegerRing,
            IntegerRing,
            u16,
        >>::from_num_den(numerator, denominator, &Z, true))
    }

    pub fn scale_integer(&self, coefficient: &Coefficient, value: i32) -> Coefficient {
        coefficient * &self.integer(i64::from(value))
    }

    pub fn scale_rational(
        &self,
        coefficient: &Coefficient,
        value: impl Borrow<ExactRational>,
    ) -> Coefficient {
        coefficient * &self.rational(value)
    }
}

#[allow(clippy::too_many_arguments)]
fn project_polynomial_parameter_free(
    polynomial: &MultivariatePolynomial<IntegerRing, u16>,
    target_template: &MultivariatePolynomial<IntegerRing, u16>,
    source_names: &[String],
    retained_positions: &[usize],
    dropped_position: usize,
    dropped_parameter: &str,
    part: CoefficientPolynomialPart,
) -> Result<MultivariatePolynomial<IntegerRing, u16>, CoefficientProjectionError> {
    let expected_exponents = polynomial
        .coefficients
        .len()
        .checked_mul(source_names.len())
        .ok_or(CoefficientProjectionError::MalformedExponentLayout {
            part,
            coefficients: polynomial.coefficients.len(),
            exponents: polynomial.exponents.len(),
            variables: source_names.len(),
        })?;
    if polynomial.exponents.len() != expected_exponents {
        return Err(CoefficientProjectionError::MalformedExponentLayout {
            part,
            coefficients: polynomial.coefficients.len(),
            exponents: polynomial.exponents.len(),
            variables: source_names.len(),
        });
    }

    let mut projected = target_template.zero_with_capacity(polynomial.coefficients.len());
    let mut projected_exponents = Vec::with_capacity(retained_positions.len());
    for (term, (coefficient, exponents)) in polynomial
        .coefficients
        .iter()
        .zip(polynomial.exponents.chunks_exact(source_names.len()))
        .enumerate()
    {
        let dropped_exponent = exponents[dropped_position];
        if dropped_exponent != 0 {
            return Err(CoefficientProjectionError::DroppedParameterDependence {
                parameter: dropped_parameter.to_owned(),
                part,
                term,
                exponent: dropped_exponent,
            });
        }

        projected_exponents.clear();
        for source_position in retained_positions {
            let exponent = u128::from(exponents[*source_position]);
            if !symbolica_coefficient_degree_is_representable(exponent) {
                return Err(CoefficientProjectionError::ExponentOutOfRange {
                    parameter: source_names[*source_position].clone(),
                    part,
                    term,
                    exponent,
                    limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
                });
            }
            let exponent = u16::try_from(exponent).map_err(|_| {
                CoefficientProjectionError::ExponentOutOfRange {
                    parameter: source_names[*source_position].clone(),
                    part,
                    term,
                    exponent,
                    limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
                }
            })?;
            projected_exponents.push(exponent);
        }
        projected.append_monomial(coefficient.clone(), &projected_exponents);
    }
    Ok(projected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_canonicalizes_rational_functions() {
        let context = CoefficientContext::new(["d", "m2"]);
        let parsed = context.parse("(3-d)/(3*m2)").unwrap();
        let d = context.parameter("d").unwrap();
        let m2 = context.parameter("m2").unwrap();
        let expected = &(&context.integer(3) - &d) / &(&context.integer(3) * &m2);
        assert_eq!(parsed, expected);
    }

    #[test]
    fn projects_mass_free_rational_polynomial_without_parsing() {
        let source = CoefficientContext::new(["d", "m2"]);
        let target = CoefficientContext::new(["d"]);
        let d = source.parameter("d").unwrap();
        let numerator = &(&d * &d) + &source.integer(3);
        let denominator = &d - &source.integer(1);
        let coefficient = &numerator / &denominator;

        let projected = source
            .project_parameter_free(&coefficient, "m2", &target)
            .unwrap();
        let target_d = target.parameter("d").unwrap();
        let expected =
            &(&(&target_d * &target_d) + &target.integer(3)) / &(&target_d - &target.integer(1));

        assert_eq!(projected, expected);
        assert_eq!(projected.get_variables(), target.template.get_variables());
    }

    #[test]
    fn projects_zero_and_the_largest_representable_exponent() {
        let source = CoefficientContext::new(["d", "m2"]);
        let target = CoefficientContext::new(["d"]);

        assert_eq!(
            source
                .project_parameter_free(&source.zero(), "m2", &target)
                .unwrap(),
            target.zero()
        );

        let mut numerator = source.template.numerator.zero_with_capacity(1);
        numerator.append_monomial(Integer::from(7), &[u16::MAX, 0]);
        let maximal = RationalPolynomial {
            numerator,
            denominator: source.template.denominator.clone(),
        };
        let projected = source
            .project_parameter_free(&maximal, "m2", &target)
            .unwrap();
        assert_eq!(projected.numerator.coefficients, vec![Integer::from(7)]);
        assert_eq!(projected.numerator.exponents, vec![u16::MAX]);
        assert_eq!(projected.denominator.exponents, vec![0]);
    }

    #[test]
    fn projection_canonicalizes_fabricated_rational_polynomials() {
        let source = CoefficientContext::new(["d", "m2"]);
        let target = CoefficientContext::new(["d"]);
        let target_d = target.parameter("d").unwrap();

        let mut twice_d = source.template.numerator.zero_with_capacity(1);
        twice_d.append_monomial(Integer::from(2), &[1, 0]);
        let two = source.template.denominator.constant(Integer::from(2));
        let unnormalized_common_factor = RationalPolynomial {
            numerator: twice_d,
            denominator: two,
        };
        assert_eq!(
            source
                .project_parameter_free(&unnormalized_common_factor, "m2", &target)
                .unwrap(),
            target_d
        );

        let mut negative_d = source.template.numerator.zero_with_capacity(1);
        negative_d.append_monomial(Integer::from(-1), &[1, 0]);
        let negative_one = source.template.denominator.constant(Integer::from(-1));
        let negative_denominator = RationalPolynomial {
            numerator: negative_d,
            denominator: negative_one,
        };
        assert_eq!(
            source
                .project_parameter_free(&negative_denominator, "m2", &target)
                .unwrap(),
            target.parameter("d").unwrap()
        );

        let mut d_plus_one = source.template.denominator.zero_with_capacity(2);
        d_plus_one.append_monomial(Integer::from(1), &[0, 0]);
        d_plus_one.append_monomial(Integer::from(1), &[1, 0]);
        let zero_over_polynomial = RationalPolynomial {
            numerator: source.template.numerator.zero(),
            denominator: d_plus_one,
        };
        assert_eq!(
            source
                .project_parameter_free(&zero_over_polynomial, "m2", &target)
                .unwrap(),
            target.zero()
        );
    }

    #[test]
    fn rejects_dropped_parameter_dependence_in_both_polynomial_parts() {
        let source = CoefficientContext::new(["d", "m2"]);
        let target = CoefficientContext::new(["d"]);
        let d = source.parameter("d").unwrap();
        let m2 = source.parameter("m2").unwrap();

        let numerator_dependent = &d + &m2;
        assert!(matches!(
            source.project_parameter_free(&numerator_dependent, "m2", &target),
            Err(CoefficientProjectionError::DroppedParameterDependence {
                part: CoefficientPolynomialPart::Numerator,
                exponent: 1,
                ..
            })
        ));

        let denominator_dependent = &d / &m2;
        assert!(matches!(
            source.project_parameter_free(&denominator_dependent, "m2", &target),
            Err(CoefficientProjectionError::DroppedParameterDependence {
                part: CoefficientPolynomialPart::Denominator,
                exponent: 1,
                ..
            })
        ));
    }

    #[test]
    fn rejects_source_and_target_map_mismatches() {
        let source = CoefficientContext::new(["d", "m2"]);
        let target = CoefficientContext::new(["d"]);
        let foreign = CoefficientContext::new(["d", "mu2"]);

        assert!(matches!(
            source.project_parameter_free(&foreign.one(), "m2", &target),
            Err(CoefficientProjectionError::SourceVariableMapMismatch {
                part: CoefficientPolynomialPart::Numerator,
            })
        ));

        let mut foreign_denominator = source.one();
        foreign_denominator.denominator.variables = foreign.variables.clone();
        assert!(matches!(
            source.project_parameter_free(&foreign_denominator, "m2", &target),
            Err(CoefficientProjectionError::SourceVariableMapMismatch {
                part: CoefficientPolynomialPart::Denominator,
            })
        ));
        assert!(matches!(
            source.project_parameter_free(&source.one(), "m2", &CoefficientContext::new(["x"])),
            Err(CoefficientProjectionError::TargetParameterMismatch { position: 0, .. })
        ));

        let mut foreign_target_variable = target.clone();
        let foreign_target_map = CoefficientContext::new(["x"]).variables;
        foreign_target_variable.variables = foreign_target_map.clone();
        foreign_target_variable.template.numerator.variables = foreign_target_map.clone();
        foreign_target_variable.template.denominator.variables = foreign_target_map;
        assert!(matches!(
            source.project_parameter_free(&source.one(), "m2", &foreign_target_variable),
            Err(CoefficientProjectionError::TargetVariableMapMismatch { position: 0, .. })
        ));

        let foreign_template_map = CoefficientContext::new(["x"]).variables;
        let mut foreign_target_numerator_template = target.clone();
        foreign_target_numerator_template
            .template
            .numerator
            .variables = foreign_template_map.clone();
        assert!(matches!(
            source.project_parameter_free(&source.one(), "m2", &foreign_target_numerator_template,),
            Err(
                CoefficientProjectionError::TargetTemplateVariableMapMismatch {
                    part: CoefficientPolynomialPart::Numerator,
                }
            )
        ));

        let mut foreign_target_denominator_template = target.clone();
        foreign_target_denominator_template
            .template
            .denominator
            .variables = foreign_template_map;
        assert!(matches!(
            source.project_parameter_free(
                &source.one(),
                "m2",
                &foreign_target_denominator_template,
            ),
            Err(
                CoefficientProjectionError::TargetTemplateVariableMapMismatch {
                    part: CoefficientPolynomialPart::Denominator,
                }
            )
        ));
        assert!(matches!(
            source.project_parameter_free(
                &source.one(),
                "m2",
                &CoefficientContext::new(["d", "x"]),
            ),
            Err(CoefficientProjectionError::TargetParameterCount {
                expected: 1,
                actual: 2,
            })
        ));
        assert!(matches!(
            source.project_parameter_free(&source.one(), "mu2", &target),
            Err(CoefficientProjectionError::DroppedParameterNotFound(parameter))
                if parameter == "mu2"
        ));
    }

    #[test]
    fn rejects_malformed_exponent_layout_and_zero_denominator() {
        let source = CoefficientContext::new(["d", "m2"]);
        let target = CoefficientContext::new(["d"]);

        let mut malformed = source.one();
        malformed.numerator.exponents.push(0);
        assert!(matches!(
            source.project_parameter_free(&malformed, "m2", &target),
            Err(CoefficientProjectionError::MalformedExponentLayout {
                part: CoefficientPolynomialPart::Numerator,
                ..
            })
        ));

        let mut zero_denominator = source.one();
        zero_denominator.denominator.coefficients.clear();
        zero_denominator.denominator.exponents.clear();
        assert_eq!(
            source.project_parameter_free(&zero_denominator, "m2", &target),
            Err(CoefficientProjectionError::ZeroDenominator)
        );

        let mut zero_term_denominator = source.one();
        zero_term_denominator.denominator.coefficients[0] = Integer::from(0);
        assert_eq!(
            source.project_parameter_free(&zero_term_denominator, "m2", &target),
            Err(CoefficientProjectionError::ZeroDenominator)
        );
    }

    #[test]
    fn exact_authentication_rejects_malformed_sparse_polynomials_without_panicking() {
        let context = CoefficientContext::new(["x"]);

        let mut malformed_layout = context.one();
        malformed_layout.numerator.exponents.push(0);
        assert!(matches!(
            context.validate(&malformed_layout),
            Err(ExactAlgebraError::MalformedExponentLayout {
                part: CoefficientPolynomialPart::Numerator,
                ..
            })
        ));
        assert!(!context.contains(&malformed_layout));

        let mut explicit_zero = context.one();
        explicit_zero.numerator.coefficients[0] = Integer::from(0);
        assert!(matches!(
            context.validate(&explicit_zero),
            Err(ExactAlgebraError::ZeroCoefficient {
                part: CoefficientPolynomialPart::Numerator,
                term: 0,
            })
        ));

        let mut wrong_order = context.one();
        wrong_order.numerator.coefficients = vec![Integer::from(1), Integer::from(1)];
        wrong_order.numerator.exponents = vec![1, 0];
        assert!(matches!(
            context.validate(&wrong_order),
            Err(ExactAlgebraError::NonCanonicalMonomialOrder {
                part: CoefficientPolynomialPart::Numerator,
                term: 1,
            })
        ));
    }

    #[test]
    fn checked_exact_multiplication_reports_u16_exponent_overflow() {
        let context = CoefficientContext::new(["x"]);
        let maximal = context.parse("x^65535").unwrap();
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
    fn rational_normalization_can_densify_beyond_input_pair_counts() {
        let context = CoefficientContext::new(["x"]);
        let geometric_numerator = context.parse("x^8-1").unwrap();
        let linear = context.parse("x-1").unwrap();
        let reciprocal_linear = context.parse("1/(x-1)").unwrap();

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

        let left = context.parse("1/(x-1)").unwrap();
        let right = context.parse("(x^8-2)/(x-1)").unwrap();
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

    #[test]
    fn checked_polynomial_multiplication_covers_empty_maps_constants_and_zero() {
        let context = CoefficientContext::new(Vec::<String>::new());
        let zero = context.template.numerator.zero();
        let two = context.template.numerator.constant(Integer::from(2));
        let three = context.template.numerator.constant(Integer::from(3));
        let one_sparse_operation = ExactAlgebraLimits {
            max_polynomial_terms: 1,
            max_term_operations: 1,
            ..ExactAlgebraLimits::default()
        };

        let product = checked_polynomial_mul_on_map(
            &two,
            &three,
            &context.variables,
            one_sparse_operation,
            one_sparse_operation.max_polynomial_terms,
        )
        .unwrap();
        assert_eq!(
            product,
            context.template.numerator.constant(Integer::from(6))
        );

        let zero_product = checked_polynomial_mul_on_map(
            &zero,
            &three,
            &context.variables,
            ExactAlgebraLimits {
                max_term_operations: 0,
                ..one_sparse_operation
            },
            0,
        )
        .unwrap();
        assert!(zero_product.is_zero());
    }

    #[test]
    fn checked_polynomial_multiplication_rejects_invalid_configuration_and_payloads() {
        let context = CoefficientContext::new(Vec::<String>::new());
        let one = context.template.numerator.one();
        assert_eq!(
            checked_polynomial_mul_on_map(
                &one,
                &one,
                &context.variables,
                ExactAlgebraLimits {
                    max_exponent: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT + 1,
                    ..ExactAlgebraLimits::default()
                },
                ExactAlgebraLimits::default().max_polynomial_terms,
            ),
            Err(ExactAlgebraError::ConfiguredExponentLimit {
                requested: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT + 1,
                representation_limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            })
        );

        let foreign = CoefficientContext::new(["x"]);
        assert!(matches!(
            checked_polynomial_mul_on_map(
                &one,
                &foreign.template.numerator,
                &context.variables,
                ExactAlgebraLimits::default(),
                ExactAlgebraLimits::default().max_polynomial_terms,
            ),
            Err(ExactAlgebraError::VariableMapMismatch {
                part: CoefficientPolynomialPart::Numerator,
            })
        ));

        let mut malformed = one.clone();
        malformed.exponents.push(0);
        assert!(matches!(
            checked_polynomial_mul_on_map(
                &malformed,
                &one,
                &context.variables,
                ExactAlgebraLimits::default(),
                ExactAlgebraLimits::default().max_polynomial_terms,
            ),
            Err(ExactAlgebraError::MalformedExponentLayout {
                part: CoefficientPolynomialPart::Numerator,
                ..
            })
        ));
    }
}
