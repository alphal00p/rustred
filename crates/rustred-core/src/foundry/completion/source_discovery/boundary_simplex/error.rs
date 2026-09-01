use std::fmt;

use crate::foundry::completion::CompletionGeometryError;
use crate::identity::TranslatedSourceError;

use super::super::simplex_support::SimplexSupportError;

/// Typed failure to construct or consume one proposal-only boundary plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BoundarySimplexPlanError {
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
    InvalidParentFreeDimension {
        requested: usize,
        maximal_input_arity: usize,
    },
    ParentFreeDimensionUnavailable {
        requested: usize,
        maximal_available: usize,
    },
    InvalidBoundaryCodimension {
        parent_free_dimension: usize,
        requested: usize,
    },
    SimplexProfileRequiresPositiveFaceDimension,
    VertexProfileRequiresZeroFaceDimension {
        actual: usize,
    },
    ZeroInteriorMargin,
    CoordinateOverflow {
        canonical_scope_ordinal: usize,
        parent_box_ordinal: usize,
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

impl fmt::Display for BoundarySimplexPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyScopeSchedule => {
                formatter.write_str("boundary-simplex sampling requires a frozen scope")
            }
            Self::EmptyStableScopeKey { input_ordinal } => write!(
                formatter,
                "boundary-simplex input scope {input_ordinal} has an empty stable key"
            ),
            Self::DuplicateStableScopeKey {
                first_canonical_ordinal,
                duplicate_canonical_ordinal,
            } => write!(
                formatter,
                "boundary-simplex canonical scope {duplicate_canonical_ordinal} repeats the stable key of scope {first_canonical_ordinal}"
            ),
            Self::WrongPartitionBoxArity {
                input_scope_ordinal,
                box_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "boundary-simplex input scope {input_scope_ordinal} box {box_ordinal} has arity {actual}, expected {expected}"
            ),
            Self::InvalidParentFreeDimension {
                requested,
                maximal_input_arity,
            } => write!(
                formatter,
                "boundary-simplex parent free dimension {requested} exceeds the maximal input arity {maximal_input_arity}"
            ),
            Self::ParentFreeDimensionUnavailable {
                requested,
                maximal_available,
            } => write!(
                formatter,
                "boundary-simplex input geometry has no box of parent free dimension {requested}; its maximal available free dimension is {maximal_available}"
            ),
            Self::InvalidBoundaryCodimension {
                parent_free_dimension,
                requested,
            } => write!(
                formatter,
                "boundary-simplex codimension {requested} exceeds parent free dimension {parent_free_dimension}"
            ),
            Self::SimplexProfileRequiresPositiveFaceDimension => formatter.write_str(
                "boundary-simplex all-pinned faces require the typed vertex profile",
            ),
            Self::VertexProfileRequiresZeroFaceDimension { actual } => write!(
                formatter,
                "boundary-simplex vertex profile requires a zero-dimensional face, not dimension {actual}"
            ),
            Self::ZeroInteriorMargin => formatter.write_str(
                "boundary-simplex positive-dimensional sampling requires a strictly positive interior margin",
            ),
            Self::CoordinateOverflow {
                canonical_scope_ordinal,
                parent_box_ordinal,
                position,
            } => write!(
                formatter,
                "boundary-simplex coordinate overflowed in scope {canonical_scope_ordinal}, parent box {parent_box_ordinal}, position {position}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "boundary-simplex {resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "boundary-simplex {resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::ValueLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "boundary-simplex {resource} value {requested} exceeds the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for boundary-simplex {resource}"
            ),
            Self::Geometry(error) => write!(
                formatter,
                "boundary-simplex chart-point conversion failed: {error}"
            ),
            Self::Shift(error) => write!(
                formatter,
                "boundary-simplex target-shift construction failed: {error}"
            ),
            Self::StaleGeometryEpoch {
                expected_ordinal,
                actual_ordinal,
            } => write!(
                formatter,
                "boundary-simplex task belongs to stale geometry epoch {actual_ordinal}; the current frozen epoch is {expected_ordinal}"
            ),
            Self::Invariant { detail } => {
                write!(formatter, "boundary-simplex planning invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for BoundarySimplexPlanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Geometry(error) => Some(error),
            Self::Shift(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CompletionGeometryError> for BoundarySimplexPlanError {
    fn from(error: CompletionGeometryError) -> Self {
        Self::Geometry(error)
    }
}

impl From<TranslatedSourceError> for BoundarySimplexPlanError {
    fn from(error: TranslatedSourceError) -> Self {
        Self::Shift(error)
    }
}

impl From<SimplexSupportError> for BoundarySimplexPlanError {
    fn from(error: SimplexSupportError) -> Self {
        match error {
            SimplexSupportError::ResourceCountOverflow { resource } => {
                Self::ResourceCountOverflow { resource }
            }
            SimplexSupportError::AllocationFailure {
                resource,
                requested,
            } => Self::AllocationFailure {
                resource,
                requested,
            },
            SimplexSupportError::Invariant { detail } => Self::Invariant { detail },
        }
    }
}
