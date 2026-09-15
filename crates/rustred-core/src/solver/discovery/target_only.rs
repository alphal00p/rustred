//! Native target-block GPLU and a one-shot full identity reconstruction.
//!
//! Write the unchanged source prefix as A=[F|R], with F ending at the target.
//! If every F row is independent, its GPLU has exactly the same pivots and
//! scales as full-A GPLU. With F=L U, native L^T w=e_j therefore recovers the
//! same monic target U row as the default backend, not another span solution.
//! The final native product checks every forbidden column and the unit target.
//! These temporary frame weights are not original-generator certificates.

use symbolica::domains::SelfRing;
use symbolica::prelude::Z;
use symbolica::tensors::sparse::{LuLMode, SparseMatrix, SparseRowReducer};

use crate::algebra::Coefficient;

use super::variables::FrameVariables;
use super::{
    ExactField, ExactRow, Integral, IntegralOrder, MaterializationError, MaterializationEvent, Term,
};

fn invalid(reason: &'static str) -> MaterializationError {
    MaterializationError::TargetOnlyInvalidDecomposition(reason)
}

pub(super) fn materialize<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    mut observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    // Common preflight checked the full column count, distinct sorted source
    // terms and every coefficient map. The trailing zero sentinel is retained.
    let block_columns =
        u32::try_from(target_column + 2).map_err(|_| MaterializationError::TooManyColumns)?;
    let mut reducer = SparseRowReducer::new(block_columns, ExactField::new(Z), LuLMode::Full);
    observe(MaterializationEvent::TargetBlockStarted {
        columns: target_column + 1,
    });
    let mut values = Vec::new();
    let mut ids = Vec::new();
    for (ordinal, row) in rows.iter().enumerate() {
        values.clear();
        ids.clear();
        for term in row {
            if term.coefficient.is_zero() {
                continue;
            }
            let column = column_id(columns, order, &term.integral);
            if column > target_column {
                break;
            }
            values.push(variables.map_coefficient(&term.coefficient)?);
            ids.push(column as u32);
        }
        observe(MaterializationEvent::RowStarted {
            row: ordinal + 1,
            input_nonzeros: values.len(),
            reducer_rows: reducer.u().nrows() as usize,
            reducer_nonzeros: reducer.u().nvalues(),
        });
        let pivot = reducer.add_row(&values, &ids);
        observe(MaterializationEvent::RowFinished {
            row: ordinal + 1,
            pivot: pivot.map(|column| columns[column as usize]),
            reducer_rows: reducer.u().nrows() as usize,
            reducer_nonzeros: reducer.u().nvalues(),
        });
        if pivot.is_none() {
            return Err(MaterializationError::TargetOnlyDependentPrefix { row: ordinal + 1 });
        }
        let prefix_len = ordinal + 1;
        if reducer.l().nrows() as usize != prefix_len || reducer.l().ncols() as usize != prefix_len
        {
            return Err(invalid(
                "L does not represent the complete independent source prefix",
            ));
        }
        if pivot != Some(target_column as u32) {
            continue;
        }
        observe(MaterializationEvent::TargetWeightsStarted {
            rows: prefix_len,
            lower_nonzeros: reducer.l().nvalues(),
        });
        let weights = solve_transposed_lower(reducer.l(), ordinal)?;
        observe(MaterializationEvent::TargetWeightsFinished {
            nonzero_weights: weights.nvalues(),
        });
        // Do not retain the full exact L/U alongside the reconstructed A.
        drop(reducer);
        observe(MaterializationEvent::TargetReconstructionStarted {
            rows: prefix_len,
            columns: columns.len(),
        });
        let result = reconstruct(
            &rows[..prefix_len],
            columns,
            order,
            target_column,
            variables,
            weights,
        )?;
        observe(MaterializationEvent::TargetReconstructionFinished {
            output_terms: result.len(),
        });
        return Ok(result);
    }
    Err(MaterializationError::TargetNotPivot)
}

/// Transpose sparse structure only; all normalization/back substitution is
/// native. L columns need not be sorted in pivot-acceptance order. Iterating
/// its source rows in order nevertheless builds sorted rows of L^T.
fn solve_transposed_lower(
    lower: &SparseMatrix<ExactField>,
    target_row: usize,
) -> Result<SparseMatrix<ExactField>, MaterializationError> {
    let count = lower.nrows() as usize;
    if count == 0 || lower.ncols() as usize != count || target_row >= count {
        return Err(invalid(
            "L must be nonempty and square with a valid target row",
        ));
    }
    let template = lower.values().first().ok_or_else(|| invalid("empty L"))?;
    let one: Coefficient = template.numerator.one().into();
    let mut transposed: Vec<Vec<(u32, &Coefficient)>> = vec![Vec::new(); count];
    for source in 0..count {
        for position in lower.row_ptrs()[source]..lower.row_ptrs()[source + 1] {
            let column = lower.col_idcs()[position] as usize;
            if column > source {
                return Err(invalid("L contains an entry above its diagonal"));
            }
            transposed[column].push((source as u32, &lower.values()[position]));
        }
    }
    let augmented_columns = lower
        .ncols()
        .checked_add(1)
        .ok_or(MaterializationError::TooManyColumns)?;
    let mut solver = SparseRowReducer::new(augmented_columns, ExactField::new(Z), LuLMode::None);
    for (row, terms) in transposed.into_iter().enumerate() {
        if terms
            .first()
            .is_none_or(|(column, value)| *column as usize != row || value.is_zero())
            || terms.windows(2).any(|pair| pair[0].0 >= pair[1].0)
        {
            return Err(invalid(
                "L has a missing/zero diagonal or repeated source entry",
            ));
        }
        let mut values: Vec<_> = terms.iter().map(|(_, value)| (*value).clone()).collect();
        let mut ids: Vec<_> = terms.iter().map(|(column, _)| *column).collect();
        if row == target_row {
            ids.push(lower.ncols());
            values.push(one.clone());
        }
        // Each upper-triangular row takes the native direct-normalization path.
        // Raw nonunit pivots must not go to from_upper_triangular_matrix.
        if solver.add_row(&values, &ids) != Some(row as u32) {
            return Err(invalid(
                "native triangular normalization changed the expected pivot",
            ));
        }
    }
    solver.back_substitute();
    let mut values = Vec::new();
    let mut ids = Vec::new();
    let upper = solver.u();
    for variable in 0..count {
        let row = solver.pivots()[variable]
            .ok_or_else(|| invalid("native triangular solution is missing a pivot"))?
            as usize;
        for position in upper.row_ptrs()[row]..upper.row_ptrs()[row + 1] {
            let column = upper.col_idcs()[position] as usize;
            let value = &upper.values()[position];
            if column == count {
                if !value.is_zero() {
                    values.push(value.clone());
                    ids.push(variable as u32);
                }
            } else if column != variable || !value.is_one() {
                return Err(invalid(
                    "native triangular solution retains an unsolved coefficient",
                ));
            }
        }
    }
    let mut result = SparseMatrix::new(0, lower.ncols(), ExactField::new(Z));
    result.add_row(values, ids);
    Ok(result)
}

fn reconstruct<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    weights: SparseMatrix<ExactField>,
) -> Result<ExactRow<N>, MaterializationError> {
    if weights.nrows() != 1 || weights.ncols() as usize != rows.len() {
        return Err(invalid(
            "source weights do not match the unchanged source prefix",
        ));
    }
    let mut original = SparseMatrix::new(0, columns.len() as u32, ExactField::new(Z));
    for row in rows {
        let mut values = Vec::with_capacity(row.len());
        let mut ids = Vec::with_capacity(row.len());
        for term in row {
            if !term.coefficient.is_zero() {
                values.push(variables.map_coefficient(&term.coefficient)?);
                ids.push(column_id(columns, order, &term.integral) as u32);
            }
        }
        original.add_row(values, ids);
    }
    // Native sparse multiplication accumulates only one output row. No local
    // matrix/field elimination or coefficient-combination kernel is introduced.
    let product = &weights * &original;
    if product.col_idcs().first().copied() != Some(target_column as u32)
        || product.values().first().is_none_or(|value| !value.is_one())
    {
        return Err(invalid(
            "full source product has a forbidden term or nonunit target",
        ));
    }
    product
        .col_idcs()
        .iter()
        .zip(product.values())
        .map(|(&column, coefficient)| {
            Ok(Term {
                integral: columns[column as usize],
                coefficient: variables.restore_coefficient(coefficient)?,
            })
        })
        .collect()
}

fn column_id<const N: usize>(
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    integral: &Integral<N>,
) -> usize {
    columns
        .binary_search_by(|column| order.compare(column, integral))
        .expect("common exact-frame registry contains every source term")
}

#[cfg(test)]
mod tests;
