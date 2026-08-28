//! Typed failures at the input-compilation and exact-lowering boundaries.

use std::fmt;

use symbolica::atom::Atom;

use crate::algebra::CoefficientContextError;
use crate::family::IntegralFamilyError;
use crate::symbolica_affine_denominator::SymbolicaAffineDenominatorError;

/// Typed failures while lowering normalized syntax to an exact family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoweringError {
    CoefficientContext(CoefficientContextError),
    AffineDenominator(SymbolicaAffineDenominatorError),
    IntegralFamily(IntegralFamilyError),
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    SymbolicaPanic {
        operation: &'static str,
    },
}

impl fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoefficientContext(error) => {
                write!(formatter, "invalid coefficient context: {error}")
            }
            Self::AffineDenominator(error) => {
                write!(formatter, "Symbolica affine lowering failed: {error}")
            }
            Self::IntegralFamily(error) => {
                write!(formatter, "integral-family authentication failed: {error}")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not allocate {requested} units for {resource}"
            ),
            Self::SymbolicaPanic { operation } => {
                write!(formatter, "Symbolica panicked during {operation}")
            }
        }
    }
}

impl std::error::Error for LoweringError {}

impl From<CoefficientContextError> for LoweringError {
    fn from(error: CoefficientContextError) -> Self {
        Self::CoefficientContext(error)
    }
}

impl From<SymbolicaAffineDenominatorError> for LoweringError {
    fn from(error: SymbolicaAffineDenominatorError) -> Self {
        Self::AffineDenominator(error)
    }
}

impl From<IntegralFamilyError> for LoweringError {
    fn from(error: IntegralFamilyError) -> Self {
        Self::IntegralFamily(error)
    }
}

/// Typed syntax and normalization failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    SymbolicaPanic {
        operation: &'static str,
    },
    Parse(String),
    UnsupportedToken {
        detail: String,
    },
    UnsafeRegisteredSymbol {
        symbol: String,
        reason: &'static str,
    },
    GrammarSymbol {
        name: &'static str,
        detail: String,
    },
    AttributedGrammarHead {
        name: &'static str,
    },
    WrongRoot,
    RootPatternMismatch,
    AmbiguousPattern {
        clause: usize,
    },
    UnknownClause {
        clause: usize,
        expression: Atom,
    },
    WrongClauseArity {
        clause: usize,
        kind: &'static str,
        expected: &'static str,
        actual: usize,
    },
    MissingClause {
        kind: &'static str,
    },
    DuplicateClause {
        kind: &'static str,
    },
    InvalidLabel {
        role: &'static str,
        expression: Atom,
    },
    InvalidLabelText {
        role: &'static str,
        label: String,
    },
    ReservedLabel {
        role: &'static str,
        label: String,
    },
    DuplicateLabel {
        role: &'static str,
        label: String,
    },
    CrossClassLabelCollision {
        label: String,
    },
    NoLoopMomenta,
    WrongPropagatorCount {
        expected: usize,
        actual: usize,
    },
    InvalidTargetPower {
        denominator: String,
        expression: Atom,
    },
    DuplicatePowerShift {
        denominator: String,
    },
    UnknownPowerShift {
        denominator: String,
    },
    UnknownExternalGramMomentum {
        momentum: String,
    },
    DiagonalGramOrientation,
    DuplicateExternalGram {
        left: String,
        right: String,
    },
    MissingExternalGram {
        left: String,
        right: String,
    },
    ForeignScalarSymbol {
        symbol: String,
    },
    ReservedScalarSymbol {
        symbol: String,
    },
    IdentifierUsedAsScalar {
        symbol: String,
    },
    UndeclaredScalarSymbol {
        symbol: String,
    },
    ConflictingParameterOverride,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not allocate {requested} units for {resource}"
            ),
            Self::SymbolicaPanic { operation } => {
                write!(formatter, "Symbolica panicked during {operation}")
            }
            Self::Parse(detail) => {
                write!(
                    formatter,
                    "could not parse Symbolica integral input: {detail}"
                )
            }
            Self::UnsupportedToken { detail } => {
                write!(formatter, "unsupported Symbolica token: {detail}")
            }
            Self::UnsafeRegisteredSymbol { symbol, reason } => write!(
                formatter,
                "registered Symbolica symbol {symbol} is unsafe for RustRed input: {reason}"
            ),
            Self::GrammarSymbol { name, detail } => {
                write!(
                    formatter,
                    "could not register grammar head {name}: {detail}"
                )
            }
            Self::AttributedGrammarHead { name } => write!(
                formatter,
                "grammar head {name} must be an un-attributed plain Symbolica symbol"
            ),
            Self::WrongRoot => formatter.write_str("compact input must have the exact root I(...)"),
            Self::RootPatternMismatch => formatter.write_str(
                "compact I(...) root failed strict whole-expression pattern authentication",
            ),
            Self::AmbiguousPattern { clause } => write!(
                formatter,
                "I clause {clause} matched more than one grammar production"
            ),
            Self::UnknownClause { clause, expression } => {
                write!(formatter, "unknown I clause {clause}: {expression}")
            }
            Self::WrongClauseArity {
                clause,
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "I clause {clause} ({kind}) has {actual} arguments, expected {expected}"
            ),
            Self::MissingClause { kind } => {
                write!(
                    formatter,
                    "compact I input is missing required {kind}(...) clause"
                )
            }
            Self::DuplicateClause { kind } => {
                write!(
                    formatter,
                    "compact I input repeats singleton {kind}(...) clause"
                )
            }
            Self::InvalidLabel { role, expression } => write!(
                formatter,
                "{role} must be an unqualified Symbolica symbol, found {expression}"
            ),
            Self::InvalidLabelText { role, label } => {
                write!(formatter, "invalid {role} label {label:?}")
            }
            Self::ReservedLabel { role, label } => {
                write!(
                    formatter,
                    "{role} label {label:?} is reserved by the v1 grammar"
                )
            }
            Self::DuplicateLabel { role, label } => {
                write!(formatter, "{role} label {label:?} is repeated")
            }
            Self::CrossClassLabelCollision { label } => write!(
                formatter,
                "label {label:?} is reused across incompatible input classes"
            ),
            Self::NoLoopMomenta => {
                formatter.write_str("loops(...) must contain at least one loop momentum")
            }
            Self::WrongPropagatorCount { expected, actual } => write!(
                formatter,
                "complete family needs {expected} propagators, found {actual}"
            ),
            Self::InvalidTargetPower {
                denominator,
                expression,
            } => write!(
                formatter,
                "target power for {denominator} is not an exact i64 integer: {expression}"
            ),
            Self::DuplicatePowerShift { denominator } => {
                write!(formatter, "power_shift for {denominator} is repeated")
            }
            Self::UnknownPowerShift { denominator } => {
                write!(
                    formatter,
                    "power_shift refers to unknown propagator {denominator}"
                )
            }
            Self::UnknownExternalGramMomentum { momentum } => {
                write!(
                    formatter,
                    "Gram clause refers to unknown external momentum {momentum}"
                )
            }
            Self::DiagonalGramOrientation => {
                formatter.write_str("internal diagonal Gram orientation failure")
            }
            Self::DuplicateExternalGram { left, right } => write!(
                formatter,
                "external Gram entry ({left},{right}) is repeated, including reversed duplicates"
            ),
            Self::MissingExternalGram { left, right } => {
                write!(formatter, "external Gram entry ({left},{right}) is missing")
            }
            Self::ForeignScalarSymbol { symbol } => {
                write!(
                    formatter,
                    "scalar symbol {symbol} is outside the rustred namespace"
                )
            }
            Self::ReservedScalarSymbol { symbol } => {
                write!(
                    formatter,
                    "scalar symbol {symbol} is reserved by the input grammar"
                )
            }
            Self::IdentifierUsedAsScalar { symbol } => write!(
                formatter,
                "family identifier {symbol} cannot also be used as a scalar parameter"
            ),
            Self::UndeclaredScalarSymbol { symbol } => {
                write!(
                    formatter,
                    "scalar symbol {symbol} is not present in parameters(...)"
                )
            }
            Self::ConflictingParameterOverride => {
                formatter.write_str("hybrid TOML parameter override conflicts with parameters(...)")
            }
        }
    }
}

impl std::error::Error for Error {}
