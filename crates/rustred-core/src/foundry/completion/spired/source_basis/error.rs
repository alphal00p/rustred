use std::fmt;

use crate::algebra::IndexedAlgebraError;
use crate::identity::ParametricRelationError;
use crate::sector::Error as SectorError;

/// Typed failure at the optional SpIReD source-preconditioning boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredSourceBasisError {
    WrongFamily,
    WrongContext,
    IncompleteOrdinarySourceLayout,
    EmptySourceCorpus,
    EmptySourceRow {
        source_ordinal: usize,
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
    NativePanic {
        operation: &'static str,
    },
    IndexedAlgebra(IndexedAlgebraError),
    Relation(ParametricRelationError),
    Sector(SectorError),
    ReplayMismatch {
        row_ordinal: usize,
        detail: &'static str,
    },
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for SpiredSourceBasisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFamily => formatter.write_str(
                "source preconditioning case and ordinary source corpus have different families",
            ),
            Self::WrongContext => formatter.write_str(
                "source preconditioning case, source corpus, and indexed context differ",
            ),
            Self::IncompleteOrdinarySourceLayout => formatter.write_str(
                "source preconditioning requires the complete ordinary IBP source layout",
            ),
            Self::EmptySourceCorpus => {
                formatter.write_str("cannot precondition an empty ordinary source corpus")
            }
            Self::EmptySourceRow { source_ordinal } => write!(
                formatter,
                "ordinary source row {source_ordinal} is identically empty"
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
                "{resource} requires {requested}, exceeding configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}"
            ),
            Self::NativePanic { operation } => {
                write!(formatter, "Symbolica panicked while {operation}")
            }
            Self::IndexedAlgebra(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
            Self::ReplayMismatch {
                row_ordinal,
                detail,
            } => write!(
                formatter,
                "preconditioned source row {row_ordinal} failed exact replay: {detail}"
            ),
            Self::Invariant { detail } => {
                write!(
                    formatter,
                    "source preconditioner invariant failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for SpiredSourceBasisError {}

impl From<IndexedAlgebraError> for SpiredSourceBasisError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::IndexedAlgebra(value)
    }
}

impl From<ParametricRelationError> for SpiredSourceBasisError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

impl From<SectorError> for SpiredSourceBasisError {
    fn from(value: SectorError) -> Self {
        Self::Sector(value)
    }
}
