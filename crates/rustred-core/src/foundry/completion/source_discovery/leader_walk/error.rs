use std::fmt;

use crate::foundry::completion::CompletionGeometryError;
use crate::identity::TranslatedSourceError;

/// Typed failure to construct or consume one proposal-only leader-walk plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LeaderWalkPlanError {
    EmptyScopeSchedule,
    EmptyStableScopeKey {
        input_ordinal: usize,
    },
    DuplicateStableScopeKey {
        first_canonical_ordinal: usize,
        duplicate_canonical_ordinal: usize,
    },
    WrongPartitionBoxArity {
        input_scope_ordinal: usize,
        box_ordinal: usize,
        expected: usize,
        actual: usize,
    },
    NoUnboundedGeometry,
    LeaderCoordinateOverflow {
        canonical_scope_ordinal: usize,
        box_ordinal: usize,
        position: usize,
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
    Geometry(CompletionGeometryError),
    Shift(TranslatedSourceError),
    StaleGeometryEpoch {
        expected_ordinal: u64,
        actual_ordinal: u64,
    },
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for LeaderWalkPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyScopeSchedule => {
                formatter.write_str("a leader walk requires at least one frozen scope")
            }
            Self::EmptyStableScopeKey { input_ordinal } => write!(
                formatter,
                "leader-walk input scope {input_ordinal} has an empty stable key"
            ),
            Self::DuplicateStableScopeKey {
                first_canonical_ordinal,
                duplicate_canonical_ordinal,
            } => write!(
                formatter,
                "leader-walk canonical scope {duplicate_canonical_ordinal} repeats the stable key of scope {first_canonical_ordinal}"
            ),
            Self::WrongPartitionBoxArity {
                input_scope_ordinal,
                box_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "leader-walk input scope {input_scope_ordinal} box {box_ordinal} has arity {actual}, expected {expected}"
            ),
            Self::NoUnboundedGeometry => formatter.write_str(
                "the frozen uncovered partitions contain no unbounded box; finite points require the exact finite-complement path",
            ),
            Self::LeaderCoordinateOverflow {
                canonical_scope_ordinal,
                box_ordinal,
                position,
            } => write!(
                formatter,
                "leader-walk depth-one coordinate overflowed in scope {canonical_scope_ordinal}, box {box_ordinal}, position {position}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "leader-walk {resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "leader-walk {resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for leader-walk {resource}"
            ),
            Self::Geometry(error) => write!(
                formatter,
                "leader-walk chart-point conversion failed: {error}"
            ),
            Self::Shift(error) => {
                write!(formatter, "leader-walk target-shift construction failed: {error}")
            }
            Self::StaleGeometryEpoch {
                expected_ordinal,
                actual_ordinal,
            } => write!(
                formatter,
                "leader-walk task belongs to stale geometry epoch {actual_ordinal}; the current frozen epoch is {expected_ordinal}"
            ),
            Self::Invariant { detail } => {
                write!(formatter, "leader-walk planning invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for LeaderWalkPlanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Geometry(error) => Some(error),
            Self::Shift(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CompletionGeometryError> for LeaderWalkPlanError {
    fn from(error: CompletionGeometryError) -> Self {
        Self::Geometry(error)
    }
}

impl From<TranslatedSourceError> for LeaderWalkPlanError {
    fn from(error: TranslatedSourceError) -> Self {
        Self::Shift(error)
    }
}
