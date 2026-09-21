//! Explicit caller policy for one cold multi-affine materialization.

use crate::algebra::ExactAlgebraLimits;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MultiAffineNumeratorExpansionLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_factors: usize,
    pub max_relation_coefficient_entries: usize,
    pub max_total_power: u64,
    /// Conservative product of the individual multinomial supports, before
    /// Symbolica coalesces collisions and cancellations.
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

impl Default for MultiAffineNumeratorExpansionLimits {
    fn default() -> Self {
        Self {
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
