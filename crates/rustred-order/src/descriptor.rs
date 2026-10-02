use std::fmt;

/// Direction of one final coordinate tie, in simpler-first orientation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoordinateGroups {
    ActiveFirst,
    InactiveFirst,
    Interleaved,
}

/// One nonnegative sign-specific linear form. In `degree_rows` it acts on
/// excess (active n-1, inactive -n); in `pre_support_degree_rows` it acts on
/// absolute powers (active n, inactive -n). The containing field fixes the
/// interpretation; moving a row between them changes its meaning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DegreeRow {
    pub active: Vec<u64>,
    pub inactive: Vec<u64>,
}

/// Finite declarative order program. Priority vectors list coordinate indices
/// in comparison order, not ranks by slot. All rows compare ascending.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderDescriptor {
    /// Absolute-power rows evaluated before any support comparison. Empty
    /// preserves the legacy support-first order and canonical v1 encoding.
    pub pre_support_degree_rows: Vec<DegreeRow>,
    pub support_weights: Vec<u64>,
    pub support_priority: Vec<usize>,
    pub degree_rows: Vec<DegreeRow>,
    pub coordinate_priority: Vec<usize>,
    pub coordinate_groups: CoordinateGroups,
    pub active_direction: Direction,
    pub inactive_direction: Direction,
}

/// Resource policy, not a family-specific arity or a small fixed row language.
/// Limits apply before decoder allocation. Encoded bytes are not a peak-RSS
/// estimate: a compiled program retains both decoded data and canonical bytes.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_encoded_bytes: usize,
    /// Conservative scalar terms in a full two-sided comparison.
    pub max_comparison_terms: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_encoded_bytes: 1024 * 1024,
            max_comparison_terms: 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    EmptyArity,
    NoDegreeRows,
    Arity {
        expected: usize,
        actual: usize,
    },
    InvalidPermutation,
    ZeroDegreeRow {
        row: usize,
    },
    UncoveredCoordinate {
        axis: usize,
        active: bool,
    },
    WeightSumOverflow,
    DimensionOverflow,
    ResourceLimit {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
    Allocation,
    InvalidEncoding,
    ExcessOutOfRange {
        axis: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyArity => f.write_str("integral order requires nonzero arity"),
            Self::NoDegreeRows => f.write_str("integral order requires a degree row"),
            Self::Arity { expected, actual } => {
                write!(f, "order arity {expected}, received {actual}")
            }
            Self::InvalidPermutation => {
                f.write_str("order priority must be a complete permutation")
            }
            Self::ZeroDegreeRow { row } => write!(f, "order degree row {row} is identically zero"),
            Self::UncoveredCoordinate { axis, active } => write!(
                f,
                "order coordinate {axis} has no positive weight for active={active}"
            ),
            Self::WeightSumOverflow => {
                f.write_str("order weight sum exceeds the checked u64 bound")
            }
            Self::DimensionOverflow => f.write_str("order dimension arithmetic overflow"),
            Self::ResourceLimit {
                resource,
                actual,
                limit,
            } => write!(f, "order {resource} requires {actual}, limit {limit}"),
            Self::Allocation => f.write_str("order allocation failed"),
            Self::InvalidEncoding => f.write_str("invalid or unsupported integral-order encoding"),
            Self::ExcessOutOfRange { axis } => write!(
                f,
                "order excess at coordinate {axis} is not derived from an i64 power or shift"
            ),
        }
    }
}
impl std::error::Error for Error {}

pub(crate) fn arity(expected: usize, actual: usize) -> Result<(), Error> {
    if expected == actual {
        Ok(())
    } else {
        Err(Error::Arity { expected, actual })
    }
}

pub(crate) fn reserve<T>(count: usize) -> Result<Vec<T>, Error> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Error::Allocation)?;
    Ok(result)
}

pub(crate) fn permutation(values: &[usize], size: usize) -> Result<(), Error> {
    arity(size, values.len())?;
    let mut seen = reserve(size)?;
    seen.resize(size, false);
    for &axis in values {
        if axis >= size || seen[axis] {
            return Err(Error::InvalidPermutation);
        }
        seen[axis] = true;
    }
    Ok(())
}

impl OrderDescriptor {
    pub(crate) fn dimensions(
        size: usize,
        pre_rows: usize,
        rows: usize,
        limits: Limits,
    ) -> Result<usize, Error> {
        if size == 0 {
            return Err(Error::EmptyArity);
        }
        let rows = pre_rows.checked_add(rows).ok_or(Error::DimensionOverflow)?;
        if rows == 0 {
            return Err(Error::NoDegreeRows);
        }
        let twice_rows = rows.checked_mul(2).ok_or(Error::DimensionOverflow)?;
        let words = size
            .checked_mul(twice_rows.checked_add(3).ok_or(Error::DimensionOverflow)?)
            .ok_or(Error::DimensionOverflow)?;
        let bytes = words
            .checked_mul(8)
            .and_then(|n| n.checked_add(if pre_rows == 0 { 27 } else { 35 }))
            .ok_or(Error::DimensionOverflow)?;
        let comparison_terms = size
            .checked_mul(twice_rows.checked_add(8).ok_or(Error::DimensionOverflow)?)
            .ok_or(Error::DimensionOverflow)?;
        for (resource, actual, limit) in [
            ("encoded bytes", bytes, limits.max_encoded_bytes),
            (
                "comparison terms",
                comparison_terms,
                limits.max_comparison_terms,
            ),
        ] {
            if actual > limit {
                return Err(Error::ResourceLimit {
                    resource,
                    actual,
                    limit,
                });
            }
        }
        Ok(bytes)
    }

    pub(crate) fn validate(&self, limits: Limits) -> Result<(), Error> {
        let size = self.support_weights.len();
        Self::dimensions(
            size,
            self.pre_support_degree_rows.len(),
            self.degree_rows.len(),
            limits,
        )?;
        permutation(&self.support_priority, size)?;
        permutation(&self.coordinate_priority, size)?;
        checked_sum(self.support_weights.iter().copied())?;
        let mut covered_active = reserve(size)?;
        let mut covered_inactive = reserve(size)?;
        covered_active.resize(size, false);
        covered_inactive.resize(size, false);
        for (row, weights) in self
            .pre_support_degree_rows
            .iter()
            .chain(&self.degree_rows)
            .enumerate()
        {
            arity(size, weights.active.len())?;
            arity(size, weights.inactive.len())?;
            // A power or offset contributes at most 2^63 in magnitude. This
            // joint L1 bound, including both signs, makes every signed sum
            // <= (2^64-1)*2^63 < i128::MAX without unchecked cancellation.
            let sum = checked_sum(weights.active.iter().chain(&weights.inactive).copied())?;
            if sum == 0 {
                return Err(Error::ZeroDegreeRow { row });
            }
            for axis in 0..size {
                covered_active[axis] |= weights.active[axis] != 0;
                covered_inactive[axis] |= weights.inactive[axis] != 0;
            }
        }
        for axis in 0..size {
            if !covered_active[axis] {
                return Err(Error::UncoveredCoordinate { axis, active: true });
            }
            if !covered_inactive[axis] {
                return Err(Error::UncoveredCoordinate {
                    axis,
                    active: false,
                });
            }
        }
        Ok(())
    }
}

fn checked_sum(mut values: impl Iterator<Item = u64>) -> Result<u64, Error> {
    values.try_fold(0u64, |sum, value| {
        sum.checked_add(value).ok_or(Error::WeightSumOverflow)
    })
}
