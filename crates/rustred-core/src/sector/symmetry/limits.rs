use crate::algebra::ExactAlgebraLimits;
use crate::algebra::matrix::{DEFAULT_MAX_INPUT_RETAINED_BYTES, DEFAULT_MAX_OUTPUT_RETAINED_BYTES};

use super::Error;

pub const DEFAULT_MAX_MATRIX_ENTRIES: usize = 16_000_000;

/// Aggregate bounds for one authoritative affine-map verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_matrix_entries: usize,
    /// Aggregate replayable admission envelope for checked scalar calls and
    /// native Symbolica schedules. Actual native calls are retained separately
    /// in [`Stats::symbolica_exact_operations`].
    pub max_exact_operations: usize,
    /// Largest individual matrix admitted inside one authenticated Symbolica
    /// determinant or product session.
    pub max_symbolica_single_matrix_entries: usize,
    /// Largest conservative simultaneously-live native matrix payload in one
    /// authenticated Symbolica session.
    pub max_symbolica_live_matrix_entries: usize,
    /// Aggregate clone-owned bytes copied into authenticated Symbolica matrix
    /// inputs across one complete derivation/replay pass.
    pub max_symbolica_input_retained_bytes: usize,
    /// Aggregate clone-owned bytes authenticated in native determinant and
    /// product outputs across one complete derivation/replay pass.
    pub max_symbolica_output_retained_bytes: usize,
    pub max_nonzero_conditions: usize,
    pub max_condition_sources: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_matrix_entries: DEFAULT_MAX_MATRIX_ENTRIES,
            max_exact_operations: 100_000_000,
            max_symbolica_single_matrix_entries: 16_000_000,
            max_symbolica_live_matrix_entries: 32_000_000,
            max_symbolica_input_retained_bytes: DEFAULT_MAX_INPUT_RETAINED_BYTES,
            max_symbolica_output_retained_bytes: DEFAULT_MAX_OUTPUT_RETAINED_BYTES,
            max_nonzero_conditions: 1_000_000,
            max_condition_sources: 4_000_000,
        }
    }
}

/// Observable work performed by one successful verification.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub(super) matrix_entries: usize,
    pub(super) exact_operations: usize,
    pub(super) symbolica_exact_operations: usize,
    pub(super) symbolica_admitted_exact_operations: usize,
    pub(super) symbolica_largest_matrix_entries: usize,
    pub(super) symbolica_peak_live_matrix_entries: usize,
    pub(super) symbolica_input_retained_bytes: usize,
    pub(super) symbolica_output_retained_bytes: usize,
    pub(super) symbolica_determinant_calls: usize,
    pub(super) symbolica_product_calls: usize,
    pub(super) symbolica_transpose_calls: usize,
    pub(super) nonzero_conditions: usize,
    pub(super) condition_sources: usize,
}

impl Stats {
    pub const fn matrix_entries(self) -> usize {
        self.matrix_entries
    }

    pub const fn exact_operations(self) -> usize {
        self.exact_operations
    }

    /// Exact arithmetic calls actually observed inside native Symbolica
    /// determinant and product sessions.
    pub const fn symbolica_exact_operations(self) -> usize {
        self.symbolica_exact_operations
    }

    /// Aggregate public-Symbolica operation envelope admitted before native
    /// execution. This may exceed `symbolica_exact_operations` for a
    /// data-dependent determinant schedule.
    pub const fn symbolica_admitted_exact_operations(self) -> usize {
        self.symbolica_admitted_exact_operations
    }

    pub const fn symbolica_largest_matrix_entries(self) -> usize {
        self.symbolica_largest_matrix_entries
    }

    pub const fn symbolica_peak_live_matrix_entries(self) -> usize {
        self.symbolica_peak_live_matrix_entries
    }

    pub const fn symbolica_input_retained_bytes(self) -> usize {
        self.symbolica_input_retained_bytes
    }

    pub const fn symbolica_output_retained_bytes(self) -> usize {
        self.symbolica_output_retained_bytes
    }

    pub const fn symbolica_determinant_calls(self) -> usize {
        self.symbolica_determinant_calls
    }

    pub const fn symbolica_product_calls(self) -> usize {
        self.symbolica_product_calls
    }

    pub const fn symbolica_transpose_calls(self) -> usize {
        self.symbolica_transpose_calls
    }

    pub const fn nonzero_conditions(self) -> usize {
        self.nonzero_conditions
    }

    pub const fn condition_sources(self) -> usize {
        self.condition_sources
    }
}

pub(super) fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, Error> {
    left.checked_add(right)
        .ok_or(Error::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), Error> {
    if requested > limit {
        Err(Error::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
