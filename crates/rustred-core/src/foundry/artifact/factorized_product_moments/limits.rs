//! Explicit bounds for cold product-program compilation and angular work.

use crate::algebra::ExactAlgebraLimits;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FactorizedProductMomentLimits {
    pub(crate) exact_algebra: ExactAlgebraLimits,
    pub(crate) max_arity: usize,
    pub(crate) max_polynomial_variables: usize,
    pub(crate) max_angular_degree: usize,
    pub(crate) max_angular_states: usize,
    pub(crate) max_angular_transitions: usize,
    pub(crate) max_pending_frames: usize,
    pub(crate) max_guards: usize,
    pub(crate) max_coalescing_additions: usize,
    pub(crate) max_retained_coefficient_terms: usize,
    pub(crate) max_retained_coefficient_clone_owned_bytes: usize,
    pub(crate) max_state_key_entries: usize,
    pub(crate) max_state_key_bytes: usize,
    pub(crate) max_guard_key_bytes: usize,
    /// Power entries in cold-validated master-embedding keys.
    pub(crate) max_compiled_embedding_key_power_entries: usize,
    /// Owner-plus-power bytes in cold-validated master-embedding keys.
    pub(crate) max_compiled_embedding_key_bytes: usize,
}

impl Default for FactorizedProductMomentLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_arity: 128,
            max_polynomial_variables: 128,
            max_angular_degree: 64,
            max_angular_states: 1_000_000,
            max_angular_transitions: 4_000_000,
            max_pending_frames: 1_000_000,
            max_guards: 1_000_000,
            max_coalescing_additions: 16_000_000,
            max_retained_coefficient_terms: 64_000_000,
            max_retained_coefficient_clone_owned_bytes: 1_000_000_000,
            max_state_key_entries: 64_000_000,
            max_state_key_bytes: 512_000_000,
            max_guard_key_bytes: 64_000_000,
            max_compiled_embedding_key_power_entries: 16_384,
            max_compiled_embedding_key_bytes: 1_000_000,
        }
    }
}
