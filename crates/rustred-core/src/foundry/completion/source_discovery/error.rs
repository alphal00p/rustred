use std::fmt;

use crate::identity::TranslatedSourceError;

/// Typed failures at the bounded inverse-incidence boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SourceDiscoveryError {
    WrongSourceLayout {
        actual: &'static str,
    },
    ScopeMismatch {
        detail: &'static str,
    },
    WrongArity {
        object: &'static str,
        expected: usize,
        actual: usize,
    },
    ShiftOverflow {
        support_ordinal: usize,
        source_ordinal: usize,
        term_ordinal: usize,
        position: usize,
        support: i64,
        source_shift: i64,
    },
    ShiftConstruction(TranslatedSourceError),
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
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for SourceDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSourceLayout { actual } => write!(
                formatter,
                "source discovery requires the complete ordinary IBP source layout, got {actual}"
            ),
            Self::ScopeMismatch { detail } => {
                write!(formatter, "source-discovery scope mismatch: {detail}")
            }
            Self::WrongArity {
                object,
                expected,
                actual,
            } => write!(
                formatter,
                "source-discovery {object} has arity {actual}, expected {expected}"
            ),
            Self::ShiftOverflow {
                support_ordinal,
                source_ordinal,
                term_ordinal,
                position,
                support,
                source_shift,
            } => write!(
                formatter,
                "inverse incidence {support}-{source_shift} overflowed at support {support_ordinal}, source {source_ordinal}, term {term_ordinal}, component {position}"
            ),
            Self::ShiftConstruction(error) => {
                write!(
                    formatter,
                    "could not retain an incident translation offset: {error}"
                )
            }
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
            Self::Invariant { detail } => {
                write!(formatter, "source-discovery invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for SourceDiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ShiftConstruction(error) => Some(error),
            _ => None,
        }
    }
}
