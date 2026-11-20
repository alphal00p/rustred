//! The global modular system: one `Discovery` over every seeded row.
//!
//! Batches arrive top-down by sector level; within a batch rows are inserted
//! easiest first (by leading integral), with one native column registration
//! per batch. Accepted rows keep only their provenance.
//!
//! The pivot set depends only on the row space and the column order. Within
//! one batch the result is a function of the row set; across batches (top-down
//! insertion) U, the L patterns and hence the selected rows also depend on the
//! batch partition, while the pivots and the masters do not. `register` and
//! `insert_batch` return `Result` for future fallible storage but cannot fail
//! today.

use std::cmp::Ordering;
use std::sync::Arc;

use symbolica::domains::finite_field::Zp64;
use symbolica::tensors::sparse::{ParallelOptions, ParallelStats, Provisional};

use super::super::discovery::{ColumnComparator, Discovery};
use super::super::{Integral, SolverError};
use super::order::BoxOrder;
use super::rows::{Provenance, Zp, ZpRow};

impl<const N: usize, T: ColumnComparator<N>> ColumnComparator<N> for Arc<T> {
    fn compare(&self, left: &Integral<N>, right: &Integral<N>) -> Ordering {
        (**self).compare(left, right)
    }
}

pub(crate) struct GlobalDiscovery<const N: usize> {
    pub(crate) discovery: Discovery<N, Arc<BoxOrder<N>>>,
    /// Provenance of each accepted row, by basis ordinal.
    pub(crate) provenance: Vec<Provenance>,
    /// Options and work counters of the last batch when it was eliminated in
    /// parallel.
    pub(crate) last_parallel: Option<(ParallelOptions, ParallelStats)>,
    /// Wall seconds of the last parallel batch: sort, column registration,
    /// native rows, forward solve (numerica), bookkeeping, dropping the rows.
    pub(crate) last_parallel_stages: [f64; 6],
}

/// The parallel elimination options, when it is on.
///
/// Opt-in while it is measured: `RUSTRED_SB_PARALLEL_ELIM=1` eliminates every
/// batch with numerica's `add_csr_rows_parallel` on the session's worker pool
/// (U, the pivots, the L patterns and the accepted ordinals are identical to
/// the serial elimination for all options and thread counts).
/// `RUSTRED_SB_PARELIM_WINDOW`, `RUSTRED_SB_PARELIM_BLOCK` and
/// `RUSTRED_SB_PARELIM_PROVISIONAL` (`copies`, `all`, `none`) override the
/// numerica defaults (speed only).
pub(crate) fn parallel_options() -> Option<ParallelOptions> {
    let on = std::env::var("RUSTRED_SB_PARALLEL_ELIM")
        .is_ok_and(|value| !value.is_empty() && value != "0");
    if !on {
        return None;
    }
    let read = |name: &str, default: usize| {
        std::env::var(name)
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|&value| value > 0)
            .unwrap_or(default)
    };
    let defaults = ParallelOptions::default();
    let provisional = match std::env::var("RUSTRED_SB_PARELIM_PROVISIONAL").as_deref() {
        Ok("all") => Provisional::All,
        Ok("none") => Provisional::None,
        _ => defaults.provisional,
    };
    Some(ParallelOptions {
        window: read("RUSTRED_SB_PARELIM_WINDOW", defaults.window),
        block: read("RUSTRED_SB_PARELIM_BLOCK", defaults.block),
        provisional,
    })
}

/// Outcome of one batch insertion.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BatchOutcome {
    pub rows: usize,
    pub accepted: usize,
}

impl<const N: usize> GlobalDiscovery<N> {
    pub(crate) fn new(order: Arc<BoxOrder<N>>, field: Zp64) -> Self {
        Self {
            discovery: Discovery::new(order, field),
            provenance: Vec::new(),
            last_parallel: None,
            last_parallel_stages: [0.0; 6],
        }
    }

    /// A fresh, empty system (used when a reorder rebuilds the system).
    pub(crate) fn rebuilt(
        order: Arc<BoxOrder<N>>,
        field: Zp64,
        _old: &mut Self,
    ) -> Result<Self, SolverError> {
        Ok(Self::new(order, field))
    }

    /// Register integrals as columns (one native `add_cols`).
    pub(crate) fn register(&mut self, integrals: Vec<Integral<N>>) -> Result<(), SolverError> {
        self.discovery.register(integrals);
        Ok(())
    }

    /// Insert one batch: easiest leading integral first, then shorter rows,
    /// then provenance, so the result is a function of the row set alone.
    pub(crate) fn insert_batch(
        &mut self,
        mut rows: Vec<ZpRow<N>>,
    ) -> Result<BatchOutcome, SolverError> {
        let order = self.discovery.order().clone();
        let compare = |left: &ZpRow<N>, right: &ZpRow<N>| {
            order
                .compare(&right.terms[0].integral, &left.terms[0].integral)
                .then(left.terms.len().cmp(&right.terms.len()))
                .then(left.provenance.cmp(&right.provenance))
        };
        let parallel = parallel_options();
        let sort_started = std::time::Instant::now();
        if parallel.is_some() {
            // provenance is unique within a batch, so the order is total and any sort agrees
            use rayon::slice::ParallelSliceMut;
            rows.par_sort_unstable_by(compare);
        } else {
            rows.sort_unstable_by(compare);
        }
        let sort_seconds = sort_started.elapsed().as_secs_f64();
        let pivots = match parallel {
            Some(options) => {
                let (pivots, stats, stages) = self.discovery.add_rows_parallel(&rows, options);
                self.last_parallel = Some((options, stats));
                self.last_parallel_stages = [
                    sort_seconds,
                    stages[0],
                    stages[1],
                    stages[2],
                    stages[3],
                    0.0,
                ];
                pivots
            }
            None => {
                self.last_parallel = None;
                self.discovery.add_rows(&rows)
            }
        };
        let mut outcome = BatchOutcome {
            rows: rows.len(),
            accepted: 0,
        };
        for (row, pivot) in rows.iter().zip(pivots) {
            if pivot.is_some() {
                self.provenance.push(row.provenance);
                outcome.accepted += 1;
            }
        }
        debug_assert_eq!(self.provenance.len(), self.discovery.basis_len());
        if self.last_parallel.is_some() {
            let started = std::time::Instant::now();
            drop(rows);
            self.last_parallel_stages[5] = started.elapsed().as_secs_f64();
        }
        Ok(outcome)
    }

    #[allow(dead_code)] // used by demand seeding (a later step)
    pub(crate) fn rank(&self) -> usize {
        self.discovery.basis_len()
    }

    /// Structural estimate of resident modular state in bytes.
    pub(crate) fn bytes(&self) -> usize {
        let u = self.discovery.u_nnz() * (std::mem::size_of::<Zp>() + 4);
        let l = self.discovery.l_entries() * 4;
        let columns = self.discovery.columns().len() * (N + 8 + 8 + 16);
        let rows = self.provenance.len() * (std::mem::size_of::<Provenance>() + 16);
        u + l + columns + rows
    }
}
