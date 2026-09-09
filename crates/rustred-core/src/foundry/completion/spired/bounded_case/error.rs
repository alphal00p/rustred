use std::fmt;

use crate::foundry::completion::spired::SpiredCoordinateCaseWorklistError;
use crate::foundry::completion::stratum::StratumRegistryError;

/// Hard failure while constructing a non-authoritative bounded case envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredBoundedCaseEnvelopeError {
    GuardedParent {
        guard_branches: usize,
    },
    InvalidParentIdentity,
    WrongSourceLayout {
        actual: &'static str,
    },
    EmptySourceRows,
    WrongSourceFamily,
    WrongSourceContext,
    WrongTargetArity {
        expected: usize,
        actual: usize,
    },
    WrongSourceTermArity {
        source_ordinal: usize,
        term_ordinal: usize,
        expected: usize,
        actual: usize,
    },
    DepthNotRepresentable {
        depth: usize,
    },
    TranslatedShiftOverflow {
        source_ordinal: usize,
        term_ordinal: usize,
        position: usize,
        source_shift: i64,
        depth: usize,
    },
    RelativeShiftEnvelopeOverflow {
        source_ordinal: usize,
        term_ordinal: usize,
        position: usize,
        source_shift: i64,
        target_shift: i64,
        depth: usize,
    },
    BaseSafeUpperOverflow {
        position: usize,
        target_shift: i64,
        max_positive_relative_shift: i64,
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
    Sector(crate::sector::Error),
    Stratum(StratumRegistryError),
    CoordinateCase(SpiredCoordinateCaseWorklistError),
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for SpiredBoundedCaseEnvelopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GuardedParent { guard_branches } => write!(
                formatter,
                "bounded SpIReD discovery requires an equality-only parent, but it has {guard_branches} guard branches"
            ),
            Self::InvalidParentIdentity => {
                formatter.write_str("bounded SpIReD discovery received an invalid parent identity")
            }
            Self::WrongSourceLayout { actual } => write!(
                formatter,
                "bounded SpIReD discovery requires complete ordinary IBP rows, not {actual}"
            ),
            Self::EmptySourceRows => formatter
                .write_str("bounded SpIReD discovery requires at least one ordinary source row"),
            Self::WrongSourceFamily => formatter.write_str(
                "bounded SpIReD source rows and parent case belong to different families",
            ),
            Self::WrongSourceContext => formatter.write_str(
                "bounded SpIReD source rows and parent case use different coefficient contexts",
            ),
            Self::WrongTargetArity { expected, actual } => write!(
                formatter,
                "bounded SpIReD target shift has arity {actual}, expected {expected}"
            ),
            Self::WrongSourceTermArity {
                source_ordinal,
                term_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "ordinary source {source_ordinal} term {term_ordinal} has shift arity {actual}, expected {expected}"
            ),
            Self::DepthNotRepresentable { depth } => write!(
                formatter,
                "bounded SpIReD signed-L1 depth {depth} is not representable by an i64 shift"
            ),
            Self::TranslatedShiftOverflow {
                source_ordinal,
                term_ordinal,
                position,
                source_shift,
                depth,
            } => write!(
                formatter,
                "ordinary source {source_ordinal} term {term_ordinal} component {position} ({source_shift}) cannot be translated through signed-L1 depth {depth} on the i64 carrier"
            ),
            Self::RelativeShiftEnvelopeOverflow {
                source_ordinal,
                term_ordinal,
                position,
                source_shift,
                target_shift,
                depth,
            } => write!(
                formatter,
                "ordinary source {source_ordinal} term {term_ordinal} component {position} has an unrepresentable target-relative upper shift: {source_shift} - {target_shift} + {depth}"
            ),
            Self::BaseSafeUpperOverflow {
                position,
                target_shift,
                max_positive_relative_shift,
            } => write!(
                formatter,
                "bounded SpIReD inactive coordinate {position} has no representable safe upper bound for target shift {target_shift} and relative envelope {max_positive_relative_shift}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "bounded SpIReD {resource} count overflowed")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "bounded SpIReD {resource} requires {requested}, limit is {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for bounded SpIReD {resource}"
            ),
            Self::Sector(error) => write!(formatter, "bounded SpIReD domain error: {error}"),
            Self::Stratum(error) => write!(formatter, "bounded SpIReD stratum error: {error}"),
            Self::CoordinateCase(error) => {
                write!(formatter, "bounded SpIReD coordinate-case error: {error}")
            }
            Self::Invariant { detail } => {
                write!(formatter, "bounded SpIReD invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for SpiredBoundedCaseEnvelopeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sector(error) => Some(error),
            Self::Stratum(error) => Some(error),
            Self::CoordinateCase(error) => Some(error),
            _ => None,
        }
    }
}

impl From<crate::sector::Error> for SpiredBoundedCaseEnvelopeError {
    fn from(value: crate::sector::Error) -> Self {
        Self::Sector(value)
    }
}

impl From<StratumRegistryError> for SpiredBoundedCaseEnvelopeError {
    fn from(value: StratumRegistryError) -> Self {
        Self::Stratum(value)
    }
}

impl From<SpiredCoordinateCaseWorklistError> for SpiredBoundedCaseEnvelopeError {
    fn from(value: SpiredCoordinateCaseWorklistError) -> Self {
        Self::CoordinateCase(value)
    }
}
