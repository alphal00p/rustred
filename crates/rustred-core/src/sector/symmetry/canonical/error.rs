use std::fmt;

use crate::family::IntegralKeyError;
use crate::sector;

/// Typed failures while sealing or applying an exact symmetry action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    EmptyGenerators,
    WrongGeneratorArity {
        generator: usize,
        expected: usize,
        actual: usize,
    },
    InvalidGenerator {
        generator: usize,
        source: usize,
        arity: usize,
    },
    DuplicateGeneratorSource {
        generator: usize,
        source: usize,
    },
    WrongKeyArity {
        expected: usize,
        actual: usize,
    },
    #[cfg(test)]
    WrongPriorityArity {
        expected: usize,
        actual: usize,
    },
    #[cfg(test)]
    UnknownGroupElement {
        ordinal: usize,
        group_order: usize,
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
    OrbitInvariant {
        detail: &'static str,
    },
    Ordering(sector::Error),
    IntegralKey(IntegralKeyError),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyGenerators => formatter.write_str(
                "a canonical symmetry action needs at least one authenticated permutation",
            ),
            Self::WrongGeneratorArity {
                generator,
                expected,
                actual,
            } => write!(
                formatter,
                "authenticated symmetry generator {generator} has arity {actual}, expected {expected}"
            ),
            Self::InvalidGenerator {
                generator,
                source,
                arity,
            } => write!(
                formatter,
                "authenticated symmetry generator {generator} names source {source} outside arity {arity}"
            ),
            Self::DuplicateGeneratorSource { generator, source } => write!(
                formatter,
                "authenticated symmetry generator {generator} repeats source denominator {source}"
            ),
            Self::WrongKeyArity { expected, actual } => write!(
                formatter,
                "integral key has arity {actual}; the symmetry action expects {expected}"
            ),
            #[cfg(test)]
            Self::WrongPriorityArity { expected, actual } => write!(
                formatter,
                "coordinate priority has arity {actual}; the symmetry action expects {expected}"
            ),
            #[cfg(test)]
            Self::UnknownGroupElement {
                ordinal,
                group_order,
            } => write!(
                formatter,
                "canonical symmetry group element {ordinal} is outside a group of order {group_order}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "canonical symmetry {resource} overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "canonical symmetry {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for canonical symmetry {resource}"
            ),
            Self::OrbitInvariant { detail } => {
                write!(
                    formatter,
                    "canonical symmetry orbit invariant failed: {detail}"
                )
            }
            Self::Ordering(error) => write!(formatter, "integral ordering failed: {error}"),
            Self::IntegralKey(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for Error {}

impl From<sector::Error> for Error {
    fn from(value: sector::Error) -> Self {
        Self::Ordering(value)
    }
}

impl From<IntegralKeyError> for Error {
    fn from(value: IntegralKeyError) -> Self {
        Self::IntegralKey(value)
    }
}
