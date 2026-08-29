use std::fmt;

use super::super::super::relation::ParametricRelationError;

/// Typed failures while constructing or applying a translated-source plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranslatedSourceError {
    EmptyIntegralShift,
    EmptySourceRows,
    EmptyOffsets,
    WrongOffsetArity {
        offset_ordinal: usize,
        expected: usize,
        actual: usize,
    },
    CompletedSourceFamilyMismatch,
    CompletedSourceContextMismatch,
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
    RelationTranslation {
        offset_ordinal: usize,
        source_ordinal: usize,
        error: ParametricRelationError,
    },
}

impl fmt::Display for TranslatedSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIntegralShift => formatter.write_str("an integral shift cannot be empty"),
            Self::EmptySourceRows => formatter
                .write_str("translated-source construction needs a nonempty sealed source batch"),
            Self::EmptyOffsets => {
                formatter.write_str("translated-source construction needs at least one offset")
            }
            Self::WrongOffsetArity {
                offset_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "translation offset {offset_ordinal} has arity {actual}, expected {expected}"
            ),
            Self::CompletedSourceFamilyMismatch => formatter
                .write_str("the completed source batch belongs to a different integral family"),
            Self::CompletedSourceContextMismatch => formatter.write_str(
                "the completed source batch uses a different indexed coefficient context",
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
            } => write!(
                formatter,
                "could not reserve {requested} units for {resource}"
            ),
            Self::RelationTranslation {
                offset_ordinal,
                source_ordinal,
                error,
            } => write!(
                formatter,
                "could not translate source row {source_ordinal} at canonical offset {offset_ordinal}: {error}"
            ),
        }
    }
}

impl std::error::Error for TranslatedSourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RelationTranslation { error, .. } => Some(error),
            _ => None,
        }
    }
}
