use std::fmt;

use crate::algebra::{ExactAlgebraError, IndexedAlgebraError};
use crate::family::IntegralKeyError;
use crate::sector;

/// Typed failure while preparing, eliminating, or replaying one anchored rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnchoredRuleError {
    EmptySourceRows,
    WrongAnchorArity {
        expected: usize,
        actual: usize,
    },
    WrongSourceContext {
        source_ordinal: usize,
    },
    WrongSourceFamily {
        source_ordinal: usize,
    },
    AnchorIndexOverflow {
        source_ordinal: usize,
        position: usize,
    },
    UnsatisfiedSourceCondition {
        source_ordinal: usize,
        condition_ordinal: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    NativePanic {
        operation: &'static str,
    },
    ReducerRejectedChronologicalRow {
        source_ordinal: usize,
    },
    ReducerInvariant {
        detail: &'static str,
    },
    NoStrictlyDescendingRule,
    ReplayMismatch {
        integral_column: usize,
    },
    IndexedAlgebra(IndexedAlgebraError),
    ExactAlgebra(ExactAlgebraError),
    IntegralKey(IntegralKeyError),
    Ordering(sector::Error),
}

impl fmt::Display for AnchoredRuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySourceRows => formatter.write_str("anchored elimination needs at least one source row"),
            Self::WrongAnchorArity { expected, actual } => write!(
                formatter,
                "anchor arity is {actual}, expected {expected}"
            ),
            Self::WrongSourceContext { source_ordinal } => write!(
                formatter,
                "source row {source_ordinal} uses a foreign indexed coefficient context"
            ),
            Self::WrongSourceFamily { source_ordinal } => write!(
                formatter,
                "source row {source_ordinal} belongs to a different family"
            ),
            Self::AnchorIndexOverflow {
                source_ordinal,
                position,
            } => write!(
                formatter,
                "source row {source_ordinal} overflows anchored index position {position}"
            ),
            Self::UnsatisfiedSourceCondition {
                source_ordinal,
                condition_ordinal,
            } => write!(
                formatter,
                "source row {source_ordinal} condition {condition_ordinal} specializes identically to zero"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(formatter, "could not reserve {requested} units for {resource}"),
            Self::NativePanic { operation } => {
                write!(formatter, "Symbolica panicked while performing {operation}")
            }
            Self::ReducerRejectedChronologicalRow { source_ordinal } => write!(
                formatter,
                "Symbolica's sparse reducer rejected identity-augmented source row {source_ordinal}"
            ),
            Self::ReducerInvariant { detail } => {
                write!(formatter, "Symbolica sparse-reducer invariant failed: {detail}")
            }
            Self::NoStrictlyDescendingRule => formatter.write_str(
                "the anchored source rows contain no pivot with a nonempty strictly lower right-hand side",
            ),
            Self::ReplayMismatch { integral_column } => write!(
                formatter,
                "exact source-row replay differs at integral column {integral_column}"
            ),
            Self::IndexedAlgebra(error) => error.fmt(formatter),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::IntegralKey(error) => error.fmt(formatter),
            Self::Ordering(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AnchoredRuleError {}

impl From<IndexedAlgebraError> for AnchoredRuleError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::IndexedAlgebra(value)
    }
}

impl From<ExactAlgebraError> for AnchoredRuleError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

impl From<IntegralKeyError> for AnchoredRuleError {
    fn from(value: IntegralKeyError) -> Self {
        Self::IntegralKey(value)
    }
}

impl From<sector::Error> for AnchoredRuleError {
    fn from(value: sector::Error) -> Self {
        Self::Ordering(value)
    }
}
