use std::fmt;

use crate::algebra::IndexedAlgebraError;

use super::super::InvolutiveError;

/// Typed failure of proposal-only projective Ore arithmetic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ProjectiveError {
    ForeignAction,
    ContextIndexArityMismatch {
        consequence_arity: usize,
        context_index_count: usize,
    },
    ContextFingerprintMismatch,
    WorkBudgetLimitsMismatch,
    ValidatedDivisorLimitsMismatch,
    MissingSubjectTarget,
    ZeroDivisor,
    ReductionTargetMismatch,
    NonDescendingDivisorTail,
    TargetExceedsPreviousSelection,
    NonExactPolynomialDivision,
    NativePanic {
        operation: &'static str,
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
    IndexedAlgebra(IndexedAlgebraError),
    Involutive(InvolutiveError),
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for ProjectiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignAction => {
                formatter.write_str("projective operands belong to different exact Ore actions")
            }
            Self::ContextIndexArityMismatch {
                consequence_arity,
                context_index_count,
            } => write!(
                formatter,
                "projective consequence arity {consequence_arity} disagrees with indexed coefficient context arity {context_index_count}",
            ),
            Self::ContextFingerprintMismatch => formatter.write_str(
                "projective consequence belongs to a different indexed coefficient context",
            ),
            Self::WorkBudgetLimitsMismatch => formatter.write_str(
                "projective work budget is bound to a different immutable limit contract",
            ),
            Self::ValidatedDivisorLimitsMismatch => formatter.write_str(
                "validated projective divisor belongs to a different immutable limit contract",
            ),
            Self::MissingSubjectTarget => formatter
                .write_str("the selected projective reduction target is absent from the subject"),
            Self::ZeroDivisor => {
                formatter.write_str("a zero projective row cannot be used as a divisor")
            }
            Self::ReductionTargetMismatch => formatter
                .write_str("the selected target is not the translated projective divisor leader"),
            Self::NonDescendingDivisorTail => formatter.write_str(
                "a translated projective divisor tail does not strictly descend from its target",
            ),
            Self::TargetExceedsPreviousSelection => formatter.write_str(
                "the selected projective replay target does not strictly descend from the previous selected reducible target",
            ),
            Self::NonExactPolynomialDivision => formatter.write_str(
                "Symbolica reported a remainder in a required exact polynomial division",
            ),
            Self::NativePanic { operation } => {
                write!(formatter, "Symbolica panicked while {operation}")
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
                "{resource} needs {requested} units, exceeding the configured limit {limit}",
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}",
            ),
            Self::IndexedAlgebra(error) => error.fmt(formatter),
            Self::Involutive(error) => error.fmt(formatter),
            Self::Invariant { detail } => write!(
                formatter,
                "projective Ore arithmetic reached an internal invariant failure: {detail}",
            ),
        }
    }
}

impl std::error::Error for ProjectiveError {}

impl From<IndexedAlgebraError> for ProjectiveError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::IndexedAlgebra(value)
    }
}

impl From<InvolutiveError> for ProjectiveError {
    fn from(value: InvolutiveError) -> Self {
        Self::Involutive(value)
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ProjectiveError> {
    left.checked_add(right)
        .ok_or(ProjectiveError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ProjectiveError> {
    left.checked_mul(right)
        .ok_or(ProjectiveError::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ProjectiveError> {
    if requested > limit {
        Err(ProjectiveError::ResourceLimit {
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
) -> Result<Vec<T>, ProjectiveError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| ProjectiveError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}
