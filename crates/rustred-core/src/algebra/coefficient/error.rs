use std::fmt;

use super::CoefficientPolynomialPart;

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

/// Typed failures from RustRed's checked exact-algebra boundary.
///
/// Admission failures occur before native entry. Native Symbolica panics are
/// contained where the checked operation can do so safely, and every
/// successful native result is authenticated before it leaves the boundary.
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
    NativePanic {
        operation: &'static str,
    },
    NonExactPolynomialDivision {
        operation: &'static str,
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
            Self::NativePanic { operation } => {
                write!(formatter, "Symbolica panicked while {operation}")
            }
            Self::NonExactPolynomialDivision { operation } => write!(
                formatter,
                "Symbolica found no exact polynomial quotient while {operation}"
            ),
        }
    }
}

impl std::error::Error for ExactAlgebraError {}

/// Typed failures produced before constructing a Symbolica polynomial map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoefficientContextError {
    DuplicateParameter(String),
    InvalidParameter { name: String, reason: String },
    ParameterSymbolCollision { name: String },
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
            Self::ParameterSymbolCollision { name } => write!(
                formatter,
                "coefficient parameter {name:?} collides with an unsafe process-global Symbolica symbol"
            ),
        }
    }
}

impl std::error::Error for CoefficientContextError {}
