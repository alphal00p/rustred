//! Native target-block GPLU and a one-shot full identity reconstruction.
//!
//! Write the unchanged source prefix as A=[F|R], with F ending at the target.
//! If every F row is independent, its GPLU has exactly the same pivots and
//! scales as full-A GPLU. With F=L U, native L^T w=e_j therefore recovers the
//! same monic target U row as the default backend, not another span solution.
//! The final native product checks every forbidden column and the unit target.
//! These temporary frame weights are not original-generator certificates.

use symbolica::domains::Field;
#[cfg(test)]
use symbolica::domains::SelfRing;
use symbolica::prelude::Z;
use symbolica::tensors::sparse::{LuLMode, SparseMatrix, SparseRowReducer};

use crate::algebra::Coefficient;

use super::variables::FrameVariables;
use super::{
    ExactField, ExactRow, Integral, IntegralOrder, MaterializationError, MaterializationEvent,
    Term, factorized,
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
    observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    materialize_in_field(
        rows,
        columns,
        order,
        target_column,
        ExactField::new(Z),
        &|value| variables.map_coefficient(value),
        &|value| variables.restore_coefficient(value),
        false,
        observe,
    )
}

/// Explicit native-field composition; no automatic backend selection.
pub(super) fn materialize_factorized<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    use symbolica::domains::factorized_rational_polynomial::FactorizedRationalPolynomialField;
    let active = variables.active_variables();
    materialize_in_field(
        rows,
        columns,
        order,
        target_column,
        FactorizedRationalPolynomialField::<_, u16>::new(Z, active.clone()),
        &|value| factorized::factor(variables.map_coefficient(value)?, &active),
        &|value| variables.restore_coefficient(&factorized::ordinary(value, &active)?),
        true,
        observe,
    )
}

/// Shared schedule over a native field. The closures only transport coefficients;
/// every algebraic operation remains in Symbolica's reducer/matrix services.
fn materialize_in_field<const N: usize, F: Field>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    field: F,
    import: &impl Fn(&Coefficient) -> Result<F::Element, MaterializationError>,
    export: &impl Fn(&F::Element) -> Result<Coefficient, MaterializationError>,
    catch_native: bool,
    mut observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    // Admit the full reconstruction width, not only the smaller target block.
    // The sentinel convention is shared with the full sparse backends.
    u32::try_from(columns.len())
        .ok()
        .and_then(|width| width.checked_add(1))
        .ok_or(MaterializationError::TooManyColumns)?;
    // Authenticate the entire input, including explicit zeros, tail-only terms
    // and rows after the eventual hit, before a shortcut can omit them.
    let one = import(&input_one(rows)?)?;
    // Common preflight checked distinct sorted source terms and every
    // coefficient map. The trailing zero sentinel is retained.
    let block_columns =
        u32::try_from(target_column + 2).map_err(|_| MaterializationError::TooManyColumns)?;
    let mut reducer = native(
        catch_native,
        "constructing the target-block reducer",
        || SparseRowReducer::new(block_columns, field, LuLMode::Full),
    )?;
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
            values.push(import(&term.coefficient)?);
            ids.push(column as u32);
        }
        observe(MaterializationEvent::RowStarted {
            row: ordinal + 1,
            input_nonzeros: values.len(),
            reducer_rows: reducer.u().nrows() as usize,
            reducer_nonzeros: reducer.u().nvalues(),
        });
        let pivot = native(catch_native, "reducing a target-block source row", || {
            reducer.add_row(&values, &ids)
        })?;
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
        let weights = solve_transposed_lower(reducer.l(), ordinal, one, catch_native)?;
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
            import,
            export,
            weights,
            catch_native,
        )?;
        observe(MaterializationEvent::TargetReconstructionFinished {
            output_terms: result.len(),
        });
        return Ok(result);
    }
    Err(MaterializationError::TargetNotPivot)
}

fn input_one<const N: usize>(rows: &[ExactRow<N>]) -> Result<Coefficient, MaterializationError> {
    let first = rows
        .iter()
        .flatten()
        .next()
        .ok_or(MaterializationError::TargetNotPivot)?;
    let variables = first.coefficient.numerator.variables();
    for term in rows.iter().flatten() {
        if term.coefficient.numerator.variables() != variables
            || term.coefficient.denominator.variables() != variables
        {
            return Err(MaterializationError::CoefficientVariableMapMismatch);
        }
        if term.coefficient.denominator.is_zero() {
            return Err(MaterializationError::InvalidFactorizedCoefficient(
                "zero input denominator",
            ));
        }
    }
    // RationalPolynomialField has no registered map of its own. Its generic
    // `one()` is not a substitute for a unit in this exact frame's context.
    Ok(first.coefficient.numerator.one().into())
}

fn native<T>(
    catch_native: bool,
    operation: &'static str,
    function: impl FnOnce() -> T,
) -> Result<T, MaterializationError> {
    if catch_native {
        factorized::native(operation, function)
    } else {
        // Ordinary target-only preserves its original native panic behavior.
        Ok(function())
    }
}

/// Transpose sparse structure only; all normalization/back substitution is
/// native. L columns need not be sorted in pivot-acceptance order. Iterating
/// its source rows in order nevertheless builds sorted rows of L^T.
fn solve_transposed_lower<F: Field>(
    lower: &SparseMatrix<F>,
    target_row: usize,
    one: F::Element,
    catch_native: bool,
) -> Result<SparseMatrix<F>, MaterializationError> {
    let count = lower.nrows() as usize;
    if count == 0 || lower.ncols() as usize != count || target_row >= count {
        return Err(invalid(
            "L must be nonempty and square with a valid target row",
        ));
    }
    if lower.values().is_empty() {
        return Err(invalid("empty L"));
    }
    let field = lower.field();
    let mut transposed: Vec<Vec<(u32, &F::Element)>> = vec![Vec::new(); count];
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
    let mut solver = native(catch_native, "constructing the triangular reducer", || {
        SparseRowReducer::new(augmented_columns, field.clone(), LuLMode::None)
    })?;
    for (row, terms) in transposed.into_iter().enumerate() {
        if terms
            .first()
            .is_none_or(|(column, value)| *column as usize != row || field.is_zero(value))
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
        if native(catch_native, "normalizing a triangular row", || {
            solver.add_row(&values, &ids)
        })? != Some(row as u32)
        {
            return Err(invalid(
                "native triangular normalization changed the expected pivot",
            ));
        }
    }
    native(catch_native, "solving target weights", || {
        solver.back_substitute()
    })?;
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
                if !field.is_zero(value) {
                    values.push(value.clone());
                    ids.push(variable as u32);
                }
            } else if column != variable || !field.is_one(value) {
                return Err(invalid(
                    "native triangular solution retains an unsolved coefficient",
                ));
            }
        }
    }
    let mut result = SparseMatrix::new(0, lower.ncols(), field.clone());
    result.add_row(values, ids);
    Ok(result)
}

fn reconstruct<const N: usize, F: Field>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    import: &impl Fn(&Coefficient) -> Result<F::Element, MaterializationError>,
    export: &impl Fn(&F::Element) -> Result<Coefficient, MaterializationError>,
    weights: SparseMatrix<F>,
    catch_native: bool,
) -> Result<ExactRow<N>, MaterializationError> {
    if weights.nrows() != 1 || weights.ncols() as usize != rows.len() {
        return Err(invalid(
            "source weights do not match the unchanged source prefix",
        ));
    }
    let mut original = SparseMatrix::new(0, columns.len() as u32, weights.field().clone());
    for row in rows {
        let mut values = Vec::with_capacity(row.len());
        let mut ids = Vec::with_capacity(row.len());
        for term in row {
            if !term.coefficient.is_zero() {
                values.push(import(&term.coefficient)?);
                ids.push(column_id(columns, order, &term.integral) as u32);
            }
        }
        original.add_row(values, ids);
    }
    // Native sparse multiplication accumulates only one output row. No local
    // matrix/field elimination or coefficient-combination kernel is introduced.
    let product = native(
        catch_native,
        "multiplying target weights by full sources",
        || &weights * &original,
    )?;
    if product.col_idcs().first().copied() != Some(target_column as u32)
        || product
            .values()
            .first()
            .is_none_or(|value| !product.field().is_one(value))
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
                coefficient: export(coefficient)?,
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

#[cfg(test)]
mod benchmark;
