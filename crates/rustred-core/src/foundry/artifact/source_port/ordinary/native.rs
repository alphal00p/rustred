//! Native exact membership proposals and independent unprojected replay.

use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::{IntegerRing, Z};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::Coefficient;
use crate::solver::{ExactRow, IntegralOrder, Term};

use super::{SourcePortAuditError, error};

mod support;

#[cfg(test)]
mod tests;

/// Try a compact proposal only as an optimization of this SAME quotient.
/// Full unprojected replay and the caller's no-new-poles check are mandatory;
/// any inconclusive fast path falls back to the unchanged complete proposal.
pub(super) fn propose_verified<const N: usize>(
    rows: &[ExactRow<N>],
    desired: &ExactRow<N>,
    order: &IntegralOrder<N>,
    omit: impl FnMut(&Term<N, Coefficient>) -> Result<bool, SourcePortAuditError>,
    mut zero_product: impl FnMut(&Term<N, Coefficient>) -> Result<bool, SourcePortAuditError>,
    mut accept_compact: impl FnMut(&[Coefficient]) -> bool,
    enable_compact: bool,
) -> Result<Option<Vec<Coefficient>>, SourcePortAuditError> {
    let projected = project(rows, desired, order, omit)?;
    let template = &desired
        .first()
        .ok_or_else(|| error("ordinary replay has no desired target"))?
        .coefficient;
    if enable_compact {
        for support in support::candidates(&projected, order) {
            if support.len() >= rows.len() {
                continue;
            }
            let compact: Vec<_> = support
                .iter()
                .map(|&row| &projected[row])
                .chain(std::iter::once(projected.last().unwrap()))
                .collect();
            let Ok(Some(compact_weights)) = propose_projected(&compact, template, order) else {
                continue;
            };
            let mut weights = vec![template.numerator.zero().into(); rows.len()];
            for (&row, weight) in support.iter().zip(compact_weights) {
                weights[row] = weight;
            }
            if accept_compact(&weights)
                && verify(rows, desired, &weights, order, &mut zero_product).is_ok()
            {
                return Ok(Some(weights));
            }
        }
    }
    let borrowed: Vec<_> = projected.iter().collect();
    let weights = propose_projected(&borrowed, template, order)?;
    if let Some(weights) = &weights {
        verify(rows, desired, weights, order, zero_product)?;
    }
    Ok(weights)
}

#[cfg(test)]
pub(super) fn propose<const N: usize>(
    rows: &[ExactRow<N>],
    desired: &ExactRow<N>,
    order: &IntegralOrder<N>,
    omit: impl FnMut(&Term<N, Coefficient>) -> Result<bool, SourcePortAuditError>,
) -> Result<Option<Vec<Coefficient>>, SourcePortAuditError> {
    let projected = project(rows, desired, order, omit)?;
    let template = &desired
        .first()
        .ok_or_else(|| error("ordinary replay has no desired target"))?
        .coefficient;
    propose_projected(&projected.iter().collect::<Vec<_>>(), template, order)
}

fn project<const N: usize>(
    rows: &[ExactRow<N>],
    desired: &ExactRow<N>,
    order: &IntegralOrder<N>,
    mut omit: impl FnMut(&Term<N, Coefficient>) -> Result<bool, SourcePortAuditError>,
) -> Result<Vec<ExactRow<N>>, SourcePortAuditError> {
    rows.iter()
        .chain(std::iter::once(desired))
        .map(|row| {
            row.iter()
                .filter_map(|term| match omit(term) {
                    Ok(true) => None,
                    Ok(false) => Some(Ok(term.clone())),
                    Err(error) => Some(Err(error)),
                })
                .collect::<Result<ExactRow<N>, _>>()
                .map(|mut row| {
                    row.sort_unstable_by(|left, right| {
                        order.compare(&left.integral, &right.integral)
                    });
                    row
                })
        })
        .collect()
}

fn propose_projected<const N: usize>(
    projected: &[&ExactRow<N>],
    template: &Coefficient,
    order: &IntegralOrder<N>,
) -> Result<Option<Vec<Coefficient>>, SourcePortAuditError> {
    let source_count = projected.len() - 1;
    let one: Coefficient = template.numerator.one().into();
    let zero: Coefficient = template.numerator.zero().into();
    let mut columns: Vec<_> = projected
        .iter()
        .flat_map(|row| row.iter())
        .map(|term| term.integral)
        .collect();
    columns.sort_unstable_by(|left, right| order.compare(left, right));
    columns.dedup();
    let desired_column = columns
        .len()
        .checked_add(source_count)
        .ok_or_else(|| error("ordinary replay column count overflow"))?;
    let ncols = desired_column
        .checked_add(2)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| error("ordinary replay exceeds native column capacity"))?;
    let mut reducer = SparseRowReducer::new(
        ncols,
        RationalPolynomialField::<IntegerRing, u16>::new(Z),
        LuLMode::None,
    );
    for (ordinal, row) in projected.iter().enumerate() {
        let mut values: Vec<_> = row.iter().map(|term| term.coefficient.clone()).collect();
        let mut indices: Vec<_> = row
            .iter()
            .map(|term| {
                columns
                    .binary_search_by(|key| order.compare(key, &term.integral))
                    .expect("native physical column exists") as u32
            })
            .collect();
        values.push(one.clone());
        indices.push((columns.len() + ordinal) as u32);
        let pivot = reducer.add_row(&values, &indices);
        if ordinal < source_count {
            continue;
        }
        if pivot.is_none_or(|pivot| (pivot as usize) < columns.len()) {
            return Ok(None);
        }
        let u = reducer.u();
        let row_number = u.nrows() as usize - 1;
        let start = u.row_ptrs()[row_number];
        let end = u.row_ptrs()[row_number + 1];
        let desired_scale = u.col_idcs()[start..end]
            .iter()
            .zip(&u.values()[start..end])
            .find_map(|(&column, value)| (column as usize == desired_column).then_some(value))
            .ok_or_else(|| error("native ordinary identity lost its desired-row coefficient"))?;
        if desired_scale.is_zero() {
            return Err(error("native desired-row coefficient is zero"));
        }
        let mut weights = vec![zero.clone(); source_count];
        for (&column, value) in u.col_idcs()[start..end].iter().zip(&u.values()[start..end]) {
            if (column as usize) < columns.len() {
                return Err(error("ordinary proposal has a physical remainder"));
            }
            let source = column as usize - columns.len();
            if source < source_count {
                weights[source] = -(value / desired_scale);
            }
        }
        return Ok(Some(weights));
    }
    Err(error(
        "ordinary replay did not process the desired identity",
    ))
}

/// This is the authority check shared by strict and speculative proposals.
/// No source term or desired-identity term has been removed from these rows.
pub(super) fn verify<const N: usize>(
    rows: &[ExactRow<N>],
    desired: &ExactRow<N>,
    weights: &[Coefficient],
    order: &IntegralOrder<N>,
    mut zero_product: impl FnMut(&Term<N, Coefficient>) -> Result<bool, SourcePortAuditError>,
) -> Result<(), SourcePortAuditError> {
    if rows.len() != weights.len() {
        return Err(error("ordinary replay weight/source counts differ"));
    }
    let zero: Coefficient = desired
        .first()
        .ok_or_else(|| error("ordinary replay has no desired target"))?
        .coefficient
        .numerator
        .zero()
        .into();
    let mut columns: Vec<_> = rows
        .iter()
        .flatten()
        .chain(desired)
        .map(|term| term.integral)
        .collect();
    columns.sort_unstable_by(|left, right| order.compare(left, right));
    columns.dedup();
    let mut replay = vec![zero; columns.len()];
    for (row, weight) in rows.iter().zip(weights) {
        if weight.is_zero() {
            continue;
        }
        for term in row {
            let column = columns
                .binary_search_by(|key| order.compare(key, &term.integral))
                .unwrap();
            replay[column] = &replay[column] + &(weight * &term.coefficient);
        }
    }
    for term in desired {
        let column = columns
            .binary_search_by(|key| order.compare(key, &term.integral))
            .unwrap();
        replay[column] = &replay[column] - &term.coefficient;
    }
    for (integral, coefficient) in columns.into_iter().zip(replay) {
        if coefficient.is_zero() {
            continue;
        }
        let term = Term {
            integral,
            coefficient,
        };
        if !zero_product(&term)? {
            return Err(error(format!(
                "full original-source replay has an unproved residual product ({}) {}",
                term.coefficient, term.integral,
            )));
        }
    }
    Ok(())
}
