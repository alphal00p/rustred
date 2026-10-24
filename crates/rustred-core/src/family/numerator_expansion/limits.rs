//! Optional diagnostic thresholds for one cold multi-affine materialization.

use crate::algebra::ExactAlgebraLimits;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MultiAffineNumeratorExpansionLimits {
    /// Enforce count/work thresholds only for an explicitly requested diagnostic.
    pub diagnostics_enabled: bool,
    pub exact_algebra: ExactAlgebraLimits,
    pub max_factors: usize,
    pub max_relation_coefficient_entries: usize,
    pub max_total_power: u64,
    /// Conservative output support: the smaller of the prefix pair-product
    /// and total-degree/used-variable monomial bounds, before Symbolica
    /// coalesces collisions and cancellations. Multiplication work retains
    /// its separate pair-product bound.
    pub max_native_polynomial_terms: usize,
    pub max_native_polynomial_operations: usize,
    /// Peak conservative sparse exponent-row payload across native inputs and
    /// outputs. Every row has the parent-family denominator arity.
    pub max_native_exponent_entries: usize,
    pub max_endpoints: usize,
    pub max_endpoint_power_entries: usize,
    pub max_retained_endpoint_key_bytes: usize,
    pub max_retained_coefficient_terms: usize,
    pub max_retained_coefficient_clone_owned_bytes: usize,
}

impl MultiAffineNumeratorExpansionLimits {
    /// Explicit opt-in for trusted computation controlled by an external RAM
    /// guard instead of fixed aggregate expansion quotas. This is not the
    /// default for loading untrusted data or general library calls.
    ///
    /// All count/byte/operation allowances are opened together, so a larger
    /// exact routing expansion does not merely fail at the next default quota.
    /// Checked arithmetic, fallible allocation, exact coefficient validation,
    /// the native exponent representation and integral-power overflow checks
    /// remain enforced. This does not promise that every requested expansion
    /// fits available memory or can be preempted inside a Symbolica operation.
    pub const fn unbounded_resources() -> Self {
        Self {
            diagnostics_enabled: false,
            exact_algebra: ExactAlgebraLimits::unbounded_resources(),
            max_factors: usize::MAX,
            max_relation_coefficient_entries: usize::MAX,
            max_total_power: u64::MAX,
            max_native_polynomial_terms: usize::MAX,
            max_native_polynomial_operations: usize::MAX,
            max_native_exponent_entries: usize::MAX,
            max_endpoints: usize::MAX,
            max_endpoint_power_entries: usize::MAX,
            max_retained_endpoint_key_bytes: usize::MAX,
            max_retained_coefficient_terms: usize::MAX,
            max_retained_coefficient_clone_owned_bytes: usize::MAX,
        }
    }
}

impl Default for MultiAffineNumeratorExpansionLimits {
    fn default() -> Self {
        Self {
            diagnostics_enabled: false,
            exact_algebra: ExactAlgebraLimits::default(),
            max_factors: 128,
            max_relation_coefficient_entries: 16_384,
            max_total_power: 1_000_000,
            max_native_polynomial_terms: 4_000_000,
            max_native_polynomial_operations: 64_000_000,
            max_native_exponent_entries: 64_000_000,
            max_endpoints: 4_000_000,
            max_endpoint_power_entries: 64_000_000,
            max_retained_endpoint_key_bytes: 1_000_000_000,
            max_retained_coefficient_terms: 64_000_000,
            max_retained_coefficient_clone_owned_bytes: 1_000_000_000,
        }
    }
}

impl MultiAffineNumeratorExpansionLimits {
    /// Explicitly enable diagnostic count/work thresholds.
    pub fn with_diagnostics(mut self) -> Self {
        self.diagnostics_enabled = true;
        self.exact_algebra = self.exact_algebra.with_diagnostics();
        self
    }
}
