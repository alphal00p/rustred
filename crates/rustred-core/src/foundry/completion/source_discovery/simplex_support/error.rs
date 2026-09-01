use std::fmt;

/// Typed failures shared by the two proposal-only simplex planners.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SimplexSupportError {
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for SimplexSupportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "simplex-support {resource} overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for simplex-support {resource}"
            ),
            Self::Invariant { detail } => {
                write!(formatter, "simplex-support invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for SimplexSupportError {}
