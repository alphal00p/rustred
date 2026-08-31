use std::fmt;

use super::super::{
    CanonicalReplayError, ExactExecutableOwnerError, scheduler::ProbeLocalSchedulerError,
};

/// Hard failure of one streaming replay transaction.
///
/// Ordinary no-hit, exact-support, guard, and anchor obstructions remain
/// typed dispositions rather than entering this channel.
#[derive(Debug)]
pub(crate) enum InteriorReplayRunError {
    Scheduler(ProbeLocalSchedulerError),
    CanonicalReplay(CanonicalReplayError),
    OwnerCompilation(ExactExecutableOwnerError),
    RelativeCoordinateOverflow {
        object: &'static str,
        position: usize,
        value: i64,
        target: i64,
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
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for InteriorReplayRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scheduler(error) => error.fmt(formatter),
            Self::CanonicalReplay(error) => error.fmt(formatter),
            Self::OwnerCompilation(error) => error.fmt(formatter),
            Self::RelativeCoordinateOverflow {
                object,
                position,
                value,
                target,
            } => write!(
                formatter,
                "{object} coordinate {position} cannot represent {value} - {target} in i64"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "interior replay {resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "interior replay {resource} needs {requested}, exceeding limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for interior replay {resource}"
            ),
            Self::Invariant { detail } => {
                write!(formatter, "interior replay invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for InteriorReplayRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Scheduler(error) => Some(error),
            Self::CanonicalReplay(error) => Some(error),
            Self::OwnerCompilation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ProbeLocalSchedulerError> for InteriorReplayRunError {
    fn from(value: ProbeLocalSchedulerError) -> Self {
        Self::Scheduler(value)
    }
}

impl From<CanonicalReplayError> for InteriorReplayRunError {
    fn from(value: CanonicalReplayError) -> Self {
        Self::CanonicalReplay(value)
    }
}

impl From<ExactExecutableOwnerError> for InteriorReplayRunError {
    fn from(value: ExactExecutableOwnerError) -> Self {
        Self::OwnerCompilation(value)
    }
}
