use std::fmt;

use crate::foundry::completion::CompletionGeometryError;
use crate::identity::TranslatedSourceError;

/// Typed failure to construct or consume one proposal-only simplex plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InteriorSimplexPlanError {
    ZeroInteriorMargin,
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
    CoordinateOverflow {
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
    ValueLimit {
        resource: &'static str,
        requested: u64,
        limit: u64,
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

impl fmt::Display for InteriorSimplexPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroInteriorMargin => formatter.write_str(
                "interior-simplex sampling requires a strictly positive interior margin",
            ),
            Self::EmptyScopeSchedule => {
                formatter.write_str("interior-simplex sampling requires a frozen scope")
            }
            Self::EmptyStableScopeKey { input_ordinal } => write!(
                formatter,
                "interior-simplex input scope {input_ordinal} has an empty stable key"
            ),
            Self::DuplicateStableScopeKey {
                first_canonical_ordinal,
                duplicate_canonical_ordinal,
            } => write!(
                formatter,
                "interior-simplex canonical scope {duplicate_canonical_ordinal} repeats the stable key of scope {first_canonical_ordinal}"
            ),
            Self::WrongPartitionBoxArity {
                input_scope_ordinal,
                box_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "interior-simplex input scope {input_scope_ordinal} box {box_ordinal} has arity {actual}, expected {expected}"
            ),
            Self::NoUnboundedGeometry => formatter.write_str(
                "the frozen uncovered partitions contain no unbounded box; interior sampling applies only to blind rays",
            ),
            Self::CoordinateOverflow {
                canonical_scope_ordinal,
                box_ordinal,
                position,
            } => write!(
                formatter,
                "interior-simplex coordinate overflowed in scope {canonical_scope_ordinal}, box {box_ordinal}, position {position}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "interior-simplex {resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "interior-simplex {resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::ValueLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "interior-simplex {resource} value {requested} exceeds the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for interior-simplex {resource}"
            ),
            Self::Geometry(error) => write!(
                formatter,
                "interior-simplex chart-point conversion failed: {error}"
            ),
            Self::Shift(error) => write!(
                formatter,
                "interior-simplex target-shift construction failed: {error}"
            ),
            Self::StaleGeometryEpoch {
                expected_ordinal,
                actual_ordinal,
            } => write!(
                formatter,
                "interior-simplex task belongs to stale geometry epoch {actual_ordinal}; the current frozen epoch is {expected_ordinal}"
            ),
            Self::Invariant { detail } => {
                write!(formatter, "interior-simplex planning invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for InteriorSimplexPlanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Geometry(error) => Some(error),
            Self::Shift(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CompletionGeometryError> for InteriorSimplexPlanError {
    fn from(error: CompletionGeometryError) -> Self {
        Self::Geometry(error)
    }
}

impl From<TranslatedSourceError> for InteriorSimplexPlanError {
    fn from(error: TranslatedSourceError) -> Self {
        Self::Shift(error)
    }
}
