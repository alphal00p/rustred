//! Closure selection over U, restricted modular back-substitution, row
//! selection from L dependencies, and the second-sample replay.
//!
//! Let `C` be the closure of the demanded columns over the U rows (a pivot
//! column brings in every column of its U row). Master candidates are the
//! non-pivot columns of `C`. Back substitution restricted to `C` gives the
//! reduced rows of the demanded integrals; the masters are the candidates
//! with a nonzero coefficient. The selected rows are the L-dependency
//! closure of the pivot rows of `C`: their span contains the U row of every
//! pivot of `C`, so the reduced rows computed from the selected rows alone
//! are those of the full system (pivots of a subspace are pivots of the
//! space, and a reduced row is unique).

use symbolica::domains::RingOps;
use symbolica::domains::finite_field::Zp64;
use symbolica::prelude::Ring;
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use super::super::discovery::{ColumnComparator, Discovery};
use super::super::{Integral, SolverError};
use super::BoxPhase;
use super::rows::Zp;

/// Read access to an echelon (U) form with live column indices.
pub(crate) trait Echelon<const N: usize> {
    fn ncols(&self) -> usize;
    fn column(&self, column: usize) -> Integral<N>;
    fn column_index(&self, integral: &Integral<N>) -> Option<usize>;
    fn pivot_row(&self, column: usize) -> Option<usize>;
    fn row_columns(&self, row: usize) -> &[u32];
    fn row_values(&self, row: usize) -> &[Zp];
    fn field(&self) -> &Zp64;
}

impl<const N: usize, O: ColumnComparator<N>> Echelon<N> for Discovery<N, O> {
    fn ncols(&self) -> usize {
        self.columns().len()
    }
    fn column(&self, column: usize) -> Integral<N> {
        self.columns()[column]
    }
    fn column_index(&self, integral: &Integral<N>) -> Option<usize> {
        Discovery::column_index(self, integral)
    }
    fn pivot_row(&self, column: usize) -> Option<usize> {
        Discovery::pivot_row(self, column)
    }
    fn row_columns(&self, row: usize) -> &[u32] {
        self.u_row_columns(row)
    }
    fn row_values(&self, row: usize) -> &[Zp] {
        self.u_row_values(row)
    }
    fn field(&self) -> &Zp64 {
        Discovery::field(self)
    }
}

/// The closure `C` of a demand over U.
#[derive(Clone, Debug, Default)]
pub(crate) struct Closure {
    /// Columns of `C`, ascending (hardest first).
    pub columns: Vec<usize>,
    /// `(column, row)` of the pivots of `C`, ascending by column.
    pub pivots: Vec<(usize, usize)>,
    /// Non-pivot columns of `C`, ascending.
    pub candidates: Vec<usize>,
}

pub(crate) fn closure<const N: usize, E: Echelon<N>>(echelon: &E, demand: &[usize]) -> Closure {
    let mut seen = vec![false; echelon.ncols()];
    let mut stack = Vec::new();
    for &column in demand {
        if !seen[column] {
            seen[column] = true;
            stack.push(column);
        }
    }
    while let Some(column) = stack.pop() {
        if let Some(row) = echelon.pivot_row(column) {
            for &next in echelon.row_columns(row) {
                let next = next as usize;
                if !seen[next] {
                    seen[next] = true;
                    stack.push(next);
                }
            }
        }
    }
    let mut result = Closure::default();
    for (column, &in_closure) in seen.iter().enumerate() {
        if !in_closure {
            continue;
        }
        result.columns.push(column);
        match echelon.pivot_row(column) {
            Some(row) => result.pivots.push((column, row)),
            None => result.candidates.push(column),
        }
    }
    result
}

/// Columns (candidates) carrying a nonzero reduced-row coefficient for some
/// demanded column, ascending. A demanded non-pivot column is its own master.
pub(crate) fn nonzero_masters<const N: usize, E: Echelon<N>>(
    echelon: &E,
    closure: &Closure,
    demand: &[usize],
) -> Vec<usize> {
    let reduced = reduced_rows(echelon, closure, demand);
    let mut master = vec![false; echelon.ncols()];
    for row in &reduced {
        for &(column, _) in row {
            master[column] = true;
        }
    }
    closure
        .candidates
        .iter()
        .copied()
        .filter(|&column| master[column])
        .collect()
}

/// Reduced rows of the `targets` (each must lie in `closure`): for each, the
/// `(candidate column, coefficient)` with `target = sum coefficient *
/// candidate`, ascending by column. A non-pivot target is `[(target, 1)]`.
pub(crate) fn reduced_rows<const N: usize, E: Echelon<N>>(
    echelon: &E,
    closure: &Closure,
    targets: &[usize],
) -> Vec<Vec<(usize, Zp)>> {
    let field = echelon.field().clone();
    let ncols = echelon.ncols();
    const NONE: u32 = u32::MAX;
    let mut slot = vec![NONE; ncols];
    for (index, &column) in closure.candidates.iter().enumerate() {
        slot[column] = index as u32;
    }
    let mut pivot_index = vec![NONE; ncols];
    for (index, &(column, _)) in closure.pivots.iter().enumerate() {
        pivot_index[column] = index as u32;
    }
    let mut reduced: Vec<Vec<(u32, Zp)>> = vec![Vec::new(); closure.pivots.len()];
    let mut accumulator = vec![field.zero(); closure.candidates.len()];
    let mut touched_flag = vec![false; closure.candidates.len()];
    let mut touched = Vec::new();
    // Easiest pivot first: every tail pivot is already reduced.
    for (index, &(column, row)) in closure.pivots.iter().enumerate().rev() {
        let columns = echelon.row_columns(row);
        let values = echelon.row_values(row);
        debug_assert_eq!(columns[0] as usize, column);
        debug_assert!(field.is_one(&values[0]));
        for (&next, value) in columns[1..].iter().zip(&values[1..]) {
            let next = next as usize;
            let candidate = slot[next];
            if candidate != NONE {
                let candidate = candidate as usize;
                field.sub_assign(&mut accumulator[candidate], value);
                if !touched_flag[candidate] {
                    touched_flag[candidate] = true;
                    touched.push(candidate as u32);
                }
            } else {
                let tail = pivot_index[next];
                debug_assert!(tail != NONE, "closure contains every U-row column");
                for &(candidate, weight) in &reduced[tail as usize] {
                    let position = candidate as usize;
                    field.sub_mul_assign(&mut accumulator[position], value, &weight);
                    if !touched_flag[position] {
                        touched_flag[position] = true;
                        touched.push(candidate);
                    }
                }
            }
        }
        touched.sort_unstable();
        let mut row_result = Vec::with_capacity(touched.len());
        for &candidate in &touched {
            let position = candidate as usize;
            let value = std::mem::replace(&mut accumulator[position], field.zero());
            touched_flag[position] = false;
            if !field.is_zero(&value) {
                row_result.push((candidate, value));
            }
        }
        touched.clear();
        reduced[index] = row_result;
    }
    targets
        .iter()
        .map(|&column| {
            if slot[column] != NONE {
                vec![(column, field.one())]
            } else {
                let index = pivot_index[column];
                assert!(
                    index != NONE,
                    "reduced rows need targets inside the closure"
                );
                // `reduced` holds x_c = sum w * candidate.
                reduced[index as usize]
                    .iter()
                    .map(|&(candidate, weight)| (closure.candidates[candidate as usize], weight))
                    .collect()
            }
        })
        .collect()
}

/// Replay of selected rows at an independent sample, in pivot-column order.
pub(crate) struct Replay<const N: usize, O> {
    order: O,
    columns: Vec<Integral<N>>,
    reducer: SparseRowReducer<Zp64>,
}

impl<const N: usize, O: ColumnComparator<N>> Replay<N, O> {
    /// Insert `rows` in order; row `i` must reproduce pivot `expected[i]`.
    /// `rows` and `expected` must have the same length (`InvalidInput`
    /// otherwise); a row that does not reproduce its pivot (also an empty or
    /// dependent row) is `UnluckyBoxSample { Replay }`.
    pub(crate) fn new(
        order: O,
        field: Zp64,
        rows: &[Vec<super::super::Term<N, Zp>>],
        expected: &[Integral<N>],
    ) -> Result<Self, SolverError> {
        // One expected pivot per row: a missing or extra row is a broken
        // selection, never silently accepted.
        if rows.len() != expected.len() {
            return Err(SolverError::InvalidInput(format!(
                "replay of {} rows with {} expected pivots",
                rows.len(),
                expected.len()
            )));
        }
        let mut columns: Vec<Integral<N>> =
            rows.iter().flatten().map(|term| term.integral).collect();
        columns.sort_unstable_by(|left, right| order.compare(left, right));
        columns.dedup();
        let mut reducer = SparseRowReducer::new(columns.len() as u32 + 1, field, LuLMode::None);
        let mut values = Vec::new();
        let mut ids = Vec::new();
        for (row, expected) in rows.iter().zip(expected) {
            values.clear();
            ids.clear();
            for term in row {
                let column = columns
                    .binary_search_by(|column| order.compare(column, &term.integral))
                    .expect("replay columns contain every row integral");
                values.push(term.coefficient);
                ids.push(column as u32);
            }
            let pivot = reducer.add_row(&values, &ids);
            if pivot.map(|pivot| columns[pivot as usize]) != Some(*expected) {
                return Err(SolverError::UnluckyBoxSample {
                    phase: BoxPhase::Replay,
                });
            }
        }
        Ok(Self {
            order,
            columns,
            reducer,
        })
    }
}

impl<const N: usize, O: ColumnComparator<N>> Echelon<N> for Replay<N, O> {
    fn ncols(&self) -> usize {
        self.columns.len()
    }
    fn column(&self, column: usize) -> Integral<N> {
        self.columns[column]
    }
    fn column_index(&self, integral: &Integral<N>) -> Option<usize> {
        self.columns
            .binary_search_by(|column| self.order.compare(column, integral))
            .ok()
    }
    fn pivot_row(&self, column: usize) -> Option<usize> {
        self.reducer.pivots()[column].map(|row| row as usize)
    }
    fn row_columns(&self, row: usize) -> &[u32] {
        let u = self.reducer.u();
        &u.col_idcs()[u.row_ptrs()[row]..u.row_ptrs()[row + 1]]
    }
    fn row_values(&self, row: usize) -> &[Zp] {
        let u = self.reducer.u();
        &u.values()[u.row_ptrs()[row]..u.row_ptrs()[row + 1]]
    }
    fn field(&self) -> &Zp64 {
        self.reducer.u().field()
    }
}
