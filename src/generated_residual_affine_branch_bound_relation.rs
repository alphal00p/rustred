//! Generated identities restricted to one complete residual-affine branch.
//!
//! A private key `q` in a retained row means `J(F(t)+q)`.  Consequently the
//! row is never exposed as a global `K(n)` identity.  Sources are selected
//! only by ordinal from the generated row span owned by the branch's discovery
//! certificate; callers cannot inject an arbitrary relation.

use std::fmt;
use std::fmt::Write as _;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::parametric_coefficient::ResidualAffineCoefficientComposition;
use crate::{
    ConcreteRelation, GeneratedSymbolicRowSpanCertificate, GuardOrigin, IndexShift, IntegralFamily,
    ParametricArithmeticLimits, ParametricCoefficientContext, ParametricCoefficientError,
    ParametricNonZeroCondition, ParametricRelation, ParametricRelationError, ParametricRowId,
    ResidualAffineBranchGuardCompositionCertificate, ResidualAffineBranchGuardCompositionClass,
    ResidualAffineBranchGuardCompositionError, ResidualAffineBranchSystemCertificate,
    ResidualAffineBranchSystemError, ResidualAffineBranchSystemOutcome,
    ResidualUnitAffineCompositionError, ResidualUnitAffineCompositionPlanLimits,
    ResidualUnitAffinePolynomialCompositionLimits, ResidualUnitAffinePolynomialCompositionStats,
};

pub const GENERATED_RESIDUAL_AFFINE_BRANCH_BOUND_RELATION_V1_SCHEMA: &str =
    "rustred-generated-residual-affine-branch-bound-relation-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedResidualAffineBranchBoundRelationLimits {
    pub translation: ParametricArithmeticLimits,
    pub composition_plan: ResidualUnitAffineCompositionPlanLimits,
    pub polynomial_composition: ResidualUnitAffinePolynomialCompositionLimits,
    pub max_row_span_rows: usize,
    pub max_translation_components: usize,
    pub max_target_row_label_bytes: usize,
    pub max_source_terms: usize,
    pub max_source_guards: usize,
    pub max_translated_terms: usize,
    pub max_translated_guards: usize,
    pub max_translation_polynomials: usize,
    pub max_total_translation_source_term_allowance: usize,
    pub max_total_translation_output_term_allowance: usize,
    pub max_total_translation_power_operation_allowance: usize,
    pub max_total_translation_integer_bit_allowance: usize,
    pub max_total_translation_normalization_input_term_pairs: usize,
    pub max_total_translation_retained_output_terms: usize,
    pub max_total_translation_retained_output_bytes: usize,
    pub max_branch_guard_entries: usize,
    pub max_polynomial_compositions: usize,
    pub max_total_source_terms: usize,
    pub max_total_source_exponent_entries: usize,
    pub max_total_expanded_contributions: usize,
    pub max_total_output_term_bound: usize,
    pub max_total_output_terms: usize,
    pub max_total_output_exponent_entry_bound: usize,
    pub max_total_output_exponent_entries: usize,
    pub max_total_power_calls: usize,
    pub max_total_native_power_heap_pairs: usize,
    pub max_total_multiplication_term_pairs: usize,
    pub max_total_addition_term_visits: usize,
    pub max_total_native_integer_bit_work: usize,
    pub max_total_integer_bit_work: usize,
    pub max_total_normalization_input_term_pairs: usize,
    pub max_total_durable_denominator_terms: usize,
    pub max_total_durable_denominator_exponent_entries: usize,
    pub max_total_durable_denominator_integer_bits: usize,
    pub max_total_guard_origin_copy_bytes: usize,
    pub max_total_retained_guard_origin_bytes: usize,
    pub max_row_local_base_assumptions: usize,
    pub max_private_free_index_guards: usize,
    pub max_condition_witnesses: usize,
    pub max_relation_manifest_bytes: usize,
    pub max_retained_terms: usize,
    pub max_retained_bytes: usize,
}

impl Default for GeneratedResidualAffineBranchBoundRelationLimits {
    fn default() -> Self {
        Self {
            translation: ParametricArithmeticLimits::default(),
            composition_plan: ResidualUnitAffineCompositionPlanLimits::default(),
            polynomial_composition: ResidualUnitAffinePolynomialCompositionLimits::default(),
            max_row_span_rows: 1_000_000,
            max_translation_components: 1_000_000,
            max_target_row_label_bytes: 1024 * 1024,
            max_source_terms: 4_000_000,
            max_source_guards: 4_000_000,
            max_translated_terms: 4_000_000,
            max_translated_guards: 8_000_000,
            max_translation_polynomials: 12_000_000,
            max_total_translation_source_term_allowance: 16_000_000_000,
            max_total_translation_output_term_allowance: 32_000_000_000,
            max_total_translation_power_operation_allowance: 64_000_000_000,
            max_total_translation_integer_bit_allowance: 64_000_000_000,
            max_total_translation_normalization_input_term_pairs: 128_000_000,
            max_total_translation_retained_output_terms: 64_000_000,
            max_total_translation_retained_output_bytes: 16 * 1024 * 1024 * 1024,
            max_branch_guard_entries: 64_000_000,
            max_polynomial_compositions: 12_000_000,
            max_total_source_terms: 32_000_000,
            max_total_source_exponent_entries: 2_147_483_648,
            max_total_expanded_contributions: 32_000_000,
            max_total_output_term_bound: 32_000_000,
            max_total_output_terms: 32_000_000,
            max_total_output_exponent_entry_bound: 2_147_483_648,
            max_total_output_exponent_entries: 2_147_483_648,
            max_total_power_calls: 2_147_483_648,
            max_total_native_power_heap_pairs: 4_294_967_296,
            max_total_multiplication_term_pairs: 4_294_967_296,
            max_total_addition_term_visits: 4_294_967_296,
            max_total_native_integer_bit_work: 16_000_000_000_000,
            max_total_integer_bit_work: 16_000_000_000_000,
            max_total_normalization_input_term_pairs: 128_000_000,
            max_total_durable_denominator_terms: 32_000_000,
            max_total_durable_denominator_exponent_entries: 2_147_483_648,
            max_total_durable_denominator_integer_bits: 8_000_000_000_000,
            max_total_guard_origin_copy_bytes: 16 * 1024 * 1024 * 1024,
            max_total_retained_guard_origin_bytes: 8 * 1024 * 1024 * 1024,
            max_row_local_base_assumptions: 8_000_000,
            max_private_free_index_guards: 8_000_000,
            max_condition_witnesses: 12_000_000,
            max_relation_manifest_bytes: 2 * 1024 * 1024 * 1024,
            max_retained_terms: 64_000_000,
            max_retained_bytes: 4 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedResidualAffineBranchBoundRelationStats {
    row_span_rows: usize,
    source_terms: usize,
    source_guards: usize,
    translated_terms: usize,
    translated_guards: usize,
    translation_components: usize,
    target_row_label_bytes: usize,
    translation_polynomials: usize,
    translation_source_terms: usize,
    translation_output_term_bound: usize,
    translation_power_operation_bound: usize,
    translation_integer_bit_work_bound: usize,
    translation_normalization_input_term_pairs: usize,
    translation_retained_output_terms: usize,
    translation_retained_output_bytes: usize,
    branch_guard_entries: usize,
    branch_contradictions: usize,
    polynomial_compositions: usize,
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
    total_native_integer_bit_work: usize,
    total_integer_bit_work: usize,
    total_normalization_input_term_pairs: usize,
    total_durable_denominator_terms: usize,
    total_durable_denominator_exponent_entries: usize,
    total_durable_denominator_integer_bits: usize,
    guard_origin_copy_bytes: usize,
    retained_guard_origin_bytes: usize,
    row_local_base_assumptions: usize,
    private_free_index_guards: usize,
    condition_witnesses: usize,
    relation_manifest_bytes: usize,
    retained_terms: usize,
    retained_bytes: usize,
}

macro_rules! branch_bound_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedResidualAffineBranchBoundRelationStats {
    branch_bound_stats_getters!(
        row_span_rows,
        source_terms,
        source_guards,
        translated_terms,
        translated_guards,
        translation_components,
        target_row_label_bytes,
        translation_polynomials,
        translation_source_terms,
        translation_output_term_bound,
        translation_power_operation_bound,
        translation_integer_bit_work_bound,
        translation_normalization_input_term_pairs,
        translation_retained_output_terms,
        translation_retained_output_bytes,
        branch_guard_entries,
        branch_contradictions,
        polynomial_compositions,
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
        total_native_integer_bit_work,
        total_integer_bit_work,
        total_normalization_input_term_pairs,
        total_durable_denominator_terms,
        total_durable_denominator_exponent_entries,
        total_durable_denominator_integer_bits,
        guard_origin_copy_bytes,
        retained_guard_origin_bytes,
        row_local_base_assumptions,
        private_free_index_guards,
        condition_witnesses,
        relation_manifest_bytes,
        retained_terms,
        retained_bytes,
    );
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedResidualAffineBranchBaseAssumption {
    condition: ParametricNonZeroCondition,
}

impl GeneratedResidualAffineBranchBaseAssumption {
    pub fn condition(&self) -> &ParametricNonZeroCondition {
        &self.condition
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedResidualAffineBranchBoundConditionSource {
    TranslatedSourceGuard {
        guard_ordinal: usize,
    },
    TranslatedSourceTermDenominator {
        term_ordinal: usize,
        translated_shift: IndexShift,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedResidualAffineBranchBoundConditionClass {
    DischargedNonzeroIntegerConstant,
    RowLocalBaseAssumption { ordinal: usize },
    PrivateFreeIndexGuard { ordinal: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedResidualAffineBranchBoundConditionWitness {
    source: GeneratedResidualAffineBranchBoundConditionSource,
    class: GeneratedResidualAffineBranchBoundConditionClass,
}

impl GeneratedResidualAffineBranchBoundConditionWitness {
    pub const fn source(&self) -> &GeneratedResidualAffineBranchBoundConditionSource {
        &self.source
    }
    pub const fn class(&self) -> &GeneratedResidualAffineBranchBoundConditionClass {
        &self.class
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedResidualAffineBranchEmptyReason {
    NonzeroGuardContradiction {
        entry_ordinal: usize,
        structural_locus_ordinal: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedResidualAffineBranchUnavailableReason {
    TranslatedSourceGuardComposesToZero {
        guard_ordinal: usize,
    },
    TranslatedSourceTermDenominatorComposesToZero {
        term_ordinal: usize,
        translated_shift: IndexShift,
    },
}

#[derive(Clone)]
struct GeneratedResidualAffineBranchBoundSource {
    row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    source_row_ordinal: usize,
    translation: IndexShift,
    target_row_id: ParametricRowId,
    branch: Arc<ResidualAffineBranchSystemCertificate>,
    branch_guards: Arc<ResidualAffineBranchGuardCompositionCertificate>,
}

impl fmt::Debug for GeneratedResidualAffineBranchBoundSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineBranchBoundSource")
            .field("source_row_ordinal", &self.source_row_ordinal)
            .field("translation", &self.translation)
            .field("target_row_id", &self.target_row_id)
            .field(
                "source_case",
                &self.branch.source_cover().source_case().value(),
            )
            .field(
                "source_work_item_ordinal",
                &self.branch.source_cover().source_work_item_ordinal(),
            )
            .field(
                "ready_terminal_ordinal",
                &self.branch.ready_terminal_ordinal(),
            )
            .finish_non_exhaustive()
    }
}

macro_rules! source_accessors {
    () => {
        pub fn row_span(&self) -> &Arc<GeneratedSymbolicRowSpanCertificate> {
            &self.source.row_span
        }
        pub const fn source_row_ordinal(&self) -> usize {
            self.source.source_row_ordinal
        }
        pub const fn translation(&self) -> &IndexShift {
            &self.source.translation
        }
        pub const fn target_row_id(&self) -> &ParametricRowId {
            &self.source.target_row_id
        }
        pub fn branch(&self) -> &Arc<ResidualAffineBranchSystemCertificate> {
            &self.source.branch
        }
        pub fn branch_guards(&self) -> &Arc<ResidualAffineBranchGuardCompositionCertificate> {
            &self.source.branch_guards
        }
    };
}

#[derive(Clone, Debug)]
pub struct GeneratedResidualAffineBranchEmptyCertificate {
    schema: &'static str,
    source: GeneratedResidualAffineBranchBoundSource,
    reason: GeneratedResidualAffineBranchEmptyReason,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
    stats: GeneratedResidualAffineBranchBoundRelationStats,
}

impl GeneratedResidualAffineBranchEmptyCertificate {
    source_accessors!();
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub const fn reason(&self) -> &GeneratedResidualAffineBranchEmptyReason {
        &self.reason
    }
    pub const fn limits(&self) -> GeneratedResidualAffineBranchBoundRelationLimits {
        self.limits
    }
    pub const fn stats(&self) -> GeneratedResidualAffineBranchBoundRelationStats {
        self.stats
    }
    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
        validate_replay_schema(self.schema)?;
        replay_expected(
            family,
            context,
            &self.source,
            self.limits,
            |value| match value {
                GeneratedResidualAffineBranchBoundRelationCompilation::EmptyBranch(other) => {
                    empty_payload_eq(self, &other)
                }
                _ => false,
            },
        )
    }
}

#[derive(Clone)]
pub struct GeneratedResidualAffineBranchUnavailableRowCertificate {
    schema: &'static str,
    source: GeneratedResidualAffineBranchBoundSource,
    reason: GeneratedResidualAffineBranchUnavailableReason,
    partial_relation: Arc<ParametricRelation>,
    base_assumptions: Vec<GeneratedResidualAffineBranchBaseAssumption>,
    condition_witnesses: Vec<GeneratedResidualAffineBranchBoundConditionWitness>,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
    stats: GeneratedResidualAffineBranchBoundRelationStats,
}

impl GeneratedResidualAffineBranchUnavailableRowCertificate {
    source_accessors!();
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub const fn reason(&self) -> &GeneratedResidualAffineBranchUnavailableReason {
        &self.reason
    }
    pub fn condition_witnesses(&self) -> &[GeneratedResidualAffineBranchBoundConditionWitness] {
        &self.condition_witnesses
    }
    pub fn base_assumptions(&self) -> &[GeneratedResidualAffineBranchBaseAssumption] {
        &self.base_assumptions
    }
    pub fn private_free_index_guards(&self) -> &[ParametricNonZeroCondition] {
        self.partial_relation.guarded_nonzero_conditions()
    }
    pub const fn limits(&self) -> GeneratedResidualAffineBranchBoundRelationLimits {
        self.limits
    }
    pub const fn stats(&self) -> GeneratedResidualAffineBranchBoundRelationStats {
        self.stats
    }
    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
        validate_replay_schema(self.schema)?;
        replay_expected(
            family,
            context,
            &self.source,
            self.limits,
            |value| match value {
                GeneratedResidualAffineBranchBoundRelationCompilation::UnavailableRow(other) => {
                    unavailable_payload_eq(self, &other)
                }
                _ => false,
            },
        )
    }
}

impl fmt::Debug for GeneratedResidualAffineBranchUnavailableRowCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineBranchUnavailableRowCertificate")
            .field("schema", &self.schema)
            .field("source", &self.source)
            .field("reason", &self.reason)
            .field("base_assumptions", &self.base_assumptions)
            .field(
                "private_free_index_guard_count",
                &self.partial_relation.guarded_nonzero_conditions().len(),
            )
            .field("condition_witnesses", &self.condition_witnesses)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct GeneratedResidualAffineBranchBoundParametricRelation {
    schema: &'static str,
    source: GeneratedResidualAffineBranchBoundSource,
    relation: Arc<ParametricRelation>,
    relation_manifest: Arc<String>,
    base_assumptions: Vec<GeneratedResidualAffineBranchBaseAssumption>,
    condition_witnesses: Vec<GeneratedResidualAffineBranchBoundConditionWitness>,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
    stats: GeneratedResidualAffineBranchBoundRelationStats,
}

impl fmt::Debug for GeneratedResidualAffineBranchBoundParametricRelation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineBranchBoundParametricRelation")
            .field("schema", &self.schema)
            .field("source_row_ordinal", &self.source.source_row_ordinal)
            .field("translation", &self.source.translation)
            .field("target_row_id", &self.source.target_row_id)
            .field("base_assumptions", &self.base_assumptions)
            .field("condition_witnesses", &self.condition_witnesses)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl GeneratedResidualAffineBranchBoundParametricRelation {
    source_accessors!();
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn relation_manifest(&self) -> &str {
        self.relation_manifest.as_str()
    }
    pub fn base_assumptions(&self) -> &[GeneratedResidualAffineBranchBaseAssumption] {
        &self.base_assumptions
    }
    pub fn condition_witnesses(&self) -> &[GeneratedResidualAffineBranchBoundConditionWitness] {
        &self.condition_witnesses
    }
    pub const fn limits(&self) -> GeneratedResidualAffineBranchBoundRelationLimits {
        self.limits
    }
    pub const fn stats(&self) -> GeneratedResidualAffineBranchBoundRelationStats {
        self.stats
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
        validate_replay_schema(self.schema)?;
        replay_expected(
            family,
            context,
            &self.source,
            self.limits,
            |value| match value {
                GeneratedResidualAffineBranchBoundRelationCompilation::Retained(other) => {
                    retained_payload_eq(self, &other)
                }
                _ => false,
            },
        )
    }

    pub fn specialize_at_free_values(
        &self,
        context: &ParametricCoefficientContext,
        free_values: &[i64],
        limits: GeneratedResidualAffineBranchConcreteSpecializationLimits,
    ) -> Result<ConcreteRelation, GeneratedResidualAffineBranchBoundRelationError> {
        catch_unwind(AssertUnwindSafe(|| {
            if context.fingerprint() != self.source.branch.context_fingerprint() {
                return Err(GeneratedResidualAffineBranchBoundRelationError::WrongContext);
            }
            let map = self
                .source
                .branch
                .affine_map()
                .ok_or(GeneratedResidualAffineBranchBoundRelationError::MissingIntegerSystem)?;
            check_limit(
                "concrete free positions",
                map.free_positions().len(),
                limits.max_free_positions,
            )?;
            if free_values.len() != map.free_positions().len() {
                return Err(
                    GeneratedResidualAffineBranchBoundRelationError::ConcreteFreeValueArity {
                        expected: map.free_positions().len(),
                        actual: free_values.len(),
                    },
                );
            }
            check_limit(
                "concrete ambient positions",
                map.ambient_arity(),
                limits.max_ambient_positions,
            )?;
            let ambient = evaluate_affine_point(map, free_values, limits)?;
            if !self
                .source
                .branch
                .matches_original_boolean_terminal_for_indices(context, &ambient)?
            {
                return Err(
                    GeneratedResidualAffineBranchBoundRelationError::ConcretePointOutsideBranch,
                );
            }
            let common_conditions = self
                .source
                .branch_guards
                .entries()
                .iter()
                .filter(|entry| entry.class().condition().is_some())
                .count();
            let query_guard_bound = checked_add(
                "concrete query clone guards",
                checked_add(
                    "concrete query clone guards",
                    checked_add(
                        "concrete query clone guards",
                        self.relation.guarded_nonzero_conditions().len(),
                        self.relation.terms().len(),
                    )?,
                    self.base_assumptions.len(),
                )?,
                common_conditions,
            )?;
            check_limit(
                "concrete query clone guards",
                query_guard_bound,
                limits.max_query_clone_guards,
            )?;
            let query_preflight = preflight_query_specialization(
                context,
                &self.relation,
                self.source
                    .branch_guards
                    .entries()
                    .iter()
                    .filter_map(|entry| entry.class().condition())
                    .chain(self.base_assumptions.iter().map(|entry| &entry.condition)),
                &ambient,
                limits,
            )?;
            debug_assert!(query_preflight.retained_terms <= limits.max_query_clone_terms);
            debug_assert!(query_preflight.retained_bytes <= limits.max_query_clone_bytes);
            Ok(self
                .relation
                .specialize_with_additional_nonzero_conditions(
                    context,
                    &ambient,
                    self.source
                        .branch_guards
                        .entries()
                        .iter()
                        .filter_map(|entry| entry.class().condition())
                        .chain(self.base_assumptions.iter().map(|entry| &entry.condition)),
                    limits.arithmetic,
                )?)
        }))
        .map_err(|_| GeneratedResidualAffineBranchBoundRelationError::SymbolicaPanic)?
    }

    pub(crate) fn relation_for_branch_bound_reelimination(&self) -> &Arc<ParametricRelation> {
        &self.relation
    }
}

#[derive(Clone, Debug)]
pub enum GeneratedResidualAffineBranchBoundRelationCompilation {
    Retained(GeneratedResidualAffineBranchBoundParametricRelation),
    EmptyBranch(GeneratedResidualAffineBranchEmptyCertificate),
    UnavailableRow(GeneratedResidualAffineBranchUnavailableRowCertificate),
}

pub struct GeneratedResidualAffineBranchBoundRelationCompiler;

impl GeneratedResidualAffineBranchBoundRelationCompiler {
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_row_ordinal: usize,
        translation: IndexShift,
        branch: Arc<ResidualAffineBranchSystemCertificate>,
        branch_guards: Arc<ResidualAffineBranchGuardCompositionCertificate>,
        limits: GeneratedResidualAffineBranchBoundRelationLimits,
    ) -> Result<
        GeneratedResidualAffineBranchBoundRelationCompilation,
        GeneratedResidualAffineBranchBoundRelationError,
    > {
        // Construction checks every source seam and execution/preflight
        // invariant in one bounded pass. Full deterministic reconstruction is
        // available through each outcome's explicit `replay`; doing it
        // unconditionally here would silently double every advertised
        // per-call work and peak-retention budget.
        compile_caught(
            family,
            context,
            source_row_ordinal,
            translation,
            branch,
            branch_guards,
            limits,
            true,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedResidualAffineBranchConcreteSpecializationLimits {
    pub arithmetic: ParametricArithmeticLimits,
    pub max_free_positions: usize,
    pub max_ambient_positions: usize,
    pub max_affine_integer_bits: usize,
    pub max_query_clone_terms: usize,
    pub max_query_clone_guards: usize,
    pub max_query_clone_bytes: usize,
    pub max_query_source_terms: usize,
    pub max_query_output_terms: usize,
    pub max_query_power_operations: usize,
    pub max_query_integer_bit_work: usize,
    pub max_query_normalization_input_term_pairs: usize,
}

impl Default for GeneratedResidualAffineBranchConcreteSpecializationLimits {
    fn default() -> Self {
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            max_free_positions: 4096,
            max_ambient_positions: 8192,
            max_affine_integer_bits: 1_000_000,
            max_query_clone_terms: 64_000_000,
            max_query_clone_guards: 16_000_000,
            max_query_clone_bytes: 4 * 1024 * 1024 * 1024,
            max_query_source_terms: 64_000_000,
            max_query_output_terms: 128_000_000,
            max_query_power_operations: 2_147_483_648,
            max_query_integer_bit_work: 16_000_000_000_000,
            max_query_normalization_input_term_pairs: 128_000_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedResidualAffineBranchBoundRelationError {
    SchemaMismatch,
    ReplayMismatch,
    WrongFamily,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    BranchGuardSourceBranchAllocationMismatch,
    BranchGuardSourceCoverAllocationMismatch,
    BranchOutcomeNotGuardedAffineMap,
    MissingIntegerSystem,
    SourceRowOrdinalOutOfRange {
        ordinal: usize,
        rows: usize,
    },
    ConcreteFreeValueArity {
        expected: usize,
        actual: usize,
    },
    ConcreteAffineValueOutOfRange {
        position: usize,
    },
    ConcretePointOutsideBranch,
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    SymbolicaPanic,
    BranchGuards(ResidualAffineBranchGuardCompositionError),
    Branch(ResidualAffineBranchSystemError),
    Composition(ResidualUnitAffineCompositionError),
    Coefficient(ParametricCoefficientError),
    Relation(ParametricRelationError),
}

impl fmt::Display for GeneratedResidualAffineBranchBoundRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for GeneratedResidualAffineBranchBoundRelationError {}
impl From<ResidualAffineBranchGuardCompositionError>
    for GeneratedResidualAffineBranchBoundRelationError
{
    fn from(value: ResidualAffineBranchGuardCompositionError) -> Self {
        Self::BranchGuards(value)
    }
}
impl From<ResidualAffineBranchSystemError> for GeneratedResidualAffineBranchBoundRelationError {
    fn from(value: ResidualAffineBranchSystemError) -> Self {
        Self::Branch(value)
    }
}
impl From<ResidualUnitAffineCompositionError> for GeneratedResidualAffineBranchBoundRelationError {
    fn from(value: ResidualUnitAffineCompositionError) -> Self {
        Self::Composition(value)
    }
}
impl From<ParametricCoefficientError> for GeneratedResidualAffineBranchBoundRelationError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::Coefficient(value)
    }
}
impl From<ParametricRelationError> for GeneratedResidualAffineBranchBoundRelationError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

#[allow(clippy::too_many_arguments)]
fn compile_caught(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source_row_ordinal: usize,
    translation: IndexShift,
    branch: Arc<ResidualAffineBranchSystemCertificate>,
    branch_guards: Arc<ResidualAffineBranchGuardCompositionCertificate>,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
    replay_sources: bool,
) -> Result<
    GeneratedResidualAffineBranchBoundRelationCompilation,
    GeneratedResidualAffineBranchBoundRelationError,
> {
    catch_unwind(AssertUnwindSafe(|| {
        validate_fresh_scope(family, context, &translation, &branch, &branch_guards)?;
        preflight_source_shape(
            family,
            context,
            source_row_ordinal,
            &translation,
            &branch,
            &branch_guards,
            limits,
        )?;
        if replay_sources {
            branch_guards.replay(family, context)?;
        }
        compile_inner(
            family,
            context,
            source_row_ordinal,
            translation,
            branch,
            branch_guards,
            limits,
        )
    }))
    .map_err(|_| GeneratedResidualAffineBranchBoundRelationError::SymbolicaPanic)?
}

fn validate_fresh_scope(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    translation: &IndexShift,
    branch: &Arc<ResidualAffineBranchSystemCertificate>,
    branch_guards: &Arc<ResidualAffineBranchGuardCompositionCertificate>,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    if family.fingerprint_ref() != branch.family_fingerprint()
        || family.fingerprint_ref() != branch_guards.family_fingerprint()
    {
        return Err(GeneratedResidualAffineBranchBoundRelationError::WrongFamily);
    }
    if context.fingerprint() != branch.context_fingerprint()
        || context.fingerprint() != branch_guards.context_fingerprint()
    {
        return Err(GeneratedResidualAffineBranchBoundRelationError::WrongContext);
    }
    if translation.arity() != context.index_count() {
        return Err(
            GeneratedResidualAffineBranchBoundRelationError::WrongArity {
                expected: context.index_count(),
                actual: translation.arity(),
            },
        );
    }
    if !Arc::ptr_eq(branch_guards.source_branch(), branch) {
        return Err(GeneratedResidualAffineBranchBoundRelationError::BranchGuardSourceBranchAllocationMismatch);
    }
    if !Arc::ptr_eq(branch_guards.source_cover(), branch.source_cover()) {
        return Err(GeneratedResidualAffineBranchBoundRelationError::BranchGuardSourceCoverAllocationMismatch);
    }
    if !matches!(
        branch.outcome(),
        ResidualAffineBranchSystemOutcome::GuardedAffineMap
    ) {
        return Err(
            GeneratedResidualAffineBranchBoundRelationError::BranchOutcomeNotGuardedAffineMap,
        );
    }
    if branch.integer_system_arc().is_none() {
        return Err(GeneratedResidualAffineBranchBoundRelationError::MissingIntegerSystem);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn preflight_source_shape(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source_row_ordinal: usize,
    translation: &IndexShift,
    branch: &ResidualAffineBranchSystemCertificate,
    branch_guards: &ResidualAffineBranchGuardCompositionCertificate,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    let row_span = branch
        .source_cover()
        .source_queue()
        .discovery()
        .row_span_arc();
    check_limit(
        "row-span rows",
        row_span.rows().len(),
        limits.max_row_span_rows,
    )?;
    let source = row_span.rows().get(source_row_ordinal).ok_or(
        GeneratedResidualAffineBranchBoundRelationError::SourceRowOrdinalOutOfRange {
            ordinal: source_row_ordinal,
            rows: row_span.rows().len(),
        },
    )?;
    if source.family_fingerprint() != family.fingerprint_ref() {
        return Err(GeneratedResidualAffineBranchBoundRelationError::WrongFamily);
    }
    if source.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedResidualAffineBranchBoundRelationError::WrongContext);
    }
    if source.arity() != context.index_count() {
        return Err(
            GeneratedResidualAffineBranchBoundRelationError::WrongArity {
                expected: context.index_count(),
                actual: source.arity(),
            },
        );
    }
    check_limit(
        "translation components",
        translation.arity(),
        limits.max_translation_components,
    )?;
    let source_terms = source.terms().len();
    let source_guards = source.guarded_nonzero_conditions().len();
    check_limit("source terms", source_terms, limits.max_source_terms)?;
    check_limit("source guards", source_guards, limits.max_source_guards)?;
    check_limit(
        "branch guard entries",
        branch_guards.entries().len(),
        limits.max_branch_guard_entries,
    )?;
    derived_target_row_label_len(
        branch,
        source_row_ordinal,
        translation,
        limits.max_target_row_label_bytes,
    )?;

    // A uniform key translation is injective. It therefore retains at most
    // one term per source term and can add at most one input-denominator
    // condition per term.
    check_limit(
        "translated terms",
        source_terms,
        limits.max_translated_terms,
    )?;
    let translated_guard_upper_bound =
        checked_add("translated guards", source_guards, source_terms)?;
    check_limit(
        "translated guards",
        translated_guard_upper_bound,
        limits.max_translated_guards,
    )?;
    let composition_upper_bound = checked_add(
        "polynomial compositions",
        checked_mul("polynomial compositions", source_terms, 2)?,
        translated_guard_upper_bound,
    )?;
    check_limit(
        "polynomial compositions",
        composition_upper_bound,
        limits.max_polynomial_compositions,
    )?;

    let translation_polynomials = checked_add(
        "translation polynomials",
        checked_mul("translation polynomials", source_terms, 2)?,
        source_guards,
    )?;
    check_limit(
        "translation polynomials",
        translation_polynomials,
        limits.max_translation_polynomials,
    )?;
    // Exact row-wide translation work is computed by the allocation-free
    // polynomial/coefficient preflight immediately before source translation.
    // Multiplying the number of polynomials by a per-call maximum is neither
    // an exact work bound nor a useful admission criterion.

    // Complete-row translation can merge equal guard polynomials. Bound the
    // worst provenance union before any translated condition/term allocates
    // its owned origin payload.
    let source_origin_count =
        source
            .guarded_nonzero_conditions()
            .iter()
            .try_fold(0usize, |total, condition| {
                checked_add("translated guard origins", total, condition.origins().len())
            })?;
    let translated_origin_upper_bound = checked_add(
        "translated guard origins",
        checked_add(
            "translated guard origins",
            source_origin_count,
            usize::from(source_guards != 0).checked_mul(2).ok_or(
                GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                    resource: "translated guard origins",
                },
            )?,
        )?,
        checked_add(
            "translated guard origins",
            source_terms,
            usize::from(source_guards != 0 || source_terms != 0),
        )?,
    )?;
    check_limit(
        "translated guard origins",
        translated_origin_upper_bound,
        limits.translation.max_guard_origins,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn compile_inner(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source_row_ordinal: usize,
    translation: IndexShift,
    branch: Arc<ResidualAffineBranchSystemCertificate>,
    branch_guards: Arc<ResidualAffineBranchGuardCompositionCertificate>,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
) -> Result<
    GeneratedResidualAffineBranchBoundRelationCompilation,
    GeneratedResidualAffineBranchBoundRelationError,
> {
    let row_span = branch
        .source_cover()
        .source_queue()
        .discovery()
        .row_span_arc()
        .clone();
    check_limit(
        "row-span rows",
        row_span.rows().len(),
        limits.max_row_span_rows,
    )?;
    let source = row_span.rows().get(source_row_ordinal).ok_or(
        GeneratedResidualAffineBranchBoundRelationError::SourceRowOrdinalOutOfRange {
            ordinal: source_row_ordinal,
            rows: row_span.rows().len(),
        },
    )?;
    if source.family_fingerprint() != family.fingerprint_ref() {
        return Err(GeneratedResidualAffineBranchBoundRelationError::WrongFamily);
    }
    if source.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedResidualAffineBranchBoundRelationError::WrongContext);
    }
    if source.arity() != context.index_count() {
        return Err(
            GeneratedResidualAffineBranchBoundRelationError::WrongArity {
                expected: context.index_count(),
                actual: source.arity(),
            },
        );
    }
    check_limit(
        "translation components",
        translation.arity(),
        limits.max_translation_components,
    )?;
    check_limit(
        "source terms",
        source.terms().len(),
        limits.max_source_terms,
    )?;
    check_limit(
        "source guards",
        source.guarded_nonzero_conditions().len(),
        limits.max_source_guards,
    )?;
    check_limit(
        "branch guard entries",
        branch_guards.entries().len(),
        limits.max_branch_guard_entries,
    )?;
    let target_row_label_bytes = derived_target_row_label_len(
        &branch,
        source_row_ordinal,
        &translation,
        limits.max_target_row_label_bytes,
    )?;
    preflight_source_binding_retained_bytes(&translation, target_row_label_bytes, limits)?;
    let target_row_id = derived_target_row_id(
        &branch,
        source_row_ordinal,
        &translation,
        limits.max_target_row_label_bytes,
    )?;
    let branch_contradictions = branch_guards
        .entries()
        .iter()
        .filter(|entry| {
            matches!(
                entry.class(),
                ResidualAffineBranchGuardCompositionClass::Contradiction
            )
        })
        .count();
    let mut stats = GeneratedResidualAffineBranchBoundRelationStats {
        row_span_rows: row_span.rows().len(),
        source_terms: source.terms().len(),
        source_guards: source.guarded_nonzero_conditions().len(),
        translation_components: translation.arity(),
        target_row_label_bytes,
        branch_guard_entries: branch_guards.entries().len(),
        branch_contradictions,
        ..Default::default()
    };
    let source_binding = GeneratedResidualAffineBranchBoundSource {
        row_span: row_span.clone(),
        source_row_ordinal,
        translation: copy_shift(&translation)?,
        target_row_id: target_row_id.clone(),
        branch: branch.clone(),
        branch_guards: branch_guards.clone(),
    };
    if let Some(entry_ordinal) = branch_guards.first_contradiction_entry_ordinal() {
        let structural_locus_ordinal = branch_guards
            .entries()
            .get(entry_ordinal)
            .ok_or(GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch)?
            .structural_locus_ordinal();
        let reason = GeneratedResidualAffineBranchEmptyReason::NonzeroGuardContradiction {
            entry_ordinal,
            structural_locus_ordinal,
        };
        stats.retained_bytes = empty_retained_byte_census(&source_binding, &reason)?;
        check_limit(
            "retained bytes",
            stats.retained_bytes,
            limits.max_retained_bytes,
        )?;
        return Ok(
            GeneratedResidualAffineBranchBoundRelationCompilation::EmptyBranch(
                GeneratedResidualAffineBranchEmptyCertificate {
                    schema: GENERATED_RESIDUAL_AFFINE_BRANCH_BOUND_RELATION_V1_SCHEMA,
                    source: source_binding,
                    reason,
                    limits,
                    stats,
                },
            ),
        );
    }

    preflight_translation(
        context,
        source,
        &translation,
        &target_row_id,
        limits,
        &mut stats,
    )?;
    let translated = source.translated(
        context,
        &translation,
        target_row_id.clone(),
        limits.translation,
    )?;
    stats.translated_terms = translated.terms().len();
    stats.translated_guards = translated.guarded_nonzero_conditions().len();
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
    let exact_compositions = checked_add(
        "polynomial compositions",
        checked_mul("polynomial compositions", translated.terms().len(), 2)?,
        translated.guarded_nonzero_conditions().len(),
    )?;
    check_limit(
        "polynomial compositions",
        exact_compositions,
        limits.max_polynomial_compositions,
    )?;
    let integer_system = branch
        .integer_system_arc()
        .ok_or(GeneratedResidualAffineBranchBoundRelationError::MissingIntegerSystem)?
        .clone();
    let plan = context.compile_residual_affine_composition_plan_from_integer_system(
        integer_system.clone(),
        limits.composition_plan,
    )?;
    if !Arc::ptr_eq(plan.certificate(), &integer_system) {
        return Err(GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch);
    }
    let composition_preflight =
        preflight_complete_row_compositions(context, &translated, &plan, limits)?;
    let mut relation =
        ParametricRelation::new(source.family_fingerprint(), target_row_id.clone(), context);
    let mut base_assumptions = Vec::new();
    let mut witnesses = Vec::new();
    let locator = BranchLocator {
        source_case: branch.source_cover().source_case().value(),
        source_work_item_ordinal: branch.source_cover().source_work_item_ordinal(),
        ready_terminal_ordinal: branch.ready_terminal_ordinal(),
    };

    for (guard_ordinal, guard) in translated.guarded_nonzero_conditions().iter().enumerate() {
        let call_limits = remaining_composition_limits(limits, &stats)?;
        let mapped = context.compose_polynomial_on_residual_affine_composition_plan(
            guard.polynomial(),
            &plan,
            call_limits,
        )?;
        let (polynomial, item_stats) = mapped.into_parts();
        stats.polynomial_compositions =
            checked_add("polynomial compositions", stats.polynomial_compositions, 1)?;
        consume_polynomial_stats(&mut stats, item_stats, limits)?;
        if polynomial.is_zero() {
            return unavailable(
                source_binding,
                GeneratedResidualAffineBranchUnavailableReason::TranslatedSourceGuardComposesToZero {
                    guard_ordinal,
                },
                relation,
                base_assumptions,
                witnesses,
                limits,
                stats,
            );
        }
        let class = if polynomial.is_nonzero_constant() {
            GeneratedResidualAffineBranchBoundConditionClass::DischargedNonzeroIntegerConstant
        } else {
            let origin = GuardOrigin::RelationResidualAffineBranchSubstitution {
                source_row: source.row_id().guard_identity(),
                target_row: target_row_id.guard_identity(),
                source_case: locator.source_case,
                source_work_item_ordinal: locator.source_work_item_ordinal,
                ready_terminal_ordinal: locator.ready_terminal_ordinal,
            };
            charge_source_guard_origins(&mut stats, guard, &origin, &target_row_id, limits)?;
            let condition = context.nonzero_condition_with_origins_and_origin_limit(
                polynomial,
                guard
                    .origins()
                    .iter()
                    .cloned()
                    .chain(std::iter::once(origin)),
                limits.polynomial_composition.exact_algebra,
                limits.polynomial_composition.max_guard_origins,
            )?;
            retain_condition(
                context,
                &mut relation,
                &mut base_assumptions,
                condition,
                &mut stats,
                limits,
            )?
        };
        push_witness(
            &mut witnesses,
            &mut stats,
            GeneratedResidualAffineBranchBoundConditionSource::TranslatedSourceGuard {
                guard_ordinal,
            },
            class,
            limits,
        )?;
    }

    for (term_ordinal, (shift, coefficient)) in translated.terms().iter().enumerate() {
        let call_limits = remaining_composition_limits(limits, &stats)?;
        let mapped = context.compose_coefficient_on_residual_affine_composition_plan(
            coefficient,
            &plan,
            call_limits,
        )?;
        stats.polynomial_compositions =
            checked_add("polynomial compositions", stats.polynomial_compositions, 2)?;
        let coefficient_stats = mapped.stats();
        consume_coefficient_stats(&mut stats, coefficient_stats, limits)?;
        let ResidualAffineCoefficientComposition::Available(mapped) = mapped else {
            preflight_unavailable_term_denominator_retained_bytes(
                &source_binding,
                &relation,
                &base_assumptions,
                &witnesses,
                shift,
                limits,
            )?;
            return unavailable(
                source_binding,
                GeneratedResidualAffineBranchUnavailableReason::TranslatedSourceTermDenominatorComposesToZero {
                    term_ordinal,
                    translated_shift: copy_shift(shift)?,
                },
                relation,
                base_assumptions,
                witnesses,
                limits,
                stats,
            );
        };
        let (value, denominator, returned_stats) = mapped.into_parts();
        if returned_stats != coefficient_stats {
            return Err(GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch);
        }
        reserve_witness_slot(&mut witnesses, &mut stats, limits)?;
        let source_locator =
            GeneratedResidualAffineBranchBoundConditionSource::TranslatedSourceTermDenominator {
                term_ordinal,
                translated_shift: copy_shift(shift)?,
            };
        let class = if denominator.is_nonzero_constant() {
            GeneratedResidualAffineBranchBoundConditionClass::DischargedNonzeroIntegerConstant
        } else {
            let row = target_row_id.guard_identity();
            let term_origin_bytes =
                GuardOrigin::residual_affine_branch_term_denominator_retained_byte_bound(
                    &row,
                    shift.arity(),
                )
                .ok_or(
                    GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                        resource: "guard origin copy bytes",
                    },
                )?;
            let attached_bytes = GuardOrigin::relation_attached_retained_byte_bound(&row).ok_or(
                GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                    resource: "guard origin copy bytes",
                },
            )?;
            let retained_origin_bytes = checked_add(
                "guard origin retained bytes",
                term_origin_bytes,
                attached_bytes,
            )?;
            check_limit(
                "guard origin retained bytes",
                retained_origin_bytes,
                limits
                    .polynomial_composition
                    .max_guard_origin_retained_bytes,
            )?;
            stats.guard_origin_copy_bytes = bounded_add(
                "guard origin copy bytes",
                stats.guard_origin_copy_bytes,
                checked_mul("guard origin copy bytes", retained_origin_bytes, 2)?,
                limits.max_total_guard_origin_copy_bytes,
            )?;
            let term_origin =
                GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
                    row,
                    shift: copy_shift_payload(shift)?,
                    source_case: locator.source_case,
                    source_work_item_ordinal: locator.source_work_item_ordinal,
                    ready_terminal_ordinal: locator.ready_terminal_ordinal,
                };
            let condition = context.nonzero_condition_with_origins_and_origin_limit(
                denominator,
                [term_origin],
                limits.polynomial_composition.exact_algebra,
                limits.polynomial_composition.max_guard_origins,
            )?;
            retain_condition(
                context,
                &mut relation,
                &mut base_assumptions,
                condition,
                &mut stats,
                limits,
            )?
        };
        push_reserved_witness(&mut witnesses, source_locator, class)?;
        if !value.is_zero() {
            charge_retained_terms(
                &mut stats,
                checked_add(
                    "retained terms",
                    value.raw().numerator.nterms(),
                    value.raw().denominator.nterms(),
                )?,
                limits,
            )?;
            relation.insert_prevalidated_distinct_term_without_denominator_discovery(
                context,
                copy_shift(shift)?,
                value,
                relation_arithmetic_limits(limits),
            )?;
        }
    }

    stats.condition_witnesses = witnesses.len();
    stats.row_local_base_assumptions = base_assumptions.len();
    stats.private_free_index_guards = relation.guarded_nonzero_conditions().len();
    check_limit(
        "private free-index guards",
        stats.private_free_index_guards,
        limits.max_private_free_index_guards,
    )?;
    let exact_retained_guard_origin_bytes = retained_origin_bytes(&relation, &base_assumptions)?;
    if exact_retained_guard_origin_bytes != stats.retained_guard_origin_bytes {
        return Err(GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch);
    }
    verify_composition_execution_within_preflight(&stats, &composition_preflight)?;
    let exact_retained_terms = retained_term_census(&relation, &base_assumptions, &witnesses)?;
    if exact_retained_terms != stats.retained_terms {
        return Err(GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch);
    }
    let prospective_manifest_bytes =
        relation.stable_manifest_byte_len_with_limit(limits.max_relation_manifest_bytes)?;
    let prospective_retained_bytes = retained_byte_census_with_manifest_capacity(
        &source_binding,
        &relation,
        prospective_manifest_bytes,
        &base_assumptions,
        &witnesses,
    )?;
    check_limit(
        "retained bytes",
        prospective_retained_bytes,
        limits.max_retained_bytes,
    )?;
    let manifest = relation.stable_manifest_with_limit(limits.max_relation_manifest_bytes)?;
    stats.relation_manifest_bytes = manifest.len();
    stats.retained_bytes = retained_byte_census(
        &source_binding,
        &relation,
        &manifest,
        &base_assumptions,
        &witnesses,
    )?;
    check_limit(
        "retained bytes",
        stats.retained_bytes,
        limits.max_retained_bytes,
    )?;
    Ok(
        GeneratedResidualAffineBranchBoundRelationCompilation::Retained(
            GeneratedResidualAffineBranchBoundParametricRelation {
                schema: GENERATED_RESIDUAL_AFFINE_BRANCH_BOUND_RELATION_V1_SCHEMA,
                source: source_binding,
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

#[derive(Clone, Copy)]
struct BranchLocator {
    source_case: u64,
    source_work_item_ordinal: usize,
    ready_terminal_ordinal: usize,
}

fn retain_condition(
    context: &ParametricCoefficientContext,
    relation: &mut ParametricRelation,
    assumptions: &mut Vec<GeneratedResidualAffineBranchBaseAssumption>,
    mut condition: ParametricNonZeroCondition,
    stats: &mut GeneratedResidualAffineBranchBoundRelationStats,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
) -> Result<
    GeneratedResidualAffineBranchBoundConditionClass,
    GeneratedResidualAffineBranchBoundRelationError,
> {
    if context.polynomial_depends_on_indices_with_limits(
        condition.polynomial(),
        limits.polynomial_composition.exact_algebra,
    )? {
        let existing = relation
            .guarded_nonzero_conditions()
            .iter()
            .enumerate()
            .find(|(_, existing)| existing.polynomial() == condition.polynomial());
        let ordinal = existing.as_ref().map_or(
            relation.guarded_nonzero_conditions().len(),
            |(ordinal, _)| *ordinal,
        );
        if ordinal == relation.guarded_nonzero_conditions().len() {
            check_limit(
                "private free-index guards",
                ordinal.checked_add(1).ok_or(
                    GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                        resource: "private free-index guards",
                    },
                )?,
                limits.max_private_free_index_guards,
            )?;
            charge_retained_terms(
                stats,
                checked_mul("retained terms", condition.polynomial().term_count(), 2)?,
                limits,
            )?;
        }
        let attached = GuardOrigin::RelationConditionAttached {
            row: relation.row_id().guard_identity(),
        };
        charge_retained_condition_origin_union(
            stats,
            existing.map(|(_, condition)| condition),
            &condition,
            Some(&attached),
            limits,
        )?;
        relation.add_guarded_nonzero_condition_with_limits(
            context,
            condition,
            relation_arithmetic_limits(limits),
        )?;
        Ok(GeneratedResidualAffineBranchBoundConditionClass::PrivateFreeIndexGuard { ordinal })
    } else {
        condition.add_origin_with_limit(
            GuardOrigin::RelationConditionAttached {
                row: relation.row_id().guard_identity(),
            },
            limits.polynomial_composition.max_guard_origins,
        )?;
        if let Some((ordinal, existing)) = assumptions
            .iter_mut()
            .enumerate()
            .find(|(_, existing)| existing.condition.polynomial() == condition.polynomial())
        {
            charge_retained_condition_origin_union(
                stats,
                Some(&existing.condition),
                &condition,
                None,
                limits,
            )?;
            existing
                .condition
                .merge_origins_from(&condition, limits.polynomial_composition.max_guard_origins)?;
            Ok(
                GeneratedResidualAffineBranchBoundConditionClass::RowLocalBaseAssumption {
                    ordinal,
                },
            )
        } else {
            let ordinal = assumptions.len();
            check_limit(
                "row-local base assumptions",
                ordinal.checked_add(1).ok_or(
                    GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                        resource: "row-local base assumptions",
                    },
                )?,
                limits.max_row_local_base_assumptions,
            )?;
            charge_retained_condition_origin_union(stats, None, &condition, None, limits)?;
            charge_retained_terms(stats, condition.polynomial().term_count(), limits)?;
            try_reserve("base assumptions", assumptions, 1)?;
            assumptions.push(GeneratedResidualAffineBranchBaseAssumption { condition });
            Ok(
                GeneratedResidualAffineBranchBoundConditionClass::RowLocalBaseAssumption {
                    ordinal,
                },
            )
        }
    }
}

/// Charge the exact additional provenance payload before a condition is
/// inserted or merged into retained state.
///
/// `ParametricNonZeroCondition` stores origins in a set, so an equal mapped
/// polynomial can accumulate a much larger provenance union than either
/// incoming condition.  The ordinary per-condition constructor limit only
/// bounds origin cardinality.  This seam additionally bounds the owned-byte
/// union and the row-wide retained-origin total before `BTreeSet::extend`
/// clones a boxed payload or allocates a node.
fn charge_retained_condition_origin_union(
    stats: &mut GeneratedResidualAffineBranchBoundRelationStats,
    existing: Option<&ParametricNonZeroCondition>,
    incoming: &ParametricNonZeroCondition,
    additional: Option<&GuardOrigin>,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    let mut final_count = existing.map_or(0, |condition| condition.origins().len());
    let mut final_bytes = existing.map_or(Ok(0usize), |condition| {
        condition
            .origins()
            .iter()
            .try_fold(0usize, |total, origin| {
                checked_add(
                    "guard origin retained bytes",
                    total,
                    origin.retained_byte_bound().ok_or(
                        GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                            resource: "guard origin retained bytes",
                        },
                    )?,
                )
            })
    })?;
    let mut additional_bytes = 0usize;

    for origin in incoming.origins() {
        if existing.is_some_and(|condition| condition.origins().contains(origin)) {
            continue;
        }
        final_count = checked_add("parametric guard origins", final_count, 1)?;
        let bytes = origin.retained_byte_bound().ok_or(
            GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                resource: "guard origin retained bytes",
            },
        )?;
        final_bytes = checked_add("guard origin retained bytes", final_bytes, bytes)?;
        additional_bytes = checked_add("retained guard origin bytes", additional_bytes, bytes)?;
    }

    if let Some(origin) = additional
        && !existing.is_some_and(|condition| condition.origins().contains(origin))
        && !incoming.origins().contains(origin)
    {
        final_count = checked_add("parametric guard origins", final_count, 1)?;
        let bytes = origin.retained_byte_bound().ok_or(
            GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                resource: "guard origin retained bytes",
            },
        )?;
        final_bytes = checked_add("guard origin retained bytes", final_bytes, bytes)?;
        additional_bytes = checked_add("retained guard origin bytes", additional_bytes, bytes)?;
    }

    check_limit(
        "parametric guard origins",
        final_count,
        limits.polynomial_composition.max_guard_origins,
    )?;
    check_limit(
        "guard origin retained bytes",
        final_bytes,
        limits
            .polynomial_composition
            .max_guard_origin_retained_bytes,
    )?;
    stats.retained_guard_origin_bytes = bounded_add(
        "retained guard origin bytes",
        stats.retained_guard_origin_bytes,
        additional_bytes,
        limits.max_total_retained_guard_origin_bytes,
    )?;
    Ok(())
}

fn push_witness(
    witnesses: &mut Vec<GeneratedResidualAffineBranchBoundConditionWitness>,
    stats: &mut GeneratedResidualAffineBranchBoundRelationStats,
    source: GeneratedResidualAffineBranchBoundConditionSource,
    class: GeneratedResidualAffineBranchBoundConditionClass,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    reserve_witness_slot(witnesses, stats, limits)?;
    push_reserved_witness(witnesses, source, class)
}

fn reserve_witness_slot(
    witnesses: &mut Vec<GeneratedResidualAffineBranchBoundConditionWitness>,
    stats: &mut GeneratedResidualAffineBranchBoundRelationStats,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    check_limit(
        "condition witnesses",
        witnesses.len().checked_add(1).ok_or(
            GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                resource: "condition witnesses",
            },
        )?,
        limits.max_condition_witnesses,
    )?;
    charge_retained_terms(stats, 1, limits)?;
    try_reserve("condition witnesses", witnesses, 1)?;
    Ok(())
}

fn charge_retained_terms(
    stats: &mut GeneratedResidualAffineBranchBoundRelationStats,
    additional: usize,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    stats.retained_terms = bounded_add(
        "retained terms",
        stats.retained_terms,
        additional,
        limits.max_retained_terms,
    )?;
    Ok(())
}

fn push_reserved_witness(
    witnesses: &mut Vec<GeneratedResidualAffineBranchBoundConditionWitness>,
    source: GeneratedResidualAffineBranchBoundConditionSource,
    class: GeneratedResidualAffineBranchBoundConditionClass,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    if witnesses.len() == witnesses.capacity() {
        return Err(GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch);
    }
    witnesses.push(GeneratedResidualAffineBranchBoundConditionWitness { source, class });
    Ok(())
}

fn unavailable(
    source: GeneratedResidualAffineBranchBoundSource,
    reason: GeneratedResidualAffineBranchUnavailableReason,
    partial_relation: ParametricRelation,
    base_assumptions: Vec<GeneratedResidualAffineBranchBaseAssumption>,
    witnesses: Vec<GeneratedResidualAffineBranchBoundConditionWitness>,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
    mut stats: GeneratedResidualAffineBranchBoundRelationStats,
) -> Result<
    GeneratedResidualAffineBranchBoundRelationCompilation,
    GeneratedResidualAffineBranchBoundRelationError,
> {
    stats.condition_witnesses = witnesses.len();
    stats.row_local_base_assumptions = base_assumptions.len();
    stats.private_free_index_guards = partial_relation.guarded_nonzero_conditions().len();
    check_limit(
        "private free-index guards",
        stats.private_free_index_guards,
        limits.max_private_free_index_guards,
    )?;
    let exact_retained_guard_origin_bytes =
        retained_origin_bytes(&partial_relation, &base_assumptions)?;
    if exact_retained_guard_origin_bytes != stats.retained_guard_origin_bytes {
        return Err(GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch);
    }
    let exact_retained_terms =
        retained_term_census(&partial_relation, &base_assumptions, &witnesses)?;
    if exact_retained_terms != stats.retained_terms {
        return Err(GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch);
    }
    stats.retained_bytes = unavailable_retained_byte_census(
        &source,
        &reason,
        &partial_relation,
        &base_assumptions,
        &witnesses,
    )?;
    check_limit(
        "retained bytes",
        stats.retained_bytes,
        limits.max_retained_bytes,
    )?;
    Ok(
        GeneratedResidualAffineBranchBoundRelationCompilation::UnavailableRow(
            GeneratedResidualAffineBranchUnavailableRowCertificate {
                schema: GENERATED_RESIDUAL_AFFINE_BRANCH_BOUND_RELATION_V1_SCHEMA,
                source,
                reason,
                partial_relation: Arc::new(partial_relation),
                base_assumptions,
                condition_witnesses: witnesses,
                limits,
                stats,
            },
        ),
    )
}

fn preflight_translation(
    context: &ParametricCoefficientContext,
    source: &ParametricRelation,
    translation: &IndexShift,
    target_row_id: &ParametricRowId,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
    stats: &mut GeneratedResidualAffineBranchBoundRelationStats,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    // A translated source guard can retain its complete source set plus the
    // index translation, whole-relation translation, and target-relation
    // attachment atoms.  Check this conservative final union before any
    // B-tree or boxed shift payload is cloned.  Translated term denominators
    // carry their input-denominator atom plus the same relation attachment.
    for condition in source.guarded_nonzero_conditions() {
        check_limit(
            "parametric guard origins",
            checked_add("parametric guard origins", condition.origins().len(), 3)?,
            limits.translation.max_guard_origins,
        )?;
    }
    if !source.terms().is_empty() {
        check_limit(
            "parametric guard origins",
            2,
            limits.translation.max_guard_origins,
        )?;
    }
    let polynomials = checked_add(
        "translation polynomials",
        checked_mul("translation polynomials", source.terms().len(), 2)?,
        source.guarded_nonzero_conditions().len(),
    )?;
    check_limit(
        "translation polynomials",
        polynomials,
        limits.max_translation_polynomials,
    )?;
    stats.translation_polynomials = polynomials;
    for condition in source.guarded_nonzero_conditions() {
        let item = context.preflight_translate_polynomial(
            condition.polynomial(),
            translation,
            limits.translation,
        )?;
        stats.translation_source_terms = bounded_add(
            "translation source-term allowance",
            stats.translation_source_terms,
            item.source_terms(),
            limits.max_total_translation_source_term_allowance,
        )?;
        stats.translation_output_term_bound = bounded_add(
            "translation output-term allowance",
            stats.translation_output_term_bound,
            item.output_term_bound(),
            limits.max_total_translation_output_term_allowance,
        )?;
        stats.translation_power_operation_bound = bounded_add(
            "translation power-operation allowance",
            stats.translation_power_operation_bound,
            item.power_operation_bound(),
            limits.max_total_translation_power_operation_allowance,
        )?;
        stats.translation_integer_bit_work_bound = bounded_add(
            "translation integer-bit allowance",
            stats.translation_integer_bit_work_bound,
            item.integer_bit_work_bound(),
            limits.max_total_translation_integer_bit_allowance,
        )?;
        stats.translation_retained_output_terms = bounded_add(
            "translation retained output terms",
            stats.translation_retained_output_terms,
            checked_mul(
                "translation retained output terms",
                item.retained_output_term_bound(),
                2,
            )?,
            limits.max_total_translation_retained_output_terms,
        )?;
        stats.translation_retained_output_bytes = bounded_add(
            "translation retained output bytes",
            stats.translation_retained_output_bytes,
            checked_mul(
                "translation retained output bytes",
                item.retained_output_byte_bound(),
                2,
            )?,
            limits.max_total_translation_retained_output_bytes,
        )?;
    }
    for coefficient in source.terms().values() {
        let item = context.preflight_translate_coefficient(
            coefficient,
            translation,
            limits.translation,
        )?;
        stats.translation_source_terms = bounded_add(
            "translation source-term allowance",
            stats.translation_source_terms,
            item.source_terms(),
            limits.max_total_translation_source_term_allowance,
        )?;
        stats.translation_output_term_bound = bounded_add(
            "translation output-term allowance",
            stats.translation_output_term_bound,
            item.output_term_bound(),
            limits.max_total_translation_output_term_allowance,
        )?;
        stats.translation_power_operation_bound = bounded_add(
            "translation power-operation allowance",
            stats.translation_power_operation_bound,
            item.power_operation_bound(),
            limits.max_total_translation_power_operation_allowance,
        )?;
        stats.translation_integer_bit_work_bound = bounded_add(
            "translation integer-bit allowance",
            stats.translation_integer_bit_work_bound,
            item.integer_bit_work_bound(),
            limits.max_total_translation_integer_bit_allowance,
        )?;
        stats.translation_normalization_input_term_pairs = bounded_add(
            "translation normalization input term pairs",
            stats.translation_normalization_input_term_pairs,
            item.normalization_input_term_pair_bound(),
            limits.max_total_translation_normalization_input_term_pairs,
        )?;
        let mapped_and_retained_terms = checked_add(
            "translation retained output terms",
            checked_add(
                "translation retained output terms",
                item.numerator().retained_output_term_bound(),
                item.denominator().retained_output_term_bound(),
            )?,
            checked_mul(
                "translation retained output terms",
                item.normalized_coefficient_term_bound(),
                3,
            )?,
        )?;
        stats.translation_retained_output_terms = bounded_add(
            "translation retained output terms",
            stats.translation_retained_output_terms,
            mapped_and_retained_terms,
            limits.max_total_translation_retained_output_terms,
        )?;
        let mapped_and_retained_bytes = checked_add(
            "translation retained output bytes",
            checked_add(
                "translation retained output bytes",
                item.numerator().retained_output_byte_bound(),
                item.denominator().retained_output_byte_bound(),
            )?,
            checked_mul(
                "translation retained output bytes",
                item.normalized_coefficient_byte_bound(),
                3,
            )?,
        )?;
        stats.translation_retained_output_bytes = bounded_add(
            "translation retained output bytes",
            stats.translation_retained_output_bytes,
            mapped_and_retained_bytes,
            limits.max_total_translation_retained_output_bytes,
        )?;
    }
    let source_row = source.row_id().guard_identity();
    let target_row = target_row_id.guard_identity();
    let relation_translation = GuardOrigin::relation_translation_retained_byte_bound(
        &source_row,
        &target_row,
        translation.arity(),
    )
    .ok_or(
        GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
            resource: "guard origin copy bytes",
        },
    )?;
    let index_translation = GuardOrigin::index_translation_retained_byte_bound(translation.arity())
        .ok_or(
            GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                resource: "guard origin copy bytes",
            },
        )?;
    let attached = GuardOrigin::relation_attached_retained_byte_bound(&target_row).ok_or(
        GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
            resource: "guard origin copy bytes",
        },
    )?;
    let mut origin_bytes = 0usize;
    for condition in source.guarded_nonzero_conditions() {
        for origin in condition.origins() {
            origin_bytes = checked_add(
                "guard origin copy bytes",
                origin_bytes,
                checked_mul(
                    "guard origin copy bytes",
                    origin.retained_byte_bound().ok_or(
                        GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                            resource: "guard origin copy bytes",
                        },
                    )?,
                    3,
                )?,
            )?;
        }
        origin_bytes = checked_add(
            "guard origin copy bytes",
            origin_bytes,
            checked_add(
                "guard origin copy bytes",
                checked_mul("guard origin copy bytes", index_translation, 2)?,
                checked_add(
                    "guard origin copy bytes",
                    checked_mul("guard origin copy bytes", relation_translation, 2)?,
                    checked_mul("guard origin copy bytes", attached, 2)?,
                )?,
            )?,
        )?;
    }
    let input_denominator = GuardOrigin::relation_input_term_denominator_retained_byte_bound(
        &target_row,
        translation.arity(),
    )
    .ok_or(
        GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
            resource: "guard origin copy bytes",
        },
    )?;
    let per_term_once = checked_add("guard origin copy bytes", input_denominator, attached)?;
    let per_term = checked_mul("guard origin copy bytes", per_term_once, 2)?;
    origin_bytes = checked_add(
        "guard origin copy bytes",
        origin_bytes,
        checked_mul("guard origin copy bytes", source.terms().len(), per_term)?,
    )?;
    stats.guard_origin_copy_bytes = bounded_add(
        "guard origin copy bytes",
        stats.guard_origin_copy_bytes,
        origin_bytes,
        limits.max_total_guard_origin_copy_bytes,
    )?;
    Ok(())
}

fn charge_source_guard_origins(
    stats: &mut GeneratedResidualAffineBranchBoundRelationStats,
    source: &ParametricNonZeroCondition,
    relation_origin: &GuardOrigin,
    target_row_id: &ParametricRowId,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    let mut bytes = relation_origin.retained_byte_bound().ok_or(
        GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
            resource: "guard origin copy bytes",
        },
    )?;
    bytes = checked_add(
        "guard origin copy bytes",
        bytes,
        GuardOrigin::relation_attached_retained_byte_bound(&target_row_id.guard_identity()).ok_or(
            GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                resource: "guard origin copy bytes",
            },
        )?,
    )?;
    for origin in source.origins() {
        bytes = checked_add(
            "guard origin copy bytes",
            bytes,
            origin.retained_byte_bound().ok_or(
                GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                    resource: "guard origin copy bytes",
                },
            )?,
        )?;
    }
    check_limit(
        "guard origin retained bytes",
        bytes,
        limits
            .polynomial_composition
            .max_guard_origin_retained_bytes,
    )?;
    stats.guard_origin_copy_bytes = bounded_add(
        "guard origin copy bytes",
        stats.guard_origin_copy_bytes,
        checked_mul("guard origin copy bytes", bytes, 2)?,
        limits.max_total_guard_origin_copy_bytes,
    )?;
    Ok(())
}

/// Scan every guard and both halves of every coefficient before the first
/// native Symbolica affine evaluator is entered.  The returned census is a
/// conservative row-wide envelope; exact retained counts produced later must
/// fit inside it.
fn preflight_complete_row_compositions(
    context: &ParametricCoefficientContext,
    translated: &ParametricRelation,
    plan: &crate::parametric_coefficient::ResidualAffineCompositionPlan,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
) -> Result<
    GeneratedResidualAffineBranchBoundRelationStats,
    GeneratedResidualAffineBranchBoundRelationError,
> {
    let mut prospective = GeneratedResidualAffineBranchBoundRelationStats::default();
    for guard in translated.guarded_nonzero_conditions() {
        let call_limits = remaining_composition_limits(limits, &prospective)?;
        let item = context.preflight_polynomial_on_residual_affine_composition_plan(
            guard.polynomial(),
            plan,
            call_limits,
        )?;
        prospective.polynomial_compositions = bounded_add(
            "polynomial compositions",
            prospective.polynomial_compositions,
            1,
            limits.max_polynomial_compositions,
        )?;
        consume_polynomial_stats(&mut prospective, item, limits)?;
        check_prospective_retained_output_totals(&prospective, limits)?;
    }
    for coefficient in translated.terms().values() {
        let call_limits = remaining_composition_limits(limits, &prospective)?;
        let item = context.preflight_coefficient_on_residual_affine_composition_plan(
            coefficient,
            plan,
            call_limits,
        )?;
        prospective.polynomial_compositions = bounded_add(
            "polynomial compositions",
            prospective.polynomial_compositions,
            2,
            limits.max_polynomial_compositions,
        )?;
        consume_polynomial_stats(&mut prospective, item.aggregate(), limits)?;
        prospective.total_integer_bit_work = bounded_add(
            "total integer-bit work",
            prospective.total_integer_bit_work,
            item.durable_denominator_integer_bit_payload_bound(),
            limits.max_total_integer_bit_work,
        )?;
        if item.total_integer_bit_work_bound()
            != checked_add(
                "coefficient total integer-bit work bound",
                item.aggregate().integer_bit_work_bound(),
                item.durable_denominator_integer_bit_payload_bound(),
            )?
        {
            return Err(GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch);
        }
        prospective.total_normalization_input_term_pairs = bounded_add(
            "total normalization input term pairs",
            prospective.total_normalization_input_term_pairs,
            item.normalization_input_term_pair_bound(),
            limits.max_total_normalization_input_term_pairs,
        )?;
        prospective.total_durable_denominator_terms = bounded_add(
            "total durable denominator terms",
            prospective.total_durable_denominator_terms,
            item.durable_denominator_term_bound(),
            limits.max_total_durable_denominator_terms,
        )?;
        prospective.total_durable_denominator_exponent_entries = bounded_add(
            "total durable denominator exponent entries",
            prospective.total_durable_denominator_exponent_entries,
            item.durable_denominator_exponent_entry_bound(),
            limits.max_total_durable_denominator_exponent_entries,
        )?;
        prospective.total_durable_denominator_integer_bits = bounded_add(
            "total durable denominator integer bits",
            prospective.total_durable_denominator_integer_bits,
            item.durable_denominator_integer_bit_payload_bound(),
            limits.max_total_durable_denominator_integer_bits,
        )?;
        check_prospective_retained_output_totals(&prospective, limits)?;
    }
    let exact_compositions = checked_add(
        "polynomial compositions",
        checked_mul("polynomial compositions", translated.terms().len(), 2)?,
        translated.guarded_nonzero_conditions().len(),
    )?;
    if prospective.polynomial_compositions != exact_compositions {
        return Err(GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch);
    }
    Ok(prospective)
}

fn check_prospective_retained_output_totals(
    prospective: &GeneratedResidualAffineBranchBoundRelationStats,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    // Actual retained output cannot exceed the expansion/exponent envelope.
    // Applying both actual-output aggregate limits here makes those limits a
    // pre-native boundary rather than a post-allocation observation.
    check_limit(
        "total output terms",
        prospective.total_output_term_bound,
        limits.max_total_output_terms,
    )?;
    check_limit(
        "total output exponent entries",
        prospective.total_output_exponent_entry_bound,
        limits.max_total_output_exponent_entries,
    )
}

fn verify_composition_execution_within_preflight(
    actual: &GeneratedResidualAffineBranchBoundRelationStats,
    prospective: &GeneratedResidualAffineBranchBoundRelationStats,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    let exact_fields_match = actual.polynomial_compositions == prospective.polynomial_compositions
        && actual.total_source_terms == prospective.total_source_terms
        && actual.total_source_exponent_entries == prospective.total_source_exponent_entries
        && actual.total_expanded_contributions == prospective.total_expanded_contributions
        && actual.total_output_term_bound == prospective.total_output_term_bound
        && actual.total_output_exponent_entry_bound
            == prospective.total_output_exponent_entry_bound
        && actual.total_power_calls == prospective.total_power_calls
        && actual.total_native_power_heap_pairs == prospective.total_native_power_heap_pairs
        && actual.total_multiplication_term_pairs == prospective.total_multiplication_term_pairs
        && actual.total_addition_term_visits == prospective.total_addition_term_visits
        && actual.total_native_integer_bit_work == prospective.total_native_integer_bit_work;
    let retained_fields_fit = actual.total_output_terms <= prospective.total_output_term_bound
        && actual.total_output_exponent_entries <= prospective.total_output_exponent_entry_bound
        && actual.total_integer_bit_work <= prospective.total_integer_bit_work
        && actual.total_normalization_input_term_pairs
            <= prospective.total_normalization_input_term_pairs
        && actual.total_durable_denominator_terms <= prospective.total_durable_denominator_terms
        && actual.total_durable_denominator_exponent_entries
            <= prospective.total_durable_denominator_exponent_entries
        && actual.total_durable_denominator_integer_bits
            <= prospective.total_durable_denominator_integer_bits;
    if exact_fields_match && retained_fields_fit {
        Ok(())
    } else {
        Err(GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch)
    }
}

fn remaining_composition_limits(
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
    stats: &GeneratedResidualAffineBranchBoundRelationStats,
) -> Result<
    ResidualUnitAffinePolynomialCompositionLimits,
    GeneratedResidualAffineBranchBoundRelationError,
> {
    let mut effective = limits.polynomial_composition;
    macro_rules! remaining_field {
        ($field:ident, $used:ident, $total:ident, $name:literal) => {
            effective.$field = effective
                .$field
                .min(remaining($name, limits.$total, stats.$used)?);
        };
    }
    remaining_field!(
        max_source_terms,
        total_source_terms,
        max_total_source_terms,
        "total source terms"
    );
    remaining_field!(
        max_source_exponent_entries,
        total_source_exponent_entries,
        max_total_source_exponent_entries,
        "total source exponent entries"
    );
    remaining_field!(
        max_expanded_contributions,
        total_expanded_contributions,
        max_total_expanded_contributions,
        "total expanded contributions"
    );
    remaining_field!(
        max_output_terms,
        total_output_term_bound,
        max_total_output_term_bound,
        "total output-term bound"
    );
    remaining_field!(
        max_output_exponent_entries,
        total_output_exponent_entry_bound,
        max_total_output_exponent_entry_bound,
        "total output exponent-entry bound"
    );
    remaining_field!(
        max_power_calls,
        total_power_calls,
        max_total_power_calls,
        "total power calls"
    );
    remaining_field!(
        max_native_power_heap_pairs,
        total_native_power_heap_pairs,
        max_total_native_power_heap_pairs,
        "total native power heap pairs"
    );
    remaining_field!(
        max_multiplication_term_pairs,
        total_multiplication_term_pairs,
        max_total_multiplication_term_pairs,
        "total multiplication term pairs"
    );
    remaining_field!(
        max_addition_term_visits,
        total_addition_term_visits,
        max_total_addition_term_visits,
        "total addition term visits"
    );
    remaining_field!(
        max_integer_bit_work,
        total_integer_bit_work,
        max_total_integer_bit_work,
        "total integer-bit work"
    );
    remaining_field!(
        max_normalization_input_term_pairs,
        total_normalization_input_term_pairs,
        max_total_normalization_input_term_pairs,
        "total normalization input term pairs"
    );
    Ok(effective)
}

fn consume_polynomial_stats(
    stats: &mut GeneratedResidualAffineBranchBoundRelationStats,
    item: ResidualUnitAffinePolynomialCompositionStats,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
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
        "total output-term bound"
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
        "total output exponent-entry bound"
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
    add!(
        total_native_integer_bit_work,
        item.native_integer_bit_work_bound(),
        max_total_native_integer_bit_work,
        "total native integer-bit work"
    );
    add!(
        total_integer_bit_work,
        item.integer_bit_work_bound(),
        max_total_integer_bit_work,
        "total integer-bit work"
    );
    Ok(())
}

fn consume_coefficient_stats(
    stats: &mut GeneratedResidualAffineBranchBoundRelationStats,
    item: crate::ResidualUnitAffineCoefficientCompositionStats,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    consume_polynomial_stats(stats, item.aggregate(), limits)?;
    // Replace the aggregate integer charge with the complete aggregate plus
    // durable denominator payload. The aggregate portion was just consumed.
    stats.total_integer_bit_work = bounded_add(
        "total integer-bit work",
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

fn relation_arithmetic_limits(
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
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

fn retained_origin_bytes(
    relation: &ParametricRelation,
    assumptions: &[GeneratedResidualAffineBranchBaseAssumption],
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    relation
        .guarded_nonzero_conditions()
        .iter()
        .map(|condition| condition.origins())
        .chain(assumptions.iter().map(|entry| entry.condition.origins()))
        .try_fold(0usize, |total, origins| {
            origins.iter().try_fold(total, |total, origin| {
                checked_add(
                    "retained guard origin bytes",
                    total,
                    origin.retained_byte_bound().ok_or(
                        GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                            resource: "retained guard origin bytes",
                        },
                    )?,
                )
            })
        })
}

fn retained_term_census(
    relation: &ParametricRelation,
    assumptions: &Vec<GeneratedResidualAffineBranchBaseAssumption>,
    witnesses: &Vec<GeneratedResidualAffineBranchBoundConditionWitness>,
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    let mut total = 0usize;
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
    for polynomial in relation.nonzero_conditions() {
        total = checked_add("retained terms", total, polynomial.term_count())?;
    }
    for condition in relation.guarded_nonzero_conditions() {
        total = checked_add("retained terms", total, condition.polynomial().term_count())?;
    }
    for entry in assumptions {
        total = checked_add(
            "retained terms",
            total,
            entry.condition.polynomial().term_count(),
        )?;
    }
    checked_add("retained terms", total, witnesses.len())
}

fn retained_byte_census(
    source: &GeneratedResidualAffineBranchBoundSource,
    relation: &ParametricRelation,
    manifest: &String,
    assumptions: &Vec<GeneratedResidualAffineBranchBaseAssumption>,
    witnesses: &Vec<GeneratedResidualAffineBranchBoundConditionWitness>,
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    retained_byte_census_with_manifest_capacity(
        source,
        relation,
        manifest.capacity(),
        assumptions,
        witnesses,
    )
}

fn retained_byte_census_with_manifest_capacity(
    source: &GeneratedResidualAffineBranchBoundSource,
    relation: &ParametricRelation,
    manifest_capacity: usize,
    assumptions: &Vec<GeneratedResidualAffineBranchBaseAssumption>,
    witnesses: &Vec<GeneratedResidualAffineBranchBoundConditionWitness>,
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    let mut bytes = size_of::<GeneratedResidualAffineBranchBoundParametricRelation>();
    bytes = checked_add("retained bytes", bytes, source_owned_bytes(source)?)?;
    bytes = checked_add(
        "retained bytes",
        bytes,
        arc_owned_value_bytes(relation_owned_bytes(relation)?)?,
    )?;
    let manifest_owned = checked_add("retained bytes", size_of::<String>(), manifest_capacity)?;
    bytes = checked_add(
        "retained bytes",
        bytes,
        arc_owned_value_bytes(manifest_owned)?,
    )?;
    bytes = checked_add(
        "retained bytes",
        bytes,
        condition_vector_owned_bytes(
            assumptions.capacity(),
            assumptions.iter().map(|v| &v.condition),
        )?,
    )?;
    bytes = checked_add(
        "retained bytes",
        bytes,
        witness_vector_owned_bytes(witnesses.capacity(), witnesses)?,
    )?;
    Ok(bytes)
}

fn preflight_source_binding_retained_bytes(
    translation: &IndexShift,
    target_row_label_bytes: usize,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    let dynamic_source_bytes = checked_add(
        "retained bytes",
        translation.owned_retained_byte_bound().ok_or(
            GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                resource: "retained bytes",
            },
        )?,
        arc_owned_value_bytes(target_row_label_bytes)?,
    )?;
    let smallest_certificate = size_of::<GeneratedResidualAffineBranchEmptyCertificate>()
        .min(size_of::<
            GeneratedResidualAffineBranchUnavailableRowCertificate,
        >())
        .min(size_of::<
            GeneratedResidualAffineBranchBoundParametricRelation,
        >());
    check_limit(
        "retained bytes",
        checked_add("retained bytes", smallest_certificate, dynamic_source_bytes)?,
        limits.max_retained_bytes,
    )
}

fn empty_retained_byte_census(
    source: &GeneratedResidualAffineBranchBoundSource,
    _reason: &GeneratedResidualAffineBranchEmptyReason,
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    checked_add(
        "retained bytes",
        size_of::<GeneratedResidualAffineBranchEmptyCertificate>(),
        source_owned_bytes(source)?,
    )
}

fn unavailable_retained_byte_census(
    source: &GeneratedResidualAffineBranchBoundSource,
    reason: &GeneratedResidualAffineBranchUnavailableReason,
    partial_relation: &ParametricRelation,
    assumptions: &Vec<GeneratedResidualAffineBranchBaseAssumption>,
    witnesses: &Vec<GeneratedResidualAffineBranchBoundConditionWitness>,
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    let mut bytes = size_of::<GeneratedResidualAffineBranchUnavailableRowCertificate>();
    bytes = checked_add("retained bytes", bytes, source_owned_bytes(source)?)?;
    bytes = checked_add(
        "retained bytes",
        bytes,
        unavailable_reason_owned_bytes(reason)?,
    )?;
    bytes = checked_add(
        "retained bytes",
        bytes,
        arc_owned_value_bytes(relation_owned_bytes(partial_relation)?)?,
    )?;
    bytes = checked_add(
        "retained bytes",
        bytes,
        condition_vector_owned_bytes(
            assumptions.capacity(),
            assumptions.iter().map(|v| &v.condition),
        )?,
    )?;
    checked_add(
        "retained bytes",
        bytes,
        witness_vector_owned_bytes(witnesses.capacity(), witnesses)?,
    )
}

fn preflight_unavailable_term_denominator_retained_bytes(
    source: &GeneratedResidualAffineBranchBoundSource,
    partial_relation: &ParametricRelation,
    assumptions: &Vec<GeneratedResidualAffineBranchBaseAssumption>,
    witnesses: &Vec<GeneratedResidualAffineBranchBoundConditionWitness>,
    reason_shift: &IndexShift,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    let mut bytes = size_of::<GeneratedResidualAffineBranchUnavailableRowCertificate>();
    bytes = checked_add("retained bytes", bytes, source_owned_bytes(source)?)?;
    bytes = checked_add(
        "retained bytes",
        bytes,
        reason_shift.owned_retained_byte_bound().ok_or(
            GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                resource: "retained bytes",
            },
        )?,
    )?;
    bytes = checked_add(
        "retained bytes",
        bytes,
        arc_owned_value_bytes(relation_owned_bytes(partial_relation)?)?,
    )?;
    bytes = checked_add(
        "retained bytes",
        bytes,
        condition_vector_owned_bytes(
            assumptions.capacity(),
            assumptions.iter().map(|entry| &entry.condition),
        )?,
    )?;
    bytes = checked_add(
        "retained bytes",
        bytes,
        witness_vector_owned_bytes(witnesses.capacity(), witnesses)?,
    )?;
    check_limit("retained bytes", bytes, limits.max_retained_bytes)
}

fn source_owned_bytes(
    source: &GeneratedResidualAffineBranchBoundSource,
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    let mut bytes = source.translation.owned_retained_byte_bound().ok_or(
        GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
            resource: "retained bytes",
        },
    )?;
    if let ParametricRowId::Derived { label } = &source.target_row_id {
        bytes = checked_add("retained bytes", bytes, arc_owned_value_bytes(label.len())?)?;
    }
    Ok(bytes)
}

fn relation_owned_bytes(
    relation: &ParametricRelation,
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    relation.owned_retained_byte_bound().ok_or(
        GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
            resource: "retained bytes",
        },
    )
}

fn condition_owned_bytes(
    condition: &ParametricNonZeroCondition,
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    condition.owned_retained_byte_bound().ok_or(
        GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
            resource: "retained bytes",
        },
    )
}

fn condition_vector_owned_bytes<'a>(
    capacity: usize,
    conditions: impl IntoIterator<Item = &'a ParametricNonZeroCondition>,
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    let mut bytes = checked_mul(
        "retained bytes",
        capacity,
        size_of::<GeneratedResidualAffineBranchBaseAssumption>(),
    )?;
    for condition in conditions {
        bytes = checked_add("retained bytes", bytes, condition_owned_bytes(condition)?)?;
    }
    Ok(bytes)
}

fn witness_vector_owned_bytes(
    capacity: usize,
    witnesses: &[GeneratedResidualAffineBranchBoundConditionWitness],
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    let mut bytes = checked_mul(
        "retained bytes",
        capacity,
        size_of::<GeneratedResidualAffineBranchBoundConditionWitness>(),
    )?;
    for witness in witnesses {
        if let GeneratedResidualAffineBranchBoundConditionSource::TranslatedSourceTermDenominator {
            translated_shift,
            ..
        } = &witness.source
        {
            bytes = checked_add(
                "retained bytes",
                bytes,
                translated_shift.owned_retained_byte_bound().ok_or(
                    GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                        resource: "retained bytes",
                    },
                )?,
            )?;
        }
    }
    Ok(bytes)
}

fn unavailable_reason_owned_bytes(
    reason: &GeneratedResidualAffineBranchUnavailableReason,
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    match reason {
        GeneratedResidualAffineBranchUnavailableReason::TranslatedSourceGuardComposesToZero {
            ..
        } => Ok(0),
        GeneratedResidualAffineBranchUnavailableReason::TranslatedSourceTermDenominatorComposesToZero {
            translated_shift,
            ..
        } => translated_shift.owned_retained_byte_bound().ok_or(
            GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                resource: "retained bytes",
            },
        ),
    }
}

fn arc_owned_value_bytes(
    value_bytes: usize,
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    checked_add(
        "retained bytes",
        value_bytes,
        checked_mul("retained bytes", size_of::<usize>(), 2)?,
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct QuerySpecializationPreflight {
    source_terms: usize,
    output_terms: usize,
    power_operations: usize,
    integer_bit_work: usize,
    normalization_input_term_pairs: usize,
    retained_terms: usize,
    retained_bytes: usize,
}

fn preflight_query_specialization<'a>(
    context: &ParametricCoefficientContext,
    relation: &ParametricRelation,
    additional: impl IntoIterator<Item = &'a ParametricNonZeroCondition>,
    assignment: &[i64],
    limits: GeneratedResidualAffineBranchConcreteSpecializationLimits,
) -> Result<QuerySpecializationPreflight, GeneratedResidualAffineBranchBoundRelationError> {
    // This is a complete pre-native pass.  It bounds both the retained
    // concrete row and the larger transient denominator-guard payload owned
    // simultaneously by `GuardedCoefficientSpecialization` while conditions
    // are copied into that row.
    let mut stats = QuerySpecializationPreflight {
        retained_bytes: checked_add(
            "concrete query clone bytes",
            relation_owned_bytes(relation)?,
            size_of::<ConcreteRelation>(),
        )?,
        ..Default::default()
    };
    // Equal specialized polynomials merge origin sets.  Bound the worst case
    // in which every source condition and every coefficient denominator
    // collapses to one concrete polynomial before any origin tree is cloned.
    let mut worst_merged_origin_count = 2usize;
    let attached =
        GuardOrigin::relation_attached_retained_byte_bound(&relation.row_id().guard_identity())
            .ok_or(
                GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                    resource: "concrete query clone bytes",
                },
            )?;
    let index_specialization =
        GuardOrigin::index_specialization_retained_byte_bound(relation.arity()).ok_or(
            GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                resource: "concrete query clone bytes",
            },
        )?;
    for condition in relation.guarded_nonzero_conditions() {
        consume_query_guard_preflight(
            context,
            condition,
            false,
            assignment,
            index_specialization,
            attached,
            limits,
            &mut stats,
            &mut worst_merged_origin_count,
        )?;
    }
    for condition in additional {
        consume_query_guard_preflight(
            context,
            condition,
            true,
            assignment,
            index_specialization,
            attached,
            limits,
            &mut stats,
            &mut worst_merged_origin_count,
        )?;
    }
    if !relation.terms().is_empty() {
        check_limit(
            "specialized guard origins",
            3,
            limits.arithmetic.max_guard_origins,
        )?;
        worst_merged_origin_count =
            checked_add("specialized guard origins", worst_merged_origin_count, 1)?;
    }
    check_limit(
        "specialized guard origins",
        worst_merged_origin_count,
        limits.arithmetic.max_guard_origins,
    )?;

    let coefficient_origin = GuardOrigin::CoefficientSpecializationDenominator
        .retained_byte_bound()
        .ok_or(
            GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                resource: "concrete query clone bytes",
            },
        )?;
    let concrete_key_bytes = checked_add(
        "concrete query clone bytes",
        size_of::<crate::parametric_relation::ConcreteIntegralKey>(),
        checked_mul(
            "concrete query clone bytes",
            assignment.len(),
            size_of::<i64>(),
        )?,
    )?;
    let concrete_term_node_bytes = checked_add(
        "concrete query clone bytes",
        checked_mul(
            "concrete query clone bytes",
            size_of::<(
                crate::parametric_relation::ConcreteIntegralKey,
                crate::Coefficient,
            )>(),
            16,
        )?,
        checked_mul("concrete query clone bytes", size_of::<usize>(), 32)?,
    )?;
    for coefficient in relation.terms().values() {
        let item =
            context.preflight_specialize_coefficient(coefficient, assignment, limits.arithmetic)?;
        stats.source_terms = bounded_add(
            "concrete query source terms",
            stats.source_terms,
            item.source_terms(),
            limits.max_query_source_terms,
        )?;
        let item_output_terms = checked_add(
            "concrete query output terms",
            item.output_term_bound(),
            checked_add(
                "concrete query output terms",
                item.normalized_coefficient_term_bound(),
                checked_mul(
                    "concrete query output terms",
                    item.denominator_guard_term_bound(),
                    4,
                )?,
            )?,
        )?;
        stats.output_terms = bounded_add(
            "concrete query output terms",
            stats.output_terms,
            item_output_terms,
            limits.max_query_output_terms,
        )?;
        stats.power_operations = bounded_add(
            "concrete query power operations",
            stats.power_operations,
            item.power_operation_bound(),
            limits.max_query_power_operations,
        )?;
        stats.integer_bit_work = bounded_add(
            "concrete query integer-bit work",
            stats.integer_bit_work,
            item.integer_bit_work_bound(),
            limits.max_query_integer_bit_work,
        )?;
        stats.normalization_input_term_pairs = bounded_add(
            "concrete query normalization input term pairs",
            stats.normalization_input_term_pairs,
            item.normalization_input_term_pair_bound(),
            limits.max_query_normalization_input_term_pairs,
        )?;
        let peak_terms = checked_add(
            "concrete query clone terms",
            item.normalized_coefficient_term_bound(),
            checked_mul(
                "concrete query clone terms",
                item.denominator_guard_term_bound(),
                4,
            )?,
        )?;
        stats.retained_terms = bounded_add(
            "concrete query clone terms",
            stats.retained_terms,
            peak_terms,
            limits.max_query_clone_terms,
        )?;

        let denominator_polynomials = checked_mul(
            "concrete query clone bytes",
            item.denominator_guard_byte_bound(),
            4,
        )?;
        let denominator_condition_structures = checked_mul(
            "concrete query clone bytes",
            size_of::<crate::parametric_coefficient::SpecializedNonZeroCondition>(),
            2,
        )?;
        // The temporary condition owns coefficient+assignment origins.  Its
        // retained copy owns those two plus the relation attachment.
        let denominator_origins = checked_add(
            "concrete query clone bytes",
            checked_mul(
                "concrete query clone bytes",
                checked_add(
                    "concrete query clone bytes",
                    coefficient_origin,
                    index_specialization,
                )?,
                2,
            )?,
            attached,
        )?;
        let coefficient_bytes = checked_add(
            "concrete query clone bytes",
            item.normalized_coefficient_byte_bound(),
            checked_add(
                "concrete query clone bytes",
                checked_add(
                    "concrete query clone bytes",
                    denominator_polynomials,
                    denominator_condition_structures,
                )?,
                checked_add(
                    "concrete query clone bytes",
                    denominator_origins,
                    checked_add(
                        "concrete query clone bytes",
                        concrete_key_bytes,
                        concrete_term_node_bytes,
                    )?,
                )?,
            )?,
        )?;
        stats.retained_bytes = bounded_add(
            "concrete query clone bytes",
            stats.retained_bytes,
            coefficient_bytes,
            limits.max_query_clone_bytes,
        )?;
    }
    check_limit(
        "concrete query clone terms",
        stats.retained_terms,
        limits.max_query_clone_terms,
    )?;
    check_limit(
        "concrete query clone bytes",
        stats.retained_bytes,
        limits.max_query_clone_bytes,
    )?;
    Ok(stats)
}

#[allow(clippy::too_many_arguments)]
fn consume_query_guard_preflight(
    context: &ParametricCoefficientContext,
    condition: &ParametricNonZeroCondition,
    additional_source: bool,
    assignment: &[i64],
    index_specialization_bytes: usize,
    attached_bytes: usize,
    limits: GeneratedResidualAffineBranchConcreteSpecializationLimits,
    stats: &mut QuerySpecializationPreflight,
    worst_merged_origin_count: &mut usize,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    check_limit(
        "specialized guard origins",
        checked_add("specialized guard origins", condition.origins().len(), 2)?,
        limits.arithmetic.max_guard_origins,
    )?;
    *worst_merged_origin_count = checked_add(
        "specialized guard origins",
        *worst_merged_origin_count,
        condition.origins().len(),
    )?;
    check_limit(
        "specialized guard origins",
        *worst_merged_origin_count,
        limits.arithmetic.max_guard_origins,
    )?;
    let item = context.preflight_specialize_polynomial(
        condition.polynomial(),
        assignment,
        limits.arithmetic,
    )?;
    stats.source_terms = bounded_add(
        "concrete query source terms",
        stats.source_terms,
        item.source_terms(),
        limits.max_query_source_terms,
    )?;
    let output_terms = checked_mul("concrete query output terms", item.output_term_bound(), 2)?;
    stats.output_terms = bounded_add(
        "concrete query output terms",
        stats.output_terms,
        output_terms,
        limits.max_query_output_terms,
    )?;
    stats.power_operations = bounded_add(
        "concrete query power operations",
        stats.power_operations,
        item.power_operation_bound(),
        limits.max_query_power_operations,
    )?;
    stats.integer_bit_work = bounded_add(
        "concrete query integer-bit work",
        stats.integer_bit_work,
        item.integer_bit_work_bound(),
        limits.max_query_integer_bit_work,
    )?;
    stats.retained_terms = bounded_add(
        "concrete query clone terms",
        stats.retained_terms,
        checked_mul(
            "concrete query clone terms",
            item.retained_output_term_bound(),
            2,
        )?,
        limits.max_query_clone_terms,
    )?;

    let mut origin_bytes = checked_add(
        "concrete query clone bytes",
        index_specialization_bytes,
        attached_bytes,
    )?;
    for origin in condition.origins() {
        origin_bytes = checked_add(
            "concrete query clone bytes",
            origin_bytes,
            origin.retained_byte_bound().ok_or(
                GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
                    resource: "concrete query clone bytes",
                },
            )?,
        )?;
    }
    let output_bytes = checked_add(
        "concrete query clone bytes",
        checked_mul(
            "concrete query clone bytes",
            item.retained_output_byte_bound(),
            2,
        )?,
        checked_add(
            "concrete query clone bytes",
            size_of::<crate::parametric_coefficient::SpecializedNonZeroCondition>(),
            origin_bytes,
        )?,
    )?;
    let input_bytes = if additional_source {
        condition_owned_bytes(condition)?
    } else {
        0
    };
    stats.retained_bytes = bounded_add(
        "concrete query clone bytes",
        stats.retained_bytes,
        checked_add("concrete query clone bytes", input_bytes, output_bytes)?,
        limits.max_query_clone_bytes,
    )?;
    Ok(())
}

fn replay_expected(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source: &GeneratedResidualAffineBranchBoundSource,
    limits: GeneratedResidualAffineBranchBoundRelationLimits,
    matches: impl FnOnce(GeneratedResidualAffineBranchBoundRelationCompilation) -> bool,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    let replayed = compile_caught(
        family,
        context,
        source.source_row_ordinal,
        copy_shift(&source.translation)?,
        source.branch.clone(),
        source.branch_guards.clone(),
        limits,
        true,
    )?;
    if matches(replayed) {
        Ok(())
    } else {
        Err(GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch)
    }
}

fn validate_replay_schema(
    schema: &str,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    if schema == GENERATED_RESIDUAL_AFFINE_BRANCH_BOUND_RELATION_V1_SCHEMA {
        Ok(())
    } else {
        Err(GeneratedResidualAffineBranchBoundRelationError::SchemaMismatch)
    }
}

fn source_payload_eq(
    left: &GeneratedResidualAffineBranchBoundSource,
    right: &GeneratedResidualAffineBranchBoundSource,
) -> bool {
    Arc::ptr_eq(&left.row_span, &right.row_span)
        && left.source_row_ordinal == right.source_row_ordinal
        && left.translation == right.translation
        && left.target_row_id == right.target_row_id
        && Arc::ptr_eq(&left.branch, &right.branch)
        && Arc::ptr_eq(&left.branch_guards, &right.branch_guards)
}
fn empty_payload_eq(
    left: &GeneratedResidualAffineBranchEmptyCertificate,
    right: &GeneratedResidualAffineBranchEmptyCertificate,
) -> bool {
    left.schema == right.schema
        && source_payload_eq(&left.source, &right.source)
        && left.reason == right.reason
        && left.limits == right.limits
        && left.stats == right.stats
}
fn unavailable_payload_eq(
    left: &GeneratedResidualAffineBranchUnavailableRowCertificate,
    right: &GeneratedResidualAffineBranchUnavailableRowCertificate,
) -> bool {
    left.schema == right.schema
        && source_payload_eq(&left.source, &right.source)
        && left.reason == right.reason
        && left
            .partial_relation
            .has_identical_guard_provenance(&right.partial_relation)
        && left.base_assumptions == right.base_assumptions
        && left.condition_witnesses == right.condition_witnesses
        && left.limits == right.limits
        && left.stats == right.stats
}
fn retained_payload_eq(
    left: &GeneratedResidualAffineBranchBoundParametricRelation,
    right: &GeneratedResidualAffineBranchBoundParametricRelation,
) -> bool {
    left.schema == right.schema
        && source_payload_eq(&left.source, &right.source)
        && left.relation_manifest == right.relation_manifest
        && left.base_assumptions == right.base_assumptions
        && left.condition_witnesses == right.condition_witnesses
        && left.limits == right.limits
        && left.stats == right.stats
}

fn derived_target_row_id(
    branch: &ResidualAffineBranchSystemCertificate,
    source_row_ordinal: usize,
    translation: &IndexShift,
    max_bytes: usize,
) -> Result<ParametricRowId, GeneratedResidualAffineBranchBoundRelationError> {
    let expected =
        derived_target_row_label_len(branch, source_row_ordinal, translation, max_bytes)?;
    let prefix = concat!(
        "rustred-generated-residual-affine-branch-bound-relation-v1",
        "|case="
    );
    let mut label = String::new();
    label.try_reserve_exact(expected).map_err(|_| {
        GeneratedResidualAffineBranchBoundRelationError::AllocationFailure {
            resource: "target row label bytes",
            requested: expected,
        }
    })?;
    write!(
        &mut label,
        "{prefix}{}|work={}|terminal={}|row={source_row_ordinal}|translation=[",
        branch.source_cover().source_case().value(),
        branch.source_cover().source_work_item_ordinal(),
        branch.ready_terminal_ordinal()
    )
    .map_err(|_| GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch)?;
    for (ordinal, value) in translation.values().iter().enumerate() {
        if ordinal != 0 {
            label.push(',');
        }
        write!(&mut label, "{value}")
            .map_err(|_| GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch)?;
    }
    label.push(']');
    if label.len() != expected {
        return Err(GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch);
    }
    Ok(ParametricRowId::Derived {
        label: label.into(),
    })
}

fn derived_target_row_label_len(
    branch: &ResidualAffineBranchSystemCertificate,
    source_row_ordinal: usize,
    translation: &IndexShift,
    max_bytes: usize,
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    let prefix = concat!(
        "rustred-generated-residual-affine-branch-bound-relation-v1",
        "|case="
    );
    let expected = [
        prefix.len(),
        decimal_digits_u64(branch.source_cover().source_case().value()),
        "|work=".len(),
        decimal_digits_usize(branch.source_cover().source_work_item_ordinal()),
        "|terminal=".len(),
        decimal_digits_usize(branch.ready_terminal_ordinal()),
        "|row=".len(),
        decimal_digits_usize(source_row_ordinal),
        "|translation=[".len(),
        translation
            .values()
            .iter()
            .enumerate()
            .try_fold(0usize, |total, (ordinal, value)| {
                checked_add(
                    "target row label bytes",
                    total,
                    signed_decimal_digits(*value) + usize::from(ordinal != 0),
                )
            })?,
        "]".len(),
    ]
    .into_iter()
    .try_fold(0usize, |total, value| {
        checked_add("target row label bytes", total, value)
    })?;
    check_limit("target row label bytes", expected, max_bytes)?;
    Ok(expected)
}

fn evaluate_affine_point(
    map: &crate::ResidualAffineIntegerMap,
    free_values: &[i64],
    limits: GeneratedResidualAffineBranchConcreteSpecializationLimits,
) -> Result<Vec<i64>, GeneratedResidualAffineBranchBoundRelationError> {
    let mut ambient = Vec::new();
    try_reserve("concrete ambient point", &mut ambient, map.ambient_arity())?;
    for row in 0..map.ambient_arity() {
        let constant = map
            .constant(row)
            .ok_or(GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch)?;
        check_limit(
            "concrete affine integer bits",
            integer_bits(constant)?,
            limits.max_affine_integer_bits,
        )?;
        let mut value = constant.clone();
        for (free_ordinal, &position) in map.free_positions().iter().enumerate() {
            let coefficient = map
                .linear_coefficient(row, position)
                .ok_or(GeneratedResidualAffineBranchBoundRelationError::ReplayMismatch)?;
            let free = Integer::from(free_values[free_ordinal]);
            if coefficient.is_zero() || free.is_zero() {
                continue;
            }
            let product_bit_bound = checked_add(
                "concrete affine integer bits",
                integer_bits(coefficient)?,
                integer_bits(&free)?,
            )?;
            check_limit(
                "concrete affine integer bits",
                product_bit_bound,
                limits.max_affine_integer_bits,
            )?;
            let contribution = coefficient * free;
            check_limit(
                "concrete affine integer bits",
                integer_bits(&contribution)?,
                limits.max_affine_integer_bits,
            )?;
            // Preflight the largest possible sum before GMP is allowed to
            // allocate it. Signed addition needs at most max(a, b) + 1 bits.
            let sum_bit_bound = checked_add(
                "concrete affine integer bits",
                integer_bits(&value)?.max(integer_bits(&contribution)?),
                1,
            )?;
            check_limit(
                "concrete affine integer bits",
                sum_bit_bound,
                limits.max_affine_integer_bits,
            )?;
            value += contribution;
            check_limit(
                "concrete affine integer bits",
                integer_bits(&value)?,
                limits.max_affine_integer_bits,
            )?;
        }
        check_limit(
            "concrete affine integer bits",
            integer_bits(&value)?,
            limits.max_affine_integer_bits,
        )?;
        ambient.push(value.to_i64().ok_or(
            GeneratedResidualAffineBranchBoundRelationError::ConcreteAffineValueOutOfRange {
                position: row,
            },
        )?);
    }
    Ok(ambient)
}

fn integer_bits(value: &Integer) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| {
        GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow {
            resource: "concrete affine integer bits",
        }
    })
}

fn copy_shift(
    shift: &IndexShift,
) -> Result<IndexShift, GeneratedResidualAffineBranchBoundRelationError> {
    Ok(IndexShift::try_new(
        shift.values().iter().copied(),
        shift.arity(),
    )?)
}
fn copy_shift_payload(
    shift: &IndexShift,
) -> Result<Box<[i64]>, GeneratedResidualAffineBranchBoundRelationError> {
    let mut values = Vec::new();
    try_reserve("term-denominator origin shift", &mut values, shift.arity())?;
    values.extend_from_slice(shift.values());
    Ok(values.into_boxed_slice())
}
fn decimal_digits_usize(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}
fn decimal_digits_u64(mut value: u64) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}
fn signed_decimal_digits(value: i64) -> usize {
    decimal_digits_u64(value.unsigned_abs()) + usize::from(value < 0)
}

fn try_reserve<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    let requested = checked_add(resource, values.len(), additional)?;
    values.try_reserve_exact(additional).map_err(|_| {
        GeneratedResidualAffineBranchBoundRelationError::AllocationFailure {
            resource,
            requested,
        }
    })
}
fn remaining(
    resource: &'static str,
    limit: usize,
    used: usize,
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    limit.checked_sub(used).ok_or(
        GeneratedResidualAffineBranchBoundRelationError::ResourceLimit {
            resource,
            requested: used,
            limit,
        },
    )
}
fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    left.checked_add(right)
        .ok_or(GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow { resource })
}
fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    left.checked_mul(right)
        .ok_or(GeneratedResidualAffineBranchBoundRelationError::ResourceCountOverflow { resource })
}
fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedResidualAffineBranchBoundRelationError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}
fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedResidualAffineBranchBoundRelationError> {
    if requested > limit {
        Err(
            GeneratedResidualAffineBranchBoundRelationError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ResidualAffineIntegerSystemCertificate, ResidualAffineIntegerSystemInputRow,
        ResidualAffineIntegerSystemLimits, ResidualAffinePrimitiveRow,
    };

    #[test]
    fn replay_schema_helper_accepts_only_the_current_schema() {
        assert_eq!(
            validate_replay_schema(GENERATED_RESIDUAL_AFFINE_BRANCH_BOUND_RELATION_V1_SCHEMA),
            Ok(())
        );
        assert_eq!(
            validate_replay_schema("rustred-generated-residual-affine-branch-bound-relation-v0"),
            Err(GeneratedResidualAffineBranchBoundRelationError::SchemaMismatch)
        );
    }

    #[test]
    fn affine_constant_bits_accept_exact_and_reject_one_below() {
        let primitive = ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
            vec![Integer::from(8), Integer::from(-1)],
            2,
            64,
            1_000,
        )
        .expect("8 - n_0 is a canonical primitive affine row");
        let input = ResidualAffineIntegerSystemInputRow::try_new(primitive, vec![0], 1)
            .expect("single structural-locus lineage is valid");
        let certificate = ResidualAffineIntegerSystemCertificate::compile(
            1,
            &[input],
            ResidualAffineIntegerSystemLimits::default(),
        )
        .expect("unit-pivot affine row compiles");
        let map = certificate
            .affine_map()
            .expect("consistent unit-pivot row has an affine map");

        let exact = GeneratedResidualAffineBranchConcreteSpecializationLimits {
            max_affine_integer_bits: 4,
            ..GeneratedResidualAffineBranchConcreteSpecializationLimits::default()
        };
        assert_eq!(evaluate_affine_point(map, &[], exact), Ok(vec![8]));

        let one_below = GeneratedResidualAffineBranchConcreteSpecializationLimits {
            max_affine_integer_bits: 3,
            ..GeneratedResidualAffineBranchConcreteSpecializationLimits::default()
        };
        assert!(matches!(
            evaluate_affine_point(map, &[], one_below),
            Err(
                GeneratedResidualAffineBranchBoundRelationError::ResourceLimit {
                    resource: "concrete affine integer bits",
                    requested: 4,
                    limit: 3,
                }
            )
        ));
    }
}
