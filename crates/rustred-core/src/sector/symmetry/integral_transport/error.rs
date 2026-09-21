use std::fmt;

use super::ExpansionError;

/// Typed admission failures; no partial combination is returned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    FamilyMismatch {
        side: &'static str,
    },
    ForeignCoefficientContext,
    DimensionMismatch,
    RootArity {
        side: &'static str,
        expected: usize,
        actual: usize,
    },
    NonUnitJacobian,
    AnalyticPowerShift {
        side: &'static str,
        axis: usize,
    },
    UnresolvedCondition {
        ordinal: usize,
    },
    NonconstantCoefficient {
        row: usize,
        column: Option<usize>,
    },
    NonUnitActiveRow {
        axis: usize,
    },
    ActiveBijection,
    WrongInputArity {
        expected: usize,
        actual: usize,
    },
    PositiveOutsideRoot {
        axis: usize,
        power: i64,
    },
    Expansion(ExpansionError),
}

impl From<ExpansionError> for Error {
    fn from(value: ExpansionError) -> Self {
        Self::Expansion(value)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FamilyMismatch { side } => write!(f, "verified transport {side} family differs"),
            Self::ForeignCoefficientContext => {
                write!(f, "transport source and target coefficient contexts differ")
            }
            Self::DimensionMismatch => write!(
                f,
                "transport source and target integration dimensions differ"
            ),
            Self::RootArity {
                side,
                expected,
                actual,
            } => write!(
                f,
                "transport {side} root arity {actual}, expected {expected}"
            ),
            Self::NonUnitJacobian => write!(f, "transport requires a unit loop Jacobian"),
            Self::AnalyticPowerShift { side, axis } => write!(
                f,
                "transport does not admit analytic power shift on {side} axis {axis}"
            ),
            Self::UnresolvedCondition { ordinal } => write!(
                f,
                "transport map condition {ordinal} is not an unconditional nonzero constant"
            ),
            Self::NonconstantCoefficient { row, column } => write!(
                f,
                "transport row {row}, coefficient {column:?} is not rational-constant"
            ),
            Self::NonUnitActiveRow { axis } => write!(
                f,
                "transport active axis {axis} does not map to one unit-scaled denominator"
            ),
            Self::ActiveBijection => write!(
                f,
                "transport active denominator map is not a root bijection"
            ),
            Self::WrongInputArity { expected, actual } => {
                write!(f, "transport input arity {actual}, expected {expected}")
            }
            Self::PositiveOutsideRoot { axis, power } => write!(
                f,
                "transport input power {power} is positive outside root at axis {axis}"
            ),
            Self::Expansion(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Expansion(error) => Some(error),
            _ => None,
        }
    }
}
