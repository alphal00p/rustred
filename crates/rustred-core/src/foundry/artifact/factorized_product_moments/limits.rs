//! Explicit work bounds for one cold product-moment evaluation.

use crate::algebra::ExactAlgebraLimits;
use crate::reduction::ReductionLimits;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FactorizedProductMomentLimits {
    pub(crate) exact_algebra: ExactAlgebraLimits,
    pub(crate) dependency_reduction: ReductionLimits,
    pub(crate) max_arity: usize,
    pub(crate) max_polynomial_variables: usize,
    pub(crate) max_total_numerator_degree: usize,
    pub(crate) max_native_polynomial_terms: usize,
    pub(crate) max_native_polynomial_operations: usize,
    pub(crate) max_angular_degree: usize,
    pub(crate) max_angular_states: usize,
    pub(crate) max_angular_transitions: usize,
    pub(crate) max_pending_frames: usize,
    pub(crate) max_guards: usize,
    pub(crate) max_radial_power: usize,
    pub(crate) max_radial_states: usize,
    pub(crate) max_radial_summands: usize,
    pub(crate) max_dependency_requests: usize,
    pub(crate) max_coalescing_additions: usize,
    pub(crate) max_output_terms: usize,
    pub(crate) max_retained_coefficient_terms: usize,
    pub(crate) max_retained_coefficient_clone_owned_bytes: usize,
    pub(crate) max_exponent_entries: usize,
    pub(crate) max_exponent_bytes: usize,
    pub(crate) max_state_key_entries: usize,
    pub(crate) max_state_key_bytes: usize,
    pub(crate) max_guard_key_bytes: usize,
    /// Aggregate power entries retained by every prototype-owned
    /// `IntegralKey`, including radial-cache maps and returned clones.
    pub(crate) max_output_key_power_entries: usize,
    /// Aggregate owner-plus-power bytes retained by every prototype-owned
    /// `IntegralKey`.
    pub(crate) max_output_key_bytes: usize,
}

impl Default for FactorizedProductMomentLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            dependency_reduction: ReductionLimits::default(),
            max_arity: 128,
            max_polynomial_variables: 128,
            max_total_numerator_degree: 64,
            max_native_polynomial_terms: 4_000_000,
            max_native_polynomial_operations: 16_000_000,
            max_angular_degree: 64,
            max_angular_states: 1_000_000,
            max_angular_transitions: 4_000_000,
            max_pending_frames: 1_000_000,
            max_guards: 1_000_000,
            max_radial_power: 64,
            max_radial_states: 1_000_000,
            max_radial_summands: 4_000_000,
            max_dependency_requests: 1_000_000,
            max_coalescing_additions: 16_000_000,
            max_output_terms: 1_024,
            max_retained_coefficient_terms: 64_000_000,
            max_retained_coefficient_clone_owned_bytes: 1_000_000_000,
            max_exponent_entries: 64_000_000,
            max_exponent_bytes: 512_000_000,
            max_state_key_entries: 64_000_000,
            max_state_key_bytes: 512_000_000,
            max_guard_key_bytes: 64_000_000,
            max_output_key_power_entries: 16_384,
            max_output_key_bytes: 1_000_000,
        }
    }
}
