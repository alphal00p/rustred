//! Bounded proposal-only physical-row dependency selection with native GPLU.

use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::finite_field::{FiniteFieldCore, PrimeIteratorU64, ToFiniteField, Zp64};
use symbolica::prelude::{Field, Ring};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::solver::{ExactRow, IntegralOrder};

const MAX_ROWS: usize = 16_384;
const MAX_INPUT_TERMS: usize = 1_048_576;
const MAX_FILL: usize = 4_194_304;
const MAX_PATTERN: usize = 8_388_608;
const PROBES: usize = 2;

pub(super) fn candidates<const N: usize>(
    rows: &[ExactRow<N>],
    order: &IntegralOrder<N>,
) -> Vec<Vec<usize>> {
    if rows.len() < 3
        || rows.len() > MAX_ROWS
        || rows.iter().map(Vec::len).sum::<usize>() > MAX_INPUT_TERMS
    {
        return Vec::new();
    }
    let Some(template) = rows.iter().flatten().next() else {
        return Vec::new();
    };
    let variables = template.coefficient.get_variables();
    if rows.iter().any(|row| {
        row.windows(2)
            .any(|pair| !order.compare(&pair[0].integral, &pair[1].integral).is_lt())
            || row.iter().any(|term| {
                term.coefficient.numerator.variables() != variables
                    || term.coefficient.denominator.variables() != variables
            })
    }) {
        return Vec::new();
    }
    let mut columns: Vec<_> = rows.iter().flatten().map(|term| term.integral).collect();
    columns.sort_unstable_by(|left, right| order.compare(left, right));
    columns.dedup();
    let Some(ncols) = columns
        .len()
        .checked_add(1)
        .and_then(|n| u32::try_from(n).ok())
    else {
        return Vec::new();
    };
    let mut output = Vec::new();
    for (probe, prime) in PrimeIteratorU64::new(1 << 61).take(PROBES).enumerate() {
        let field = Zp64::new(prime);
        let mut rng = StdRng::seed_from_u64(0x736f_7572_6365_7300 + probe as u64);
        let point: Vec<_> = (0..variables.len())
            .map(|_| field.to_element(rng.random_range(1..prime)))
            .collect();
        if let Some(support) = select(rows, order, &columns, ncols, &field, &point)
            && !output.contains(&support)
        {
            output.push(support);
        }
    }
    output
}

fn select<const N: usize>(
    rows: &[ExactRow<N>],
    order: &IntegralOrder<N>,
    columns: &[crate::solver::Integral<N>],
    ncols: u32,
    field: &Zp64,
    point: &[symbolica::domains::finite_field::FiniteFieldElement<u64>],
) -> Option<Vec<usize>> {
    // The extra structural zero column prevents native full-rank early return.
    let mut reducer = SparseRowReducer::new(ncols, field.clone(), LuLMode::Pattern);
    let mut accepted_sources = Vec::new();
    let mut accepted_lower_rows = Vec::new();
    for (ordinal, row) in rows.iter().enumerate() {
        let mut values = Vec::with_capacity(row.len());
        let mut ids = Vec::with_capacity(row.len());
        for term in row {
            let numerator = term.coefficient.numerator.evaluate_with_coeff_map(
                |value| value.to_finite_field(field),
                point,
                field,
            );
            let denominator = term.coefficient.denominator.evaluate_with_coeff_map(
                |value| value.to_finite_field(field),
                point,
                field,
            );
            if field.is_zero(&denominator) {
                return None;
            }
            let value = field.div(&numerator, &denominator);
            if !field.is_zero(&value) {
                values.push(value);
                ids.push(
                    columns
                        .binary_search_by(|key| order.compare(key, &term.integral))
                        .ok()? as u32,
                );
            }
        }
        let before_lower = reducer.l().nrows() as usize;
        let pivot = reducer.add_row(&values, &ids);
        if reducer.u().nvalues() > MAX_FILL || reducer.l().col_idcs().len() > MAX_PATTERN {
            return None;
        }
        if ordinal + 1 < rows.len() {
            if pivot.is_some() {
                if reducer.l().nrows() as usize != before_lower + 1 {
                    return None;
                }
                accepted_sources.push(ordinal);
                accepted_lower_rows.push(before_lower);
            }
            continue;
        }
        // An empty desired image appends no L row and is not membership evidence.
        if pivot.is_some() || values.is_empty() || reducer.l().nrows() as usize != before_lower + 1
        {
            return None;
        }
        let lower = reducer.l();
        let mut pending: Vec<_> = lower.col_idcs()
            [lower.row_ptrs()[before_lower]..lower.row_ptrs()[before_lower + 1]]
            .iter()
            .map(|&row| row as usize)
            .collect();
        let mut included = vec![false; accepted_sources.len()];
        while let Some(row) = pending.pop() {
            if row >= included.len() {
                return None;
            }
            if std::mem::replace(&mut included[row], true) {
                continue;
            }
            let native_row = accepted_lower_rows[row];
            for &dependency in
                &lower.col_idcs()[lower.row_ptrs()[native_row]..lower.row_ptrs()[native_row + 1]]
            {
                let dependency = dependency as usize;
                if dependency > row {
                    return None;
                }
                if dependency != row {
                    pending.push(dependency);
                }
            }
        }
        // Accepted source order is original source order, independent of pivot IDs.
        return Some(
            accepted_sources
                .into_iter()
                .zip(included)
                .filter_map(|(source, keep)| keep.then_some(source))
                .collect(),
        );
    }
    None
}

#[cfg(test)]
mod tests;
