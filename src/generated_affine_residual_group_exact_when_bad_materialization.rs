//! Owning, non-publishing materialization of exact generated-affine `WhenBad` data.
//!
//! This current-lineage phase consumes only an authenticated
//! [`GeneratedAffineResidualGroupExactConditionPlan`].  It follows that
//! plan's source schedule exactly, maps every target premise, row guard, pivot
//! coefficient, and descending RHS coefficient through the sealed compact
//! target transform, and retains the complete mapped payload required by the
//! later relative-domain partitioner.  Coefficient normalization is never
//! allowed to erase substitution-domain evidence: both the normalized mapped
//! denominator and the pre-normalization mapped denominator are projected as
//! distinct physical-parameter identity sources.
//!
//! Exact inactive-orthant ranges remain lazy until their complete cardinality
//! has been admitted.  Their bounded materialization and affine-boundary
//! numerator decisions are delegated to the source-neutral Symbolica kernels
//! in `parametric_coefficient`; this module performs no polynomial algebra of
//! its own.  No outcome mutates the session, consumes a target, constructs a
//! relative partition, or publishes a rule.

use std::fmt;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::integer::Integer;

use crate::generated_affine_residual_group_exact_when_bad_conditions::{
    GeneratedAffineResidualGroupExactConditionHazardLocator,
    GeneratedAffineResidualGroupExactConditionPlan,
    GeneratedAffineResidualGroupExactConditionPlanError,
    GeneratedAffineResidualGroupExactConditionSourceLocator,
};
use crate::parametric_coefficient::{
    ParametricCoefficientValidationPayloadCensus, ParametricParameterIdentityClass,
    ParametricParameterIdentityProjection, ParametricParameterIdentityProjectionLimits,
    ParametricParameterIdentityProjectionStats, ParametricPolynomialValidationPayloadCensus,
    ResidualAffineBoundaryKernelError, ResidualAffineBoundaryKernelLimits,
    ResidualAffineBoundaryKernelStats, ResidualAffineBoundaryNumeratorDisposition,
    ResidualAffineBoundaryNumeratorLimits, ResidualAffineBoundaryNumeratorStats,
    ResidualAffineCoefficientComposition, ResidualAffineCoefficientCompositionPreflight,
    ResidualAffineMappedBoundaryClass, ResidualUnitAffineCoefficientCompositionStats,
    ResidualUnitAffineCompositionError, ResidualUnitAffinePolynomialCompositionLimits,
    ResidualUnitAffinePolynomialCompositionStats,
};
use crate::solver::exact_session::GeneratedAffineResidualGroupExactSession;
use crate::{
    IntegralFamily, ParametricCoefficient, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricPolynomial,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_WHEN_BAD_MATERIALIZATION_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-when-bad-materialization-v1";

#[cfg(test)]
std::thread_local! {
    static MATERIALIZATION_BOUNDARY_PANIC_FOR_TEST: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static MATERIALIZATION_PARTIAL_OWNERSHIP_PANIC_FOR_TEST: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static MATERIALIZATION_ADMISSIONS_RESERVE_OBSERVED_FOR_TEST: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn inject_materialization_boundary_panic_for_test() {
    MATERIALIZATION_BOUNDARY_PANIC_FOR_TEST.with(|panic_next| panic_next.set(true));
}

#[cfg(test)]
fn maybe_inject_materialization_boundary_panic_for_test() {
    MATERIALIZATION_BOUNDARY_PANIC_FOR_TEST.with(|panic_next| {
        if panic_next.replace(false) {
            panic!("injected exact WhenBad materialization boundary panic");
        }
    });
}

#[cfg(test)]
fn inject_materialization_partial_ownership_panic_for_test() {
    MATERIALIZATION_PARTIAL_OWNERSHIP_PANIC_FOR_TEST.with(|panic_next| panic_next.set(true));
}

#[cfg(test)]
fn maybe_inject_materialization_partial_ownership_panic_for_test(source_count: usize) {
    if source_count == 1 {
        MATERIALIZATION_PARTIAL_OWNERSHIP_PANIC_FOR_TEST.with(|panic_next| {
            if panic_next.replace(false) {
                panic!("injected exact WhenBad post-source-ownership panic");
            }
        });
    }
}

#[cfg(test)]
fn reset_materialization_admissions_reserve_observed_for_test() {
    MATERIALIZATION_ADMISSIONS_RESERVE_OBSERVED_FOR_TEST.with(|observed| observed.set(false));
}

#[cfg(test)]
fn mark_materialization_admissions_reserve_observed_for_test() {
    MATERIALIZATION_ADMISSIONS_RESERVE_OBSERVED_FOR_TEST.with(|observed| observed.set(true));
}

#[cfg(test)]
fn materialization_admissions_reserve_was_observed_for_test() -> bool {
    MATERIALIZATION_ADMISSIONS_RESERVE_OBSERVED_FOR_TEST.with(std::cell::Cell::get)
}

#[cfg(not(test))]
fn maybe_inject_materialization_boundary_panic_for_test() {}

#[cfg(not(test))]
fn maybe_inject_materialization_partial_ownership_panic_for_test(_source_count: usize) {}

#[cfg(not(test))]
fn mark_materialization_admissions_reserve_observed_for_test() {}

/// Per-attempt limits for newly created materialization payload only.
///
/// The already-live condition-plan/Ready graph is deliberately excluded.
/// Every newly retained mapped coefficient, polynomial, projection, boundary
/// value, event slot, and the largest current child scratch envelope is
/// included.  Child limits are intersected with the remaining aggregate
/// allowance; they are never reset to their defaults for each source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadMaterializationLimits {
    pub(crate) polynomial_composition: ResidualUnitAffinePolynomialCompositionLimits,
    pub(crate) parameter_identity: ParametricParameterIdentityProjectionLimits,
    pub(crate) boundary_mapping: ResidualAffineBoundaryKernelLimits,
    pub(crate) boundary_numerator: ResidualAffineBoundaryNumeratorLimits,
    pub(crate) max_source_records: usize,
    pub(crate) max_condition_records: usize,
    pub(crate) max_coefficient_records: usize,
    pub(crate) max_denominator_identity_sources: usize,
    pub(crate) max_denominator_identity_loci: usize,
    pub(crate) max_hazard_ranges: usize,
    pub(crate) max_boundary_values: usize,
    pub(crate) max_boundary_value_integer_bits: usize,
    pub(crate) max_boundary_value_retained_logical_bytes: usize,
    pub(crate) max_boundary_enumeration_integer_bit_work: usize,
    pub(crate) max_mapping_source_terms: usize,
    pub(crate) max_mapping_source_exponent_entries: usize,
    pub(crate) max_mapping_source_integer_bits: usize,
    pub(crate) max_mapping_admitted_retained_byte_bound: usize,
    pub(crate) max_mapping_admission_temporary_byte_peak: usize,
    pub(crate) max_mapping_expanded_contribution_bound: usize,
    pub(crate) max_mapping_output_exponent_entry_bound: usize,
    pub(crate) max_mapping_power_calls: usize,
    pub(crate) max_mapping_native_power_heap_pair_bound: usize,
    pub(crate) max_mapping_multiplication_term_pair_bound: usize,
    pub(crate) max_mapping_addition_term_visit_bound: usize,
    pub(crate) max_mapping_native_integer_bit_work_bound: usize,
    pub(crate) max_mapping_integer_bit_work_bound: usize,
    pub(crate) max_mapping_normalization_input_term_pairs: usize,
    pub(crate) max_projection_source_terms: usize,
    pub(crate) max_projection_source_exponent_entries: usize,
    pub(crate) max_projection_source_integer_bits: usize,
    pub(crate) max_projection_native_workspace_byte_envelope: usize,
    pub(crate) max_projection_retained_output_byte_bound: usize,
    pub(crate) max_projection_temporary_byte_envelope: usize,
    pub(crate) max_boundary_mapping_constructed_terms: usize,
    pub(crate) max_boundary_mapping_constructed_exponent_entries: usize,
    pub(crate) max_boundary_mapping_constructed_integer_bits: usize,
    pub(crate) max_boundary_mapping_mapped_term_bound: usize,
    pub(crate) max_boundary_mapping_mapped_exponent_entry_bound: usize,
    pub(crate) max_boundary_mapping_mapped_integer_bit_bound: usize,
    pub(crate) max_boundary_mapping_affine_term_visits: usize,
    pub(crate) max_boundary_mapping_affine_exponent_visits: usize,
    pub(crate) max_boundary_mapping_retained_output_byte_bound: usize,
    pub(crate) max_boundary_mapping_constructed_source_temporary_byte_peak: usize,
    pub(crate) max_boundary_mapping_child_compilation_byte_peak: usize,
    pub(crate) max_boundary_numerator_boundary_terms: usize,
    pub(crate) max_boundary_numerator_boundary_exponent_entries: usize,
    pub(crate) max_boundary_numerator_boundary_integer_bits: usize,
    pub(crate) max_boundary_numerator_numerator_terms: usize,
    pub(crate) max_boundary_numerator_numerator_exponent_entries: usize,
    pub(crate) max_boundary_numerator_numerator_integer_bits: usize,
    pub(crate) max_boundary_numerator_affine_term_visits: usize,
    pub(crate) max_boundary_numerator_affine_exponent_visits: usize,
    pub(crate) max_boundary_numerator_divisibility_term_pairs: usize,
    pub(crate) max_boundary_numerator_divisibility_calls: usize,
    pub(crate) max_boundary_numerator_source_copy_temporary_byte_peak: usize,
    pub(crate) max_boundary_numerator_retained_owned_logical_bytes: usize,
    pub(crate) max_retained_owned_logical_bytes: usize,
    pub(crate) max_compilation_owned_logical_peak_upper_bound: usize,
}

impl Default for GeneratedAffineResidualGroupExactWhenBadMaterializationLimits {
    fn default() -> Self {
        const LARGE: usize = 64_000_000_000;
        const HUGE: usize = usize::MAX;
        Self {
            polynomial_composition: ResidualUnitAffinePolynomialCompositionLimits::default(),
            parameter_identity: ParametricParameterIdentityProjectionLimits::default(),
            boundary_mapping: ResidualAffineBoundaryKernelLimits::default(),
            boundary_numerator: ResidualAffineBoundaryNumeratorLimits::default(),
            max_source_records: 48_000_000,
            max_condition_records: 32_000_000,
            max_coefficient_records: 16_000_000,
            max_denominator_identity_sources: 32_000_000,
            max_denominator_identity_loci: 256_000_000,
            max_hazard_ranges: LARGE,
            max_boundary_values: 256_000_000,
            max_boundary_value_integer_bits: HUGE,
            max_boundary_value_retained_logical_bytes: HUGE,
            max_boundary_enumeration_integer_bit_work: HUGE,
            max_mapping_source_terms: HUGE,
            max_mapping_source_exponent_entries: HUGE,
            max_mapping_source_integer_bits: HUGE,
            max_mapping_admitted_retained_byte_bound: HUGE,
            max_mapping_admission_temporary_byte_peak: HUGE,
            max_mapping_expanded_contribution_bound: HUGE,
            max_mapping_output_exponent_entry_bound: HUGE,
            max_mapping_power_calls: HUGE,
            max_mapping_native_power_heap_pair_bound: HUGE,
            max_mapping_multiplication_term_pair_bound: HUGE,
            max_mapping_addition_term_visit_bound: HUGE,
            max_mapping_native_integer_bit_work_bound: HUGE,
            max_mapping_integer_bit_work_bound: HUGE,
            max_mapping_normalization_input_term_pairs: HUGE,
            max_projection_source_terms: HUGE,
            max_projection_source_exponent_entries: HUGE,
            max_projection_source_integer_bits: HUGE,
            max_projection_native_workspace_byte_envelope: HUGE,
            max_projection_retained_output_byte_bound: HUGE,
            max_projection_temporary_byte_envelope: HUGE,
            max_boundary_mapping_constructed_terms: HUGE,
            max_boundary_mapping_constructed_exponent_entries: HUGE,
            max_boundary_mapping_constructed_integer_bits: HUGE,
            max_boundary_mapping_mapped_term_bound: HUGE,
            max_boundary_mapping_mapped_exponent_entry_bound: HUGE,
            max_boundary_mapping_mapped_integer_bit_bound: HUGE,
            max_boundary_mapping_affine_term_visits: HUGE,
            max_boundary_mapping_affine_exponent_visits: HUGE,
            max_boundary_mapping_retained_output_byte_bound: HUGE,
            max_boundary_mapping_constructed_source_temporary_byte_peak: HUGE,
            max_boundary_mapping_child_compilation_byte_peak: HUGE,
            max_boundary_numerator_boundary_terms: HUGE,
            max_boundary_numerator_boundary_exponent_entries: HUGE,
            max_boundary_numerator_boundary_integer_bits: HUGE,
            max_boundary_numerator_numerator_terms: HUGE,
            max_boundary_numerator_numerator_exponent_entries: HUGE,
            max_boundary_numerator_numerator_integer_bits: HUGE,
            max_boundary_numerator_affine_term_visits: HUGE,
            max_boundary_numerator_affine_exponent_visits: HUGE,
            max_boundary_numerator_divisibility_term_pairs: HUGE,
            max_boundary_numerator_divisibility_calls: HUGE,
            max_boundary_numerator_source_copy_temporary_byte_peak: HUGE,
            max_boundary_numerator_retained_owned_logical_bytes: HUGE,
            max_retained_owned_logical_bytes: HUGE,
            max_compilation_owned_logical_peak_upper_bound: HUGE,
        }
    }
}

/// Aggregate prospective Symbolica mapping work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadMappingStats {
    source_terms: usize,
    source_exponent_entries: usize,
    source_integer_bits: usize,
    admitted_retained_byte_bound: usize,
    /// Conservative prospective live payload peak while executing the source
    /// schedule.  This is cumulative mapped retained output through the
    /// current source plus its separately owned normalized-denominator copy;
    /// it is deliberately not an observed allocation count.
    admission_temporary_byte_peak: usize,
    expanded_contribution_bound: usize,
    output_exponent_entry_bound: usize,
    power_calls: usize,
    native_power_heap_pair_bound: usize,
    multiplication_term_pair_bound: usize,
    addition_term_visit_bound: usize,
    native_integer_bit_work_bound: usize,
    integer_bit_work_bound: usize,
    normalization_input_term_pairs: usize,
}

macro_rules! mapping_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualGroupExactWhenBadMappingStats {
    mapping_stats_getters!(
        source_terms,
        source_exponent_entries,
        source_integer_bits,
        admitted_retained_byte_bound,
        admission_temporary_byte_peak,
        expanded_contribution_bound,
        output_exponent_entry_bound,
        power_calls,
        native_power_heap_pair_bound,
        multiplication_term_pair_bound,
        addition_term_visit_bound,
        native_integer_bit_work_bound,
        integer_bit_work_bound,
        normalization_input_term_pairs,
    );
}

/// Aggregate physical-parameter projection work and durable bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadProjectionStats {
    sources: usize,
    source_terms: usize,
    source_exponent_entries: usize,
    source_integer_bits: usize,
    native_workspace_byte_envelope: usize,
    retained_output_byte_bound: usize,
    temporary_byte_envelope: usize,
    projected_physical_monomials: usize,
    conditional_loci: usize,
}

impl GeneratedAffineResidualGroupExactWhenBadProjectionStats {
    mapping_stats_getters!(
        sources,
        source_terms,
        source_exponent_entries,
        source_integer_bits,
        native_workspace_byte_envelope,
        retained_output_byte_bound,
        temporary_byte_envelope,
        projected_physical_monomials,
        conditional_loci,
    );
}

/// Aggregate exact-value, mapping, and numerator-restriction census for all
/// materialized hazard values.  Every field is summed across the complete
/// event stream; no child allowance is reset per boundary.  The three value
/// and enumeration fields are conservative endpoint-derived admissions, not
/// observed allocation counters; the single later materialization walk
/// authenticates its observed totals against them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadBoundaryStats {
    value_integer_bits: usize,
    value_retained_logical_bytes: usize,
    enumeration_integer_bit_work: usize,
    mapping_constructed_terms: usize,
    mapping_constructed_exponent_entries: usize,
    mapping_constructed_integer_bits: usize,
    mapping_mapped_term_bound: usize,
    mapping_mapped_exponent_entry_bound: usize,
    mapping_mapped_integer_bit_bound: usize,
    mapping_affine_term_visits: usize,
    mapping_affine_exponent_visits: usize,
    mapping_retained_output_byte_bound: usize,
    mapping_constructed_source_temporary_byte_peak: usize,
    mapping_child_compilation_byte_peak: usize,
    numerator_boundary_terms: usize,
    numerator_boundary_exponent_entries: usize,
    numerator_boundary_integer_bits: usize,
    numerator_numerator_terms: usize,
    numerator_numerator_exponent_entries: usize,
    numerator_numerator_integer_bits: usize,
    numerator_affine_term_visits: usize,
    numerator_affine_exponent_visits: usize,
    numerator_divisibility_term_pairs: usize,
    numerator_divisibility_calls: usize,
    numerator_source_copy_temporary_byte_peak: usize,
    numerator_retained_owned_logical_bytes: usize,
}

impl GeneratedAffineResidualGroupExactWhenBadBoundaryStats {
    mapping_stats_getters!(
        value_integer_bits,
        value_retained_logical_bytes,
        enumeration_integer_bit_work,
        mapping_constructed_terms,
        mapping_constructed_exponent_entries,
        mapping_constructed_integer_bits,
        mapping_mapped_term_bound,
        mapping_mapped_exponent_entry_bound,
        mapping_mapped_integer_bit_bound,
        mapping_affine_term_visits,
        mapping_affine_exponent_visits,
        mapping_retained_output_byte_bound,
        mapping_constructed_source_temporary_byte_peak,
        mapping_child_compilation_byte_peak,
        numerator_boundary_terms,
        numerator_boundary_exponent_entries,
        numerator_boundary_integer_bits,
        numerator_numerator_terms,
        numerator_numerator_exponent_entries,
        numerator_numerator_integer_bits,
        numerator_affine_term_visits,
        numerator_affine_exponent_visits,
        numerator_divisibility_term_pairs,
        numerator_divisibility_calls,
        numerator_source_copy_temporary_byte_peak,
        numerator_retained_owned_logical_bytes,
    );
}

/// Deterministic census for one complete materialization transcript.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadMaterializationStats {
    source_records: usize,
    condition_records: usize,
    coefficient_records: usize,
    denominator_identity_sources: usize,
    denominator_identity_loci: usize,
    hazard_ranges: usize,
    admitted_boundary_values: usize,
    boundary_values: usize,
    empty_boundaries: usize,
    whole_target_boundaries: usize,
    suppressed_boundaries: usize,
    retained_boundaries: usize,
    mapping: GeneratedAffineResidualGroupExactWhenBadMappingStats,
    projection: GeneratedAffineResidualGroupExactWhenBadProjectionStats,
    boundary: GeneratedAffineResidualGroupExactWhenBadBoundaryStats,
    source_phase_retained_logical_byte_bound: usize,
    /// Admissions and coefficient-source lookup arenas currently owned by
    /// the materialization transaction but absent from the durable result.
    active_transaction_arena_byte_bound: usize,
    /// Private scoped scratch kept nonzero only while a normalized
    /// denominator copy is live across its two identity projections.
    active_normalized_denominator_temporary_byte_bound: usize,
    retained_owned_logical_bytes: usize,
    compilation_owned_logical_peak_upper_bound: usize,
}

impl GeneratedAffineResidualGroupExactWhenBadMaterializationStats {
    mapping_stats_getters!(
        source_records,
        condition_records,
        coefficient_records,
        denominator_identity_sources,
        denominator_identity_loci,
        hazard_ranges,
        admitted_boundary_values,
        boundary_values,
        empty_boundaries,
        whole_target_boundaries,
        suppressed_boundaries,
        retained_boundaries,
        source_phase_retained_logical_byte_bound,
        retained_owned_logical_bytes,
        compilation_owned_logical_peak_upper_bound,
    );

    pub(crate) const fn mapping(self) -> GeneratedAffineResidualGroupExactWhenBadMappingStats {
        self.mapping
    }

    pub(crate) const fn projection(
        self,
    ) -> GeneratedAffineResidualGroupExactWhenBadProjectionStats {
        self.projection
    }

    pub(crate) const fn boundary(self) -> GeneratedAffineResidualGroupExactWhenBadBoundaryStats {
        self.boundary
    }
}

/// Classification of one mapped nonzero target premise or candidate guard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactMappedConditionClass {
    DischargedNonzeroIntegerConstant,
    DischargedCoefficientFieldUnit,
    BaseParameterAssumption,
    IndexDependent,
}

/// Stable mapping census retained beside every mapped schedule source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactSourceMappingStats {
    IdentityPolynomial(ParametricPolynomialValidationPayloadCensus),
    IdentityCoefficient(ParametricCoefficientValidationPayloadCensus),
    CompactPolynomial(ResidualUnitAffinePolynomialCompositionStats),
    CompactCoefficient(ResidualUnitAffineCoefficientCompositionStats),
}

/// One mapped target premise or recentered candidate guard.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactMappedCondition {
    source: GeneratedAffineResidualGroupExactConditionSourceLocator,
    class: GeneratedAffineResidualGroupExactMappedConditionClass,
    polynomial: ParametricPolynomial,
    mapping: GeneratedAffineResidualGroupExactSourceMappingStats,
}

impl GeneratedAffineResidualGroupExactMappedCondition {
    pub(crate) const fn source(&self) -> GeneratedAffineResidualGroupExactConditionSourceLocator {
        self.source
    }

    pub(crate) const fn class(&self) -> GeneratedAffineResidualGroupExactMappedConditionClass {
        self.class
    }

    pub(crate) const fn polynomial(&self) -> &ParametricPolynomial {
        &self.polynomial
    }

    pub(crate) const fn mapping(&self) -> GeneratedAffineResidualGroupExactSourceMappingStats {
        self.mapping
    }
}

/// The two deliberately distinct denominator-identity source roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactDenominatorIdentityKind {
    PreNormalizationMappedDenominator,
    NormalizedMappedDenominator,
}

/// One complete physical-parameter identity projection with source role.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactDenominatorIdentity {
    kind: GeneratedAffineResidualGroupExactDenominatorIdentityKind,
    projection: ParametricParameterIdentityProjection,
}

impl GeneratedAffineResidualGroupExactDenominatorIdentity {
    pub(crate) const fn kind(&self) -> GeneratedAffineResidualGroupExactDenominatorIdentityKind {
        self.kind
    }

    pub(crate) const fn projection(&self) -> &ParametricParameterIdentityProjection {
        &self.projection
    }
}

/// One available mapped pivot or RHS coefficient.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactMappedCoefficient {
    source: GeneratedAffineResidualGroupExactConditionSourceLocator,
    normalized_value: ParametricCoefficient,
    normalized_numerator: ParametricPolynomial,
    pre_normalization_mapped_denominator: ParametricPolynomial,
    denominator_identities: [GeneratedAffineResidualGroupExactDenominatorIdentity; 2],
    mapping: GeneratedAffineResidualGroupExactSourceMappingStats,
}

impl GeneratedAffineResidualGroupExactMappedCoefficient {
    pub(crate) const fn source(&self) -> GeneratedAffineResidualGroupExactConditionSourceLocator {
        self.source
    }

    pub(crate) const fn normalized_value(&self) -> &ParametricCoefficient {
        &self.normalized_value
    }

    pub(crate) const fn normalized_numerator(&self) -> &ParametricPolynomial {
        &self.normalized_numerator
    }

    pub(crate) const fn pre_normalization_mapped_denominator(&self) -> &ParametricPolynomial {
        &self.pre_normalization_mapped_denominator
    }

    pub(crate) const fn denominator_identities(
        &self,
    ) -> &[GeneratedAffineResidualGroupExactDenominatorIdentity; 2] {
        &self.denominator_identities
    }

    pub(crate) const fn mapping(&self) -> GeneratedAffineResidualGroupExactSourceMappingStats {
        self.mapping
    }
}

/// One materialized schedule record, in exact condition-plan order.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactMappedSource {
    Condition(GeneratedAffineResidualGroupExactMappedCondition),
    Coefficient(GeneratedAffineResidualGroupExactMappedCoefficient),
}

impl GeneratedAffineResidualGroupExactMappedSource {
    pub(crate) const fn source(&self) -> GeneratedAffineResidualGroupExactConditionSourceLocator {
        match self {
            Self::Condition(value) => value.source,
            Self::Coefficient(value) => value.source,
        }
    }

    pub(crate) const fn coefficient(
        &self,
    ) -> Option<&GeneratedAffineResidualGroupExactMappedCoefficient> {
        match self {
            Self::Coefficient(value) => Some(value),
            Self::Condition(_) => None,
        }
    }
}

/// Exact outcome of specializing one mapped numerator on one affine boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactBoundaryDisposition {
    Empty,
    WholeTarget,
    SuppressedByNumerator,
    RetainedBadBoundary,
}

/// One bounded exact hazard value and its complete Symbolica decision.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactBoundaryEvent {
    ordinal: usize,
    source: GeneratedAffineResidualGroupExactConditionHazardLocator,
    value: Integer,
    disposition: GeneratedAffineResidualGroupExactBoundaryDisposition,
    boundary: Option<ParametricPolynomial>,
    mapping_stats: ResidualAffineBoundaryKernelStats,
    numerator_stats: Option<ResidualAffineBoundaryNumeratorStats>,
}

impl GeneratedAffineResidualGroupExactBoundaryEvent {
    pub(crate) const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub(crate) const fn source(&self) -> GeneratedAffineResidualGroupExactConditionHazardLocator {
        self.source
    }

    pub(crate) const fn disposition(&self) -> GeneratedAffineResidualGroupExactBoundaryDisposition {
        self.disposition
    }

    pub(crate) const fn value(&self) -> &Integer {
        &self.value
    }

    pub(crate) const fn boundary(&self) -> Option<&ParametricPolynomial> {
        self.boundary.as_ref()
    }

    pub(crate) const fn mapping_stats(&self) -> ResidualAffineBoundaryKernelStats {
        self.mapping_stats
    }

    pub(crate) const fn numerator_stats(&self) -> Option<ResidualAffineBoundaryNumeratorStats> {
        self.numerator_stats
    }
}

/// Complete semantic witness for an all-domain bad candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactWhenBadIdenticallyBadReason {
    CandidateGuardMappedToZero {
        source: GeneratedAffineResidualGroupExactConditionSourceLocator,
    },
    ZeroMappedDenominator {
        source: GeneratedAffineResidualGroupExactConditionSourceLocator,
    },
    WholeTargetInactiveActivation {
        source: GeneratedAffineResidualGroupExactConditionHazardLocator,
        boundary_ordinal: usize,
    },
}

/// The decisive payload is owned, not summarized to an ordinal.  A zero guard
/// retains its mapped polynomial, while a whole-target hazard retains the exact
/// arbitrary-width value and boundary-kernel transcript that proved the result.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness {
    CandidateGuardMappedToZero {
        source: GeneratedAffineResidualGroupExactConditionSourceLocator,
        polynomial: ParametricPolynomial,
        mapping: GeneratedAffineResidualGroupExactSourceMappingStats,
    },
    ZeroMappedDenominator {
        source: GeneratedAffineResidualGroupExactConditionSourceLocator,
        mapping: ResidualUnitAffineCoefficientCompositionStats,
    },
    WholeTargetInactiveActivation {
        event: GeneratedAffineResidualGroupExactBoundaryEvent,
    },
}

impl GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness {
    pub(crate) const fn reason(
        &self,
    ) -> GeneratedAffineResidualGroupExactWhenBadIdenticallyBadReason {
        match self {
            Self::CandidateGuardMappedToZero { source, .. } => {
                GeneratedAffineResidualGroupExactWhenBadIdenticallyBadReason::CandidateGuardMappedToZero {
                    source: *source,
                }
            }
            Self::ZeroMappedDenominator { source, .. } => {
                GeneratedAffineResidualGroupExactWhenBadIdenticallyBadReason::ZeroMappedDenominator {
                    source: *source,
                }
            }
            Self::WholeTargetInactiveActivation { event } => {
                GeneratedAffineResidualGroupExactWhenBadIdenticallyBadReason::WholeTargetInactiveActivation {
                    source: event.source,
                    boundary_ordinal: event.ordinal,
                }
            }
        }
    }
}

/// Successful mapped payload awaiting arbitrary-width formula interning and
/// the source-neutral relative partition compiler.
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadReadyForPartition {
    schema: &'static str,
    plan: GeneratedAffineResidualGroupExactConditionPlan,
    sources: Vec<GeneratedAffineResidualGroupExactMappedSource>,
    boundaries: Vec<GeneratedAffineResidualGroupExactBoundaryEvent>,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    stats: GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
}

impl GeneratedAffineResidualGroupExactWhenBadReadyForPartition {
    pub(crate) const fn condition_plan(&self) -> &GeneratedAffineResidualGroupExactConditionPlan {
        &self.plan
    }

    pub(crate) fn sources(&self) -> &[GeneratedAffineResidualGroupExactMappedSource] {
        &self.sources
    }

    pub(crate) fn boundaries(&self) -> &[GeneratedAffineResidualGroupExactBoundaryEvent] {
        &self.boundaries
    }

    pub(crate) const fn stats(
        &self,
    ) -> GeneratedAffineResidualGroupExactWhenBadMaterializationStats {
        self.stats
    }

    pub(crate) const fn limits(
        &self,
    ) -> GeneratedAffineResidualGroupExactWhenBadMaterializationLimits {
        self.limits
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    /// Drop materialization-only evidence after its conditions have been
    /// compiled into the source-neutral application partition.
    pub(crate) fn into_condition_plan_for_publication(
        self,
    ) -> GeneratedAffineResidualGroupExactConditionPlan {
        self.plan
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        session: &GeneratedAffineResidualGroupExactSession,
    ) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
        replay_materialization(
            self.schema,
            &self.plan,
            family,
            context,
            session,
            self.limits,
            &self.sources,
            &self.boundaries,
            None,
            self.stats,
        )
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactWhenBadReadyForPartition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactWhenBadReadyForPartition")
            .field("schema", &self.schema)
            .field("source_records", &self.sources.len())
            .field("boundary_events", &self.boundaries.len())
            .field("stats", &self.stats)
            .field("targets_consumed", &0)
            .field("publishes_rule", &false)
            .field("private_plan", &"<redacted>")
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

/// Authenticated all-domain bad materialization.  The exact plan and complete
/// prefix through the decisive witness remain owned for replay and later
/// non-publishing disposition.
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadIdenticallyBad {
    schema: &'static str,
    plan: GeneratedAffineResidualGroupExactConditionPlan,
    sources: Vec<GeneratedAffineResidualGroupExactMappedSource>,
    boundaries: Vec<GeneratedAffineResidualGroupExactBoundaryEvent>,
    witness: GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    stats: GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
}

impl GeneratedAffineResidualGroupExactWhenBadIdenticallyBad {
    pub(crate) const fn reason(
        &self,
    ) -> GeneratedAffineResidualGroupExactWhenBadIdenticallyBadReason {
        self.witness.reason()
    }

    pub(crate) const fn witness(
        &self,
    ) -> &GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness {
        &self.witness
    }

    pub(crate) fn sources(&self) -> &[GeneratedAffineResidualGroupExactMappedSource] {
        &self.sources
    }

    pub(crate) fn boundaries(&self) -> &[GeneratedAffineResidualGroupExactBoundaryEvent] {
        &self.boundaries
    }

    pub(crate) const fn stats(
        &self,
    ) -> GeneratedAffineResidualGroupExactWhenBadMaterializationStats {
        self.stats
    }

    pub(crate) const fn limits(
        &self,
    ) -> GeneratedAffineResidualGroupExactWhenBadMaterializationLimits {
        self.limits
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    /// Drop the materialization transcript only after its all-domain badness
    /// has been replayed by the caller. The original sealed Ready owner is the
    /// sole operational capability carried into the non-publishing rejected
    /// candidate transition.
    pub(crate) fn into_condition_plan_for_rejection(
        self,
    ) -> GeneratedAffineResidualGroupExactConditionPlan {
        self.plan
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        session: &GeneratedAffineResidualGroupExactSession,
    ) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
        replay_materialization(
            self.schema,
            &self.plan,
            family,
            context,
            session,
            self.limits,
            &self.sources,
            &self.boundaries,
            Some(&self.witness),
            self.stats,
        )
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactWhenBadIdenticallyBad {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactWhenBadIdenticallyBad")
            .field("schema", &self.schema)
            .field("source_records", &self.sources.len())
            .field("boundary_events", &self.boundaries.len())
            .field("reason", &self.witness.reason())
            .field("stats", &self.stats)
            .field("targets_consumed", &0)
            .field("publishes_rule", &false)
            .field("private_plan", &"<redacted>")
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

/// Non-publishing semantic outcome of this first owning Phase C slice.
pub(crate) enum GeneratedAffineResidualGroupExactWhenBadMaterialization {
    ReadyForPartition(GeneratedAffineResidualGroupExactWhenBadReadyForPartition),
    IdenticallyBad(GeneratedAffineResidualGroupExactWhenBadIdenticallyBad),
}

impl GeneratedAffineResidualGroupExactWhenBadMaterialization {
    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    pub(crate) const fn stats(
        &self,
    ) -> GeneratedAffineResidualGroupExactWhenBadMaterializationStats {
        match self {
            Self::ReadyForPartition(value) => value.stats,
            Self::IdenticallyBad(value) => value.stats,
        }
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        session: &GeneratedAffineResidualGroupExactSession,
    ) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
        match self {
            Self::ReadyForPartition(value) => value.replay(family, context, session),
            Self::IdenticallyBad(value) => value.replay(family, context, session),
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactWhenBadMaterialization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadyForPartition(value) => value.fmt(formatter),
            Self::IdenticallyBad(value) => value.fmt(formatter),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactWhenBadMaterializationError {
    ConditionPlan(GeneratedAffineResidualGroupExactConditionPlanError),
    Composition(ResidualUnitAffineCompositionError),
    Coefficient(ParametricCoefficientError),
    Boundary(ResidualAffineBoundaryKernelError),
    SchemaMismatch,
    ReplayMismatch,
    MalformedPlan,
    IdenticallyZeroTargetPremise {
        source: GeneratedAffineResidualGroupExactConditionSourceLocator,
    },
    DenominatorProjectionInvariantViolation {
        source: GeneratedAffineResidualGroupExactConditionSourceLocator,
        kind: GeneratedAffineResidualGroupExactDenominatorIdentityKind,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ExactIntegerResourceLimit {
        resource: &'static str,
        requested: Integer,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    SymbolicaPanic,
}

impl fmt::Display for GeneratedAffineResidualGroupExactWhenBadMaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConditionPlan(error) => error.fmt(formatter),
            Self::Composition(error) => error.fmt(formatter),
            Self::Coefficient(error) => error.fmt(formatter),
            Self::Boundary(error) => error.fmt(formatter),
            Self::SchemaMismatch => {
                formatter.write_str("exact WhenBad materialization schema mismatch")
            }
            Self::ReplayMismatch => {
                formatter.write_str("exact WhenBad materialization replay mismatch")
            }
            Self::MalformedPlan => formatter.write_str("exact WhenBad condition plan is malformed"),
            Self::IdenticallyZeroTargetPremise { .. } => {
                formatter.write_str("authenticated target premise mapped identically to zero")
            }
            Self::DenominatorProjectionInvariantViolation { .. } => formatter.write_str(
                "authenticated nonzero denominator projected to an all-domain zero identity",
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested}, configured limit is {limit}",
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ExactIntegerResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested exact cardinality {requested}, configured limit is {limit}",
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "{resource} allocation of {requested} entries failed after bounded preflight",
            ),
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked inside exact WhenBad materialization")
            }
        }
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactWhenBadMaterializationError {}

impl From<GeneratedAffineResidualGroupExactConditionPlanError>
    for GeneratedAffineResidualGroupExactWhenBadMaterializationError
{
    fn from(error: GeneratedAffineResidualGroupExactConditionPlanError) -> Self {
        Self::ConditionPlan(error)
    }
}

impl From<ResidualUnitAffineCompositionError>
    for GeneratedAffineResidualGroupExactWhenBadMaterializationError
{
    fn from(error: ResidualUnitAffineCompositionError) -> Self {
        Self::Composition(error)
    }
}

impl From<ParametricCoefficientError>
    for GeneratedAffineResidualGroupExactWhenBadMaterializationError
{
    fn from(error: ParametricCoefficientError) -> Self {
        Self::Coefficient(error)
    }
}

impl From<ResidualAffineBoundaryKernelError>
    for GeneratedAffineResidualGroupExactWhenBadMaterializationError
{
    fn from(error: ResidualAffineBoundaryKernelError) -> Self {
        Self::Boundary(error)
    }
}

/// Recoverable operational failure retaining the exact non-Clone input plan.
pub(crate) struct GeneratedAffineResidualGroupExactWhenBadMaterializationFailure {
    error: GeneratedAffineResidualGroupExactWhenBadMaterializationError,
    plan: GeneratedAffineResidualGroupExactConditionPlan,
}

impl GeneratedAffineResidualGroupExactWhenBadMaterializationFailure {
    pub(crate) const fn error(
        &self,
    ) -> &GeneratedAffineResidualGroupExactWhenBadMaterializationError {
        &self.error
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        GeneratedAffineResidualGroupExactWhenBadMaterializationError,
        GeneratedAffineResidualGroupExactConditionPlan,
    ) {
        (self.error, self.plan)
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactWhenBadMaterializationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactWhenBadMaterializationFailure")
            .field("error", &self.error)
            .field("private_plan", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactWhenBadMaterializationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactWhenBadMaterializationFailure {}

pub(crate) struct GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler;

impl GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler {
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        session: &GeneratedAffineResidualGroupExactSession,
        plan: GeneratedAffineResidualGroupExactConditionPlan,
        limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    ) -> Result<
        GeneratedAffineResidualGroupExactWhenBadMaterialization,
        GeneratedAffineResidualGroupExactWhenBadMaterializationFailure,
    > {
        let prepared = catch_unwind(AssertUnwindSafe(|| {
            prepare_materialization(family, context, session, &plan, limits)
        }));
        match prepared {
            Ok(Ok(PreparedMaterialization::Ready {
                sources,
                boundaries,
                stats,
            })) => Ok(
                GeneratedAffineResidualGroupExactWhenBadMaterialization::ReadyForPartition(
                    GeneratedAffineResidualGroupExactWhenBadReadyForPartition {
                        schema:
                            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_WHEN_BAD_MATERIALIZATION_V1_SCHEMA,
                        plan,
                        sources,
                        boundaries,
                        limits,
                        stats,
                    },
                ),
            ),
            Ok(Ok(PreparedMaterialization::IdenticallyBad {
                sources,
                boundaries,
                witness,
                stats,
            })) => Ok(
                GeneratedAffineResidualGroupExactWhenBadMaterialization::IdenticallyBad(
                    GeneratedAffineResidualGroupExactWhenBadIdenticallyBad {
                        schema:
                            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_WHEN_BAD_MATERIALIZATION_V1_SCHEMA,
                        plan,
                        sources,
                        boundaries,
                        witness,
                        limits,
                        stats,
                    },
                ),
            ),
            Ok(Err(error)) => {
                Err(GeneratedAffineResidualGroupExactWhenBadMaterializationFailure { error, plan })
            }
            Err(_) => Err(
                GeneratedAffineResidualGroupExactWhenBadMaterializationFailure {
                    error:
                        GeneratedAffineResidualGroupExactWhenBadMaterializationError::SymbolicaPanic,
                    plan,
                },
            ),
        }
    }
}

enum PreparedMaterialization {
    Ready {
        sources: Vec<GeneratedAffineResidualGroupExactMappedSource>,
        boundaries: Vec<GeneratedAffineResidualGroupExactBoundaryEvent>,
        stats: GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
    },
    IdenticallyBad {
        sources: Vec<GeneratedAffineResidualGroupExactMappedSource>,
        boundaries: Vec<GeneratedAffineResidualGroupExactBoundaryEvent>,
        witness: GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness,
        stats: GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceMappingAdmissionKind {
    IdentityPolynomial(ParametricPolynomialValidationPayloadCensus),
    IdentityCoefficient(ParametricCoefficientValidationPayloadCensus),
    CompactPolynomial(ResidualUnitAffinePolynomialCompositionStats),
    CompactCoefficient(ResidualAffineCoefficientCompositionPreflight),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourceMappingAdmission {
    source: GeneratedAffineResidualGroupExactConditionSourceLocator,
    kind: SourceMappingAdmissionKind,
    retained_output_byte_bound: usize,
    normalized_denominator_temporary_byte_bound: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BoundaryValueAdmission {
    count: usize,
    stats: GeneratedAffineResidualGroupExactWhenBadBoundaryStats,
    enumeration_temporary_byte_peak: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ObservedBoundaryEnumeration {
    count: usize,
    value_integer_bits: usize,
    value_retained_logical_bytes: usize,
    enumeration_integer_bit_work: usize,
}

fn preflight_source_schedule(
    context: &ParametricCoefficientContext,
    plan: &GeneratedAffineResidualGroupExactConditionPlan,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    source_count: usize,
) -> Result<
    (
        Vec<SourceMappingAdmission>,
        GeneratedAffineResidualGroupExactWhenBadMappingStats,
    ),
    GeneratedAffineResidualGroupExactWhenBadMaterializationError,
> {
    if source_count != plan.source_schedule().len() {
        return Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan);
    }
    mark_materialization_admissions_reserve_observed_for_test();
    let mut admissions =
        try_vec_with_exact_capacity("exact WhenBad source mapping admissions", source_count)?;
    let mut aggregate = GeneratedAffineResidualGroupExactWhenBadMappingStats::default();
    for &source_locator in plan.source_schedule() {
        let (kind, retained_output_byte_bound) = match source_locator {
            GeneratedAffineResidualGroupExactConditionSourceLocator::TargetPremise {
                premise_ordinal,
            } => {
                let source = plan
                    .ready()
                    .ready()
                    .target_premises()
                    .get(premise_ordinal)
                    .ok_or(
                        GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan,
                    )?
                    .polynomial();
                preflight_condition_mapping(context, plan, source, limits, &mut aggregate)?
            }
            GeneratedAffineResidualGroupExactConditionSourceLocator::RecenteredRowGuard {
                guard_ordinal,
            } => {
                let source = plan
                    .ready()
                    .ready()
                    .row_guards()
                    .get(guard_ordinal)
                    .ok_or(
                        GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan,
                    )?
                    .polynomial();
                preflight_condition_mapping(context, plan, source, limits, &mut aggregate)?
            }
            GeneratedAffineResidualGroupExactConditionSourceLocator::PivotCoefficient {
                term_ordinal,
            }
            | GeneratedAffineResidualGroupExactConditionSourceLocator::RhsCoefficient {
                term_ordinal,
                ..
            } => {
                let source = plan
                    .ready()
                    .ready()
                    .terms()
                    .get(term_ordinal)
                    .ok_or(
                        GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan,
                    )?
                    .coefficient();
                preflight_coefficient_mapping(context, plan, source, limits, &mut aggregate)?
            }
        };
        aggregate.admitted_retained_byte_bound = checked_add(
            "exact WhenBad mapping admitted retained bytes",
            aggregate.admitted_retained_byte_bound,
            retained_output_byte_bound,
        )?;
        let normalized_denominator_temporary_byte_bound = if matches!(
            kind,
            SourceMappingAdmissionKind::IdentityCoefficient(_)
                | SourceMappingAdmissionKind::CompactCoefficient(_)
        ) {
            // The full mapped-core envelope strictly contains a deep copy of
            // either one normalized rational half.  Execution authenticates
            // the actual whole-coefficient and denominator-only bounds before
            // entering the fallible copy seam.
            retained_output_byte_bound
        } else {
            0
        };
        // The separately owned normalized denominator remains live across
        // both physical-parameter projections.  This source-phase peak is a
        // prospective live bound, not an observed allocation counter.
        let prospective_live = checked_add(
            "exact WhenBad mapping admission temporary bytes",
            aggregate.admitted_retained_byte_bound,
            normalized_denominator_temporary_byte_bound,
        )?;
        aggregate.admission_temporary_byte_peak = aggregate
            .admission_temporary_byte_peak
            .max(prospective_live);
        check_mapping_stats(aggregate, limits)?;
        admissions.push(SourceMappingAdmission {
            source: source_locator,
            kind,
            retained_output_byte_bound,
            normalized_denominator_temporary_byte_bound,
        });
    }
    if admissions.len() != source_count {
        return Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan);
    }
    Ok((admissions, aggregate))
}

fn preflight_condition_mapping(
    context: &ParametricCoefficientContext,
    plan: &GeneratedAffineResidualGroupExactConditionPlan,
    source: &ParametricPolynomial,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    aggregate: &mut GeneratedAffineResidualGroupExactWhenBadMappingStats,
) -> Result<
    (SourceMappingAdmissionKind, usize),
    GeneratedAffineResidualGroupExactWhenBadMaterializationError,
> {
    let census = preflight_identity_polynomial(context, source, limits, *aggregate)?;
    if let Some(compact) = plan.compact_target_transform() {
        let prepared = context.prepare_guard_on_residual_affine_compact_composition_plan(
            source,
            compact,
            remaining_composition_limits(limits, *aggregate)?,
        )?;
        let prospective = prepared.stats();
        drop(prepared);
        if prospective.source_terms() != census.source_terms()
            || prospective.source_exponent_entries() != census.source_exponent_entries()
        {
            return Err(
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch,
            );
        }
        aggregate.source_integer_bits = checked_add(
            "exact WhenBad mapping source integer bits",
            aggregate.source_integer_bits,
            census.source_integer_bits(),
        )?;
        admit_polynomial_mapping_stats(aggregate, prospective, limits)?;
        Ok((
            SourceMappingAdmissionKind::CompactPolynomial(prospective),
            mapped_condition_envelope(prospective)?,
        ))
    } else {
        admit_identity_polynomial_mapping_stats(aggregate, census, limits)?;
        let source_bytes = source.owned_retained_byte_bound().ok_or(
            GeneratedAffineResidualGroupExactWhenBadMaterializationError::ResourceCountOverflow {
                resource: "exact WhenBad identity condition retained bytes",
            },
        )?;
        Ok((
            SourceMappingAdmissionKind::IdentityPolynomial(census),
            checked_add(
                "exact WhenBad identity condition retained bytes",
                size_of::<GeneratedAffineResidualGroupExactMappedCondition>(),
                source_bytes,
            )?,
        ))
    }
}

fn preflight_coefficient_mapping(
    context: &ParametricCoefficientContext,
    plan: &GeneratedAffineResidualGroupExactConditionPlan,
    source: &ParametricCoefficient,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    aggregate: &mut GeneratedAffineResidualGroupExactWhenBadMappingStats,
) -> Result<
    (SourceMappingAdmissionKind, usize),
    GeneratedAffineResidualGroupExactWhenBadMaterializationError,
> {
    let census = preflight_identity_coefficient(context, source, limits, *aggregate)?;
    if let Some(compact) = plan.compact_target_transform() {
        let prepared = context.prepare_coefficient_on_residual_affine_compact_composition_plan(
            source,
            compact,
            remaining_composition_limits(limits, *aggregate)?,
        )?;
        let prospective = prepared.stats();
        drop(prepared);
        if prospective.aggregate().source_terms() != census.source_terms()
            || prospective.aggregate().source_exponent_entries() != census.source_exponent_entries()
        {
            return Err(
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch,
            );
        }
        aggregate.source_integer_bits = checked_add(
            "exact WhenBad mapping source integer bits",
            aggregate.source_integer_bits,
            census.source_integer_bits(),
        )?;
        admit_coefficient_mapping_stats(aggregate, prospective, limits)?;
        Ok((
            SourceMappingAdmissionKind::CompactCoefficient(prospective),
            mapped_coefficient_envelope(prospective)?,
        ))
    } else {
        admit_identity_coefficient_mapping_stats(aggregate, census, limits)?;
        let source_bytes = source.owned_retained_byte_bound().ok_or(
            GeneratedAffineResidualGroupExactWhenBadMaterializationError::ResourceCountOverflow {
                resource: "exact WhenBad identity coefficient retained bytes",
            },
        )?;
        let three_payloads = checked_mul(
            "exact WhenBad identity coefficient retained bytes",
            source_bytes,
            3,
        )?;
        Ok((
            SourceMappingAdmissionKind::IdentityCoefficient(census),
            checked_add(
                "exact WhenBad identity coefficient retained bytes",
                size_of::<GeneratedAffineResidualGroupExactMappedCoefficient>(),
                three_payloads,
            )?,
        ))
    }
}

fn replay_materialization(
    schema: &'static str,
    plan: &GeneratedAffineResidualGroupExactConditionPlan,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    session: &GeneratedAffineResidualGroupExactSession,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    sources: &[GeneratedAffineResidualGroupExactMappedSource],
    boundaries: &[GeneratedAffineResidualGroupExactBoundaryEvent],
    witness: Option<&GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness>,
    stats: GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    catch_unwind(AssertUnwindSafe(|| {
        if schema != GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_WHEN_BAD_MATERIALIZATION_V1_SCHEMA {
            return Err(
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::SchemaMismatch,
            );
        }
        let rebuilt = prepare_materialization(family, context, session, plan, limits)?;
        match rebuilt {
            PreparedMaterialization::Ready {
                sources: rebuilt_sources,
                boundaries: rebuilt_boundaries,
                stats: rebuilt_stats,
            } if witness.is_none()
                && rebuilt_sources.as_slice() == sources
                && rebuilt_boundaries.as_slice() == boundaries
                && rebuilt_stats == stats =>
            {
                Ok(())
            }
            PreparedMaterialization::IdenticallyBad {
                sources: rebuilt_sources,
                boundaries: rebuilt_boundaries,
                witness: rebuilt_witness,
                stats: rebuilt_stats,
            } if Some(&rebuilt_witness) == witness
                && rebuilt_sources.as_slice() == sources
                && rebuilt_boundaries.as_slice() == boundaries
                && rebuilt_stats == stats =>
            {
                Ok(())
            }
            _ => Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch),
        }
    }))
    .map_err(|_| GeneratedAffineResidualGroupExactWhenBadMaterializationError::SymbolicaPanic)?
}

fn prepare_materialization(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    session: &GeneratedAffineResidualGroupExactSession,
    plan: &GeneratedAffineResidualGroupExactConditionPlan,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<PreparedMaterialization, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    maybe_inject_materialization_boundary_panic_for_test();
    plan.replay(family, context, session)?;

    let source_count = plan.source_schedule().len();
    check_limit(
        "exact WhenBad materialization source records",
        source_count,
        limits.max_source_records,
    )?;
    let condition_count = plan
        .source_schedule()
        .iter()
        .filter(|source| {
            matches!(
                source,
                GeneratedAffineResidualGroupExactConditionSourceLocator::TargetPremise { .. }
                    | GeneratedAffineResidualGroupExactConditionSourceLocator::RecenteredRowGuard { .. }
            )
        })
        .count();
    let coefficient_count = source_count
        .checked_sub(condition_count)
        .ok_or(GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan)?;
    check_limit(
        "exact WhenBad materialization condition records",
        condition_count,
        limits.max_condition_records,
    )?;
    check_limit(
        "exact WhenBad materialization coefficient records",
        coefficient_count,
        limits.max_coefficient_records,
    )?;
    let admitted_denominator_identity_sources = checked_mul(
        "exact WhenBad denominator identity sources",
        coefficient_count,
        2,
    )?;
    check_limit(
        "exact WhenBad denominator identity sources",
        admitted_denominator_identity_sources,
        limits.max_denominator_identity_sources,
    )?;
    let hazard_count = plan.hazard_schedule().len();
    check_limit(
        "exact WhenBad materialization hazard ranges",
        hazard_count,
        limits.max_hazard_ranges,
    )?;
    let source_slots = checked_mul(
        "exact WhenBad materialization source record slots",
        source_count,
        size_of::<GeneratedAffineResidualGroupExactMappedSource>(),
    )?;
    let admission_slots = checked_mul(
        "exact WhenBad source admission slots",
        source_count,
        size_of::<SourceMappingAdmission>(),
    )?;
    let term_count = plan.ready().ready().terms().len();
    let lookup_slots = checked_mul(
        "exact WhenBad coefficient source lookup slots",
        term_count,
        size_of::<Option<usize>>(),
    )?;
    // `preflight_source_schedule` reserves this exact arena immediately on
    // entry.  Admit it against the outer owner before that first allocation;
    // later combined-live checks additionally include every durable arena.
    check_limit(
        "exact WhenBad materialization compilation owned logical peak",
        admission_slots,
        limits.max_compilation_owned_logical_peak_upper_bound,
    )?;
    // Exact lazy-range admission is independent of every mapped source.  It
    // must therefore reject here, before composition preflight and certainly
    // before any composition/copy execution.
    let boundary_value_admission = census_boundary_values(plan, limits)?;
    let boundary_value_count = boundary_value_admission.count;
    let boundary_slots = checked_mul(
        "exact WhenBad boundary event slots",
        boundary_value_count,
        size_of::<GeneratedAffineResidualGroupExactBoundaryEvent>(),
    )?;
    let root_header = size_of::<PreparedMaterialization>();
    let durable_fixed_arena_bytes = [root_header, source_slots, boundary_slots]
        .into_iter()
        .try_fold(0usize, |sum, bytes| {
            checked_add(
                "exact WhenBad durable fixed materialization arenas",
                sum,
                bytes,
            )
        })?;
    let combined_live_fixed_arena_bytes = [
        root_header,
        source_slots,
        admission_slots,
        lookup_slots,
        boundary_slots,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        checked_add("exact WhenBad fixed materialization arenas", sum, bytes)
    })?;
    let pre_source_durable_minimum = checked_add(
        "exact WhenBad pre-source durable materialization bytes",
        durable_fixed_arena_bytes,
        boundary_value_admission.stats.value_retained_logical_bytes,
    )?;
    check_limit(
        "exact WhenBad materialization retained owned logical bytes",
        pre_source_durable_minimum,
        limits.max_retained_owned_logical_bytes,
    )?;
    check_limit(
        "exact WhenBad materialization compilation owned logical peak",
        pre_source_durable_minimum.max(boundary_value_admission.enumeration_temporary_byte_peak),
        limits.max_compilation_owned_logical_peak_upper_bound,
    )?;
    let (admissions, mapping_stats) =
        preflight_source_schedule(context, plan, limits, source_count)?;
    let mapping_retained_bound = checked_add(
        "exact WhenBad mapping admitted retained bytes",
        mapping_stats.admitted_retained_byte_bound,
        durable_fixed_arena_bytes,
    )?;
    let known_durable_bound = checked_add(
        "exact WhenBad known durable materialization bytes",
        mapping_retained_bound,
        boundary_value_admission.stats.value_retained_logical_bytes,
    )?;
    check_limit(
        "exact WhenBad materialization retained owned logical bytes",
        known_durable_bound,
        limits.max_retained_owned_logical_bytes,
    )?;
    let initial_compilation_peak = checked_add(
        "exact WhenBad source admission combined-live peak",
        combined_live_fixed_arena_bytes,
        mapping_stats.admission_temporary_byte_peak,
    )?
    .max(known_durable_bound)
    .max(boundary_value_admission.enumeration_temporary_byte_peak);
    check_limit(
        "exact WhenBad materialization compilation owned logical peak",
        initial_compilation_peak,
        limits.max_compilation_owned_logical_peak_upper_bound,
    )?;
    let mut sources =
        try_vec_with_exact_capacity("exact WhenBad materialization source records", source_count)?;
    let mut coefficient_source_by_term =
        try_vec_with_exact_capacity("exact WhenBad coefficient source lookup", term_count)?;
    coefficient_source_by_term.resize(term_count, None);
    let mut stats = GeneratedAffineResidualGroupExactWhenBadMaterializationStats {
        source_records: source_count,
        condition_records: condition_count,
        coefficient_records: coefficient_count,
        denominator_identity_sources: admitted_denominator_identity_sources,
        hazard_ranges: hazard_count,
        admitted_boundary_values: boundary_value_count,
        mapping: mapping_stats,
        boundary: boundary_value_admission.stats,
        source_phase_retained_logical_byte_bound: mapping_retained_bound,
        active_transaction_arena_byte_bound: checked_add(
            "exact WhenBad active transaction arenas",
            admission_slots,
            lookup_slots,
        )?,
        compilation_owned_logical_peak_upper_bound: initial_compilation_peak,
        ..GeneratedAffineResidualGroupExactWhenBadMaterializationStats::default()
    };

    for (&source_locator, admission) in plan.source_schedule().iter().zip(admissions.iter()) {
        if admission.source != source_locator {
            return Err(
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan,
            );
        }
        let mapped = match source_locator {
            GeneratedAffineResidualGroupExactConditionSourceLocator::TargetPremise {
                premise_ordinal,
            } => {
                let source = plan
                    .ready()
                    .ready()
                    .target_premises()
                    .get(premise_ordinal)
                    .ok_or(
                        GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan,
                    )?;
                match map_condition_source(
                    context,
                    plan,
                    source_locator,
                    source.polynomial(),
                    true,
                    admission,
                    limits,
                )? {
                    MappedConditionOutcome::Available(value) => {
                        GeneratedAffineResidualGroupExactMappedSource::Condition(value)
                    }
                    MappedConditionOutcome::CandidateIdenticallyBad { .. } => {
                        return Err(
                            GeneratedAffineResidualGroupExactWhenBadMaterializationError::IdenticallyZeroTargetPremise {
                                source: source_locator,
                            },
                        );
                    }
                }
            }
            GeneratedAffineResidualGroupExactConditionSourceLocator::RecenteredRowGuard {
                guard_ordinal,
            } => {
                let source = plan.ready().ready().row_guards().get(guard_ordinal).ok_or(
                    GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan,
                )?;
                match map_condition_source(
                    context,
                    plan,
                    source_locator,
                    source.polynomial(),
                    false,
                    admission,
                    limits,
                )? {
                    MappedConditionOutcome::Available(value) => {
                        GeneratedAffineResidualGroupExactMappedSource::Condition(value)
                    }
                    MappedConditionOutcome::CandidateIdenticallyBad {
                        polynomial,
                        mapping,
                    } => {
                        return finish_identically_bad(
                            sources,
                            Vec::new(),
                            GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness::CandidateGuardMappedToZero {
                                source: source_locator,
                                polynomial,
                                mapping,
                            },
                            stats,
                            limits,
                        );
                    }
                }
            }
            GeneratedAffineResidualGroupExactConditionSourceLocator::PivotCoefficient {
                term_ordinal,
            }
            | GeneratedAffineResidualGroupExactConditionSourceLocator::RhsCoefficient {
                term_ordinal,
                ..
            } => {
                let source = plan
                    .ready()
                    .ready()
                    .terms()
                    .get(term_ordinal)
                    .ok_or(
                        GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan,
                    )?
                    .coefficient();
                match map_coefficient_source(
                    context,
                    plan,
                    source_locator,
                    source,
                    admission,
                    limits,
                    &mut stats,
                )? {
                    MappedCoefficientOutcome::Available(value) => {
                        if coefficient_source_by_term
                            .get(term_ordinal)
                            .and_then(|entry| *entry)
                            .is_some()
                        {
                            return Err(
                                GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan,
                            );
                        }
                        coefficient_source_by_term[term_ordinal] = Some(sources.len());
                        GeneratedAffineResidualGroupExactMappedSource::Coefficient(value)
                    }
                    MappedCoefficientOutcome::IdenticallyBad { witness } => {
                        return finish_identically_bad(sources, Vec::new(), witness, stats, limits);
                    }
                }
            }
        };
        sources.push(mapped);
        maybe_inject_materialization_partial_ownership_panic_for_test(sources.len());
    }
    if sources.len() != source_count {
        return Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan);
    }
    drop(admissions);
    stats.active_transaction_arena_byte_bound = lookup_slots;

    let mut boundaries =
        try_vec_with_exact_capacity("exact WhenBad boundary events", boundary_value_count)?;
    let terminal_witness = materialize_boundaries(
        context,
        plan,
        &sources,
        &coefficient_source_by_term,
        limits,
        &mut stats,
        &mut boundaries,
    )?;
    if let Some(witness) = terminal_witness {
        return finish_identically_bad(sources, boundaries, witness, stats, limits);
    }
    if boundaries.len() != boundary_value_count {
        return Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan);
    }
    finish_ready(sources, boundaries, stats, limits)
}

enum MappedConditionOutcome {
    Available(GeneratedAffineResidualGroupExactMappedCondition),
    CandidateIdenticallyBad {
        polynomial: ParametricPolynomial,
        mapping: GeneratedAffineResidualGroupExactSourceMappingStats,
    },
}

fn map_condition_source(
    context: &ParametricCoefficientContext,
    plan: &GeneratedAffineResidualGroupExactConditionPlan,
    source_locator: GeneratedAffineResidualGroupExactConditionSourceLocator,
    source: &ParametricPolynomial,
    inherited_target_premise: bool,
    admission: &SourceMappingAdmission,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<MappedConditionOutcome, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let (mapped, mapping) =
        if let Some(compact) = plan.compact_target_transform() {
            let prepared = context.prepare_guard_on_residual_affine_compact_composition_plan(
                source,
                compact,
                limits.polynomial_composition,
            )?;
            let prospective = prepared.stats();
            if admission.kind != SourceMappingAdmissionKind::CompactPolynomial(prospective) {
                return Err(
                    GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch,
                );
            }
            let mapped = prepared.execute()?;
            let (value, observed) = mapped.into_parts();
            authenticate_polynomial_mapping_stats(prospective, observed)?;
            (
                value,
                GeneratedAffineResidualGroupExactSourceMappingStats::CompactPolynomial(observed),
            )
        } else {
            let SourceMappingAdmissionKind::IdentityPolynomial(census) = admission.kind else {
                return Err(
                    GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch,
                );
            };
            let value = source.try_copy_authenticated_sparse_payload().map_err(|resource| {
            GeneratedAffineResidualGroupExactWhenBadMaterializationError::AllocationFailure {
                resource,
                requested: census.source_terms(),
            }
        })?;
            (
                value,
                GeneratedAffineResidualGroupExactSourceMappingStats::IdentityPolynomial(census),
            )
        };
    let observed_retained = checked_add(
        "exact WhenBad mapped condition retained bytes",
        size_of::<GeneratedAffineResidualGroupExactMappedCondition>(),
        polynomial_retained_bound(&mapped)?,
    )?;
    if observed_retained > admission.retained_output_byte_bound {
        return Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch);
    }
    if mapped.is_zero() {
        if inherited_target_premise {
            return Err(
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::IdenticallyZeroTargetPremise {
                    source: source_locator,
                },
            );
        }
        return Ok(MappedConditionOutcome::CandidateIdenticallyBad {
            polynomial: mapped,
            mapping,
        });
    }
    let class = if mapped.is_nonzero_constant() {
        GeneratedAffineResidualGroupExactMappedConditionClass::DischargedNonzeroIntegerConstant
    } else if context.polynomial_depends_on_indices_with_limits(
        &mapped,
        limits.polynomial_composition.exact_algebra,
    )? {
        GeneratedAffineResidualGroupExactMappedConditionClass::IndexDependent
    } else if inherited_target_premise {
        GeneratedAffineResidualGroupExactMappedConditionClass::BaseParameterAssumption
    } else {
        GeneratedAffineResidualGroupExactMappedConditionClass::DischargedCoefficientFieldUnit
    };
    Ok(MappedConditionOutcome::Available(
        GeneratedAffineResidualGroupExactMappedCondition {
            source: source_locator,
            class,
            polynomial: mapped,
            mapping,
        },
    ))
}

enum MappedCoefficientOutcome {
    Available(GeneratedAffineResidualGroupExactMappedCoefficient),
    IdenticallyBad {
        witness: GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness,
    },
}

fn map_coefficient_source(
    context: &ParametricCoefficientContext,
    plan: &GeneratedAffineResidualGroupExactConditionPlan,
    source_locator: GeneratedAffineResidualGroupExactConditionSourceLocator,
    source: &ParametricCoefficient,
    admission: &SourceMappingAdmission,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    stats: &mut GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
) -> Result<MappedCoefficientOutcome, GeneratedAffineResidualGroupExactWhenBadMaterializationError>
{
    let (normalized_value, pre_normalization_mapped_denominator, mapping) = if let Some(compact) =
        plan.compact_target_transform()
    {
        let prepared = context.prepare_coefficient_on_residual_affine_compact_composition_plan(
            source,
            compact,
            limits.polynomial_composition,
        )?;
        let prospective = prepared.stats();
        if admission.kind != SourceMappingAdmissionKind::CompactCoefficient(prospective) {
            return Err(
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch,
            );
        }
        match prepared.execute()? {
            ResidualAffineCoefficientComposition::ZeroMappedDenominator { stats: observed } => {
                authenticate_coefficient_mapping_stats(prospective, observed)?;
                return Ok(MappedCoefficientOutcome::IdenticallyBad {
                        witness: GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness::ZeroMappedDenominator {
                            source: source_locator,
                            mapping: observed,
                        },
                    });
            }
            ResidualAffineCoefficientComposition::Available(mapped) => {
                let (value, denominator, observed) = mapped.into_parts();
                authenticate_coefficient_mapping_stats(prospective, observed)?;
                (
                    value,
                    denominator,
                    GeneratedAffineResidualGroupExactSourceMappingStats::CompactCoefficient(
                        observed,
                    ),
                )
            }
        }
    } else {
        let SourceMappingAdmissionKind::IdentityCoefficient(census) = admission.kind else {
            return Err(
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch,
            );
        };
        let normalized_value = source.try_copy_authenticated_sparse_payload().map_err(
            |resource| {
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::AllocationFailure {
                    resource,
                    requested: census.source_terms(),
                }
            },
        )?;
        let pre_normalization_mapped_denominator = source
            .try_copy_prevalidated_denominator_condition()
            .map_err(|resource| {
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::AllocationFailure {
                    resource,
                    requested: census.source_terms(),
                }
            })?;
        (
            normalized_value,
            pre_normalization_mapped_denominator,
            GeneratedAffineResidualGroupExactSourceMappingStats::IdentityCoefficient(census),
        )
    };

    let normalized_clone_bound = coefficient_retained_bound(&normalized_value)?;
    if normalized_clone_bound > admission.normalized_denominator_temporary_byte_bound {
        return Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch);
    }
    // Both mapped paths above return an authenticated normalized coefficient.
    // Its complete deep-copy bound strictly contains either single sparse
    // half, so admit that bound as live scratch before entering either
    // fallible copy seam.  The denominator stays active through both identity
    // projections and is included by every remaining-child calculation.
    stats.active_normalized_denominator_temporary_byte_bound =
        admission.normalized_denominator_temporary_byte_bound;
    refresh_compilation_peak(stats, limits)?;
    let normalized_numerator = normalized_value
        .try_copy_prevalidated_numerator_condition()
        .map_err(|resource| {
            GeneratedAffineResidualGroupExactWhenBadMaterializationError::AllocationFailure {
                resource,
                requested: normalized_clone_bound,
            }
        })?;
    if polynomial_retained_bound(&normalized_numerator)? > normalized_clone_bound {
        return Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch);
    }
    let normalized_denominator = normalized_value
        .try_copy_prevalidated_denominator_condition()
        .map_err(|resource| {
            GeneratedAffineResidualGroupExactWhenBadMaterializationError::AllocationFailure {
                resource,
                requested: normalized_clone_bound,
            }
        })?;
    if polynomial_retained_bound(&normalized_denominator)? > normalized_clone_bound {
        return Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch);
    }
    let observed_mapping_retained = mapped_coefficient_core_owned_retained_byte_bound(
        &normalized_value,
        &normalized_numerator,
        &pre_normalization_mapped_denominator,
    )?;
    if observed_mapping_retained > admission.retained_output_byte_bound {
        return Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch);
    }

    let pre_normalization = project_denominator_identity(
        context,
        &pre_normalization_mapped_denominator,
        source_locator,
        GeneratedAffineResidualGroupExactDenominatorIdentityKind::PreNormalizationMappedDenominator,
        limits,
        stats,
    )?;
    let normalized = project_denominator_identity(
        context,
        &normalized_denominator,
        source_locator,
        GeneratedAffineResidualGroupExactDenominatorIdentityKind::NormalizedMappedDenominator,
        limits,
        stats,
    )?;
    stats.active_normalized_denominator_temporary_byte_bound = 0;
    Ok(MappedCoefficientOutcome::Available(
        GeneratedAffineResidualGroupExactMappedCoefficient {
            source: source_locator,
            normalized_value,
            normalized_numerator,
            pre_normalization_mapped_denominator,
            denominator_identities: [pre_normalization, normalized],
            mapping,
        },
    ))
}

fn project_denominator_identity(
    context: &ParametricCoefficientContext,
    polynomial: &ParametricPolynomial,
    source: GeneratedAffineResidualGroupExactConditionSourceLocator,
    kind: GeneratedAffineResidualGroupExactDenominatorIdentityKind,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    stats: &mut GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
) -> Result<
    GeneratedAffineResidualGroupExactDenominatorIdentity,
    GeneratedAffineResidualGroupExactWhenBadMaterializationError,
> {
    if polynomial.is_zero() {
        return Err(
            GeneratedAffineResidualGroupExactWhenBadMaterializationError::DenominatorProjectionInvariantViolation {
                source,
                kind,
            },
        );
    }
    let effective = remaining_parameter_identity_limits(limits, *stats)?;
    let prepared = context.prepare_parameter_identity_projection(polynomial, effective)?;
    let prospective = prepared.stats();
    admit_projection_stats(&mut stats.projection, prospective, limits)?;
    let prospective_loci = checked_add(
        "exact WhenBad denominator identity loci",
        stats.denominator_identity_loci,
        prospective.conditional_locus_bound(),
    )?;
    check_limit(
        "exact WhenBad denominator identity loci",
        prospective_loci,
        limits.max_denominator_identity_loci,
    )?;
    stats.denominator_identity_loci = prospective_loci;
    refresh_compilation_peak(stats, limits)?;
    let projection = prepared.execute()?;
    authenticate_projection_stats(prospective, projection.stats())?;
    if matches!(
        projection.class(),
        ParametricParameterIdentityClass::AlwaysIdentityZero
    ) {
        return Err(
            GeneratedAffineResidualGroupExactWhenBadMaterializationError::DenominatorProjectionInvariantViolation {
                source,
                kind,
            },
        );
    }
    stats.projection.projected_physical_monomials = checked_add(
        "exact WhenBad projected physical monomials",
        stats.projection.projected_physical_monomials,
        projection.stats().projected_physical_monomials(),
    )?;
    let conditional_loci = projection
        .class()
        .coefficient_loci()
        .map_or(0, |loci| loci.len());
    stats.projection.conditional_loci = checked_add(
        "exact WhenBad conditional denominator loci",
        stats.projection.conditional_loci,
        conditional_loci,
    )?;
    Ok(GeneratedAffineResidualGroupExactDenominatorIdentity { kind, projection })
}

fn preflight_identity_polynomial(
    context: &ParametricCoefficientContext,
    source: &ParametricPolynomial,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    prior: GeneratedAffineResidualGroupExactWhenBadMappingStats,
) -> Result<
    ParametricPolynomialValidationPayloadCensus,
    GeneratedAffineResidualGroupExactWhenBadMaterializationError,
> {
    context
        .preflight_polynomial_validation_payload_with_limits(
            source,
            limits.polynomial_composition.exact_algebra,
            remaining_limit(
                "exact WhenBad mapping source terms",
                limits.max_mapping_source_terms,
                prior.source_terms,
            )?,
            remaining_limit(
                "exact WhenBad mapping source exponent entries",
                limits.max_mapping_source_exponent_entries,
                prior.source_exponent_entries,
            )?,
            remaining_limit(
                "exact WhenBad mapping source integer bits",
                limits.max_mapping_source_integer_bits,
                prior.source_integer_bits,
            )?,
        )
        .map_err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::from)
}

fn preflight_identity_coefficient(
    context: &ParametricCoefficientContext,
    source: &ParametricCoefficient,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    prior: GeneratedAffineResidualGroupExactWhenBadMappingStats,
) -> Result<
    ParametricCoefficientValidationPayloadCensus,
    GeneratedAffineResidualGroupExactWhenBadMaterializationError,
> {
    context
        .preflight_validation_payload_with_limits(
            source,
            limits.polynomial_composition.exact_algebra,
            remaining_limit(
                "exact WhenBad mapping source terms",
                limits.max_mapping_source_terms,
                prior.source_terms,
            )?,
            remaining_limit(
                "exact WhenBad mapping source exponent entries",
                limits.max_mapping_source_exponent_entries,
                prior.source_exponent_entries,
            )?,
            remaining_limit(
                "exact WhenBad mapping source integer bits",
                limits.max_mapping_source_integer_bits,
                prior.source_integer_bits,
            )?,
        )
        .map_err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::from)
}

fn admit_identity_polynomial_mapping_stats(
    aggregate: &mut GeneratedAffineResidualGroupExactWhenBadMappingStats,
    census: ParametricPolynomialValidationPayloadCensus,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    aggregate.source_terms = checked_add(
        "exact WhenBad mapping source terms",
        aggregate.source_terms,
        census.source_terms(),
    )?;
    aggregate.source_exponent_entries = checked_add(
        "exact WhenBad mapping source exponent entries",
        aggregate.source_exponent_entries,
        census.source_exponent_entries(),
    )?;
    aggregate.source_integer_bits = checked_add(
        "exact WhenBad mapping source integer bits",
        aggregate.source_integer_bits,
        census.source_integer_bits(),
    )?;
    check_mapping_stats(*aggregate, limits)
}

fn admit_identity_coefficient_mapping_stats(
    aggregate: &mut GeneratedAffineResidualGroupExactWhenBadMappingStats,
    census: ParametricCoefficientValidationPayloadCensus,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    aggregate.source_terms = checked_add(
        "exact WhenBad mapping source terms",
        aggregate.source_terms,
        census.source_terms(),
    )?;
    aggregate.source_exponent_entries = checked_add(
        "exact WhenBad mapping source exponent entries",
        aggregate.source_exponent_entries,
        census.source_exponent_entries(),
    )?;
    aggregate.source_integer_bits = checked_add(
        "exact WhenBad mapping source integer bits",
        aggregate.source_integer_bits,
        census.source_integer_bits(),
    )?;
    check_mapping_stats(*aggregate, limits)
}

fn admit_polynomial_mapping_stats(
    aggregate: &mut GeneratedAffineResidualGroupExactWhenBadMappingStats,
    value: ResidualUnitAffinePolynomialCompositionStats,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    add_polynomial_mapping_stats(aggregate, value)?;
    check_mapping_stats(*aggregate, limits)
}

fn admit_coefficient_mapping_stats(
    aggregate: &mut GeneratedAffineResidualGroupExactWhenBadMappingStats,
    value: ResidualAffineCoefficientCompositionPreflight,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    add_polynomial_mapping_stats(aggregate, value.aggregate())?;
    aggregate.integer_bit_work_bound = checked_add(
        "exact WhenBad mapping integer-bit work",
        aggregate.integer_bit_work_bound,
        value.durable_denominator_integer_bit_payload_bound(),
    )?;
    aggregate.normalization_input_term_pairs = checked_add(
        "exact WhenBad mapping normalization input term pairs",
        aggregate.normalization_input_term_pairs,
        value.normalization_input_term_pair_bound(),
    )?;
    check_mapping_stats(*aggregate, limits)
}

fn add_polynomial_mapping_stats(
    aggregate: &mut GeneratedAffineResidualGroupExactWhenBadMappingStats,
    value: ResidualUnitAffinePolynomialCompositionStats,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    macro_rules! add_field {
        ($field:ident, $getter:ident, $resource:literal) => {
            aggregate.$field = checked_add($resource, aggregate.$field, value.$getter())?;
        };
    }
    add_field!(
        source_terms,
        source_terms,
        "exact WhenBad mapping source terms"
    );
    add_field!(
        source_exponent_entries,
        source_exponent_entries,
        "exact WhenBad mapping source exponent entries"
    );
    add_field!(
        expanded_contribution_bound,
        expanded_contribution_bound,
        "exact WhenBad mapping expanded contributions"
    );
    add_field!(
        output_exponent_entry_bound,
        output_exponent_entry_bound,
        "exact WhenBad mapping output exponent-entry bound"
    );
    add_field!(
        power_calls,
        power_calls,
        "exact WhenBad mapping power calls"
    );
    add_field!(
        native_power_heap_pair_bound,
        native_power_heap_pair_bound,
        "exact WhenBad mapping native power heap pairs"
    );
    add_field!(
        multiplication_term_pair_bound,
        multiplication_term_pair_bound,
        "exact WhenBad mapping multiplication term pairs"
    );
    add_field!(
        addition_term_visit_bound,
        addition_term_visit_bound,
        "exact WhenBad mapping addition term visits"
    );
    add_field!(
        native_integer_bit_work_bound,
        native_integer_bit_work_bound,
        "exact WhenBad mapping native integer-bit work"
    );
    add_field!(
        integer_bit_work_bound,
        integer_bit_work_bound,
        "exact WhenBad mapping integer-bit work"
    );
    Ok(())
}

fn check_mapping_stats(
    stats: GeneratedAffineResidualGroupExactWhenBadMappingStats,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    for (resource, requested, limit) in [
        (
            "exact WhenBad mapping source terms",
            stats.source_terms,
            limits.max_mapping_source_terms,
        ),
        (
            "exact WhenBad mapping source exponent entries",
            stats.source_exponent_entries,
            limits.max_mapping_source_exponent_entries,
        ),
        (
            "exact WhenBad mapping source integer bits",
            stats.source_integer_bits,
            limits.max_mapping_source_integer_bits,
        ),
        (
            "exact WhenBad mapping admitted retained bytes",
            stats.admitted_retained_byte_bound,
            limits.max_mapping_admitted_retained_byte_bound,
        ),
        (
            "exact WhenBad mapping admission temporary byte peak",
            stats.admission_temporary_byte_peak,
            limits.max_mapping_admission_temporary_byte_peak,
        ),
        (
            "exact WhenBad mapping expanded contributions",
            stats.expanded_contribution_bound,
            limits.max_mapping_expanded_contribution_bound,
        ),
        (
            "exact WhenBad mapping output exponent-entry bound",
            stats.output_exponent_entry_bound,
            limits.max_mapping_output_exponent_entry_bound,
        ),
        (
            "exact WhenBad mapping power calls",
            stats.power_calls,
            limits.max_mapping_power_calls,
        ),
        (
            "exact WhenBad mapping native power heap pairs",
            stats.native_power_heap_pair_bound,
            limits.max_mapping_native_power_heap_pair_bound,
        ),
        (
            "exact WhenBad mapping multiplication term pairs",
            stats.multiplication_term_pair_bound,
            limits.max_mapping_multiplication_term_pair_bound,
        ),
        (
            "exact WhenBad mapping addition term visits",
            stats.addition_term_visit_bound,
            limits.max_mapping_addition_term_visit_bound,
        ),
        (
            "exact WhenBad mapping native integer-bit work",
            stats.native_integer_bit_work_bound,
            limits.max_mapping_native_integer_bit_work_bound,
        ),
        (
            "exact WhenBad mapping integer-bit work",
            stats.integer_bit_work_bound,
            limits.max_mapping_integer_bit_work_bound,
        ),
        (
            "exact WhenBad mapping normalization input term pairs",
            stats.normalization_input_term_pairs,
            limits.max_mapping_normalization_input_term_pairs,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    Ok(())
}

fn remaining_composition_limits(
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    prior: GeneratedAffineResidualGroupExactWhenBadMappingStats,
) -> Result<
    ResidualUnitAffinePolynomialCompositionLimits,
    GeneratedAffineResidualGroupExactWhenBadMaterializationError,
> {
    let mut child = limits.polynomial_composition;
    child.max_source_terms = child.max_source_terms.min(remaining_limit(
        "exact WhenBad mapping source terms",
        limits.max_mapping_source_terms,
        prior.source_terms,
    )?);
    child.max_source_exponent_entries = child.max_source_exponent_entries.min(remaining_limit(
        "exact WhenBad mapping source exponent entries",
        limits.max_mapping_source_exponent_entries,
        prior.source_exponent_entries,
    )?);
    child.max_expanded_contributions = child.max_expanded_contributions.min(remaining_limit(
        "exact WhenBad mapping expanded contributions",
        limits.max_mapping_expanded_contribution_bound,
        prior.expanded_contribution_bound,
    )?);
    child.max_output_exponent_entries = child.max_output_exponent_entries.min(remaining_limit(
        "exact WhenBad mapping output exponent entries",
        limits.max_mapping_output_exponent_entry_bound,
        prior.output_exponent_entry_bound,
    )?);
    child.max_power_calls = child.max_power_calls.min(remaining_limit(
        "exact WhenBad mapping power calls",
        limits.max_mapping_power_calls,
        prior.power_calls,
    )?);
    child.max_native_power_heap_pairs = child.max_native_power_heap_pairs.min(remaining_limit(
        "exact WhenBad mapping native power heap pairs",
        limits.max_mapping_native_power_heap_pair_bound,
        prior.native_power_heap_pair_bound,
    )?);
    child.max_multiplication_term_pairs = child.max_multiplication_term_pairs.min(remaining_limit(
        "exact WhenBad mapping multiplication term pairs",
        limits.max_mapping_multiplication_term_pair_bound,
        prior.multiplication_term_pair_bound,
    )?);
    child.max_addition_term_visits = child.max_addition_term_visits.min(remaining_limit(
        "exact WhenBad mapping addition term visits",
        limits.max_mapping_addition_term_visit_bound,
        prior.addition_term_visit_bound,
    )?);
    child.max_native_integer_bit_work = child.max_native_integer_bit_work.min(remaining_limit(
        "exact WhenBad mapping native integer-bit work",
        limits.max_mapping_native_integer_bit_work_bound,
        prior.native_integer_bit_work_bound,
    )?);
    child.max_integer_bit_work = child.max_integer_bit_work.min(remaining_limit(
        "exact WhenBad mapping integer-bit work",
        limits.max_mapping_integer_bit_work_bound,
        prior.integer_bit_work_bound,
    )?);
    child.max_normalization_input_term_pairs =
        child
            .max_normalization_input_term_pairs
            .min(remaining_limit(
                "exact WhenBad mapping normalization input term pairs",
                limits.max_mapping_normalization_input_term_pairs,
                prior.normalization_input_term_pairs,
            )?);
    Ok(child)
}

fn admit_projection_stats(
    aggregate: &mut GeneratedAffineResidualGroupExactWhenBadProjectionStats,
    value: ParametricParameterIdentityProjectionStats,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    aggregate.sources = checked_add("exact WhenBad projection sources", aggregate.sources, 1)?;
    aggregate.source_terms = checked_add(
        "exact WhenBad projection source terms",
        aggregate.source_terms,
        value.source_terms(),
    )?;
    aggregate.source_exponent_entries = checked_add(
        "exact WhenBad projection source exponent entries",
        aggregate.source_exponent_entries,
        value.source_exponent_entries(),
    )?;
    aggregate.source_integer_bits = checked_add(
        "exact WhenBad projection source integer bits",
        aggregate.source_integer_bits,
        value.source_integer_bits(),
    )?;
    aggregate.native_workspace_byte_envelope = checked_add(
        "exact WhenBad projection native workspace bytes",
        aggregate.native_workspace_byte_envelope,
        value.native_projection_grouping_workspace_byte_envelope(),
    )?;
    aggregate.retained_output_byte_bound = checked_add(
        "exact WhenBad projection retained output bytes",
        aggregate.retained_output_byte_bound,
        value.retained_output_byte_bound(),
    )?;
    aggregate.temporary_byte_envelope = aggregate
        .temporary_byte_envelope
        .max(value.rustred_visible_temporary_byte_envelope());
    check_projection_stats(*aggregate, limits)
}

fn check_projection_stats(
    stats: GeneratedAffineResidualGroupExactWhenBadProjectionStats,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    for (resource, requested, limit) in [
        (
            "exact WhenBad projection source terms",
            stats.source_terms,
            limits.max_projection_source_terms,
        ),
        (
            "exact WhenBad projection source exponent entries",
            stats.source_exponent_entries,
            limits.max_projection_source_exponent_entries,
        ),
        (
            "exact WhenBad projection source integer bits",
            stats.source_integer_bits,
            limits.max_projection_source_integer_bits,
        ),
        (
            "exact WhenBad projection native workspace bytes",
            stats.native_workspace_byte_envelope,
            limits.max_projection_native_workspace_byte_envelope,
        ),
        (
            "exact WhenBad projection retained output bytes",
            stats.retained_output_byte_bound,
            limits.max_projection_retained_output_byte_bound,
        ),
        (
            "exact WhenBad projection temporary bytes",
            stats.temporary_byte_envelope,
            limits.max_projection_temporary_byte_envelope,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    Ok(())
}

fn remaining_parameter_identity_limits(
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    stats: GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
) -> Result<
    ParametricParameterIdentityProjectionLimits,
    GeneratedAffineResidualGroupExactWhenBadMaterializationError,
> {
    let prior = stats.projection;
    let mut child = limits.parameter_identity;
    child.max_source_terms = child.max_source_terms.min(remaining_limit(
        "exact WhenBad projection source terms",
        limits.max_projection_source_terms,
        prior.source_terms,
    )?);
    child.max_source_exponent_entries = child.max_source_exponent_entries.min(remaining_limit(
        "exact WhenBad projection source exponent entries",
        limits.max_projection_source_exponent_entries,
        prior.source_exponent_entries,
    )?);
    child.max_source_integer_bits = child.max_source_integer_bits.min(remaining_limit(
        "exact WhenBad projection source integer bits",
        limits.max_projection_source_integer_bits,
        prior.source_integer_bits,
    )?);
    child.max_native_projection_grouping_workspace_byte_envelope = child
        .max_native_projection_grouping_workspace_byte_envelope
        .min(remaining_limit(
            "exact WhenBad projection native workspace bytes",
            limits.max_projection_native_workspace_byte_envelope,
            prior.native_workspace_byte_envelope,
        )?);
    child.max_retained_output_byte_bound =
        child.max_retained_output_byte_bound.min(remaining_limit(
            "exact WhenBad projection retained output bytes",
            limits.max_projection_retained_output_byte_bound,
            prior.retained_output_byte_bound,
        )?);
    child.max_rustred_visible_temporary_byte_envelope = child
        .max_rustred_visible_temporary_byte_envelope
        .min(limits.max_projection_temporary_byte_envelope)
        .min(remaining_outer_compilation_bytes(limits, stats)?);
    Ok(child)
}

fn authenticate_projection_stats(
    prepared: ParametricParameterIdentityProjectionStats,
    observed: ParametricParameterIdentityProjectionStats,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let same = prepared.context_fingerprint_comparison_bytes()
        == observed.context_fingerprint_comparison_bytes()
        && prepared.variable_map_entry_comparisons() == observed.variable_map_entry_comparisons()
        && prepared.source_terms() == observed.source_terms()
        && prepared.source_exponent_entries() == observed.source_exponent_entries()
        && prepared.source_integer_bits() == observed.source_integer_bits()
        && prepared.source_integer_capacity_bytes() == observed.source_integer_capacity_bytes()
        && prepared.projection_variable_mask_comparison_bound()
            == observed.projection_variable_mask_comparison_bound()
        && prepared.projection_hash_key_exponent_entry_bound()
            == observed.projection_hash_key_exponent_entry_bound()
        && prepared.native_projection_grouping_workspace_byte_envelope()
            == observed.native_projection_grouping_workspace_byte_envelope()
        && prepared.projected_physical_monomial_bound()
            == observed.projected_physical_monomial_bound()
        && prepared.projected_outer_exponent_entry_bound()
            == observed.projected_outer_exponent_entry_bound()
        && prepared.projected_coefficient_exponent_entry_bound()
            == observed.projected_coefficient_exponent_entry_bound()
        && prepared.variable_unification_exponent_entry_bound()
            == observed.variable_unification_exponent_entry_bound()
        && prepared.conditional_locus_bound() == observed.conditional_locus_bound()
        && prepared.retained_physical_exponent_entry_bound()
            == observed.retained_physical_exponent_entry_bound()
        && prepared.retained_locus_term_bound() == observed.retained_locus_term_bound()
        && prepared.retained_locus_exponent_entry_bound()
            == observed.retained_locus_exponent_entry_bound()
        && prepared.retained_locus_integer_bit_bound()
            == observed.retained_locus_integer_bit_bound()
        && prepared.transport_coefficient_comparison_term_bound()
            == observed.transport_coefficient_comparison_term_bound()
        && prepared.retained_output_byte_bound() == observed.retained_output_byte_bound()
        && prepared.rustred_visible_temporary_byte_envelope()
            == observed.rustred_visible_temporary_byte_envelope();
    if same {
        Ok(())
    } else {
        Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch)
    }
}

fn authenticate_polynomial_mapping_stats(
    prepared: ResidualUnitAffinePolynomialCompositionStats,
    observed: ResidualUnitAffinePolynomialCompositionStats,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    if execution_polynomial_fits_preflight(observed, prepared) {
        Ok(())
    } else {
        Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch)
    }
}

fn authenticate_coefficient_mapping_stats(
    prepared: ResidualAffineCoefficientCompositionPreflight,
    observed: ResidualUnitAffineCoefficientCompositionStats,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    if execution_coefficient_fits_preflight(observed, prepared) {
        Ok(())
    } else {
        Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch)
    }
}

fn execution_polynomial_fits_preflight(
    actual: ResidualUnitAffinePolynomialCompositionStats,
    prospective: ResidualUnitAffinePolynomialCompositionStats,
) -> bool {
    actual.source_terms() == prospective.source_terms()
        && actual.source_exponent_entries() == prospective.source_exponent_entries()
        && actual.expanded_contribution_bound() == prospective.expanded_contribution_bound()
        && actual.output_exponent_entry_bound() == prospective.output_exponent_entry_bound()
        && actual.power_calls() == prospective.power_calls()
        && actual.native_power_heap_pair_bound() == prospective.native_power_heap_pair_bound()
        && actual.multiplication_term_pair_bound() == prospective.multiplication_term_pair_bound()
        && actual.addition_term_visit_bound() == prospective.addition_term_visit_bound()
        && actual.largest_kronecker_exponent_bits() == prospective.largest_kronecker_exponent_bits()
        && actual.largest_integer_coefficient_bit_bound()
            == prospective.largest_integer_coefficient_bit_bound()
        && actual.native_integer_bit_work_bound() == prospective.native_integer_bit_work_bound()
        && actual.output_terms() <= prospective.expanded_contribution_bound()
        && actual.output_exponent_entries() <= prospective.output_exponent_entry_bound()
        && actual.integer_bit_work_bound() <= prospective.integer_bit_work_bound()
}

fn execution_coefficient_fits_preflight(
    actual: ResidualUnitAffineCoefficientCompositionStats,
    prospective: ResidualAffineCoefficientCompositionPreflight,
) -> bool {
    execution_polynomial_fits_preflight(actual.numerator(), prospective.numerator())
        && execution_polynomial_fits_preflight(actual.denominator(), prospective.denominator())
        && execution_polynomial_fits_preflight(actual.aggregate(), prospective.aggregate())
        && actual.durable_denominator_terms() <= prospective.durable_denominator_term_bound()
        && actual.durable_denominator_exponent_entries()
            <= prospective.durable_denominator_exponent_entry_bound()
        && actual.durable_denominator_integer_bit_payload()
            <= prospective.durable_denominator_integer_bit_payload_bound()
        && actual.normalization_input_term_pairs()
            <= prospective.normalization_input_term_pair_bound()
        && actual.total_integer_bit_work_bound() <= prospective.total_integer_bit_work_bound()
}

fn census_boundary_values(
    plan: &GeneratedAffineResidualGroupExactConditionPlan,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<BoundaryValueAdmission, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    if plan.hazard_schedule().len() != plan.ready().hazards().len() {
        return Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan);
    }
    let exact_limit = Integer::from(limits.max_boundary_values);
    let zero = Integer::from(0);
    let mut total = Integer::from(0);
    let mut cardinality_integer_bit_work = 0usize;
    let mut enumeration_temporary_byte_peak = 0usize;
    for locator in plan.hazard_schedule().iter().copied() {
        let hazard =
            plan.ready().hazards().get(locator.hazard_ordinal()).ok_or(
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan,
            )?;
        if hazard.rhs_ordinal() != locator.rhs_ordinal()
            || hazard.term_ordinal() != locator.term_ordinal()
            || hazard.coordinate() != locator.coordinate()
            || hazard.first().cmp(hazard.last()) == std::cmp::Ordering::Greater
            || hazard.count().cmp(&zero) != std::cmp::Ordering::Greater
        {
            return Err(
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan,
            );
        }
        let total_bits =
            integer_magnitude_bits(&total, "exact WhenBad boundary cardinality integer bits")?;
        let count_bits = integer_magnitude_bits(
            hazard.count(),
            "exact WhenBad boundary cardinality integer bits",
        )?;
        let addition_bits = checked_add(
            "exact WhenBad boundary enumeration integer-bit work",
            total_bits.max(count_bits).max(1),
            1,
        )?;
        cardinality_integer_bit_work = checked_add(
            "exact WhenBad boundary enumeration integer-bit work",
            cardinality_integer_bit_work,
            addition_bits,
        )?;
        check_limit(
            "exact WhenBad boundary enumeration integer-bit work",
            cardinality_integer_bit_work,
            limits.max_boundary_enumeration_integer_bit_work,
        )?;
        let addition_result_bytes = integer_owned_logical_byte_bound(addition_bits)?;
        let addition_live_bytes = checked_add(
            "exact WhenBad boundary enumeration temporary bytes",
            integer_actual_owned_logical_byte_bound(&total)?,
            addition_result_bytes,
        )?;
        check_limit(
            "exact WhenBad materialization compilation owned logical peak",
            addition_live_bytes,
            limits.max_compilation_owned_logical_peak_upper_bound,
        )?;
        enumeration_temporary_byte_peak = enumeration_temporary_byte_peak.max(addition_live_bytes);
        // Symbolica's borrowed arbitrary-precision add avoids an unnecessary
        // clone of the authenticated range cardinality.  The possible old +
        // replacement allocation pair was admitted immediately above.
        total += hazard.count();
        if total.cmp(&exact_limit) == std::cmp::Ordering::Greater {
            return Err(
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::ExactIntegerResourceLimit {
                    resource: "exact WhenBad boundary values",
                    requested: total,
                    limit: limits.max_boundary_values,
                },
            );
        }
    }
    let count = usize::try_from(total).map_err(|_| {
        GeneratedAffineResidualGroupExactWhenBadMaterializationError::ResourceCountOverflow {
            resource: "exact WhenBad boundary values",
        }
    })?;

    // Admit a complete conservative enumeration envelope before cloning the
    // first endpoint. The maximum magnitude on a contiguous integer interval
    // is attained at one of its endpoints.
    let mut upper_value_bits = 0usize;
    let mut upper_value_bytes = 0usize;
    let mut upper_enumeration_work = cardinality_integer_bit_work;
    for locator in plan.hazard_schedule().iter().copied() {
        let hazard = &plan.ready().hazards()[locator.hazard_ordinal()];
        let range_count =
            integer_to_usize_borrowed(hazard.count(), "exact WhenBad boundary range values")?;
        let maximum_bits =
            integer_magnitude_bits(hazard.first(), "exact WhenBad boundary value integer bits")?
                .max(integer_magnitude_bits(
                    hazard.last(),
                    "exact WhenBad boundary value integer bits",
                )?);
        upper_value_bits = checked_add(
            "exact WhenBad boundary value integer bits",
            upper_value_bits,
            checked_mul(
                "exact WhenBad boundary value integer bits",
                range_count,
                maximum_bits,
            )?,
        )?;
        let per_value_bytes = integer_owned_logical_byte_bound(maximum_bits)?
            .max(integer_actual_owned_logical_byte_bound(hazard.first())?)
            .max(integer_actual_owned_logical_byte_bound(hazard.last())?);
        let per_value_bytes = checked_add(
            "exact WhenBad boundary value retained logical bytes",
            per_value_bytes,
            size_of::<usize>(),
        )?;
        upper_value_bytes = checked_add(
            "exact WhenBad boundary value retained logical bytes",
            upper_value_bytes,
            checked_mul(
                "exact WhenBad boundary value retained logical bytes",
                range_count,
                per_value_bytes,
            )?,
        )?;
        let per_increment_work = checked_add(
            "exact WhenBad boundary enumeration integer-bit work",
            checked_mul(
                "exact WhenBad boundary enumeration integer-bit work",
                maximum_bits.max(1),
                2,
            )?,
            1,
        )?;
        upper_enumeration_work = checked_add(
            "exact WhenBad boundary enumeration integer-bit work",
            upper_enumeration_work,
            checked_mul(
                "exact WhenBad boundary enumeration integer-bit work",
                range_count,
                per_increment_work,
            )?,
        )?;
        enumeration_temporary_byte_peak = enumeration_temporary_byte_peak.max(checked_mul(
            "exact WhenBad boundary enumeration temporary bytes",
            per_value_bytes,
            2,
        )?);
    }
    check_limit(
        "exact WhenBad boundary value integer bits",
        upper_value_bits,
        limits.max_boundary_value_integer_bits,
    )?;
    check_limit(
        "exact WhenBad boundary value retained logical bytes",
        upper_value_bytes,
        limits.max_boundary_value_retained_logical_bytes,
    )?;
    check_limit(
        "exact WhenBad boundary enumeration integer-bit work",
        upper_enumeration_work,
        limits.max_boundary_enumeration_integer_bit_work,
    )?;
    check_limit(
        "exact WhenBad materialization compilation owned logical peak",
        enumeration_temporary_byte_peak,
        limits.max_compilation_owned_logical_peak_upper_bound,
    )?;

    let stats = GeneratedAffineResidualGroupExactWhenBadBoundaryStats {
        value_integer_bits: upper_value_bits,
        value_retained_logical_bytes: upper_value_bytes,
        enumeration_integer_bit_work: upper_enumeration_work,
        ..GeneratedAffineResidualGroupExactWhenBadBoundaryStats::default()
    };
    check_boundary_stats(stats, limits)?;
    Ok(BoundaryValueAdmission {
        count,
        stats,
        enumeration_temporary_byte_peak,
    })
}

fn materialize_boundaries(
    context: &ParametricCoefficientContext,
    plan: &GeneratedAffineResidualGroupExactConditionPlan,
    sources: &[GeneratedAffineResidualGroupExactMappedSource],
    coefficient_source_by_term: &[Option<usize>],
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    stats: &mut GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
    boundaries: &mut Vec<GeneratedAffineResidualGroupExactBoundaryEvent>,
) -> Result<
    Option<GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness>,
    GeneratedAffineResidualGroupExactWhenBadMaterializationError,
> {
    let mut observed_enumeration = ObservedBoundaryEnumeration::default();
    for locator in plan.hazard_schedule().iter().copied() {
        let hazard =
            plan.ready().hazards().get(locator.hazard_ordinal()).ok_or(
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan,
            )?;
        if hazard.rhs_ordinal() != locator.rhs_ordinal()
            || hazard.term_ordinal() != locator.term_ordinal()
            || hazard.coordinate() != locator.coordinate()
        {
            return Err(
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan,
            );
        }
        let source_ordinal = coefficient_source_by_term
            .get(locator.term_ordinal())
            .and_then(|value| *value)
            .ok_or(GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan)?;
        let coefficient = sources
            .get(source_ordinal)
            .and_then(GeneratedAffineResidualGroupExactMappedSource::coefficient)
            .ok_or(GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan)?;
        if coefficient.source
            != (GeneratedAffineResidualGroupExactConditionSourceLocator::RhsCoefficient {
                rhs_ordinal: locator.rhs_ordinal(),
                term_ordinal: locator.term_ordinal(),
            })
        {
            return Err(
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan,
            );
        }

        let mut value = hazard.first().clone();
        loop {
            let is_last = &value == hazard.last();
            observe_boundary_enumeration_value(&mut observed_enumeration, &value, is_last)?;
            refresh_compilation_peak(stats, limits)?;
            let next_value = if is_last {
                None
            } else {
                Some(hazard_value_successor(&value))
            };

            let mapping_limits = remaining_boundary_mapping_limits(
                limits,
                *stats,
                plan.compact_target_transform().is_some(),
            )?;
            let prepared = context.prepare_residual_affine_boundary_mapping(
                locator.coordinate(),
                &value,
                plan.compact_target_transform(),
                mapping_limits,
            )?;
            let prospective = prepared.stats();
            admit_boundary_mapping_stats(&mut stats.boundary, prospective, limits)?;
            if let Some(composition) = prospective.composition() {
                stats.mapping.source_integer_bits = checked_add(
                    "exact WhenBad mapping source integer bits",
                    stats.mapping.source_integer_bits,
                    prospective.constructed_integer_bits(),
                )?;
                admit_polynomial_mapping_stats(&mut stats.mapping, composition, limits)?;
            }
            refresh_compilation_peak(stats, limits)?;
            let mapping = prepared.execute()?;
            let (class, observed_mapping) = mapping.into_parts();
            authenticate_boundary_mapping_stats(prospective, observed_mapping)?;

            let ordinal = stats.boundary_values;
            stats.boundary_values = checked_add(
                "exact WhenBad materialized boundary values",
                stats.boundary_values,
                1,
            )?;
            check_limit(
                "exact WhenBad materialized boundary values",
                stats.boundary_values,
                stats.admitted_boundary_values,
            )?;

            match class {
                ResidualAffineMappedBoundaryClass::Empty => {
                    stats.empty_boundaries =
                        checked_add("exact WhenBad empty boundaries", stats.empty_boundaries, 1)?;
                    boundaries.push(GeneratedAffineResidualGroupExactBoundaryEvent {
                        ordinal,
                        source: locator,
                        value,
                        disposition: GeneratedAffineResidualGroupExactBoundaryDisposition::Empty,
                        boundary: None,
                        mapping_stats: observed_mapping,
                        numerator_stats: None,
                    });
                }
                ResidualAffineMappedBoundaryClass::WholeTarget => {
                    stats.whole_target_boundaries = checked_add(
                        "exact WhenBad whole-target boundaries",
                        stats.whole_target_boundaries,
                        1,
                    )?;
                    let event = GeneratedAffineResidualGroupExactBoundaryEvent {
                        ordinal,
                        source: locator,
                        value,
                        disposition:
                            GeneratedAffineResidualGroupExactBoundaryDisposition::WholeTarget,
                        boundary: None,
                        mapping_stats: observed_mapping,
                        numerator_stats: None,
                    };
                    authenticate_observed_boundary_enumeration(
                        observed_enumeration,
                        *stats,
                        false,
                    )?;
                    return Ok(Some(
                        GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness::WholeTargetInactiveActivation {
                            event,
                        },
                    ));
                }
                ResidualAffineMappedBoundaryClass::IndexDependentAffine { polynomial } => {
                    let numerator_limits = remaining_boundary_numerator_limits(limits, *stats)?;
                    let prepared = context
                        .prepare_residual_affine_boundary_numerator_classification(
                            &polynomial,
                            coefficient.normalized_numerator(),
                            numerator_limits,
                        )?;
                    let prospective_numerator = prepared.stats();
                    admit_boundary_numerator_stats(
                        &mut stats.boundary,
                        prospective_numerator,
                        limits,
                    )?;
                    refresh_compilation_peak(stats, limits)?;
                    let classification = prepared.execute()?;
                    let (numerator_disposition, observed_numerator) = classification.into_parts();
                    authenticate_boundary_numerator_stats(
                        prospective_numerator,
                        observed_numerator,
                    )?;
                    let disposition = match numerator_disposition {
                        ResidualAffineBoundaryNumeratorDisposition::Suppressed => {
                            stats.suppressed_boundaries = checked_add(
                                "exact WhenBad numerator-suppressed boundaries",
                                stats.suppressed_boundaries,
                                1,
                            )?;
                            GeneratedAffineResidualGroupExactBoundaryDisposition::SuppressedByNumerator
                        }
                        ResidualAffineBoundaryNumeratorDisposition::Retained => {
                            stats.retained_boundaries = checked_add(
                                "exact WhenBad retained bad boundaries",
                                stats.retained_boundaries,
                                1,
                            )?;
                            GeneratedAffineResidualGroupExactBoundaryDisposition::RetainedBadBoundary
                        }
                    };
                    boundaries.push(GeneratedAffineResidualGroupExactBoundaryEvent {
                        ordinal,
                        source: locator,
                        value,
                        disposition,
                        boundary: Some(polynomial),
                        mapping_stats: observed_mapping,
                        numerator_stats: Some(observed_numerator),
                    });
                }
            }
            if is_last {
                break;
            }
            value = next_value.ok_or(
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::MalformedPlan,
            )?;
        }
    }
    authenticate_observed_boundary_enumeration(observed_enumeration, *stats, true)?;
    Ok(None)
}

fn hazard_value_successor(value: &Integer) -> Integer {
    value + 1_i64
}

fn observe_boundary_enumeration_value(
    observed: &mut ObservedBoundaryEnumeration,
    value: &Integer,
    is_last: bool,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let bits = integer_magnitude_bits(value, "exact WhenBad boundary value integer bits")?;
    observed.count = checked_add("exact WhenBad observed boundary values", observed.count, 1)?;
    observed.value_integer_bits = checked_add(
        "exact WhenBad observed boundary value integer bits",
        observed.value_integer_bits,
        bits,
    )?;
    observed.value_retained_logical_bytes = checked_add(
        "exact WhenBad observed boundary value retained bytes",
        observed.value_retained_logical_bytes,
        integer_actual_owned_logical_byte_bound(value)?,
    )?;
    let work = if is_last {
        bits.max(1)
    } else {
        checked_add(
            "exact WhenBad observed boundary enumeration integer-bit work",
            checked_mul(
                "exact WhenBad observed boundary enumeration integer-bit work",
                bits.max(1),
                2,
            )?,
            1,
        )?
    };
    observed.enumeration_integer_bit_work = checked_add(
        "exact WhenBad observed boundary enumeration integer-bit work",
        observed.enumeration_integer_bit_work,
        work,
    )?;
    Ok(())
}

fn authenticate_observed_boundary_enumeration(
    observed: ObservedBoundaryEnumeration,
    stats: GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
    complete: bool,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let admitted = stats.boundary;
    let fits = observed.count <= stats.admitted_boundary_values
        && observed.count == stats.boundary_values
        && observed.value_integer_bits <= admitted.value_integer_bits
        && observed.value_retained_logical_bytes <= admitted.value_retained_logical_bytes
        && observed.enumeration_integer_bit_work <= admitted.enumeration_integer_bit_work
        && (!complete || observed.count == stats.admitted_boundary_values);
    if fits {
        Ok(())
    } else {
        Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch)
    }
}

fn admit_boundary_mapping_stats(
    aggregate: &mut GeneratedAffineResidualGroupExactWhenBadBoundaryStats,
    value: ResidualAffineBoundaryKernelStats,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    macro_rules! sum {
        ($field:ident, $getter:ident, $resource:literal) => {
            aggregate.$field = checked_add($resource, aggregate.$field, value.$getter())?;
        };
    }
    sum!(
        mapping_constructed_terms,
        constructed_terms,
        "exact WhenBad boundary mapping constructed terms"
    );
    sum!(
        mapping_constructed_exponent_entries,
        constructed_exponent_entries,
        "exact WhenBad boundary mapping constructed exponent entries"
    );
    sum!(
        mapping_constructed_integer_bits,
        constructed_integer_bits,
        "exact WhenBad boundary mapping constructed integer bits"
    );
    sum!(
        mapping_mapped_term_bound,
        mapped_term_bound,
        "exact WhenBad boundary mapping mapped term bound"
    );
    sum!(
        mapping_mapped_exponent_entry_bound,
        mapped_exponent_entry_bound,
        "exact WhenBad boundary mapping mapped exponent-entry bound"
    );
    sum!(
        mapping_mapped_integer_bit_bound,
        mapped_integer_bit_bound,
        "exact WhenBad boundary mapping mapped integer-bit bound"
    );
    sum!(
        mapping_affine_term_visits,
        affine_authentication_term_visit_bound,
        "exact WhenBad boundary mapping affine term visits"
    );
    sum!(
        mapping_affine_exponent_visits,
        affine_authentication_exponent_entry_visit_bound,
        "exact WhenBad boundary mapping affine exponent visits"
    );
    sum!(
        mapping_retained_output_byte_bound,
        retained_output_byte_bound,
        "exact WhenBad boundary mapping retained output bytes"
    );
    aggregate.mapping_constructed_source_temporary_byte_peak = aggregate
        .mapping_constructed_source_temporary_byte_peak
        .max(value.constructed_source_retained_byte_bound());
    aggregate.mapping_child_compilation_byte_peak = aggregate
        .mapping_child_compilation_byte_peak
        .max(value.rustred_visible_compilation_peak_byte_bound());
    check_boundary_stats(*aggregate, limits)
}

fn admit_boundary_numerator_stats(
    aggregate: &mut GeneratedAffineResidualGroupExactWhenBadBoundaryStats,
    value: ResidualAffineBoundaryNumeratorStats,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    macro_rules! sum {
        ($field:ident, $getter:ident, $resource:literal) => {
            aggregate.$field = checked_add($resource, aggregate.$field, value.$getter())?;
        };
    }
    sum!(
        numerator_boundary_terms,
        boundary_terms,
        "exact WhenBad boundary numerator boundary terms"
    );
    sum!(
        numerator_boundary_exponent_entries,
        boundary_exponent_entries,
        "exact WhenBad boundary numerator boundary exponent entries"
    );
    sum!(
        numerator_boundary_integer_bits,
        boundary_integer_bits,
        "exact WhenBad boundary numerator boundary integer bits"
    );
    sum!(
        numerator_numerator_terms,
        numerator_terms,
        "exact WhenBad boundary numerator terms"
    );
    sum!(
        numerator_numerator_exponent_entries,
        numerator_exponent_entries,
        "exact WhenBad boundary numerator exponent entries"
    );
    sum!(
        numerator_numerator_integer_bits,
        numerator_integer_bits,
        "exact WhenBad boundary numerator integer bits"
    );
    sum!(
        numerator_affine_term_visits,
        affine_authentication_term_visits,
        "exact WhenBad boundary numerator affine term visits"
    );
    sum!(
        numerator_affine_exponent_visits,
        affine_authentication_exponent_entry_visits,
        "exact WhenBad boundary numerator affine exponent visits"
    );
    sum!(
        numerator_divisibility_term_pairs,
        divisibility_input_term_pair_bound,
        "exact WhenBad boundary numerator divisibility term pairs"
    );
    sum!(
        numerator_divisibility_calls,
        divisibility_call_bound,
        "exact WhenBad boundary numerator divisibility calls"
    );
    sum!(
        numerator_retained_owned_logical_bytes,
        retained_owned_logical_bytes,
        "exact WhenBad boundary numerator retained logical bytes"
    );
    aggregate.numerator_source_copy_temporary_byte_peak = aggregate
        .numerator_source_copy_temporary_byte_peak
        .max(value.source_copy_temporary_byte_bound());
    check_boundary_stats(*aggregate, limits)
}

fn check_boundary_stats(
    stats: GeneratedAffineResidualGroupExactWhenBadBoundaryStats,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    for (resource, requested, limit) in [
        (
            "exact WhenBad boundary value integer bits",
            stats.value_integer_bits,
            limits.max_boundary_value_integer_bits,
        ),
        (
            "exact WhenBad boundary value retained logical bytes",
            stats.value_retained_logical_bytes,
            limits.max_boundary_value_retained_logical_bytes,
        ),
        (
            "exact WhenBad boundary enumeration integer-bit work",
            stats.enumeration_integer_bit_work,
            limits.max_boundary_enumeration_integer_bit_work,
        ),
        (
            "exact WhenBad boundary mapping constructed terms",
            stats.mapping_constructed_terms,
            limits.max_boundary_mapping_constructed_terms,
        ),
        (
            "exact WhenBad boundary mapping constructed exponent entries",
            stats.mapping_constructed_exponent_entries,
            limits.max_boundary_mapping_constructed_exponent_entries,
        ),
        (
            "exact WhenBad boundary mapping constructed integer bits",
            stats.mapping_constructed_integer_bits,
            limits.max_boundary_mapping_constructed_integer_bits,
        ),
        (
            "exact WhenBad boundary mapping mapped term bound",
            stats.mapping_mapped_term_bound,
            limits.max_boundary_mapping_mapped_term_bound,
        ),
        (
            "exact WhenBad boundary mapping mapped exponent-entry bound",
            stats.mapping_mapped_exponent_entry_bound,
            limits.max_boundary_mapping_mapped_exponent_entry_bound,
        ),
        (
            "exact WhenBad boundary mapping mapped integer-bit bound",
            stats.mapping_mapped_integer_bit_bound,
            limits.max_boundary_mapping_mapped_integer_bit_bound,
        ),
        (
            "exact WhenBad boundary mapping affine term visits",
            stats.mapping_affine_term_visits,
            limits.max_boundary_mapping_affine_term_visits,
        ),
        (
            "exact WhenBad boundary mapping affine exponent visits",
            stats.mapping_affine_exponent_visits,
            limits.max_boundary_mapping_affine_exponent_visits,
        ),
        (
            "exact WhenBad boundary mapping retained output bytes",
            stats.mapping_retained_output_byte_bound,
            limits.max_boundary_mapping_retained_output_byte_bound,
        ),
        (
            "exact WhenBad boundary mapping constructed-source temporary peak",
            stats.mapping_constructed_source_temporary_byte_peak,
            limits.max_boundary_mapping_constructed_source_temporary_byte_peak,
        ),
        (
            "exact WhenBad boundary mapping child compilation peak",
            stats.mapping_child_compilation_byte_peak,
            limits.max_boundary_mapping_child_compilation_byte_peak,
        ),
        (
            "exact WhenBad boundary numerator boundary terms",
            stats.numerator_boundary_terms,
            limits.max_boundary_numerator_boundary_terms,
        ),
        (
            "exact WhenBad boundary numerator boundary exponent entries",
            stats.numerator_boundary_exponent_entries,
            limits.max_boundary_numerator_boundary_exponent_entries,
        ),
        (
            "exact WhenBad boundary numerator boundary integer bits",
            stats.numerator_boundary_integer_bits,
            limits.max_boundary_numerator_boundary_integer_bits,
        ),
        (
            "exact WhenBad boundary numerator terms",
            stats.numerator_numerator_terms,
            limits.max_boundary_numerator_numerator_terms,
        ),
        (
            "exact WhenBad boundary numerator exponent entries",
            stats.numerator_numerator_exponent_entries,
            limits.max_boundary_numerator_numerator_exponent_entries,
        ),
        (
            "exact WhenBad boundary numerator integer bits",
            stats.numerator_numerator_integer_bits,
            limits.max_boundary_numerator_numerator_integer_bits,
        ),
        (
            "exact WhenBad boundary numerator affine term visits",
            stats.numerator_affine_term_visits,
            limits.max_boundary_numerator_affine_term_visits,
        ),
        (
            "exact WhenBad boundary numerator affine exponent visits",
            stats.numerator_affine_exponent_visits,
            limits.max_boundary_numerator_affine_exponent_visits,
        ),
        (
            "exact WhenBad boundary numerator divisibility term pairs",
            stats.numerator_divisibility_term_pairs,
            limits.max_boundary_numerator_divisibility_term_pairs,
        ),
        (
            "exact WhenBad boundary numerator divisibility calls",
            stats.numerator_divisibility_calls,
            limits.max_boundary_numerator_divisibility_calls,
        ),
        (
            "exact WhenBad boundary numerator source-copy temporary peak",
            stats.numerator_source_copy_temporary_byte_peak,
            limits.max_boundary_numerator_source_copy_temporary_byte_peak,
        ),
        (
            "exact WhenBad boundary numerator retained logical bytes",
            stats.numerator_retained_owned_logical_bytes,
            limits.max_boundary_numerator_retained_owned_logical_bytes,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    Ok(())
}

fn remaining_boundary_mapping_limits(
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    stats: GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
    compact_mapping: bool,
) -> Result<
    ResidualAffineBoundaryKernelLimits,
    GeneratedAffineResidualGroupExactWhenBadMaterializationError,
> {
    let mut child = limits.boundary_mapping;
    child.composition = remaining_composition_limits(limits, stats.mapping)?;
    macro_rules! remaining_sum {
        ($child:ident, $limit:ident, $used:ident, $resource:literal) => {
            child.$child = child.$child.min(remaining_limit(
                $resource,
                limits.$limit,
                stats.boundary.$used,
            )?);
        };
    }
    remaining_sum!(
        max_constructed_terms,
        max_boundary_mapping_constructed_terms,
        mapping_constructed_terms,
        "exact WhenBad boundary mapping constructed terms"
    );
    remaining_sum!(
        max_constructed_exponent_entries,
        max_boundary_mapping_constructed_exponent_entries,
        mapping_constructed_exponent_entries,
        "exact WhenBad boundary mapping constructed exponent entries"
    );
    remaining_sum!(
        max_constructed_integer_bits,
        max_boundary_mapping_constructed_integer_bits,
        mapping_constructed_integer_bits,
        "exact WhenBad boundary mapping constructed integer bits"
    );
    remaining_sum!(
        max_mapped_term_bound,
        max_boundary_mapping_mapped_term_bound,
        mapping_mapped_term_bound,
        "exact WhenBad boundary mapping mapped term bound"
    );
    remaining_sum!(
        max_mapped_exponent_entry_bound,
        max_boundary_mapping_mapped_exponent_entry_bound,
        mapping_mapped_exponent_entry_bound,
        "exact WhenBad boundary mapping mapped exponent-entry bound"
    );
    remaining_sum!(
        max_mapped_integer_bit_bound,
        max_boundary_mapping_mapped_integer_bit_bound,
        mapping_mapped_integer_bit_bound,
        "exact WhenBad boundary mapping mapped integer-bit bound"
    );
    remaining_sum!(
        max_affine_authentication_term_visit_bound,
        max_boundary_mapping_affine_term_visits,
        mapping_affine_term_visits,
        "exact WhenBad boundary mapping affine term visits"
    );
    remaining_sum!(
        max_affine_authentication_exponent_entry_visit_bound,
        max_boundary_mapping_affine_exponent_visits,
        mapping_affine_exponent_visits,
        "exact WhenBad boundary mapping affine exponent visits"
    );
    remaining_sum!(
        max_retained_output_byte_bound,
        max_boundary_mapping_retained_output_byte_bound,
        mapping_retained_output_byte_bound,
        "exact WhenBad boundary mapping retained output bytes"
    );
    if compact_mapping {
        child.max_constructed_integer_bits =
            child.max_constructed_integer_bits.min(remaining_limit(
                "exact WhenBad mapping source integer bits",
                limits.max_mapping_source_integer_bits,
                stats.mapping.source_integer_bits,
            )?);
    }
    child.max_constructed_source_retained_byte_bound = child
        .max_constructed_source_retained_byte_bound
        .min(limits.max_boundary_mapping_constructed_source_temporary_byte_peak)
        .min(remaining_outer_compilation_bytes(limits, stats)?);
    child.max_rustred_visible_compilation_peak_byte_bound = child
        .max_rustred_visible_compilation_peak_byte_bound
        .min(limits.max_boundary_mapping_child_compilation_byte_peak)
        .min(remaining_outer_compilation_bytes(limits, stats)?);
    Ok(child)
}

fn remaining_boundary_numerator_limits(
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    stats: GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
) -> Result<
    ResidualAffineBoundaryNumeratorLimits,
    GeneratedAffineResidualGroupExactWhenBadMaterializationError,
> {
    let mut child = limits.boundary_numerator;
    macro_rules! remaining_sum {
        ($child:ident, $limit:ident, $used:ident, $resource:literal) => {
            child.$child = child.$child.min(remaining_limit(
                $resource,
                limits.$limit,
                stats.boundary.$used,
            )?);
        };
    }
    remaining_sum!(
        max_boundary_terms,
        max_boundary_numerator_boundary_terms,
        numerator_boundary_terms,
        "exact WhenBad boundary numerator boundary terms"
    );
    remaining_sum!(
        max_boundary_exponent_entries,
        max_boundary_numerator_boundary_exponent_entries,
        numerator_boundary_exponent_entries,
        "exact WhenBad boundary numerator boundary exponent entries"
    );
    remaining_sum!(
        max_boundary_integer_bits,
        max_boundary_numerator_boundary_integer_bits,
        numerator_boundary_integer_bits,
        "exact WhenBad boundary numerator boundary integer bits"
    );
    remaining_sum!(
        max_numerator_terms,
        max_boundary_numerator_numerator_terms,
        numerator_numerator_terms,
        "exact WhenBad boundary numerator terms"
    );
    remaining_sum!(
        max_numerator_exponent_entries,
        max_boundary_numerator_numerator_exponent_entries,
        numerator_numerator_exponent_entries,
        "exact WhenBad boundary numerator exponent entries"
    );
    remaining_sum!(
        max_numerator_integer_bits,
        max_boundary_numerator_numerator_integer_bits,
        numerator_numerator_integer_bits,
        "exact WhenBad boundary numerator integer bits"
    );
    remaining_sum!(
        max_affine_authentication_term_visits,
        max_boundary_numerator_affine_term_visits,
        numerator_affine_term_visits,
        "exact WhenBad boundary numerator affine term visits"
    );
    remaining_sum!(
        max_affine_authentication_exponent_entry_visits,
        max_boundary_numerator_affine_exponent_visits,
        numerator_affine_exponent_visits,
        "exact WhenBad boundary numerator affine exponent visits"
    );
    remaining_sum!(
        max_divisibility_input_term_pair_bound,
        max_boundary_numerator_divisibility_term_pairs,
        numerator_divisibility_term_pairs,
        "exact WhenBad boundary numerator divisibility term pairs"
    );
    remaining_sum!(
        max_divisibility_call_bound,
        max_boundary_numerator_divisibility_calls,
        numerator_divisibility_calls,
        "exact WhenBad boundary numerator divisibility calls"
    );
    remaining_sum!(
        max_retained_owned_logical_bytes,
        max_boundary_numerator_retained_owned_logical_bytes,
        numerator_retained_owned_logical_bytes,
        "exact WhenBad boundary numerator retained logical bytes"
    );
    child.max_source_copy_temporary_byte_bound = child
        .max_source_copy_temporary_byte_bound
        .min(limits.max_boundary_numerator_source_copy_temporary_byte_peak)
        .min(remaining_outer_compilation_bytes(limits, stats)?);
    Ok(child)
}

fn authenticate_boundary_mapping_stats(
    prepared: ResidualAffineBoundaryKernelStats,
    observed: ResidualAffineBoundaryKernelStats,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let composition_matches = match (prepared.composition(), observed.composition()) {
        (None, None) => true,
        (Some(left), Some(right)) => execution_polynomial_fits_preflight(right, left),
        _ => false,
    };
    let stable = prepared.context_fingerprint_comparison_bytes()
        == observed.context_fingerprint_comparison_bytes()
        && prepared.ambient_arity() == observed.ambient_arity()
        && prepared.boundary_value_integer_bits() == observed.boundary_value_integer_bits()
        && prepared.construction_symbolica_calls() == observed.construction_symbolica_calls()
        && prepared.constructed_terms() == observed.constructed_terms()
        && prepared.constructed_exponent_entries() == observed.constructed_exponent_entries()
        && prepared.constructed_integer_bits() == observed.constructed_integer_bits()
        && prepared.constructed_source_retained_byte_bound()
            == observed.constructed_source_retained_byte_bound()
        && prepared.mapped_term_bound() == observed.mapped_term_bound()
        && prepared.mapped_exponent_entry_bound() == observed.mapped_exponent_entry_bound()
        && prepared.mapped_integer_bit_bound() == observed.mapped_integer_bit_bound()
        && prepared.affine_authentication_term_visit_bound()
            == observed.affine_authentication_term_visit_bound()
        && prepared.affine_authentication_exponent_entry_visit_bound()
            == observed.affine_authentication_exponent_entry_visit_bound()
        && prepared.identity_copy_retained_byte_bound()
            == observed.identity_copy_retained_byte_bound()
        && prepared.retained_output_byte_bound() == observed.retained_output_byte_bound()
        && prepared.rustred_visible_compilation_peak_byte_bound()
            == observed.rustred_visible_compilation_peak_byte_bound()
        && observed.mapped_terms() <= prepared.mapped_term_bound()
        && observed.mapped_exponent_entries() <= prepared.mapped_exponent_entry_bound()
        && observed.mapped_integer_bits() <= prepared.mapped_integer_bit_bound()
        && observed.retained_output_bytes() <= prepared.retained_output_byte_bound();
    if composition_matches && stable {
        Ok(())
    } else {
        Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch)
    }
}

fn authenticate_boundary_numerator_stats(
    prepared: ResidualAffineBoundaryNumeratorStats,
    observed: ResidualAffineBoundaryNumeratorStats,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let stable = prepared.context_fingerprint_comparison_bytes()
        == observed.context_fingerprint_comparison_bytes()
        && prepared.boundary_terms() == observed.boundary_terms()
        && prepared.boundary_exponent_entries() == observed.boundary_exponent_entries()
        && prepared.boundary_integer_bits() == observed.boundary_integer_bits()
        && prepared.numerator_terms() == observed.numerator_terms()
        && prepared.numerator_exponent_entries() == observed.numerator_exponent_entries()
        && prepared.numerator_integer_bits() == observed.numerator_integer_bits()
        && prepared.affine_authentication_term_visits()
            == observed.affine_authentication_term_visits()
        && prepared.affine_authentication_exponent_entry_visits()
            == observed.affine_authentication_exponent_entry_visits()
        && prepared.divisibility_input_term_pair_bound()
            == observed.divisibility_input_term_pair_bound()
        && prepared.divisibility_call_bound() == observed.divisibility_call_bound()
        && prepared.source_copy_temporary_byte_bound()
            == observed.source_copy_temporary_byte_bound()
        && prepared.retained_owned_logical_bytes() == observed.retained_owned_logical_bytes()
        && observed.divisibility_calls() <= prepared.divisibility_call_bound();
    if stable {
        Ok(())
    } else {
        Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch)
    }
}

fn finish_ready(
    sources: Vec<GeneratedAffineResidualGroupExactMappedSource>,
    boundaries: Vec<GeneratedAffineResidualGroupExactBoundaryEvent>,
    mut stats: GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<PreparedMaterialization, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let retained = retained_materialization_payload_bound(&sources, &boundaries, None)?;
    finalize_materialization_stats(&mut stats, retained, 0, limits)?;
    Ok(PreparedMaterialization::Ready {
        sources,
        boundaries,
        stats,
    })
}

fn finish_identically_bad(
    sources: Vec<GeneratedAffineResidualGroupExactMappedSource>,
    boundaries: Vec<GeneratedAffineResidualGroupExactBoundaryEvent>,
    witness: GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness,
    mut stats: GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<PreparedMaterialization, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let retained = retained_materialization_payload_bound(&sources, &boundaries, Some(&witness))?;
    finalize_materialization_stats(
        &mut stats,
        retained,
        size_of::<GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness>(),
        limits,
    )?;
    Ok(PreparedMaterialization::IdenticallyBad {
        sources,
        boundaries,
        witness,
        stats,
    })
}

fn finalize_materialization_stats(
    stats: &mut GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
    observed_retained: usize,
    terminal_witness_header_bytes: usize,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    if stats.active_normalized_denominator_temporary_byte_bound != 0 {
        return Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch);
    }
    let child_retained = [
        stats.boundary.value_retained_logical_bytes,
        stats.projection.retained_output_byte_bound,
        stats.boundary.mapping_retained_output_byte_bound,
        stats.boundary.numerator_retained_owned_logical_bytes,
        terminal_witness_header_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        checked_add(
            "exact WhenBad materialization retained child bytes",
            sum,
            bytes,
        )
    })?;
    let retained = checked_add(
        "exact WhenBad materialization retained owned logical bytes",
        stats.source_phase_retained_logical_byte_bound,
        child_retained,
    )?;
    if observed_retained > retained {
        return Err(GeneratedAffineResidualGroupExactWhenBadMaterializationError::ReplayMismatch);
    }
    check_limit(
        "exact WhenBad materialization retained owned logical bytes",
        retained,
        limits.max_retained_owned_logical_bytes,
    )?;
    stats.retained_owned_logical_bytes = retained;
    refresh_compilation_peak(stats, limits)?;
    let child_peak = stats
        .projection
        .temporary_byte_envelope
        .max(
            stats
                .boundary
                .mapping_constructed_source_temporary_byte_peak,
        )
        .max(stats.boundary.mapping_child_compilation_byte_peak)
        .max(stats.boundary.numerator_source_copy_temporary_byte_peak);
    stats.compilation_owned_logical_peak_upper_bound = stats
        .compilation_owned_logical_peak_upper_bound
        .max(checked_add(
            "exact WhenBad materialization compilation owned logical peak",
            retained,
            child_peak,
        )?);
    check_limit(
        "exact WhenBad materialization compilation owned logical peak",
        stats.compilation_owned_logical_peak_upper_bound,
        limits.max_compilation_owned_logical_peak_upper_bound,
    )?;
    // No fallible child operation follows finalization.  The transaction-only
    // arenas are dropped as `prepare_materialization` returns and therefore
    // must not survive in the replayed durable stats.
    stats.active_transaction_arena_byte_bound = 0;
    Ok(())
}

fn refresh_compilation_peak(
    stats: &mut GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let durable_children = [
        stats.projection.retained_output_byte_bound,
        stats.boundary.value_retained_logical_bytes,
        stats.boundary.mapping_retained_output_byte_bound,
        stats.boundary.numerator_retained_owned_logical_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        checked_add(
            "exact WhenBad materialization live retained child bytes",
            sum,
            bytes,
        )
    })?;
    let durable_live = checked_add(
        "exact WhenBad materialization live retained bytes",
        stats.source_phase_retained_logical_byte_bound,
        durable_children,
    )?;
    check_limit(
        "exact WhenBad materialization retained owned logical bytes",
        durable_live,
        limits.max_retained_owned_logical_bytes,
    )?;
    let scratch_peak = stats
        .projection
        .temporary_byte_envelope
        .max(
            stats
                .boundary
                .mapping_constructed_source_temporary_byte_peak,
        )
        .max(stats.boundary.mapping_child_compilation_byte_peak)
        .max(stats.boundary.numerator_source_copy_temporary_byte_peak);
    let active_source_and_scratch = checked_add(
        "exact WhenBad materialization active source temporary bytes",
        stats.active_normalized_denominator_temporary_byte_bound,
        scratch_peak,
    )?;
    let active_and_scratch = checked_add(
        "exact WhenBad materialization active transaction arena bytes",
        stats.active_transaction_arena_byte_bound,
        active_source_and_scratch,
    )?;
    let candidate = checked_add(
        "exact WhenBad materialization compilation owned logical peak",
        durable_live,
        active_and_scratch,
    )?;
    stats.compilation_owned_logical_peak_upper_bound = stats
        .compilation_owned_logical_peak_upper_bound
        .max(candidate);
    check_limit(
        "exact WhenBad materialization compilation owned logical peak",
        stats.compilation_owned_logical_peak_upper_bound,
        limits.max_compilation_owned_logical_peak_upper_bound,
    )
}

fn remaining_outer_compilation_bytes(
    limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    stats: GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let durable_children = [
        stats.projection.retained_output_byte_bound,
        stats.boundary.value_retained_logical_bytes,
        stats.boundary.mapping_retained_output_byte_bound,
        stats.boundary.numerator_retained_owned_logical_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        checked_add(
            "exact WhenBad materialization live retained child bytes",
            sum,
            bytes,
        )
    })?;
    let durable_live = checked_add(
        "exact WhenBad materialization live retained bytes",
        stats.source_phase_retained_logical_byte_bound,
        durable_children,
    )?;
    let live_with_source = checked_add(
        "exact WhenBad materialization active source temporary bytes",
        durable_live,
        stats.active_normalized_denominator_temporary_byte_bound,
    )?;
    let live = checked_add(
        "exact WhenBad materialization active transaction arena bytes",
        live_with_source,
        stats.active_transaction_arena_byte_bound,
    )?;
    remaining_limit(
        "exact WhenBad materialization compilation owned logical peak",
        limits.max_compilation_owned_logical_peak_upper_bound,
        live,
    )
}

fn retained_materialization_payload_bound(
    sources: &[GeneratedAffineResidualGroupExactMappedSource],
    boundaries: &[GeneratedAffineResidualGroupExactBoundaryEvent],
    witness: Option<&GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness>,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let mut bytes = size_of::<PreparedMaterialization>();
    for source in sources {
        bytes = checked_add(
            "exact WhenBad retained mapped sources",
            bytes,
            mapped_source_owned_retained_byte_bound(source)?,
        )?;
    }
    for event in boundaries {
        bytes = checked_add(
            "exact WhenBad retained boundary events",
            bytes,
            boundary_event_owned_retained_byte_bound(event)?,
        )?;
    }
    if let Some(witness) = witness {
        bytes = checked_add(
            "exact WhenBad retained decisive witness",
            bytes,
            identically_bad_witness_owned_retained_byte_bound(witness)?,
        )?;
    }
    Ok(bytes)
}

fn mapped_source_owned_retained_byte_bound(
    source: &GeneratedAffineResidualGroupExactMappedSource,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    match source {
        GeneratedAffineResidualGroupExactMappedSource::Condition(value) => checked_add(
            "exact WhenBad retained mapped condition",
            size_of::<GeneratedAffineResidualGroupExactMappedCondition>(),
            polynomial_retained_bound(&value.polynomial)?,
        ),
        GeneratedAffineResidualGroupExactMappedSource::Coefficient(value) => {
            mapped_coefficient_payload_owned_retained_byte_bound(
                &value.normalized_value,
                &value.normalized_numerator,
                &value.pre_normalization_mapped_denominator,
                value.denominator_identities.iter(),
            )
        }
    }
}

fn mapped_coefficient_payload_owned_retained_byte_bound<'a>(
    normalized_value: &ParametricCoefficient,
    normalized_numerator: &ParametricPolynomial,
    pre_normalization_mapped_denominator: &ParametricPolynomial,
    identities: impl Iterator<Item = &'a GeneratedAffineResidualGroupExactDenominatorIdentity>,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let mut bytes = size_of::<GeneratedAffineResidualGroupExactMappedCoefficient>();
    bytes = checked_add(
        "exact WhenBad retained mapped coefficient",
        bytes,
        coefficient_retained_bound(normalized_value)?,
    )?;
    bytes = checked_add(
        "exact WhenBad retained mapped coefficient",
        bytes,
        polynomial_retained_bound(normalized_numerator)?,
    )?;
    bytes = checked_add(
        "exact WhenBad retained mapped coefficient",
        bytes,
        polynomial_retained_bound(pre_normalization_mapped_denominator)?,
    )?;
    for identity in identities {
        bytes = checked_add(
            "exact WhenBad retained denominator identities",
            bytes,
            identity.projection.stats().retained_output_byte_bound(),
        )?;
    }
    Ok(bytes)
}

fn mapped_coefficient_core_owned_retained_byte_bound(
    normalized_value: &ParametricCoefficient,
    normalized_numerator: &ParametricPolynomial,
    pre_normalization_mapped_denominator: &ParametricPolynomial,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let mut bytes = size_of::<GeneratedAffineResidualGroupExactMappedCoefficient>();
    bytes = checked_add(
        "exact WhenBad retained mapped coefficient core",
        bytes,
        coefficient_retained_bound(normalized_value)?,
    )?;
    bytes = checked_add(
        "exact WhenBad retained mapped coefficient core",
        bytes,
        polynomial_retained_bound(normalized_numerator)?,
    )?;
    checked_add(
        "exact WhenBad retained mapped coefficient core",
        bytes,
        polynomial_retained_bound(pre_normalization_mapped_denominator)?,
    )
}

fn boundary_event_owned_retained_byte_bound(
    event: &GeneratedAffineResidualGroupExactBoundaryEvent,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let mut bytes = size_of::<GeneratedAffineResidualGroupExactBoundaryEvent>();
    bytes = checked_add(
        "exact WhenBad retained boundary value",
        bytes,
        integer_owned_logical_byte_bound(integer_magnitude_bits(
            &event.value,
            "exact WhenBad retained boundary value integer bits",
        )?)?
        .max(integer_actual_owned_logical_byte_bound(&event.value)?),
    )?;
    if let Some(boundary) = &event.boundary {
        bytes = checked_add(
            "exact WhenBad retained mapped boundary polynomial",
            bytes,
            polynomial_retained_bound(boundary)?,
        )?;
    }
    Ok(bytes)
}

fn identically_bad_witness_owned_retained_byte_bound(
    witness: &GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let bytes = size_of::<GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness>();
    match witness {
        GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness::CandidateGuardMappedToZero {
            polynomial,
            ..
        } => checked_add(
            "exact WhenBad retained zero mapped guard witness",
            bytes,
            polynomial_retained_bound(polynomial)?,
        ),
        GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness::ZeroMappedDenominator { .. } => Ok(bytes),
        GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness::WholeTargetInactiveActivation { event } => {
            checked_add(
                "exact WhenBad retained whole-target boundary witness",
                bytes,
                boundary_event_owned_retained_byte_bound(event)?,
            )
        }
    }
}

fn mapped_condition_envelope(
    stats: ResidualUnitAffinePolynomialCompositionStats,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    checked_add(
        "exact WhenBad compact condition retained envelope",
        size_of::<GeneratedAffineResidualGroupExactMappedCondition>(),
        prospective_polynomial_retained_envelope(stats)?,
    )
}

fn mapped_coefficient_envelope(
    stats: ResidualAffineCoefficientCompositionPreflight,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let aggregate_polynomial = prospective_polynomial_retained_envelope(stats.aggregate())?;
    let durable_denominator = sparse_polynomial_retained_envelope(
        stats.durable_denominator_term_bound(),
        stats.durable_denominator_exponent_entry_bound(),
        stats.durable_denominator_integer_bit_payload_bound(),
    )?;
    // A normalized rational owns two sparse halves; its extracted normalized
    // numerator and the distinct pre-normalization denominator are retained
    // alongside it. Normalization can cancel support, not grow beyond the
    // admitted aggregate expansion envelope.
    let normalized_and_numerator = checked_mul(
        "exact WhenBad compact coefficient retained envelope",
        aggregate_polynomial,
        3,
    )?;
    checked_add(
        "exact WhenBad compact coefficient retained envelope",
        checked_add(
            "exact WhenBad compact coefficient retained envelope",
            size_of::<GeneratedAffineResidualGroupExactMappedCoefficient>(),
            normalized_and_numerator,
        )?,
        durable_denominator,
    )
}

fn prospective_polynomial_retained_envelope(
    stats: ResidualUnitAffinePolynomialCompositionStats,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    Ok(
        crate::parametric_coefficient::residual_affine_composition_output_retained_byte_envelope(
            stats,
        )?,
    )
}

fn sparse_polynomial_retained_envelope(
    term_bound: usize,
    exponent_entry_bound: usize,
    integer_bit_bound: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    Ok(
        crate::parametric_coefficient::residual_affine_polynomial_retained_byte_envelope(
            term_bound,
            exponent_entry_bound,
            integer_bit_bound,
        )?,
    )
}

fn polynomial_retained_bound(
    polynomial: &ParametricPolynomial,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    polynomial.owned_retained_byte_bound().ok_or(
        GeneratedAffineResidualGroupExactWhenBadMaterializationError::ResourceCountOverflow {
            resource: "exact WhenBad polynomial retained bytes",
        },
    )
}

fn coefficient_retained_bound(
    coefficient: &ParametricCoefficient,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    coefficient.owned_retained_byte_bound().ok_or(
        GeneratedAffineResidualGroupExactWhenBadMaterializationError::ResourceCountOverflow {
            resource: "exact WhenBad coefficient retained bytes",
        },
    )
}

fn integer_magnitude_bits(
    value: &Integer,
    resource: &'static str,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| {
        GeneratedAffineResidualGroupExactWhenBadMaterializationError::ResourceCountOverflow {
            resource,
        }
    })
}

fn integer_to_usize_borrowed(
    value: &Integer,
    resource: &'static str,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let converted = match value {
        Integer::Single(value) => usize::try_from(*value).ok(),
        Integer::Double(value) => usize::try_from(*value).ok(),
        Integer::Large(value) => value
            .to_u128()
            .and_then(|value| usize::try_from(value).ok()),
    };
    converted.ok_or(
        GeneratedAffineResidualGroupExactWhenBadMaterializationError::ResourceCountOverflow {
            resource,
        },
    )
}

fn integer_owned_logical_byte_bound(
    bits: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let rounded_bits = checked_add("exact WhenBad integer retained logical bytes", bits, 63)?;
    let limbs = rounded_bits.checked_div(64).ok_or(
        GeneratedAffineResidualGroupExactWhenBadMaterializationError::ResourceCountOverflow {
            resource: "exact WhenBad integer retained logical bytes",
        },
    )?;
    let limbs = checked_add(
        "exact WhenBad integer retained logical bytes",
        limbs,
        usize::from(bits > 0),
    )?;
    checked_add(
        "exact WhenBad integer retained logical bytes",
        size_of::<Integer>(),
        checked_mul(
            "exact WhenBad integer retained logical bytes",
            limbs,
            size_of::<usize>(),
        )?,
    )
}

fn integer_actual_owned_logical_byte_bound(
    value: &Integer,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let dynamic = match value {
        Integer::Single(_) | Integer::Double(_) => 0,
        Integer::Large(value) => {
            let capacity_bits = usize::try_from(value.capacity()).map_err(|_| {
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::ResourceCountOverflow {
                    resource: "exact WhenBad integer retained logical bytes",
                }
            })?;
            checked_add(
                "exact WhenBad integer retained logical bytes",
                capacity_bits,
                7,
            )?
            .checked_div(8)
            .ok_or(
                GeneratedAffineResidualGroupExactWhenBadMaterializationError::ResourceCountOverflow {
                    resource: "exact WhenBad integer retained logical bytes",
                },
            )?
        }
    };
    checked_add(
        "exact WhenBad integer retained logical bytes",
        size_of::<Integer>(),
        dynamic,
    )
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    left.checked_add(right).ok_or(
        GeneratedAffineResidualGroupExactWhenBadMaterializationError::ResourceCountOverflow {
            resource,
        },
    )
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    left.checked_mul(right).ok_or(
        GeneratedAffineResidualGroupExactWhenBadMaterializationError::ResourceCountOverflow {
            resource,
        },
    )
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    if requested > limit {
        Err(
            GeneratedAffineResidualGroupExactWhenBadMaterializationError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

fn remaining_limit(
    resource: &'static str,
    limit: usize,
    used: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    limit.checked_sub(used).ok_or(
        GeneratedAffineResidualGroupExactWhenBadMaterializationError::ResourceLimit {
            resource,
            requested: used,
            limit,
        },
    )
}

fn try_vec_with_exact_capacity<T>(
    resource: &'static str,
    requested: usize,
) -> Result<Vec<T>, GeneratedAffineResidualGroupExactWhenBadMaterializationError> {
    let mut values = Vec::new();
    values.try_reserve_exact(requested).map_err(|_| {
        GeneratedAffineResidualGroupExactWhenBadMaterializationError::AllocationFailure {
            resource,
            requested,
        }
    })?;
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_affine_residual_group_exact_when_bad_conditions::{
        GeneratedAffineResidualGroupExactConditionPlanCompiler,
        GeneratedAffineResidualGroupExactConditionPlanLimits,
    };
    use crate::solver::exact_session::test_support::{
        ExactConditionPlanTestFixture, exact_condition_plan_test_fixture,
        exact_condition_plan_test_fixture_in_sector,
    };

    fn condition_plan(
        name: &str,
        compact: bool,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        GeneratedAffineResidualGroupExactSession,
        GeneratedAffineResidualGroupExactConditionPlan,
    ) {
        condition_plan_in_sector(name, "111", compact)
    }

    fn condition_plan_in_sector(
        name: &str,
        sector_bits: &str,
        compact: bool,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        GeneratedAffineResidualGroupExactSession,
        GeneratedAffineResidualGroupExactConditionPlan,
    ) {
        let ExactConditionPlanTestFixture {
            family,
            context,
            session,
            source: _,
            ready,
        } = if sector_bits == "111" {
            exact_condition_plan_test_fixture(name, compact)
        } else {
            exact_condition_plan_test_fixture_in_sector(name, sector_bits, compact)
        };
        let plan = GeneratedAffineResidualGroupExactConditionPlanCompiler::compile(
            &family,
            &context,
            &session,
            ready,
            GeneratedAffineResidualGroupExactConditionPlanLimits::default(),
        )
        .unwrap();
        (family, context, session, plan)
    }

    fn compile(
        name: &str,
        compact: bool,
        limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    ) -> Result<
        (
            IntegralFamily,
            ParametricCoefficientContext,
            GeneratedAffineResidualGroupExactSession,
            GeneratedAffineResidualGroupExactWhenBadMaterialization,
        ),
        (
            IntegralFamily,
            ParametricCoefficientContext,
            GeneratedAffineResidualGroupExactSession,
            GeneratedAffineResidualGroupExactWhenBadMaterializationFailure,
        ),
    > {
        compile_in_sector(name, "111", compact, limits)
    }

    fn compile_in_sector(
        name: &str,
        sector_bits: &str,
        compact: bool,
        limits: GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
    ) -> Result<
        (
            IntegralFamily,
            ParametricCoefficientContext,
            GeneratedAffineResidualGroupExactSession,
            GeneratedAffineResidualGroupExactWhenBadMaterialization,
        ),
        (
            IntegralFamily,
            ParametricCoefficientContext,
            GeneratedAffineResidualGroupExactSession,
            GeneratedAffineResidualGroupExactWhenBadMaterializationFailure,
        ),
    > {
        let (family, context, session, plan) = condition_plan_in_sector(name, sector_bits, compact);
        match GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler::compile(
            &family, &context, &session, plan, limits,
        ) {
            Ok(materialization) => Ok((family, context, session, materialization)),
            Err(failure) => Err((family, context, session, failure)),
        }
    }

    macro_rules! define_outer_limit_probes {
        ($stats:ident; $($variant:ident => $field:ident = $observed:expr),+ $(,)?) => {
            #[repr(usize)]
            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            enum OuterLimitProbe {
                $($variant),+
            }

            const ALL_OUTER_LIMIT_PROBES: &[OuterLimitProbe] = &[
                $(OuterLimitProbe::$variant),+
            ];

            impl OuterLimitProbe {
                fn observed(
                    self,
                    $stats: GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
                ) -> usize {
                    match self {
                        $(Self::$variant => $observed),+
                    }
                }

                fn set(
                    self,
                    limits: &mut GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
                    value: usize,
                ) {
                    match self {
                        $(Self::$variant => limits.$field = value),+
                    }
                }
            }
        };
    }

    define_outer_limit_probes!(
        stats;
        SourceRecords => max_source_records = stats.source_records(),
        ConditionRecords => max_condition_records = stats.condition_records(),
        CoefficientRecords => max_coefficient_records = stats.coefficient_records(),
        DenominatorIdentitySources => max_denominator_identity_sources = stats.denominator_identity_sources(),
        DenominatorIdentityLoci => max_denominator_identity_loci = stats.denominator_identity_loci(),
        HazardRanges => max_hazard_ranges = stats.hazard_ranges(),
        BoundaryValues => max_boundary_values = stats.admitted_boundary_values(),
        BoundaryValueIntegerBits => max_boundary_value_integer_bits = stats.boundary().value_integer_bits(),
        BoundaryValueRetainedBytes => max_boundary_value_retained_logical_bytes = stats.boundary().value_retained_logical_bytes(),
        BoundaryEnumerationWork => max_boundary_enumeration_integer_bit_work = stats.boundary().enumeration_integer_bit_work(),
        MappingSourceTerms => max_mapping_source_terms = stats.mapping().source_terms(),
        MappingSourceExponentEntries => max_mapping_source_exponent_entries = stats.mapping().source_exponent_entries(),
        MappingSourceIntegerBits => max_mapping_source_integer_bits = stats.mapping().source_integer_bits(),
        MappingAdmittedRetainedBytes => max_mapping_admitted_retained_byte_bound = stats.mapping().admitted_retained_byte_bound(),
        MappingAdmissionTemporaryPeak => max_mapping_admission_temporary_byte_peak = stats.mapping().admission_temporary_byte_peak(),
        MappingExpandedContributions => max_mapping_expanded_contribution_bound = stats.mapping().expanded_contribution_bound(),
        MappingOutputExponentEntries => max_mapping_output_exponent_entry_bound = stats.mapping().output_exponent_entry_bound(),
        MappingPowerCalls => max_mapping_power_calls = stats.mapping().power_calls(),
        MappingNativePowerHeapPairs => max_mapping_native_power_heap_pair_bound = stats.mapping().native_power_heap_pair_bound(),
        MappingMultiplicationTermPairs => max_mapping_multiplication_term_pair_bound = stats.mapping().multiplication_term_pair_bound(),
        MappingAdditionTermVisits => max_mapping_addition_term_visit_bound = stats.mapping().addition_term_visit_bound(),
        MappingNativeIntegerBitWork => max_mapping_native_integer_bit_work_bound = stats.mapping().native_integer_bit_work_bound(),
        MappingIntegerBitWork => max_mapping_integer_bit_work_bound = stats.mapping().integer_bit_work_bound(),
        MappingNormalizationInputPairs => max_mapping_normalization_input_term_pairs = stats.mapping().normalization_input_term_pairs(),
        ProjectionSourceTerms => max_projection_source_terms = stats.projection().source_terms(),
        ProjectionSourceExponentEntries => max_projection_source_exponent_entries = stats.projection().source_exponent_entries(),
        ProjectionSourceIntegerBits => max_projection_source_integer_bits = stats.projection().source_integer_bits(),
        ProjectionNativeWorkspaceBytes => max_projection_native_workspace_byte_envelope = stats.projection().native_workspace_byte_envelope(),
        ProjectionRetainedOutputBytes => max_projection_retained_output_byte_bound = stats.projection().retained_output_byte_bound(),
        ProjectionTemporaryBytes => max_projection_temporary_byte_envelope = stats.projection().temporary_byte_envelope(),
        BoundaryMappingConstructedTerms => max_boundary_mapping_constructed_terms = stats.boundary().mapping_constructed_terms(),
        BoundaryMappingConstructedExponentEntries => max_boundary_mapping_constructed_exponent_entries = stats.boundary().mapping_constructed_exponent_entries(),
        BoundaryMappingConstructedIntegerBits => max_boundary_mapping_constructed_integer_bits = stats.boundary().mapping_constructed_integer_bits(),
        BoundaryMappingMappedTerms => max_boundary_mapping_mapped_term_bound = stats.boundary().mapping_mapped_term_bound(),
        BoundaryMappingMappedExponentEntries => max_boundary_mapping_mapped_exponent_entry_bound = stats.boundary().mapping_mapped_exponent_entry_bound(),
        BoundaryMappingMappedIntegerBits => max_boundary_mapping_mapped_integer_bit_bound = stats.boundary().mapping_mapped_integer_bit_bound(),
        BoundaryMappingAffineTermVisits => max_boundary_mapping_affine_term_visits = stats.boundary().mapping_affine_term_visits(),
        BoundaryMappingAffineExponentVisits => max_boundary_mapping_affine_exponent_visits = stats.boundary().mapping_affine_exponent_visits(),
        BoundaryMappingRetainedOutputBytes => max_boundary_mapping_retained_output_byte_bound = stats.boundary().mapping_retained_output_byte_bound(),
        BoundaryMappingConstructedSourceTemporaryPeak => max_boundary_mapping_constructed_source_temporary_byte_peak = stats.boundary().mapping_constructed_source_temporary_byte_peak(),
        BoundaryMappingChildCompilationPeak => max_boundary_mapping_child_compilation_byte_peak = stats.boundary().mapping_child_compilation_byte_peak(),
        BoundaryNumeratorBoundaryTerms => max_boundary_numerator_boundary_terms = stats.boundary().numerator_boundary_terms(),
        BoundaryNumeratorBoundaryExponentEntries => max_boundary_numerator_boundary_exponent_entries = stats.boundary().numerator_boundary_exponent_entries(),
        BoundaryNumeratorBoundaryIntegerBits => max_boundary_numerator_boundary_integer_bits = stats.boundary().numerator_boundary_integer_bits(),
        BoundaryNumeratorNumeratorTerms => max_boundary_numerator_numerator_terms = stats.boundary().numerator_numerator_terms(),
        BoundaryNumeratorNumeratorExponentEntries => max_boundary_numerator_numerator_exponent_entries = stats.boundary().numerator_numerator_exponent_entries(),
        BoundaryNumeratorNumeratorIntegerBits => max_boundary_numerator_numerator_integer_bits = stats.boundary().numerator_numerator_integer_bits(),
        BoundaryNumeratorAffineTermVisits => max_boundary_numerator_affine_term_visits = stats.boundary().numerator_affine_term_visits(),
        BoundaryNumeratorAffineExponentVisits => max_boundary_numerator_affine_exponent_visits = stats.boundary().numerator_affine_exponent_visits(),
        BoundaryNumeratorDivisibilityTermPairs => max_boundary_numerator_divisibility_term_pairs = stats.boundary().numerator_divisibility_term_pairs(),
        BoundaryNumeratorDivisibilityCalls => max_boundary_numerator_divisibility_calls = stats.boundary().numerator_divisibility_calls(),
        BoundaryNumeratorSourceCopyTemporaryPeak => max_boundary_numerator_source_copy_temporary_byte_peak = stats.boundary().numerator_source_copy_temporary_byte_peak(),
        BoundaryNumeratorRetainedBytes => max_boundary_numerator_retained_owned_logical_bytes = stats.boundary().numerator_retained_owned_logical_bytes(),
        RetainedOwnedBytes => max_retained_owned_logical_bytes = stats.retained_owned_logical_bytes(),
        CompilationOwnedPeak => max_compilation_owned_logical_peak_upper_bound = stats.compilation_owned_logical_peak_upper_bound(),
    );

    const OUTER_LIMIT_SHARD_ZERO: &[OuterLimitProbe] = &[
        OuterLimitProbe::SourceRecords,
        OuterLimitProbe::ConditionRecords,
        OuterLimitProbe::CoefficientRecords,
        OuterLimitProbe::DenominatorIdentitySources,
        OuterLimitProbe::HazardRanges,
        OuterLimitProbe::BoundaryValues,
        OuterLimitProbe::BoundaryValueIntegerBits,
        OuterLimitProbe::BoundaryValueRetainedBytes,
        OuterLimitProbe::BoundaryEnumerationWork,
        OuterLimitProbe::MappingSourceTerms,
        OuterLimitProbe::MappingSourceExponentEntries,
    ];

    const OUTER_LIMIT_SHARD_ONE: &[OuterLimitProbe] = &[
        OuterLimitProbe::MappingSourceIntegerBits,
        OuterLimitProbe::MappingAdmittedRetainedBytes,
        OuterLimitProbe::MappingAdmissionTemporaryPeak,
        OuterLimitProbe::MappingExpandedContributions,
        OuterLimitProbe::MappingOutputExponentEntries,
        OuterLimitProbe::MappingPowerCalls,
        OuterLimitProbe::MappingNativePowerHeapPairs,
    ];

    const OUTER_LIMIT_SHARD_TWO: &[OuterLimitProbe] = &[
        OuterLimitProbe::MappingMultiplicationTermPairs,
        OuterLimitProbe::MappingAdditionTermVisits,
        OuterLimitProbe::MappingNativeIntegerBitWork,
        OuterLimitProbe::MappingIntegerBitWork,
        OuterLimitProbe::MappingNormalizationInputPairs,
        OuterLimitProbe::ProjectionSourceTerms,
        OuterLimitProbe::ProjectionSourceExponentEntries,
    ];

    const OUTER_LIMIT_SHARD_THREE: &[OuterLimitProbe] = &[
        OuterLimitProbe::DenominatorIdentityLoci,
        OuterLimitProbe::ProjectionSourceIntegerBits,
        OuterLimitProbe::ProjectionNativeWorkspaceBytes,
        OuterLimitProbe::ProjectionRetainedOutputBytes,
        OuterLimitProbe::ProjectionTemporaryBytes,
        OuterLimitProbe::BoundaryMappingConstructedTerms,
        OuterLimitProbe::BoundaryMappingConstructedExponentEntries,
        OuterLimitProbe::BoundaryMappingConstructedIntegerBits,
        OuterLimitProbe::BoundaryMappingMappedTerms,
        OuterLimitProbe::BoundaryMappingMappedExponentEntries,
        OuterLimitProbe::BoundaryMappingMappedIntegerBits,
        OuterLimitProbe::BoundaryMappingAffineTermVisits,
        OuterLimitProbe::BoundaryMappingAffineExponentVisits,
        OuterLimitProbe::BoundaryMappingRetainedOutputBytes,
        OuterLimitProbe::BoundaryMappingConstructedSourceTemporaryPeak,
        OuterLimitProbe::BoundaryMappingChildCompilationPeak,
        OuterLimitProbe::BoundaryNumeratorBoundaryTerms,
        OuterLimitProbe::BoundaryNumeratorBoundaryExponentEntries,
        OuterLimitProbe::BoundaryNumeratorBoundaryIntegerBits,
        OuterLimitProbe::BoundaryNumeratorNumeratorTerms,
        OuterLimitProbe::BoundaryNumeratorNumeratorExponentEntries,
        OuterLimitProbe::BoundaryNumeratorNumeratorIntegerBits,
        OuterLimitProbe::BoundaryNumeratorAffineTermVisits,
        OuterLimitProbe::BoundaryNumeratorAffineExponentVisits,
        OuterLimitProbe::BoundaryNumeratorDivisibilityTermPairs,
        OuterLimitProbe::BoundaryNumeratorDivisibilityCalls,
        OuterLimitProbe::BoundaryNumeratorSourceCopyTemporaryPeak,
        OuterLimitProbe::BoundaryNumeratorRetainedBytes,
        OuterLimitProbe::RetainedOwnedBytes,
        OuterLimitProbe::CompilationOwnedPeak,
    ];

    const OUTER_LIMIT_SHARDS: &[&[OuterLimitProbe]] = &[
        OUTER_LIMIT_SHARD_ZERO,
        OUTER_LIMIT_SHARD_ONE,
        OUTER_LIMIT_SHARD_TWO,
        OUTER_LIMIT_SHARD_THREE,
    ];

    // The compact sector-111 baseline deliberately has no inactive-coordinate
    // hazards.  These four exact shards exercise every boundary-owned outer
    // allowance against a production-generated sector-011 owner instead.
    const BOUNDARY_OUTER_LIMIT_SHARD_ZERO: &[OuterLimitProbe] = &[
        OuterLimitProbe::HazardRanges,
        OuterLimitProbe::BoundaryValues,
        OuterLimitProbe::BoundaryValueIntegerBits,
        OuterLimitProbe::BoundaryValueRetainedBytes,
        OuterLimitProbe::BoundaryEnumerationWork,
        OuterLimitProbe::BoundaryMappingConstructedTerms,
        OuterLimitProbe::BoundaryMappingConstructedExponentEntries,
    ];

    const BOUNDARY_OUTER_LIMIT_SHARD_ONE: &[OuterLimitProbe] = &[
        OuterLimitProbe::BoundaryMappingConstructedIntegerBits,
        OuterLimitProbe::BoundaryMappingMappedTerms,
        OuterLimitProbe::BoundaryMappingMappedExponentEntries,
        OuterLimitProbe::BoundaryMappingMappedIntegerBits,
        OuterLimitProbe::BoundaryMappingAffineTermVisits,
        OuterLimitProbe::BoundaryMappingAffineExponentVisits,
        OuterLimitProbe::BoundaryMappingRetainedOutputBytes,
    ];

    const BOUNDARY_OUTER_LIMIT_SHARD_TWO: &[OuterLimitProbe] = &[
        OuterLimitProbe::BoundaryMappingConstructedSourceTemporaryPeak,
        OuterLimitProbe::BoundaryMappingChildCompilationPeak,
        OuterLimitProbe::BoundaryNumeratorBoundaryTerms,
        OuterLimitProbe::BoundaryNumeratorBoundaryExponentEntries,
        OuterLimitProbe::BoundaryNumeratorBoundaryIntegerBits,
        OuterLimitProbe::BoundaryNumeratorNumeratorTerms,
        OuterLimitProbe::BoundaryNumeratorNumeratorExponentEntries,
    ];

    const BOUNDARY_OUTER_LIMIT_SHARD_THREE: &[OuterLimitProbe] = &[
        OuterLimitProbe::BoundaryNumeratorNumeratorIntegerBits,
        OuterLimitProbe::BoundaryNumeratorAffineTermVisits,
        OuterLimitProbe::BoundaryNumeratorAffineExponentVisits,
        OuterLimitProbe::BoundaryNumeratorDivisibilityTermPairs,
        OuterLimitProbe::BoundaryNumeratorDivisibilityCalls,
        OuterLimitProbe::BoundaryNumeratorSourceCopyTemporaryPeak,
        OuterLimitProbe::BoundaryNumeratorRetainedBytes,
    ];

    const BOUNDARY_OUTER_LIMIT_SHARDS: &[&[OuterLimitProbe]] = &[
        BOUNDARY_OUTER_LIMIT_SHARD_ZERO,
        BOUNDARY_OUTER_LIMIT_SHARD_ONE,
        BOUNDARY_OUTER_LIMIT_SHARD_TWO,
        BOUNDARY_OUTER_LIMIT_SHARD_THREE,
    ];

    // These owner-wide gates are intentionally separate from the exact
    // boundary-only partition above.  Running them on the nonempty sector-011
    // baseline proves that durable boundary payload and boundary scratch feed
    // the transaction's aggregate retained and compilation-peak envelopes.
    const BOUNDARY_AGGREGATE_OUTER_LIMIT_PROBES: &[OuterLimitProbe] = &[
        OuterLimitProbe::RetainedOwnedBytes,
        OuterLimitProbe::CompilationOwnedPeak,
    ];

    fn shared_baseline_stats() -> GeneratedAffineResidualGroupExactWhenBadMaterializationStats {
        static STATS: std::sync::OnceLock<
            GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
        > = std::sync::OnceLock::new();
        *STATS.get_or_init(|| {
            compile(
                "exact-when-bad-materialization-shared-limit-baseline",
                true,
                GeneratedAffineResidualGroupExactWhenBadMaterializationLimits::default(),
            )
            .unwrap()
            .3
            .stats()
        })
    }

    fn shared_boundary_baseline_stats()
    -> GeneratedAffineResidualGroupExactWhenBadMaterializationStats {
        static STATS: std::sync::OnceLock<
            GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
        > = std::sync::OnceLock::new();
        *STATS.get_or_init(|| {
            compile_in_sector(
                "exact-when-bad-materialization-shared-sector-011-boundary-baseline",
                "011",
                false,
                GeneratedAffineResidualGroupExactWhenBadMaterializationLimits::default(),
            )
            .unwrap()
            .3
            .stats()
        })
    }

    fn exact_outer_limits(
        stats: GeneratedAffineResidualGroupExactWhenBadMaterializationStats,
    ) -> GeneratedAffineResidualGroupExactWhenBadMaterializationLimits {
        let mut limits = GeneratedAffineResidualGroupExactWhenBadMaterializationLimits::default();
        for probe in ALL_OUTER_LIMIT_PROBES {
            probe.set(&mut limits, probe.observed(stats));
        }
        limits
    }

    fn run_one_below_shard(name: &str, probes: &[OuterLimitProbe]) {
        let stats = shared_baseline_stats();
        let exact = exact_outer_limits(stats);
        let (family, context, session, mut plan) = condition_plan(name, true);
        let mut attempted = 0usize;
        for probe in probes {
            let observed = probe.observed(stats);
            if observed == 0 {
                continue;
            }
            let mut one_below = exact;
            probe.set(&mut one_below, observed - 1);
            let failure = GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler::compile(
                &family, &context, &session, plan, one_below,
            )
            .unwrap_err();
            let (_, recovered) = failure.into_parts();
            assert_eq!(recovered.targets_consumed(), 0, "{probe:?}");
            assert!(!recovered.publishes_rule(), "{probe:?}");
            recovered.replay(&family, &context, &session).unwrap();
            session.replay(&family, &context).unwrap();
            plan = recovered;
            attempted += 1;
        }
        assert!(attempted > 0, "shard {name} had no positive outer field");
        let materialization =
            GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler::compile(
                &family, &context, &session, plan, exact,
            )
            .unwrap();
        assert_eq!(materialization.targets_consumed(), 0);
        assert!(!materialization.publishes_rule());
        assert_eq!(materialization.stats(), stats);
        materialization.replay(&family, &context, &session).unwrap();
    }

    fn run_boundary_one_below_shard(name: &str, probes: &[OuterLimitProbe]) {
        let stats = shared_boundary_baseline_stats();
        let exact = exact_outer_limits(stats);
        let (family, context, session, mut plan) = condition_plan_in_sector(name, "011", false);
        for probe in probes {
            let observed = probe.observed(stats);
            assert!(
                observed > 0,
                "sector-011 boundary probe {probe:?} is vacuous"
            );
            let mut one_below = exact;
            probe.set(&mut one_below, observed - 1);
            let failure = GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler::compile(
                &family, &context, &session, plan, one_below,
            )
            .unwrap_err();
            let (_, recovered) = failure.into_parts();
            assert_eq!(recovered.targets_consumed(), 0, "{probe:?}");
            assert!(!recovered.publishes_rule(), "{probe:?}");
            recovered.replay(&family, &context, &session).unwrap();
            session.replay(&family, &context).unwrap();
            plan = recovered;
        }
        let materialization =
            GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler::compile(
                &family, &context, &session, plan, exact,
            )
            .unwrap();
        assert_eq!(materialization.targets_consumed(), 0);
        assert!(!materialization.publishes_rule());
        assert_eq!(materialization.stats(), stats);
        materialization.replay(&family, &context, &session).unwrap();
    }

    #[test]
    fn identity_and_compact_materializations_follow_schedule_and_replay() {
        for (name, compact) in [
            ("exact-when-bad-materialization-identity", false),
            ("exact-when-bad-materialization-compact", true),
        ] {
            let (family, context, session, materialization) = compile(
                name,
                compact,
                GeneratedAffineResidualGroupExactWhenBadMaterializationLimits::default(),
            )
            .unwrap();
            assert_eq!(materialization.targets_consumed(), 0);
            assert!(!materialization.publishes_rule());
            materialization.replay(&family, &context, &session).unwrap();
            session.replay(&family, &context).unwrap();
            match &materialization {
                GeneratedAffineResidualGroupExactWhenBadMaterialization::ReadyForPartition(
                    ready,
                ) => {
                    assert_eq!(ready.sources().len(), ready.stats().source_records());
                    assert_eq!(
                        ready.boundaries().len(),
                        ready.stats().admitted_boundary_values()
                    );
                    for (ordinal, source) in ready.sources().iter().enumerate() {
                        assert_eq!(source.source(), ready.plan.source_schedule()[ordinal]);
                        if let Some(coefficient) = source.coefficient() {
                            assert_eq!(coefficient.denominator_identities().len(), 2);
                            assert_eq!(
                                coefficient.denominator_identities()[0].kind(),
                                GeneratedAffineResidualGroupExactDenominatorIdentityKind::PreNormalizationMappedDenominator,
                            );
                            assert_eq!(
                                coefficient.denominator_identities()[1].kind(),
                                GeneratedAffineResidualGroupExactDenominatorIdentityKind::NormalizedMappedDenominator,
                            );
                            assert!(coefficient.denominator_identities().iter().all(|identity| {
                                !matches!(
                                    identity.projection().class(),
                                    ParametricParameterIdentityClass::AlwaysIdentityZero
                                )
                            }));
                        }
                    }
                }
                GeneratedAffineResidualGroupExactWhenBadMaterialization::IdenticallyBad(bad) => {
                    assert!(bad.sources().len() <= bad.stats().source_records());
                    assert!(matches!(
                        bad.witness(),
                        GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness::CandidateGuardMappedToZero { .. }
                            | GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness::ZeroMappedDenominator { .. }
                            | GeneratedAffineResidualGroupExactWhenBadIdenticallyBadWitness::WholeTargetInactiveActivation { .. }
                    ));
                }
            }
        }
    }

    #[test]
    fn generated_sector_011_identity_owner_materializes_exact_boundary_semantics() {
        let fixture_name = "exact-when-bad-materialization-sector-011-semantics";
        let (family, context, session, materialization) = compile_in_sector(
            fixture_name,
            "011",
            false,
            GeneratedAffineResidualGroupExactWhenBadMaterializationLimits::default(),
        )
        .unwrap();
        assert_eq!(materialization.targets_consumed(), 0);
        assert!(!materialization.publishes_rule());
        let GeneratedAffineResidualGroupExactWhenBadMaterialization::ReadyForPartition(ready) =
            &materialization
        else {
            panic!("sector-011 production owner unexpectedly became identically bad");
        };

        let plan = &ready.plan;
        assert!(plan.target_transform_is_identity());
        assert!(plan.compact_target_transform().is_none());
        assert_eq!(plan.ready().ready().target_premises().len(), 0);
        assert_eq!(plan.ready().ready().row_guards().len(), 2);
        assert_eq!(plan.ready().ready().terms().len(), 5);
        assert_eq!(plan.ready().pivot_term_ordinal(), 4);
        assert_eq!(plan.ready().descent().len(), 4);
        assert_eq!(
            plan.source_schedule(),
            [
                GeneratedAffineResidualGroupExactConditionSourceLocator::RecenteredRowGuard {
                    guard_ordinal: 0,
                },
                GeneratedAffineResidualGroupExactConditionSourceLocator::RecenteredRowGuard {
                    guard_ordinal: 1,
                },
                GeneratedAffineResidualGroupExactConditionSourceLocator::PivotCoefficient {
                    term_ordinal: 4,
                },
                GeneratedAffineResidualGroupExactConditionSourceLocator::RhsCoefficient {
                    rhs_ordinal: 0,
                    term_ordinal: 0,
                },
                GeneratedAffineResidualGroupExactConditionSourceLocator::RhsCoefficient {
                    rhs_ordinal: 1,
                    term_ordinal: 1,
                },
                GeneratedAffineResidualGroupExactConditionSourceLocator::RhsCoefficient {
                    rhs_ordinal: 2,
                    term_ordinal: 2,
                },
                GeneratedAffineResidualGroupExactConditionSourceLocator::RhsCoefficient {
                    rhs_ordinal: 3,
                    term_ordinal: 3,
                },
            ]
        );
        assert_eq!(plan.hazard_schedule().len(), 4);
        for (ordinal, hazard) in plan.hazard_schedule().iter().enumerate() {
            assert_eq!(hazard.hazard_ordinal(), ordinal);
            assert_eq!(hazard.rhs_ordinal(), ordinal);
            assert_eq!(hazard.term_ordinal(), ordinal);
            assert_eq!(hazard.coordinate(), 0);
        }
        for (ordinal, hazard) in plan.ready().hazards().iter().enumerate() {
            assert_eq!(hazard.rhs_ordinal(), ordinal);
            assert_eq!(hazard.term_ordinal(), ordinal);
            assert_eq!(hazard.coordinate(), 0);
            assert_eq!(
                hazard.first(),
                &Integer::from(if ordinal == 0 { -1 } else { 0 })
            );
            assert_eq!(hazard.last(), &Integer::from(0));
            assert_eq!(
                hazard.count(),
                &Integer::from(if ordinal == 0 { 2 } else { 1 })
            );
        }

        assert_eq!(ready.sources().len(), 7);
        for (ordinal, source) in ready.sources().iter().enumerate() {
            assert_eq!(source.source(), plan.source_schedule()[ordinal]);
            let mapping = match source {
                GeneratedAffineResidualGroupExactMappedSource::Condition(condition) => {
                    condition.mapping()
                }
                GeneratedAffineResidualGroupExactMappedSource::Coefficient(coefficient) => {
                    assert_eq!(
                        coefficient.denominator_identities()[0].kind(),
                        GeneratedAffineResidualGroupExactDenominatorIdentityKind::PreNormalizationMappedDenominator,
                    );
                    assert_eq!(
                        coefficient.denominator_identities()[1].kind(),
                        GeneratedAffineResidualGroupExactDenominatorIdentityKind::NormalizedMappedDenominator,
                    );
                    coefficient.mapping()
                }
            };
            assert!(matches!(
                mapping,
                GeneratedAffineResidualGroupExactSourceMappingStats::IdentityPolynomial(_)
                    | GeneratedAffineResidualGroupExactSourceMappingStats::IdentityCoefficient(_)
            ));
        }

        let mut conditional_identities = 0usize;
        let mut never_identities = 0usize;
        for coefficient in ready
            .sources()
            .iter()
            .filter_map(|source| source.coefficient())
        {
            for identity in coefficient.denominator_identities() {
                match identity.projection().class() {
                    ParametricParameterIdentityClass::Conditional { coefficient_loci } => {
                        conditional_identities += 1;
                        assert_eq!(coefficient_loci.len(), 1);
                    }
                    ParametricParameterIdentityClass::NeverIdentityZero { .. } => {
                        never_identities += 1;
                    }
                    ParametricParameterIdentityClass::AlwaysIdentityZero => {
                        panic!("nonzero denominator violated the projection invariant");
                    }
                }
            }
        }
        assert_eq!(conditional_identities, 4);
        assert_eq!(never_identities, 6);

        let expected_events = [
            (
                0,
                0,
                -1,
                GeneratedAffineResidualGroupExactBoundaryDisposition::SuppressedByNumerator,
            ),
            (
                1,
                0,
                0,
                GeneratedAffineResidualGroupExactBoundaryDisposition::RetainedBadBoundary,
            ),
            (
                2,
                1,
                0,
                GeneratedAffineResidualGroupExactBoundaryDisposition::RetainedBadBoundary,
            ),
            (
                3,
                2,
                0,
                GeneratedAffineResidualGroupExactBoundaryDisposition::RetainedBadBoundary,
            ),
            (
                4,
                3,
                0,
                GeneratedAffineResidualGroupExactBoundaryDisposition::RetainedBadBoundary,
            ),
        ];
        assert_eq!(ready.boundaries().len(), expected_events.len());
        for (event, (ordinal, hazard_ordinal, value, disposition)) in
            ready.boundaries().iter().zip(expected_events)
        {
            assert_eq!(event.ordinal(), ordinal);
            assert_eq!(event.source().hazard_ordinal(), hazard_ordinal);
            assert_eq!(event.source().rhs_ordinal(), hazard_ordinal);
            assert_eq!(event.source().term_ordinal(), hazard_ordinal);
            assert_eq!(event.source().coordinate(), 0);
            assert_eq!(event.value(), &Integer::from(value));
            assert_eq!(event.disposition(), disposition);
            assert!(event.boundary().is_some());
            assert!(event.numerator_stats().is_some());
            assert!(event.mapping_stats().composition().is_none());
        }

        let stats = ready.stats();
        assert_eq!(stats.source_records(), 7);
        assert_eq!(stats.condition_records(), 2);
        assert_eq!(stats.coefficient_records(), 5);
        assert_eq!(stats.denominator_identity_sources(), 10);
        assert_eq!(stats.denominator_identity_loci(), 14);
        assert_eq!(stats.hazard_ranges(), 4);
        assert_eq!(stats.admitted_boundary_values(), 5);
        assert_eq!(stats.boundary_values(), 5);
        assert_eq!(stats.empty_boundaries(), 0);
        assert_eq!(stats.whole_target_boundaries(), 0);
        assert_eq!(stats.suppressed_boundaries(), 1);
        assert_eq!(stats.retained_boundaries(), 4);

        let mapping = stats.mapping();
        assert_eq!(mapping.source_terms(), 20);
        assert_eq!(mapping.source_exponent_entries(), 100);
        assert_eq!(mapping.source_integer_bits(), 23);
        assert_eq!(mapping.expanded_contribution_bound(), 0);
        assert_eq!(mapping.output_exponent_entry_bound(), 0);
        assert_eq!(mapping.power_calls(), 0);
        assert_eq!(mapping.native_power_heap_pair_bound(), 0);
        assert_eq!(mapping.multiplication_term_pair_bound(), 0);
        assert_eq!(mapping.addition_term_visit_bound(), 0);
        assert_eq!(mapping.native_integer_bit_work_bound(), 0);
        assert_eq!(mapping.integer_bit_work_bound(), 0);
        assert_eq!(mapping.normalization_input_term_pairs(), 0);

        let projection = stats.projection();
        assert_eq!(projection.sources(), 10);
        assert_eq!(projection.source_terms(), 14);
        assert_eq!(projection.source_exponent_entries(), 70);
        assert_eq!(projection.source_integer_bits(), 14);
        assert_eq!(projection.projected_physical_monomials(), 10);
        assert_eq!(projection.conditional_loci(), 4);

        let boundary = stats.boundary();
        assert_eq!(boundary.mapping_constructed_terms(), 6);
        assert_eq!(boundary.mapping_constructed_exponent_entries(), 30);
        assert_eq!(boundary.mapping_constructed_integer_bits(), 6);
        assert_eq!(boundary.mapping_mapped_term_bound(), 6);
        assert_eq!(boundary.mapping_mapped_exponent_entry_bound(), 30);
        assert_eq!(boundary.mapping_mapped_integer_bit_bound(), 5);
        assert_eq!(boundary.mapping_affine_term_visits(), 6);
        assert_eq!(boundary.mapping_affine_exponent_visits(), 18);
        assert_eq!(boundary.numerator_boundary_terms(), 6);
        assert_eq!(boundary.numerator_boundary_exponent_entries(), 30);
        assert_eq!(boundary.numerator_boundary_integer_bits(), 6);
        assert_eq!(boundary.numerator_numerator_terms(), 10);
        assert_eq!(boundary.numerator_numerator_exponent_entries(), 50);
        assert_eq!(boundary.numerator_numerator_integer_bits(), 15);
        assert_eq!(boundary.numerator_affine_term_visits(), 6);
        assert_eq!(boundary.numerator_affine_exponent_visits(), 18);
        assert_eq!(boundary.numerator_divisibility_term_pairs(), 12);
        assert_eq!(boundary.numerator_divisibility_calls(), 5);
        for bytes in [
            mapping.admitted_retained_byte_bound(),
            mapping.admission_temporary_byte_peak(),
            projection.native_workspace_byte_envelope(),
            projection.retained_output_byte_bound(),
            projection.temporary_byte_envelope(),
            boundary.value_retained_logical_bytes(),
            boundary.mapping_retained_output_byte_bound(),
            boundary.mapping_constructed_source_temporary_byte_peak(),
            boundary.mapping_child_compilation_byte_peak(),
            boundary.numerator_source_copy_temporary_byte_peak(),
            boundary.numerator_retained_owned_logical_bytes(),
            stats.source_phase_retained_logical_byte_bound(),
            stats.retained_owned_logical_bytes(),
            stats.compilation_owned_logical_peak_upper_bound(),
        ] {
            assert!(bytes > 0);
        }
        let exact_retained = [
            stats.source_phase_retained_logical_byte_bound(),
            boundary.value_retained_logical_bytes(),
            projection.retained_output_byte_bound(),
            boundary.mapping_retained_output_byte_bound(),
            boundary.numerator_retained_owned_logical_bytes(),
        ]
        .into_iter()
        .sum::<usize>();
        assert_eq!(stats.retained_owned_logical_bytes(), exact_retained);
        let largest_child_scratch = projection
            .temporary_byte_envelope()
            .max(boundary.mapping_constructed_source_temporary_byte_peak())
            .max(boundary.mapping_child_compilation_byte_peak())
            .max(boundary.numerator_source_copy_temporary_byte_peak());
        assert!(
            stats.compilation_owned_logical_peak_upper_bound()
                >= exact_retained + largest_child_scratch
        );

        materialization.replay(&family, &context, &session).unwrap();
        session.replay(&family, &context).unwrap();
        let (foreign_family, foreign_context, foreign_session, foreign_plan) =
            condition_plan_in_sector(fixture_name, "011", false);
        assert_eq!(foreign_plan.stats(), plan.stats());
        assert_eq!(foreign_plan.source_schedule(), plan.source_schedule());
        assert_eq!(foreign_plan.hazard_schedule(), plan.hazard_schedule());
        foreign_plan
            .replay(&foreign_family, &foreign_context, &foreign_session)
            .unwrap();
        assert!(
            materialization
                .replay(&foreign_family, &foreign_context, &foreign_session)
                .is_err()
        );
    }

    #[test]
    fn denominator_projection_wrapper_rejects_zero_before_symbolica_projection() {
        let (_family, context, _session, _plan) = condition_plan_in_sector(
            "exact-when-bad-materialization-zero-denominator-projection",
            "011",
            false,
        );
        let zero = context.numerator_condition(&context.zero()).unwrap();
        let source = GeneratedAffineResidualGroupExactConditionSourceLocator::PivotCoefficient {
            term_ordinal: 4,
        };
        let kind =
            GeneratedAffineResidualGroupExactDenominatorIdentityKind::NormalizedMappedDenominator;
        let error = project_denominator_identity(
            &context,
            &zero,
            source,
            kind,
            GeneratedAffineResidualGroupExactWhenBadMaterializationLimits::default(),
            &mut GeneratedAffineResidualGroupExactWhenBadMaterializationStats::default(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            GeneratedAffineResidualGroupExactWhenBadMaterializationError::DenominatorProjectionInvariantViolation {
                source,
                kind,
            }
        );
    }

    #[test]
    fn outer_limit_shards_are_exact_and_only_the_compact_boundary_lane_is_vacuous() {
        let mut counts = vec![0usize; ALL_OUTER_LIMIT_PROBES.len()];
        for shard in OUTER_LIMIT_SHARDS {
            for probe in *shard {
                counts[*probe as usize] += 1;
            }
        }
        assert!(counts.iter().all(|count| *count == 1));

        let compact_stats = shared_baseline_stats();
        let mut boundary_counts = vec![0usize; ALL_OUTER_LIMIT_PROBES.len()];
        for shard in BOUNDARY_OUTER_LIMIT_SHARDS {
            for probe in *shard {
                boundary_counts[*probe as usize] += 1;
            }
        }
        let mut boundary_aggregate_counts = vec![0usize; ALL_OUTER_LIMIT_PROBES.len()];
        for probe in BOUNDARY_AGGREGATE_OUTER_LIMIT_PROBES {
            boundary_aggregate_counts[*probe as usize] += 1;
        }
        for probe in ALL_OUTER_LIMIT_PROBES {
            let ordinal = *probe as usize;
            let is_boundary_probe = (OuterLimitProbe::HazardRanges as usize
                ..=OuterLimitProbe::BoundaryEnumerationWork as usize)
                .contains(&ordinal)
                || (OuterLimitProbe::BoundaryMappingConstructedTerms as usize
                    ..=OuterLimitProbe::BoundaryNumeratorRetainedBytes as usize)
                    .contains(&ordinal);
            assert_eq!(boundary_counts[ordinal], usize::from(is_boundary_probe));
            if is_boundary_probe {
                assert_eq!(probe.observed(compact_stats), 0, "{probe:?}");
                assert!(
                    probe.observed(shared_boundary_baseline_stats()) > 0,
                    "{probe:?}"
                );
            }
            let is_boundary_aggregate_probe = matches!(
                probe,
                OuterLimitProbe::RetainedOwnedBytes | OuterLimitProbe::CompilationOwnedPeak
            );
            assert_eq!(
                boundary_aggregate_counts[ordinal],
                usize::from(is_boundary_aggregate_probe)
            );
        }
    }

    #[test]
    fn positive_outer_limits_one_below_shard_zero_recover_the_owner() {
        run_one_below_shard(
            "exact-when-bad-materialization-one-below-shard-zero",
            OUTER_LIMIT_SHARD_ZERO,
        );
    }

    #[test]
    fn positive_outer_limits_one_below_shard_one_recover_the_owner() {
        run_one_below_shard(
            "exact-when-bad-materialization-one-below-shard-one",
            OUTER_LIMIT_SHARD_ONE,
        );
    }

    #[test]
    fn positive_outer_limits_one_below_shard_two_recover_the_owner() {
        run_one_below_shard(
            "exact-when-bad-materialization-one-below-shard-two",
            OUTER_LIMIT_SHARD_TWO,
        );
    }

    #[test]
    fn positive_outer_limits_one_below_shard_three_recover_the_owner() {
        run_one_below_shard(
            "exact-when-bad-materialization-one-below-shard-three",
            OUTER_LIMIT_SHARD_THREE,
        );
    }

    #[test]
    fn sector_011_boundary_limits_one_below_shard_zero_recover_the_owner() {
        run_boundary_one_below_shard(
            "exact-when-bad-materialization-sector-011-boundary-shard-zero",
            BOUNDARY_OUTER_LIMIT_SHARD_ZERO,
        );
    }

    #[test]
    fn sector_011_boundary_limits_one_below_shard_one_recover_the_owner() {
        run_boundary_one_below_shard(
            "exact-when-bad-materialization-sector-011-boundary-shard-one",
            BOUNDARY_OUTER_LIMIT_SHARD_ONE,
        );
    }

    #[test]
    fn sector_011_boundary_limits_one_below_shard_two_recover_the_owner() {
        run_boundary_one_below_shard(
            "exact-when-bad-materialization-sector-011-boundary-shard-two",
            BOUNDARY_OUTER_LIMIT_SHARD_TWO,
        );
    }

    #[test]
    fn sector_011_boundary_limits_one_below_shard_three_recover_the_owner() {
        run_boundary_one_below_shard(
            "exact-when-bad-materialization-sector-011-boundary-shard-three",
            BOUNDARY_OUTER_LIMIT_SHARD_THREE,
        );
    }

    #[test]
    fn sector_011_boundary_aggregate_limits_one_below_recover_the_owner() {
        run_boundary_one_below_shard(
            "exact-when-bad-materialization-sector-011-boundary-aggregate",
            BOUNDARY_AGGREGATE_OUTER_LIMIT_PROBES,
        );
    }

    #[test]
    fn admissions_arena_is_rejected_before_its_first_reserve() {
        let (family, context, session, plan) = condition_plan(
            "exact-when-bad-materialization-admissions-preallocation",
            true,
        );
        let admission_slots = plan.source_schedule().len() * size_of::<SourceMappingAdmission>();
        assert!(admission_slots > 0);
        let mut limits = GeneratedAffineResidualGroupExactWhenBadMaterializationLimits::default();
        limits.max_compilation_owned_logical_peak_upper_bound = admission_slots - 1;
        reset_materialization_admissions_reserve_observed_for_test();
        let failure = GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler::compile(
            &family, &context, &session, plan, limits,
        )
        .unwrap_err();
        assert!(matches!(
            failure.error(),
            GeneratedAffineResidualGroupExactWhenBadMaterializationError::ResourceLimit {
                resource: "exact WhenBad materialization compilation owned logical peak",
                requested,
                limit,
            } if *requested == admission_slots && *limit == admission_slots - 1
        ));
        assert!(!materialization_admissions_reserve_was_observed_for_test());
        let (_, recovered) = failure.into_parts();
        recovered.replay(&family, &context, &session).unwrap();
    }

    #[test]
    fn post_partial_ownership_panic_recovers_and_reuses_the_exact_plan() {
        let (family, context, session, plan) =
            condition_plan("exact-when-bad-materialization-partial-panic-owner", true);
        inject_materialization_partial_ownership_panic_for_test();
        let failure = GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler::compile(
            &family,
            &context,
            &session,
            plan,
            GeneratedAffineResidualGroupExactWhenBadMaterializationLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            failure.error(),
            &GeneratedAffineResidualGroupExactWhenBadMaterializationError::SymbolicaPanic
        );
        let (_, recovered) = failure.into_parts();
        recovered.replay(&family, &context, &session).unwrap();
        let materialization =
            GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler::compile(
                &family,
                &context,
                &session,
                recovered,
                GeneratedAffineResidualGroupExactWhenBadMaterializationLimits::default(),
            )
            .unwrap();
        assert_eq!(materialization.targets_consumed(), 0);
        assert!(!materialization.publishes_rule());
        materialization.replay(&family, &context, &session).unwrap();
        session.replay(&family, &context).unwrap();
    }
    #[test]
    fn panic_boundary_recovers_and_replays_the_exact_plan() {
        let (family, context, session, plan) =
            condition_plan("exact-when-bad-materialization-panic-owner", true);
        inject_materialization_boundary_panic_for_test();
        let failure = GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler::compile(
            &family,
            &context,
            &session,
            plan,
            GeneratedAffineResidualGroupExactWhenBadMaterializationLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            failure.error(),
            &GeneratedAffineResidualGroupExactWhenBadMaterializationError::SymbolicaPanic
        );
        let (_, recovered) = failure.into_parts();
        recovered.replay(&family, &context, &session).unwrap();
        session.replay(&family, &context).unwrap();
    }
}
