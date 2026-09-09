use crate::algebra::{ExactAlgebraLimits, IndexedAlgebraLimits};

/// Bounded policy for one sector- and ordering-local source preconditioner.
///
/// Symbolica's sparse reducer and multivariate GCD do not expose allocator
/// callbacks.  The dense-entry and retained-entry bounds are therefore
/// conservative ingress envelopes; every native result is authenticated
/// again before it enters the proof object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredSourceBasisLimits {
    pub(super) exact_algebra: ExactAlgebraLimits,
    pub(super) translation: IndexedAlgebraLimits,
    pub(super) max_source_rows: usize,
    pub(super) max_physical_columns: usize,
    pub(super) max_input_nonzero_entries: usize,
    pub(super) max_augmented_columns: usize,
    pub(super) max_native_dense_entry_bound: usize,
    pub(super) max_native_retained_nonzero_entries: usize,
    pub(super) max_basis_rows: usize,
    pub(super) max_basis_term_entries: usize,
    pub(super) max_provenance_entries: usize,
    pub(super) max_reverse_span_entries: usize,
    pub(super) max_polynomial_operations: usize,
    pub(super) max_replay_exact_operations: usize,
}

impl Default for SpiredSourceBasisLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            translation: IndexedAlgebraLimits::default(),
            max_source_rows: 65_536,
            max_physical_columns: 1_000_000,
            max_input_nonzero_entries: 16_000_000,
            max_augmented_columns: 1_100_000,
            max_native_dense_entry_bound: 64_000_000,
            max_native_retained_nonzero_entries: 64_000_000,
            max_basis_rows: 65_536,
            max_basis_term_entries: 16_000_000,
            max_provenance_entries: 16_000_000,
            max_reverse_span_entries: 16_000_000,
            max_polynomial_operations: 16_000_000,
            max_replay_exact_operations: 64_000_000,
        }
    }
}

impl SpiredSourceBasisLimits {
    #[cfg(test)]
    pub(super) const fn with_max_physical_columns(mut self, limit: usize) -> Self {
        self.max_physical_columns = limit;
        self
    }

    #[cfg(test)]
    pub(super) const fn with_max_native_dense_entry_bound(mut self, limit: usize) -> Self {
        self.max_native_dense_entry_bound = limit;
        self
    }

    #[cfg(test)]
    pub(super) const fn with_max_replay_exact_operations(mut self, limit: usize) -> Self {
        self.max_replay_exact_operations = limit;
        self
    }

    #[cfg(test)]
    pub(super) const fn with_max_polynomial_operations(mut self, limit: usize) -> Self {
        self.max_polynomial_operations = limit;
        self
    }
}
