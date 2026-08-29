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
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    ComplexityOverflow {
        measure: &'static str,
    },
    InvalidInteriorBounds {
        position: usize,
        lower: i64,
        upper: i64,
    },
    InteriorOutsideSector {
        position: usize,
        active: bool,
        lower: i64,
        upper: i64,
    },
    EmptyShiftInterior {
        position: usize,
    },
    ShiftNotCovered {
        position: usize,
        shift: i64,
    },
    PivotLeavesParentSector {
        position: usize,
        shift: i64,
    },
    InactiveLineActivation {
        position: usize,
        shift: i64,
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
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} bounded entries for {resource}"
            ),
            Self::ComplexityOverflow { measure } => {
                write!(
                    formatter,
                    "integral {measure} complexity overflowed its exact accumulator"
                )
            }
            Self::InvalidInteriorBounds {
                position,
                lower,
                upper,
            } => write!(
                formatter,
                "sector-interior coordinate {position} has empty bounds [{lower}, {upper}]"
            ),
            Self::InteriorOutsideSector {
                position,
                active,
                lower,
                upper,
            } => write!(
                formatter,
                "sector-interior coordinate {position} has bounds [{lower}, {upper}], which are not wholly {}",
                if *active { "active" } else { "inactive" }
            ),
            Self::EmptyShiftInterior { position } => write!(
                formatter,
                "the requested shifts leave no representable sector interior at coordinate {position}"
            ),
            Self::ShiftNotCovered { position, shift } => write!(
                formatter,
                "sector-interior coordinate {position} does not cover shift {shift}"
            ),
            Self::PivotLeavesParentSector { position, shift } => write!(
                formatter,
                "pivot shift {shift} at position {position} leaves the parent sector on the proposed sector-monotone domain"
            ),
            Self::InactiveLineActivation { position, shift } => write!(
                formatter,
                "positive shift {shift} at inactive position {position} activates a line on the proposed sector-monotone domain"
            ),
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
