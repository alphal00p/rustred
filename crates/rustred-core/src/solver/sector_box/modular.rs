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
        rows.sort_unstable_by(|left, right| {
            order
                .compare(&right.terms[0].integral, &left.terms[0].integral)
                .then(left.terms.len().cmp(&right.terms.len()))
                .then(left.provenance.cmp(&right.provenance))
        });
        let pivots = self.discovery.add_rows(&rows);
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
