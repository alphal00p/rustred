//! Parametric rows bound to one authenticated generated-affine residual case.
//!
//! This is the V2 source-neutral replacement for the older branch-bound row
//! adapter.  A caller selects only an ordinal from the exact row allocation
//! owned by a [`GeneratedAffineResidualCaseAuthority`] and a sealed point from
//! the exact generated prepare-point schedule.  The caller cannot inject a
//! relation, a translation, affine geometry, or a condition.
//!
//! The source row is translated first.  Every translated guard and both halves
//! of every translated rational coefficient are then preflighted and composed
//! through the compact affine map authenticated by the case authority.  A
//! denominator or guard which maps to zero is a typed semantic outcome.  Such
//! an outcome retains no partial row, condition, or witness payload.

use std::fmt;
use std::fmt::Write as _;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::domains::integer::Integer;

use crate::affine_parametric_ordering::AffineStartIntegralComplexityKey;
use crate::generated_affine_parametric_ordering::GeneratedAffineParametricOrderingCertificate;
use crate::generated_affine_prepare_point_schedule::{
    GeneratedAffinePreparePointAuthenticationLimits,
    GeneratedAffinePreparePointAuthenticationStats, GeneratedAffinePreparePointScheduleCertificate,
    GeneratedAffinePreparePointSchedulePointHandle,
};
use crate::generated_affine_residual_case_inventory::{
    GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseSourceRecordView,
    GeneratedAffineResidualCaseSourceRowLimits, GeneratedAffineResidualCaseSourceRowStats,
    GeneratedAffineResidualInventoryGroupSourceView,
};
use crate::generated_affine_residual_case_premises::{
    GeneratedAffineResidualCasePremisesCertificate, GeneratedAffineResidualCasePremisesStats,
};
use crate::parametric_coefficient::{
    ParametricBasePolynomialAssociateLimits, PreparedResidualAffineCompactCoefficientComposition,
    PreparedResidualAffineCompactGuardComposition, ResidualAffineCoefficientComposition,
    ResidualAffineCoefficientCompositionPreflight, ResidualAffineCompactCompositionPlan,
    ResidualAffineCompactCompositionPlanLimits, ResidualAffineCompactCompositionPlanStats,
    ResidualAffineCompactMapView,
};
use crate::{
    GuardOrigin, IndexShift, IntegralFamily, ParametricArithmeticLimits,
    ParametricCoefficientContext, ParametricNonZeroCondition, ParametricPolynomial,
    ParametricRelation, ParametricRowId, ResidualUnitAffineCoefficientCompositionStats,
    ResidualUnitAffineCompositionError, ResidualUnitAffinePolynomialCompositionLimits,
    ResidualUnitAffinePolynomialCompositionStats,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_CASE_BOUND_RELATION_V2_SCHEMA: &str =
    "rustred-generated-affine-residual-case-bound-relation-v2";

#[cfg(test)]
thread_local! {
    static BOUND_RELATION_PANIC_FOR_TEST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static BOUND_RELATION_TOKEN_RESERVE_ATTEMPTS_FOR_TEST: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn inject_bound_relation_panic_for_test() {
    BOUND_RELATION_PANIC_FOR_TEST.with(|panic_next| panic_next.set(true));
}

#[cfg(test)]
fn maybe_inject_bound_relation_panic_for_test() {
    BOUND_RELATION_PANIC_FOR_TEST.with(|panic_next| {
        if panic_next.replace(false) {
            panic!("injected generated affine bound-relation panic");
        }
    });
}

#[cfg(test)]
fn reset_bound_relation_token_reserve_attempts_for_test() {
    BOUND_RELATION_TOKEN_RESERVE_ATTEMPTS_FOR_TEST.with(|attempts| attempts.set(0));
}

#[cfg(test)]
fn note_bound_relation_token_reserve_attempt_for_test() {
    BOUND_RELATION_TOKEN_RESERVE_ATTEMPTS_FOR_TEST.with(|attempts| {
        attempts.set(attempts.get().saturating_add(1));
    });
}

#[cfg(test)]
fn bound_relation_token_reserve_attempts_for_test() -> usize {
    BOUND_RELATION_TOKEN_RESERVE_ATTEMPTS_FOR_TEST.with(std::cell::Cell::get)
}

/// Complete construction/replay envelope.
///
/// Child limits are kept verbatim so the row owner can prove that source-row
/// resolution, point authentication, compact-plan compilation, translation,
/// and each selected Symbolica composition were independently admitted. Every positive
/// outer statistic has a corresponding limit below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseBoundRelationLimits {
    pub(crate) source_row: GeneratedAffineResidualCaseSourceRowLimits,
    pub(crate) point_authentication: GeneratedAffinePreparePointAuthenticationLimits,
    pub(crate) compact_plan: ResidualAffineCompactCompositionPlanLimits,
    pub(crate) translation: ParametricArithmeticLimits,
    pub(crate) polynomial_composition: ResidualUnitAffinePolynomialCompositionLimits,
    pub(crate) base_polynomial_associate: ParametricBasePolynomialAssociateLimits,
    pub(crate) max_scope_comparison_bytes: usize,
    pub(crate) max_parent_allocation_comparisons: usize,
    pub(crate) max_premise_replays: usize,
    pub(crate) max_source_row_resolutions: usize,
    pub(crate) max_case_lookups: usize,
    pub(crate) max_group_lookups: usize,
    pub(crate) max_geometry_shape_checks: usize,
    pub(crate) max_geometry_integer_entries: usize,
    pub(crate) max_geometry_integer_bits: usize,
    pub(crate) max_compact_plan_compilations: usize,
    pub(crate) max_compact_plan_replays: usize,
    pub(crate) max_translation_components: usize,
    pub(crate) max_target_row_label_bytes: usize,
    pub(crate) max_source_terms: usize,
    pub(crate) max_source_guards: usize,
    pub(crate) max_translated_terms: usize,
    pub(crate) max_translated_guards: usize,
    pub(crate) max_translation_polynomials: usize,
    pub(crate) max_translation_numerator_polynomials: usize,
    pub(crate) max_translation_denominator_polynomials: usize,
    pub(crate) max_total_translation_source_terms: usize,
    pub(crate) max_total_translation_source_exponent_entries: usize,
    pub(crate) max_total_translation_output_term_bound: usize,
    pub(crate) max_total_translation_output_exponent_entry_bound: usize,
    pub(crate) max_total_translation_power_operation_bound: usize,
    pub(crate) max_total_translation_integer_bit_work_bound: usize,
    pub(crate) max_total_translation_normalization_input_term_pairs: usize,
    pub(crate) max_total_translation_retained_output_terms: usize,
    pub(crate) max_total_translation_retained_output_bytes: usize,
    pub(crate) max_guard_composition_preflights: usize,
    pub(crate) max_coefficient_composition_preflights: usize,
    pub(crate) max_numerator_composition_preflights: usize,
    pub(crate) max_denominator_composition_preflights: usize,
    pub(crate) max_prepared_composition_token_bytes: usize,
    pub(crate) max_guard_compositions: usize,
    pub(crate) max_coefficient_compositions: usize,
    pub(crate) max_numerator_compositions: usize,
    pub(crate) max_denominator_compositions: usize,
    pub(crate) max_total_source_terms: usize,
    pub(crate) max_total_source_exponent_entries: usize,
    pub(crate) max_total_expanded_contributions: usize,
    pub(crate) max_total_output_term_bound: usize,
    pub(crate) max_total_output_terms: usize,
    pub(crate) max_total_output_exponent_entry_bound: usize,
    pub(crate) max_total_output_exponent_entries: usize,
    pub(crate) max_total_power_calls: usize,
    pub(crate) max_total_native_power_heap_pairs: usize,
    pub(crate) max_total_multiplication_term_pairs: usize,
    pub(crate) max_total_addition_term_visits: usize,
    pub(crate) max_total_native_integer_bit_work: usize,
    pub(crate) max_total_integer_bit_work: usize,
    pub(crate) max_total_normalization_input_term_pairs: usize,
    pub(crate) max_total_durable_denominator_terms: usize,
    pub(crate) max_total_durable_denominator_exponent_entries: usize,
    pub(crate) max_total_durable_denominator_integer_bits: usize,
    pub(crate) max_condition_classifications: usize,
    pub(crate) max_inherited_premise_comparisons: usize,
    pub(crate) max_inherited_premise_matches: usize,
    pub(crate) max_private_guard_associate_comparisons: usize,
    pub(crate) max_base_assumption_associate_comparisons: usize,
    pub(crate) max_row_local_base_assumptions: usize,
    pub(crate) max_private_free_index_guards: usize,
    pub(crate) max_condition_witnesses: usize,
    pub(crate) max_relation_manifest_bytes: usize,
    pub(crate) max_retained_terms: usize,
    pub(crate) max_retained_bytes: usize,
    pub(crate) max_peak_scratch_bytes: usize,
}

impl Default for GeneratedAffineResidualCaseBoundRelationLimits {
    fn default() -> Self {
        Self {
            source_row: GeneratedAffineResidualCaseSourceRowLimits::default(),
            point_authentication: GeneratedAffinePreparePointAuthenticationLimits::default(),
            compact_plan: ResidualAffineCompactCompositionPlanLimits::default(),
            translation: ParametricArithmeticLimits::default(),
            polynomial_composition: ResidualUnitAffinePolynomialCompositionLimits::default(),
            base_polynomial_associate: ParametricBasePolynomialAssociateLimits::default(),
            max_scope_comparison_bytes: 64 * 1024 * 1024,
            max_parent_allocation_comparisons: 4,
            max_premise_replays: 1,
            max_source_row_resolutions: 1,
            max_case_lookups: 1,
            max_group_lookups: 1,
            max_geometry_shape_checks: 10,
            max_geometry_integer_entries: 1_000_000_000,
            max_geometry_integer_bits: portable_usize(16_000_000_000_000_000),
            max_compact_plan_compilations: 1,
            max_compact_plan_replays: 1,
            max_translation_components: 1_000_000,
            max_target_row_label_bytes: 1024 * 1024,
            max_source_terms: 4_000_000,
            max_source_guards: 4_000_000,
            max_translated_terms: 4_000_000,
            max_translated_guards: 8_000_000,
            max_translation_polynomials: 12_000_000,
            max_translation_numerator_polynomials: 4_000_000,
            max_translation_denominator_polynomials: 4_000_000,
            max_total_translation_source_terms: portable_usize(16_000_000_000),
            max_total_translation_source_exponent_entries: portable_usize(64_000_000_000),
            max_total_translation_output_term_bound: portable_usize(32_000_000_000),
            max_total_translation_output_exponent_entry_bound: portable_usize(128_000_000_000),
            max_total_translation_power_operation_bound: portable_usize(64_000_000_000),
            max_total_translation_integer_bit_work_bound: portable_usize(64_000_000_000_000),
            max_total_translation_normalization_input_term_pairs: 128_000_000,
            max_total_translation_retained_output_terms: portable_usize(64_000_000_000),
            max_total_translation_retained_output_bytes: portable_usize(16 * 1024 * 1024 * 1024),
            max_guard_composition_preflights: 8_000_000,
            max_coefficient_composition_preflights: 4_000_000,
            max_numerator_composition_preflights: 4_000_000,
            max_denominator_composition_preflights: 4_000_000,
            max_prepared_composition_token_bytes: portable_usize(4 * 1024 * 1024 * 1024),
            max_guard_compositions: 8_000_000,
            max_coefficient_compositions: 4_000_000,
            max_numerator_compositions: 4_000_000,
            max_denominator_compositions: 4_000_000,
            max_total_source_terms: 32_000_000,
            max_total_source_exponent_entries: portable_usize(64_000_000_000),
            max_total_expanded_contributions: 32_000_000,
            max_total_output_term_bound: 32_000_000,
            max_total_output_terms: 32_000_000,
            max_total_output_exponent_entry_bound: portable_usize(64_000_000_000),
            max_total_output_exponent_entries: portable_usize(64_000_000_000),
            max_total_power_calls: portable_usize(64_000_000_000),
            max_total_native_power_heap_pairs: portable_usize(64_000_000_000),
            max_total_multiplication_term_pairs: portable_usize(64_000_000_000),
            max_total_addition_term_visits: portable_usize(64_000_000_000),
            max_total_native_integer_bit_work: portable_usize(16_000_000_000_000_000),
            max_total_integer_bit_work: portable_usize(16_000_000_000_000_000),
            max_total_normalization_input_term_pairs: 128_000_000,
            max_total_durable_denominator_terms: 32_000_000,
            max_total_durable_denominator_exponent_entries: portable_usize(64_000_000_000),
            max_total_durable_denominator_integer_bits: portable_usize(8_000_000_000_000_000),
            max_condition_classifications: 12_000_000,
            max_inherited_premise_comparisons: portable_usize(64_000_000_000),
            max_inherited_premise_matches: 12_000_000,
            max_private_guard_associate_comparisons: portable_usize(64_000_000_000),
            max_base_assumption_associate_comparisons: portable_usize(64_000_000_000),
            max_row_local_base_assumptions: 8_000_000,
            max_private_free_index_guards: 8_000_000,
            max_condition_witnesses: 12_000_000,
            max_relation_manifest_bytes: portable_usize(2 * 1024 * 1024 * 1024),
            max_retained_terms: portable_usize(64_000_000_000),
            max_retained_bytes: portable_usize(32 * 1024 * 1024 * 1024),
            max_peak_scratch_bytes: portable_usize(64 * 1024 * 1024 * 1024),
        }
    }
}

/// Exact successful-path counters plus conservative memory envelopes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseBoundRelationStats {
    source_row: GeneratedAffineResidualCaseSourceRowStats,
    point_authentication: GeneratedAffinePreparePointAuthenticationStats,
    premises: GeneratedAffineResidualCasePremisesStats,
    compact_plan: ResidualAffineCompactCompositionPlanStats,
    scope_comparison_bytes: usize,
    parent_allocation_comparisons: usize,
    premise_replays: usize,
    source_row_resolutions: usize,
    case_lookups: usize,
    group_lookups: usize,
    geometry_shape_checks: usize,
    geometry_integer_entries: usize,
    geometry_integer_bits: usize,
    compact_plan_compilations: usize,
    compact_plan_replays: usize,
    translation_components: usize,
    target_row_label_bytes: usize,
    source_terms: usize,
    source_guards: usize,
    translated_term_admission_demand: usize,
    translated_guard_admission_demand: usize,
    translated_terms: usize,
    translated_guards: usize,
    translation_polynomials: usize,
    translation_numerator_polynomials: usize,
    translation_denominator_polynomials: usize,
    translation_source_terms: usize,
    translation_source_exponent_entries: usize,
    translation_output_term_bound: usize,
    translation_output_exponent_entry_bound: usize,
    translation_power_operation_bound: usize,
    translation_integer_bit_work_bound: usize,
    translation_normalization_input_term_pairs: usize,
    translation_retained_output_terms: usize,
    translation_retained_output_bytes: usize,
    guard_composition_preflights: usize,
    coefficient_composition_preflights: usize,
    numerator_composition_preflights: usize,
    denominator_composition_preflights: usize,
    prepared_composition_token_byte_envelope: usize,
    prepared_composition_token_bytes: usize,
    guard_compositions: usize,
    coefficient_compositions: usize,
    numerator_compositions: usize,
    denominator_compositions: usize,
    preflight_total_source_terms: usize,
    preflight_total_source_exponent_entries: usize,
    preflight_total_expanded_contributions: usize,
    preflight_total_output_term_bound: usize,
    preflight_total_output_terms: usize,
    preflight_total_output_exponent_entry_bound: usize,
    preflight_total_output_exponent_entries: usize,
    preflight_total_power_calls: usize,
    preflight_total_native_power_heap_pairs: usize,
    preflight_total_multiplication_term_pairs: usize,
    preflight_total_addition_term_visits: usize,
    preflight_largest_kronecker_exponent_bits: usize,
    preflight_largest_integer_coefficient_bits: usize,
    preflight_total_native_integer_bit_work: usize,
    preflight_total_integer_bit_work: usize,
    preflight_total_normalization_input_term_pairs: usize,
    preflight_total_durable_denominator_terms: usize,
    preflight_total_durable_denominator_exponent_entries: usize,
    preflight_total_durable_denominator_integer_bits: usize,
    total_source_terms: usize,
    total_source_exponent_entries: usize,
    total_expanded_contributions: usize,
    total_output_term_bound: usize,
    total_output_terms: usize,
    total_output_exponent_entry_bound: usize,
    total_output_exponent_entries: usize,
    total_power_calls: usize,
    total_native_power_heap_pairs: usize,
    total_multiplication_term_pairs: usize,
    total_addition_term_visits: usize,
    largest_kronecker_exponent_bits: usize,
    largest_integer_coefficient_bits: usize,
    total_native_integer_bit_work: usize,
    total_integer_bit_work: usize,
    total_normalization_input_term_pairs: usize,
    total_durable_denominator_terms: usize,
    total_durable_denominator_exponent_entries: usize,
    total_durable_denominator_integer_bits: usize,
    condition_classification_admission_demand: usize,
    condition_witness_admission_demand: usize,
    inherited_premise_comparison_admission_demand: usize,
    private_guard_associate_comparison_admission_demand: usize,
    base_assumption_associate_comparison_admission_demand: usize,
    condition_classifications: usize,
    inherited_premise_comparisons: usize,
    inherited_premise_matches: usize,
    private_guard_associate_comparisons: usize,
    base_assumption_associate_comparisons: usize,
    row_local_base_assumptions: usize,
    private_free_index_guards: usize,
    condition_witnesses: usize,
    relation_manifest_bytes: usize,
    retained_term_envelope: usize,
    retained_terms: usize,
    retained_byte_envelope: usize,
    retained_bytes: usize,
    peak_scratch_byte_envelope: usize,
}

macro_rules! bound_relation_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualCaseBoundRelationStats {
    pub(crate) const fn source_row(self) -> GeneratedAffineResidualCaseSourceRowStats {
        self.source_row
    }
    pub(crate) const fn point_authentication(
        self,
    ) -> GeneratedAffinePreparePointAuthenticationStats {
        self.point_authentication
    }
    pub(crate) const fn premises(self) -> GeneratedAffineResidualCasePremisesStats {
        self.premises
    }
    pub(crate) const fn compact_plan(self) -> ResidualAffineCompactCompositionPlanStats {
        self.compact_plan
    }
    bound_relation_stats_getters!(
        scope_comparison_bytes,
        parent_allocation_comparisons,
        premise_replays,
        source_row_resolutions,
        case_lookups,
        group_lookups,
        geometry_shape_checks,
        geometry_integer_entries,
        geometry_integer_bits,
        compact_plan_compilations,
        compact_plan_replays,
        translation_components,
        target_row_label_bytes,
        source_terms,
        source_guards,
        translated_term_admission_demand,
        translated_guard_admission_demand,
        translated_terms,
        translated_guards,
        translation_polynomials,
        translation_numerator_polynomials,
        translation_denominator_polynomials,
        translation_source_terms,
        translation_source_exponent_entries,
        translation_output_term_bound,
        translation_output_exponent_entry_bound,
        translation_power_operation_bound,
        translation_integer_bit_work_bound,
        translation_normalization_input_term_pairs,
        translation_retained_output_terms,
        translation_retained_output_bytes,
        guard_composition_preflights,
        coefficient_composition_preflights,
        numerator_composition_preflights,
        denominator_composition_preflights,
        prepared_composition_token_byte_envelope,
        prepared_composition_token_bytes,
        guard_compositions,
        coefficient_compositions,
        numerator_compositions,
        denominator_compositions,
        preflight_total_source_terms,
        preflight_total_source_exponent_entries,
        preflight_total_expanded_contributions,
        preflight_total_output_term_bound,
        preflight_total_output_terms,
        preflight_total_output_exponent_entry_bound,
        preflight_total_output_exponent_entries,
        preflight_total_power_calls,
        preflight_total_native_power_heap_pairs,
        preflight_total_multiplication_term_pairs,
        preflight_total_addition_term_visits,
        preflight_largest_kronecker_exponent_bits,
        preflight_largest_integer_coefficient_bits,
        preflight_total_native_integer_bit_work,
        preflight_total_integer_bit_work,
        preflight_total_normalization_input_term_pairs,
        preflight_total_durable_denominator_terms,
        preflight_total_durable_denominator_exponent_entries,
        preflight_total_durable_denominator_integer_bits,
        total_source_terms,
        total_source_exponent_entries,
        total_expanded_contributions,
        total_output_term_bound,
        total_output_terms,
        total_output_exponent_entry_bound,
        total_output_exponent_entries,
        total_power_calls,
        total_native_power_heap_pairs,
        total_multiplication_term_pairs,
        total_addition_term_visits,
        largest_kronecker_exponent_bits,
        largest_integer_coefficient_bits,
        total_native_integer_bit_work,
        total_integer_bit_work,
        total_normalization_input_term_pairs,
        total_durable_denominator_terms,
        total_durable_denominator_exponent_entries,
        total_durable_denominator_integer_bits,
        condition_classification_admission_demand,
        condition_witness_admission_demand,
        inherited_premise_comparison_admission_demand,
        private_guard_associate_comparison_admission_demand,
        base_assumption_associate_comparison_admission_demand,
        condition_classifications,
        inherited_premise_comparisons,
        inherited_premise_matches,
        private_guard_associate_comparisons,
        base_assumption_associate_comparisons,
        row_local_base_assumptions,
        private_free_index_guards,
        condition_witnesses,
        relation_manifest_bytes,
        retained_term_envelope,
        retained_terms,
        retained_byte_envelope,
        retained_bytes,
        peak_scratch_byte_envelope,
    );
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseBoundBaseAssumption {
    condition: ParametricNonZeroCondition,
}

impl GeneratedAffineResidualCaseBoundBaseAssumption {
    pub(crate) const fn condition(&self) -> &ParametricNonZeroCondition {
        &self.condition
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCaseBoundConditionSource {
    TranslatedSourceGuard { guard_ordinal: usize },
    TranslatedSourceTermDenominator { term_ordinal: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCaseBoundConditionClass {
    DischargedNonzeroIntegerConstant,
    InheritedPremise { ordinal: usize },
    RowLocalBaseAssumption { ordinal: usize },
    PrivateFreeIndexGuard { ordinal: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseBoundConditionWitness {
    source: GeneratedAffineResidualCaseBoundConditionSource,
    class: GeneratedAffineResidualCaseBoundConditionClass,
}

impl GeneratedAffineResidualCaseBoundConditionWitness {
    pub(crate) const fn source(&self) -> GeneratedAffineResidualCaseBoundConditionSource {
        self.source
    }
    pub(crate) const fn class(&self) -> GeneratedAffineResidualCaseBoundConditionClass {
        self.class
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCaseBoundUnavailableReason {
    TranslatedSourceGuardComposesToZero { guard_ordinal: usize },
    TranslatedSourceTermDenominatorComposesToZero { term_ordinal: usize },
}

#[derive(Clone)]
struct GeneratedAffineResidualCaseBoundSource {
    authority: Arc<GeneratedAffineResidualCaseAuthority>,
    ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
    schedule: Arc<GeneratedAffinePreparePointScheduleCertificate>,
    premises: Arc<GeneratedAffineResidualCasePremisesCertificate>,
    source_row_ordinal: usize,
    point_depth: usize,
    point_ordinal: usize,
    point_key: AffineStartIntegralComplexityKey,
    target_row_id: ParametricRowId,
    composition_plan: Arc<ResidualAffineCompactCompositionPlan>,
}

impl fmt::Debug for GeneratedAffineResidualCaseBoundSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseBoundSource")
            .field("case_ordinal", &self.authority.case_ordinal())
            .field("group_ordinal", &self.authority.group_ordinal())
            .field("source_row_ordinal", &self.source_row_ordinal)
            .field("point_depth", &self.point_depth)
            .field("point_ordinal", &self.point_ordinal)
            .field("private_parent_graph", &"<redacted>")
            .field("private_point", &"<redacted>")
            .field("private_geometry", &"<redacted>")
            .finish()
    }
}

macro_rules! bound_source_scalar_accessors {
    () => {
        pub(crate) const fn source_row_ordinal(&self) -> usize {
            self.source.source_row_ordinal
        }
        pub(crate) const fn point_depth(&self) -> usize {
            self.source.point_depth
        }
        pub(crate) const fn point_ordinal(&self) -> usize {
            self.source.point_ordinal
        }
        pub(crate) fn target_row_id(&self) -> &ParametricRowId {
            &self.source.target_row_id
        }
        pub(crate) fn same_parent_allocations(
            &self,
            authority: &Arc<GeneratedAffineResidualCaseAuthority>,
            ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
            schedule: &Arc<GeneratedAffinePreparePointScheduleCertificate>,
            premises: &Arc<GeneratedAffineResidualCasePremisesCertificate>,
        ) -> bool {
            Arc::ptr_eq(&self.source.authority, authority)
                && Arc::ptr_eq(&self.source.ordering, ordering)
                && Arc::ptr_eq(&self.source.schedule, schedule)
                && Arc::ptr_eq(&self.source.premises, premises)
        }
    };
}

pub(crate) struct GeneratedAffineResidualCaseBoundUnavailableCertificate {
    schema: &'static str,
    source: GeneratedAffineResidualCaseBoundSource,
    reason: GeneratedAffineResidualCaseBoundUnavailableReason,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
    stats: GeneratedAffineResidualCaseBoundRelationStats,
}

impl fmt::Debug for GeneratedAffineResidualCaseBoundUnavailableCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseBoundUnavailableCertificate")
            .field("schema", &self.schema)
            .field("source", &self.source)
            .field("reason", &self.reason)
            .field("private_partial_payload", &"<none>")
            .finish()
    }
}

impl GeneratedAffineResidualCaseBoundUnavailableCertificate {
    bound_source_scalar_accessors!();
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) const fn reason(&self) -> GeneratedAffineResidualCaseBoundUnavailableReason {
        self.reason
    }
    pub(crate) const fn limits(&self) -> GeneratedAffineResidualCaseBoundRelationLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualCaseBoundRelationStats {
        self.stats
    }
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
        schedule: &Arc<GeneratedAffinePreparePointScheduleCertificate>,
        premises: &Arc<GeneratedAffineResidualCasePremisesCertificate>,
    ) -> Result<(), GeneratedAffineResidualCaseBoundRelationError> {
        replay_expected(
            self.schema,
            &self.source,
            self.limits,
            family,
            context,
            authority,
            ordering,
            schedule,
            premises,
            |compilation| match compilation {
                GeneratedAffineResidualCaseBoundRelationCompilation::Unavailable(other) => {
                    self.reason == other.reason
                        && self.stats == other.stats
                        && self.limits == other.limits
                        && source_payload_eq(&self.source, &other.source)
                }
                GeneratedAffineResidualCaseBoundRelationCompilation::Retained(_) => false,
            },
        )
    }
}

pub(crate) struct GeneratedAffineResidualCaseBoundParametricRelation {
    schema: &'static str,
    source: GeneratedAffineResidualCaseBoundSource,
    relation: Arc<ParametricRelation>,
    relation_manifest: Arc<String>,
    base_assumptions: Vec<GeneratedAffineResidualCaseBoundBaseAssumption>,
    condition_witnesses: Vec<GeneratedAffineResidualCaseBoundConditionWitness>,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
    stats: GeneratedAffineResidualCaseBoundRelationStats,
}

impl fmt::Debug for GeneratedAffineResidualCaseBoundParametricRelation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseBoundParametricRelation")
            .field("schema", &self.schema)
            .field("source_row_ordinal", &self.source.source_row_ordinal)
            .field("point_depth", &self.source.point_depth)
            .field("point_ordinal", &self.source.point_ordinal)
            .field("base_assumption_count", &self.base_assumptions.len())
            .field("condition_witness_count", &self.condition_witnesses.len())
            .field("private_relation", &"<redacted>")
            .field("private_manifest", &"<redacted>")
            .field("private_parent_graph", &"<redacted>")
            .finish()
    }
}

impl GeneratedAffineResidualCaseBoundParametricRelation {
    bound_source_scalar_accessors!();
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) fn relation(&self) -> &ParametricRelation {
        self.relation.as_ref()
    }
    pub(crate) fn relation_manifest(&self) -> &str {
        self.relation_manifest.as_str()
    }
    pub(crate) fn base_assumptions(&self) -> &[GeneratedAffineResidualCaseBoundBaseAssumption] {
        &self.base_assumptions
    }
    /// Test-only corruption/invariant fixture for the certificate-owned
    /// re-elimination authentication seam. Production base assumptions are
    /// created exclusively by `classify_and_retain_condition`.
    #[cfg(test)]
    pub(crate) fn push_base_assumption_for_reelimination_authentication_test(
        &mut self,
        condition: ParametricNonZeroCondition,
    ) {
        self.base_assumptions
            .push(GeneratedAffineResidualCaseBoundBaseAssumption { condition });
    }
    pub(crate) fn condition_witnesses(
        &self,
    ) -> &[GeneratedAffineResidualCaseBoundConditionWitness] {
        &self.condition_witnesses
    }
    pub(crate) const fn limits(&self) -> GeneratedAffineResidualCaseBoundRelationLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualCaseBoundRelationStats {
        self.stats
    }
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
        schedule: &Arc<GeneratedAffinePreparePointScheduleCertificate>,
        premises: &Arc<GeneratedAffineResidualCasePremisesCertificate>,
    ) -> Result<(), GeneratedAffineResidualCaseBoundRelationError> {
        replay_expected(
            self.schema,
            &self.source,
            self.limits,
            family,
            context,
            authority,
            ordering,
            schedule,
            premises,
            |compilation| match compilation {
                GeneratedAffineResidualCaseBoundRelationCompilation::Retained(other) => {
                    source_payload_eq(&self.source, &other.source)
                        && self.relation_manifest == other.relation_manifest
                        && self
                            .relation
                            .has_identical_guard_provenance(&other.relation)
                        && self.base_assumptions == other.base_assumptions
                        && self.condition_witnesses == other.condition_witnesses
                        && self.limits == other.limits
                        && self.stats == other.stats
                }
                GeneratedAffineResidualCaseBoundRelationCompilation::Unavailable(_) => false,
            },
        )
    }
}

pub(crate) enum GeneratedAffineResidualCaseBoundRelationCompilation {
    Retained(GeneratedAffineResidualCaseBoundParametricRelation),
    Unavailable(GeneratedAffineResidualCaseBoundUnavailableCertificate),
}

impl fmt::Debug for GeneratedAffineResidualCaseBoundRelationCompilation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Retained(value) => formatter.debug_tuple("Retained").field(value).finish(),
            Self::Unavailable(value) => formatter.debug_tuple("Unavailable").field(value).finish(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCaseBoundRelationError {
    SchemaMismatch,
    ReplayMismatch,
    WrongFamily,
    WrongContext,
    WrongArity,
    WrongParentAllocation,
    WrongPointBinding,
    WrongPremiseBinding,
    SourceBinding,
    GeometryBinding,
    SourceRowOutOfRange,
    ConditionMaterialization,
    RelationConstruction,
    Composition,
    AllocationFailure {
        resource: &'static str,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    RetainedByteEnvelopeExceeded,
    SymbolicaPanic,
}

impl GeneratedAffineResidualCaseBoundRelationError {
    const fn kind(self) -> &'static str {
        match self {
            Self::SchemaMismatch => "SchemaMismatch",
            Self::ReplayMismatch => "ReplayMismatch",
            Self::WrongFamily => "WrongFamily",
            Self::WrongContext => "WrongContext",
            Self::WrongArity => "WrongArity",
            Self::WrongParentAllocation => "WrongParentAllocation",
            Self::WrongPointBinding => "WrongPointBinding",
            Self::WrongPremiseBinding => "WrongPremiseBinding",
            Self::SourceBinding => "SourceBinding",
            Self::GeometryBinding => "GeometryBinding",
            Self::SourceRowOutOfRange => "SourceRowOutOfRange",
            Self::ConditionMaterialization => "ConditionMaterialization",
            Self::RelationConstruction => "RelationConstruction",
            Self::Composition => "Composition",
            Self::AllocationFailure { .. } => "AllocationFailure",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::RetainedByteEnvelopeExceeded => "RetainedByteEnvelopeExceeded",
            Self::SymbolicaPanic => "SymbolicaPanic",
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualCaseBoundRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseBoundRelationError")
            .field("kind", &self.kind())
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualCaseBoundRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "generated affine bound-relation {}", self.kind())
    }
}

impl std::error::Error for GeneratedAffineResidualCaseBoundRelationError {}

pub(crate) struct GeneratedAffineResidualCaseBoundRelationCompiler;

impl GeneratedAffineResidualCaseBoundRelationCompiler {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn compile<'schedule>(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
        schedule: Arc<GeneratedAffinePreparePointScheduleCertificate>,
        premises: Arc<GeneratedAffineResidualCasePremisesCertificate>,
        source_row_ordinal: usize,
        point: GeneratedAffinePreparePointSchedulePointHandle<'schedule>,
        limits: GeneratedAffineResidualCaseBoundRelationLimits,
    ) -> Result<
        GeneratedAffineResidualCaseBoundRelationCompilation,
        GeneratedAffineResidualCaseBoundRelationError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            compile_inner(
                family,
                context,
                authority,
                ordering,
                schedule,
                premises,
                source_row_ordinal,
                point,
                limits,
            )
        }))
        .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::SymbolicaPanic)?
    }
}

#[allow(clippy::too_many_arguments)]
fn compile_inner<'schedule>(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    authority: Arc<GeneratedAffineResidualCaseAuthority>,
    ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
    schedule: Arc<GeneratedAffinePreparePointScheduleCertificate>,
    premises: Arc<GeneratedAffineResidualCasePremisesCertificate>,
    source_row_ordinal: usize,
    point_handle: GeneratedAffinePreparePointSchedulePointHandle<'schedule>,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
) -> Result<
    GeneratedAffineResidualCaseBoundRelationCompilation,
    GeneratedAffineResidualCaseBoundRelationError,
> {
    const PARENT_ALLOCATION_COMPARISONS: usize = 4;
    const PREMISE_REPLAYS: usize = 1;
    const SOURCE_ROW_RESOLUTIONS: usize = 1;
    const CASE_LOOKUPS: usize = 1;
    const GROUP_LOOKUPS: usize = 1;
    const COMPACT_PLAN_COMPILATIONS: usize = 1;
    const COMPACT_PLAN_REPLAYS: usize = 1;

    let mut stats = GeneratedAffineResidualCaseBoundRelationStats::default();
    stats.scope_comparison_bytes = authenticate_scope(
        family,
        context,
        authority.as_ref(),
        ordering.as_ref(),
        limits.max_scope_comparison_bytes,
    )?;
    check_limit(
        "parent allocation comparisons",
        PARENT_ALLOCATION_COMPARISONS,
        limits.max_parent_allocation_comparisons,
    )?;
    stats.parent_allocation_comparisons = PARENT_ALLOCATION_COMPARISONS;
    if !schedule.same_ordering_allocation(&ordering) {
        return Err(GeneratedAffineResidualCaseBoundRelationError::WrongParentAllocation);
    }
    if !premises.same_authority_allocation(&authority) {
        return Err(GeneratedAffineResidualCaseBoundRelationError::WrongPremiseBinding);
    }
    check_limit(
        "premise replays",
        PREMISE_REPLAYS,
        limits.max_premise_replays,
    )?;
    premises
        .replay(family, context, &authority)
        .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::WrongPremiseBinding)?;
    stats.premise_replays = PREMISE_REPLAYS;
    stats.premises = premises.stats();

    let authenticated_point = schedule
        .authenticate_point_handle(
            family,
            context,
            &ordering,
            &authority,
            point_handle,
            limits.point_authentication,
        )
        .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::WrongPointBinding)?;
    if !authenticated_point.same_schedule_allocation(&schedule)
        || !authenticated_point.same_ordering_allocation(&ordering)
    {
        return Err(GeneratedAffineResidualCaseBoundRelationError::WrongPointBinding);
    }
    stats.point_authentication = authenticated_point.stats();
    stats.translation_components = authenticated_point.translation().arity();
    check_limit(
        "translation components",
        stats.translation_components,
        limits.max_translation_components,
    )?;
    if stats.translation_components != context.index_count() {
        return Err(GeneratedAffineResidualCaseBoundRelationError::WrongArity);
    }
    let point_depth = authenticated_point.depth();
    let point_ordinal = authenticated_point.point_ordinal();
    let point_key = authenticated_point.key().clone();

    check_limit(
        "source row resolutions",
        SOURCE_ROW_RESOLUTIONS,
        limits.max_source_row_resolutions,
    )?;
    if source_row_ordinal >= authority.source_row_count() {
        return Err(GeneratedAffineResidualCaseBoundRelationError::SourceRowOutOfRange);
    }
    let source_row = authority
        .authenticated_source_row_view(family, context, source_row_ordinal, limits.source_row)
        .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::SourceBinding)?;
    stats.source_row_resolutions = SOURCE_ROW_RESOLUTIONS;
    stats.source_row = source_row.stats();
    let source = source_row.relation();
    stats.source_terms = source.terms().len();
    stats.source_guards = source.guarded_nonzero_conditions().len();
    check_limit("source terms", stats.source_terms, limits.max_source_terms)?;
    check_limit(
        "source guards",
        stats.source_guards,
        limits.max_source_guards,
    )?;
    if source.family_fingerprint() != family.fingerprint_ref()
        || source.context_fingerprint() != context.fingerprint()
    {
        return Err(GeneratedAffineResidualCaseBoundRelationError::SourceBinding);
    }
    if source.arity() != context.index_count() {
        return Err(GeneratedAffineResidualCaseBoundRelationError::WrongArity);
    }

    check_limit("case lookups", CASE_LOOKUPS, limits.max_case_lookups)?;
    let case = authority
        .authenticated_source_neutral_case_view(context)
        .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::SourceBinding)?;
    stats.case_lookups = CASE_LOOKUPS;
    check_limit("group lookups", GROUP_LOOKUPS, limits.max_group_lookups)?;
    let group = authority
        .authenticated_source_neutral_group_view(context)
        .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::SourceBinding)?;
    stats.group_lookups = GROUP_LOOKUPS;
    authenticate_geometry(authority.as_ref(), case, group, limits, &mut stats)?;
    let geometry = ResidualAffineCompactMapView::new(
        context.fingerprint(),
        group.ambient_arity(),
        case.constants(),
        group.free_positions(),
        group.compact_linear_coefficients(),
    );

    check_limit(
        "compact plan compilations",
        COMPACT_PLAN_COMPILATIONS,
        limits.max_compact_plan_compilations,
    )?;
    let composition_plan = Arc::new(
        context
            .compile_residual_affine_compact_composition_plan(geometry, limits.compact_plan)
            .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::Composition)?,
    );
    stats.compact_plan_compilations = COMPACT_PLAN_COMPILATIONS;
    stats.compact_plan = composition_plan.stats();
    check_limit(
        "compact plan replays",
        COMPACT_PLAN_REPLAYS,
        limits.max_compact_plan_replays,
    )?;
    composition_plan
        .replay(context, geometry)
        .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::Composition)?;
    stats.compact_plan_replays = COMPACT_PLAN_REPLAYS;

    let target_row_label_bytes = derived_target_row_label_len(
        authority.as_ref(),
        source_row_ordinal,
        point_depth,
        point_ordinal,
        limits.max_target_row_label_bytes,
    )?;
    stats.target_row_label_bytes = target_row_label_bytes;
    preflight_translation(
        context,
        source,
        authenticated_point.translation(),
        limits,
        &mut stats,
    )?;
    let pre_translation_peak =
        pre_translation_peak_envelope(&stats, target_row_label_bytes, composition_plan.stats())?;
    check_limit(
        "peak scratch bytes",
        pre_translation_peak,
        limits.max_peak_scratch_bytes,
    )?;

    // The first locally owned diagnostic string is allocated only after the
    // complete source, geometry, plan, and translation census has passed.
    let target_row_id = derived_target_row_id(
        authority.as_ref(),
        source_row_ordinal,
        point_depth,
        point_ordinal,
        target_row_label_bytes,
    )?;
    let translated = source
        .translated(
            context,
            authenticated_point.translation(),
            target_row_id.clone(),
            limits.translation,
        )
        .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::RelationConstruction)?;
    stats.translated_terms = translated.terms().len();
    stats.translated_guards = translated.guarded_nonzero_conditions().len();
    if stats.translated_terms > stats.translated_term_admission_demand
        || stats.translated_guards > stats.translated_guard_admission_demand
    {
        return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
    }
    check_limit(
        "translated terms",
        stats.translated_terms,
        limits.max_translated_terms,
    )?;
    check_limit(
        "translated guards",
        stats.translated_guards,
        limits.max_translated_guards,
    )?;

    // Token vectors are the first allocations made by the complete-row
    // composition preflight.  Admit their count-only logical envelope against
    // the global concurrently-live peak before either vector is reserved.
    // The later, stronger composition peak remains the gate before either
    // selected Symbolica composition backend is entered.
    let prepared_token_byte_envelope =
        prepared_composition_token_byte_envelope(stats.translated_guards, stats.translated_terms)?;
    check_limit(
        "prepared composition token bytes",
        prepared_token_byte_envelope,
        limits.max_prepared_composition_token_bytes,
    )?;
    let pre_token_allocation_peak = pre_token_allocation_peak_envelope(
        translated.owned_retained_byte_bound().ok_or(
            GeneratedAffineResidualCaseBoundRelationError::ResourceCountOverflow {
                resource: "translated relation retained bytes",
            },
        )?,
        target_row_label_bytes,
        composition_plan.stats(),
        prepared_token_byte_envelope,
    )?;
    check_limit(
        "peak scratch bytes",
        pre_token_allocation_peak,
        limits.max_peak_scratch_bytes,
    )?;

    let prepared = preflight_complete_row_compositions(
        context,
        &translated,
        composition_plan.as_ref(),
        limits,
    )?;
    copy_composition_preflight_census(&prepared.stats, &mut stats);
    stats.retained_term_envelope = prospective_retained_term_envelope(&stats)?;
    check_limit(
        "retained terms",
        stats.retained_term_envelope,
        limits.max_retained_terms,
    )?;
    stats.retained_byte_envelope = prospective_retained_byte_envelope(
        &stats,
        target_row_label_bytes,
        composition_plan.stats(),
        limits.max_relation_manifest_bytes,
    )?;
    check_limit(
        "retained bytes",
        stats.retained_byte_envelope,
        limits.max_retained_bytes,
    )?;
    stats.peak_scratch_byte_envelope = composition_peak_envelope(
        &stats,
        translated.owned_retained_byte_bound().ok_or(
            GeneratedAffineResidualCaseBoundRelationError::ResourceCountOverflow {
                resource: "translated relation retained bytes",
            },
        )?,
        composition_plan.stats(),
    )?;
    check_limit(
        "peak scratch bytes",
        stats.peak_scratch_byte_envelope,
        limits.max_peak_scratch_bytes,
    )?;

    #[cfg(test)]
    maybe_inject_bound_relation_panic_for_test();

    let source_binding = GeneratedAffineResidualCaseBoundSource {
        authority,
        ordering,
        schedule,
        premises,
        source_row_ordinal,
        point_depth,
        point_ordinal,
        point_key,
        target_row_id: target_row_id.clone(),
        composition_plan: Arc::clone(&composition_plan),
    };
    execute_complete_row(
        context,
        &translated,
        source_binding,
        limits,
        stats,
        prepared,
    )
}

fn authenticate_scope(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    authority: &GeneratedAffineResidualCaseAuthority,
    ordering: &GeneratedAffineParametricOrderingCertificate,
    max_bytes: usize,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    let bytes = checked_sum(
        "scope comparison bytes",
        [
            family.fingerprint_ref().len(),
            authority.family_fingerprint().len(),
            ordering.family_fingerprint().len(),
            context.fingerprint().len(),
            authority.context_fingerprint().len(),
            ordering.context_fingerprint().len(),
        ],
    )?;
    check_limit("scope comparison bytes", bytes, max_bytes)?;
    if family.fingerprint_ref() != authority.family_fingerprint()
        || family.fingerprint_ref() != ordering.family_fingerprint()
    {
        return Err(GeneratedAffineResidualCaseBoundRelationError::WrongFamily);
    }
    if context.fingerprint() != authority.context_fingerprint()
        || context.fingerprint() != ordering.context_fingerprint()
    {
        return Err(GeneratedAffineResidualCaseBoundRelationError::WrongContext);
    }
    if context.index_count() != authority.arity()
        || context.index_count() != ordering.arity()
        || authority.sector().arity() != authority.arity()
    {
        return Err(GeneratedAffineResidualCaseBoundRelationError::WrongArity);
    }
    Ok(bytes)
}

const GEOMETRY_SHAPE_CHECKS: usize = 10;

fn authenticate_geometry(
    authority: &GeneratedAffineResidualCaseAuthority,
    case: GeneratedAffineResidualCaseSourceRecordView<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
    stats: &mut GeneratedAffineResidualCaseBoundRelationStats,
) -> Result<(), GeneratedAffineResidualCaseBoundRelationError> {
    check_limit(
        "geometry shape checks",
        GEOMETRY_SHAPE_CHECKS,
        limits.max_geometry_shape_checks,
    )?;
    stats.geometry_shape_checks = GEOMETRY_SHAPE_CHECKS;
    let arity = authority.arity();
    let free_count = group.free_positions().len();
    let compact_entries = checked_mul("compact geometry entries", arity, free_count)?;
    if case.ordinal() != authority.case_ordinal()
        || case.group_ordinal() != authority.group_ordinal()
        || group.ordinal() != authority.group_ordinal()
        || group.ambient_arity() != arity
        || case.constants().len() != arity
        || group.compact_linear_coefficients().len() != compact_entries
        || group.anchor_offsets().len() != group.case_ordinals().len()
        || group
            .case_ordinals()
            .get(case.ordinal_within_group())
            .copied()
            != Some(case.ordinal())
    {
        return Err(GeneratedAffineResidualCaseBoundRelationError::GeometryBinding);
    }
    stats.geometry_integer_entries = checked_add(
        "geometry integer entries",
        case.constants().len(),
        group.compact_linear_coefficients().len(),
    )?;
    check_limit(
        "geometry integer entries",
        stats.geometry_integer_entries,
        limits.max_geometry_integer_entries,
    )?;
    let mut bits = 0usize;
    for value in case
        .constants()
        .iter()
        .chain(group.compact_linear_coefficients())
    {
        bits = bounded_add(
            "geometry integer bits",
            bits,
            integer_magnitude_bits(value)?,
            limits.max_geometry_integer_bits,
        )?;
    }
    stats.geometry_integer_bits = bits;
    Ok(())
}

fn derived_target_row_label_len(
    authority: &GeneratedAffineResidualCaseAuthority,
    source_row_ordinal: usize,
    point_depth: usize,
    point_ordinal: usize,
    max_bytes: usize,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    let bytes = checked_sum(
        "target row label bytes",
        [
            GENERATED_AFFINE_RESIDUAL_CASE_BOUND_RELATION_V2_SCHEMA.len(),
            "|case=".len(),
            decimal_digits_usize(authority.case_ordinal()),
            "|group=".len(),
            decimal_digits_usize(authority.group_ordinal()),
            "|row=".len(),
            decimal_digits_usize(source_row_ordinal),
            "|depth=".len(),
            decimal_digits_usize(point_depth),
            "|point=".len(),
            decimal_digits_usize(point_ordinal),
        ],
    )?;
    check_limit("target row label bytes", bytes, max_bytes)?;
    Ok(bytes)
}

fn derived_target_row_id(
    authority: &GeneratedAffineResidualCaseAuthority,
    source_row_ordinal: usize,
    point_depth: usize,
    point_ordinal: usize,
    exact_bytes: usize,
) -> Result<ParametricRowId, GeneratedAffineResidualCaseBoundRelationError> {
    let mut label = String::new();
    label.try_reserve_exact(exact_bytes).map_err(|_| {
        GeneratedAffineResidualCaseBoundRelationError::AllocationFailure {
            resource: "target row label bytes",
        }
    })?;
    write!(
        &mut label,
        "{}|case={}|group={}|row={}|depth={}|point={}",
        GENERATED_AFFINE_RESIDUAL_CASE_BOUND_RELATION_V2_SCHEMA,
        authority.case_ordinal(),
        authority.group_ordinal(),
        source_row_ordinal,
        point_depth,
        point_ordinal,
    )
    .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch)?;
    if label.len() != exact_bytes {
        return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
    }
    Ok(ParametricRowId::Derived {
        label: Arc::from(label),
    })
}

fn preflight_translation(
    context: &ParametricCoefficientContext,
    source: &ParametricRelation,
    translation: &IndexShift,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
    stats: &mut GeneratedAffineResidualCaseBoundRelationStats,
) -> Result<(), GeneratedAffineResidualCaseBoundRelationError> {
    let guard_count = source.guarded_nonzero_conditions().len();
    let term_count = source.terms().len();
    stats.translation_polynomials = checked_add(
        "translation polynomials",
        guard_count,
        checked_mul("translation polynomials", term_count, 2)?,
    )?;
    stats.translation_numerator_polynomials = term_count;
    stats.translation_denominator_polynomials = term_count;
    check_limit(
        "translation polynomials",
        stats.translation_polynomials,
        limits.max_translation_polynomials,
    )?;
    check_limit(
        "translation numerator polynomials",
        term_count,
        limits.max_translation_numerator_polynomials,
    )?;
    check_limit(
        "translation denominator polynomials",
        term_count,
        limits.max_translation_denominator_polynomials,
    )?;
    // Uniform key translation is injective and can add at most one normalized
    // denominator guard for each source term.
    stats.translated_term_admission_demand = term_count;
    stats.translated_guard_admission_demand =
        checked_add("translated guards", guard_count, term_count)?;
    check_limit(
        "translated terms",
        stats.translated_term_admission_demand,
        limits.max_translated_terms,
    )?;
    check_limit(
        "translated guards",
        stats.translated_guard_admission_demand,
        limits.max_translated_guards,
    )?;
    for guard in source.guarded_nonzero_conditions() {
        let item = context
            .preflight_translate_polynomial(guard.polynomial(), translation, limits.translation)
            .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::Composition)?;
        consume_translation_polynomial(stats, item, 2, limits)?;
    }
    for coefficient in source.terms().values() {
        let item = context
            .preflight_translate_coefficient(coefficient, translation, limits.translation)
            .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::Composition)?;
        consume_translation_polynomial(stats, item.numerator(), 2, limits)?;
        consume_translation_polynomial(stats, item.denominator(), 2, limits)?;
        stats.translation_normalization_input_term_pairs = bounded_add(
            "translation normalization input term pairs",
            stats.translation_normalization_input_term_pairs,
            item.normalization_input_term_pair_bound(),
            limits.max_total_translation_normalization_input_term_pairs,
        )?;
        stats.translation_retained_output_terms = bounded_add(
            "translation retained output terms",
            stats.translation_retained_output_terms,
            item.normalized_coefficient_term_bound(),
            limits.max_total_translation_retained_output_terms,
        )?;
        stats.translation_retained_output_bytes = bounded_add(
            "translation retained output bytes",
            stats.translation_retained_output_bytes,
            item.normalized_coefficient_byte_bound(),
            limits.max_total_translation_retained_output_bytes,
        )?;
    }
    Ok(())
}

fn consume_translation_polynomial(
    stats: &mut GeneratedAffineResidualCaseBoundRelationStats,
    item: crate::parametric_coefficient::ParametricPolynomialTranslationPreflight,
    retained_copies: usize,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
) -> Result<(), GeneratedAffineResidualCaseBoundRelationError> {
    stats.translation_source_terms = bounded_add(
        "translation source terms",
        stats.translation_source_terms,
        item.source_terms(),
        limits.max_total_translation_source_terms,
    )?;
    stats.translation_source_exponent_entries = bounded_add(
        "translation source exponent entries",
        stats.translation_source_exponent_entries,
        item.source_exponent_entries(),
        limits.max_total_translation_source_exponent_entries,
    )?;
    stats.translation_output_term_bound = bounded_add(
        "translation output term bound",
        stats.translation_output_term_bound,
        item.output_term_bound(),
        limits.max_total_translation_output_term_bound,
    )?;
    stats.translation_output_exponent_entry_bound = bounded_add(
        "translation output exponent entry bound",
        stats.translation_output_exponent_entry_bound,
        item.output_exponent_entry_bound(),
        limits.max_total_translation_output_exponent_entry_bound,
    )?;
    stats.translation_power_operation_bound = bounded_add(
        "translation power operation bound",
        stats.translation_power_operation_bound,
        item.power_operation_bound(),
        limits.max_total_translation_power_operation_bound,
    )?;
    stats.translation_integer_bit_work_bound = bounded_add(
        "translation integer bit work bound",
        stats.translation_integer_bit_work_bound,
        item.integer_bit_work_bound(),
        limits.max_total_translation_integer_bit_work_bound,
    )?;
    stats.translation_retained_output_terms = bounded_add(
        "translation retained output terms",
        stats.translation_retained_output_terms,
        checked_mul(
            "translation retained output terms",
            item.retained_output_term_bound(),
            retained_copies,
        )?,
        limits.max_total_translation_retained_output_terms,
    )?;
    stats.translation_retained_output_bytes = bounded_add(
        "translation retained output bytes",
        stats.translation_retained_output_bytes,
        checked_mul(
            "translation retained output bytes",
            item.retained_output_byte_bound(),
            retained_copies,
        )?,
        limits.max_total_translation_retained_output_bytes,
    )?;
    Ok(())
}

/// Complete-row algebra census.  Apart from exactly reserved sealed-token
/// vectors, no result payload is allocated and no selected Symbolica affine
/// composition backend is entered until this function has visited every
/// translated guard and both halves of every translated coefficient.
struct CompleteRowCompositionPreflight<'prepared> {
    stats: GeneratedAffineResidualCaseBoundRelationStats,
    guards: Vec<PreparedResidualAffineCompactGuardComposition<'prepared>>,
    coefficients: Vec<PreparedResidualAffineCompactCoefficientComposition<'prepared>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AggregateCompositionClampProvenance {
    source_terms: bool,
    source_exponent_entries: bool,
    expanded_contributions: bool,
    output_term_bound: bool,
    output_exponent_entry_bound: bool,
    power_calls: bool,
    native_power_heap_pairs: bool,
    multiplication_term_pairs: bool,
    addition_term_visits: bool,
    integer_bit_work: bool,
    normalization_input_term_pairs: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AggregateCompositionCallLimits {
    effective: ResidualUnitAffinePolynomialCompositionLimits,
    clamps: AggregateCompositionClampProvenance,
}

fn prepared_composition_token_byte_envelope(
    guard_count: usize,
    coefficient_count: usize,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    let logical_token_bytes = checked_sum(
        "prepared composition token bytes",
        [
            checked_mul(
                "prepared composition token bytes",
                guard_count,
                size_of::<PreparedResidualAffineCompactGuardComposition<'_>>(),
            )?,
            checked_mul(
                "prepared composition token bytes",
                coefficient_count,
                size_of::<PreparedResidualAffineCompactCoefficientComposition<'_>>(),
            )?,
        ],
    )?;
    checked_mul("prepared composition token bytes", logical_token_bytes, 2)
}

fn preflight_complete_row_compositions<'prepared>(
    context: &'prepared ParametricCoefficientContext,
    translated: &'prepared ParametricRelation,
    plan: &'prepared ResidualAffineCompactCompositionPlan,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
) -> Result<CompleteRowCompositionPreflight<'prepared>, GeneratedAffineResidualCaseBoundRelationError>
{
    let mut prospective = GeneratedAffineResidualCaseBoundRelationStats::default();
    let guard_count = translated.guarded_nonzero_conditions().len();
    let coefficient_count = translated.terms().len();
    for (resource, requested, limit) in [
        (
            "guard composition preflights",
            guard_count,
            limits.max_guard_composition_preflights,
        ),
        (
            "coefficient composition preflights",
            coefficient_count,
            limits.max_coefficient_composition_preflights,
        ),
        (
            "numerator composition preflights",
            coefficient_count,
            limits.max_numerator_composition_preflights,
        ),
        (
            "denominator composition preflights",
            coefficient_count,
            limits.max_denominator_composition_preflights,
        ),
        (
            "guard compositions",
            guard_count,
            limits.max_guard_compositions,
        ),
        (
            "coefficient compositions",
            coefficient_count,
            limits.max_coefficient_compositions,
        ),
        (
            "numerator compositions",
            coefficient_count,
            limits.max_numerator_compositions,
        ),
        (
            "denominator compositions",
            coefficient_count,
            limits.max_denominator_compositions,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    prospective.guard_composition_preflights = guard_count;
    prospective.coefficient_composition_preflights = coefficient_count;
    prospective.numerator_composition_preflights = coefficient_count;
    prospective.denominator_composition_preflights = coefficient_count;

    prospective.prepared_composition_token_byte_envelope =
        prepared_composition_token_byte_envelope(guard_count, coefficient_count)?;
    check_limit(
        "prepared composition token bytes",
        prospective.prepared_composition_token_byte_envelope,
        limits.max_prepared_composition_token_bytes,
    )?;
    let mut guards = Vec::new();
    #[cfg(test)]
    note_bound_relation_token_reserve_attempt_for_test();
    guards.try_reserve_exact(guard_count).map_err(|_| {
        GeneratedAffineResidualCaseBoundRelationError::AllocationFailure {
            resource: "prepared guard composition tokens",
        }
    })?;
    let mut coefficients = Vec::new();
    #[cfg(test)]
    note_bound_relation_token_reserve_attempt_for_test();
    coefficients
        .try_reserve_exact(coefficient_count)
        .map_err(
            |_| GeneratedAffineResidualCaseBoundRelationError::AllocationFailure {
                resource: "prepared coefficient composition tokens",
            },
        )?;
    prospective.prepared_composition_token_bytes = checked_sum(
        "prepared composition token bytes",
        [
            checked_mul(
                "prepared composition token bytes",
                guards.capacity(),
                size_of::<PreparedResidualAffineCompactGuardComposition<'_>>(),
            )?,
            checked_mul(
                "prepared composition token bytes",
                coefficients.capacity(),
                size_of::<PreparedResidualAffineCompactCoefficientComposition<'_>>(),
            )?,
        ],
    )?;
    check_limit(
        "prepared composition token bytes",
        prospective.prepared_composition_token_bytes,
        limits.max_prepared_composition_token_bytes,
    )?;
    if prospective.prepared_composition_token_bytes
        > prospective.prepared_composition_token_byte_envelope
    {
        return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
    }

    for guard in translated.guarded_nonzero_conditions() {
        let call_limits = remaining_composition_limits(limits, &prospective)?;
        let item = context
            .prepare_guard_on_residual_affine_compact_composition_plan(
                guard.polynomial(),
                plan,
                call_limits.effective,
            )
            .map_err(|error| {
                map_prepared_composition_error(error, call_limits, limits, &prospective)
            })?;
        consume_polynomial_stats(&mut prospective, item.stats(), limits)?;
        guards.push(item);
    }
    for coefficient in translated.terms().values() {
        let call_limits = remaining_composition_limits(limits, &prospective)?;
        let item = context
            .prepare_coefficient_on_residual_affine_compact_composition_plan(
                coefficient,
                plan,
                call_limits.effective,
            )
            .map_err(|error| {
                map_prepared_composition_error(error, call_limits, limits, &prospective)
            })?;
        consume_coefficient_preflight(&mut prospective, item.stats(), limits)?;
        coefficients.push(item);
    }
    if guards.len() != guard_count || coefficients.len() != coefficient_count {
        return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
    }
    Ok(CompleteRowCompositionPreflight {
        stats: prospective,
        guards,
        coefficients,
    })
}

fn copy_composition_preflight_census(
    prospective: &GeneratedAffineResidualCaseBoundRelationStats,
    stats: &mut GeneratedAffineResidualCaseBoundRelationStats,
) {
    stats.guard_composition_preflights = prospective.guard_composition_preflights;
    stats.coefficient_composition_preflights = prospective.coefficient_composition_preflights;
    stats.numerator_composition_preflights = prospective.numerator_composition_preflights;
    stats.denominator_composition_preflights = prospective.denominator_composition_preflights;
    stats.prepared_composition_token_byte_envelope =
        prospective.prepared_composition_token_byte_envelope;
    stats.prepared_composition_token_bytes = prospective.prepared_composition_token_bytes;
    stats.preflight_total_source_terms = prospective.total_source_terms;
    stats.preflight_total_source_exponent_entries = prospective.total_source_exponent_entries;
    stats.preflight_total_expanded_contributions = prospective.total_expanded_contributions;
    stats.preflight_total_output_term_bound = prospective.total_output_term_bound;
    stats.preflight_total_output_terms = prospective.total_output_terms;
    stats.preflight_total_output_exponent_entry_bound =
        prospective.total_output_exponent_entry_bound;
    stats.preflight_total_output_exponent_entries = prospective.total_output_exponent_entries;
    stats.preflight_total_power_calls = prospective.total_power_calls;
    stats.preflight_total_native_power_heap_pairs = prospective.total_native_power_heap_pairs;
    stats.preflight_total_multiplication_term_pairs = prospective.total_multiplication_term_pairs;
    stats.preflight_total_addition_term_visits = prospective.total_addition_term_visits;
    stats.preflight_largest_kronecker_exponent_bits = prospective.largest_kronecker_exponent_bits;
    stats.preflight_largest_integer_coefficient_bits = prospective.largest_integer_coefficient_bits;
    stats.preflight_total_native_integer_bit_work = prospective.total_native_integer_bit_work;
    stats.preflight_total_integer_bit_work = prospective.total_integer_bit_work;
    stats.preflight_total_normalization_input_term_pairs =
        prospective.total_normalization_input_term_pairs;
    stats.preflight_total_durable_denominator_terms = prospective.total_durable_denominator_terms;
    stats.preflight_total_durable_denominator_exponent_entries =
        prospective.total_durable_denominator_exponent_entries;
    stats.preflight_total_durable_denominator_integer_bits =
        prospective.total_durable_denominator_integer_bits;
}

fn remaining_composition_limits(
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
    stats: &GeneratedAffineResidualCaseBoundRelationStats,
) -> Result<AggregateCompositionCallLimits, GeneratedAffineResidualCaseBoundRelationError> {
    let mut effective = limits.polynomial_composition;
    let mut clamps = AggregateCompositionClampProvenance::default();
    macro_rules! clamp_remaining {
        ($field:ident, $clamp:ident, $used:ident, $total:ident, $name:literal) => {{
            let aggregate_remaining = remaining($name, limits.$total, stats.$used)?;
            if aggregate_remaining < effective.$field {
                effective.$field = aggregate_remaining;
                clamps.$clamp = true;
            }
        }};
    }
    clamp_remaining!(
        max_source_terms,
        source_terms,
        total_source_terms,
        max_total_source_terms,
        "total source terms"
    );
    clamp_remaining!(
        max_source_exponent_entries,
        source_exponent_entries,
        total_source_exponent_entries,
        max_total_source_exponent_entries,
        "total source exponent entries"
    );
    clamp_remaining!(
        max_expanded_contributions,
        expanded_contributions,
        total_expanded_contributions,
        max_total_expanded_contributions,
        "total expanded contributions"
    );
    clamp_remaining!(
        max_output_terms,
        output_term_bound,
        total_output_term_bound,
        max_total_output_term_bound,
        "total output term bound"
    );
    clamp_remaining!(
        max_output_exponent_entries,
        output_exponent_entry_bound,
        total_output_exponent_entry_bound,
        max_total_output_exponent_entry_bound,
        "total output exponent entry bound"
    );
    clamp_remaining!(
        max_power_calls,
        power_calls,
        total_power_calls,
        max_total_power_calls,
        "total power calls"
    );
    clamp_remaining!(
        max_native_power_heap_pairs,
        native_power_heap_pairs,
        total_native_power_heap_pairs,
        max_total_native_power_heap_pairs,
        "total native power heap pairs"
    );
    clamp_remaining!(
        max_multiplication_term_pairs,
        multiplication_term_pairs,
        total_multiplication_term_pairs,
        max_total_multiplication_term_pairs,
        "total multiplication term pairs"
    );
    clamp_remaining!(
        max_addition_term_visits,
        addition_term_visits,
        total_addition_term_visits,
        max_total_addition_term_visits,
        "total addition term visits"
    );
    clamp_remaining!(
        max_integer_bit_work,
        integer_bit_work,
        total_integer_bit_work,
        max_total_integer_bit_work,
        "total integer bit work"
    );
    clamp_remaining!(
        max_normalization_input_term_pairs,
        normalization_input_term_pairs,
        total_normalization_input_term_pairs,
        max_total_normalization_input_term_pairs,
        "total normalization input term pairs"
    );
    Ok(AggregateCompositionCallLimits { effective, clamps })
}

fn map_prepared_composition_error(
    error: ResidualUnitAffineCompositionError,
    call: AggregateCompositionCallLimits,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
    stats: &GeneratedAffineResidualCaseBoundRelationStats,
) -> GeneratedAffineResidualCaseBoundRelationError {
    let ResidualUnitAffineCompositionError::ResourceLimit {
        resource,
        requested,
        limit: child_limit,
    } = error
    else {
        return GeneratedAffineResidualCaseBoundRelationError::Composition;
    };

    let remap = |outer_resource: &'static str,
                 spent_before_call: usize,
                 effective_call_limit: usize,
                 outer_limit: usize| {
        let Some(spent_inside_call) = effective_call_limit.checked_sub(child_limit) else {
            return GeneratedAffineResidualCaseBoundRelationError::Composition;
        };
        match checked_sum(
            outer_resource,
            [spent_before_call, spent_inside_call, requested],
        ) {
            Ok(requested) => GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
                resource: outer_resource,
                requested,
                limit: outer_limit,
            },
            Err(error) => error,
        }
    };

    macro_rules! direct_remap {
        ($clamp:ident, $field:ident, $used:ident, $outer:ident, $outer_name:literal, [$($child_name:literal),+ $(,)?]) => {
            if call.clamps.$clamp && matches!(resource, $($child_name)|+) {
                return remap(
                    $outer_name,
                    stats.$used,
                    call.effective.$field,
                    limits.$outer,
                );
            }
        };
    }

    direct_remap!(
        source_terms,
        max_source_terms,
        total_source_terms,
        max_total_source_terms,
        "total source terms",
        ["polynomial source terms"]
    );
    direct_remap!(
        source_exponent_entries,
        max_source_exponent_entries,
        total_source_exponent_entries,
        max_total_source_exponent_entries,
        "total source exponent entries",
        ["polynomial source exponent entries"]
    );

    // `heap_pow` receives min(expanded, output, exact-algebra) as one shared
    // cap and reports the distinct affine-power resource. Resolve that
    // shared minimum when one outer-clamped field is the unique controller,
    // or deterministically as expanded when both outer fields impose the same
    // strict cap. A tie with an unclamped sibling remains Composition.
    // `child_limit` is compared with the unchanged exact-algebra cap because
    // coefficient preflight subtracts numerator work before applying the
    // denominator's child limits.
    if resource == "affine power terms" {
        let exact_cap = call.effective.exact_algebra.max_polynomial_terms;
        if child_limit < exact_cap {
            if call.clamps.expanded_contributions
                && (call.effective.max_expanded_contributions < call.effective.max_output_terms
                    || (call.effective.max_expanded_contributions
                        == call.effective.max_output_terms
                        && call.clamps.output_term_bound))
            {
                return remap(
                    "total expanded contributions",
                    stats.total_expanded_contributions,
                    call.effective.max_expanded_contributions,
                    limits.max_total_expanded_contributions,
                );
            }
            if call.clamps.output_term_bound
                && call.effective.max_output_terms < call.effective.max_expanded_contributions
            {
                return remap(
                    "total output term bound",
                    stats.total_output_term_bound,
                    call.effective.max_output_terms,
                    limits.max_total_output_term_bound,
                );
            }
        }
    }
    if call.clamps.expanded_contributions
        && resource == "expanded polynomial contributions"
        && child_limit < call.effective.exact_algebra.max_polynomial_terms
        && (call.effective.max_output_terms > call.effective.max_expanded_contributions
            || call.clamps.output_term_bound)
    {
        return remap(
            "total expanded contributions",
            stats.total_expanded_contributions,
            call.effective.max_expanded_contributions,
            limits.max_total_expanded_contributions,
        );
    }
    if call.clamps.output_term_bound
        && resource == "prospective output terms"
        && child_limit < call.effective.exact_algebra.max_polynomial_terms
    {
        return remap(
            "total output term bound",
            stats.total_output_term_bound,
            call.effective.max_output_terms,
            limits.max_total_output_term_bound,
        );
    }
    direct_remap!(
        output_exponent_entry_bound,
        max_output_exponent_entries,
        total_output_exponent_entry_bound,
        max_total_output_exponent_entry_bound,
        "total output exponent entry bound",
        ["prospective output exponent entries"]
    );
    direct_remap!(
        power_calls,
        max_power_calls,
        total_power_calls,
        max_total_power_calls,
        "total power calls",
        ["native power calls"]
    );
    direct_remap!(
        native_power_heap_pairs,
        max_native_power_heap_pairs,
        total_native_power_heap_pairs,
        max_total_native_power_heap_pairs,
        "total native power heap pairs",
        ["native power heap pairs"]
    );
    direct_remap!(
        multiplication_term_pairs,
        max_multiplication_term_pairs,
        total_multiplication_term_pairs,
        max_total_multiplication_term_pairs,
        "total multiplication term pairs",
        ["native multiplication term pairs"]
    );
    direct_remap!(
        addition_term_visits,
        max_addition_term_visits,
        total_addition_term_visits,
        max_total_addition_term_visits,
        "total addition term visits",
        [
            "native addition term visits",
            "Symbolica backend structural term visits"
        ]
    );
    direct_remap!(
        integer_bit_work,
        max_integer_bit_work,
        total_integer_bit_work,
        max_total_integer_bit_work,
        "total integer bit work",
        [
            "integer bit work",
            "coefficient total integer-bit work bound"
        ]
    );
    if call.clamps.normalization_input_term_pairs
        && resource == "coefficient normalization input term-pair bound"
        && child_limit < call.effective.exact_algebra.max_term_operations
    {
        return remap(
            "total normalization input term pairs",
            stats.total_normalization_input_term_pairs,
            call.effective.max_normalization_input_term_pairs,
            limits.max_total_normalization_input_term_pairs,
        );
    }
    GeneratedAffineResidualCaseBoundRelationError::Composition
}

fn consume_polynomial_stats(
    stats: &mut GeneratedAffineResidualCaseBoundRelationStats,
    item: ResidualUnitAffinePolynomialCompositionStats,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
) -> Result<(), GeneratedAffineResidualCaseBoundRelationError> {
    macro_rules! add {
        ($field:ident, $value:expr, $limit:ident, $name:literal) => {
            stats.$field = bounded_add($name, stats.$field, $value, limits.$limit)?;
        };
    }
    add!(
        total_source_terms,
        item.source_terms(),
        max_total_source_terms,
        "total source terms"
    );
    add!(
        total_source_exponent_entries,
        item.source_exponent_entries(),
        max_total_source_exponent_entries,
        "total source exponent entries"
    );
    add!(
        total_expanded_contributions,
        item.expanded_contribution_bound(),
        max_total_expanded_contributions,
        "total expanded contributions"
    );
    add!(
        total_output_term_bound,
        item.expanded_contribution_bound(),
        max_total_output_term_bound,
        "total output term bound"
    );
    add!(
        total_output_terms,
        item.output_terms(),
        max_total_output_terms,
        "total output terms"
    );
    add!(
        total_output_exponent_entry_bound,
        item.output_exponent_entry_bound(),
        max_total_output_exponent_entry_bound,
        "total output exponent entry bound"
    );
    add!(
        total_output_exponent_entries,
        item.output_exponent_entries(),
        max_total_output_exponent_entries,
        "total output exponent entries"
    );
    add!(
        total_power_calls,
        item.power_calls(),
        max_total_power_calls,
        "total power calls"
    );
    add!(
        total_native_power_heap_pairs,
        item.native_power_heap_pair_bound(),
        max_total_native_power_heap_pairs,
        "total native power heap pairs"
    );
    add!(
        total_multiplication_term_pairs,
        item.multiplication_term_pair_bound(),
        max_total_multiplication_term_pairs,
        "total multiplication term pairs"
    );
    add!(
        total_addition_term_visits,
        item.addition_term_visit_bound(),
        max_total_addition_term_visits,
        "total addition term visits"
    );
    stats.largest_kronecker_exponent_bits = stats
        .largest_kronecker_exponent_bits
        .max(item.largest_kronecker_exponent_bits());
    stats.largest_integer_coefficient_bits = stats
        .largest_integer_coefficient_bits
        .max(item.largest_integer_coefficient_bit_bound());
    add!(
        total_native_integer_bit_work,
        item.native_integer_bit_work_bound(),
        max_total_native_integer_bit_work,
        "total native integer bit work"
    );
    add!(
        total_integer_bit_work,
        item.integer_bit_work_bound(),
        max_total_integer_bit_work,
        "total integer bit work"
    );
    Ok(())
}

fn consume_coefficient_preflight(
    stats: &mut GeneratedAffineResidualCaseBoundRelationStats,
    item: ResidualAffineCoefficientCompositionPreflight,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
) -> Result<(), GeneratedAffineResidualCaseBoundRelationError> {
    if !coefficient_aggregate_matches(item.numerator(), item.denominator(), item.aggregate())? {
        return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
    }
    consume_polynomial_stats(stats, item.aggregate(), limits)?;
    stats.total_integer_bit_work = bounded_add(
        "total integer bit work",
        stats.total_integer_bit_work,
        item.durable_denominator_integer_bit_payload_bound(),
        limits.max_total_integer_bit_work,
    )?;
    if item.total_integer_bit_work_bound()
        != checked_add(
            "coefficient integer bit work",
            item.aggregate().integer_bit_work_bound(),
            item.durable_denominator_integer_bit_payload_bound(),
        )?
    {
        return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
    }
    stats.total_normalization_input_term_pairs = bounded_add(
        "total normalization input term pairs",
        stats.total_normalization_input_term_pairs,
        item.normalization_input_term_pair_bound(),
        limits.max_total_normalization_input_term_pairs,
    )?;
    stats.total_durable_denominator_terms = bounded_add(
        "total durable denominator terms",
        stats.total_durable_denominator_terms,
        item.durable_denominator_term_bound(),
        limits.max_total_durable_denominator_terms,
    )?;
    stats.total_durable_denominator_exponent_entries = bounded_add(
        "total durable denominator exponent entries",
        stats.total_durable_denominator_exponent_entries,
        item.durable_denominator_exponent_entry_bound(),
        limits.max_total_durable_denominator_exponent_entries,
    )?;
    stats.total_durable_denominator_integer_bits = bounded_add(
        "total durable denominator integer bits",
        stats.total_durable_denominator_integer_bits,
        item.durable_denominator_integer_bit_payload_bound(),
        limits.max_total_durable_denominator_integer_bits,
    )?;
    Ok(())
}

fn consume_coefficient_stats(
    stats: &mut GeneratedAffineResidualCaseBoundRelationStats,
    item: ResidualUnitAffineCoefficientCompositionStats,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
) -> Result<(), GeneratedAffineResidualCaseBoundRelationError> {
    if !coefficient_aggregate_matches(item.numerator(), item.denominator(), item.aggregate())? {
        return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
    }
    consume_polynomial_stats(stats, item.aggregate(), limits)?;
    stats.total_integer_bit_work = bounded_add(
        "total integer bit work",
        stats.total_integer_bit_work,
        item.durable_denominator_integer_bit_payload(),
        limits.max_total_integer_bit_work,
    )?;
    stats.total_normalization_input_term_pairs = bounded_add(
        "total normalization input term pairs",
        stats.total_normalization_input_term_pairs,
        item.normalization_input_term_pairs(),
        limits.max_total_normalization_input_term_pairs,
    )?;
    stats.total_durable_denominator_terms = bounded_add(
        "total durable denominator terms",
        stats.total_durable_denominator_terms,
        item.durable_denominator_terms(),
        limits.max_total_durable_denominator_terms,
    )?;
    stats.total_durable_denominator_exponent_entries = bounded_add(
        "total durable denominator exponent entries",
        stats.total_durable_denominator_exponent_entries,
        item.durable_denominator_exponent_entries(),
        limits.max_total_durable_denominator_exponent_entries,
    )?;
    stats.total_durable_denominator_integer_bits = bounded_add(
        "total durable denominator integer bits",
        stats.total_durable_denominator_integer_bits,
        item.durable_denominator_integer_bit_payload(),
        limits.max_total_durable_denominator_integer_bits,
    )?;
    Ok(())
}

fn coefficient_aggregate_matches(
    numerator: ResidualUnitAffinePolynomialCompositionStats,
    denominator: ResidualUnitAffinePolynomialCompositionStats,
    aggregate: ResidualUnitAffinePolynomialCompositionStats,
) -> Result<bool, GeneratedAffineResidualCaseBoundRelationError> {
    let add = |left: usize, right: usize| checked_add("coefficient aggregate", left, right);
    Ok(
        aggregate.source_terms() == add(numerator.source_terms(), denominator.source_terms())?
            && aggregate.source_exponent_entries()
                == add(
                    numerator.source_exponent_entries(),
                    denominator.source_exponent_entries(),
                )?
            && aggregate.expanded_contribution_bound()
                == add(
                    numerator.expanded_contribution_bound(),
                    denominator.expanded_contribution_bound(),
                )?
            && aggregate.output_terms()
                == add(numerator.output_terms(), denominator.output_terms())?
            && aggregate.output_exponent_entry_bound()
                == add(
                    numerator.output_exponent_entry_bound(),
                    denominator.output_exponent_entry_bound(),
                )?
            && aggregate.output_exponent_entries()
                == add(
                    numerator.output_exponent_entries(),
                    denominator.output_exponent_entries(),
                )?
            && aggregate.power_calls() == add(numerator.power_calls(), denominator.power_calls())?
            && aggregate.native_power_heap_pair_bound()
                == add(
                    numerator.native_power_heap_pair_bound(),
                    denominator.native_power_heap_pair_bound(),
                )?
            && aggregate.multiplication_term_pair_bound()
                == add(
                    numerator.multiplication_term_pair_bound(),
                    denominator.multiplication_term_pair_bound(),
                )?
            && aggregate.addition_term_visit_bound()
                == add(
                    numerator.addition_term_visit_bound(),
                    denominator.addition_term_visit_bound(),
                )?
            && aggregate.largest_kronecker_exponent_bits()
                == numerator
                    .largest_kronecker_exponent_bits()
                    .max(denominator.largest_kronecker_exponent_bits())
            && aggregate.largest_integer_coefficient_bit_bound()
                == numerator
                    .largest_integer_coefficient_bit_bound()
                    .max(denominator.largest_integer_coefficient_bit_bound())
            && aggregate.native_integer_bit_work_bound()
                == add(
                    numerator.native_integer_bit_work_bound(),
                    denominator.native_integer_bit_work_bound(),
                )?
            && aggregate.integer_bit_work_bound()
                == add(
                    numerator.integer_bit_work_bound(),
                    denominator.integer_bit_work_bound(),
                )?,
    )
}

fn pre_translation_peak_envelope(
    stats: &GeneratedAffineResidualCaseBoundRelationStats,
    target_row_label_bytes: usize,
    plan: ResidualAffineCompactCompositionPlanStats,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    checked_sum(
        "peak scratch bytes",
        [
            source_binding_owned_envelope(target_row_label_bytes, plan)?,
            stats.translation_retained_output_bytes,
            size_of::<ParametricRelation>(),
        ],
    )
}

fn pre_token_allocation_peak_envelope(
    translated_retained_bytes: usize,
    target_row_label_bytes: usize,
    plan: ResidualAffineCompactCompositionPlanStats,
    prepared_token_byte_envelope: usize,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    checked_sum(
        "peak scratch bytes",
        [
            source_binding_owned_envelope(target_row_label_bytes, plan)?,
            translated_retained_bytes,
            prepared_token_byte_envelope,
        ],
    )
}

fn prospective_retained_term_envelope(
    stats: &GeneratedAffineResidualCaseBoundRelationStats,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    let condition_occurrences = checked_add(
        "retained terms",
        stats.translated_guards,
        stats.translated_terms,
    )?;
    checked_add(
        "retained terms",
        checked_mul("retained terms", stats.preflight_total_output_term_bound, 3)?,
        condition_occurrences,
    )
}

fn prospective_retained_byte_envelope(
    stats: &GeneratedAffineResidualCaseBoundRelationStats,
    target_row_label_bytes: usize,
    plan: ResidualAffineCompactCompositionPlanStats,
    manifest_bytes: usize,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    let condition_occurrences = checked_add(
        "retained bytes",
        stats.translated_guards,
        stats.translated_terms,
    )?;
    let btree_node = checked_add(
        "retained bytes",
        checked_mul(
            "retained bytes",
            size_of::<(IndexShift, crate::ParametricCoefficient)>(),
            16,
        )?,
        checked_mul("retained bytes", size_of::<usize>(), 32)?,
    )?;
    let term_container_bytes = checked_mul(
        "retained bytes",
        stats.translated_terms,
        checked_add(
            "retained bytes",
            btree_node,
            checked_mul(
                "retained bytes",
                stats.translation_components,
                size_of::<i64>(),
            )?,
        )?,
    )?;
    let integer_payload_per_term = checked_add(
        "retained bytes",
        checked_add(
            "retained bytes",
            stats.preflight_largest_integer_coefficient_bits,
            7,
        )? / 8,
        size_of::<usize>(),
    )?;
    let polynomial_payload = checked_sum(
        "retained bytes",
        [
            checked_mul(
                "retained bytes",
                stats.preflight_total_output_term_bound,
                size_of::<Integer>(),
            )?,
            checked_mul(
                "retained bytes",
                stats.preflight_total_output_exponent_entry_bound,
                size_of::<u16>(),
            )?,
            checked_mul(
                "retained bytes",
                stats.preflight_total_output_term_bound,
                integer_payload_per_term,
            )?,
        ],
    )?;
    // One mapped polynomial can be retained in a coefficient, in a guarded
    // condition, and in the relation's compatibility polynomial vector.
    let tripled_polynomial_payload = checked_mul("retained bytes", polynomial_payload, 3)?;
    let condition_slots = checked_mul(
        "retained bytes",
        condition_occurrences,
        checked_sum(
            "retained bytes",
            [
                size_of::<ParametricNonZeroCondition>(),
                size_of::<ParametricPolynomial>(),
                size_of::<GeneratedAffineResidualCaseBoundBaseAssumption>(),
                size_of::<GeneratedAffineResidualCaseBoundConditionWitness>(),
                // One sealed origin plus a possible relation-attachment origin.
                checked_mul("retained bytes", size_of::<GuardOrigin>(), 2)?,
            ],
        )?,
    )?;
    checked_sum(
        "retained bytes",
        [
            size_of::<GeneratedAffineResidualCaseBoundParametricRelation>(),
            source_binding_owned_envelope(target_row_label_bytes, plan)?,
            size_of::<ParametricRelation>(),
            term_container_bytes,
            tripled_polynomial_payload,
            condition_slots,
            arc_string_owned_envelope(manifest_bytes)?,
        ],
    )
}

fn source_binding_owned_envelope(
    target_row_label_bytes: usize,
    plan: ResidualAffineCompactCompositionPlanStats,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    let outer_plan_arc_overhead =
        arc_payload_control_and_padding_byte_bound::<ResidualAffineCompactCompositionPlan>()?
            .checked_sub(size_of::<ResidualAffineCompactCompositionPlan>())
            .ok_or(
                GeneratedAffineResidualCaseBoundRelationError::ResourceCountOverflow {
                    resource: "compact plan Arc overhead",
                },
            )?;
    checked_sum(
        "retained bytes",
        [
            size_of::<GeneratedAffineResidualCaseBoundSource>(),
            arc_str_owned_envelope(target_row_label_bytes)?,
            outer_plan_arc_overhead,
            plan.retained_owned_logical_bytes(),
        ],
    )
}

fn composition_peak_envelope(
    stats: &GeneratedAffineResidualCaseBoundRelationStats,
    translated_retained_bytes: usize,
    plan: ResidualAffineCompactCompositionPlanStats,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    let contribution_slots = checked_sum(
        "composition scratch bytes",
        [
            checked_mul(
                "composition scratch bytes",
                stats.preflight_total_expanded_contributions,
                size_of::<Integer>(),
            )?,
            checked_mul(
                "composition scratch bytes",
                stats.preflight_total_output_exponent_entry_bound,
                size_of::<u16>(),
            )?,
            checked_mul(
                "composition scratch bytes",
                stats.preflight_total_native_power_heap_pairs,
                checked_mul("composition scratch bytes", size_of::<usize>(), 2)?,
            )?,
            checked_mul(
                "composition scratch bytes",
                stats.preflight_total_multiplication_term_pairs,
                checked_mul("composition scratch bytes", size_of::<usize>(), 2)?,
            )?,
            checked_add(
                "composition scratch bytes",
                stats.preflight_total_integer_bit_work,
                7,
            )? / 8,
        ],
    )?;
    checked_sum(
        "peak scratch bytes",
        [
            translated_retained_bytes,
            plan.retained_owned_logical_bytes(),
            stats.retained_byte_envelope,
            stats.prepared_composition_token_byte_envelope,
            contribution_slots,
        ],
    )
}

fn execute_complete_row(
    context: &ParametricCoefficientContext,
    translated: &ParametricRelation,
    source: GeneratedAffineResidualCaseBoundSource,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
    mut stats: GeneratedAffineResidualCaseBoundRelationStats,
    prepared: CompleteRowCompositionPreflight<'_>,
) -> Result<
    GeneratedAffineResidualCaseBoundRelationCompilation,
    GeneratedAffineResidualCaseBoundRelationError,
> {
    let premises = Arc::clone(&source.premises);
    let CompleteRowCompositionPreflight {
        stats: prospective,
        guards: prepared_guards,
        coefficients: prepared_coefficients,
    } = prepared;
    let condition_upper_bound = checked_add(
        "condition witnesses",
        translated.guarded_nonzero_conditions().len(),
        translated.terms().len(),
    )?;
    check_limit(
        "condition classifications",
        condition_upper_bound,
        limits.max_condition_classifications,
    )?;
    check_limit(
        "condition witnesses",
        condition_upper_bound,
        limits.max_condition_witnesses,
    )?;
    stats.condition_classification_admission_demand = condition_upper_bound;
    stats.condition_witness_admission_demand = condition_upper_bound;
    let premise_comparison_upper_bound = checked_mul(
        "inherited premise comparisons",
        condition_upper_bound,
        premises.premises().len(),
    )?;
    check_limit(
        "inherited premise comparisons",
        premise_comparison_upper_bound,
        limits.max_inherited_premise_comparisons,
    )?;
    stats.inherited_premise_comparison_admission_demand = premise_comparison_upper_bound;
    let associate_comparison_upper_bound = checked_mul(
        "condition associate comparisons",
        condition_upper_bound,
        condition_upper_bound.saturating_sub(1),
    )? / 2;
    check_limit(
        "private guard associate comparisons",
        associate_comparison_upper_bound,
        limits.max_private_guard_associate_comparisons,
    )?;
    check_limit(
        "base assumption associate comparisons",
        associate_comparison_upper_bound,
        limits.max_base_assumption_associate_comparisons,
    )?;
    stats.private_guard_associate_comparison_admission_demand = associate_comparison_upper_bound;
    stats.base_assumption_associate_comparison_admission_demand = associate_comparison_upper_bound;
    if condition_upper_bound != 0 {
        check_limit(
            "parametric guard origins",
            2,
            limits.polynomial_composition.max_guard_origins,
        )?;
    }
    let mut relation = ParametricRelation::new(
        source.authority.family_fingerprint(),
        source.target_row_id.clone(),
        context,
    );
    let mut base_assumptions = Vec::new();
    base_assumptions
        .try_reserve_exact(condition_upper_bound)
        .map_err(
            |_| GeneratedAffineResidualCaseBoundRelationError::AllocationFailure {
                resource: "row-local base assumptions",
            },
        )?;
    let mut witnesses = Vec::new();
    witnesses
        .try_reserve_exact(condition_upper_bound)
        .map_err(
            |_| GeneratedAffineResidualCaseBoundRelationError::AllocationFailure {
                resource: "condition witnesses",
            },
        )?;

    if translated.guarded_nonzero_conditions().len() != prepared_guards.len() {
        return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
    }
    for (guard_ordinal, prepared_guard) in prepared_guards.into_iter().enumerate() {
        let expected = prepared_guard.stats();
        let mapped = prepared_guard
            .execute()
            .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::Composition)?;
        let (polynomial, item_stats) = mapped.into_parts();
        if !execution_polynomial_fits_preflight(item_stats, expected) {
            return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
        }
        stats.guard_compositions = checked_add("guard compositions", stats.guard_compositions, 1)?;
        consume_polynomial_stats(&mut stats, item_stats, limits)?;
        if polynomial.is_zero() {
            drop(relation);
            drop(base_assumptions);
            drop(witnesses);
            return unavailable(
                source,
                GeneratedAffineResidualCaseBoundUnavailableReason::TranslatedSourceGuardComposesToZero {
                    guard_ordinal,
                },
                limits,
                stats,
            );
        }
        let class = classify_and_retain_condition(
            context,
            polynomial,
            &mut relation,
            &mut base_assumptions,
            premises.as_ref(),
            limits,
            &mut stats,
        )?;
        push_witness(
            &mut witnesses,
            GeneratedAffineResidualCaseBoundConditionSource::TranslatedSourceGuard {
                guard_ordinal,
            },
            class,
        )?;
    }

    if translated.terms().len() != prepared_coefficients.len() {
        return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
    }
    for (term_ordinal, ((shift, _), prepared_coefficient)) in translated
        .terms()
        .iter()
        .zip(prepared_coefficients)
        .enumerate()
    {
        let expected = prepared_coefficient.stats();
        let mapped = prepared_coefficient
            .execute()
            .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::Composition)?;
        let item_stats = mapped.stats();
        if !execution_coefficient_fits_preflight(item_stats, expected)? {
            return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
        }
        stats.coefficient_compositions = checked_add(
            "coefficient compositions",
            stats.coefficient_compositions,
            1,
        )?;
        stats.numerator_compositions =
            checked_add("numerator compositions", stats.numerator_compositions, 1)?;
        stats.denominator_compositions = checked_add(
            "denominator compositions",
            stats.denominator_compositions,
            1,
        )?;
        consume_coefficient_stats(&mut stats, item_stats, limits)?;
        let ResidualAffineCoefficientComposition::Available(mapped) = mapped else {
            drop(relation);
            drop(base_assumptions);
            drop(witnesses);
            return unavailable(
                source,
                GeneratedAffineResidualCaseBoundUnavailableReason::TranslatedSourceTermDenominatorComposesToZero {
                    term_ordinal,
                },
                limits,
                stats,
            );
        };
        let (value, mapped_denominator, returned_stats) = mapped.into_parts();
        if returned_stats != item_stats {
            return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
        }
        // Classify the exact pre-normalization denominator before inspecting
        // the normalized numerator.  This preserves the domain of 0/p and of a
        // denominator later cancelled by Symbolica's fraction normalizer.
        let class = classify_and_retain_condition(
            context,
            mapped_denominator,
            &mut relation,
            &mut base_assumptions,
            premises.as_ref(),
            limits,
            &mut stats,
        )?;
        push_witness(
            &mut witnesses,
            GeneratedAffineResidualCaseBoundConditionSource::TranslatedSourceTermDenominator {
                term_ordinal,
            },
            class,
        )?;
        if !value.is_zero() {
            relation
                .insert_prevalidated_distinct_term_without_denominator_discovery(
                    context,
                    shift.clone(),
                    value,
                    relation_arithmetic_limits(limits),
                )
                .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::RelationConstruction)?;
        }
    }

    stats.condition_witnesses = witnesses.len();
    stats.row_local_base_assumptions = base_assumptions.len();
    stats.private_free_index_guards = relation.guarded_nonzero_conditions().len();
    if stats.guard_compositions != prospective.guard_composition_preflights
        || stats.coefficient_compositions != prospective.coefficient_composition_preflights
        || stats.numerator_compositions != prospective.numerator_composition_preflights
        || stats.denominator_compositions != prospective.denominator_composition_preflights
        || !execution_aggregate_fits_preflight(&stats, &prospective)
    {
        return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
    }
    finish_retained(source, relation, base_assumptions, witnesses, limits, stats)
}

fn classify_and_retain_condition(
    context: &ParametricCoefficientContext,
    polynomial: ParametricPolynomial,
    relation: &mut ParametricRelation,
    assumptions: &mut Vec<GeneratedAffineResidualCaseBoundBaseAssumption>,
    premises: &GeneratedAffineResidualCasePremisesCertificate,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
    stats: &mut GeneratedAffineResidualCaseBoundRelationStats,
) -> Result<
    GeneratedAffineResidualCaseBoundConditionClass,
    GeneratedAffineResidualCaseBoundRelationError,
> {
    stats.condition_classifications = bounded_add(
        "condition classifications",
        stats.condition_classifications,
        1,
        limits.max_condition_classifications,
    )?;
    if polynomial.is_zero() {
        return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
    }
    if polynomial.is_nonzero_constant() {
        return Ok(
            GeneratedAffineResidualCaseBoundConditionClass::DischargedNonzeroIntegerConstant,
        );
    }
    // Dependency is semantic for predicate-locus association.  Base-only
    // assumptions may differ only by a unit of Q, whereas an index-dependent
    // predicate may differ by a unit of Q(theta).  Compute this before inherited
    // matching so an unrelated base premise cannot absorb another physical-
    // parameter locus through the broader coefficient-field relation.
    let depends_on_indices = condition_depends_on_indices(context, &polynomial, limits)?;
    for (ordinal, inherited) in premises.premises().iter().enumerate() {
        stats.inherited_premise_comparisons = bounded_add(
            "inherited premise comparisons",
            stats.inherited_premise_comparisons,
            1,
            limits.max_inherited_premise_comparisons,
        )?;
        // Literal equality remains the allocation-free fast path.  For
        // distinct payloads, authenticate the inherited dependency class before
        // choosing the admissible unit group; a class mismatch is simply a
        // nonmatch and never reaches either associate proof.
        let matches = if &polynomial == inherited.polynomial() {
            true
        } else {
            let inherited_depends_on_indices =
                condition_depends_on_indices(context, inherited.polynomial(), limits)?;
            inherited_depends_on_indices == depends_on_indices
                && distinct_condition_loci_are_associates(
                    context,
                    &polynomial,
                    inherited.polynomial(),
                    depends_on_indices,
                    limits,
                )?
        };
        if matches {
            stats.inherited_premise_matches = bounded_add(
                "inherited premise matches",
                stats.inherited_premise_matches,
                1,
                limits.max_inherited_premise_matches,
            )?;
            return Ok(
                GeneratedAffineResidualCaseBoundConditionClass::InheritedPremise { ordinal },
            );
        }
    }
    let condition = context
        .nonzero_condition_with_origins_and_origin_limit(
            polynomial,
            [GuardOrigin::GeneratedAffineSealedCondition],
            limits.polynomial_composition.exact_algebra,
            limits.polynomial_composition.max_guard_origins,
        )
        .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::ConditionMaterialization)?;
    if depends_on_indices {
        for (ordinal, existing) in relation.guarded_nonzero_conditions().iter().enumerate() {
            stats.private_guard_associate_comparisons = bounded_add(
                "private guard associate comparisons",
                stats.private_guard_associate_comparisons,
                1,
                limits.max_private_guard_associate_comparisons,
            )?;
            if existing.polynomial() == condition.polynomial()
                || distinct_condition_loci_are_associates(
                    context,
                    existing.polynomial(),
                    condition.polynomial(),
                    true,
                    limits,
                )?
            {
                return Ok(
                    GeneratedAffineResidualCaseBoundConditionClass::PrivateFreeIndexGuard {
                        ordinal,
                    },
                );
            }
        }
        let ordinal = relation.guarded_nonzero_conditions().len();
        check_limit(
            "private free-index guards",
            checked_add("private free-index guards", ordinal, 1)?,
            limits.max_private_free_index_guards,
        )?;
        relation
            .add_guarded_nonzero_condition_with_limits(
                context,
                condition,
                relation_arithmetic_limits(limits),
            )
            .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::RelationConstruction)?;
        stats.private_free_index_guards = relation.guarded_nonzero_conditions().len();
        Ok(GeneratedAffineResidualCaseBoundConditionClass::PrivateFreeIndexGuard { ordinal })
    } else {
        for (ordinal, existing) in assumptions.iter().enumerate() {
            stats.base_assumption_associate_comparisons = bounded_add(
                "base assumption associate comparisons",
                stats.base_assumption_associate_comparisons,
                1,
                limits.max_base_assumption_associate_comparisons,
            )?;
            if existing.condition.polynomial() == condition.polynomial()
                || distinct_condition_loci_are_associates(
                    context,
                    existing.condition.polynomial(),
                    condition.polynomial(),
                    false,
                    limits,
                )?
            {
                return Ok(
                    GeneratedAffineResidualCaseBoundConditionClass::RowLocalBaseAssumption {
                        ordinal,
                    },
                );
            }
        }
        let ordinal = assumptions.len();
        check_limit(
            "row-local base assumptions",
            checked_add("row-local base assumptions", ordinal, 1)?,
            limits.max_row_local_base_assumptions,
        )?;
        if assumptions.len() == assumptions.capacity() {
            return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
        }
        assumptions.push(GeneratedAffineResidualCaseBoundBaseAssumption { condition });
        stats.row_local_base_assumptions = assumptions.len();
        Ok(GeneratedAffineResidualCaseBoundConditionClass::RowLocalBaseAssumption { ordinal })
    }
}

fn condition_depends_on_indices(
    context: &ParametricCoefficientContext,
    polynomial: &ParametricPolynomial,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
) -> Result<bool, GeneratedAffineResidualCaseBoundRelationError> {
    context
        .polynomial_depends_on_indices_with_limits(
            polynomial,
            limits.polynomial_composition.exact_algebra,
        )
        .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::ConditionMaterialization)
}

/// Compare two already-distinct predicates in one authenticated dependency
/// class. Index-dependent loci live projectively over `Q(theta)*`; base-only
/// physical-parameter loci use only rational units `Q*`.
fn distinct_condition_loci_are_associates(
    context: &ParametricCoefficientContext,
    left: &ParametricPolynomial,
    right: &ParametricPolynomial,
    index_dependent: bool,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
) -> Result<bool, GeneratedAffineResidualCaseBoundRelationError> {
    if index_dependent {
        context
            .polynomial_loci_are_associates_with_limits(
                left,
                right,
                limits.polynomial_composition.exact_algebra,
            )
            .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::ConditionMaterialization)
    } else {
        let mut child = limits.base_polynomial_associate;
        child.exact_algebra.max_exponent = child
            .exact_algebra
            .max_exponent
            .min(limits.polynomial_composition.exact_algebra.max_exponent);
        child.exact_algebra.max_polynomial_terms = child.exact_algebra.max_polynomial_terms.min(
            limits
                .polynomial_composition
                .exact_algebra
                .max_polynomial_terms,
        );
        child.exact_algebra.max_term_operations = child.exact_algebra.max_term_operations.min(
            limits
                .polynomial_composition
                .exact_algebra
                .max_term_operations,
        );
        context
            .base_polynomial_loci_are_rational_associates_with_census(left, right, child)
            .map(|result| result.associated())
            .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::ConditionMaterialization)
    }
}

fn push_witness(
    witnesses: &mut Vec<GeneratedAffineResidualCaseBoundConditionWitness>,
    source: GeneratedAffineResidualCaseBoundConditionSource,
    class: GeneratedAffineResidualCaseBoundConditionClass,
) -> Result<(), GeneratedAffineResidualCaseBoundRelationError> {
    if witnesses.len() == witnesses.capacity() {
        return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
    }
    witnesses.push(GeneratedAffineResidualCaseBoundConditionWitness { source, class });
    Ok(())
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
) -> Result<bool, GeneratedAffineResidualCaseBoundRelationError> {
    Ok(
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
            && actual.total_integer_bit_work_bound() <= prospective.total_integer_bit_work_bound(),
    )
}

fn execution_aggregate_fits_preflight(
    actual: &GeneratedAffineResidualCaseBoundRelationStats,
    prospective: &GeneratedAffineResidualCaseBoundRelationStats,
) -> bool {
    actual.total_source_terms == prospective.total_source_terms
        && actual.total_source_exponent_entries == prospective.total_source_exponent_entries
        && actual.total_expanded_contributions == prospective.total_expanded_contributions
        && actual.total_output_term_bound == prospective.total_output_term_bound
        && actual.total_output_exponent_entry_bound == prospective.total_output_exponent_entry_bound
        && actual.total_power_calls == prospective.total_power_calls
        && actual.total_native_power_heap_pairs == prospective.total_native_power_heap_pairs
        && actual.total_multiplication_term_pairs == prospective.total_multiplication_term_pairs
        && actual.total_addition_term_visits == prospective.total_addition_term_visits
        && actual.largest_kronecker_exponent_bits == prospective.largest_kronecker_exponent_bits
        && actual.largest_integer_coefficient_bits == prospective.largest_integer_coefficient_bits
        && actual.total_native_integer_bit_work == prospective.total_native_integer_bit_work
        && actual.total_output_terms <= prospective.total_output_term_bound
        && actual.total_output_exponent_entries <= prospective.total_output_exponent_entry_bound
        && actual.total_integer_bit_work <= prospective.total_integer_bit_work
        && actual.total_normalization_input_term_pairs
            <= prospective.total_normalization_input_term_pairs
        && actual.total_durable_denominator_terms <= prospective.total_durable_denominator_terms
        && actual.total_durable_denominator_exponent_entries
            <= prospective.total_durable_denominator_exponent_entries
        && actual.total_durable_denominator_integer_bits
            <= prospective.total_durable_denominator_integer_bits
}

fn relation_arithmetic_limits(
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
) -> ParametricArithmeticLimits {
    ParametricArithmeticLimits {
        exact_algebra: limits.polynomial_composition.exact_algebra,
        max_source_terms: limits.polynomial_composition.max_source_terms,
        max_output_terms: limits.polynomial_composition.max_output_terms,
        max_specialization_power_operations: limits.polynomial_composition.max_power_calls,
        max_specialization_integer_bits: limits.polynomial_composition.max_integer_bit_work,
        max_guard_origins: limits.polynomial_composition.max_guard_origins,
    }
}

fn unavailable(
    source: GeneratedAffineResidualCaseBoundSource,
    reason: GeneratedAffineResidualCaseBoundUnavailableReason,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
    mut stats: GeneratedAffineResidualCaseBoundRelationStats,
) -> Result<
    GeneratedAffineResidualCaseBoundRelationCompilation,
    GeneratedAffineResidualCaseBoundRelationError,
> {
    stats.relation_manifest_bytes = 0;
    stats.retained_terms = 0;
    stats.retained_bytes = checked_add(
        "retained bytes",
        size_of::<GeneratedAffineResidualCaseBoundUnavailableCertificate>(),
        source_binding_owned_envelope(
            stats.target_row_label_bytes,
            source.composition_plan.stats(),
        )?,
    )?;
    check_limit(
        "retained bytes",
        stats.retained_bytes,
        limits.max_retained_bytes,
    )?;
    if stats.retained_bytes > stats.retained_byte_envelope {
        return Err(GeneratedAffineResidualCaseBoundRelationError::RetainedByteEnvelopeExceeded);
    }
    Ok(
        GeneratedAffineResidualCaseBoundRelationCompilation::Unavailable(
            GeneratedAffineResidualCaseBoundUnavailableCertificate {
                schema: GENERATED_AFFINE_RESIDUAL_CASE_BOUND_RELATION_V2_SCHEMA,
                source,
                reason,
                limits,
                stats,
            },
        ),
    )
}

fn finish_retained(
    source: GeneratedAffineResidualCaseBoundSource,
    relation: ParametricRelation,
    base_assumptions: Vec<GeneratedAffineResidualCaseBoundBaseAssumption>,
    witnesses: Vec<GeneratedAffineResidualCaseBoundConditionWitness>,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
    mut stats: GeneratedAffineResidualCaseBoundRelationStats,
) -> Result<
    GeneratedAffineResidualCaseBoundRelationCompilation,
    GeneratedAffineResidualCaseBoundRelationError,
> {
    if base_assumptions.iter().any(|assumption| {
        assumption.condition.origins().len() != 1
            || !assumption
                .condition
                .origins()
                .contains(&GuardOrigin::GeneratedAffineSealedCondition)
    }) {
        return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
    }
    let observed_terms = observed_retained_terms(&relation, &base_assumptions, &witnesses)?;
    if observed_terms > stats.retained_term_envelope {
        return Err(GeneratedAffineResidualCaseBoundRelationError::RetainedByteEnvelopeExceeded);
    }
    stats.retained_terms = observed_terms;
    let manifest_bytes = relation
        .stable_manifest_byte_len_with_limit(limits.max_relation_manifest_bytes)
        .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::RelationConstruction)?;
    stats.relation_manifest_bytes = manifest_bytes;
    let prospective_retained = retained_logical_bytes(
        &source,
        &relation,
        manifest_bytes,
        &base_assumptions,
        &witnesses,
    )?;
    check_limit(
        "retained bytes",
        prospective_retained,
        limits.max_retained_bytes,
    )?;
    if prospective_retained > stats.retained_byte_envelope {
        return Err(GeneratedAffineResidualCaseBoundRelationError::RetainedByteEnvelopeExceeded);
    }
    // Manifest allocation is last and occurs only after its exact byte length
    // and the complete retained certificate census have passed.
    let manifest = relation
        .stable_manifest_with_limit(limits.max_relation_manifest_bytes)
        .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::RelationConstruction)?;
    if manifest.len() != manifest_bytes {
        return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
    }
    stats.retained_bytes = retained_logical_bytes(
        &source,
        &relation,
        manifest.capacity(),
        &base_assumptions,
        &witnesses,
    )?;
    check_limit(
        "retained bytes",
        stats.retained_bytes,
        limits.max_retained_bytes,
    )?;
    if stats.retained_bytes > stats.retained_byte_envelope {
        return Err(GeneratedAffineResidualCaseBoundRelationError::RetainedByteEnvelopeExceeded);
    }
    Ok(
        GeneratedAffineResidualCaseBoundRelationCompilation::Retained(
            GeneratedAffineResidualCaseBoundParametricRelation {
                schema: GENERATED_AFFINE_RESIDUAL_CASE_BOUND_RELATION_V2_SCHEMA,
                source,
                relation: Arc::new(relation),
                relation_manifest: Arc::new(manifest),
                base_assumptions,
                condition_witnesses: witnesses,
                limits,
                stats,
            },
        ),
    )
}

fn observed_retained_terms(
    relation: &ParametricRelation,
    assumptions: &[GeneratedAffineResidualCaseBoundBaseAssumption],
    witnesses: &[GeneratedAffineResidualCaseBoundConditionWitness],
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    let mut total = witnesses.len();
    for coefficient in relation.terms().values() {
        total = checked_add(
            "retained terms",
            total,
            coefficient.raw().numerator.nterms(),
        )?;
        total = checked_add(
            "retained terms",
            total,
            coefficient.raw().denominator.nterms(),
        )?;
    }
    for condition in relation.guarded_nonzero_conditions() {
        total = checked_add(
            "retained terms",
            total,
            checked_mul("retained terms", condition.polynomial().term_count(), 2)?,
        )?;
    }
    for assumption in assumptions {
        total = checked_add(
            "retained terms",
            total,
            assumption.condition.polynomial().term_count(),
        )?;
    }
    Ok(total)
}

fn retained_logical_bytes(
    source: &GeneratedAffineResidualCaseBoundSource,
    relation: &ParametricRelation,
    manifest_capacity: usize,
    assumptions: &Vec<GeneratedAffineResidualCaseBoundBaseAssumption>,
    witnesses: &Vec<GeneratedAffineResidualCaseBoundConditionWitness>,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    let relation_arc_overhead = arc_payload_control_and_padding_byte_bound::<ParametricRelation>()?
        .checked_sub(size_of::<ParametricRelation>())
        .ok_or(
            GeneratedAffineResidualCaseBoundRelationError::ResourceCountOverflow {
                resource: "relation Arc overhead",
            },
        )?;
    let mut bytes = checked_sum(
        "retained bytes",
        [
            size_of::<GeneratedAffineResidualCaseBoundParametricRelation>(),
            source_binding_owned_envelope(
                source_target_label_bytes(source)?,
                source.composition_plan.stats(),
            )?,
            relation_arc_overhead,
            relation.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualCaseBoundRelationError::ResourceCountOverflow {
                    resource: "relation retained bytes",
                },
            )?,
            arc_string_owned_envelope(manifest_capacity)?,
            checked_mul(
                "retained bytes",
                assumptions.capacity(),
                size_of::<GeneratedAffineResidualCaseBoundBaseAssumption>(),
            )?,
            checked_mul(
                "retained bytes",
                witnesses.capacity(),
                size_of::<GeneratedAffineResidualCaseBoundConditionWitness>(),
            )?,
        ],
    )?;
    for assumption in assumptions {
        let owned = assumption.condition.owned_retained_byte_bound().ok_or(
            GeneratedAffineResidualCaseBoundRelationError::ResourceCountOverflow {
                resource: "base assumption retained bytes",
            },
        )?;
        bytes = checked_add(
            "retained bytes",
            bytes,
            owned
                .checked_sub(size_of::<ParametricNonZeroCondition>())
                .ok_or(
                    GeneratedAffineResidualCaseBoundRelationError::ResourceCountOverflow {
                        resource: "base assumption retained bytes",
                    },
                )?,
        )?;
    }
    Ok(bytes)
}

fn source_target_label_bytes(
    source: &GeneratedAffineResidualCaseBoundSource,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    let ParametricRowId::Derived { label } = &source.target_row_id else {
        return Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch);
    };
    Ok(label.len())
}

#[allow(clippy::too_many_arguments)]
fn replay_expected(
    schema: &'static str,
    source: &GeneratedAffineResidualCaseBoundSource,
    limits: GeneratedAffineResidualCaseBoundRelationLimits,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
    schedule: &Arc<GeneratedAffinePreparePointScheduleCertificate>,
    premises: &Arc<GeneratedAffineResidualCasePremisesCertificate>,
    matches: impl FnOnce(GeneratedAffineResidualCaseBoundRelationCompilation) -> bool,
) -> Result<(), GeneratedAffineResidualCaseBoundRelationError> {
    catch_unwind(AssertUnwindSafe(|| {
        if schema != GENERATED_AFFINE_RESIDUAL_CASE_BOUND_RELATION_V2_SCHEMA {
            return Err(GeneratedAffineResidualCaseBoundRelationError::SchemaMismatch);
        }
        check_limit(
            "parent allocation comparisons",
            4,
            limits.max_parent_allocation_comparisons,
        )?;
        if !Arc::ptr_eq(&source.authority, authority)
            || !Arc::ptr_eq(&source.ordering, ordering)
            || !Arc::ptr_eq(&source.schedule, schedule)
            || !Arc::ptr_eq(&source.premises, premises)
        {
            return Err(GeneratedAffineResidualCaseBoundRelationError::WrongParentAllocation);
        }
        let handle = schedule
            .point_handle(source.point_depth, source.point_ordinal)
            .ok_or(GeneratedAffineResidualCaseBoundRelationError::WrongPointBinding)?;
        let compilation = GeneratedAffineResidualCaseBoundRelationCompiler::compile(
            family,
            context,
            Arc::clone(authority),
            Arc::clone(ordering),
            Arc::clone(schedule),
            Arc::clone(premises),
            source.source_row_ordinal,
            handle,
            limits,
        )?;
        if matches(compilation) {
            Ok(())
        } else {
            Err(GeneratedAffineResidualCaseBoundRelationError::ReplayMismatch)
        }
    }))
    .map_err(|_| GeneratedAffineResidualCaseBoundRelationError::SymbolicaPanic)?
}

fn source_payload_eq(
    left: &GeneratedAffineResidualCaseBoundSource,
    right: &GeneratedAffineResidualCaseBoundSource,
) -> bool {
    Arc::ptr_eq(&left.authority, &right.authority)
        && Arc::ptr_eq(&left.ordering, &right.ordering)
        && Arc::ptr_eq(&left.schedule, &right.schedule)
        && Arc::ptr_eq(&left.premises, &right.premises)
        && left.source_row_ordinal == right.source_row_ordinal
        && left.point_depth == right.point_depth
        && left.point_ordinal == right.point_ordinal
        && left.point_key == right.point_key
        && left.target_row_id == right.target_row_id
        && left.composition_plan.manifest() == right.composition_plan.manifest()
        && left.composition_plan.stats() == right.composition_plan.stats()
        && left.composition_plan.limits() == right.composition_plan.limits()
}

fn integer_magnitude_bits(
    value: &Integer,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| {
        GeneratedAffineResidualCaseBoundRelationError::ResourceCountOverflow {
            resource: "integer magnitude bits",
        }
    })
}

fn arc_payload_control_and_padding_byte_bound<T>()
-> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    checked_sum(
        "Arc allocation bytes",
        [
            checked_mul("Arc allocation bytes", size_of::<usize>(), 2)?,
            align_of::<T>().saturating_sub(1),
            size_of::<T>(),
        ],
    )
}

fn arc_string_owned_envelope(
    capacity: usize,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    checked_add(
        "Arc<String> retained bytes",
        arc_payload_control_and_padding_byte_bound::<String>()?,
        capacity,
    )
}

fn arc_str_owned_envelope(
    payload_bytes: usize,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    checked_sum(
        "Arc<str> retained bytes",
        [
            checked_mul("Arc<str> retained bytes", size_of::<usize>(), 2)?,
            align_of::<u8>().saturating_sub(1),
            payload_bytes,
        ],
    )
}

fn remaining(
    resource: &'static str,
    limit: usize,
    spent: usize,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    limit.checked_sub(spent).ok_or(
        GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
            resource,
            requested: spent,
            limit,
        },
    )
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualCaseBoundRelationError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineResidualCaseBoundRelationError::ResourceCountOverflow { resource })
}

fn checked_sum<const N: usize>(
    resource: &'static str,
    values: [usize; N],
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    values
        .into_iter()
        .try_fold(0usize, |total, value| checked_add(resource, total, value))
}

fn bounded_add(
    resource: &'static str,
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, GeneratedAffineResidualCaseBoundRelationError> {
    let requested = checked_add(resource, current, additional)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualCaseBoundRelationError> {
    if requested > limit {
        Err(
            GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

fn decimal_digits_usize(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

const fn portable_usize(value: u64) -> usize {
    if usize::BITS >= u64::BITS {
        value as usize
    } else if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Weak};
    use std::thread;

    use super::*;
    use crate::generated_affine_parametric_ordering::GeneratedAffineParametricOrderingLimits;
    use crate::generated_affine_prepare_point_schedule::GeneratedAffinePreparePointScheduleLimits;
    use crate::generated_affine_residual_boolean_cover::{
        GeneratedAffineResidualBooleanCoverCompiler, GeneratedAffineResidualBooleanCoverLimits,
    };
    use crate::generated_affine_residual_case_inventory::{
        GeneratedAffineResidualCaseAuthorityLimits,
        GeneratedAffineResidualCaseInventoryCertificate,
        GeneratedAffineResidualCaseInventoryCompiler, GeneratedAffineResidualCaseInventoryLimits,
    };
    use crate::generated_affine_residual_case_premises::{
        GeneratedAffineResidualCasePremisesLimits, GeneratedAffineResidualCasePremisesOutcome,
        compile_generated_affine_residual_case_premises,
    };
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
    use crate::{
        AffineDenominator, CoefficientContext, ExactAlgebraLimits,
        GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
        GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits,
        IntegralOrderingPolicy, ParametricIbpGenerator, SectorMask,
    };

    struct NaturalFixture {
        family: IntegralFamily,
        context: ParametricCoefficientContext,
        inventory: Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
        schedule: Arc<GeneratedAffinePreparePointScheduleCertificate>,
        premises: Arc<GeneratedAffineResidualCasePremisesCertificate>,
        source_row_ordinal: usize,
        point_depth: usize,
        point_ordinal: usize,
    }

    impl NaturalFixture {
        fn compile(
            &self,
            limits: GeneratedAffineResidualCaseBoundRelationLimits,
        ) -> Result<
            GeneratedAffineResidualCaseBoundRelationCompilation,
            GeneratedAffineResidualCaseBoundRelationError,
        > {
            let point = self
                .schedule
                .point_handle(self.point_depth, self.point_ordinal)
                .unwrap();
            GeneratedAffineResidualCaseBoundRelationCompiler::compile(
                &self.family,
                &self.context,
                Arc::clone(&self.authority),
                Arc::clone(&self.ordering),
                Arc::clone(&self.schedule),
                Arc::clone(&self.premises),
                self.source_row_ordinal,
                point,
                limits,
            )
        }
    }

    fn compilation_stats(
        compilation: &GeneratedAffineResidualCaseBoundRelationCompilation,
    ) -> GeneratedAffineResidualCaseBoundRelationStats {
        match compilation {
            GeneratedAffineResidualCaseBoundRelationCompilation::Retained(certificate) => {
                certificate.stats()
            }
            GeneratedAffineResidualCaseBoundRelationCompilation::Unavailable(certificate) => {
                certificate.stats()
            }
        }
    }

    fn limits_from_successful_stats(
        stats: GeneratedAffineResidualCaseBoundRelationStats,
    ) -> GeneratedAffineResidualCaseBoundRelationLimits {
        let mut limits = GeneratedAffineResidualCaseBoundRelationLimits::default();
        let source = stats.source_row();
        limits.source_row.max_scope_comparison_bytes = source.scope_comparison_bytes();
        limits.source_row.max_source_rows = source.source_rows();
        limits.source_row.max_relation_terms = source.relation_terms();
        limits.source_row.max_guard_conditions = source.guard_conditions();

        let point = stats.point_authentication();
        limits.point_authentication.max_schedule_replays = point.schedule_replays();
        limits.point_authentication.max_pointer_checks = point.pointer_checks();
        limits.point_authentication.max_index_checks = point.index_checks();

        let plan = stats.compact_plan();
        let composition = plan.composition();
        limits.compact_plan.composition.max_variables = composition.variables();
        limits.compact_plan.composition.max_full_images = composition.full_images();
        limits
            .compact_plan
            .composition
            .max_geometry_entries_inspected = composition.geometry_entries_inspected();
        limits
            .compact_plan
            .composition
            .max_geometry_entries_retained = composition.geometry_entries_retained();
        limits.compact_plan.composition.max_support_entries_retained =
            composition.support_entries_retained();
        limits.compact_plan.composition.max_total_image_terms = composition.total_image_terms();
        limits
            .compact_plan
            .composition
            .max_total_image_exponent_entries = composition.total_image_exponent_entries();
        limits.compact_plan.composition.max_image_integer_bits =
            composition.largest_image_integer_bits();
        limits.compact_plan.composition.max_total_image_integer_bits =
            composition.total_image_integer_bits();
        limits.compact_plan.max_context_fingerprint_bytes = plan.context_fingerprint_bytes();
        limits.compact_plan.max_geometry_integer_bit_work = plan.geometry_integer_bit_work();
        limits.compact_plan.max_geometry_replay_comparison_work =
            plan.geometry_replay_comparison_work();
        limits.compact_plan.max_geometry_replay_integer_bit_work =
            plan.geometry_replay_integer_bit_work();
        limits
            .compact_plan
            .max_geometry_replay_scratch_logical_bytes =
            plan.geometry_replay_scratch_logical_bytes();
        limits.compact_plan.max_retained_owned_logical_bytes = plan.retained_owned_logical_bytes();
        limits
            .compact_plan
            .max_compilation_owned_logical_peak_upper_bound =
            plan.compilation_owned_logical_peak_upper_bound();

        limits.max_scope_comparison_bytes = stats.scope_comparison_bytes();
        limits.max_parent_allocation_comparisons = stats.parent_allocation_comparisons();
        limits.max_premise_replays = stats.premise_replays();
        limits.max_source_row_resolutions = stats.source_row_resolutions();
        limits.max_case_lookups = stats.case_lookups();
        limits.max_group_lookups = stats.group_lookups();
        limits.max_geometry_shape_checks = stats.geometry_shape_checks();
        limits.max_geometry_integer_entries = stats.geometry_integer_entries();
        limits.max_geometry_integer_bits = stats.geometry_integer_bits();
        limits.max_compact_plan_compilations = stats.compact_plan_compilations();
        limits.max_compact_plan_replays = stats.compact_plan_replays();
        limits.max_translation_components = stats.translation_components();
        limits.max_target_row_label_bytes = stats.target_row_label_bytes();
        limits.max_source_terms = stats.source_terms();
        limits.max_source_guards = stats.source_guards();
        limits.max_translated_terms = stats.translated_term_admission_demand();
        limits.max_translated_guards = stats.translated_guard_admission_demand();
        limits.max_translation_polynomials = stats.translation_polynomials();
        limits.max_translation_numerator_polynomials = stats.translation_numerator_polynomials();
        limits.max_translation_denominator_polynomials =
            stats.translation_denominator_polynomials();
        limits.max_total_translation_source_terms = stats.translation_source_terms();
        limits.max_total_translation_source_exponent_entries =
            stats.translation_source_exponent_entries();
        limits.max_total_translation_output_term_bound = stats.translation_output_term_bound();
        limits.max_total_translation_output_exponent_entry_bound =
            stats.translation_output_exponent_entry_bound();
        limits.max_total_translation_power_operation_bound =
            stats.translation_power_operation_bound();
        limits.max_total_translation_integer_bit_work_bound =
            stats.translation_integer_bit_work_bound();
        limits.max_total_translation_normalization_input_term_pairs =
            stats.translation_normalization_input_term_pairs();
        limits.max_total_translation_retained_output_terms =
            stats.translation_retained_output_terms();
        limits.max_total_translation_retained_output_bytes =
            stats.translation_retained_output_bytes();
        limits.max_guard_composition_preflights = stats.guard_composition_preflights();
        limits.max_coefficient_composition_preflights = stats.coefficient_composition_preflights();
        limits.max_numerator_composition_preflights = stats.numerator_composition_preflights();
        limits.max_denominator_composition_preflights = stats.denominator_composition_preflights();
        limits.max_prepared_composition_token_bytes =
            stats.prepared_composition_token_byte_envelope();
        limits.max_guard_compositions = stats.guard_compositions();
        limits.max_coefficient_compositions = stats.coefficient_compositions();
        limits.max_numerator_compositions = stats.numerator_compositions();
        limits.max_denominator_compositions = stats.denominator_compositions();
        limits.max_total_source_terms = stats
            .preflight_total_source_terms()
            .max(stats.total_source_terms());
        limits.max_total_source_exponent_entries = stats
            .preflight_total_source_exponent_entries()
            .max(stats.total_source_exponent_entries());
        limits.max_total_expanded_contributions = stats
            .preflight_total_expanded_contributions()
            .max(stats.total_expanded_contributions());
        limits.max_total_output_term_bound = stats
            .preflight_total_output_term_bound()
            .max(stats.total_output_term_bound());
        limits.max_total_output_terms = stats
            .preflight_total_output_terms()
            .max(stats.total_output_terms());
        limits.max_total_output_exponent_entry_bound = stats
            .preflight_total_output_exponent_entry_bound()
            .max(stats.total_output_exponent_entry_bound());
        limits.max_total_output_exponent_entries = stats
            .preflight_total_output_exponent_entries()
            .max(stats.total_output_exponent_entries());
        limits.max_total_power_calls = stats
            .preflight_total_power_calls()
            .max(stats.total_power_calls());
        limits.max_total_native_power_heap_pairs = stats
            .preflight_total_native_power_heap_pairs()
            .max(stats.total_native_power_heap_pairs());
        limits.max_total_multiplication_term_pairs = stats
            .preflight_total_multiplication_term_pairs()
            .max(stats.total_multiplication_term_pairs());
        limits.max_total_addition_term_visits = stats
            .preflight_total_addition_term_visits()
            .max(stats.total_addition_term_visits());
        limits.max_total_native_integer_bit_work = stats
            .preflight_total_native_integer_bit_work()
            .max(stats.total_native_integer_bit_work());
        limits.max_total_integer_bit_work = stats
            .preflight_total_integer_bit_work()
            .max(stats.total_integer_bit_work());
        limits.max_total_normalization_input_term_pairs = stats
            .preflight_total_normalization_input_term_pairs()
            .max(stats.total_normalization_input_term_pairs());
        limits.max_total_durable_denominator_terms = stats
            .preflight_total_durable_denominator_terms()
            .max(stats.total_durable_denominator_terms());
        limits.max_total_durable_denominator_exponent_entries = stats
            .preflight_total_durable_denominator_exponent_entries()
            .max(stats.total_durable_denominator_exponent_entries());
        limits.max_total_durable_denominator_integer_bits = stats
            .preflight_total_durable_denominator_integer_bits()
            .max(stats.total_durable_denominator_integer_bits());
        limits.max_condition_classifications = stats.condition_classification_admission_demand();
        limits.max_inherited_premise_comparisons =
            stats.inherited_premise_comparison_admission_demand();
        limits.max_inherited_premise_matches = stats.inherited_premise_matches();
        limits.max_private_guard_associate_comparisons =
            stats.private_guard_associate_comparison_admission_demand();
        limits.max_base_assumption_associate_comparisons =
            stats.base_assumption_associate_comparison_admission_demand();
        limits.max_row_local_base_assumptions = stats.row_local_base_assumptions();
        limits.max_private_free_index_guards = stats.private_free_index_guards();
        limits.max_condition_witnesses = stats.condition_witness_admission_demand();
        limits.max_relation_manifest_bytes = stats.relation_manifest_bytes();
        limits.max_retained_terms = stats.retained_term_envelope();
        limits.max_retained_bytes = stats.retained_byte_envelope();
        limits.max_peak_scratch_bytes = stats.peak_scratch_byte_envelope();
        limits
    }

    fn exact_natural_limits(
        fixture: &NaturalFixture,
    ) -> (
        GeneratedAffineResidualCaseBoundRelationLimits,
        GeneratedAffineResidualCaseBoundRelationStats,
    ) {
        let baseline = fixture
            .compile(GeneratedAffineResidualCaseBoundRelationLimits::default())
            .unwrap();
        let provisional = limits_from_successful_stats(compilation_stats(&baseline));
        let converged = fixture.compile(provisional).unwrap_or_else(|error| match error {
            GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
                resource,
                requested,
                limit,
            } => panic!(
                "provisional exact limits rejected {resource}: requested {requested}, limit {limit}"
            ),
            _ => panic!("provisional exact limits returned {}", error.kind()),
        });
        let converged_stats = compilation_stats(&converged);
        let exact = limits_from_successful_stats(converged_stats);
        let verified = fixture.compile(exact).unwrap();
        assert_eq!(compilation_stats(&verified), converged_stats);
        (exact, converged_stats)
    }

    fn assert_outer_one_below(
        fixture: &NaturalFixture,
        exact: GeneratedAffineResidualCaseBoundRelationLimits,
        name: &'static str,
        value: usize,
        lower: impl FnOnce(&mut GeneratedAffineResidualCaseBoundRelationLimits, usize),
    ) {
        if value == 0 {
            return;
        }
        let mut limits = exact;
        lower(&mut limits, value - 1);
        match fixture.compile(limits) {
            Err(GeneratedAffineResidualCaseBoundRelationError::ResourceLimit { .. }) => {}
            Err(error) => panic!("{name} returned {} instead of ResourceLimit", error.kind()),
            Ok(_) => panic!("{name} accepted one below its successful demand"),
        }
    }

    fn assert_folded_one_below(
        fixture: &NaturalFixture,
        exact: GeneratedAffineResidualCaseBoundRelationLimits,
        name: &'static str,
        value: usize,
        lower: impl FnOnce(&mut GeneratedAffineResidualCaseBoundRelationLimits, usize),
        expected: GeneratedAffineResidualCaseBoundRelationError,
    ) {
        if value == 0 {
            return;
        }
        let mut limits = exact;
        lower(&mut limits, value - 1);
        match fixture.compile(limits) {
            Err(error) if error == expected => {}
            Err(error) => panic!(
                "{name} returned {} instead of {}",
                error.kind(),
                expected.kind()
            ),
            Ok(_) => panic!("{name} accepted one below its successful demand"),
        }
    }

    fn equal_mass_two_loop_family(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![one.clone(), zero.clone(), zero.clone()],
                ),
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![zero.clone(), zero.clone(), one.clone()],
                ),
                AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
            ],
            Vec::new(),
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap()
    }

    fn natural_fixture(name: &str) -> NaturalFixture {
        let family = equal_mass_two_loop_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string("011").unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 0;
        queue_limits.max_translation_points = 1;
        let queue = Arc::new(
            GeneratedSectorLiveLeafQueueCompiler::compile(
                &family,
                &context,
                &discovery,
                queue_limits,
            )
            .unwrap(),
        );
        let boolean = Arc::new(
            GeneratedAffineResidualBooleanCoverCompiler::compile(
                &family,
                &context,
                GeneratedAffineResidualSourceAuthority::initial_global(queue),
                GeneratedAffineResidualBooleanCoverLimits::default(),
            )
            .unwrap(),
        );
        let inventory = Arc::new(
            GeneratedAffineResidualCaseInventoryCompiler::compile(
                &family,
                &context,
                boolean,
                GeneratedAffineResidualCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        assert!(inventory.case_count() > 4);
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                &family,
                &context,
                Arc::clone(&inventory),
                4,
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        let premises = Arc::new(
            match compile_generated_affine_residual_case_premises(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineResidualCasePremisesLimits::default(),
            )
            .unwrap()
            {
                GeneratedAffineResidualCasePremisesOutcome::Ready(certificate) => certificate,
                GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(_) => {
                    panic!("natural case four unexpectedly requires equality refinement")
                }
            },
        );
        let ordering = Arc::new(
            GeneratedAffineParametricOrderingCertificate::try_new(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineParametricOrderingLimits::default(),
            )
            .unwrap(),
        );
        let schedule = Arc::new(
            GeneratedAffinePreparePointScheduleCertificate::compile(
                &family,
                &context,
                Arc::clone(&ordering),
                &authority,
                1,
                GeneratedAffinePreparePointScheduleLimits::default(),
            )
            .unwrap(),
        );
        assert!(schedule.layers()[1].point_count() > 0);

        let source_row_ordinal = (0..authority.source_row_count())
            .max_by_key(|&ordinal| {
                let row = authority
                    .authenticated_source_row_view(
                        &family,
                        &context,
                        ordinal,
                        GeneratedAffineResidualCaseSourceRowLimits::default(),
                    )
                    .unwrap();
                let nonconstant_denominators = row
                    .relation()
                    .terms()
                    .values()
                    .filter(|coefficient| coefficient.raw().denominator.nterms() > 1)
                    .count();
                (
                    row.relation().guarded_nonzero_conditions().len(),
                    nonconstant_denominators,
                    row.relation().terms().len(),
                )
            })
            .unwrap();
        NaturalFixture {
            family,
            context,
            inventory,
            authority,
            ordering,
            schedule,
            premises,
            source_row_ordinal,
            point_depth: 1,
            point_ordinal: 0,
        }
    }

    struct OracleRow {
        translated: ParametricRelation,
        plan: Arc<ResidualAffineCompactCompositionPlan>,
        constants: Vec<Integer>,
        free_positions: Vec<usize>,
        linear_coefficients: Vec<Integer>,
    }

    fn oracle_row(fixture: &NaturalFixture) -> OracleRow {
        let point = fixture
            .schedule
            .authenticate_point_handle(
                &fixture.family,
                &fixture.context,
                &fixture.ordering,
                &fixture.authority,
                fixture
                    .schedule
                    .point_handle(fixture.point_depth, fixture.point_ordinal)
                    .unwrap(),
                GeneratedAffinePreparePointAuthenticationLimits::default(),
            )
            .unwrap();
        let source = fixture
            .authority
            .authenticated_source_row_view(
                &fixture.family,
                &fixture.context,
                fixture.source_row_ordinal,
                GeneratedAffineResidualCaseSourceRowLimits::default(),
            )
            .unwrap();
        let translated = source
            .relation()
            .translated(
                &fixture.context,
                point.translation(),
                ParametricRowId::Derived {
                    label: Arc::from("independent-translate-then-compose-oracle"),
                },
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        let case = fixture
            .authority
            .authenticated_case_view(&fixture.context)
            .unwrap();
        let group = fixture
            .authority
            .authenticated_group_view(&fixture.context)
            .unwrap();
        let constants = case.constants().to_vec();
        let free_positions = group.free_positions().to_vec();
        let linear_coefficients = group.compact_linear_coefficients().to_vec();
        let geometry = ResidualAffineCompactMapView::new(
            fixture.context.fingerprint(),
            group.ambient_arity(),
            case.constants(),
            group.free_positions(),
            group.compact_linear_coefficients(),
        );
        let plan = Arc::new(
            fixture
                .context
                .compile_residual_affine_compact_composition_plan(
                    geometry,
                    ResidualAffineCompactCompositionPlanLimits::default(),
                )
                .unwrap(),
        );
        OracleRow {
            translated,
            plan,
            constants,
            free_positions,
            linear_coefficients,
        }
    }

    fn assert_condition_witness_matches(
        fixture: &NaturalFixture,
        certificate: &GeneratedAffineResidualCaseBoundParametricRelation,
        polynomial: &ParametricPolynomial,
        class: GeneratedAffineResidualCaseBoundConditionClass,
    ) {
        let index_dependent = fixture
            .context
            .polynomial_depends_on_indices_with_limits(
                polynomial,
                certificate.limits().polynomial_composition.exact_algebra,
            )
            .unwrap();
        let associate = |other: &ParametricPolynomial, expected_index_dependent: Option<bool>| {
            if expected_index_dependent.is_some_and(|expected| expected != index_dependent) {
                return false;
            }
            if polynomial == other {
                return true;
            }
            let other_index_dependent = fixture
                .context
                .polynomial_depends_on_indices_with_limits(
                    other,
                    certificate.limits().polynomial_composition.exact_algebra,
                )
                .unwrap();
            if other_index_dependent != index_dependent {
                return false;
            }
            distinct_condition_loci_are_associates(
                &fixture.context,
                polynomial,
                other,
                index_dependent,
                certificate.limits(),
            )
            .unwrap()
        };
        match class {
            GeneratedAffineResidualCaseBoundConditionClass::DischargedNonzeroIntegerConstant => {
                assert!(polynomial.is_nonzero_constant());
            }
            GeneratedAffineResidualCaseBoundConditionClass::InheritedPremise { ordinal } => {
                assert!(associate(
                    fixture.premises.premises()[ordinal].polynomial(),
                    None
                ));
            }
            GeneratedAffineResidualCaseBoundConditionClass::RowLocalBaseAssumption { ordinal } => {
                assert!(associate(
                    certificate.base_assumptions()[ordinal]
                        .condition()
                        .polynomial(),
                    Some(false),
                ));
            }
            GeneratedAffineResidualCaseBoundConditionClass::PrivateFreeIndexGuard { ordinal } => {
                assert!(associate(
                    certificate.relation().guarded_nonzero_conditions()[ordinal].polynomial(),
                    Some(true),
                ));
            }
        }
    }

    #[test]
    fn production_prefix_is_topology_neutral() {
        let implementation = include_str!("generated_affine_residual_case_bound_relation.rs");
        let production = implementation
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("the test-module boundary is present");
        for topology_marker in [
            "sunset",
            "vacuum_bubble",
            "one_loop",
            "two_loop",
            "three_loop",
            "four_loop",
            "five_loop",
            "equal_mass",
        ] {
            assert!(
                !production.contains(topology_marker),
                "production implementation contains topology marker {topology_marker}"
            );
        }
    }

    #[test]
    fn exact_successful_census_is_self_reproducing() {
        let fixture = natural_fixture("bound-v2-exact-census-private");
        let (exact, stats) = exact_natural_limits(&fixture);
        assert_eq!(exact.max_retained_terms, stats.retained_term_envelope());
        assert_eq!(exact.max_retained_bytes, stats.retained_byte_envelope());
        assert_eq!(
            exact.max_peak_scratch_bytes,
            stats.peak_scratch_byte_envelope()
        );
        assert_eq!(
            exact.max_prepared_composition_token_bytes,
            stats.prepared_composition_token_byte_envelope()
        );
        assert!(stats.translated_terms() <= stats.translated_term_admission_demand());
        assert!(stats.translated_guards() <= stats.translated_guard_admission_demand());
    }

    #[test]
    fn aggregate_clamp_provenance_maps_only_exclusive_outer_failures() {
        let map = |limits: GeneratedAffineResidualCaseBoundRelationLimits,
                   stats: GeneratedAffineResidualCaseBoundRelationStats,
                   resource: &'static str,
                   requested: usize,
                   child_limit: usize| {
            let call = remaining_composition_limits(limits, &stats).unwrap();
            map_prepared_composition_error(
                ResidualUnitAffineCompositionError::ResourceLimit {
                    resource,
                    requested,
                    limit: child_limit,
                },
                call,
                limits,
                &stats,
            )
        };

        let mut limits = GeneratedAffineResidualCaseBoundRelationLimits::default();
        limits.polynomial_composition.max_source_terms = 100;
        limits.max_total_source_terms = 15;
        let mut stats = GeneratedAffineResidualCaseBoundRelationStats::default();
        stats.total_source_terms = 10;
        assert_eq!(
            map(limits, stats, "polynomial source terms", 6, 5),
            GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
                resource: "total source terms",
                requested: 16,
                limit: 15,
            }
        );

        // Both selected Symbolica affine backends expose the same aggregate
        // budget while retaining their backend-specific child resource label.
        let mut addition_limits = GeneratedAffineResidualCaseBoundRelationLimits::default();
        addition_limits
            .polynomial_composition
            .max_addition_term_visits = 100;
        addition_limits.max_total_addition_term_visits = 15;
        let mut addition_stats = GeneratedAffineResidualCaseBoundRelationStats::default();
        addition_stats.total_addition_term_visits = 10;
        for resource in [
            "native addition term visits",
            "Symbolica backend structural term visits",
        ] {
            assert_eq!(
                map(addition_limits, addition_stats, resource, 6, 5),
                GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
                    resource: "total addition term visits",
                    requested: 16,
                    limit: 15,
                },
                "failed to remap {resource}",
            );
        }

        // Numerator work is reflected by the reduction from the top-of-call
        // cap (10) to the denominator's reported child cap (6).
        limits.max_total_source_terms = 17;
        stats.total_source_terms = 7;
        assert_eq!(
            map(limits, stats, "polynomial source terms", 7, 6),
            GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
                resource: "total source terms",
                requested: 18,
                limit: 17,
            }
        );

        // Equality does not prove that the outer limit caused the child
        // rejection, and a strictly smaller child limit is likewise private
        // to the composition layer.
        limits.max_total_source_terms = 107;
        limits.polynomial_composition.max_source_terms = 100;
        assert_eq!(
            map(limits, stats, "polynomial source terms", 101, 100),
            GeneratedAffineResidualCaseBoundRelationError::Composition
        );
        limits.max_total_source_terms = 112;
        limits.polynomial_composition.max_source_terms = 100;
        assert_eq!(
            map(limits, stats, "polynomial source terms", 101, 100),
            GeneratedAffineResidualCaseBoundRelationError::Composition
        );

        let mut overflow_limits = GeneratedAffineResidualCaseBoundRelationLimits::default();
        overflow_limits.polynomial_composition.max_source_terms = usize::MAX;
        overflow_limits.max_total_source_terms = usize::MAX;
        let mut overflow_stats = GeneratedAffineResidualCaseBoundRelationStats::default();
        overflow_stats.total_source_terms = usize::MAX - 1;
        assert_eq!(
            map(
                overflow_limits,
                overflow_stats,
                "polynomial source terms",
                usize::MAX,
                1,
            ),
            GeneratedAffineResidualCaseBoundRelationError::ResourceCountOverflow {
                resource: "total source terms",
            }
        );

        let mut coefficient_limits = GeneratedAffineResidualCaseBoundRelationLimits::default();
        coefficient_limits
            .polynomial_composition
            .max_normalization_input_term_pairs = 100;
        coefficient_limits.max_total_normalization_input_term_pairs = 7;
        coefficient_limits
            .polynomial_composition
            .max_integer_bit_work = 100;
        coefficient_limits.max_total_integer_bit_work = 13;
        let mut coefficient_stats = GeneratedAffineResidualCaseBoundRelationStats::default();
        coefficient_stats.total_normalization_input_term_pairs = 4;
        coefficient_stats.total_integer_bit_work = 8;
        assert_eq!(
            map(
                coefficient_limits,
                coefficient_stats,
                "coefficient normalization input term-pair bound",
                4,
                3,
            ),
            GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
                resource: "total normalization input term pairs",
                requested: 8,
                limit: 7,
            }
        );
        assert_eq!(
            map(
                coefficient_limits,
                coefficient_stats,
                "coefficient total integer-bit work bound",
                6,
                5,
            ),
            GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
                resource: "total integer bit work",
                requested: 14,
                limit: 13,
            }
        );
        coefficient_limits
            .polynomial_composition
            .exact_algebra
            .max_term_operations = 3;
        assert_eq!(
            map(
                coefficient_limits,
                coefficient_stats,
                "coefficient normalization input term-pair bound",
                4,
                3,
            ),
            GeneratedAffineResidualCaseBoundRelationError::Composition
        );

        let shared_case = |expanded_remaining: usize,
                           output_remaining: usize,
                           child_expanded: usize,
                           child_output: usize,
                           exact_cap: usize,
                           child_limit: usize| {
            let mut limits = GeneratedAffineResidualCaseBoundRelationLimits::default();
            limits.polynomial_composition.max_expanded_contributions = child_expanded;
            limits.polynomial_composition.max_output_terms = child_output;
            limits
                .polynomial_composition
                .exact_algebra
                .max_polynomial_terms = exact_cap;
            limits.max_total_expanded_contributions = 10 + expanded_remaining;
            limits.max_total_output_term_bound = 10 + output_remaining;
            let mut stats = GeneratedAffineResidualCaseBoundRelationStats::default();
            stats.total_expanded_contributions = 10;
            stats.total_output_term_bound = 10;
            (
                map(
                    limits,
                    stats,
                    "affine power terms",
                    child_limit + 1,
                    child_limit,
                ),
                limits,
                stats,
            )
        };

        assert_eq!(
            shared_case(5, 8, 200, 200, 1_000, 5).0,
            GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
                resource: "total expanded contributions",
                requested: 16,
                limit: 15,
            }
        );
        assert_eq!(
            shared_case(8, 5, 200, 200, 1_000, 5).0,
            GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
                resource: "total output term bound",
                requested: 16,
                limit: 15,
            }
        );
        assert_eq!(
            shared_case(5, 5, 200, 200, 1_000, 5).0,
            GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
                resource: "total expanded contributions",
                requested: 16,
                limit: 15,
            }
        );
        assert_eq!(
            shared_case(5, usize::MAX - 10, 200, 5, 1_000, 5).0,
            GeneratedAffineResidualCaseBoundRelationError::Composition
        );
        assert_eq!(
            shared_case(5, 8, 200, 200, 5, 5).0,
            GeneratedAffineResidualCaseBoundRelationError::Composition
        );

        let mut direct_limits = GeneratedAffineResidualCaseBoundRelationLimits::default();
        direct_limits
            .polynomial_composition
            .max_expanded_contributions = 200;
        direct_limits.polynomial_composition.max_output_terms = 200;
        direct_limits.max_total_expanded_contributions = 10;
        direct_limits.max_total_output_term_bound = 5;
        let direct_stats = GeneratedAffineResidualCaseBoundRelationStats::default();
        assert_eq!(
            map(
                direct_limits,
                direct_stats,
                "expanded polynomial contributions",
                16,
                10,
            ),
            GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
                resource: "total expanded contributions",
                requested: 16,
                limit: 10,
            }
        );
        direct_limits.polynomial_composition.max_output_terms = 5;
        direct_limits.max_total_output_term_bound = usize::MAX;
        assert_eq!(
            map(
                direct_limits,
                direct_stats,
                "expanded polynomial contributions",
                16,
                10,
            ),
            GeneratedAffineResidualCaseBoundRelationError::Composition
        );
        direct_limits.polynomial_composition.max_output_terms = 200;
        direct_limits.max_total_output_term_bound = 5;
        direct_limits
            .polynomial_composition
            .exact_algebra
            .max_polynomial_terms = 10;
        assert_eq!(
            map(
                direct_limits,
                direct_stats,
                "expanded polynomial contributions",
                16,
                10,
            ),
            GeneratedAffineResidualCaseBoundRelationError::Composition
        );

        // The top-of-call outer cap (100) is above exact algebra (80), but
        // numerator consumption lowers the denominator child cap to 70. The
        // reported child cap, not the top-level cap, proves outer provenance.
        let (ignored, mut denominator_limits, mut denominator_stats) =
            shared_case(100, 120, 200, 200, 80, 70);
        denominator_limits.max_total_expanded_contributions = 110;
        denominator_stats.total_expanded_contributions = 10;
        assert_eq!(
            map(
                denominator_limits,
                denominator_stats,
                "affine power terms",
                71,
                70,
            ),
            GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
                resource: "total expanded contributions",
                requested: 111,
                limit: 110,
            }
        );
        drop(ignored);

        let mut output_limits = GeneratedAffineResidualCaseBoundRelationLimits::default();
        output_limits.polynomial_composition.max_output_terms = 200;
        output_limits.max_total_output_term_bound = 15;
        output_limits
            .polynomial_composition
            .exact_algebra
            .max_polynomial_terms = 5;
        let mut output_stats = GeneratedAffineResidualCaseBoundRelationStats::default();
        output_stats.total_output_term_bound = 10;
        assert_eq!(
            map(
                output_limits,
                output_stats,
                "prospective output terms",
                6,
                5,
            ),
            GeneratedAffineResidualCaseBoundRelationError::Composition
        );
    }

    #[test]
    fn prepared_multipower_direct_expansion_maps_exact_outer_resource() {
        let fixture = natural_fixture("bound-v2-prepared-multipower-private");
        let arity = fixture.context.index_count();
        assert!(arity >= 3);

        // A topology-neutral idempotent affine projection: n0 is free and
        // every other ambient index maps to 1+n0. Thus n1^3*n2^3 has two
        // affine powers with four terms each. Each power passes the shared
        // cap five, while their direct Cartesian product requests 16 terms.
        let mut constants = vec![Integer::one(); arity];
        constants[0] = Integer::zero();
        let free_positions = vec![0usize];
        let linear_coefficients = vec![Integer::one(); arity];
        let geometry = ResidualAffineCompactMapView::new(
            fixture.context.fingerprint(),
            arity,
            &constants,
            &free_positions,
            &linear_coefficients,
        );
        let plan = fixture
            .context
            .compile_residual_affine_compact_composition_plan(
                geometry,
                ResidualAffineCompactCompositionPlanLimits::default(),
            )
            .unwrap();
        let mut source = fixture.context.one();
        for position in [1usize, 2] {
            let index = fixture.context.index(position).unwrap();
            for _ in 0..3 {
                source = fixture.context.mul(&source, &index).unwrap();
            }
        }
        let polynomial = fixture.context.numerator_condition(&source).unwrap();
        let mut translated = synthetic_relation(&fixture, "prepared-multipower-input");
        translated
            .add_nonzero_condition(&fixture.context, polynomial.clone())
            .unwrap();

        let mut limits = GeneratedAffineResidualCaseBoundRelationLimits::default();
        limits.max_total_expanded_contributions = 10;
        limits.max_total_output_term_bound = 5;
        let zero_stats = GeneratedAffineResidualCaseBoundRelationStats::default();
        let call = remaining_composition_limits(limits, &zero_stats).unwrap();
        assert!(call.clamps.expanded_contributions);
        assert!(call.clamps.output_term_bound);
        assert!(matches!(
            fixture
                .context
                .prepare_guard_on_residual_affine_compact_composition_plan(
                    &polynomial,
                    &plan,
                    call.effective,
                ),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "expanded polynomial contributions",
                requested: 16,
                limit: 10,
            })
        ));
        assert_eq!(
            preflight_complete_row_compositions(&fixture.context, &translated, &plan, limits,)
                .err()
                .unwrap(),
            GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
                resource: "total expanded contributions",
                requested: 16,
                limit: 10,
            }
        );
    }

    #[test]
    fn exact_limits_one_below_authority_point_and_source_shard() {
        let fixture = natural_fixture("bound-v2-limit-authority-source-private");
        let (exact, stats) = exact_natural_limits(&fixture);
        macro_rules! below {
            ($field:ident, $value:expr) => {
                assert_outer_one_below(
                    &fixture,
                    exact,
                    stringify!($field),
                    $value,
                    |limits, value| {
                        limits.$field = value;
                    },
                );
            };
        }
        below!(max_scope_comparison_bytes, stats.scope_comparison_bytes());
        below!(
            max_parent_allocation_comparisons,
            stats.parent_allocation_comparisons()
        );
        below!(max_premise_replays, stats.premise_replays());
        below!(max_source_row_resolutions, stats.source_row_resolutions());
        below!(max_case_lookups, stats.case_lookups());
        below!(max_group_lookups, stats.group_lookups());
        below!(max_geometry_shape_checks, stats.geometry_shape_checks());
        below!(
            max_geometry_integer_entries,
            stats.geometry_integer_entries()
        );
        below!(max_geometry_integer_bits, stats.geometry_integer_bits());
        below!(
            max_compact_plan_compilations,
            stats.compact_plan_compilations()
        );
        below!(max_compact_plan_replays, stats.compact_plan_replays());

        let source = stats.source_row();
        for (name, value, lower) in [
            (
                "source_row.max_scope_comparison_bytes",
                source.scope_comparison_bytes(),
                0usize,
            ),
            ("source_row.max_source_rows", source.source_rows(), 1usize),
            (
                "source_row.max_relation_terms",
                source.relation_terms(),
                2usize,
            ),
            (
                "source_row.max_guard_conditions",
                source.guard_conditions(),
                3usize,
            ),
        ] {
            assert_folded_one_below(
                &fixture,
                exact,
                name,
                value,
                |limits, value| match lower {
                    0 => limits.source_row.max_scope_comparison_bytes = value,
                    1 => limits.source_row.max_source_rows = value,
                    2 => limits.source_row.max_relation_terms = value,
                    3 => limits.source_row.max_guard_conditions = value,
                    _ => unreachable!(),
                },
                GeneratedAffineResidualCaseBoundRelationError::SourceBinding,
            );
        }

        let point = stats.point_authentication();
        for (name, value, lower) in [
            (
                "point_authentication.max_schedule_replays",
                point.schedule_replays(),
                0usize,
            ),
            (
                "point_authentication.max_pointer_checks",
                point.pointer_checks(),
                1usize,
            ),
            (
                "point_authentication.max_index_checks",
                point.index_checks(),
                2usize,
            ),
        ] {
            assert_folded_one_below(
                &fixture,
                exact,
                name,
                value,
                |limits, value| match lower {
                    0 => limits.point_authentication.max_schedule_replays = value,
                    1 => limits.point_authentication.max_pointer_checks = value,
                    2 => limits.point_authentication.max_index_checks = value,
                    _ => unreachable!(),
                },
                GeneratedAffineResidualCaseBoundRelationError::WrongPointBinding,
            );
        }
    }

    #[test]
    fn exact_limits_one_below_compact_plan_and_translation_shape_shard() {
        let fixture = natural_fixture("bound-v2-limit-plan-translation-private");
        let (exact, stats) = exact_natural_limits(&fixture);
        let plan = stats.compact_plan();
        let composition = plan.composition();
        macro_rules! plan_composition_below {
            ($field:ident, $value:expr) => {
                assert_folded_one_below(
                    &fixture,
                    exact,
                    concat!("compact_plan.composition.", stringify!($field)),
                    $value,
                    |limits, value| limits.compact_plan.composition.$field = value,
                    GeneratedAffineResidualCaseBoundRelationError::Composition,
                );
            };
        }
        macro_rules! plan_below {
            ($field:ident, $value:expr) => {
                assert_folded_one_below(
                    &fixture,
                    exact,
                    concat!("compact_plan.", stringify!($field)),
                    $value,
                    |limits, value| limits.compact_plan.$field = value,
                    GeneratedAffineResidualCaseBoundRelationError::Composition,
                );
            };
        }
        plan_composition_below!(max_variables, composition.variables());
        plan_composition_below!(max_full_images, composition.full_images());
        plan_composition_below!(
            max_geometry_entries_inspected,
            composition.geometry_entries_inspected()
        );
        plan_composition_below!(
            max_geometry_entries_retained,
            composition.geometry_entries_retained()
        );
        plan_composition_below!(
            max_support_entries_retained,
            composition.support_entries_retained()
        );
        plan_composition_below!(max_total_image_terms, composition.total_image_terms());
        plan_composition_below!(
            max_total_image_exponent_entries,
            composition.total_image_exponent_entries()
        );
        plan_composition_below!(
            max_image_integer_bits,
            composition.largest_image_integer_bits()
        );
        plan_composition_below!(
            max_total_image_integer_bits,
            composition.total_image_integer_bits()
        );
        plan_below!(
            max_context_fingerprint_bytes,
            plan.context_fingerprint_bytes()
        );
        plan_below!(
            max_geometry_integer_bit_work,
            plan.geometry_integer_bit_work()
        );
        plan_below!(
            max_geometry_replay_comparison_work,
            plan.geometry_replay_comparison_work()
        );
        plan_below!(
            max_geometry_replay_integer_bit_work,
            plan.geometry_replay_integer_bit_work()
        );
        plan_below!(
            max_geometry_replay_scratch_logical_bytes,
            plan.geometry_replay_scratch_logical_bytes()
        );
        plan_below!(
            max_retained_owned_logical_bytes,
            plan.retained_owned_logical_bytes()
        );
        plan_below!(
            max_compilation_owned_logical_peak_upper_bound,
            plan.compilation_owned_logical_peak_upper_bound()
        );

        macro_rules! below {
            ($field:ident, $value:expr) => {
                assert_outer_one_below(
                    &fixture,
                    exact,
                    stringify!($field),
                    $value,
                    |limits, value| {
                        limits.$field = value;
                    },
                );
            };
        }
        below!(max_translation_components, stats.translation_components());
        below!(max_target_row_label_bytes, stats.target_row_label_bytes());
        below!(max_source_terms, stats.source_terms());
        below!(max_source_guards, stats.source_guards());
        below!(
            max_translated_terms,
            stats.translated_term_admission_demand()
        );
        below!(
            max_translated_guards,
            stats.translated_guard_admission_demand()
        );
        below!(max_translation_polynomials, stats.translation_polynomials());
        below!(
            max_translation_numerator_polynomials,
            stats.translation_numerator_polynomials()
        );
        below!(
            max_translation_denominator_polynomials,
            stats.translation_denominator_polynomials()
        );
    }

    #[test]
    fn exact_limits_one_below_translation_and_composition_census_shard() {
        let fixture = natural_fixture("bound-v2-limit-algebra-census-private");
        let (exact, stats) = exact_natural_limits(&fixture);
        macro_rules! below {
            ($field:ident, $value:expr) => {
                assert_outer_one_below(
                    &fixture,
                    exact,
                    stringify!($field),
                    $value,
                    |limits, value| {
                        limits.$field = value;
                    },
                );
            };
        }
        below!(
            max_total_translation_source_terms,
            stats.translation_source_terms()
        );
        below!(
            max_total_translation_source_exponent_entries,
            stats.translation_source_exponent_entries()
        );
        below!(
            max_total_translation_output_term_bound,
            stats.translation_output_term_bound()
        );
        below!(
            max_total_translation_output_exponent_entry_bound,
            stats.translation_output_exponent_entry_bound()
        );
        below!(
            max_total_translation_power_operation_bound,
            stats.translation_power_operation_bound()
        );
        below!(
            max_total_translation_integer_bit_work_bound,
            stats.translation_integer_bit_work_bound()
        );
        below!(
            max_total_translation_normalization_input_term_pairs,
            stats.translation_normalization_input_term_pairs()
        );
        below!(
            max_total_translation_retained_output_terms,
            stats.translation_retained_output_terms()
        );
        below!(
            max_total_translation_retained_output_bytes,
            stats.translation_retained_output_bytes()
        );
        below!(
            max_guard_composition_preflights,
            stats.guard_composition_preflights()
        );
        below!(
            max_coefficient_composition_preflights,
            stats.coefficient_composition_preflights()
        );
        below!(
            max_numerator_composition_preflights,
            stats.numerator_composition_preflights()
        );
        below!(
            max_denominator_composition_preflights,
            stats.denominator_composition_preflights()
        );
        below!(
            max_prepared_composition_token_bytes,
            stats.prepared_composition_token_byte_envelope()
        );
        below!(max_guard_compositions, stats.guard_compositions());
        below!(
            max_coefficient_compositions,
            stats.coefficient_compositions()
        );
        below!(max_numerator_compositions, stats.numerator_compositions());
        below!(
            max_denominator_compositions,
            stats.denominator_compositions()
        );
        below!(
            max_total_source_terms,
            stats
                .preflight_total_source_terms()
                .max(stats.total_source_terms())
        );
        below!(
            max_total_source_exponent_entries,
            stats
                .preflight_total_source_exponent_entries()
                .max(stats.total_source_exponent_entries())
        );
        below!(
            max_total_expanded_contributions,
            stats
                .preflight_total_expanded_contributions()
                .max(stats.total_expanded_contributions())
        );
        below!(
            max_total_output_term_bound,
            stats
                .preflight_total_output_term_bound()
                .max(stats.total_output_term_bound())
        );
        below!(
            max_total_output_terms,
            stats
                .preflight_total_output_terms()
                .max(stats.total_output_terms())
        );
        below!(
            max_total_output_exponent_entry_bound,
            stats
                .preflight_total_output_exponent_entry_bound()
                .max(stats.total_output_exponent_entry_bound())
        );
        below!(
            max_total_output_exponent_entries,
            stats
                .preflight_total_output_exponent_entries()
                .max(stats.total_output_exponent_entries())
        );
    }

    #[test]
    fn exact_limits_one_below_execution_conditions_and_memory_shard() {
        let fixture = natural_fixture("bound-v2-limit-execution-memory-private");
        let (exact, stats) = exact_natural_limits(&fixture);
        macro_rules! below {
            ($field:ident, $value:expr) => {
                assert_outer_one_below(
                    &fixture,
                    exact,
                    stringify!($field),
                    $value,
                    |limits, value| {
                        limits.$field = value;
                    },
                );
            };
        }
        below!(
            max_total_power_calls,
            stats
                .preflight_total_power_calls()
                .max(stats.total_power_calls())
        );
        below!(
            max_total_native_power_heap_pairs,
            stats
                .preflight_total_native_power_heap_pairs()
                .max(stats.total_native_power_heap_pairs())
        );
        below!(
            max_total_multiplication_term_pairs,
            stats
                .preflight_total_multiplication_term_pairs()
                .max(stats.total_multiplication_term_pairs())
        );
        below!(
            max_total_addition_term_visits,
            stats
                .preflight_total_addition_term_visits()
                .max(stats.total_addition_term_visits())
        );
        below!(
            max_total_native_integer_bit_work,
            stats
                .preflight_total_native_integer_bit_work()
                .max(stats.total_native_integer_bit_work())
        );
        below!(
            max_total_integer_bit_work,
            stats
                .preflight_total_integer_bit_work()
                .max(stats.total_integer_bit_work())
        );
        below!(
            max_total_normalization_input_term_pairs,
            stats
                .preflight_total_normalization_input_term_pairs()
                .max(stats.total_normalization_input_term_pairs())
        );
        below!(
            max_total_durable_denominator_terms,
            stats
                .preflight_total_durable_denominator_terms()
                .max(stats.total_durable_denominator_terms())
        );
        below!(
            max_total_durable_denominator_exponent_entries,
            stats
                .preflight_total_durable_denominator_exponent_entries()
                .max(stats.total_durable_denominator_exponent_entries())
        );
        below!(
            max_total_durable_denominator_integer_bits,
            stats
                .preflight_total_durable_denominator_integer_bits()
                .max(stats.total_durable_denominator_integer_bits())
        );
        below!(
            max_condition_classifications,
            stats.condition_classification_admission_demand()
        );
        below!(
            max_inherited_premise_comparisons,
            stats.inherited_premise_comparison_admission_demand()
        );
        below!(
            max_inherited_premise_matches,
            stats.inherited_premise_matches()
        );
        below!(
            max_private_guard_associate_comparisons,
            stats.private_guard_associate_comparison_admission_demand()
        );
        below!(
            max_base_assumption_associate_comparisons,
            stats.base_assumption_associate_comparison_admission_demand()
        );
        below!(
            max_row_local_base_assumptions,
            stats.row_local_base_assumptions()
        );
        below!(
            max_private_free_index_guards,
            stats.private_free_index_guards()
        );
        below!(
            max_condition_witnesses,
            stats.condition_witness_admission_demand()
        );
        assert_folded_one_below(
            &fixture,
            exact,
            "max_relation_manifest_bytes",
            stats.relation_manifest_bytes(),
            |limits, value| limits.max_relation_manifest_bytes = value,
            GeneratedAffineResidualCaseBoundRelationError::RelationConstruction,
        );
        below!(max_retained_terms, stats.retained_term_envelope());
        below!(max_retained_bytes, stats.retained_byte_envelope());
        below!(max_peak_scratch_bytes, stats.peak_scratch_byte_envelope());
    }

    #[test]
    fn combined_peak_is_admitted_before_prepared_token_reserves() {
        let fixture = natural_fixture("bound-v2-pre-token-peak-private");
        let compilation = fixture
            .compile(GeneratedAffineResidualCaseBoundRelationLimits::default())
            .unwrap();
        let GeneratedAffineResidualCaseBoundRelationCompilation::Retained(certificate) =
            compilation
        else {
            panic!("pre-token peak fixture unexpectedly unavailable")
        };
        let point = fixture
            .schedule
            .authenticate_point_handle(
                &fixture.family,
                &fixture.context,
                &fixture.ordering,
                &fixture.authority,
                fixture
                    .schedule
                    .point_handle(fixture.point_depth, fixture.point_ordinal)
                    .unwrap(),
                GeneratedAffinePreparePointAuthenticationLimits::default(),
            )
            .unwrap();
        let source = fixture
            .authority
            .authenticated_source_row_view(
                &fixture.family,
                &fixture.context,
                fixture.source_row_ordinal,
                GeneratedAffineResidualCaseSourceRowLimits::default(),
            )
            .unwrap();
        let translated = source
            .relation()
            .translated(
                &fixture.context,
                point.translation(),
                certificate.target_row_id().clone(),
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        let stats = certificate.stats();
        let earlier_peak = pre_translation_peak_envelope(
            &stats,
            stats.target_row_label_bytes(),
            stats.compact_plan(),
        )
        .unwrap();
        let token_peak = pre_token_allocation_peak_envelope(
            translated.owned_retained_byte_bound().unwrap(),
            stats.target_row_label_bytes(),
            stats.compact_plan(),
            stats.prepared_composition_token_byte_envelope(),
        )
        .unwrap();
        assert!(
            token_peak > earlier_peak,
            "fixture must isolate the new post-translation pre-reserve gate"
        );
        let mut limits = GeneratedAffineResidualCaseBoundRelationLimits::default();
        limits.max_peak_scratch_bytes = token_peak - 1;
        reset_bound_relation_token_reserve_attempts_for_test();
        assert!(matches!(
            fixture.compile(limits),
            Err(GeneratedAffineResidualCaseBoundRelationError::ResourceLimit { .. })
        ));
        assert_eq!(bound_relation_token_reserve_attempts_for_test(), 0);
    }

    #[test]
    fn complete_row_matches_independent_translate_then_symbolica_compose() {
        let fixture = natural_fixture("bound-v2-complete-differential-private");
        let oracle = oracle_row(&fixture);
        assert!(oracle.translated.terms().len() >= 2);
        assert!(
            oracle
                .translated
                .terms()
                .values()
                .any(|coefficient| coefficient.raw().numerator.nterms() > 1
                    || coefficient.raw().denominator.nterms() > 1)
        );
        let GeneratedAffineResidualCaseBoundRelationCompilation::Retained(certificate) = fixture
            .compile(GeneratedAffineResidualCaseBoundRelationLimits::default())
            .unwrap()
        else {
            panic!("differential fixture unexpectedly became unavailable")
        };
        assert_eq!(
            certificate.condition_witnesses().len(),
            oracle.translated.guarded_nonzero_conditions().len() + oracle.translated.terms().len()
        );

        for (guard_ordinal, guard) in oracle
            .translated
            .guarded_nonzero_conditions()
            .iter()
            .enumerate()
        {
            let native = fixture
                .context
                .compose_guard_on_residual_affine_compact_composition_plan(
                    guard.polynomial(),
                    &oracle.plan,
                    ResidualUnitAffinePolynomialCompositionLimits::default(),
                )
                .unwrap();
            let witness = certificate
                .condition_witnesses()
                .iter()
                .find(|witness| {
                    witness.source()
                        == GeneratedAffineResidualCaseBoundConditionSource::TranslatedSourceGuard {
                            guard_ordinal,
                        }
                })
                .unwrap();
            assert_condition_witness_matches(
                &fixture,
                &certificate,
                native.value(),
                witness.class(),
            );
        }

        let mut expected_nonzero_keys = Vec::new();
        for (term_ordinal, (shift, coefficient)) in oracle.translated.terms().iter().enumerate() {
            let denominator = fixture.context.denominator_condition(coefficient).unwrap();
            let native_denominator = fixture
                .context
                .compose_guard_on_residual_affine_compact_composition_plan(
                    &denominator,
                    &oracle.plan,
                    ResidualUnitAffinePolynomialCompositionLimits::default(),
                )
                .unwrap();

            let ResidualAffineCoefficientComposition::Available(mapped) = fixture
                .context
                .compose_coefficient_on_residual_affine_compact_composition_plan(
                    coefficient,
                    &oracle.plan,
                    ResidualUnitAffinePolynomialCompositionLimits::default(),
                )
                .unwrap()
            else {
                panic!("retained differential fixture has a zero mapped denominator")
            };
            let (mapped_value, mapped_denominator, _) = mapped.into_parts();
            assert_eq!(&mapped_denominator, native_denominator.value());
            if mapped_value.is_zero() {
                assert!(!certificate.relation().terms().contains_key(shift));
            } else {
                assert_eq!(
                    certificate.relation().terms().get(shift),
                    Some(&mapped_value)
                );
                expected_nonzero_keys.push(shift.clone());
            }
            let witness = certificate
                .condition_witnesses()
                .iter()
                .find(|witness| {
                    witness.source()
                        == GeneratedAffineResidualCaseBoundConditionSource::TranslatedSourceTermDenominator {
                            term_ordinal,
                        }
                })
                .unwrap();
            assert_condition_witness_matches(
                &fixture,
                &certificate,
                &mapped_denominator,
                witness.class(),
            );
        }
        assert_eq!(
            certificate
                .relation()
                .terms()
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            expected_nonzero_keys
        );
    }

    fn ambient_assignment(oracle: &OracleRow, free_assignment: &[i64]) -> Vec<i64> {
        let free_count = oracle.free_positions.len();
        (0..oracle.constants.len())
            .map(|row| {
                let mut value = oracle.constants[row].clone();
                for (free_ordinal, &free_position) in oracle.free_positions.iter().enumerate() {
                    let contribution = &oracle.linear_coefficients[row * free_count + free_ordinal]
                        * &Integer::from(free_assignment[free_position]);
                    value = &value + &contribution;
                }
                value.to_i64().unwrap()
            })
            .collect()
    }

    fn affine_constraint_and_surviving_factor(
        fixture: &NaturalFixture,
        oracle: &OracleRow,
    ) -> (crate::ParametricCoefficient, crate::ParametricCoefficient) {
        let mut constraint = None;
        for row in 0..fixture.context.index_count() {
            let mut candidate = fixture.context.index(row).unwrap();
            candidate = fixture
                .context
                .sub(
                    &candidate,
                    &fixture
                        .context
                        .integer(oracle.constants[row].to_i64().unwrap()),
                )
                .unwrap();
            for (free_ordinal, &free_position) in oracle.free_positions.iter().enumerate() {
                let scalar = oracle.linear_coefficients
                    [row * oracle.free_positions.len() + free_ordinal]
                    .to_i64()
                    .unwrap();
                let contribution = fixture
                    .context
                    .mul(
                        &fixture.context.integer(scalar),
                        &fixture.context.index(free_position).unwrap(),
                    )
                    .unwrap();
                candidate = fixture.context.sub(&candidate, &contribution).unwrap();
            }
            let polynomial = fixture.context.numerator_condition(&candidate).unwrap();
            if polynomial.is_zero() {
                continue;
            }
            let mapped = fixture
                .context
                .compose_guard_on_residual_affine_compact_composition_plan(
                    &polynomial,
                    oracle.plan.as_ref(),
                    ResidualUnitAffinePolynomialCompositionLimits::default(),
                )
                .unwrap();
            if mapped.value().is_zero() {
                constraint = Some(candidate);
                break;
            }
        }
        let constraint = constraint.expect("the residual affine case must impose a relation");
        let seed_position = oracle.free_positions.first().copied().unwrap_or(0);
        let surviving = (2..=16)
            .find_map(|offset| {
                let candidate = fixture
                    .context
                    .add(
                        &fixture.context.index(seed_position).unwrap(),
                        &fixture.context.integer(offset),
                    )
                    .unwrap();
                let polynomial = fixture.context.numerator_condition(&candidate).unwrap();
                let mapped = fixture
                    .context
                    .compose_guard_on_residual_affine_compact_composition_plan(
                        &polynomial,
                        oracle.plan.as_ref(),
                        ResidualUnitAffinePolynomialCompositionLimits::default(),
                    )
                    .unwrap();
                (!mapped.value().is_zero()).then_some(candidate)
            })
            .unwrap();
        (constraint, surviving)
    }

    fn synthetic_shift(arity: usize, first: i64) -> IndexShift {
        let mut values = vec![0; arity];
        values[0] = first;
        IndexShift::try_new(values, arity).unwrap()
    }

    fn synthetic_relation(fixture: &NaturalFixture, label: &str) -> ParametricRelation {
        ParametricRelation::new(
            fixture.family.fingerprint_ref(),
            ParametricRowId::Derived {
                label: Arc::from(label),
            },
            &fixture.context,
        )
    }

    fn execute_synthetic_translated_row(
        fixture: &NaturalFixture,
        oracle: &OracleRow,
        translated: &ParametricRelation,
    ) -> Result<
        GeneratedAffineResidualCaseBoundRelationCompilation,
        GeneratedAffineResidualCaseBoundRelationError,
    > {
        execute_synthetic_translated_row_with_limits(
            fixture,
            oracle,
            translated,
            GeneratedAffineResidualCaseBoundRelationLimits::default(),
        )
    }

    fn execute_synthetic_translated_row_with_limits(
        fixture: &NaturalFixture,
        oracle: &OracleRow,
        translated: &ParametricRelation,
        limits: GeneratedAffineResidualCaseBoundRelationLimits,
    ) -> Result<
        GeneratedAffineResidualCaseBoundRelationCompilation,
        GeneratedAffineResidualCaseBoundRelationError,
    > {
        let prepared = preflight_complete_row_compositions(
            &fixture.context,
            translated,
            oracle.plan.as_ref(),
            limits,
        )?;
        let mut stats = GeneratedAffineResidualCaseBoundRelationStats::default();
        stats.compact_plan = oracle.plan.stats();
        stats.translation_components = fixture.context.index_count();
        stats.translated_terms = translated.terms().len();
        stats.translated_guards = translated.guarded_nonzero_conditions().len();
        let target_label: Arc<str> = Arc::from("synthetic-bound-v2-target");
        stats.target_row_label_bytes = target_label.len();
        copy_composition_preflight_census(&prepared.stats, &mut stats);
        stats.retained_term_envelope = prospective_retained_term_envelope(&stats)?;
        check_limit(
            "retained terms",
            stats.retained_term_envelope,
            limits.max_retained_terms,
        )?;
        stats.retained_byte_envelope = prospective_retained_byte_envelope(
            &stats,
            stats.target_row_label_bytes,
            oracle.plan.stats(),
            limits.max_relation_manifest_bytes,
        )?;
        check_limit(
            "retained bytes",
            stats.retained_byte_envelope,
            limits.max_retained_bytes,
        )?;
        stats.peak_scratch_byte_envelope = composition_peak_envelope(
            &stats,
            translated.owned_retained_byte_bound().unwrap(),
            oracle.plan.stats(),
        )?;
        check_limit(
            "peak scratch bytes",
            stats.peak_scratch_byte_envelope,
            limits.max_peak_scratch_bytes,
        )?;
        let point = fixture
            .schedule
            .authenticate_point_handle(
                &fixture.family,
                &fixture.context,
                &fixture.ordering,
                &fixture.authority,
                fixture
                    .schedule
                    .point_handle(fixture.point_depth, fixture.point_ordinal)
                    .unwrap(),
                GeneratedAffinePreparePointAuthenticationLimits::default(),
            )
            .unwrap();
        let source = GeneratedAffineResidualCaseBoundSource {
            authority: Arc::clone(&fixture.authority),
            ordering: Arc::clone(&fixture.ordering),
            schedule: Arc::clone(&fixture.schedule),
            premises: Arc::clone(&fixture.premises),
            source_row_ordinal: fixture.source_row_ordinal,
            point_depth: fixture.point_depth,
            point_ordinal: fixture.point_ordinal,
            point_key: point.key().clone(),
            target_row_id: ParametricRowId::Derived {
                label: target_label,
            },
            composition_plan: Arc::clone(&oracle.plan),
        };
        execute_complete_row(
            &fixture.context,
            translated,
            source,
            limits,
            stats,
            prepared,
        )
    }

    fn add_synthetic_guard(
        fixture: &NaturalFixture,
        relation: &mut ParametricRelation,
        value: &crate::ParametricCoefficient,
    ) {
        relation
            .add_nonzero_condition(
                &fixture.context,
                fixture.context.numerator_condition(value).unwrap(),
            )
            .unwrap();
    }

    #[test]
    fn base_loci_use_q_star_while_rational_associates_reuse_the_first_row() {
        let fixture = natural_fixture("bound-v2-base-q-star-private");
        let oracle = oracle_row(&fixture);
        let exact = ExactAlgebraLimits::default();

        let is_new_base_locus = |value: &crate::ParametricCoefficient| {
            let polynomial = fixture.context.numerator_condition(value).unwrap();
            assert!(
                !fixture
                    .context
                    .polynomial_depends_on_indices_with_limits(&polynomial, exact)
                    .unwrap()
            );
            fixture.premises.premises().iter().all(|inherited| {
                if &polynomial == inherited.polynomial() {
                    return false;
                }
                if fixture
                    .context
                    .polynomial_depends_on_indices_with_limits(inherited.polynomial(), exact)
                    .unwrap()
                {
                    return true;
                }
                !fixture
                    .context
                    .base_polynomial_loci_are_rational_associates_with_census(
                        &polynomial,
                        inherited.polynomial(),
                        ParametricBasePolynomialAssociateLimits::default(),
                    )
                    .unwrap()
                    .associated()
            })
        };

        // Pick two neighboring affine base polynomials outside the inherited
        // premise table. This keeps the regression stable if the generated
        // fixture's premise set grows without weakening the Q* distinction.
        let (first, distinct) = (2..=128)
            .find_map(|offset| {
                let first_base = fixture
                    .context
                    .base()
                    .parse(&format!("d+{offset}"))
                    .unwrap();
                let distinct_base = fixture
                    .context
                    .base()
                    .parse(&format!("d+{}", offset + 1))
                    .unwrap();
                let first = fixture.context.lift(&first_base).unwrap();
                let distinct = fixture.context.lift(&distinct_base).unwrap();
                (is_new_base_locus(&first) && is_new_base_locus(&distinct))
                    .then_some((first, distinct))
            })
            .expect("two non-inherited base loci must be available");
        let rational_associate = fixture
            .context
            .mul(&fixture.context.integer(-2), &first)
            .unwrap();

        let first_polynomial = fixture.context.numerator_condition(&first).unwrap();
        let distinct_polynomial = fixture.context.numerator_condition(&distinct).unwrap();
        let rational_associate_polynomial = fixture
            .context
            .numerator_condition(&rational_associate)
            .unwrap();
        assert!(
            !fixture
                .context
                .base_polynomial_loci_are_rational_associates_with_census(
                    &first_polynomial,
                    &distinct_polynomial,
                    ParametricBasePolynomialAssociateLimits::default(),
                )
                .unwrap()
                .associated()
        );
        assert!(
            fixture
                .context
                .base_polynomial_loci_are_rational_associates_with_census(
                    &first_polynomial,
                    &rational_associate_polynomial,
                    ParametricBasePolynomialAssociateLimits::default(),
                )
                .unwrap()
                .associated()
        );

        let mut translated = synthetic_relation(&fixture, "synthetic-base-q-star-input");
        add_synthetic_guard(&fixture, &mut translated, &first);
        add_synthetic_guard(&fixture, &mut translated, &distinct);
        add_synthetic_guard(&fixture, &mut translated, &rational_associate);

        let mut exact_child_limits = GeneratedAffineResidualCaseBoundRelationLimits::default();
        exact_child_limits
            .base_polynomial_associate
            .max_native_scale_calls = 2;
        let GeneratedAffineResidualCaseBoundRelationCompilation::Retained(certificate) =
            execute_synthetic_translated_row_with_limits(
                &fixture,
                &oracle,
                &translated,
                exact_child_limits,
            )
            .unwrap()
        else {
            panic!("base-only synthetic row unexpectedly became unavailable")
        };
        assert_eq!(certificate.base_assumptions().len(), 2);
        assert!(
            certificate
                .relation()
                .guarded_nonzero_conditions()
                .is_empty()
        );
        assert_eq!(certificate.condition_witnesses().len(), 3);
        assert_eq!(
            certificate.condition_witnesses()[0].class(),
            GeneratedAffineResidualCaseBoundConditionClass::RowLocalBaseAssumption { ordinal: 0 }
        );
        assert_eq!(
            certificate.condition_witnesses()[1].class(),
            GeneratedAffineResidualCaseBoundConditionClass::RowLocalBaseAssumption { ordinal: 1 }
        );
        assert_eq!(
            certificate.condition_witnesses()[2].class(),
            GeneratedAffineResidualCaseBoundConditionClass::RowLocalBaseAssumption { ordinal: 0 }
        );

        let mut one_below = exact_child_limits;
        one_below.base_polynomial_associate.max_native_scale_calls = 1;
        assert_eq!(
            execute_synthetic_translated_row_with_limits(
                &fixture,
                &oracle,
                &translated,
                one_below,
            )
            .unwrap_err(),
            GeneratedAffineResidualCaseBoundRelationError::ConditionMaterialization
        );
    }

    #[test]
    fn constructed_multiple_guards_zero_numerator_and_cancellation_are_complete() {
        let fixture = natural_fixture("bound-v2-constructed-complete-private");
        let oracle = oracle_row(&fixture);
        let (constraint, surviving) = affine_constraint_and_surviving_factor(&fixture, &oracle);
        let associate_guard = fixture
            .context
            .mul(&fixture.context.integer(-2), &surviving)
            .unwrap();
        let zero_numerator = fixture
            .context
            .checked_div(&constraint, &surviving)
            .unwrap();
        let cancellation_numerator = fixture.context.add(&constraint, &surviving).unwrap();
        let cancellation = fixture
            .context
            .checked_div(&cancellation_numerator, &surviving)
            .unwrap();
        assert!(!zero_numerator.raw().denominator.is_constant());
        assert!(!cancellation.raw().denominator.is_constant());
        assert_ne!(cancellation, fixture.context.one());

        let mut translated = synthetic_relation(&fixture, "synthetic-complete-input");
        add_synthetic_guard(&fixture, &mut translated, &surviving);
        add_synthetic_guard(&fixture, &mut translated, &associate_guard);
        let synthetic_guards = translated.guarded_nonzero_conditions();
        assert_eq!(synthetic_guards.len(), 2);
        let mapped_guards = synthetic_guards
            .iter()
            .map(|guard| {
                let mapped = fixture
                    .context
                    .compose_guard_on_residual_affine_compact_composition_plan(
                        guard.polynomial(),
                        oracle.plan.as_ref(),
                        ResidualUnitAffinePolynomialCompositionLimits::default(),
                    )
                    .unwrap();
                assert!(!mapped.value().is_zero());
                mapped.value().clone()
            })
            .collect::<Vec<_>>();
        let minus_two = fixture
            .context
            .numerator_condition(&fixture.context.integer(-2))
            .unwrap();
        let expected_associate = fixture
            .context
            .multiply_polynomial_conditions_with_limits(
                &mapped_guards[0],
                &minus_two,
                ExactAlgebraLimits::default(),
            )
            .unwrap();
        assert!(!expected_associate.is_zero());
        assert_eq!(mapped_guards[1], expected_associate);
        translated
            .insert_prevalidated_distinct_term_without_denominator_discovery(
                &fixture.context,
                synthetic_shift(fixture.context.index_count(), 0),
                zero_numerator,
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        translated
            .insert_prevalidated_distinct_term_without_denominator_discovery(
                &fixture.context,
                synthetic_shift(fixture.context.index_count(), 1),
                cancellation,
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        let GeneratedAffineResidualCaseBoundRelationCompilation::Retained(certificate) =
            execute_synthetic_translated_row(&fixture, &oracle, &translated).unwrap()
        else {
            panic!("constructed complete row unexpectedly unavailable")
        };
        assert_eq!(certificate.stats().guard_composition_preflights(), 2);
        assert_eq!(certificate.stats().guard_compositions(), 2);
        assert_eq!(certificate.stats().numerator_composition_preflights(), 2);
        assert_eq!(certificate.stats().denominator_composition_preflights(), 2);
        assert_eq!(certificate.stats().numerator_compositions(), 2);
        assert_eq!(certificate.stats().denominator_compositions(), 2);
        assert!(certificate.stats().private_guard_associate_comparisons() > 0);
        assert!(
            !certificate
                .relation()
                .terms()
                .contains_key(&synthetic_shift(fixture.context.index_count(), 0))
        );
        assert_eq!(
            certificate
                .relation()
                .terms()
                .get(&synthetic_shift(fixture.context.index_count(), 1)),
            Some(&fixture.context.one())
        );
        let zero_domain_witness = certificate
            .condition_witnesses()
            .iter()
            .find(|witness| {
                witness.source()
                    == GeneratedAffineResidualCaseBoundConditionSource::TranslatedSourceTermDenominator {
                        term_ordinal: 0,
                    }
            })
            .expect("the omitted zero numerator must retain its denominator-domain witness");
        assert!(matches!(
            zero_domain_witness.class(),
            GeneratedAffineResidualCaseBoundConditionClass::PrivateFreeIndexGuard { .. }
                | GeneratedAffineResidualCaseBoundConditionClass::RowLocalBaseAssumption { .. }
                | GeneratedAffineResidualCaseBoundConditionClass::InheritedPremise { .. }
        ));
        assert_eq!(certificate.condition_witnesses().len(), 4);
    }

    #[test]
    fn constructed_zero_guard_and_denominator_are_typed_stable_and_partial_free() {
        let fixture = natural_fixture("bound-v2-constructed-unavailable-private");
        let oracle = oracle_row(&fixture);
        let (constraint, surviving) = affine_constraint_and_surviving_factor(&fixture, &oracle);

        let mut guard_zero = synthetic_relation(&fixture, "synthetic-zero-guard-input");
        add_synthetic_guard(&fixture, &mut guard_zero, &surviving);
        add_synthetic_guard(&fixture, &mut guard_zero, &constraint);
        assert_eq!(guard_zero.guarded_nonzero_conditions().len(), 2);
        let first_guard = execute_synthetic_translated_row(&fixture, &oracle, &guard_zero).unwrap();
        let second_guard =
            execute_synthetic_translated_row(&fixture, &oracle, &guard_zero).unwrap();
        let (
            GeneratedAffineResidualCaseBoundRelationCompilation::Unavailable(first_guard),
            GeneratedAffineResidualCaseBoundRelationCompilation::Unavailable(second_guard),
        ) = (first_guard, second_guard)
        else {
            panic!("affine-zero guard must be unavailable")
        };
        assert_eq!(
            first_guard.reason(),
            GeneratedAffineResidualCaseBoundUnavailableReason::TranslatedSourceGuardComposesToZero {
                guard_ordinal: 1,
            }
        );
        assert_eq!(first_guard.reason(), second_guard.reason());
        assert_eq!(first_guard.stats(), second_guard.stats());
        assert_eq!(first_guard.stats().relation_manifest_bytes(), 0);
        assert_eq!(first_guard.stats().retained_terms(), 0);
        assert!(source_payload_eq(&first_guard.source, &second_guard.source));
        let guard_debug = format!("{first_guard:?}");
        assert!(guard_debug.contains("private_partial_payload: \"<none>\""));
        assert!(!guard_debug.contains("m2"));

        let regular = fixture
            .context
            .checked_div(&fixture.context.one(), &surviving)
            .unwrap();
        let zero_denominator = fixture
            .context
            .checked_div(&fixture.context.one(), &constraint)
            .unwrap();
        assert!(!zero_denominator.raw().denominator.is_constant());
        let mut denominator_zero = synthetic_relation(&fixture, "synthetic-zero-denominator-input");
        denominator_zero
            .insert_prevalidated_distinct_term_without_denominator_discovery(
                &fixture.context,
                synthetic_shift(fixture.context.index_count(), 0),
                regular,
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        denominator_zero
            .insert_prevalidated_distinct_term_without_denominator_discovery(
                &fixture.context,
                synthetic_shift(fixture.context.index_count(), 1),
                zero_denominator,
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        let first_denominator =
            execute_synthetic_translated_row(&fixture, &oracle, &denominator_zero).unwrap();
        let second_denominator =
            execute_synthetic_translated_row(&fixture, &oracle, &denominator_zero).unwrap();
        let (
            GeneratedAffineResidualCaseBoundRelationCompilation::Unavailable(first_denominator),
            GeneratedAffineResidualCaseBoundRelationCompilation::Unavailable(second_denominator),
        ) = (first_denominator, second_denominator)
        else {
            panic!("affine-zero term denominator must be unavailable")
        };
        assert_eq!(
            first_denominator.reason(),
            GeneratedAffineResidualCaseBoundUnavailableReason::TranslatedSourceTermDenominatorComposesToZero {
                term_ordinal: 1,
            }
        );
        assert_eq!(first_denominator.reason(), second_denominator.reason());
        assert_eq!(first_denominator.stats(), second_denominator.stats());
        assert_eq!(first_denominator.stats().relation_manifest_bytes(), 0);
        assert_eq!(first_denominator.stats().retained_terms(), 0);
        assert!(source_payload_eq(
            &first_denominator.source,
            &second_denominator.source
        ));
        let denominator_debug = format!("{first_denominator:?}");
        assert!(denominator_debug.contains("private_partial_payload: \"<none>\""));
        assert!(!denominator_debug.contains("m2"));
    }

    #[test]
    fn exact_four_parent_graph_and_foreign_point_are_enforced_and_transitively_owned() {
        let fixture = natural_fixture("bound-v2-parent-graph-private");
        let GeneratedAffineResidualCaseBoundRelationCompilation::Retained(certificate) = fixture
            .compile(GeneratedAffineResidualCaseBoundRelationLimits::default())
            .unwrap()
        else {
            panic!("parent graph fixture unexpectedly unavailable")
        };
        let duplicate_authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                &fixture.family,
                &fixture.context,
                Arc::clone(&fixture.inventory),
                fixture.authority.case_ordinal(),
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        let duplicate_ordering = Arc::new(
            GeneratedAffineParametricOrderingCertificate::try_new(
                &fixture.family,
                &fixture.context,
                Arc::clone(&fixture.authority),
                GeneratedAffineParametricOrderingLimits::default(),
            )
            .unwrap(),
        );
        let duplicate_schedule = Arc::new(
            GeneratedAffinePreparePointScheduleCertificate::compile(
                &fixture.family,
                &fixture.context,
                Arc::clone(&fixture.ordering),
                &fixture.authority,
                1,
                GeneratedAffinePreparePointScheduleLimits::default(),
            )
            .unwrap(),
        );
        let duplicate_premises = Arc::new(
            match compile_generated_affine_residual_case_premises(
                &fixture.family,
                &fixture.context,
                Arc::clone(&fixture.authority),
                GeneratedAffineResidualCasePremisesLimits::default(),
            )
            .unwrap()
            {
                GeneratedAffineResidualCasePremisesOutcome::Ready(value) => value,
                GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(_) => {
                    panic!("duplicate Ready parent unexpectedly deferred")
                }
            },
        );
        let foreign_premises = Arc::new(
            match compile_generated_affine_residual_case_premises(
                &fixture.family,
                &fixture.context,
                Arc::clone(&duplicate_authority),
                GeneratedAffineResidualCasePremisesLimits::default(),
            )
            .unwrap()
            {
                GeneratedAffineResidualCasePremisesOutcome::Ready(value) => value,
                GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(_) => {
                    panic!("foreign Ready parent unexpectedly deferred")
                }
            },
        );
        assert!(!Arc::ptr_eq(&fixture.authority, &duplicate_authority));
        assert!(!Arc::ptr_eq(&fixture.ordering, &duplicate_ordering));
        assert!(!Arc::ptr_eq(&fixture.schedule, &duplicate_schedule));
        assert!(!Arc::ptr_eq(&fixture.premises, &duplicate_premises));

        let compile_with =
            |authority: Arc<GeneratedAffineResidualCaseAuthority>,
             ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
             schedule: Arc<GeneratedAffinePreparePointScheduleCertificate>,
             premises: Arc<GeneratedAffineResidualCasePremisesCertificate>,
             point: GeneratedAffinePreparePointSchedulePointHandle<'_>| {
                GeneratedAffineResidualCaseBoundRelationCompiler::compile(
                    &fixture.family,
                    &fixture.context,
                    authority,
                    ordering,
                    schedule,
                    premises,
                    fixture.source_row_ordinal,
                    point,
                    GeneratedAffineResidualCaseBoundRelationLimits::default(),
                )
            };
        assert!(matches!(
            compile_with(
                Arc::clone(&duplicate_authority),
                Arc::clone(&fixture.ordering),
                Arc::clone(&fixture.schedule),
                Arc::clone(&fixture.premises),
                fixture.schedule.point_handle(1, 0).unwrap(),
            ),
            Err(GeneratedAffineResidualCaseBoundRelationError::WrongPremiseBinding)
        ));
        assert!(matches!(
            compile_with(
                Arc::clone(&fixture.authority),
                Arc::clone(&duplicate_ordering),
                Arc::clone(&fixture.schedule),
                Arc::clone(&fixture.premises),
                fixture.schedule.point_handle(1, 0).unwrap(),
            ),
            Err(GeneratedAffineResidualCaseBoundRelationError::WrongParentAllocation)
        ));
        assert!(matches!(
            compile_with(
                Arc::clone(&fixture.authority),
                Arc::clone(&fixture.ordering),
                Arc::clone(&duplicate_schedule),
                Arc::clone(&fixture.premises),
                fixture.schedule.point_handle(1, 0).unwrap(),
            ),
            Err(GeneratedAffineResidualCaseBoundRelationError::WrongPointBinding)
        ));
        let duplicate_premises_compilation = compile_with(
            Arc::clone(&fixture.authority),
            Arc::clone(&fixture.ordering),
            Arc::clone(&fixture.schedule),
            Arc::clone(&duplicate_premises),
            fixture.schedule.point_handle(1, 0).unwrap(),
        )
        .unwrap();
        let GeneratedAffineResidualCaseBoundRelationCompilation::Retained(
            duplicate_premises_certificate,
        ) = duplicate_premises_compilation
        else {
            panic!("same-authority Ready premise allocation unexpectedly unavailable")
        };
        assert!(duplicate_premises_certificate.same_parent_allocations(
            &fixture.authority,
            &fixture.ordering,
            &fixture.schedule,
            &duplicate_premises,
        ));
        assert!(matches!(
            compile_with(
                Arc::clone(&fixture.authority),
                Arc::clone(&fixture.ordering),
                Arc::clone(&fixture.schedule),
                Arc::clone(&foreign_premises),
                fixture.schedule.point_handle(1, 0).unwrap(),
            ),
            Err(GeneratedAffineResidualCaseBoundRelationError::WrongPremiseBinding)
        ));
        assert!(matches!(
            compile_with(
                Arc::clone(&fixture.authority),
                Arc::clone(&fixture.ordering),
                Arc::clone(&fixture.schedule),
                Arc::clone(&fixture.premises),
                duplicate_schedule.point_handle(1, 0).unwrap(),
            ),
            Err(GeneratedAffineResidualCaseBoundRelationError::WrongPointBinding)
        ));
        for wrong in [
            certificate.replay(
                &fixture.family,
                &fixture.context,
                &duplicate_authority,
                &fixture.ordering,
                &fixture.schedule,
                &fixture.premises,
            ),
            certificate.replay(
                &fixture.family,
                &fixture.context,
                &fixture.authority,
                &duplicate_ordering,
                &fixture.schedule,
                &fixture.premises,
            ),
            certificate.replay(
                &fixture.family,
                &fixture.context,
                &fixture.authority,
                &fixture.ordering,
                &duplicate_schedule,
                &fixture.premises,
            ),
            certificate.replay(
                &fixture.family,
                &fixture.context,
                &fixture.authority,
                &fixture.ordering,
                &fixture.schedule,
                &duplicate_premises,
            ),
        ] {
            assert!(matches!(
                wrong,
                Err(GeneratedAffineResidualCaseBoundRelationError::WrongParentAllocation)
            ));
        }
        duplicate_premises_certificate
            .replay(
                &fixture.family,
                &fixture.context,
                &fixture.authority,
                &fixture.ordering,
                &fixture.schedule,
                &duplicate_premises,
            )
            .unwrap();
        assert!(matches!(
            duplicate_premises_certificate.replay(
                &fixture.family,
                &fixture.context,
                &fixture.authority,
                &fixture.ordering,
                &fixture.schedule,
                &fixture.premises,
            ),
            Err(GeneratedAffineResidualCaseBoundRelationError::WrongParentAllocation)
        ));
        drop(duplicate_premises_certificate);
        thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    certificate
                        .replay(
                            &fixture.family,
                            &fixture.context,
                            &fixture.authority,
                            &fixture.ordering,
                            &fixture.schedule,
                            &fixture.premises,
                        )
                        .unwrap();
                });
            }
        });

        let weak_inventory = Arc::downgrade(&fixture.inventory);
        let weak_authority = Arc::downgrade(&fixture.authority);
        let weak_ordering = Arc::downgrade(&fixture.ordering);
        let weak_schedule = Arc::downgrade(&fixture.schedule);
        let weak_premises = Arc::downgrade(&fixture.premises);
        let NaturalFixture {
            family,
            context,
            inventory,
            authority,
            ordering,
            schedule,
            premises,
            ..
        } = fixture;
        drop(inventory);
        drop(authority);
        drop(ordering);
        drop(schedule);
        drop(premises);
        assert!(weak_inventory.upgrade().is_some());
        assert!(weak_authority.upgrade().is_some());
        assert!(weak_ordering.upgrade().is_some());
        assert!(weak_schedule.upgrade().is_some());
        assert!(weak_premises.upgrade().is_some());
        let owned_authority = Arc::clone(&certificate.source.authority);
        let owned_ordering = Arc::clone(&certificate.source.ordering);
        let owned_schedule = Arc::clone(&certificate.source.schedule);
        let owned_premises = Arc::clone(&certificate.source.premises);
        certificate
            .replay(
                &family,
                &context,
                &owned_authority,
                &owned_ordering,
                &owned_schedule,
                &owned_premises,
            )
            .unwrap();
        drop(certificate);
        drop(owned_schedule);
        drop(owned_ordering);
        drop(owned_premises);
        drop(owned_authority);
        drop(duplicate_schedule);
        drop(duplicate_ordering);
        drop(duplicate_premises);
        drop(foreign_premises);
        drop(duplicate_authority);
        assert!(weak_schedule.upgrade().is_none());
        assert!(weak_ordering.upgrade().is_none());
        assert!(weak_premises.upgrade().is_none());
        assert!(weak_authority.upgrade().is_none());
        assert!(weak_inventory.upgrade().is_none());
    }

    #[test]
    fn panic_boundary_and_debug_error_payloads_are_redacted() {
        let fixture = natural_fixture("bound-v2-panic-redaction-private");
        inject_bound_relation_panic_for_test();
        assert!(matches!(
            fixture.compile(GeneratedAffineResidualCaseBoundRelationLimits::default()),
            Err(GeneratedAffineResidualCaseBoundRelationError::SymbolicaPanic)
        ));
        let compilation = fixture
            .compile(GeneratedAffineResidualCaseBoundRelationLimits::default())
            .unwrap();
        let debug = format!("{compilation:?}");
        let error = format!(
            "{:?}",
            GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
                resource: "m2-private-resource",
                requested: 13,
                limit: 12,
            }
        );
        assert!(debug.contains("<redacted>"));
        assert!(error.contains("<redacted>"));
        assert!(!debug.contains("m2"));
        assert!(!error.contains("m2"));
        assert!(!error.contains("13"));
        assert!(!error.contains("12"));
    }

    #[test]
    fn concrete_free_values_agree_with_ambient_affine_substitution_away_from_guards() {
        let fixture = natural_fixture("bound-v2-concrete-oracle-private");
        let oracle = oracle_row(&fixture);
        let GeneratedAffineResidualCaseBoundRelationCompilation::Retained(certificate) = fixture
            .compile(GeneratedAffineResidualCaseBoundRelationLimits::default())
            .unwrap()
        else {
            panic!("concrete differential fixture unexpectedly became unavailable")
        };
        let mapped_conditions = certificate
            .relation()
            .guarded_nonzero_conditions()
            .iter()
            .map(ParametricNonZeroCondition::polynomial)
            .chain(
                certificate
                    .base_assumptions()
                    .iter()
                    .map(|assumption| assumption.condition().polynomial()),
            )
            .collect::<Vec<_>>();
        let (free_assignment, ambient) = (-5..=5)
            .find_map(|seed| {
                let mut free = vec![0; fixture.context.index_count()];
                for (ordinal, &position) in oracle.free_positions.iter().enumerate() {
                    free[position] = seed + i64::try_from(ordinal).unwrap();
                }
                if mapped_conditions.iter().any(|condition| {
                    fixture
                        .context
                        .specialize_polynomial(
                            condition,
                            &free,
                            ParametricArithmeticLimits::default(),
                        )
                        .unwrap()
                        .is_zero()
                }) {
                    return None;
                }
                let ambient = ambient_assignment(&oracle, &free);
                if oracle.translated.terms().values().any(|coefficient| {
                    fixture
                        .context
                        .specialize(coefficient, &ambient, ParametricArithmeticLimits::default())
                        .is_err()
                }) {
                    None
                } else {
                    Some((free, ambient))
                }
            })
            .expect("a small free point away from every retained guard should exist");

        for (shift, coefficient) in oracle.translated.terms() {
            let ResidualAffineCoefficientComposition::Available(mapped) = fixture
                .context
                .compose_coefficient_on_residual_affine_compact_composition_plan(
                    coefficient,
                    &oracle.plan,
                    ResidualUnitAffinePolynomialCompositionLimits::default(),
                )
                .unwrap()
            else {
                panic!("concrete retained fixture has a zero mapped denominator")
            };
            let (mapped_value, _, _) = mapped.into_parts();
            let source_value = fixture
                .context
                .specialize(coefficient, &ambient, ParametricArithmeticLimits::default())
                .unwrap()
                .value;
            let oracle_value = fixture
                .context
                .specialize(
                    &mapped_value,
                    &free_assignment,
                    ParametricArithmeticLimits::default(),
                )
                .unwrap()
                .value;
            assert_eq!(source_value, oracle_value);
            if mapped_value.is_zero() {
                assert!(!certificate.relation().terms().contains_key(shift));
            } else {
                let retained_value = fixture
                    .context
                    .specialize(
                        &certificate.relation().terms()[shift],
                        &free_assignment,
                        ParametricArithmeticLimits::default(),
                    )
                    .unwrap()
                    .value;
                assert_eq!(retained_value, source_value);
            }
        }
    }

    #[test]
    fn natural_nonzero_point_compiles_replays_and_keeps_parent_graph_alive() {
        let fixture = natural_fixture("bound-v2-natural-smoke-private");
        let weak_inventory: Weak<GeneratedAffineResidualCaseInventoryCertificate> =
            Arc::downgrade(&fixture.inventory);
        let compilation = fixture
            .compile(GeneratedAffineResidualCaseBoundRelationLimits::default())
            .unwrap();
        let GeneratedAffineResidualCaseBoundRelationCompilation::Retained(certificate) =
            compilation
        else {
            panic!("natural retained fixture unexpectedly became unavailable")
        };
        assert_eq!(certificate.point_depth(), 1);
        assert!(certificate.stats().translation_components() > 0);
        assert_eq!(
            certificate.stats().guard_composition_preflights(),
            certificate.stats().guard_compositions()
        );
        assert_eq!(
            certificate.stats().coefficient_composition_preflights(),
            certificate.stats().coefficient_compositions()
        );
        certificate
            .replay(
                &fixture.family,
                &fixture.context,
                &fixture.authority,
                &fixture.ordering,
                &fixture.schedule,
                &fixture.premises,
            )
            .unwrap();
        drop(fixture.inventory);
        assert!(weak_inventory.upgrade().is_some());
    }
}
