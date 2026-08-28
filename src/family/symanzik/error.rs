//! Failures from checked Symanzik-polynomial construction.

use std::fmt;

use crate::algebra::ExactAlgebraError;

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
                write!(
                    formatter,
                    "Feynman-polynomial internal verification failed: {detail}"
                )
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
