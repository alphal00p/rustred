use std::error::Error as StdError;
use std::fmt;

use symbolica::prelude::Atom;

use crate::algebra::{CoefficientContextError, ExactAlgebraError};

use super::SCALAR_PRODUCT_NAME;

/// Typed failures at the untrusted Symbolica-expression boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolicaAffineDenominatorError {
    CoefficientContext(CoefficientContextError),
    ExactAlgebra(ExactAlgebraError),
    Parse(String),
    NoLoopMomenta,
    EmptyLabel {
        role: &'static str,
        position: usize,
    },
    DuplicateLabel {
        label: String,
        first_role: &'static str,
        second_role: &'static str,
    },
    ReservedLabel(String),
    ImpureDeclaredSymbol {
        label: String,
        violation: &'static str,
    },
    WrongExternalGramRowCount {
        expected: usize,
        actual: usize,
    },
    WrongExternalGramColumnCount {
        row: usize,
        expected: usize,
        actual: usize,
    },
    AsymmetricExternalGram {
        row: usize,
        column: usize,
    },
    InvalidExternalGramCoefficient {
        row: usize,
        column: usize,
        error: ExactAlgebraError,
    },
    CombinedVariableMapMismatch {
        position: usize,
        label: String,
    },
    UnsupportedCombinedVariable {
        position: usize,
        label: String,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    UnknownSymbol(Atom),
    UnsupportedFunction(Atom),
    MalformedScalarProduct {
        atom: Atom,
        arguments: usize,
    },
    NestedScalarProduct(Atom),
    InvalidScalarProductArgument {
        argument: usize,
        atom: Atom,
    },
    UnsupportedPower(Atom),
    NegativeMomentumPower {
        atom: Atom,
        exponent: i64,
    },
    UnsupportedNumericAtom(Atom),
    MomentumDependentRationalDenominator,
    MomentumDegreeOne {
        numerator_term: usize,
    },
    MomentumDegreeTooHigh {
        numerator_term: usize,
        degree: u32,
    },
    InvalidQuadraticMomentumMonomial {
        numerator_term: usize,
    },
    BaseCoefficientContainsMomentum,
    NormalizedExpressionTooLarge {
        requested: usize,
        limit: usize,
    },
    SymbolicaPanic {
        stage: &'static str,
    },
    InternalVerificationFailure {
        detail: &'static str,
    },
}

impl fmt::Display for SymbolicaAffineDenominatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoefficientContext(error) => {
                write!(formatter, "invalid coefficient context: {error}")
            }
            Self::ExactAlgebra(error) => {
                write!(formatter, "exact coefficient arithmetic failed: {error}")
            }
            Self::Parse(error) => {
                write!(formatter, "could not parse Symbolica denominator: {error}")
            }
            Self::NoLoopMomenta => formatter
                .write_str("an affine denominator compiler needs at least one loop momentum"),
            Self::EmptyLabel { role, position } => {
                write!(formatter, "{role} label {position} is empty")
            }
            Self::DuplicateLabel {
                label,
                first_role,
                second_role,
            } => write!(
                formatter,
                "label {label:?} is used by both {first_role} and {second_role} declarations"
            ),
            Self::ReservedLabel(label) => write!(
                formatter,
                "label {label:?} collides with the reserved scalar-product head {SCALAR_PRODUCT_NAME}"
            ),
            Self::ImpureDeclaredSymbol { label, violation } => write!(
                formatter,
                "declared Symbolica symbol {label:?} is not a plain authenticated symbol: {violation}"
            ),
            Self::WrongExternalGramRowCount { expected, actual } => write!(
                formatter,
                "external Gram matrix has {actual} rows, expected {expected}"
            ),
            Self::WrongExternalGramColumnCount {
                row,
                expected,
                actual,
            } => write!(
                formatter,
                "external Gram row {row} has {actual} columns, expected {expected}"
            ),
            Self::AsymmetricExternalGram { row, column } => write!(
                formatter,
                "external Gram entries ({row},{column}) and ({column},{row}) differ"
            ),
            Self::InvalidExternalGramCoefficient { row, column, error } => write!(
                formatter,
                "external Gram entry ({row},{column}) is invalid: {error}"
            ),
            Self::CombinedVariableMapMismatch { position, label } => write!(
                formatter,
                "combined Symbolica variable {position} for {label:?} does not preserve the base coefficient map"
            ),
            Self::UnsupportedCombinedVariable { position, label } => write!(
                formatter,
                "combined Symbolica variable {position} for {label:?} is not a plain symbol"
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
                write!(formatter, "{resource} count overflowed its representation")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} units for {resource}"
            ),
            Self::UnknownSymbol(atom) => {
                write!(formatter, "denominator contains undeclared symbol {atom}")
            }
            Self::UnsupportedFunction(atom) => write!(
                formatter,
                "denominator contains unsupported function {atom}"
            ),
            Self::MalformedScalarProduct { atom, arguments } => write!(
                formatter,
                "scalar product {atom} has {arguments} arguments, expected 2"
            ),
            Self::NestedScalarProduct(atom) => write!(
                formatter,
                "scalar-product argument contains nested scalar product {atom}"
            ),
            Self::InvalidScalarProductArgument { argument, atom } => write!(
                formatter,
                "scalar-product argument {argument} is not homogeneous and vector-linear: {atom}"
            ),
            Self::UnsupportedPower(atom) => write!(
                formatter,
                "denominator contains unsupported noninteger or oversized power {atom}"
            ),
            Self::NegativeMomentumPower { atom, exponent } => write!(
                formatter,
                "momentum-dependent base {atom} has negative power {exponent}"
            ),
            Self::UnsupportedNumericAtom(atom) => write!(
                formatter,
                "numeric atom {atom} is not an exact rational number"
            ),
            Self::MomentumDependentRationalDenominator => formatter
                .write_str("the rational denominator depends on a loop or external momentum"),
            Self::MomentumDegreeOne { numerator_term } => write!(
                formatter,
                "expanded numerator term {numerator_term} has momentum degree 1; denominators must be affine in scalar products"
            ),
            Self::MomentumDegreeTooHigh {
                numerator_term,
                degree,
            } => write!(
                formatter,
                "expanded numerator term {numerator_term} has momentum degree {degree}, above 2"
            ),
            Self::InvalidQuadraticMomentumMonomial { numerator_term } => write!(
                formatter,
                "expanded numerator term {numerator_term} is not a quadratic momentum monomial"
            ),
            Self::BaseCoefficientContainsMomentum => formatter
                .write_str("base coefficient expression contains a loop or external momentum"),
            Self::NormalizedExpressionTooLarge { requested, limit } => write!(
                formatter,
                "normalized expression retains {requested} bytes, exceeding the configured limit {limit}"
            ),
            Self::SymbolicaPanic { stage } => write!(
                formatter,
                "Symbolica panicked during affine-denominator {stage}"
            ),
            Self::InternalVerificationFailure { detail } => write!(
                formatter,
                "internal affine-denominator verification failed: {detail}"
            ),
        }
    }
}

impl StdError for SymbolicaAffineDenominatorError {}

impl From<CoefficientContextError> for SymbolicaAffineDenominatorError {
    fn from(value: CoefficientContextError) -> Self {
        Self::CoefficientContext(value)
    }
}

impl From<ExactAlgebraError> for SymbolicaAffineDenominatorError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}
