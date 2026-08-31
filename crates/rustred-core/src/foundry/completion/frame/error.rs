use std::fmt;

use crate::identity::TranslatedSourceError;

/// Typed failures while constructing a bounded physical translated-source
/// frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PhysicalFrameError {
    WrongSourceLayout {
        actual: &'static str,
    },
    WrongSectorArity {
        expected: usize,
        actual: usize,
    },
    WrongSourceOffsetArity {
        row: usize,
        expected: usize,
        actual: usize,
    },
    WrongSourceTermArity {
        row: usize,
        expected: usize,
        actual: usize,
    },
    DegreeNotRepresentable {
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
    U32NotRepresentable {
        resource: &'static str,
        value: usize,
    },
    IntegralShift(TranslatedSourceError),
    TranslatedSources(TranslatedSourceError),
    ZeroSourceTerm {
        row: usize,
    },
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for PhysicalFrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSourceLayout { actual } => write!(
                formatter,
                "physical-frame construction requires the complete ordinary IBP source layout, got {actual}"
            ),
            Self::WrongSectorArity { expected, actual } => write!(
                formatter,
                "physical-frame sector has arity {actual}, expected {expected}"
            ),
            Self::WrongSourceOffsetArity {
                row,
                expected,
                actual,
            } => write!(
                formatter,
                "physical-frame source row {row} has provenance-offset arity {actual}, expected {expected}"
            ),
            Self::WrongSourceTermArity {
                row,
                expected,
                actual,
            } => write!(
                formatter,
                "physical-frame source row {row} has term-shift arity {actual}, expected {expected}"
            ),
            Self::DegreeNotRepresentable { degree } => write!(
                formatter,
                "physical-frame degree {degree} cannot be represented by an i64 shift"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}"
            ),
            Self::U32NotRepresentable { resource, value } => {
                write!(formatter, "{resource} value {value} does not fit u32")
            }
            Self::IntegralShift(error) => {
                write!(
                    formatter,
                    "could not construct a physical-frame shift: {error}"
                )
            }
            Self::TranslatedSources(error) => {
                write!(
                    formatter,
                    "could not translate the physical-frame sources: {error}"
                )
            }
            Self::ZeroSourceTerm { row } => {
                write!(
                    formatter,
                    "physical-frame source row {row} retained a zero term"
                )
            }
            Self::Invariant { detail } => {
                write!(formatter, "physical-frame invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for PhysicalFrameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IntegralShift(error) | Self::TranslatedSources(error) => Some(error),
            _ => None,
        }
    }
}
