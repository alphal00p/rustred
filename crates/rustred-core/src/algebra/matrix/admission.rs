//! Matrix shape admission, resource bounds, and retained-value census.

use std::cell::RefCell;
use std::rc::Rc;

use symbolica::prelude::{Matrix, Ring};

use crate::algebra::{
    Coefficient, CoefficientContext, ExactAlgebraLimits,
    coefficient_clone_owned_retained_byte_bound,
};

use super::error::SymbolicaCoefficientMatrixError;
use super::field::{CheckedCoefficientField, CheckedFieldState, call_native};

const DEFAULT_MAX_SINGLE_MATRIX_ENTRIES: usize = 16_000_000;
const DEFAULT_MAX_LIVE_MATRIX_ENTRIES: usize = 32_000_000;
pub(crate) const DEFAULT_MAX_EXACT_OPERATIONS: usize = 100_000_000;
pub(crate) const DEFAULT_MAX_INPUT_RETAINED_BYTES: usize = 1024 * 1024 * 1024;
pub(crate) const DEFAULT_MAX_OUTPUT_RETAINED_BYTES: usize = 1024 * 1024 * 1024;

/// Admission policy for one bounded Symbolica coefficient or matrix session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SymbolicaCoefficientMatrixLimits {
    pub(crate) exact_algebra: ExactAlgebraLimits,
    /// Largest individual native matrix payload.  General inversion needs the
    /// augmented `n x 2n` matrix here.
    pub(crate) max_single_matrix_entries: usize,
    /// Largest conservative simultaneously-live native payload.
    pub(crate) max_live_matrix_entries: usize,
    /// Largest number of checked exact arithmetic operations admitted for the
    /// complete requested native operation. Constant construction and
    /// zero/one predicates are censused separately.
    pub(crate) max_exact_operations: usize,
    /// Aggregate clone-owned retained bytes in authenticated caller inputs.
    pub(crate) max_input_retained_bytes: usize,
    /// Aggregate clone-owned retained bytes in powers, determinants, inverses,
    /// and verification-product outputs inspected during the native session.
    pub(crate) max_output_retained_bytes: usize,
}

impl SymbolicaCoefficientMatrixLimits {
    /// Adapt the historical family limit, which bounds the `n x 2n` augmented
    /// matrix, to this module's individual and live-payload limits.
    pub(crate) const fn for_family(
        exact_algebra: ExactAlgebraLimits,
        max_augmented_entries: usize,
        max_exact_operations: usize,
        max_input_retained_bytes: usize,
        max_output_retained_bytes: usize,
    ) -> Self {
        Self {
            exact_algebra,
            max_single_matrix_entries: max_augmented_entries,
            max_live_matrix_entries: max_augmented_entries.saturating_mul(2),
            max_exact_operations,
            max_input_retained_bytes,
            max_output_retained_bytes,
        }
    }
}

impl Default for SymbolicaCoefficientMatrixLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_single_matrix_entries: DEFAULT_MAX_SINGLE_MATRIX_ENTRIES,
            max_live_matrix_entries: DEFAULT_MAX_LIVE_MATRIX_ENTRIES,
            max_exact_operations: DEFAULT_MAX_EXACT_OPERATIONS,
            max_input_retained_bytes: DEFAULT_MAX_INPUT_RETAINED_BYTES,
            max_output_retained_bytes: DEFAULT_MAX_OUTPUT_RETAINED_BYTES,
        }
    }
}

/// Exact census of one admitted native coefficient or matrix session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SymbolicaCoefficientMatrixStats {
    pub(super) input_entries: usize,
    pub(super) output_entries: usize,
    pub(super) authenticated_entries: usize,
    pub(super) admitted_single_matrix_entries: usize,
    pub(super) admitted_peak_live_entries: usize,
    pub(super) admitted_exact_operations: usize,
    pub(super) input_retained_bytes: usize,
    pub(super) output_retained_bytes: usize,
    pub(super) exact_operations: usize,
    pub(super) additions: usize,
    pub(super) subtractions: usize,
    pub(super) multiplications: usize,
    pub(super) divisions: usize,
    pub(super) negations: usize,
    pub(super) zero_constants: usize,
    pub(super) one_constants: usize,
    pub(super) zero_tests: usize,
    pub(super) one_tests: usize,
    pub(super) determinant_calls: usize,
    pub(super) inverse_calls: usize,
    pub(super) product_calls: usize,
    pub(super) transpose_calls: usize,
    pub(super) rank_calls: usize,
    pub(super) power_calls: usize,
    pub(super) admitted_power_exponent: u64,
    pub(super) admitted_power_term_operations: usize,
    pub(super) admitted_power_numerator_terms: usize,
    pub(super) admitted_power_denominator_terms: usize,
    pub(super) output_power_numerator_terms: usize,
    pub(super) output_power_denominator_terms: usize,
    pub(super) non_matrix_trait_calls: usize,
}

impl SymbolicaCoefficientMatrixStats {
    pub(crate) const fn admitted_single_matrix_entries(self) -> usize {
        self.admitted_single_matrix_entries
    }

    pub(crate) const fn admitted_peak_live_entries(self) -> usize {
        self.admitted_peak_live_entries
    }

    pub(crate) const fn admitted_exact_operations(self) -> usize {
        self.admitted_exact_operations
    }

    pub(crate) const fn exact_operations(self) -> usize {
        self.exact_operations
    }

    pub(crate) const fn input_retained_bytes(self) -> usize {
        self.input_retained_bytes
    }

    pub(crate) const fn output_retained_bytes(self) -> usize {
        self.output_retained_bytes
    }

    pub(crate) const fn determinant_calls(self) -> usize {
        self.determinant_calls
    }

    pub(crate) const fn product_calls(self) -> usize {
        self.product_calls
    }

    pub(crate) const fn transpose_calls(self) -> usize {
        self.transpose_calls
    }

    pub(crate) const fn rank_calls(self) -> usize {
        self.rank_calls
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct MatrixShape {
    pub(super) rows: usize,
    pub(super) columns: usize,
    pub(super) rows_u32: u32,
    pub(super) columns_u32: u32,
    pub(super) entries: usize,
}

pub(super) fn inspect_rows(
    rows: &[Vec<Coefficient>],
) -> Result<MatrixShape, SymbolicaCoefficientMatrixError> {
    if rows.is_empty() {
        return Err(SymbolicaCoefficientMatrixError::EmptyMatrix);
    }
    let columns = rows[0].len();
    if columns == 0 {
        return Err(SymbolicaCoefficientMatrixError::EmptyMatrix);
    }
    if let Some((row, actual_columns)) = rows
        .iter()
        .enumerate()
        .find_map(|(row, values)| (values.len() != columns).then_some((row, values.len())))
    {
        return Err(SymbolicaCoefficientMatrixError::RaggedMatrix {
            row,
            expected_columns: columns,
            actual_columns,
        });
    }
    checked_shape(rows.len(), columns)
}

pub(super) fn checked_shape(
    rows: usize,
    columns: usize,
) -> Result<MatrixShape, SymbolicaCoefficientMatrixError> {
    let rows_u32 = u32::try_from(rows)
        .map_err(|_| SymbolicaCoefficientMatrixError::DimensionOverflow { rows, columns })?;
    let columns_u32 = u32::try_from(columns)
        .map_err(|_| SymbolicaCoefficientMatrixError::DimensionOverflow { rows, columns })?;
    let entries = rows.checked_mul(columns).ok_or(
        SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "coefficient matrix entries",
        },
    )?;
    rows_u32
        .checked_mul(columns_u32)
        .ok_or(SymbolicaCoefficientMatrixError::DimensionOverflow { rows, columns })?;
    Ok(MatrixShape {
        rows,
        columns,
        rows_u32,
        columns_u32,
        entries,
    })
}

pub(super) fn require_square(shape: MatrixShape) -> Result<usize, SymbolicaCoefficientMatrixError> {
    if shape.rows == shape.columns {
        Ok(shape.rows)
    } else {
        Err(SymbolicaCoefficientMatrixError::NotSquare {
            rows: shape.rows,
            columns: shape.columns,
        })
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    left.checked_add(right)
        .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    left.checked_mul(right)
        .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SymbolicaCoefficientMatrixError> {
    if requested > limit {
        Err(SymbolicaCoefficientMatrixError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn increment_session_counter(
    state: &Rc<RefCell<CheckedFieldState>>,
    resource: &'static str,
    select: impl FnOnce(&mut SymbolicaCoefficientMatrixStats) -> &mut usize,
) -> Result<(), SymbolicaCoefficientMatrixError> {
    let mut state = state.borrow_mut();
    let counter = select(&mut state.stats);
    *counter = counter
        .checked_add(1)
        .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow { resource })?;
    Ok(())
}

pub(super) fn square_sum(bound: usize) -> Result<usize, SymbolicaCoefficientMatrixError> {
    let a = bound;
    let b = bound
        .checked_add(1)
        .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "Symbolica determinant operation bound",
        })?;
    let c = bound
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "Symbolica determinant operation bound",
        })?;
    // Cancel 2 and 3 before multiplying to avoid rejecting representable sums.
    let mut factors = [a, b, c];
    let even = factors.iter().position(|value| value % 2 == 0).unwrap_or(0);
    factors[even] /= 2;
    let by_three = factors.iter().position(|value| value % 3 == 0).unwrap_or(0);
    factors[by_three] /= 3;
    checked_mul(
        "Symbolica determinant operation bound",
        checked_mul(
            "Symbolica determinant operation bound",
            factors[0],
            factors[1],
        )?,
        factors[2],
    )
}

pub(super) fn determinant_operation_bound(
    size: usize,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    match size {
        0 => Ok(0),
        1 => Ok(0),
        2 => Ok(3),
        3 => Ok(14),
        _ => {
            let cells = square_sum(size - 1)?;
            let divisions = square_sum(size - 2)?;
            checked_add(
                "Symbolica determinant operation bound",
                checked_add(
                    "Symbolica determinant operation bound",
                    checked_mul("Symbolica determinant operation bound", 3, cells)?,
                    divisions,
                )?,
                1,
            )
        }
    }
}

pub(super) fn inverse_operation_bound(
    size: usize,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    match size {
        0 => Ok(0),
        1 => Ok(4),
        2 => Ok(10),
        3 => Ok(42),
        _ => {
            let cube = checked_mul(
                "Symbolica inverse operation bound",
                checked_mul("Symbolica inverse operation bound", size, size)?,
                size,
            )?;
            let square = checked_mul("Symbolica inverse operation bound", size, size)?;
            let positive = checked_add(
                "Symbolica inverse operation bound",
                checked_mul("Symbolica inverse operation bound", 3, cube)?,
                checked_mul("Symbolica inverse operation bound", 3, size)?,
            )?;
            positive
                .checked_sub(checked_mul("Symbolica inverse operation bound", 2, square)?)
                .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow {
                    resource: "Symbolica inverse operation bound",
                })
        }
    }
}

pub(super) fn product_operation_bound(
    rows: usize,
    inner: usize,
    columns: usize,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    checked_mul(
        "Symbolica matrix product operation bound",
        2,
        checked_mul(
            "Symbolica matrix product operation bound",
            checked_mul("Symbolica matrix product operation bound", rows, inner)?,
            columns,
        )?,
    )
}

pub(super) fn square_representation_bounds(
    size: usize,
) -> Result<(usize, usize, usize), SymbolicaCoefficientMatrixError> {
    let shape = checked_shape(size, size)?;
    let doubled =
        size.checked_mul(2)
            .ok_or(SymbolicaCoefficientMatrixError::DimensionOverflow {
                rows: size,
                columns: size,
            })?;
    let doubled_u32 =
        u32::try_from(doubled).map_err(|_| SymbolicaCoefficientMatrixError::DimensionOverflow {
            rows: size,
            columns: doubled,
        })?;
    shape.rows_u32.checked_mul(doubled_u32).ok_or(
        SymbolicaCoefficientMatrixError::DimensionOverflow {
            rows: size,
            columns: doubled,
        },
    )?;
    let augmented = checked_mul("augmented Symbolica matrix entries", shape.entries, 2)?;
    let peak_live = checked_mul("live Symbolica matrix entries", shape.entries, 4)?;
    Ok((shape.entries, augmented, peak_live))
}

pub(super) fn validate_rows(
    context: &CoefficientContext,
    rows: &[Vec<Coefficient>],
    limits: ExactAlgebraLimits,
) -> Result<(), SymbolicaCoefficientMatrixError> {
    for (row, values) in rows.iter().enumerate() {
        for (column, coefficient) in values.iter().enumerate() {
            context
                .validate_with_limits(coefficient, limits)
                .map_err(
                    |error| SymbolicaCoefficientMatrixError::InvalidCoefficient {
                        row,
                        column,
                        error,
                    },
                )?;
        }
    }
    Ok(())
}

pub(super) fn coefficient_retained_bytes(
    coefficient: &Coefficient,
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    coefficient_clone_owned_retained_byte_bound(coefficient).ok_or(
        SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "coefficient matrix retained bytes",
        },
    )
}

pub(super) fn rows_retained_bytes(
    rows: &[Vec<Coefficient>],
) -> Result<usize, SymbolicaCoefficientMatrixError> {
    let mut bytes = 0usize;
    for coefficient in rows.iter().flatten() {
        bytes = checked_add(
            "coefficient matrix input retained bytes",
            bytes,
            coefficient_retained_bytes(coefficient)?,
        )?;
    }
    Ok(bytes)
}

pub(super) fn authenticate_output_coefficient(
    context: &CoefficientContext,
    coefficient: &Coefficient,
    limits: SymbolicaCoefficientMatrixLimits,
    state: &Rc<RefCell<CheckedFieldState>>,
) -> Result<(), SymbolicaCoefficientMatrixError> {
    context
        .validate_with_limits(coefficient, limits.exact_algebra)
        .map_err(SymbolicaCoefficientMatrixError::ExactAlgebra)?;
    let bytes = coefficient_retained_bytes(coefficient)?;
    let mut state = state.borrow_mut();
    let prospective = checked_add(
        "coefficient matrix output retained bytes",
        state.stats.output_retained_bytes,
        bytes,
    )?;
    check_limit(
        "coefficient matrix output retained bytes",
        prospective,
        limits.max_output_retained_bytes,
    )?;
    state.stats.output_retained_bytes = prospective;
    state.stats.authenticated_entries = checked_add(
        "authenticated Symbolica matrix entries",
        state.stats.authenticated_entries,
        1,
    )?;
    Ok(())
}

pub(super) fn matrix_from_rows<'context>(
    rows: &[Vec<Coefficient>],
    shape: MatrixShape,
    field: CheckedCoefficientField<'context>,
) -> Result<Matrix<CheckedCoefficientField<'context>>, SymbolicaCoefficientMatrixError> {
    validate_rows(field.context, rows, field.limits.exact_algebra)?;
    let retained_bytes = rows_retained_bytes(rows)?;
    {
        let mut state = field.state.borrow_mut();
        let prospective_bytes = checked_add(
            "coefficient matrix input retained bytes",
            state.stats.input_retained_bytes,
            retained_bytes,
        )?;
        check_limit(
            "coefficient matrix input retained bytes",
            prospective_bytes,
            field.limits.max_input_retained_bytes,
        )?;
        state.stats.input_retained_bytes = prospective_bytes;
        state.stats.input_entries = checked_add(
            "coefficient matrix input entries",
            state.stats.input_entries,
            shape.entries,
        )?;
    }
    let mut data = Vec::new();
    data.try_reserve_exact(shape.entries).map_err(|_| {
        SymbolicaCoefficientMatrixError::AllocationFailure {
            resource: "coefficient matrix entries",
            requested: shape.entries,
        }
    })?;
    for row in rows {
        data.extend(row.iter().cloned());
    }
    call_native("construction", || {
        Matrix::from_linear(data, shape.rows_u32, shape.columns_u32, field)
    })?
    .map_err(|_| SymbolicaCoefficientMatrixError::InternalShapeFailure {
        operation: "construction",
    })
}

pub(super) fn authenticate_native<F>(
    context: &CoefficientContext,
    matrix: &Matrix<F>,
    limits: SymbolicaCoefficientMatrixLimits,
    state: &Rc<RefCell<CheckedFieldState>>,
) -> Result<(), SymbolicaCoefficientMatrixError>
where
    F: Ring<Element = Coefficient>,
{
    let mut retained_bytes = 0usize;
    for (offset, coefficient) in matrix.iter().enumerate() {
        let columns = matrix.ncols();
        context
            .validate_with_limits(coefficient, limits.exact_algebra)
            .map_err(
                |error| SymbolicaCoefficientMatrixError::InvalidCoefficient {
                    row: offset / columns,
                    column: offset % columns,
                    error,
                },
            )?;
        retained_bytes = checked_add(
            "coefficient matrix output retained bytes",
            retained_bytes,
            coefficient_retained_bytes(coefficient)?,
        )?;
    }
    let count = matrix.nrows().checked_mul(matrix.ncols()).ok_or(
        SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "authenticated Symbolica matrix entries",
        },
    )?;
    let mut state = state.borrow_mut();
    let prospective_bytes = checked_add(
        "coefficient matrix output retained bytes",
        state.stats.output_retained_bytes,
        retained_bytes,
    )?;
    check_limit(
        "coefficient matrix output retained bytes",
        prospective_bytes,
        limits.max_output_retained_bytes,
    )?;
    state.stats.output_retained_bytes = prospective_bytes;
    state.stats.authenticated_entries = checked_add(
        "authenticated Symbolica matrix entries",
        state.stats.authenticated_entries,
        count,
    )?;
    Ok(())
}

pub(super) fn native_into_rows<F>(
    matrix: Matrix<F>,
    state: &Rc<RefCell<CheckedFieldState>>,
) -> Result<Vec<Vec<Coefficient>>, SymbolicaCoefficientMatrixError>
where
    F: Ring<Element = Coefficient>,
{
    let rows = matrix.nrows();
    let columns = matrix.ncols();
    let entries = rows.checked_mul(columns).ok_or(
        SymbolicaCoefficientMatrixError::ResourceCountOverflow {
            resource: "coefficient matrix output entries",
        },
    )?;
    let mut output = Vec::new();
    output.try_reserve_exact(rows).map_err(|_| {
        SymbolicaCoefficientMatrixError::AllocationFailure {
            resource: "coefficient matrix output rows",
            requested: rows,
        }
    })?;
    let mut data = matrix.into_vec().into_iter();
    for _ in 0..rows {
        let mut row = Vec::new();
        row.try_reserve_exact(columns).map_err(|_| {
            SymbolicaCoefficientMatrixError::AllocationFailure {
                resource: "coefficient matrix output entries",
                requested: columns,
            }
        })?;
        row.extend(data.by_ref().take(columns));
        output.push(row);
    }
    if data.next().is_some() || output.iter().map(Vec::len).sum::<usize>() != entries {
        return Err(SymbolicaCoefficientMatrixError::InternalShapeFailure {
            operation: "output conversion",
        });
    }
    let mut state = state.borrow_mut();
    state.stats.output_entries = checked_add(
        "coefficient matrix output entries",
        state.stats.output_entries,
        entries,
    )?;
    Ok(output)
}
