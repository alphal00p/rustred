//! Arity-independent finite-field GPLU and native accepted-L weight solving.

use symbolica::domains::finite_field::{ToFiniteField, Zp64};
use symbolica::domains::{Field, Ring};
use symbolica::tensors::sparse::{LuLMode, SparseMatrix, SparseRowReducer};

use super::super::Fp;
use super::{MaterializationError, ProbeFrame, invalid};

#[derive(Debug)]
pub(super) struct WeightImage {
    pub(super) row: Vec<(u32, Fp)>,
    pub(super) pivots: Vec<Option<u32>>,
    pub(super) weights: Vec<Fp>,
    pub(super) accepted_sources: Vec<usize>,
    pub(super) accepted_l_rows: Vec<usize>,
    pub(super) harder_rank: usize,
    pub(super) lower_nonzeros: usize,
}

#[cfg(test)]
pub(super) fn probe_image(
    frame: &ProbeFrame,
    target_column: usize,
    max_weight_slots: usize,
    field: &Zp64,
    point: &[Fp],
) -> Result<Option<WeightImage>, MaterializationError> {
    probe_prefix(
        frame,
        target_column,
        max_weight_slots,
        frame.rows().len(),
        field,
        point,
    )
}

pub(super) fn probe_prefix(
    frame: &ProbeFrame,
    target_column: usize,
    max_weight_slots: usize,
    source_limit: usize,
    field: &Zp64,
    point: &[Fp],
) -> Result<Option<WeightImage>, MaterializationError> {
    let width = frame.columns_with_sentinel();
    let target = u32::try_from(target_column).map_err(|_| MaterializationError::TooManyColumns)?;
    if target >= width - 1 {
        return Err(MaterializationError::TargetAbsent);
    }
    let mut reducer = SparseRowReducer::new(width, field.clone(), LuLMode::Full);
    // Separate native rank service restricted to ALL harder columns. The
    // trailing zero column prevents native full-rank short circuits.
    let mut harder = SparseRowReducer::new(target + 1, field.clone(), LuLMode::None);
    let mut accepted_sources = Vec::new();
    let mut accepted_l_rows = Vec::new();
    let mut pivots = Vec::new();
    let mut values = Vec::new();
    let mut ids = Vec::new();
    for (source, row) in frame.rows().iter().take(source_limit).enumerate() {
        let prefix_len = source
            .checked_add(1)
            .ok_or_else(|| invalid("source prefix overflow"))?;
        if prefix_len > max_weight_slots {
            return Err(invalid(format!(
                "source-weight slot budget exceeded: {prefix_len} > {max_weight_slots}",
            )));
        }
        values.clear();
        ids.clear();
        for (column, coefficient) in row {
            if coefficient.numerator.nvars() != point.len()
                || coefficient.denominator.nvars() != point.len()
            {
                return Err(invalid(
                    "probe variable dimension does not match prepared frame",
                ));
            }
            let numerator = coefficient.numerator.evaluate_with_coeff_map(
                |value| value.to_finite_field(field),
                point,
                field,
            );
            let denominator = coefficient.denominator.evaluate_with_coeff_map(
                |value| value.to_finite_field(field),
                point,
                field,
            );
            if field.is_zero(&denominator) {
                return Ok(None);
            }
            let value = field.div(&numerator, &denominator);
            if !field.is_zero(&value) {
                ids.push(*column);
                values.push(value);
            }
        }
        let before_l = reducer.l().nrows();
        let before_u = reducer.u().nrows();
        let pivot = reducer.add_row(&values, &ids);
        let growth_l = reducer
            .l()
            .nrows()
            .checked_sub(before_l)
            .ok_or_else(|| invalid("native L row count decreased"))?;
        let growth_u = reducer
            .u()
            .nrows()
            .checked_sub(before_u)
            .ok_or_else(|| invalid("native U row count decreased"))?;
        if growth_l > 1 || growth_u != u32::from(pivot.is_some()) || growth_u > growth_l {
            return Err(invalid("unexpected native L/U source embedding"));
        }
        if pivot.is_some() {
            accepted_sources.push(source);
            accepted_l_rows.push(before_l as usize);
        }
        pivots.push(pivot);
        if pivot != Some(target) {
            let harder_len = ids.partition_point(|column| *column < target);
            harder.add_row(&values[..harder_len], &ids[..harder_len]);
            continue;
        }
        let count = accepted_sources.len();
        let mut lower = SparseMatrix::new(0, reducer.l().ncols(), field.clone());
        for &native_row in &accepted_l_rows {
            let range = reducer.l().row_ptrs()[native_row]..reducer.l().row_ptrs()[native_row + 1];
            let mut entries: Vec<_> = reducer.l().col_idcs()[range.clone()]
                .iter()
                .copied()
                .zip(reducer.l().values()[range].iter().copied())
                .collect();
            // L columns use accepted-U order, not integral column order.
            entries.sort_unstable_by_key(|(column, _)| *column);
            let (columns, values) = entries.into_iter().unzip();
            lower.add_row(values, columns);
        }
        if lower.nrows() as usize != count || lower.ncols() as usize != count {
            return Err(invalid("accepted native L is not square"));
        }
        let solved = super::super::super::target_only::solve_transposed_lower(
            &lower,
            count - 1,
            field.one(),
            false,
        )?;
        if solved.nrows() != 1 || solved.ncols() as usize != count {
            return Err(invalid("native solved weights have the wrong dimensions"));
        }
        let mut weights = vec![field.zero(); prefix_len];
        for (&accepted, value) in solved.col_idcs().iter().zip(solved.values()) {
            weights[accepted_sources[accepted as usize]] = *value;
        }
        let upper = reducer.u();
        let range = upper.row_ptrs()[count - 1]..upper.row_ptrs()[count];
        let row = upper.col_idcs()[range.clone()]
            .iter()
            .copied()
            .zip(upper.values()[range].iter().copied())
            .collect();
        return Ok(Some(WeightImage {
            row,
            pivots,
            weights,
            accepted_sources,
            accepted_l_rows,
            harder_rank: harder.u().nrows() as usize,
            lower_nonzeros: lower.nvalues(),
        }));
    }
    Ok(None)
}
