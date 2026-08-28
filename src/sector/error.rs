use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    EmptyIndexSpace,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    IndexOutOfRange {
        position: usize,
        arity: usize,
    },
    DuplicateIndex {
        position: usize,
    },
    InvalidSectorBit {
        position: usize,
        byte: u8,
    },
    InvalidPatternSlot {
        position: usize,
        byte: u8,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    ComplexityOverflow {
        measure: &'static str,
    },
    UnknownOrderingPolicy {
        id: String,
    },
    NotStrictDescent,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIndexSpace => formatter.write_str("a sector needs at least one index"),
            Self::WrongArity { expected, actual } => {
                write!(formatter, "sector arity is {actual}, expected {expected}")
            }
            Self::IndexOutOfRange { position, arity } => write!(
                formatter,
                "sector index {position} is outside an index space of arity {arity}"
            ),
            Self::DuplicateIndex { position } => {
                write!(formatter, "sector index {position} is repeated")
            }
            Self::InvalidSectorBit { position, byte } => write!(
                formatter,
                "invalid sector bit byte {byte} at position {position}; expected 0 or 1"
            ),
            Self::InvalidPatternSlot { position, byte } => write!(
                formatter,
                "invalid sector-pattern byte {byte} at position {position}; expected *, 0, or 1"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} bounded entries for {resource}"
            ),
            Self::ComplexityOverflow { measure } => {
                write!(formatter, "integral {measure} complexity overflowed u128")
            }
            Self::UnknownOrderingPolicy { id } => {
                write!(formatter, "unknown integral-ordering policy {id:?}")
            }
            Self::NotStrictDescent => formatter.write_str(
                "the proposed target is not strictly simpler under the named ordering policy",
            ),
        }
    }
}

impl std::error::Error for Error {}

pub(super) fn try_reserve_exact<T>(
    values: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), Error> {
    let requested = values.len().checked_add(additional).unwrap_or(usize::MAX);
    values
        .try_reserve_exact(additional)
        .map_err(|_| Error::AllocationFailure {
            resource,
            requested,
        })
}

pub(super) fn try_collect_vec<T>(
    values: impl IntoIterator<Item = T>,
    resource: &'static str,
) -> Result<Vec<T>, Error> {
    let iterator = values.into_iter();
    // A non-exact upper hint can be arbitrarily loose. Reserve only the
    // iterator-guaranteed lower bound, then grow through checked seams.
    let (initial, _) = iterator.size_hint();
    let mut retained = Vec::new();
    try_reserve_exact(&mut retained, initial, resource)?;
    for value in iterator {
        if retained.len() == retained.capacity() {
            try_reserve_exact(&mut retained, 1, resource)?;
        }
        retained.push(value);
    }
    Ok(retained)
}

pub(super) fn try_copy_string(source: &str, resource: &'static str) -> Result<String, Error> {
    let mut retained = String::new();
    retained
        .try_reserve_exact(source.len())
        .map_err(|_| Error::AllocationFailure {
            resource,
            requested: source.len(),
        })?;
    retained.push_str(source);
    Ok(retained)
}
