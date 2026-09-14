//! One ordered integral-column space for numerical GPLU and exact replay.

use std::cmp::Ordering;
use std::fmt;

use symbolica::domains::finite_field::Zp64;
use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::domains::{Ring, Set};
use symbolica::prelude::{IntegerRing, Z};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use super::{ExactRow, Integral, IntegralOrder, Term};

type NumericalCoefficient = <Zp64 as Set>::Element;
type ExactField = RationalPolynomialField<IntegerRing, u16>;

/// Sizes of the numerical system, excluding its structural zero sentinel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DiscoveryStats {
    pub rows_seen: usize,
    pub independent_rows: usize,
    pub columns: usize,
    pub reducer_nonzeros: usize,
    /// Direct dependencies of accepted rows, excluding their diagonals.
    pub dependency_edges: usize,
    /// Native L rows retained for both independent and dependent inputs.
    /// Empty numerical inputs do not append a native L row.
    pub retained_l_rows: usize,
    /// Native L column indices, including accepted diagonals and the patterns
    /// of dependent inputs. Pattern mode stores no coefficient values.
    pub retained_l_entries: usize,
}

/// Incremental SpIRed GPLU over all integral columns in harder-first order.
///
/// The caller retains an original exact row only when [`Self::add_row`]
/// returns a pivot. Its ordinal among those retained rows is the basis ordinal
/// used by [`Self::trace`]. Whether a pivot solves a case belongs to the caller:
/// discovery does not collapse case matches into a distinguished target column.
///
/// Unlike C++ SpIRed, the current native reducer retains the L patterns of
/// dependent inputs. The accepted-row mapping preserves trace correctness,
/// but cannot reclaim those patterns through the available public API. For
/// `m` inputs and rank `r`, retained L can therefore grow as `O(m * r)` even
/// when the basis stops growing. [`Self::stats`] reports this storage separately
/// from accepted dependency edges; its entry counts are not allocation bytes.
#[derive(Debug)]
pub struct Discovery<const N: usize> {
    order: IntegralOrder<N>,
    columns: Vec<Integral<N>>,
    reducer: SparseRowReducer<Zp64>,
    /// Native L also records dependent input rows, whereas its column indices
    /// refer to accepted U rows. Keep that distinction without copying L.
    accepted_l_rows: Vec<usize>,
    rows_seen: usize,
    dependency_edges: usize,
}

impl<const N: usize> Discovery<N> {
    pub fn new(order: IntegralOrder<N>, field: Zp64) -> Self {
        Self {
            order,
            columns: Vec::new(),
            // A permanent zero column follows every physical integral. It
            // prevents the native full-rank shortcut from suppressing L rows.
            reducer: SparseRowReducer::new(1, field, LuLMode::Pattern),
            accepted_l_rows: Vec::new(),
            rows_seen: 0,
            dependency_edges: 0,
        }
    }

    /// Add terms with distinct integral keys in this discovery's order.
    /// Numerically zero coefficients are omitted from the native row, but
    /// their integral columns are still registered.
    pub fn add_row(&mut self, terms: &[Term<N, NumericalCoefficient>]) -> Option<Integral<N>> {
        debug_assert!(terms.windows(2).all(|pair| {
            self.order.compare(&pair[0].integral, &pair[1].integral) == Ordering::Less
        }));
        self.rows_seen += 1;
        self.register_columns(terms);

        let mut values = Vec::with_capacity(terms.len());
        let mut column_ids = Vec::with_capacity(terms.len());
        for term in terms {
            if !self.reducer.u().field().is_zero(&term.coefficient) {
                let column = self
                    .columns
                    .binary_search_by(|column| self.order.compare(column, &term.integral))
                    .expect("every row integral was registered");
                values.push(term.coefficient);
                column_ids.push(column as u32);
            }
        }

        let pivot = self.reducer.add_row(&values, &column_ids)?;
        let basis_row = self.accepted_l_rows.len();
        let l = self.reducer.l();
        let l_row = l.row_ptrs().len() - 2;
        let dependencies = &l.col_idcs()[l.row_ptrs()[l_row]..l.row_ptrs()[l_row + 1]];
        debug_assert!(dependencies.contains(&(basis_row as u32)));
        debug_assert!(dependencies.iter().all(|&row| row as usize <= basis_row));
        self.dependency_edges += dependencies
            .iter()
            .filter(|&&row| row as usize != basis_row)
            .count();
        self.accepted_l_rows.push(l_row);
        Some(self.columns[pivot as usize])
    }

    pub fn basis_len(&self) -> usize {
        self.accepted_l_rows.len()
    }

    pub fn stats(&self) -> DiscoveryStats {
        DiscoveryStats {
            rows_seen: self.rows_seen,
            independent_rows: self.basis_len(),
            columns: self.columns.len(),
            reducer_nonzeros: self.reducer.u().nvalues(),
            dependency_edges: self.dependency_edges,
            retained_l_rows: self.reducer.l().nrows() as usize,
            retained_l_entries: self.reducer.l().col_idcs().len(),
        }
    }

    /// Original accepted-row ordinals needed for `root`, in pivot-column order.
    ///
    /// This is SpIRed's `optimizeSystem`: close the L dependencies, retain
    /// pivots at or to the left of the root pivot, then sort by live column.
    /// Column positions are read after insertion, so late harder integrals
    /// cannot leave a stale pivot index in the trace.
    pub fn trace(&self, root: usize) -> Vec<usize> {
        self.trace_many(&[root])
    }

    /// Union of winning traces in live pivot-column order. Each root retains
    /// only ancestors at or left of its own pivot, as in C++ optimizeSystem.
    /// This operation is performed after discovery, never per streamed row.
    pub fn trace_many(&self, roots: &[usize]) -> Vec<usize> {
        assert!(roots.iter().all(|&root| root < self.basis_len()));
        let u = self.reducer.u();
        let l = self.reducer.l();
        let mut needed = vec![false; self.basis_len()];
        let mut visited = vec![usize::MAX; self.basis_len()];
        let mut pending = Vec::new();
        for (visit, &root) in roots.iter().enumerate() {
            let root_column = u.col_idcs()[u.row_ptrs()[root]];
            pending.push(root);
            while let Some(row) = pending.pop() {
                if visited[row] == visit {
                    continue;
                }
                visited[row] = visit;
                if u.col_idcs()[u.row_ptrs()[row]] <= root_column {
                    needed[row] = true;
                }
                let l_row = self.accepted_l_rows[row];
                for &dependency in &l.col_idcs()[l.row_ptrs()[l_row]..l.row_ptrs()[l_row + 1]] {
                    let dependency = dependency as usize;
                    if dependency != row && visited[dependency] != visit {
                        pending.push(dependency);
                    }
                }
            }
        }
        self.reducer
            .pivots()
            .iter()
            .filter_map(|&row| row.map(|row| row as usize))
            .filter(|&row| needed[row])
            .collect()
    }

    fn register_columns(&mut self, terms: &[Term<N, NumericalCoefficient>]) {
        // Positions are relative to the old registry, as required by native
        // add_cols. Several new columns may have the same insertion position.
        let mut positions = Vec::new();
        let mut additions = Vec::new();
        for term in terms {
            if let Err(position) = self
                .columns
                .binary_search_by(|column| self.order.compare(column, &term.integral))
            {
                positions.push(position as u32);
                additions.push(term.integral);
            }
        }
        if additions.is_empty() {
            return;
        }
        let next_len = self
            .columns
            .len()
            .checked_add(additions.len())
            .expect("integral-column count overflow");
        assert!(
            next_len < u32::MAX as usize,
            "native sparse integral columns and sentinel must fit in u32"
        );
        self.reducer.add_cols(&positions);

        let mut merged = Vec::with_capacity(next_len);
        let mut old_position = 0;
        for (integral, &position) in additions.into_iter().zip(&positions) {
            let position = position as usize;
            merged.extend_from_slice(&self.columns[old_position..position]);
            merged.push(integral);
            old_position = position;
        }
        merged.extend_from_slice(&self.columns[old_position..]);
        self.columns = merged;
    }
}

/// Exact replay failed to recover the numerical target pivot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterializationError {
    TooManyColumns,
    NonCanonicalRow { row: usize },
    TargetAbsent,
    TargetNotPivot,
}

impl fmt::Display for MaterializationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyColumns => write!(f, "exact integral-column count does not fit in u32"),
            Self::NonCanonicalRow { row } => {
                write!(
                    f,
                    "exact row {row} is not in distinct harder-first integral order"
                )
            }
            Self::TargetAbsent => {
                write!(f, "target integral is absent from the selected exact rows")
            }
            Self::TargetNotPivot => {
                write!(f, "selected exact rows do not produce the target pivot")
            }
        }
    }
}

impl std::error::Error for MaterializationError {}

/// Replay pruned original rows over Symbolica's rational-polynomial field.
///
/// `rows` must be in the order returned by [`Discovery::trace`]. Like SpIRed's
/// `runGPLU`, stop when `target` is the new pivot and return that normalized U
/// row. Later pivots and back substitution are unnecessary for this solution.
pub fn exact_materialize<const N: usize>(
    rows: &[ExactRow<N>],
    order: &IntegralOrder<N>,
    target: Integral<N>,
) -> Result<ExactRow<N>, MaterializationError> {
    let mut columns = Vec::new();
    for (row_id, row) in rows.iter().enumerate() {
        if !row
            .windows(2)
            .all(|pair| order.compare(&pair[0].integral, &pair[1].integral) == Ordering::Less)
        {
            return Err(MaterializationError::NonCanonicalRow { row: row_id });
        }
        columns.extend(row.iter().map(|term| term.integral));
    }
    columns.sort_unstable_by(|a, b| order.compare(a, b));
    columns.dedup();
    let target_column = columns
        .binary_search_by(|column| order.compare(column, &target))
        .map_err(|_| MaterializationError::TargetAbsent)?;
    let native_columns = columns
        .len()
        .checked_add(1)
        .and_then(|count| u32::try_from(count).ok())
        .ok_or(MaterializationError::TooManyColumns)?;
    let field = ExactField::new(Z);
    let mut reducer = SparseRowReducer::new(native_columns, field, LuLMode::None);
    let mut values = Vec::new();
    let mut column_ids = Vec::new();
    for row in rows {
        values.clear();
        column_ids.clear();
        for term in row {
            if !term.coefficient.is_zero() {
                let column = columns
                    .binary_search_by(|column| order.compare(column, &term.integral))
                    .expect("exact column union contains every row integral");
                values.push(term.coefficient.clone());
                column_ids.push(column as u32);
            }
        }
        if reducer.add_row(&values, &column_ids) == Some(target_column as u32) {
            let u = reducer.u();
            let start = u.row_ptrs()[u.nrows() as usize - 1];
            let end = u.row_ptrs()[u.nrows() as usize];
            return Ok(u.col_idcs()[start..end]
                .iter()
                .zip(&u.values()[start..end])
                .map(|(&column, coefficient)| Term {
                    integral: columns[column as usize],
                    coefficient: coefficient.clone(),
                })
                .collect());
        }
    }
    Err(MaterializationError::TargetNotPivot)
}

#[cfg(test)]
mod tests {
    use symbolica::domains::finite_field::FiniteFieldCore;

    use crate::algebra::CoefficientContext;

    use super::*;

    fn order() -> IntegralOrder<1> {
        IntegralOrder::new([true], [false])
    }

    fn integral(shift: i16) -> Integral<1> {
        Integral::symbolic([shift]).unwrap()
    }

    fn numerical(field: &Zp64, entries: &[(i16, u64)]) -> Vec<Term<1, NumericalCoefficient>> {
        entries
            .iter()
            .map(|&(shift, coefficient)| Term {
                integral: integral(shift),
                coefficient: field.to_element(coefficient),
            })
            .collect()
    }

    #[test]
    fn dynamic_columns_keep_live_pivots_and_trace_in_integral_order() {
        let field = Zp64::new(101);
        let mut discovery = Discovery::new(order(), field.clone());
        assert_eq!(
            discovery.add_row(&numerical(&field, &[(3, 1), (1, 1)])),
            Some(integral(3))
        );
        // Two insertions before the first old column, one between the old
        // columns, and one before the trailing sentinel all happen together.
        let row = [(5, 1), (4, 1), (3, 1), (2, 1), (1, 1), (0, 1)];
        assert_eq!(
            discovery.add_row(&numerical(&field, &row)),
            Some(integral(5))
        );
        assert_eq!(discovery.reducer.pivots()[2], Some(0));
        assert_eq!(
            discovery.add_row(&numerical(
                &field,
                &[(5, 1), (4, 1), (3, 1), (2, 2), (1, 1), (0, 1)],
            )),
            Some(integral(2))
        );
        assert_eq!(discovery.trace(2), [1, 2]);
        assert_eq!(discovery.trace(0), [0]);
        assert_eq!(discovery.trace_many(&[0, 2]), [1, 0, 2]);
        assert_eq!(discovery.trace_many(&[2, 0, 2]), [1, 0, 2]);
        assert!(discovery.trace_many(&[]).is_empty());
        assert_eq!(discovery.stats().columns, 6);
        assert_eq!(discovery.reducer.u().ncols(), 7);
    }

    #[test]
    fn dependent_and_empty_inputs_do_not_shift_accepted_dependency_ids() {
        let field = Zp64::new(101);
        let mut discovery = Discovery::new(order(), field.clone());
        let first = numerical(&field, &[(5, 1), (3, 1)]);
        assert_eq!(discovery.add_row(&first), Some(integral(5)));
        assert_eq!(
            discovery.add_row(&numerical(&field, &[(5, 2), (3, 2)])),
            None
        );
        assert_eq!(discovery.add_row(&[]), None);
        let second = numerical(&field, &[(5, 1), (3, 2), (1, 1)]);
        assert_eq!(discovery.add_row(&second), Some(integral(3)));
        assert_eq!(discovery.add_row(&second), None);
        assert_eq!(
            discovery.add_row(&numerical(&field, &[(3, 1), (1, 2)])),
            Some(integral(1))
        );
        assert_eq!(discovery.accepted_l_rows, [0, 2, 4]);
        assert_eq!(discovery.trace(2), [0, 1, 2]);
        assert_eq!(discovery.basis_len(), 3);
        assert_eq!(discovery.stats().rows_seen, 6);
        assert_eq!(discovery.stats().dependency_edges, 2);
        assert!(discovery.reducer.l().values().is_empty());
    }

    #[test]
    fn full_physical_rank_and_structural_zero_columns_remain_extendable() {
        let field = Zp64::new(101);
        let mut discovery = Discovery::new(order(), field.clone());
        let first = numerical(&field, &[(2, 1)]);
        assert_eq!(discovery.add_row(&first), Some(integral(2)));
        assert_eq!(discovery.add_row(&first), None);
        // Even at full physical rank the native reducer emitted the dependent
        // L slice, because the sentinel prevents its full-rank early return.
        assert_eq!(discovery.reducer.l().nrows(), 2);
        assert_eq!(
            discovery.add_row(&numerical(&field, &[(4, 0), (3, 1), (2, 1)])),
            Some(integral(3))
        );
        assert_eq!(discovery.columns, [integral(4), integral(3), integral(2)]);
        assert_eq!(discovery.trace(1), [1]);
        assert_eq!(discovery.reducer.pivots()[0], None);
        assert_eq!(
            discovery.add_row(&numerical(&field, &[(4, 1)])),
            Some(integral(4))
        );
    }

    #[test]
    fn stats_expose_dependent_l_storage_growth_at_fixed_basis_size() {
        let field = Zp64::new(101);
        let mut discovery = Discovery::new(order(), field.clone());
        assert_eq!(
            discovery.add_row(&numerical(&field, &[(3, 1), (1, 1)])),
            Some(integral(3))
        );
        assert_eq!(
            discovery.add_row(&numerical(&field, &[(2, 1), (1, 1)])),
            Some(integral(2))
        );
        let dependent = numerical(&field, &[(3, 1), (2, 1), (1, 2)]);
        for repetitions in 1..=4 {
            assert_eq!(discovery.add_row(&dependent), None);
            let stats = discovery.stats();
            assert_eq!(stats.independent_rows, 2);
            assert_eq!(stats.reducer_nonzeros, 4);
            assert_eq!(stats.dependency_edges, 0);
            assert_eq!(stats.retained_l_rows, 2 + repetitions);
            assert_eq!(stats.retained_l_entries, 2 + 2 * repetitions);
        }
        // The retained patterns consume indices even though Pattern mode has
        // no values; nvalues() alone would incorrectly report zero storage.
        assert!(discovery.reducer.l().values().is_empty());
        assert_eq!(discovery.trace(0), [0]);
        assert_eq!(discovery.trace(1), [1]);
        let before_empty = discovery.stats();
        assert_eq!(discovery.add_row(&[]), None);
        assert_eq!(
            discovery.stats().retained_l_rows,
            before_empty.retained_l_rows
        );
        assert_eq!(
            discovery.stats().retained_l_entries,
            before_empty.retained_l_entries
        );
    }

    #[test]
    fn target_trace_prunes_unrelated_rows_and_keeps_transitive_dependencies() {
        let field = Zp64::new(101);
        let mut discovery = Discovery::new(order(), field.clone());
        for entries in [
            &[(6, 1), (-2, 1)][..],
            &[(4, 1), (2, 1)][..],
            &[(4, 1), (2, 2), (0, 1)][..],
            &[(2, 1), (0, 2)][..],
        ] {
            assert!(discovery.add_row(&numerical(&field, entries)).is_some());
        }
        assert_eq!(discovery.trace(3), [1, 2, 3]);
        assert_eq!(discovery.trace(1), [1]);
    }

    #[test]
    fn rational_exact_replay_uses_pruned_originals_and_stops_at_the_target() {
        let field = Zp64::new(101);
        let mut discovery = Discovery::new(order(), field.clone());
        for entries in [
            &[(5, 1), (0, 1)][..],
            &[(3, 2), (2, 3), (1, 1)][..],
            &[(3, 1), (2, 3), (1, 2)][..],
        ] {
            assert!(discovery.add_row(&numerical(&field, entries)).is_some());
        }
        let context = CoefficientContext::new(["x"]);
        let originals: Vec<ExactRow<1>> = [
            &[(5, "1"), (0, "1")][..],
            &[(3, "x/(x+1)"), (2, "1"), (1, "1/(x+1)")][..],
            &[(3, "1/(x+1)"), (2, "1"), (1, "2/(x+1)")][..],
        ]
        .iter()
        .map(|entries| {
            entries
                .iter()
                .map(|&(shift, coefficient)| Term {
                    integral: integral(shift),
                    coefficient: context.coefficient_fixture(coefficient),
                })
                .collect()
        })
        .collect();
        let selected = discovery.trace(2);
        assert_eq!(selected, [1, 2]);
        let mut rows: Vec<_> = selected.iter().map(|&row| originals[row].clone()).collect();
        // A later exact pivot must not change the already produced target row.
        rows.push(vec![Term {
            integral: integral(1),
            coefficient: context.one(),
        }]);
        let result = exact_materialize(&rows, &order(), integral(2)).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].integral, integral(2));
        assert_eq!(result[0].coefficient, context.one());
        assert_eq!(result[1].integral, integral(1));
        assert_eq!(
            result[1].coefficient,
            context.coefficient_fixture("(2*x-1)/((x+1)*(x-1))")
        );
    }

    #[test]
    fn exact_replay_reports_absent_nonpivot_and_noncanonical_inputs() {
        let context = CoefficientContext::new(["x"]);
        let mut row = vec![
            Term {
                integral: integral(3),
                coefficient: context.one(),
            },
            Term {
                integral: integral(2),
                coefficient: context.one(),
            },
        ];
        assert_eq!(
            exact_materialize(&[row.clone()], &order(), integral(1)),
            Err(MaterializationError::TargetAbsent)
        );
        assert_eq!(
            exact_materialize(&[row.clone()], &order(), integral(2)),
            Err(MaterializationError::TargetNotPivot)
        );
        row.reverse();
        assert_eq!(
            exact_materialize(&[row], &order(), integral(2)),
            Err(MaterializationError::NonCanonicalRow { row: 0 })
        );
    }
}
