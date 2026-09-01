//! Explicit retained-work bounds for one cold routing compilation.

use crate::algebra::ExactAlgebraLimits;

/// Resource policy around sign-gauge enumeration and the retained recurrence.
/// Matrix arithmetic remains subject to the authenticated family's own
/// Symbolica limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FactorizedNumeratorLiftLimits {
    pub(crate) max_arity: usize,
    pub(crate) max_sign_gauges: usize,
    pub(crate) max_recurrence_branches: usize,
}

impl Default for FactorizedNumeratorLiftLimits {
    fn default() -> Self {
        Self {
            // K=21 at six loops is comfortably inside this structural bound.
            max_arity: 128,
            // Row signs are a finite 2^L gauge portfolio.  This default admits
            // twelve loops while keeping compilation explicitly bounded.
            max_sign_gauges: 4_096,
            max_recurrence_branches: 129,
        }
    }
}

/// Resource policy for one cold, non-owning endpoint expansion.
///
/// These limits bound RustRed-observable retained support and structural work
/// before entering Symbolica's native sparse-polynomial power.  They are not
/// a numerator-rank restriction: width-one actions take a direct checked lane,
/// while wider actions report a typed resource failure when their exact
/// multinomial support cannot be materialized under the caller's policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FactorizedNumeratorLiftExpansionLimits {
    pub(crate) exact_algebra: ExactAlgebraLimits,
    pub(crate) max_endpoints: usize,
    /// Aggregate integral-power slots retained by endpoint keys.
    pub(crate) max_endpoint_power_entries: usize,
    /// Lower-bound bytes retained by endpoint key owners and their power
    /// payloads.  Map-node and allocator overhead remain implementation
    /// details; both exact caller-controlled dimensions are admitted here.
    pub(crate) max_retained_endpoint_key_bytes: usize,
    pub(crate) max_exponent_entries: usize,
    pub(crate) max_structural_term_operations: usize,
    /// Aggregate authenticated group routes examined when endpoint
    /// canonicalization is explicitly requested.
    pub(crate) max_canonicalization_routes: usize,
    /// Aggregate integral-power entries transported by those routes.
    pub(crate) max_canonicalization_power_entries: usize,
    /// Aggregate numerator-plus-denominator terms owned by live endpoint
    /// coefficients.  Canonicalization counts the borrowed input expansion
    /// together with its concurrently constructed output.
    pub(crate) max_retained_endpoint_coefficient_terms: usize,
    /// Aggregate clone-owned byte bound for those same live coefficients.
    pub(crate) max_retained_endpoint_coefficient_clone_owned_bytes: usize,
    /// Bound on Symbolica's public `Ring::pow` work for a nontrivial direct
    /// coefficient power. Unit and minus-unit coefficients bypass this work
    /// exactly.
    pub(crate) max_direct_coefficient_power: usize,
}

impl Default for FactorizedNumeratorLiftExpansionLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_endpoints: 4_000_000,
            max_endpoint_power_entries: 64_000_000,
            max_retained_endpoint_key_bytes: 1_000_000_000,
            max_exponent_entries: 64_000_000,
            max_structural_term_operations: 16_000_000,
            max_canonicalization_routes: 16_000_000,
            max_canonicalization_power_entries: 256_000_000,
            max_retained_endpoint_coefficient_terms: 64_000_000,
            max_retained_endpoint_coefficient_clone_owned_bytes: 1_000_000_000,
            max_direct_coefficient_power: 1_000_000,
        }
    }
}
