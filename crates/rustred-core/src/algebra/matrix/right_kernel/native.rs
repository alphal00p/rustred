use symbolica::domains::rational::RationalField;
use symbolica::prelude::{Integer, Matrix, Q, Rational, Ring, RingOps, Z};

use super::admission::{
    RightKernelError, check_integer_bits, try_vec, try_zero_bools, try_zero_rationals,
};

pub(super) fn deterministic_rref_kernel(
    reduced: &Matrix<RationalField>,
    rank: usize,
    columns: usize,
) -> Result<Vec<Rational>, RightKernelError> {
    let mut pivot_for_row = try_vec(rank, "RREF pivot rows")?;
    let mut pivot_columns = try_zero_bools(columns, "RREF pivot columns")?;
    for row in 0..rank {
        let pivot = (0..columns)
            .find(|&column| !Q.is_zero(&reduced[(row as u32, column as u32)]))
            .ok_or(RightKernelError::MissingPivot)?;
        if pivot_columns[pivot] {
            return Err(RightKernelError::RepeatedPivot);
        }
        if !Q.is_one(&reduced[(row as u32, pivot as u32)]) {
            return Err(RightKernelError::UnnormalizedPivot);
        }
        pivot_columns[pivot] = true;
        pivot_for_row.push(pivot);
    }
    let free = pivot_columns
        .iter()
        .position(|&pivot| !pivot)
        .ok_or(RightKernelError::MissingFreeColumn)?;
    let mut kernel = try_zero_rationals(columns, "rational right-kernel witness")?;
    kernel[free] = Rational::one();
    for (row, &pivot) in pivot_for_row.iter().enumerate() {
        kernel[pivot] = Q.neg(&reduced[(row as u32, free as u32)]);
    }
    Ok(kernel)
}

pub(super) fn normalize_with_primitive_part(
    rational_kernel: Vec<Rational>,
    columns: usize,
    max_integer_bits: usize,
) -> Result<Vec<Integer>, RightKernelError> {
    let rational_kernel = Matrix::new_vec(rational_kernel, Q).primitive_part();
    if rational_kernel.nrows() != columns || rational_kernel.ncols() != 1 {
        return Err(RightKernelError::NativeShape);
    }
    let mut integers = try_vec(columns, "primitive integer right-kernel witness")?;
    for value in rational_kernel.into_vec() {
        if !value.is_integer() {
            return Err(RightKernelError::NonIntegralPrimitive);
        }
        let integer = value.numerator_ref().clone();
        check_integer_bits(
            &integer,
            "certificate kernel integer bits",
            max_integer_bits,
        )?;
        integers.push(integer);
    }
    if integers.iter().all(Integer::is_zero) {
        return Err(RightKernelError::ZeroPrimitive);
    }
    if integers
        .iter()
        .find(|value| !value.is_zero())
        .is_some_and(Integer::is_negative)
    {
        for value in &mut integers {
            *value = Z.neg(&*value);
        }
    }
    Ok(integers)
}

/// Replay through Symbolica's native `Matrix<Z>` multiplication and return the
/// witness allocation after validation, avoiding a second kernel clone.
pub(in crate::algebra::matrix) fn verify_with_native_product(
    entries: &[u16],
    rows: usize,
    columns: usize,
    kernel: Vec<Integer>,
) -> Result<Vec<Integer>, RightKernelError> {
    if kernel.len() != columns || kernel.iter().all(Integer::is_zero) {
        return Err(RightKernelError::ReplayFailure);
    }
    let mut integer_entries = try_vec(entries.len(), "integer replay matrix")?;
    integer_entries.extend(entries.iter().map(|&entry| Integer::from(i64::from(entry))));
    let matrix = Matrix::from_linear(integer_entries, rows as u32, columns as u32, Z)
        .map_err(|_| RightKernelError::NativeShape)?;
    let kernel_matrix = Matrix::new_vec(kernel, Z);
    let product = &matrix * &kernel_matrix;
    if !product.is_zero() {
        return Err(RightKernelError::ReplayFailure);
    }
    Ok(kernel_matrix.into_vec())
}
