use std::fmt;

use crate::algebra::IndexedAlgebraError;

use super::super::super::InvolutiveError;
use super::super::ModularGuideError;

/// Typed failure at the exact-lazy authority boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ExactLazyError {
    WrongSessionOwner,
    WrongCompletionLedger,
    WrongIndexedContext,
    WrongOreAction,
    WrongSourceModule,
    WrongLimitsContract,
    WrongNormalFormMode {
        expected: &'static str,
        actual: &'static str,
    },
    FrozenDivisorOutOfRange {
        ordinal: usize,
        divisor_count: usize,
    },
    WrongArity {
        object: &'static str,
        expected: usize,
        actual: usize,
    },
    InvalidSupport {
        detail: &'static str,
    },
    InvalidProof {
        detail: &'static str,
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
    TransactionRollback {
        detail: &'static str,
    },
    Modular(ModularGuideError),
    Involutive(InvolutiveError),
}

impl fmt::Display for ExactLazyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSessionOwner => {
                formatter.write_str("value belongs to another exact-lazy session")
            }
            Self::WrongCompletionLedger => {
                formatter.write_str("operation supplied another exact-lazy completion ledger")
            }
            Self::WrongIndexedContext => {
                formatter.write_str("exact-lazy session belongs to another indexed context")
            }
            Self::WrongOreAction => {
                formatter.write_str("exact-lazy session belongs to another Ore action")
            }
            Self::WrongSourceModule => {
                formatter.write_str("exact-lazy session belongs to another completed source module")
            }
            Self::WrongLimitsContract => formatter
                .write_str("exact-lazy operation supplied a different immutable limit contract"),
            Self::WrongNormalFormMode { expected, actual } => write!(
                formatter,
                "exact-lazy normal-form mode is {actual}, expected {expected}"
            ),
            Self::FrozenDivisorOutOfRange {
                ordinal,
                divisor_count,
            } => write!(
                formatter,
                "frozen Janet divisor ordinal {ordinal} is outside {divisor_count} retained rows"
            ),
            Self::WrongArity {
                object,
                expected,
                actual,
            } => write!(
                formatter,
                "{object} has arity {actual}, expected {expected}"
            ),
            Self::InvalidSupport { detail } => {
                write!(formatter, "invalid exact-lazy support: {detail}")
            }
            Self::InvalidProof { detail } => {
                write!(formatter, "invalid exact-lazy support proof: {detail}")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "{resource} overflowed its checked integer carrier"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}"
            ),
            Self::TransactionRollback { detail } => {
                write!(
                    formatter,
                    "exact-lazy transaction rollback failed: {detail}"
                )
            }
            Self::Modular(error) => error.fmt(formatter),
            Self::Involutive(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ExactLazyError {}

impl From<ModularGuideError> for ExactLazyError {
    fn from(value: ModularGuideError) -> Self {
        match value {
            ModularGuideError::WrongIndexedContext => Self::WrongIndexedContext,
            ModularGuideError::WrongDagOwner => Self::WrongSessionOwner,
            other => Self::Modular(other),
        }
    }
}

impl From<InvolutiveError> for ExactLazyError {
    fn from(value: InvolutiveError) -> Self {
        match value {
            InvolutiveError::ForeignOreAction => Self::WrongOreAction,
            InvolutiveError::WrongArity {
                object,
                expected,
                actual,
            } => Self::WrongArity {
                object,
                expected,
                actual,
            },
            other => Self::Involutive(other),
        }
    }
}

impl From<IndexedAlgebraError> for ExactLazyError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::Modular(ModularGuideError::Algebra(value))
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactLazyError> {
    left.checked_add(right)
        .ok_or(ExactLazyError::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ExactLazyError> {
    if requested > limit {
        Err(ExactLazyError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, ExactLazyError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| ExactLazyError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}
