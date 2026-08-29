use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::prelude::{Integer, Matrix, Q, Rational};

mod admission;
mod native;

#[cfg(test)]
mod tests;

pub(crate) use admission::{RightKernelError, RightKernelLimits};

use admission::{
    check_limit, checked_mul, preflight_rref_bits, try_vec, try_zero_integers,
    validate_rational_matrix_bits,
};
use native::{
    deterministic_rref_kernel, normalize_with_primitive_part, verify_with_native_product,
};

#[cfg(test)]
pub(super) use native::verify_with_native_product as verify_product_fixture;

/// The deterministic first right-kernel witness, or full column rank.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RightKernelDecision {
    FullColumnRank {
        rank: usize,
    },
    Deficient {
        rank: usize,
        primitive_kernel: Box<[Integer]>,
    },
}

/// Compute one primitive integer vector in the right kernel of an integer
/// matrix. Symbolica owns RREF, rational primitive-part normalization, and the
/// exact integer matrix product used to replay the witness. RustRed owns only
/// the stable convention of selecting the first free RREF column.
pub(crate) fn first_primitive_right_kernel(
    entries: &[u16],
    rows: usize,
    columns: usize,
    limits: RightKernelLimits,
) -> Result<RightKernelDecision, RightKernelError> {
    catch_unwind(AssertUnwindSafe(|| {
        first_primitive_right_kernel_inner(entries, rows, columns, limits)
    }))
    .map_err(|_| RightKernelError::NativePanic)?
}

fn first_primitive_right_kernel_inner(
    entries: &[u16],
    rows: usize,
    columns: usize,
    limits: RightKernelLimits,
) -> Result<RightKernelDecision, RightKernelError> {
    if columns == 0 {
        return Err(RightKernelError::ZeroColumns);
    }
    let expected_entries = checked_mul(rows, columns, "rank matrix entries")?;
    if entries.len() != expected_entries {
        return Err(RightKernelError::Shape {
            rows,
            columns,
            entries: entries.len(),
        });
    }
    check_limit("rank matrix rows", rows, limits.max_rows)?;
    check_limit("rank matrix columns", columns, limits.max_columns)?;
    check_limit("rank matrix entries", expected_entries, limits.max_entries)?;
    if rows > u32::MAX as usize
        || columns > u32::MAX as usize
        || expected_entries > u32::MAX as usize
    {
        return Err(RightKernelError::DimensionOverflow { rows, columns });
    }

    if rows == 0 {
        check_limit(
            "certificate kernel entries",
            columns,
            limits.max_kernel_entries,
        )?;
        check_limit(
            "certificate kernel integer bits",
            1,
            limits.max_kernel_integer_bits,
        )?;
        let mut primitive_kernel = try_zero_integers(columns, "right-kernel witness")?;
        primitive_kernel[0] = Integer::one();
        let primitive_kernel =
            verify_with_native_product(entries, rows, columns, primitive_kernel)?;
        return Ok(RightKernelDecision::Deficient {
            rank: 0,
            primitive_kernel: primitive_kernel.into_boxed_slice(),
        });
    }

    let rank_operations = checked_mul(expected_entries, rows.min(columns), "rank operations")?;
    check_limit(
        "rank operations",
        rank_operations,
        limits.max_rank_operations,
    )?;
    let minor_bit_bound =
        preflight_rref_bits(entries, rows, columns, limits.max_rref_integer_bits)?;

    let mut rational_entries = try_vec(expected_entries, "rational rank matrix")?;
    rational_entries.extend(
        entries
            .iter()
            .map(|&entry| Rational::from(i64::from(entry))),
    );
    let mut reduced = Matrix::from_linear(rational_entries, rows as u32, columns as u32, Q)
        .map_err(|_| RightKernelError::NativeShape)?;
    let rank = reduced.row_reduce(columns as u32);
    validate_rational_matrix_bits(&reduced, limits.max_rref_integer_bits)?;
    if rank == columns {
        return Ok(RightKernelDecision::FullColumnRank { rank });
    }
    if rank > rows.min(columns) {
        return Err(RightKernelError::MissingPivot);
    }

    check_limit(
        "certificate kernel entries",
        columns,
        limits.max_kernel_entries,
    )?;
    let kernel_bit_bound = checked_mul(
        columns,
        minor_bit_bound,
        "certificate kernel integer bit bound",
    )?;
    check_limit(
        "certificate kernel integer bits",
        kernel_bit_bound,
        limits.max_kernel_integer_bits,
    )?;

    let rational_kernel = deterministic_rref_kernel(&reduced, rank, columns)?;
    drop(reduced);
    let primitive_kernel =
        normalize_with_primitive_part(rational_kernel, columns, limits.max_kernel_integer_bits)?;
    let primitive_kernel = verify_with_native_product(entries, rows, columns, primitive_kernel)?;

    Ok(RightKernelDecision::Deficient {
        rank,
        primitive_kernel: primitive_kernel.into_boxed_slice(),
    })
}
