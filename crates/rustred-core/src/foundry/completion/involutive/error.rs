use std::fmt;

use crate::algebra::IndexedAlgebraError;
use crate::sector;

use super::super::CompletionGeometryError;
use super::EpochId;

/// Typed failures from proposal-only Janet/Ore construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InvolutiveError {
    EmptyCoordinateSpace,
    EmptyInitialBasis,
    WrongArity {
        object: &'static str,
        expected: usize,
        actual: usize,
    },
    CoordinateOutOfRange {
        position: usize,
        arity: usize,
    },
    ShiftCoordinateNotRepresentable {
        position: usize,
        coordinate: u64,
    },
    ShiftCoordinateLimit {
        position: usize,
        requested: u64,
        limit: u64,
    },
    NonDivisibleForwardShift {
        position: usize,
        dividend: u64,
        divisor: u64,
    },
    SourceOrdinalOutOfRange {
        source_ordinal: usize,
        source_count: usize,
    },
    DuplicateLeadingShift,
    ZeroBasisRow,
    ForeignOreAction,
    StaleEpoch {
        expected: EpochId,
        actual: EpochId,
    },
    EpochLimit {
        requested: u64,
        limit: u64,
    },
    InvalidProlongation {
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
    NativePolynomialPanic {
        operation: &'static str,
    },
    NonExactPolynomialDivision {
        operation: &'static str,
    },
    LocalizationDomainMismatch,
    Algebra(IndexedAlgebraError),
    Ordering(sector::Error),
    Geometry(CompletionGeometryError),
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for InvolutiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCoordinateSpace => {
                formatter.write_str("involutive completion needs at least one coordinate")
            }
            Self::EmptyInitialBasis => {
                formatter.write_str("involutive completion needs at least one initial Ore row")
            }
            Self::WrongArity {
                object,
                expected,
                actual,
            } => write!(
                formatter,
                "{object} has arity {actual}, expected involutive arity {expected}"
            ),
            Self::CoordinateOutOfRange { position, arity } => write!(
                formatter,
                "coordinate {position} is outside involutive arity {arity}"
            ),
            Self::ShiftCoordinateNotRepresentable {
                position,
                coordinate,
            } => write!(
                formatter,
                "forward-shift coordinate {coordinate} at position {position} has no checked i64 Ore translation"
            ),
            Self::ShiftCoordinateLimit {
                position,
                requested,
                limit,
            } => write!(
                formatter,
                "forward-shift coordinate {requested} at position {position} exceeds the configured limit {limit}"
            ),
            Self::NonDivisibleForwardShift {
                position,
                dividend,
                divisor,
            } => write!(
                formatter,
                "forward-shift coordinate {divisor} does not divide coordinate {dividend} at position {position}"
            ),
            Self::SourceOrdinalOutOfRange {
                source_ordinal,
                source_count,
            } => write!(
                formatter,
                "Ore source ordinal {source_ordinal} is outside the sealed source module 0..{source_count}"
            ),
            Self::DuplicateLeadingShift => {
                formatter.write_str("an involutive basis cannot retain duplicate leading shifts")
            }
            Self::ZeroBasisRow => {
                formatter.write_str("a zero Ore consequence cannot enter a Janet basis")
            }
            Self::ForeignOreAction => formatter
                .write_str("Ore data belongs to a different sector action or ranking instance"),
            Self::StaleEpoch { expected, actual }
                if expected.same_instance(actual) && expected.revision() == actual.revision() =>
            {
                write!(
                    formatter,
                    "Janet object belongs to a sibling immutable snapshot at revision {}",
                    actual.revision(),
                )
            }
            Self::StaleEpoch { expected, actual } => write!(
                formatter,
                "Janet object belongs to {} basis revision {}, current revision is {}",
                if expected.same_instance(actual) {
                    "the same Janet"
                } else {
                    "a foreign Janet"
                },
                actual.revision(),
                expected.revision(),
            ),
            Self::EpochLimit { requested, limit } => write!(
                formatter,
                "Janet epoch {requested} exceeds the configured epoch limit {limit}"
            ),
            Self::InvalidProlongation { detail } => {
                write!(formatter, "invalid Janet prolongation: {detail}")
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
            Self::NativePolynomialPanic { operation } => {
                write!(formatter, "Symbolica panicked while {operation}")
            }
            Self::NonExactPolynomialDivision { operation } => {
                write!(formatter, "{operation} was not an exact polynomial division")
            }
            Self::LocalizationDomainMismatch => formatter.write_str(
                "the authenticated lazy localization does not imply every replay-required nonzero condition",
            ),
            Self::Algebra(error) => error.fmt(formatter),
            Self::Ordering(error) => error.fmt(formatter),
            Self::Geometry(error) => error.fmt(formatter),
            Self::Invariant { detail } => write!(
                formatter,
                "involutive completion reached an internal invariant failure: {detail}"
            ),
        }
    }
}

impl std::error::Error for InvolutiveError {}

impl From<IndexedAlgebraError> for InvolutiveError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::Algebra(value)
    }
}

impl From<sector::Error> for InvolutiveError {
    fn from(value: sector::Error) -> Self {
        Self::Ordering(value)
    }
}

impl From<CompletionGeometryError> for InvolutiveError {
    fn from(value: CompletionGeometryError) -> Self {
        Self::Geometry(value)
    }
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), InvolutiveError> {
    if requested > limit {
        Err(InvolutiveError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, InvolutiveError> {
    left.checked_add(right)
        .ok_or(InvolutiveError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, InvolutiveError> {
    left.checked_mul(right)
        .ok_or(InvolutiveError::ResourceCountOverflow { resource })
}

pub(super) fn checked_sort_coordinate_work(
    resource: &'static str,
    items: usize,
    arity: usize,
) -> Result<usize, InvolutiveError> {
    let rounds = if items <= 1 {
        0
    } else {
        usize::BITS as usize - (items - 1).leading_zeros() as usize
    };
    checked_mul(resource, checked_mul(resource, items, rounds)?, arity)
}

pub(super) fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, InvolutiveError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(capacity)
        .map_err(|_| InvolutiveError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(result)
}

pub(super) fn reserve_additional<T>(
    values: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), InvolutiveError> {
    let requested = checked_add(resource, values.len(), additional)?;
    values
        .try_reserve_exact(additional)
        .map_err(|_| InvolutiveError::AllocationFailure {
            resource,
            requested,
        })
}

/// Amortized fallible push under an already meaningful logical cap.
///
/// Unlike `try_reserve_exact(1)` on every insertion, this grows by bounded
/// geometric chunks and therefore performs only logarithmically many
/// allocation attempts. Logical length is checked before any growth.
pub(super) fn try_push_bounded<T>(
    values: &mut Vec<T>,
    value: T,
    resource: &'static str,
    limit: usize,
) -> Result<(), InvolutiveError> {
    let requested = checked_add(resource, values.len(), 1)?;
    check_limit(resource, requested, limit)?;
    if values.len() == values.capacity() {
        let remaining = limit.saturating_sub(values.len());
        let geometric = values.capacity().max(4);
        let additional = geometric.min(remaining);
        values
            .try_reserve_exact(additional)
            .map_err(|_| InvolutiveError::AllocationFailure {
                resource,
                requested: checked_add(resource, values.len(), additional).unwrap_or(usize::MAX),
            })?;
    }
    values.push(value);
    Ok(())
}
