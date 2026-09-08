use std::fmt;

use crate::identity::TranslatedSourceError;
use crate::sector::{Error as SectorError, OrderingPolicy};

/// Typed construction failures for the private targeted-completion
/// foundation.
///
/// Resource exhaustion and a finite depth limit are inconclusive discovery
/// outcomes. Neither may be promoted into a master or closure statement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredFoundationError {
    UnsupportedAffineLattice {
        ambient_arity: usize,
    },
    WrongTargetArity {
        expected: usize,
        actual: usize,
    },
    WrongOwnerArity {
        expected: usize,
        actual: usize,
    },
    WrongOwnerFamily,
    WrongOwnerContext,
    WrongOrderingArity {
        expected: usize,
        actual: usize,
    },
    OwnerOrderingMismatch {
        requested: OrderingPolicy,
        snapshot: OrderingPolicy,
    },
    InvalidTargetShift(SectorError),
    EmptyIndexSpace,
    EmptySourceRows,
    EmptyRequestChunk,
    DepthNotRepresentable {
        depth: usize,
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
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for SpiredFoundationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedAffineLattice { ambient_arity } => write!(
                formatter,
                "SpIRed affine-lattice cases of ambient arity {ambient_arity} are not supported"
            ),
            Self::WrongTargetArity { expected, actual } => write!(
                formatter,
                "SpIRed target shift has arity {actual}, expected coordinate-case arity {expected}"
            ),
            Self::WrongOwnerArity { expected, actual } => write!(
                formatter,
                "SpIRed owner snapshot has arity {actual}, expected coordinate-case arity {expected}"
            ),
            Self::WrongOwnerFamily => formatter.write_str(
                "SpIRed owner snapshot and coordinate case belong to different families",
            ),
            Self::WrongOwnerContext => formatter.write_str(
                "SpIRed owner snapshot and coordinate case use different coefficient contexts",
            ),
            Self::WrongOrderingArity { expected, actual } => write!(
                formatter,
                "SpIRed ordering has coordinate arity {actual}, expected case arity {expected}"
            ),
            Self::OwnerOrderingMismatch {
                requested,
                snapshot,
            } => write!(
                formatter,
                "SpIRed ordering {} differs from owner-snapshot ordering {}",
                requested.stable_id(),
                snapshot.stable_id(),
            ),
            Self::InvalidTargetShift(error) => write!(
                formatter,
                "SpIRed target shift is invalid on its coordinate case: {error}"
            ),
            Self::EmptyIndexSpace => {
                formatter.write_str("SpIRed scheduling requires a nonempty index space")
            }
            Self::EmptySourceRows => {
                formatter.write_str("SpIRed scheduling requires at least one source row")
            }
            Self::EmptyRequestChunk => {
                formatter.write_str("SpIRed request chunks require a positive size bound")
            }
            Self::DepthNotRepresentable { depth } => write!(
                formatter,
                "SpIRed signed-L1 depth {depth} cannot be represented by an i64 lattice shift"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "SpIRed {resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "SpIRed {resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for SpIRed {resource}"
            ),
            Self::Shift(error) => {
                write!(
                    formatter,
                    "could not construct a SpIRed source shift: {error}"
                )
            }
            Self::Invariant { detail } => {
                write!(formatter, "SpIRed foundation invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for SpiredFoundationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Shift(error) => Some(error),
            Self::InvalidTargetShift(error) => Some(error),
            _ => None,
        }
    }
}

impl From<TranslatedSourceError> for SpiredFoundationError {
    fn from(error: TranslatedSourceError) -> Self {
        Self::Shift(error)
    }
}
