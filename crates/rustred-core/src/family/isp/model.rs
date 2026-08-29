//! ISP-completion resource policy and deterministic work census.

use crate::algebra::matrix::{DEFAULT_MAX_INPUT_RETAINED_BYTES, DEFAULT_MAX_OUTPUT_RETAINED_BYTES};
use crate::family::IntegralFamilyLimits;

/// Stable schema whose work census counts Symbolica's native rank operations.
pub const ISP_COMPLETION_V2_SCHEMA: &str = "rustred-isp-completion-v2";

/// Resource policy for the generic-rank completion pass and final family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IspCompletionLimits {
    pub family: IntegralFamilyLimits,
    /// Largest transient rectangular matrix admitted by one rank test.
    pub max_rank_matrix_entries: usize,
    /// Aggregate numerator-plus-denominator terms admitted before one native
    /// rank-matrix copy.
    pub max_rank_coefficient_terms: usize,
    /// Aggregate canonical-display bytes admitted before one native
    /// rank-matrix copy.
    pub max_rank_coefficient_bytes: usize,
    /// Clone-owned bytes admitted for one authenticated native rank input.
    pub max_rank_input_retained_bytes: usize,
    /// Retained bytes admitted while authenticating one native rank output.
    pub max_rank_output_retained_bytes: usize,
    /// Cumulative checked exact arithmetic calls admitted across construction.
    pub max_rank_operations: usize,
    /// Maximum initial and candidate rank tests during construction.
    pub max_rank_tests: usize,
}

impl Default for IspCompletionLimits {
    fn default() -> Self {
        Self {
            family: IntegralFamilyLimits::default(),
            max_rank_matrix_entries: 16_000_000,
            max_rank_coefficient_terms: 64_000_000,
            max_rank_coefficient_bytes: 2 * 1024 * 1024 * 1024,
            max_rank_input_retained_bytes: DEFAULT_MAX_INPUT_RETAINED_BYTES,
            max_rank_output_retained_bytes: DEFAULT_MAX_OUTPUT_RETAINED_BYTES,
            max_rank_operations: 64_000_000,
            max_rank_tests: 65_536,
        }
    }
}

/// Work census retained with a completed family.
///
/// `rank_operations` counts checked exact arithmetic calls performed by
/// Symbolica's native matrix-rank implementation. Zero/one construction and
/// predicates are excluded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IspCompletionStats {
    pub(super) rank_tests: usize,
    pub(super) rank_operations: usize,
    pub(super) appended_isps: usize,
}

impl IspCompletionStats {
    pub const fn rank_tests(self) -> usize {
        self.rank_tests
    }

    pub const fn rank_operations(self) -> usize {
        self.rank_operations
    }

    pub const fn appended_isps(self) -> usize {
        self.appended_isps
    }
}
