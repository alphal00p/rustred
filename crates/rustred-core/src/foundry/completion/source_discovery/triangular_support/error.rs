use std::fmt;

use crate::foundry::completion::frame::PhysicalFrameError;
use crate::identity::TranslatedSourceError;

use super::super::CampaignError;

/// Typed failures while producing one bounded triangular selected-source
/// frame. Exhausting a resource is only a construction stop and carries no
/// terminal or closure meaning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TriangularSupportError {
    WrongSourceLayout {
        actual: &'static str,
    },
    WrongSectorArity {
        expected: usize,
        actual: usize,
    },
    WrongSourceCeilingCount {
        expected: usize,
        actual: usize,
    },
    AxisOutOfRange {
        position: usize,
        axis: usize,
        arity: usize,
    },
    DuplicateAxis {
        first_position: usize,
        duplicate_position: usize,
        axis: usize,
    },
    DegreeNotRepresentable {
        source_ordinal: usize,
        degree: usize,
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
    Shift(TranslatedSourceError),
    RequestAccumulation(CampaignError),
    SourceTranslation(TranslatedSourceError),
    PhysicalFrame(PhysicalFrameError),
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for TriangularSupportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSourceLayout { actual } => write!(
                formatter,
                "triangular support requires complete ordinary IBP sources, got {actual}"
            ),
            Self::WrongSectorArity { expected, actual } => write!(
                formatter,
                "triangular-support sector has arity {actual}, expected {expected}"
            ),
            Self::WrongSourceCeilingCount { expected, actual } => write!(
                formatter,
                "triangular support has {actual} source ceilings, expected {expected}"
            ),
            Self::AxisOutOfRange {
                position,
                axis,
                arity,
            } => write!(
                formatter,
                "triangular-support axis {axis} at position {position} is outside 0..{arity}"
            ),
            Self::DuplicateAxis {
                first_position,
                duplicate_position,
                axis,
            } => write!(
                formatter,
                "triangular-support axis {axis} is repeated at positions {first_position} and {duplicate_position}"
            ),
            Self::DegreeNotRepresentable {
                source_ordinal,
                degree,
            } => write!(
                formatter,
                "triangular-support degree {degree} for source {source_ordinal} cannot be represented by an i64 shift"
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
            Self::Shift(error) => {
                write!(
                    formatter,
                    "could not retain a triangular-support shift: {error}"
                )
            }
            Self::RequestAccumulation(error) => write!(
                formatter,
                "could not canonicalize triangular-support requests: {error}"
            ),
            Self::SourceTranslation(error) => write!(
                formatter,
                "could not translate triangular-support requests: {error}"
            ),
            Self::PhysicalFrame(error) => write!(
                formatter,
                "could not assemble the triangular selected-source frame: {error}"
            ),
            Self::Invariant { detail } => {
                write!(formatter, "triangular-support invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for TriangularSupportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Shift(error) | Self::SourceTranslation(error) => Some(error),
            Self::RequestAccumulation(error) => Some(error),
            Self::PhysicalFrame(error) => Some(error),
            _ => None,
        }
    }
}
