use std::fmt;

use crate::algebra::IndexedAlgebraError;
use crate::family::IntegralFamilyError;

use super::super::condition::IdentityConditionError;
use super::super::relation::ParametricRelationError;

/// Typed failures from generic parametric IBP/LI generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricIbpError {
    RowCountOverflow {
        loops: usize,
        externals: usize,
    },
    RowOrdinalOutOfRange {
        batch: &'static str,
        ordinal: usize,
        rows: usize,
    },
    WrongSourceRowCount {
        batch: &'static str,
        expected: usize,
        actual: usize,
    },
    SourceRowLayoutMismatch {
        position: usize,
        expected: &'static str,
        actual: &'static str,
    },
    SourceRowScopeMismatch {
        batch: &'static str,
        position: usize,
    },
    SourceRowOrdinalMismatch {
        batch: &'static str,
        position: usize,
        actual: usize,
    },
    CompletedSourceScopeMismatch,
    IdentityCondition(IdentityConditionError),
    Coefficient(IndexedAlgebraError),
    Relation(ParametricRelationError),
    Family(IntegralFamilyError),
}

impl fmt::Display for ParametricIbpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RowCountOverflow { loops, externals } => write!(
                formatter,
                "the IBP/LI row count for {loops} loops and {externals} external momenta overflowed usize"
            ),
            Self::RowOrdinalOutOfRange {
                batch,
                ordinal,
                rows,
            } => write!(
                formatter,
                "{batch} row ordinal {ordinal} is outside the prepared row count {rows}"
            ),
            Self::WrongSourceRowCount {
                batch,
                expected,
                actual,
            } => write!(
                formatter,
                "{batch} completion received {actual} rows, expected {expected}"
            ),
            Self::SourceRowLayoutMismatch {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "source row at completion position {position} uses {actual} layout, expected {expected}"
            ),
            Self::SourceRowScopeMismatch { batch, position } => write!(
                formatter,
                "{batch} row at completion position {position} has a foreign semantic source scope"
            ),
            Self::SourceRowOrdinalMismatch {
                batch,
                position,
                actual,
            } => write!(
                formatter,
                "{batch} completion position {position} received row ordinal {actual}"
            ),
            Self::CompletedSourceScopeMismatch => formatter
                .write_str("completed IBP source rows use a foreign family or coefficient context"),
            Self::IdentityCondition(error) => error.fmt(formatter),
            Self::Coefficient(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Family(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ParametricIbpError {}

impl From<IndexedAlgebraError> for ParametricIbpError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::Coefficient(value)
    }
}

impl From<IdentityConditionError> for ParametricIbpError {
    fn from(value: IdentityConditionError) -> Self {
        Self::IdentityCondition(value)
    }
}

impl From<ParametricRelationError> for ParametricIbpError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

impl From<IntegralFamilyError> for ParametricIbpError {
    fn from(value: IntegralFamilyError) -> Self {
        Self::Family(value)
    }
}
