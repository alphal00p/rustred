use symbolica::domains::rational::RationalField;
use symbolica::prelude::{Integer, Matrix, Rational};

/// Exact-work budgets for one dense rational right-kernel query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RightKernelLimits {
    pub(crate) max_rows: usize,
    pub(crate) max_columns: usize,
    pub(crate) max_entries: usize,
    pub(crate) max_rank_operations: usize,
    pub(crate) max_rref_integer_bits: usize,
    pub(crate) max_kernel_entries: usize,
    pub(crate) max_kernel_integer_bits: usize,
}

/// Typed failures at the private Symbolica right-kernel boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RightKernelError {
    ZeroColumns,
    Shape {
        rows: usize,
        columns: usize,
        entries: usize,
    },
    DimensionOverflow {
        rows: usize,
        columns: usize,
    },
    CountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
    },
    MissingPivot,
    RepeatedPivot,
    UnnormalizedPivot,
    MissingFreeColumn,
    NonIntegralPrimitive,
    ZeroPrimitive,
    ReplayFailure,
    NativeShape,
    NativePanic,
}

/// Bound exact Gaussian-elimination temporaries from the Leibniz bound
/// `r! M^r <= (r M)^r` for every integer minor. Canonical RREF entries
/// are ratios of minors; one rational product/addition can temporarily need
/// at most twice that many bits plus carry bits.
pub(super) fn preflight_rref_bits(
    entries: &[u16],
    rows: usize,
    columns: usize,
    limit: usize,
) -> Result<usize, RightKernelError> {
    let rank_dimension = rows.min(columns);
    let maximum_entry = entries.iter().copied().max().unwrap_or(1);
    let entry_bits = usize::try_from(u16::BITS - maximum_entry.leading_zeros())
        .map_err(|_| RightKernelError::CountOverflow {
            resource: "rank matrix entry bit length",
        })?
        .max(1);
    let dimension_bits = ceil_log2(rank_dimension.max(1));
    let minor_bits = checked_add(
        checked_mul(
            rank_dimension,
            checked_add(entry_bits, dimension_bits, "RREF minor bit bound")?,
            "RREF minor bit bound",
        )?,
        1,
        "RREF minor bit bound",
    )?;
    let temporary_bits = checked_add(
        checked_mul(2, minor_bits, "RREF integer bit bound")?,
        2,
        "RREF integer bit bound",
    )?;
    check_limit("RREF integer bits", temporary_bits, limit)?;
    Ok(minor_bits)
}

pub(super) fn validate_rational_matrix_bits(
    matrix: &Matrix<RationalField>,
    limit: usize,
) -> Result<(), RightKernelError> {
    for row in matrix.row_iter() {
        for value in row {
            check_integer_bits(value.numerator_ref(), "RREF integer bits", limit)?;
            check_integer_bits(value.denominator_ref(), "RREF integer bits", limit)?;
        }
    }
    Ok(())
}

pub(super) fn check_integer_bits(
    integer: &Integer,
    resource: &'static str,
    limit: usize,
) -> Result<(), RightKernelError> {
    check_limit(resource, integer_bit_length(integer)?, limit)
}

fn integer_bit_length(integer: &Integer) -> Result<usize, RightKernelError> {
    let bits = match integer {
        Integer::Single(value) => u64::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u64::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u64::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| RightKernelError::CountOverflow {
        resource: "integer bit length",
    })
}

fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        (usize::BITS - (value - 1).leading_zeros()) as usize
    }
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), RightKernelError> {
    if requested > limit {
        Err(RightKernelError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, RightKernelError> {
    left.checked_add(right)
        .ok_or(RightKernelError::CountOverflow { resource })
}

pub(super) fn checked_mul(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, RightKernelError> {
    left.checked_mul(right)
        .ok_or(RightKernelError::CountOverflow { resource })
}

pub(super) fn try_vec<T>(
    capacity: usize,
    resource: &'static str,
) -> Result<Vec<T>, RightKernelError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| RightKernelError::AllocationFailure { resource })?;
    Ok(values)
}

pub(super) fn try_zero_bools(
    length: usize,
    resource: &'static str,
) -> Result<Vec<bool>, RightKernelError> {
    let mut values = try_vec(length, resource)?;
    values.resize(length, false);
    Ok(values)
}

pub(super) fn try_zero_integers(
    length: usize,
    resource: &'static str,
) -> Result<Vec<Integer>, RightKernelError> {
    let mut values = try_vec(length, resource)?;
    values.resize_with(length, Integer::zero);
    Ok(values)
}

pub(super) fn try_zero_rationals(
    length: usize,
    resource: &'static str,
) -> Result<Vec<Rational>, RightKernelError> {
    let mut values = try_vec(length, resource)?;
    values.resize_with(length, Rational::zero);
    Ok(values)
}
