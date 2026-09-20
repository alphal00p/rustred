//! Shared finite-corner search, corresponding to SpIRed's `solveNumCases`.
//!
//! All corners share one modular reducer, one accepted original-row bank, and
//! one exact replay of the union of the winning dependency traces. Exhaustion
//! leaves explicit residuals: it is not a proof that they are independent
//! masters, nor a certificate of family closure.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::{Integer, Z};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use super::instantiate::{canonicalize, instantiate};
use super::search::Probe;
use super::{
    CoordinateCase, DiscoveryStats, ExactRow, Integral, IntegralOrder, RuleCandidate,
    SearchOptions, SearchStats, SectorSolver, SeedSource, Seeds, SolverError, Term,
};

/// Coefficient representation for the shared exact finite-corner replay.
/// Search, source traces and terminal selection do not depend on this choice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NumericalExactBackend {
    #[default]
    Sparse,
    SparseFactorized,
}

/// Aggregate work of one shared numerical-case search, not a sum of per-rule
/// statistics (which refer to shared work and must not be added together).
#[derive(Clone, Copy, Debug, Default)]
pub struct NumericStats {
    pub cases: usize,
    pub seeds: usize,
    pub duplicate_seeds: usize,
    pub rows: usize,
    pub direct_rules: usize,
    pub modular_rules: usize,
    pub exact_trace_rows: usize,
    pub discovery: Option<DiscoveryStats>,
    pub elapsed: Duration,
    pub exact_materialization: Duration,
}

#[derive(Debug)]
pub struct NumericResult<const N: usize> {
    pub rules: Vec<RuleCandidate<N>>,
    /// Fully fixed corners left unsolved within the requested seed depth.
    /// These are deliberately not called proven masters.
    pub residuals: Vec<CoordinateCase<N>>,
    pub stats: NumericStats,
}

impl<'a, const N: usize> SectorSolver<'a, N> {
    /// Search fully fixed cases together with one shared GPLU system.
    ///
    /// `max_depth` must be present. Seed centres follow SpIRed's equality-case
    /// order (lexicographically decreasing index values). Each centre has its
    /// own signed-L1 depth budget; a global seed set suppresses duplicates
    /// without materializing any shell. Solving the current centre switches
    /// to the next unresolved centre after the current source batch.
    pub fn solve_numeric_cases(
        &self,
        mut cases: Vec<CoordinateCase<N>>,
        options: SearchOptions,
    ) -> Result<NumericResult<N>, SolverError> {
        let max_depth = options.max_depth.ok_or_else(|| {
            SolverError::InvalidInput("shared numerical search requires a finite depth".into())
        })?;
        if options.prime < 3 || !Integer::from(options.prime).is_prime(0) {
            return Err(SolverError::InvalidInput(
                "modular probe requires an odd prime".into(),
            ));
        }
        for case in &cases {
            if !case.is_numerical() || !case.is_in_sector(self.order.sector()) {
                return Err(SolverError::InvalidInput(
                    "shared numerical cases must be fully fixed and inside their sector".into(),
                ));
            }
            if self
                .config
                .removed_deltas
                .iter()
                .zip(case.fixed())
                .any(|(removed, value)| *removed && *value != Some(1))
            {
                return Err(SolverError::InvalidInput(
                    "removed linear deltas must be fixed to one".into(),
                ));
            }
        }
        // C++ simpleDioEq stores the constant -n_i; all indices are fixed,
        // so its case comparison is the reverse of fixed-value comparison.
        cases.sort_unstable_by(|left, right| right.fixed().cmp(left.fixed()));
        cases.dedup();
        let start = Instant::now();
        let mut stats = NumericStats {
            cases: cases.len(),
            ..Default::default()
        };
        let mut rules = Vec::new();
        if cases.is_empty() {
            stats.elapsed = start.elapsed();
            return Ok(NumericResult {
                rules,
                residuals: cases,
                stats,
            });
        }

        // Equality-case order need not agree with complexity order. Use the
        // actual easiest target for the safe row cutoff, not the last case.
        let easiest = cases
            .iter()
            .map(CoordinateCase::integral)
            .max_by(|left, right| self.order.compare(left, right))
            .expect("nonempty case list");
        let mut pending = vec![true; cases.len()];
        let mut remaining = cases.len();
        let mut current = Some(0);
        let mut seeds = Seeds::new(
            cases[0].integral(),
            *self.order.sector(),
            self.config.removed_deltas,
        );
        let mut visited = BTreeSet::new();
        let mut probe: Option<Probe<N>> = None;
        let mut original_rows = Vec::new();
        let mut original_sources = Vec::new();
        let mut modular_hits = Vec::new();

        while remaining != 0 {
            let next = seeds.next();
            // The first excluded-shell seed can overflow the compact power.
            // Check the shell bound before interpreting that seed's result.
            if next.is_none() || seeds.depth() > max_depth {
                current = current.and_then(|from| next_pending(&pending, from));
                let Some(index) = current else { break };
                seeds = Seeds::new(
                    cases[index].integral(),
                    *self.order.sector(),
                    self.config.removed_deltas,
                );
                continue;
            }
            let seed = next.expect("checked seed availability")?;
            if !visited.insert(seed.integral) {
                stats.duplicate_seeds += 1;
                continue;
            }
            stats.seeds += 1;

            for (basis_row, source) in self.basis.iter().enumerate() {
                let row = instantiate(
                    source,
                    &seed,
                    &self.system.indices,
                    self.system.fixed(),
                    &self.order,
                    &self.config.zero_sectors,
                    None,
                )?;
                stats.rows += 1;
                let Some(leading) = row.first() else { continue };
                if self.order.compare(&easiest, &leading.integral) == Ordering::Less {
                    continue;
                }
                let source = SeedSource { basis_row, seed };
                if let Some(index) = matching_case(&cases, &pending, leading.integral) {
                    // Keep the homogeneous source intact for the shared
                    // reducer. Building a RHS must not erase its pivot there.
                    let (target, rhs) = canonicalize(row.clone(), &self.system.indices)?;
                    rules.push(RuleCandidate {
                        case: cases[index].into(),
                        target,
                        rhs,
                        sources: vec![source],
                        stats: SearchStats {
                            seeds: stats.seeds,
                            rows: stats.rows,
                            independent_rows: original_rows.len(),
                            exact_trace_rows: 1,
                            direct_hit: true,
                            elapsed: start.elapsed(),
                            discovery: probe.as_ref().map(|p| p.discovery.stats()),
                            ..Default::default()
                        },
                    });
                    stats.direct_rules += 1;
                    pending[index] = false;
                    remaining -= 1;
                    switch_solved_centre(
                        index,
                        &cases,
                        &pending,
                        &mut current,
                        &mut seeds,
                        *self.order.sector(),
                        self.config.removed_deltas,
                    );
                    if remaining == 0 {
                        break;
                    }
                }

                let probe = probe.get_or_insert_with(|| {
                    Probe::new(self.order, self.system.variable_count, options)
                });
                let modular_row = probe.evaluate(&row)?;
                if let Some(pivot) = probe.discovery.add_row(&modular_row) {
                    original_rows.push(row);
                    original_sources.push(source);
                    if let Some(index) = matching_case(&cases, &pending, pivot) {
                        modular_hits.push((index, original_rows.len() - 1));
                        pending[index] = false;
                        remaining -= 1;
                        switch_solved_centre(
                            index,
                            &cases,
                            &pending,
                            &mut current,
                            &mut seeds,
                            *self.order.sector(),
                            self.config.removed_deltas,
                        );
                    }
                }
                if remaining == 0 {
                    break;
                }
            }
        }

        stats.discovery = probe.as_ref().map(|p| p.discovery.stats());
        if !modular_hits.is_empty() {
            let probe = probe.as_ref().expect("modular hits require a probe");
            let roots: Vec<_> = modular_hits.iter().map(|(_, row)| *row).collect();
            let trace = probe.discovery.trace_many(&roots);
            stats.exact_trace_rows = trace.len();
            let selected: Vec<_> = trace
                .iter()
                .map(|row| std::mem::take(&mut original_rows[*row]))
                .collect();
            drop(original_rows);
            let sources: Vec<_> = trace.iter().map(|row| original_sources[*row]).collect();
            let targets: Vec<_> = modular_hits
                .iter()
                .map(|(index, _)| cases[*index].integral())
                .collect();
            let exact_start = Instant::now();
            let solutions = exact_materialize_many_using(
                &selected,
                &self.order,
                &targets,
                self.config.numerical_exact_backend,
            )?;
            let exact_elapsed = exact_start.elapsed();
            stats.modular_rules = solutions.len();
            stats.exact_materialization = exact_elapsed;
            for (target_id, exact) in solutions {
                let (target, rhs) = canonicalize(exact, &self.system.indices)?;
                rules.push(RuleCandidate {
                    case: cases[modular_hits[target_id].0].into(),
                    target,
                    rhs,
                    sources: sources.clone(),
                    stats: SearchStats {
                        seeds: stats.seeds,
                        rows: stats.rows,
                        independent_rows: probe.discovery.basis_len(),
                        exact_trace_rows: trace.len(),
                        direct_hit: false,
                        elapsed: start.elapsed(),
                        exact_materialization: exact_elapsed,
                        discovery: stats.discovery,
                    },
                });
            }
        }
        stats.elapsed = start.elapsed();
        Ok(NumericResult {
            rules,
            residuals: cases
                .into_iter()
                .zip(pending)
                .filter_map(|(case, pending)| pending.then_some(case))
                .collect(),
            stats,
        })
    }
}

fn next_pending(pending: &[bool], after: usize) -> Option<usize> {
    (after + 1..pending.len()).find(|index| pending[*index])
}

fn matching_case<const N: usize>(
    cases: &[CoordinateCase<N>],
    pending: &[bool],
    integral: Integral<N>,
) -> Option<usize> {
    cases
        .iter()
        .zip(pending)
        .position(|(case, pending)| *pending && case.matches(&integral))
}

fn switch_solved_centre<const N: usize>(
    solved: usize,
    cases: &[CoordinateCase<N>],
    pending: &[bool],
    current: &mut Option<usize>,
    seeds: &mut Seeds<N>,
    sector: [bool; N],
    removed_deltas: [bool; N],
) {
    if *current != Some(solved) {
        return;
    }
    *current = next_pending(pending, solved);
    if let Some(next) = *current {
        // The caller retains the current Seed by value through the current
        // source batch, just like numSeedRunner::changeCase defers its update
        // of currentRepl until the next call of next().
        *seeds = Seeds::new(cases[next].integral(), sector, removed_deltas);
    }
}

/// Replay a union trace once and capture each requested pivot's native U row.
/// This is orchestration around Symbolica GPLU, not an elimination algorithm.
#[cfg(test)]
fn exact_materialize_many<const N: usize>(
    rows: &[ExactRow<N>],
    order: &IntegralOrder<N>,
    targets: &[Integral<N>],
) -> Result<Vec<(usize, ExactRow<N>)>, SolverError> {
    exact_materialize_many_using(rows, order, targets, NumericalExactBackend::Sparse)
}

fn exact_materialize_many_using<const N: usize>(
    rows: &[ExactRow<N>],
    order: &IntegralOrder<N>,
    targets: &[Integral<N>],
    backend: NumericalExactBackend,
) -> Result<Vec<(usize, ExactRow<N>)>, SolverError> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }
    let mut columns = Vec::new();
    for row in rows {
        if !row
            .windows(2)
            .all(|pair| order.compare(&pair[0].integral, &pair[1].integral) == Ordering::Less)
        {
            return Err(SolverError::ExactReplay(
                "noncanonical exact row in numerical union trace".into(),
            ));
        }
        columns.extend(row.iter().map(|term| term.integral));
    }
    columns.sort_unstable_by(|left, right| order.compare(left, right));
    columns.dedup();
    let ncolumns = columns
        .len()
        .checked_add(1)
        .and_then(|count| u32::try_from(count).ok())
        .ok_or_else(|| SolverError::ExactReplay("exact column count exceeds u32".into()))?;
    let mut wanted = vec![None; columns.len()];
    let mut target_columns = Vec::with_capacity(targets.len());
    for (index, target) in targets.iter().enumerate() {
        let column = columns
            .binary_search_by(|column| order.compare(column, target))
            .map_err(|_| SolverError::ExactReplay("target is absent from union trace".into()))?;
        if wanted[column].replace(index).is_some() {
            return Err(SolverError::ExactReplay(
                "duplicate target in numerical union trace".into(),
            ));
        }
        target_columns.push(column);
    }
    if backend == NumericalExactBackend::SparseFactorized {
        return super::discovery::exact_materialize_factorized_targets(
            rows,
            &columns,
            order,
            &target_columns,
        )
        .map_err(|error| SolverError::ExactReplay(error.to_string()));
    }
    let mut reducer =
        SparseRowReducer::new(ncolumns, RationalPolynomialField::new(Z), LuLMode::None);
    let mut values = Vec::new();
    let mut column_ids = Vec::new();
    let mut solutions = Vec::with_capacity(targets.len());
    for row in rows {
        values.clear();
        column_ids.clear();
        for term in row {
            if !term.coefficient.is_zero() {
                values.push(term.coefficient.clone());
                column_ids.push(
                    columns
                        .binary_search_by(|column| order.compare(column, &term.integral))
                        .expect("union contains every term") as u32,
                );
            }
        }
        let Some(pivot) = reducer.add_row(&values, &column_ids) else {
            continue;
        };
        if let Some(target) = wanted[pivot as usize].take() {
            let u = reducer.u();
            let start = u.row_ptrs()[u.nrows() as usize - 1];
            let end = u.row_ptrs()[u.nrows() as usize];
            solutions.push((
                target,
                u.col_idcs()[start..end]
                    .iter()
                    .zip(&u.values()[start..end])
                    .map(|(&column, coefficient)| Term {
                        integral: columns[column as usize],
                        coefficient: coefficient.clone(),
                    })
                    .collect(),
            ));
            if solutions.len() == targets.len() {
                return Ok(solutions);
            }
        }
    }
    Err(SolverError::ExactReplay(
        "exact union trace does not recover every modular target pivot".into(),
    ))
}

#[cfg(test)]
mod tests;
