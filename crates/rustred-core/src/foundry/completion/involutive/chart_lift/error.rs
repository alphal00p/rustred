use std::fmt;

use crate::algebra::IndexedAlgebraError;
use crate::identity::ParametricRelationError;

use super::super::InvolutiveError;

/// Typed failures at the ordinary-source/Ore-chart trust boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum OrdinaryChartLiftError {
    SourceLayout {
        actual: &'static str,
    },
    EmptySourceRows,
    EmptySourceRelation {
        source_ordinal: usize,
    },
    ContextMismatch,
    ForeignSourceOwner,
    SourceOrdinalOutOfRange {
        source_ordinal: usize,
        source_rows: usize,
    },
    SourceRowMismatch {
        source_ordinal: usize,
    },
    Relation(ParametricRelationError),
    Involutive(InvolutiveError),
}

impl fmt::Display for OrdinaryChartLiftError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceLayout { actual } => write!(
                formatter,
                "ordinary Ore-chart lifting requires the complete ordinary source layout, got {actual}"
            ),
            Self::EmptySourceRows => {
                formatter.write_str("ordinary Ore-chart lifting received no source rows")
            }
            Self::EmptySourceRelation { source_ordinal } => write!(
                formatter,
                "ordinary source row {source_ordinal} is the zero relation"
            ),
            Self::ContextMismatch => formatter.write_str(
                "ordinary source rows and Ore-chart coefficients belong to different indexed contexts",
            ),
            Self::ForeignSourceOwner => formatter.write_str(
                "ordinary Ore-chart provenance was replayed against a different sealed source owner",
            ),
            Self::SourceOrdinalOutOfRange {
                source_ordinal,
                source_rows,
            } => write!(
                formatter,
                "ordinary source ordinal {source_ordinal} is outside the retained {source_rows} rows"
            ),
            Self::SourceRowMismatch { source_ordinal } => write!(
                formatter,
                "ordinary source row identity changed at ordinal {source_ordinal}"
            ),
            Self::Relation(error) => error.fmt(formatter),
            Self::Involutive(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for OrdinaryChartLiftError {}

impl From<ParametricRelationError> for OrdinaryChartLiftError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

impl From<InvolutiveError> for OrdinaryChartLiftError {
    fn from(value: InvolutiveError) -> Self {
        Self::Involutive(value)
    }
}

impl From<IndexedAlgebraError> for OrdinaryChartLiftError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::Involutive(InvolutiveError::Algebra(value))
    }
}
