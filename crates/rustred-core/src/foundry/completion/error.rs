use std::fmt;

/// Typed failures in finite sector-lattice geometry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CompletionGeometryError {
    EmptyCoordinateSpace,
    WrongArity {
        object: &'static str,
        expected: usize,
        actual: usize,
    },
    IntegralOutsideSector {
        position: usize,
        power: i64,
        active: bool,
    },
    CoordinateNotRepresentable {
        position: usize,
        coordinate: u64,
        active: bool,
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
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for CompletionGeometryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCoordinateSpace => {
                formatter.write_str("completion geometry requires at least one coordinate")
            }
            Self::WrongArity {
                object,
                expected,
                actual,
            } => write!(
                formatter,
                "{object} has arity {actual}, but completion geometry expects {expected}"
            ),
            Self::IntegralOutsideSector {
                position,
                power,
                active,
            } => write!(
                formatter,
                "integral power {power} at position {position} is outside the declared {} slot",
                if *active { "active" } else { "inactive" }
            ),
            Self::CoordinateNotRepresentable {
                position,
                coordinate,
                active,
            } => write!(
                formatter,
                "lattice coordinate {coordinate} at position {position} is not representable in an i64 {} power",
                if *active { "active" } else { "inactive" }
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
                "{resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}"
            ),
            Self::Invariant { detail } => write!(
                formatter,
                "completion geometry reached an internal invariant failure: {detail}"
            ),
        }
    }
}

impl std::error::Error for CompletionGeometryError {}
