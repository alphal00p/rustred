//! Public session limits and exact operation census.

use crate::algebra::ExactAlgebraLimits;

const DEFAULT_MAX_SINGLE_MATRIX_ENTRIES: usize = 16_000_000;
const DEFAULT_MAX_LIVE_MATRIX_ENTRIES: usize = 32_000_000;
pub(crate) const DEFAULT_MAX_EXACT_OPERATIONS: usize = 100_000_000;
pub(crate) const DEFAULT_MAX_INPUT_RETAINED_BYTES: usize = 1024 * 1024 * 1024;
pub(crate) const DEFAULT_MAX_OUTPUT_RETAINED_BYTES: usize = 1024 * 1024 * 1024;

/// Admission policy for one bounded Symbolica coefficient or matrix session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SymbolicaCoefficientMatrixLimits {
    pub(crate) exact_algebra: ExactAlgebraLimits,
    /// Largest individual native matrix payload. General inversion needs the
    /// augmented `n x 2n` matrix here.
    pub(crate) max_single_matrix_entries: usize,
    /// Largest conservative simultaneously-live native payload.
    pub(crate) max_live_matrix_entries: usize,
    /// Largest number of checked exact arithmetic operations admitted for the
    /// complete requested native operation. Constant construction and
    /// zero/one predicates are censused separately.
    pub(crate) max_exact_operations: usize,
    /// Aggregate clone-owned retained bytes in authenticated caller inputs.
    pub(crate) max_input_retained_bytes: usize,
    /// Aggregate clone-owned retained bytes in powers, determinants, inverses,
    /// and verification-product outputs inspected during the native session.
    pub(crate) max_output_retained_bytes: usize,
}

impl SymbolicaCoefficientMatrixLimits {
    /// Adapt the historical family limit, which bounds the `n x 2n` augmented
    /// matrix, to this module's individual and live-payload limits.
    pub(crate) const fn for_family(
        exact_algebra: ExactAlgebraLimits,
        max_augmented_entries: usize,
        max_exact_operations: usize,
        max_input_retained_bytes: usize,
        max_output_retained_bytes: usize,
    ) -> Self {
        Self {
            exact_algebra,
            max_single_matrix_entries: max_augmented_entries,
            max_live_matrix_entries: max_augmented_entries.saturating_mul(2),
            max_exact_operations,
            max_input_retained_bytes,
            max_output_retained_bytes,
        }
    }
}

impl Default for SymbolicaCoefficientMatrixLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_single_matrix_entries: DEFAULT_MAX_SINGLE_MATRIX_ENTRIES,
            max_live_matrix_entries: DEFAULT_MAX_LIVE_MATRIX_ENTRIES,
            max_exact_operations: DEFAULT_MAX_EXACT_OPERATIONS,
            max_input_retained_bytes: DEFAULT_MAX_INPUT_RETAINED_BYTES,
            max_output_retained_bytes: DEFAULT_MAX_OUTPUT_RETAINED_BYTES,
        }
    }
}

/// Exact census of one admitted native coefficient or matrix session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SymbolicaCoefficientMatrixStats {
    pub(in crate::algebra::matrix) input_entries: usize,
    pub(in crate::algebra::matrix) output_entries: usize,
    pub(in crate::algebra::matrix) authenticated_entries: usize,
    pub(in crate::algebra::matrix) admitted_single_matrix_entries: usize,
    pub(in crate::algebra::matrix) admitted_peak_live_entries: usize,
    pub(in crate::algebra::matrix) admitted_exact_operations: usize,
    pub(in crate::algebra::matrix) input_retained_bytes: usize,
    pub(in crate::algebra::matrix) output_retained_bytes: usize,
    pub(in crate::algebra::matrix) exact_operations: usize,
    pub(in crate::algebra::matrix) additions: usize,
    pub(in crate::algebra::matrix) subtractions: usize,
    pub(in crate::algebra::matrix) multiplications: usize,
    pub(in crate::algebra::matrix) divisions: usize,
    pub(in crate::algebra::matrix) negations: usize,
    pub(in crate::algebra::matrix) zero_constants: usize,
    pub(in crate::algebra::matrix) one_constants: usize,
    pub(in crate::algebra::matrix) zero_tests: usize,
    pub(in crate::algebra::matrix) one_tests: usize,
    pub(in crate::algebra::matrix) determinant_calls: usize,
    pub(in crate::algebra::matrix) inverse_calls: usize,
    pub(in crate::algebra::matrix) product_calls: usize,
    pub(in crate::algebra::matrix) transpose_calls: usize,
    pub(in crate::algebra::matrix) rank_calls: usize,
    pub(in crate::algebra::matrix) power_calls: usize,
    pub(in crate::algebra::matrix) admitted_power_exponent: u64,
    pub(in crate::algebra::matrix) admitted_power_term_operations: usize,
    pub(in crate::algebra::matrix) admitted_power_numerator_terms: usize,
    pub(in crate::algebra::matrix) admitted_power_denominator_terms: usize,
    pub(in crate::algebra::matrix) output_power_numerator_terms: usize,
    pub(in crate::algebra::matrix) output_power_denominator_terms: usize,
    pub(in crate::algebra::matrix) non_matrix_trait_calls: usize,
}

impl SymbolicaCoefficientMatrixStats {
    pub(crate) const fn admitted_single_matrix_entries(self) -> usize {
        self.admitted_single_matrix_entries
    }

    pub(crate) const fn admitted_peak_live_entries(self) -> usize {
        self.admitted_peak_live_entries
    }

    pub(crate) const fn admitted_exact_operations(self) -> usize {
        self.admitted_exact_operations
    }

    pub(crate) const fn exact_operations(self) -> usize {
        self.exact_operations
    }

    pub(crate) const fn input_retained_bytes(self) -> usize {
        self.input_retained_bytes
    }

    pub(crate) const fn output_retained_bytes(self) -> usize {
        self.output_retained_bytes
    }

    pub(crate) const fn determinant_calls(self) -> usize {
        self.determinant_calls
    }

    pub(crate) const fn product_calls(self) -> usize {
        self.product_calls
    }

    pub(crate) const fn transpose_calls(self) -> usize {
        self.transpose_calls
    }

    pub(crate) const fn rank_calls(self) -> usize {
        self.rank_calls
    }
}
