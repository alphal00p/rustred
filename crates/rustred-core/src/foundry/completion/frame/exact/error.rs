use std::fmt;

use crate::algebra::IndexedAlgebraError;

/// Typed failure while lifting and replaying one modular target circuit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExactCircuitError {
    ForeignFrameHit,
    PartitionVerificationFailed,
    TargetMismatch {
        hit: usize,
        partition: usize,
    },
    ForbiddenColumnsMismatch,
    InvalidModularHitRanks {
        forbidden_rank: usize,
        augmented_rank: usize,
        selected_rows: usize,
    },
    SelectedSourceRowOutOfRange {
        row: usize,
        rows: usize,
    },
    SelectedSourceRowsNotStrictlyIncreasing,
    WrongIndexedContext {
        row: usize,
    },
    IdenticallyZeroSourceCondition {
        row: usize,
        condition: usize,
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
    NativePanic {
        operation: &'static str,
    },
    ReducerRejectedSelectedRow {
        frame_row: usize,
    },
    ReplayMismatch {
        physical_column: usize,
        detail: &'static str,
    },
    Invariant {
        detail: &'static str,
    },
    IndexedAlgebra(IndexedAlgebraError),
}

impl fmt::Display for ExactCircuitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignFrameHit => formatter.write_str(
                "the modular hit and exact target partition belong to different physical frames",
            ),
            Self::PartitionVerificationFailed => {
                formatter.write_str("the supplied exact target partition failed cold verification")
            }
            Self::TargetMismatch { hit, partition } => write!(
                formatter,
                "modular hit target column {hit} differs from exact partition target {partition}"
            ),
            Self::ForbiddenColumnsMismatch => formatter
                .write_str("modular hit forbidden columns differ from the exact target partition"),
            Self::InvalidModularHitRanks {
                forbidden_rank,
                augmented_rank,
                selected_rows,
            } => write!(
                formatter,
                "modular hit has forbidden rank {forbidden_rank}, augmented rank {augmented_rank}, and {selected_rows} selected rows"
            ),
            Self::SelectedSourceRowOutOfRange { row, rows } => write!(
                formatter,
                "selected physical-frame row {row} is outside the {rows}-row frame"
            ),
            Self::SelectedSourceRowsNotStrictlyIncreasing => formatter
                .write_str("selected physical-frame rows are not in strict original chronology"),
            Self::WrongIndexedContext { row } => write!(
                formatter,
                "selected physical-frame row {row} belongs to a foreign indexed context"
            ),
            Self::IdenticallyZeroSourceCondition { row, condition } => write!(
                formatter,
                "selected physical-frame row {row} has identically zero condition {condition}"
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
            Self::U32NotRepresentable { resource, value } => {
                write!(formatter, "{resource} value {value} does not fit u32")
            }
            Self::NativePanic { operation } => {
                write!(formatter, "Symbolica panicked while {operation}")
            }
            Self::ReducerRejectedSelectedRow { frame_row } => write!(
                formatter,
                "Symbolica rejected modularly independent exact frame row {frame_row}"
            ),
            Self::ReplayMismatch {
                physical_column,
                detail,
            } => write!(
                formatter,
                "exact source replay failed at physical column {physical_column}: {detail}"
            ),
            Self::Invariant { detail } => {
                write!(formatter, "exact target-circuit invariant failed: {detail}")
            }
            Self::IndexedAlgebra(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ExactCircuitError {}

impl From<IndexedAlgebraError> for ExactCircuitError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::IndexedAlgebra(value)
    }
}
