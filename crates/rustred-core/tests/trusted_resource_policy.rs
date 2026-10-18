//! These constructors are explicit opt-ins, not changes to loader defaults.
use rustred::{
    algebra::{ExactAlgebraLimits, IndexedAlgebraLimits, SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT},
    sector::symmetry::integral_transport::ExpansionLimits as MultiAffineNumeratorExpansionLimits,
};

#[test]
fn trusted_resource_policy_opens_all_transport_quotas_without_changing_defaults() {
    let defaults = MultiAffineNumeratorExpansionLimits::default();
    let limits = MultiAffineNumeratorExpansionLimits::unbounded_resources();
    assert_eq!(defaults.max_native_exponent_entries, 64_000_000);
    assert_eq!(
        limits.exact_algebra,
        ExactAlgebraLimits::unbounded_resources()
    );
    for field in [
        limits.max_factors,
        limits.max_relation_coefficient_entries,
        limits.max_native_polynomial_terms,
        limits.max_native_polynomial_operations,
        limits.max_native_exponent_entries,
        limits.max_endpoints,
        limits.max_endpoint_power_entries,
        limits.max_retained_endpoint_key_bytes,
        limits.max_retained_coefficient_terms,
        limits.max_retained_coefficient_clone_owned_bytes,
    ] {
        assert_eq!(field, usize::MAX);
    }
    assert_eq!(limits.max_total_power, u64::MAX);
    assert_eq!(
        limits.exact_algebra.max_exponent,
        SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT
    );
    assert_eq!(defaults, MultiAffineNumeratorExpansionLimits::default());
}

#[test]
fn trusted_algebra_policy_preserves_native_representation_boundaries() {
    let limits = IndexedAlgebraLimits::unbounded_resources();
    assert_eq!(limits.max_specialization_power_operations, usize::MAX);
    assert_eq!(limits.max_specialization_integer_bits, usize::MAX);
    assert_eq!(
        limits.exact_algebra.max_exponent,
        SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT
    );
    assert_eq!(limits.exact_algebra.max_polynomial_terms, usize::MAX);
    assert_eq!(limits.exact_algebra.max_term_operations, usize::MAX);
}
