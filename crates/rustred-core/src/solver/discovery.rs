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

mod fraction_free;
mod semi_numerical;
mod target_only;
mod variables;

/// Exact lifting for a single symbolic target. Shared numerical-tail lifting
/// remains sparse; this diagnostic choice does not change source discovery.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SymbolicExactBackend {
    #[default]
    Sparse,
    /// Native GPLU on the harder/target block, followed by a native triangular
    /// weight solve and one full-row product. Requires an independent prefix.
    /// This is an opt-in scheduling experiment, not a different source search.
    SparseTargetOnly,
    /// Native dense polynomial elimination. The bound limits initial matrix
    /// slots, not coefficient growth or total memory. Use an external run cap.
    DenseFractionFree { max_matrix_entries: usize },
    /// Reconstruct the target row from finite-field black-box evaluations
    /// using Symbolica's native multivariate rational reconstruction. This is
    /// SpIReD's semi-numerical route; reconstructed rows still pass the normal
    /// exact replay/publication gates. The bounds are deliberately explicit.
    SemiNumerical {
        max_degree: u16,
        max_probes: usize,
        max_attempts: usize,
        max_primes: usize,
    },
}

/// Native coefficient-variable order during single-target exact lifting only.
/// This bijective representation change never reorders integral columns or
/// source rows. The original variable map is restored before rule extraction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CoefficientVariableOrder {
    #[default]
    Original,
    Reverse,
    /// Registered source priority: physical index variables, dimension when
    /// identifiable as one parameter, then the remaining parameters. This
    /// changes variable layout in the native Lex field, not monomial order.
    IndicesFirst,
}

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

/// Exact-frame diagnostics, distinct from the larger modular discovery system.
/// All sizes are structural native storage counts; no coefficient expansion,
/// expression copies, timing or formatting is performed for these events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterializationEvent<const N: usize> {
    FramePrepared {
        source_rows: usize,
        integral_columns: usize,
        target_column: usize,
        /// Stored input terms, including any explicit coefficient zeros.
        input_terms: usize,
        coefficient_variables: usize,
        active_variables: usize,
    },
    DenseFractionFreeStarted {
        rows: usize,
        columns: usize,
        reduction_columns: usize,
        /// True for the native Q-polynomial lane; otherwise native Z.
        rational_coefficients: bool,
    },
    DenseFractionFreeFinished {
        rank: usize,
    },
    TargetBlockStarted {
        columns: usize,
    },
    TargetWeightsStarted {
        rows: usize,
        lower_nonzeros: usize,
    },
    TargetWeightsFinished {
        nonzero_weights: usize,
    },
    TargetReconstructionStarted {
        rows: usize,
        columns: usize,
    },
    TargetReconstructionFinished {
        output_terms: usize,
    },
    SemiNumericalStarted {
        rows: usize,
        columns: usize,
        variables: usize,
    },
    SemiNumericalCoefficient {
        column: usize,
        probes: usize,
        primes: usize,
    },
    SemiNumericalFinished {
        output_terms: usize,
    },
    RowStarted {
        /// One-based ordinal in the selected original-source trace.
        row: usize,
        input_nonzeros: usize,
        reducer_rows: usize,
        reducer_nonzeros: usize,
    },
    RowFinished {
        row: usize,
        pivot: Option<Integral<N>>,
        reducer_rows: usize,
        reducer_nonzeros: usize,
    },
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MaterializationError {
    TooManyColumns,
    NonCanonicalRow {
        row: usize,
    },
    TargetAbsent,
    TargetNotPivot,
    CoefficientVariableMapMismatch,
    InvalidCoefficientVariablePriority,
    TargetOnlyDependentPrefix {
        row: usize,
    },
    TargetOnlyInvalidDecomposition(&'static str),
    CoefficientVariableRemap(String),
    FractionFreeNonPolynomialCoefficient {
        row: usize,
        term: usize,
    },
    FractionFreeMatrixBudget {
        rows: usize,
        columns: usize,
        limit: usize,
    },
    FractionFreeDimensionOverflow,
    SemiNumericalReconstruction(String),
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
            Self::TargetOnlyDependentPrefix { row } => write!(
                f,
                "target-only lifting requires an independent harder/target prefix; row {row} is empty or dependent"
            ),
            Self::TargetOnlyInvalidDecomposition(reason) => {
                write!(f, "invalid native target-only decomposition: {reason}")
            }
            Self::FractionFreeNonPolynomialCoefficient { row, term } => write!(
                f,
                "fraction-free lifting requires polynomial input over Z or Q; row {row}, term {term} has a zero or variable-dependent denominator"
            ),
            Self::FractionFreeMatrixBudget {
                rows,
                columns,
                limit,
            } => write!(
                f,
                "fraction-free matrix {rows}x{columns} exceeds the {limit}-entry initial allocation budget"
            ),
            Self::FractionFreeDimensionOverflow => {
                write!(f, "fraction-free matrix dimensions overflow native storage")
            }
            Self::SemiNumericalReconstruction(reason) => {
                write!(f, "semi-numerical reconstruction failed: {reason}")
            }
            Self::CoefficientVariableMapMismatch => {
                write!(
                    f,
                    "selected exact coefficients have inconsistent variable maps"
                )
            }
            Self::InvalidCoefficientVariablePriority => write!(
                f,
                "exact coefficient priority must permute the complete original variable map"
            ),
            Self::CoefficientVariableRemap(reason) => {
                write!(
                    f,
                    "native exact coefficient variable remap failed: {reason}"
                )
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
#[cfg(test)]
pub fn exact_materialize<const N: usize>(
    rows: &[ExactRow<N>],
    order: &IntegralOrder<N>,
    target: Integral<N>,
) -> Result<ExactRow<N>, MaterializationError> {
    exact_materialize_with_observer(rows, order, target, |_| {})
}

/// The observer brackets each native exact row reduction, including dependent
/// inputs. It cannot change row order, pivots, or the early target stop.
#[cfg(test)]
pub fn exact_materialize_with_observer<const N: usize>(
    rows: &[ExactRow<N>],
    order: &IntegralOrder<N>,
    target: Integral<N>,
    observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    exact_materialize_using_with_observer(
        rows,
        order,
        target,
        SymbolicExactBackend::Sparse,
        CoefficientVariableOrder::Original,
        &[],
        observe,
    )
}

/// Alternative backends change exact elimination scheduling, not the discovery
/// trace. They are diagnostics and grant no artifact or closure authority.
pub(super) fn exact_materialize_using_with_observer<const N: usize>(
    rows: &[ExactRow<N>],
    order: &IntegralOrder<N>,
    target: Integral<N>,
    backend: SymbolicExactBackend,
    coefficient_order: CoefficientVariableOrder,
    source_priority: &[usize],
    mut observe: impl FnMut(MaterializationEvent<N>),
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
    let variables = variables::FrameVariables::try_new(rows, coefficient_order, source_priority)?;
    observe(MaterializationEvent::FramePrepared {
        source_rows: rows.len(),
        integral_columns: columns.len(),
        target_column,
        input_terms: rows.iter().map(Vec::len).sum(),
        coefficient_variables: variables.original_len(),
        active_variables: variables.active_len(),
    });
    if backend == SymbolicExactBackend::SparseTargetOnly {
        return target_only::materialize(rows, &columns, order, target_column, &variables, observe);
    }
    if let SymbolicExactBackend::SemiNumerical {
        max_degree,
        max_probes,
        max_attempts,
        max_primes,
    } = backend
    {
        return semi_numerical::materialize(
            rows,
            &columns,
            order,
            target_column,
            &variables,
            max_degree,
            max_probes,
            max_attempts,
            max_primes,
            observe,
        );
    }
    if let SymbolicExactBackend::DenseFractionFree { max_matrix_entries } = backend {
        return fraction_free::materialize(
            rows,
            &columns,
            order,
            target_column,
            &variables,
            max_matrix_entries,
            observe,
        );
    }
    let field = ExactField::new(Z);
    let mut reducer = SparseRowReducer::new(native_columns, field, LuLMode::None);
    let mut values = Vec::new();
    let mut column_ids = Vec::new();
    for (ordinal, row) in rows.iter().enumerate() {
        values.clear();
        column_ids.clear();
        for term in row {
            if !term.coefficient.is_zero() {
                let column = columns
                    .binary_search_by(|column| order.compare(column, &term.integral))
                    .expect("exact column union contains every row integral");
                values.push(variables.map_coefficient(&term.coefficient)?);
                column_ids.push(column as u32);
            }
        }
        observe(MaterializationEvent::RowStarted {
            row: ordinal + 1,
            input_nonzeros: values.len(),
            reducer_rows: reducer.u().nrows() as usize,
            reducer_nonzeros: reducer.u().nvalues(),
        });
        let pivot = reducer.add_row(&values, &column_ids);
        observe(MaterializationEvent::RowFinished {
            row: ordinal + 1,
            pivot: pivot.map(|column| columns[column as usize]),
            reducer_rows: reducer.u().nrows() as usize,
            reducer_nonzeros: reducer.u().nvalues(),
        });
        if pivot == Some(target_column as u32) {
            let u = reducer.u();
            let start = u.row_ptrs()[u.nrows() as usize - 1];
            let end = u.row_ptrs()[u.nrows() as usize];
            return u.col_idcs()[start..end]
                .iter()
                .zip(&u.values()[start..end])
                .map(|(&column, coefficient)| {
                    Ok(Term {
                        integral: columns[column as usize],
                        coefficient: variables.restore_coefficient(coefficient)?,
                    })
                })
                .collect();
        }
    }
    Err(MaterializationError::TargetNotPivot)
}

#[cfg(test)]
mod tests;
