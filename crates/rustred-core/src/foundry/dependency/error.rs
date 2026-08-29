use std::fmt;

use crate::sector;

/// Typed failure while planning or streaming proper-subsector obligations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricDependencyError {
    RuleHasNoSectorMonotoneAdmission,
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
    InvalidCursor,
    Invariant {
        detail: &'static str,
    },
    Sector(sector::Error),
}

impl fmt::Display for ParametricDependencyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuleHasNoSectorMonotoneAdmission => formatter
                .write_str("proper-subsector discovery requires a sector-monotone parametric rule"),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requires {requested} work units, exceeding limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} bounded entries for {resource}"
            ),
            Self::InvalidCursor => formatter.write_str(
                "the proper-subsector cursor is not a valid boundary of this discovery plan",
            ),
            Self::Invariant { detail } => {
                write!(
                    formatter,
                    "proper-subsector discovery invariant failed: {detail}"
                )
            }
            Self::Sector(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ParametricDependencyError {}

impl From<sector::Error> for ParametricDependencyError {
    fn from(value: sector::Error) -> Self {
        Self::Sector(value)
    }
}
