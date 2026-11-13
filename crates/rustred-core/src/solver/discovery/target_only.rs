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

use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::variables::FrameVariables;
use super::{
    ExactField, ExactRow, Integral, IntegralOrder, MaterializationError, MaterializationEvent,
    Term, WitnessPoles, factorized,
};

fn invalid(reason: &'static str) -> MaterializationError {
    MaterializationError::TargetOnlyInvalidDecomposition(reason)
}

#[cfg(test)]
pub(super) fn materialize<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    materialize_witnessed(rows, columns, order, target_column, variables, observe).map(|(r, _)| r)
}

/// [`materialize`] plus the denominators of its source weights ([`WitnessPoles`]).
pub(super) fn materialize_witnessed<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    observe: impl FnMut(MaterializationEvent<N>),
) -> Result<(ExactRow<N>, WitnessPoles), MaterializationError> {
    materialize_in_field_witnessed(
        rows,
        columns,
        order,
        target_column,
        ExactField::new(Z),
        &|value| variables.map_coefficient(value),
        &|value| variables.restore_coefficient(value),
        &|value: &Coefficient| Ok(vec![value.denominator.clone()]),
        &|poly| variables.restore_polynomial(poly),
        false,
        observe,
    )
}

/// Explicit native-field composition; no automatic backend selection.
#[cfg(test)]
pub(super) fn materialize_factorized<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    materialize_factorized_witnessed(rows, columns, order, target_column, variables, observe)
        .map(|(r, _)| r)
}

/// [`materialize_factorized`] plus its weight denominators: every irreducible
/// denominator factor of the native factorized weights, one entry each.
pub(super) fn materialize_factorized_witnessed<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    observe: impl FnMut(MaterializationEvent<N>),
) -> Result<(ExactRow<N>, WitnessPoles), MaterializationError> {
    use symbolica::domains::factorized_rational_polynomial::FactorizedRationalPolynomialField;
    let active = variables.active_variables();
    materialize_in_field_witnessed(
        rows,
        columns,
        order,
        target_column,
        FactorizedRationalPolynomialField::<_, u16>::new(Z, active.clone()),
        &|value| factorized::factor(variables.map_coefficient(value)?, &active),
        &|value| variables.restore_coefficient(&factorized::ordinary(value, &active)?),
        &|value| Ok(factorized::denominator_factors(value)),
        &|poly| variables.restore_polynomial(poly),
        true,
        observe,
    )
}

/// Shared schedule over a native field. The closures only transport coefficients;
/// every algebraic operation remains in Symbolica's reducer/matrix services.
#[cfg(test)]
fn materialize_in_field<const N: usize, F: Field>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    field: F,
    import: &impl Fn(&Coefficient) -> Result<F::Element, MaterializationError>,
    export: &impl Fn(&F::Element) -> Result<Coefficient, MaterializationError>,
    catch_native: bool,
    observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    materialize_in_field_witnessed(
        rows,
        columns,
        order,
        target_column,
        field,
        import,
        export,
        &|_| Ok(Vec::new()),
        &|poly| Ok(poly.clone()),
        catch_native,
        observe,
    )
    .map(|(r, _)| r)
}

/// The shared schedule; `poles` maps one native weight to its denominator
/// polynomials in the ORIGINAL coefficient map (see [`WitnessPoles`]).
#[allow(clippy::too_many_arguments, reason = "native-field transport closures")]
fn materialize_in_field_witnessed<const N: usize, F: Field>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    field: F,
    import: &impl Fn(&Coefficient) -> Result<F::Element, MaterializationError>,
    export: &impl Fn(&F::Element) -> Result<Coefficient, MaterializationError>,
    poles: &impl Fn(&F::Element) -> Result<Vec<CoefficientPolynomial>, MaterializationError>,
    restore: &impl Fn(&CoefficientPolynomial) -> Result<CoefficientPolynomial, MaterializationError>,
    catch_native: bool,
    mut observe: impl FnMut(MaterializationEvent<N>),
) -> Result<(ExactRow<N>, WitnessPoles), MaterializationError> {
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
        let witness = witness_of(&rows[..prefix_len], &weights, poles, restore)?;
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
        return Ok((result, witness));
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
pub(super) fn solve_transposed_lower<F: Field>(
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

/// The rows of a `LuLMode::Full` L that belong to independent inputs (square,
/// lower triangular in U-row order) and the input row of each.
pub(super) fn independent_lower<F: Field>(
    lower: &SparseMatrix<F>,
) -> Result<(SparseMatrix<F>, Vec<usize>), MaterializationError> {
    let mut square = SparseMatrix::new(0, lower.ncols(), lower.field().clone());
    let mut inputs = Vec::new();
    for input in 0..lower.nrows() as usize {
        let (start, end) = (lower.row_ptrs()[input], lower.row_ptrs()[input + 1]);
        // An independent input ends with its diagonal: column = its own U row.
        if end > start && lower.col_idcs()[end - 1] as usize == inputs.len() {
            square.add_row(
                lower.values()[start..end].to_vec(),
                lower.col_idcs()[start..end].to_vec(),
            );
            inputs.push(input);
        }
    }
    if inputs.len() != lower.ncols() as usize {
        return Err(invalid("full L does not have one diagonal per U row"));
    }
    Ok((square, inputs))
}

/// Witness of a full-row GPLU with `LuLMode::Full` that may have skipped
/// dependent inputs (sparse and factorized backends): keep the L rows of the
/// independent inputs (square, lower triangular in U-row order), solve the
/// target U row's weights natively and report their poles. `target_u_row` is
/// the U row of the target pivot (the last one when the lift stops there).
#[allow(clippy::too_many_arguments)]
pub(super) fn witness_of_full_lower<const N: usize, F: Field>(
    rows: &[ExactRow<N>],
    lower: &SparseMatrix<F>,
    target_u_row: usize,
    one: F::Element,
    poles: &impl Fn(&F::Element) -> Result<Vec<CoefficientPolynomial>, MaterializationError>,
    restore: &impl Fn(&CoefficientPolynomial) -> Result<CoefficientPolynomial, MaterializationError>,
    export: &impl Fn(&F::Element) -> Result<Coefficient, MaterializationError>,
    solution: &ExactRow<N>,
    catch_native: bool,
) -> Result<WitnessPoles, MaterializationError> {
    let field = lower.field().clone();
    let (square, inputs) = independent_lower(lower)?;
    let weights = solve_transposed_lower(&square, target_u_row, one, catch_native)?;
    // guard-fix N1 (helper Y3b): L rows are reducer rows. If the reducer kept no row for an EMPTY input row, the
    // L row index is the index among the NON-EMPTY input rows: map through that list, never through the raw input
    // index (an empty row would shift every later one onto the wrong input row and its denominators).
    let nonempty: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.iter().any(|term| !term.coefficient.is_zero()))
        .map(|(i, _)| i)
        .collect();
    let to_input: Vec<usize> = if lower.nrows() as usize == rows.len() {
        (0..rows.len()).collect()
    } else if lower.nrows() as usize == nonempty.len() {
        nonempty
    } else {
        return Err(invalid(
            "L rows match neither all input rows nor the non-empty ones",
        ));
    };
    // Re-index the weights by input row before collecting the poles.
    let mut by_input = SparseMatrix::new(0, rows.len() as u32, field);
    let input_of = |k: u32| -> Result<u32, MaterializationError> {
        let l_row = *inputs
            .get(k as usize)
            .ok_or_else(|| invalid("weight index outside the independent L rows"))?;
        to_input
            .get(l_row)
            .map(|&i| i as u32)
            .ok_or_else(|| invalid("L row without an input row"))
    };
    by_input.add_row(
        weights.values().to_vec(),
        weights
            .col_idcs()
            .iter()
            .map(|&k| input_of(k))
            .collect::<Result<Vec<_>, _>>()?,
    );
    // guard-fix N3 (helper Y3b): the witness must reproduce the returned (monic) U row: sum_j w_j A_j = U at one
    // random point mod p (fail closed: a witness that does not is never used for guards).
    let exported: Vec<(usize, Coefficient)> = by_input
        .col_idcs()
        .iter()
        .zip(by_input.values())
        .filter(|(_, v)| !by_input.field().is_zero(v))
        .map(|(&j, v)| Ok((j as usize, export(v)?)))
        .collect::<Result<_, MaterializationError>>()?;
    check_witness_at_random_point(rows, &exported, solution)?;
    witness_of(rows, &by_input, poles, restore)
}

/// guard-fix N3: `sum_j w_j rows[j] == solution` at a deterministic pseudo-random point mod 2^61 - 1, every column
/// (weights and rows in the ORIGINAL coefficient map). Up to 4 points are tried when a denominator vanishes; Err on a
/// mismatch or when no regular point is found.
pub(super) fn check_witness_at_random_point<const N: usize>(
    rows: &[ExactRow<N>],
    weights: &[(usize, Coefficient)],
    solution: &ExactRow<N>,
) -> Result<(), MaterializationError> {
    use std::collections::BTreeMap;
    use symbolica::domains::finite_field::{ToFiniteField, Zp64};
    use symbolica::domains::{Ring, RingOps};
    use symbolica::prelude::Integer;
    let Some(first) = solution.iter().chain(rows.iter().flatten()).next() else {
        return Ok(());
    };
    let nvars = first.coefficient.numerator.variables().len();
    let prime: u64 = 2_305_843_009_213_693_951;
    let field = Zp64::new(prime);
    let mut state: u64 =
        0x9e37_79b9_7f4a_7c15 ^ (rows.len() as u64) ^ ((weights.len() as u64) << 32);
    let mut next = move || {
        // splitmix64
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    };
    'attempt: for _ in 0..4 {
        let point: Vec<_> = (0..nvars)
            .map(|_| Integer::from(1 + next() % (prime - 1)).to_finite_field(&field))
            .collect();
        let eval = |c: &Coefficient| -> Option<<Zp64 as symbolica::domains::Set>::Element> {
            let num =
                c.numerator
                    .evaluate_with_coeff_map(|v| v.to_finite_field(&field), &point, &field);
            let den = c.denominator.evaluate_with_coeff_map(
                |v| v.to_finite_field(&field),
                &point,
                &field,
            );
            if field.is_zero(&den) {
                None
            } else {
                Some(field.div(&num, &den))
            }
        };
        let mut acc: BTreeMap<Integral<N>, <Zp64 as symbolica::domains::Set>::Element> =
            BTreeMap::new();
        for (j, w) in weights {
            let Some(wv) = eval(w) else { continue 'attempt };
            let row = rows
                .get(*j)
                .ok_or_else(|| invalid("witness weight on a missing input row"))?;
            for term in row {
                if term.coefficient.is_zero() {
                    continue;
                }
                let Some(v) = eval(&term.coefficient) else {
                    continue 'attempt;
                };
                let e = acc.entry(term.integral).or_insert_with(|| field.zero());
                *e = field.add(&*e, &field.mul(&wv, &v));
            }
        }
        for term in solution {
            let Some(v) = eval(&term.coefficient) else {
                continue 'attempt;
            };
            let e = acc.entry(term.integral).or_insert_with(|| field.zero());
            *e = field.sub(&*e, &v);
        }
        if acc.values().any(|v| !field.is_zero(v)) {
            return Err(invalid(
                "guard-fix N3: the source weights do not reproduce the lifted row at a random point",
            ));
        }
        return Ok(());
    }
    Err(invalid(
        "guard-fix N3: no regular random point for the witness check",
    ))
}

/// Witness poles of `sum_j weights[j] * rows[j]`: the denominators of every
/// nonzero weight plus the input denominators of the rows they multiply.
pub(super) fn witness_of<const N: usize, F: Field>(
    rows: &[ExactRow<N>],
    weights: &SparseMatrix<F>,
    poles: &impl Fn(&F::Element) -> Result<Vec<CoefficientPolynomial>, MaterializationError>,
    restore: &impl Fn(&CoefficientPolynomial) -> Result<CoefficientPolynomial, MaterializationError>,
) -> Result<WitnessPoles, MaterializationError> {
    use symbolica::domains::InternalOrdering;
    // native-map denominators, deduplicated BEFORE the map restoration (weights share most factors)
    let mut native = Vec::new();
    let mut denominators = Vec::new();
    for (&row, value) in weights.col_idcs().iter().zip(weights.values()) {
        if weights.field().is_zero(value) {
            continue;
        }
        native.extend(poles(value)?.into_iter().filter(|p| !p.is_constant()));
        if let Some(row) = rows.get(row as usize) {
            denominators.extend(
                row.iter()
                    .filter(|term| !term.coefficient.denominator.is_constant())
                    .map(|term| term.coefficient.denominator.clone()),
            );
        }
    }
    native.sort_unstable_by(InternalOrdering::internal_cmp);
    native.dedup();
    for poly in &native {
        denominators.push(restore(poly)?);
    }
    Ok(WitnessPoles::recorded(denominators))
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
