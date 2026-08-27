//! Public proof-boundary checks for residual unit-affine composition.
//!
//! The raw plan and composition operations intentionally remain crate-private:
//! without an affine-locus-bound relation, their shifted integral keys cannot
//! soundly be interpreted as a global parametric identity.  Exhaustive native
//! composition tests therefore live beside `parametric_coefficient.rs`; this
//! external test authenticates only the stable public schema, limits/stats
//! vocabulary, typed errors, and compact provenance locators.  The future
//! affine-locus relation wrapper will be the first public algebraic consumer.

use rustred::{
    GuardOrigin, RESIDUAL_UNIT_AFFINE_COMPOSITION_V1_SCHEMA,
    ResidualUnitAffineCoefficientCompositionStats, ResidualUnitAffineCompositionError,
    ResidualUnitAffineCompositionPlanLimits, ResidualUnitAffinePolynomialCompositionLimits,
    ResidualUnitAffinePolynomialCompositionStats,
};

#[test]
fn public_schema_and_budget_vocabulary_are_stable_without_exporting_raw_composition() {
    assert_eq!(
        RESIDUAL_UNIT_AFFINE_COMPOSITION_V1_SCHEMA,
        "rustred-residual-unit-affine-composition-v1"
    );

    let plan = ResidualUnitAffineCompositionPlanLimits::default();
    let polynomial = ResidualUnitAffinePolynomialCompositionLimits::default();
    assert!(plan.max_full_images <= plan.max_variables);
    assert!(plan.max_total_image_terms > 0);
    assert!(polynomial.max_expanded_contributions > 0);
    assert!(polynomial.max_native_power_heap_pairs > 0);
    assert!(polynomial.max_native_integer_bit_work > 0);
    assert!(polynomial.max_integer_bit_work > 0);
    assert!(polynomial.max_normalization_input_term_pairs > 0);
    assert!(polynomial.max_guard_origin_retained_bytes > 0);

    let stats = ResidualUnitAffinePolynomialCompositionStats::default();
    assert_eq!(stats.source_terms(), 0);
    assert_eq!(stats.expanded_contribution_bound(), 0);
    assert_eq!(stats.output_terms(), 0);
    assert_eq!(stats.output_exponent_entry_bound(), 0);
    assert_eq!(stats.output_exponent_entries(), 0);
    assert_eq!(stats.native_integer_bit_work_bound(), 0);
    let coefficient_stats = ResidualUnitAffineCoefficientCompositionStats::default();
    assert_eq!(coefficient_stats.numerator(), stats);
    assert_eq!(coefficient_stats.denominator(), stats);
    assert_eq!(coefficient_stats.aggregate(), stats);
    assert_eq!(coefficient_stats.durable_guard_terms(), 0);
    assert_eq!(coefficient_stats.durable_guard_exponent_entries(), 0);
    assert_eq!(coefficient_stats.durable_guard_integer_bit_payload(), 0);
    assert_eq!(coefficient_stats.durable_guard_origin_retained_bytes(), 0);
    assert_eq!(coefficient_stats.total_integer_bit_work_bound(), 0);
    assert_eq!(coefficient_stats.normalization_input_term_pairs(), 0);

    assert_eq!(
        ResidualUnitAffineCompositionError::NonFreeIndexSurvived { position: 7 }.to_string(),
        "non-free index position 7 survived unit-affine composition"
    );
}

#[test]
fn affine_provenance_is_typed_and_has_streaming_stable_identity() {
    let substitution = GuardOrigin::ResidualUnitAffineIndexSubstitution {
        source_case: 11,
        predicate_ordinal: 5,
        bound_position: 2,
    };
    let denominator = GuardOrigin::CoefficientResidualUnitAffineSubstitutionDenominator {
        source_case: 11,
        predicate_ordinal: 5,
        bound_position: 2,
    };

    assert_eq!(
        substitution.stable_string(),
        "residual-unit-affine-index-substitution:11:5:2"
    );
    assert_eq!(
        denominator.stable_string(),
        "coefficient-residual-unit-affine-substitution-denominator:11:5:2"
    );
    assert_ne!(substitution, denominator);
}
