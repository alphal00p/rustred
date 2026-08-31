use std::fmt;

/// Typed failures at the bounded modular-discovery boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ModularKernelError {
    WrongFrameContext,
    UnsupportedEvenModulus {
        modulus: u64,
    },
    NonPrimeModulus {
        modulus: u64,
    },
    WrongBaseParameterArity {
        expected: usize,
        actual: usize,
    },
    WrongChartCoordinateArity {
        expected: usize,
        actual: usize,
    },
    WrongContextIndexArity {
        expected: usize,
        actual: usize,
    },
    WrongIndexedContext {
        row: usize,
    },
    CoefficientDenominatorZero {
        row: usize,
        physical_column: usize,
    },
    SourceConditionZero {
        row: usize,
        condition: usize,
    },
    TargetColumnOutOfRange {
        target: usize,
        columns: usize,
    },
    ForbiddenColumnOutOfRange {
        column: usize,
        columns: usize,
    },
    DuplicateForbiddenColumn {
        column: usize,
    },
    TargetIsForbidden {
        target: usize,
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
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for ModularKernelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFrameContext => write!(
                formatter,
                "modular sample context does not belong to the physical frame"
            ),
            Self::UnsupportedEvenModulus { modulus } => write!(
                formatter,
                "modular physical-frame sampling requires an odd prime, got even modulus {modulus}"
            ),
            Self::NonPrimeModulus { modulus } => write!(
                formatter,
                "modular physical-frame sampling requires a prime modulus, got {modulus}"
            ),
            Self::WrongBaseParameterArity { expected, actual } => write!(
                formatter,
                "modular sample has {actual} base-parameter values, expected {expected}"
            ),
            Self::WrongChartCoordinateArity { expected, actual } => write!(
                formatter,
                "modular sample has {actual} chart coordinates, expected {expected}"
            ),
            Self::WrongContextIndexArity { expected, actual } => write!(
                formatter,
                "modular sample context has {actual} index variables, expected frame arity {expected}"
            ),
            Self::WrongIndexedContext { row } => write!(
                formatter,
                "physical-frame row {row} does not belong to the supplied indexed coefficient context"
            ),
            Self::CoefficientDenominatorZero {
                row,
                physical_column,
            } => write!(
                formatter,
                "physical-frame coefficient at row {row}, column {physical_column} has zero denominator at the modular sample"
            ),
            Self::SourceConditionZero { row, condition } => write!(
                formatter,
                "physical-frame source condition {condition} on row {row} vanishes at the modular sample"
            ),
            Self::TargetColumnOutOfRange { target, columns } => write!(
                formatter,
                "target physical column {target} is outside the {columns}-column frame"
            ),
            Self::ForbiddenColumnOutOfRange { column, columns } => write!(
                formatter,
                "forbidden physical column {column} is outside the {columns}-column frame"
            ),
            Self::DuplicateForbiddenColumn { column } => {
                write!(formatter, "forbidden physical column {column} is repeated")
            }
            Self::TargetIsForbidden { target } => {
                write!(
                    formatter,
                    "target physical column {target} is also forbidden"
                )
            }
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
            Self::NativePanic { operation } => {
                write!(formatter, "Symbolica panicked while {operation}")
            }
            Self::Invariant { detail } => {
                write!(
                    formatter,
                    "modular physical-frame invariant failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for ModularKernelError {}
