//! Parametric identities restricted to one replayable unit-affine residual locus.
//!
//! The private inner relation uses a shift `q` to mean `J(F(t)+q)`.  It is
//! therefore deliberately unavailable through the public API: without the
//! attached affine-map certificate it is not a global identity on `K(n)`.

use std::fmt;
use std::fmt::Write as _;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::domains::SelfRing;
use symbolica::prelude::{Integer, PrintOptions, PrintState};

use crate::parametric_coefficient::{
    ResidualUnitAffineCoefficientComposition, ResidualUnitAffineConditionClass,
};
use crate::{
    ConcreteRelation, GuardOrigin, IndexShift, ParametricArithmeticLimits,
    ParametricCoefficientContext, ParametricNonZeroCondition, ParametricRelation,
    ParametricRelationError, ParametricRowId, ResidualUnitAffineCompositionError,
    ResidualUnitAffineCompositionPlanLimits, ResidualUnitAffineIndexMapCertificate,
    ResidualUnitAffinePolynomialCompositionLimits, ResidualUnitAffinePolynomialCompositionStats,
    SymbolicPolynomialPredicateKind,
};

pub const AFFINE_LOCUS_BOUND_PARAMETRIC_RELATION_V1_SCHEMA: &str =
    "rustred-affine-locus-bound-parametric-relation-v1";

/// Per-row limits for translation followed by simultaneous affine composition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AffineLocusBoundRelationLimits {
    pub translation: ParametricArithmeticLimits,
    pub plan: ResidualUnitAffineCompositionPlanLimits,
    pub composition: ResidualUnitAffinePolynomialCompositionLimits,
    pub max_terms: usize,
    pub max_source_guards: usize,
    pub max_polynomial_compositions: usize,
    pub max_base_assumptions: usize,
    pub max_retained_guards: usize,
    pub max_target_row_label_bytes: usize,
    /// Aggregate count of numerator, denominator, and guard translations.
    pub max_translation_polynomials: usize,
    /// Conservative row-wide products of the per-polynomial translation
    /// allowances. Translation does not yet return a measured work census.
    pub max_total_translation_source_term_allowance: usize,
    pub max_total_translation_output_term_allowance: usize,
    pub max_total_translation_power_operation_allowance: usize,
    pub max_total_translation_integer_bit_allowance: usize,
    pub max_total_source_terms: usize,
    pub max_total_source_exponent_entries: usize,
    pub max_total_expanded_contributions: usize,
    pub max_total_output_terms: usize,
    pub max_total_output_exponent_entry_bound: usize,
    pub max_total_power_calls: usize,
    pub max_total_native_power_heap_pairs: usize,
    pub max_total_multiplication_term_pairs: usize,
    pub max_total_addition_term_visits: usize,
    pub max_total_integer_bit_work: usize,
    pub max_total_guard_origin_retained_bytes: usize,
    pub max_total_normalization_input_term_pairs: usize,
    pub max_retained_terms: usize,
    pub max_retained_bytes: usize,
}

impl Default for AffineLocusBoundRelationLimits {
    fn default() -> Self {
        Self {
            translation: ParametricArithmeticLimits::default(),
            plan: ResidualUnitAffineCompositionPlanLimits::default(),
            composition: ResidualUnitAffinePolynomialCompositionLimits::default(),
            max_terms: 4_000_000,
            max_source_guards: 4_000_000,
            max_polynomial_compositions: 12_000_000,
            max_base_assumptions: 4_000_000,
            max_retained_guards: 4_000_000,
            max_target_row_label_bytes: 1024 * 1024,
            max_translation_polynomials: 12_000_000,
            max_total_translation_source_term_allowance: 2_000_000_000,
            max_total_translation_output_term_allowance: 2_000_000_000,
            max_total_translation_power_operation_allowance: 2_000_000_000,
            max_total_translation_integer_bit_allowance: 2_000_000_000,
            max_total_source_terms: 32_000_000,
            max_total_source_exponent_entries: 536_870_912,
            max_total_expanded_contributions: 32_000_000,
            max_total_output_terms: 32_000_000,
            max_total_output_exponent_entry_bound: 536_870_912,
            max_total_power_calls: 536_870_912,
            max_total_native_power_heap_pairs: 1_073_741_824,
            max_total_multiplication_term_pairs: 1_073_741_824,
            max_total_addition_term_visits: 1_073_741_824,
            max_total_integer_bit_work: 2_147_483_648,
            max_total_guard_origin_retained_bytes: 2_147_483_648,
            max_total_normalization_input_term_pairs: 128_000_000,
            max_retained_terms: 32_000_000,
            max_retained_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AffineLocusBoundRelationStats {
    source_terms: usize,
    source_guards: usize,
    translated_terms: usize,
    polynomial_compositions: usize,
    base_assumptions: usize,
    retained_guards: usize,
    target_row_label_bytes: usize,
    translation_polynomials: usize,
    translation_source_term_allowance: usize,
    translation_output_term_allowance: usize,
    translation_power_operation_allowance: usize,
    translation_integer_bit_allowance: usize,
    composition_source_terms: usize,
    composition_source_exponent_entries: usize,
    expanded_contribution_bound: usize,
    composition_output_terms: usize,
    composition_output_exponent_entry_bound: usize,
    composition_output_exponent_entries: usize,
    power_calls: usize,
    native_power_heap_pair_bound: usize,
    multiplication_term_pair_bound: usize,
    addition_term_visit_bound: usize,
    largest_kronecker_exponent_bits: usize,
    largest_integer_coefficient_bit_bound: usize,
    native_integer_bit_work_bound: usize,
    durable_guard_terms: usize,
    durable_guard_exponent_entries: usize,
    durable_guard_integer_bit_payload: usize,
    guard_origin_retained_bytes: usize,
    integer_bit_work_bound: usize,
    normalization_input_term_pairs: usize,
    retained_terms: usize,
    retained_bytes: usize,
}

macro_rules! affine_relation_stats_getters {
    ($($field:ident),* $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )* };
}

impl AffineLocusBoundRelationStats {
    affine_relation_stats_getters!(
        source_terms,
        source_guards,
        translated_terms,
        polynomial_compositions,
        base_assumptions,
        retained_guards,
        target_row_label_bytes,
        translation_polynomials,
        translation_source_term_allowance,
        translation_output_term_allowance,
        translation_power_operation_allowance,
        translation_integer_bit_allowance,
        composition_source_terms,
        composition_source_exponent_entries,
        expanded_contribution_bound,
        composition_output_terms,
        composition_output_exponent_entry_bound,
        composition_output_exponent_entries,
        power_calls,
        native_power_heap_pair_bound,
        multiplication_term_pair_bound,
        addition_term_visit_bound,
        largest_kronecker_exponent_bits,
        largest_integer_coefficient_bit_bound,
        native_integer_bit_work_bound,
        durable_guard_terms,
        durable_guard_exponent_entries,
        durable_guard_integer_bit_payload,
        guard_origin_retained_bytes,
        integer_bit_work_bound,
        normalization_input_term_pairs,
        retained_terms,
        retained_bytes,
    );
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AffineLocusBaseAssumption {
    condition: ParametricNonZeroCondition,
}

impl AffineLocusBaseAssumption {
    pub fn condition(&self) -> &ParametricNonZeroCondition {
        &self.condition
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AffineLocusUnavailableReason {
    SourceGuardComposesToZero {
        guard_ordinal: usize,
    },
    SourceTermDenominatorComposesToZero {
        term_ordinal: usize,
        shift: IndexShift,
    },
}

#[derive(Clone, Debug)]
pub struct AffineLocusUnavailableRowCertificate {
    schema: &'static str,
    source: Arc<ParametricRelation>,
    translation: IndexShift,
    target_row_id: ParametricRowId,
    affine_map: Arc<ResidualUnitAffineIndexMapCertificate>,
    reason: AffineLocusUnavailableReason,
    limits: AffineLocusBoundRelationLimits,
    stats: AffineLocusBoundRelationStats,
}

impl AffineLocusUnavailableRowCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn source(&self) -> &Arc<ParametricRelation> {
        &self.source
    }
    pub fn translation(&self) -> &IndexShift {
        &self.translation
    }
    pub fn target_row_id(&self) -> &ParametricRowId {
        &self.target_row_id
    }
    pub fn affine_map(&self) -> &Arc<ResidualUnitAffineIndexMapCertificate> {
        &self.affine_map
    }
    pub fn reason(&self) -> &AffineLocusUnavailableReason {
        &self.reason
    }
    pub const fn limits(&self) -> AffineLocusBoundRelationLimits {
        self.limits
    }
    pub const fn stats(&self) -> AffineLocusBoundRelationStats {
        self.stats
    }

    pub fn replay(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), AffineLocusBoundRelationError> {
        if self.schema != AFFINE_LOCUS_BOUND_PARAMETRIC_RELATION_V1_SCHEMA {
            return Err(AffineLocusBoundRelationError::SchemaMismatch);
        }
        let replayed = compile_caught(
            context,
            self.source.clone(),
            self.translation.clone(),
            self.target_row_id.clone(),
            self.affine_map.clone(),
            self.limits,
        )?;
        match replayed {
            AffineLocusBoundRelationCompilation::Unavailable(other)
                if unavailable_payload_eq(self, &other) =>
            {
                Ok(())
            }
            _ => Err(AffineLocusBoundRelationError::ReplayMismatch),
        }
    }
}

/// Exact affine-locus identity whose raw `J(F(t)+q)` relation stays private.
#[derive(Clone, Debug)]
pub struct AffineLocusBoundParametricRelation {
    schema: &'static str,
    source: Arc<ParametricRelation>,
    translation: IndexShift,
    target_row_id: ParametricRowId,
    affine_map: Arc<ResidualUnitAffineIndexMapCertificate>,
    relation: ParametricRelation,
    base_assumptions: Box<[AffineLocusBaseAssumption]>,
    limits: AffineLocusBoundRelationLimits,
    stats: AffineLocusBoundRelationStats,
}

impl AffineLocusBoundParametricRelation {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn source(&self) -> &Arc<ParametricRelation> {
        &self.source
    }
    pub fn translation(&self) -> &IndexShift {
        &self.translation
    }
    pub fn target_row_id(&self) -> &ParametricRowId {
        &self.target_row_id
    }
    pub fn affine_map(&self) -> &Arc<ResidualUnitAffineIndexMapCertificate> {
        &self.affine_map
    }
    pub fn base_assumptions(&self) -> &[AffineLocusBaseAssumption] {
        &self.base_assumptions
    }
    pub const fn limits(&self) -> AffineLocusBoundRelationLimits {
        self.limits
    }
    pub const fn stats(&self) -> AffineLocusBoundRelationStats {
        self.stats
    }

    pub fn replay(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), AffineLocusBoundRelationError> {
        if self.schema != AFFINE_LOCUS_BOUND_PARAMETRIC_RELATION_V1_SCHEMA {
            return Err(AffineLocusBoundRelationError::SchemaMismatch);
        }
        let replayed = compile_caught(
            context,
            self.source.clone(),
            self.translation.clone(),
            self.target_row_id.clone(),
            self.affine_map.clone(),
            self.limits,
        )?;
        match replayed {
            AffineLocusBoundRelationCompilation::Retained(other)
                if retained_payload_eq(self, &other) =>
            {
                Ok(())
            }
            _ => Err(AffineLocusBoundRelationError::ReplayMismatch),
        }
    }

    /// Safely query the private row at one point of the certified free lattice.
    pub fn specialize_at_free_values(
        &self,
        context: &ParametricCoefficientContext,
        free_values: &[i64],
        limits: AffineLocusConcreteSpecializationLimits,
    ) -> Result<ConcreteRelation, AffineLocusBoundRelationError> {
        catch_unwind(AssertUnwindSafe(|| {
            self.specialize_at_free_values_inner(context, free_values, limits)
        }))
        .map_err(|_| {
            AffineLocusBoundRelationError::Composition(
                ResidualUnitAffineCompositionError::SymbolicaPanic {
                    stage: "affine-locus concrete specialization",
                },
            )
        })?
    }

    fn specialize_at_free_values_inner(
        &self,
        context: &ParametricCoefficientContext,
        free_values: &[i64],
        limits: AffineLocusConcreteSpecializationLimits,
    ) -> Result<ConcreteRelation, AffineLocusBoundRelationError> {
        validate_common(context, &self.source, &self.translation, &self.affine_map)?;
        check_limit(
            "concrete affine free positions",
            self.affine_map.free_positions().len(),
            limits.max_free_positions,
        )?;
        if free_values.len() != self.affine_map.free_positions().len() {
            return Err(AffineLocusBoundRelationError::ConcreteFreeValueArity {
                expected: self.affine_map.free_positions().len(),
                actual: free_values.len(),
            });
        }
        check_limit(
            "concrete affine ambient positions",
            self.affine_map.ambient_arity(),
            limits.max_ambient_positions,
        )?;
        let ambient = evaluate_affine_point(&self.affine_map, free_values, limits)?;
        validate_source_case(context, &self.affine_map, &ambient, limits)?;

        // The compiled retained-size proof bounds this clone.  Reattaching the
        // separately classified base assumptions is essential: specializing
        // only the private row would silently enlarge its domain.
        check_limit(
            "concrete query clone terms",
            self.stats.retained_terms,
            limits.max_query_clone_terms,
        )?;
        check_limit(
            "concrete query clone guards",
            self.stats.retained_guards,
            limits.max_query_clone_guards,
        )?;
        check_limit(
            "concrete query clone bytes",
            self.stats.retained_bytes,
            limits.max_query_clone_bytes,
        )?;
        let mut query = self.relation.clone();
        for assumption in &self.base_assumptions {
            query.add_guarded_nonzero_condition_with_limits(
                context,
                assumption.condition.clone(),
                limits.arithmetic,
            )?;
        }
        query
            .specialize(context, &ambient, limits.arithmetic)
            .map_err(Into::into)
    }

    pub(crate) fn relation_for_affine_reelimination(&self) -> &ParametricRelation {
        &self.relation
    }
}

#[derive(Clone, Debug)]
pub enum AffineLocusBoundRelationCompilation {
    Retained(AffineLocusBoundParametricRelation),
    Unavailable(AffineLocusUnavailableRowCertificate),
}

pub struct AffineLocusBoundRelationCompiler;

impl AffineLocusBoundRelationCompiler {
    pub fn compile(
        context: &ParametricCoefficientContext,
        source: Arc<ParametricRelation>,
        translation: IndexShift,
        target_row_id: ParametricRowId,
        affine_map: Arc<ResidualUnitAffineIndexMapCertificate>,
        limits: AffineLocusBoundRelationLimits,
    ) -> Result<AffineLocusBoundRelationCompilation, AffineLocusBoundRelationError> {
        let compiled = compile_caught(
            context,
            source,
            translation,
            target_row_id,
            affine_map,
            limits,
        )?;
        match &compiled {
            AffineLocusBoundRelationCompilation::Retained(row) => row.replay(context)?,
            AffineLocusBoundRelationCompilation::Unavailable(row) => row.replay(context)?,
        }
        Ok(compiled)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AffineLocusConcreteSpecializationLimits {
    pub arithmetic: ParametricArithmeticLimits,
    pub max_free_positions: usize,
    pub max_ambient_positions: usize,
    pub max_affine_integer_bits: usize,
    pub max_source_case_predicates: usize,
    pub max_source_case_predicate_terms: usize,
    pub max_query_clone_terms: usize,
    pub max_query_clone_guards: usize,
    pub max_query_clone_bytes: usize,
}

impl Default for AffineLocusConcreteSpecializationLimits {
    fn default() -> Self {
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            max_free_positions: 4096,
            max_ambient_positions: 8192,
            max_affine_integer_bits: 1_000_000,
            max_source_case_predicates: 1_000_000,
            max_source_case_predicate_terms: 16_000_000,
            max_query_clone_terms: 32_000_000,
            max_query_clone_guards: 4_000_000,
            max_query_clone_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AffineLocusBoundRelationError {
    SchemaMismatch,
    ReplayMismatch,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    ConcreteFreeValueArity {
        expected: usize,
        actual: usize,
    },
    ConcreteAffineValueOutOfRange {
        position: usize,
    },
    ConcretePointOutsideSourceOrthant,
    ConcretePointOutsideSourceCase {
        predicate_ordinal: usize,
    },
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
    Composition(ResidualUnitAffineCompositionError),
    Relation(ParametricRelationError),
}

impl fmt::Display for AffineLocusBoundRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => {
                formatter.write_str("affine-locus-bound relation schema mismatch")
            }
            Self::ReplayMismatch => {
                formatter.write_str("affine-locus-bound relation does not replay")
            }
            Self::WrongContext => {
                formatter.write_str("affine-locus-bound relation belongs to another K(n) context")
            }
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "affine-locus-bound relation has arity {actual}, expected {expected}"
            ),
            Self::ConcreteFreeValueArity { expected, actual } => write!(
                formatter,
                "affine-locus query has {actual} free values, expected {expected}"
            ),
            Self::ConcreteAffineValueOutOfRange { position } => write!(
                formatter,
                "affine-locus ambient component {position} does not fit i64"
            ),
            Self::ConcretePointOutsideSourceOrthant => {
                formatter.write_str("affine-locus query lies outside the certified source orthant")
            }
            Self::ConcretePointOutsideSourceCase { predicate_ordinal } => write!(
                formatter,
                "affine-locus query violates source-case predicate {predicate_ordinal}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "affine-locus {resource} needs {requested} units, exceeding limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "affine-locus {resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "affine-locus {resource} could not allocate {requested} bounded entries"
            ),
            Self::Composition(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AffineLocusBoundRelationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Composition(error) => Some(error),
            Self::Relation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ResidualUnitAffineCompositionError> for AffineLocusBoundRelationError {
    fn from(value: ResidualUnitAffineCompositionError) -> Self {
        Self::Composition(value)
    }
}

impl From<ParametricRelationError> for AffineLocusBoundRelationError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

fn compile_inner(
    context: &ParametricCoefficientContext,
    source: Arc<ParametricRelation>,
    translation: IndexShift,
    target_row_id: ParametricRowId,
    affine_map: Arc<ResidualUnitAffineIndexMapCertificate>,
    limits: AffineLocusBoundRelationLimits,
) -> Result<AffineLocusBoundRelationCompilation, AffineLocusBoundRelationError> {
    validate_common(context, &source, &translation, &affine_map)?;
    check_limit(
        "source relation terms",
        source.terms().len(),
        limits.max_terms,
    )?;
    check_limit(
        "source relation guards",
        source.guarded_nonzero_conditions().len(),
        limits.max_source_guards,
    )?;
    let target_row_label_bytes = row_id_stable_bytes(&target_row_id)?;
    check_limit(
        "target row label bytes",
        target_row_label_bytes,
        limits.max_target_row_label_bytes,
    )?;
    let translation_polynomials = checked_add(
        "translation polynomials",
        checked_mul("translation polynomials", source.terms().len(), 2)?,
        source.guarded_nonzero_conditions().len(),
    )?;
    check_limit(
        "translation polynomials",
        translation_polynomials,
        limits.max_translation_polynomials,
    )?;
    let translation_source_term_allowance = checked_mul(
        "translation source-term allowance",
        translation_polynomials,
        limits.translation.max_source_terms,
    )?;
    check_limit(
        "translation source-term allowance",
        translation_source_term_allowance,
        limits.max_total_translation_source_term_allowance,
    )?;
    let translation_output_term_allowance = checked_mul(
        "translation output-term allowance",
        translation_polynomials,
        limits.translation.max_output_terms,
    )?;
    check_limit(
        "translation output-term allowance",
        translation_output_term_allowance,
        limits.max_total_translation_output_term_allowance,
    )?;
    let translation_power_operation_allowance = checked_mul(
        "translation power-operation allowance",
        translation_polynomials,
        limits.translation.max_specialization_power_operations,
    )?;
    check_limit(
        "translation power-operation allowance",
        translation_power_operation_allowance,
        limits.max_total_translation_power_operation_allowance,
    )?;
    let translation_integer_bit_allowance = checked_mul(
        "translation integer-bit allowance",
        translation_polynomials,
        limits.translation.max_specialization_integer_bits,
    )?;
    check_limit(
        "translation integer-bit allowance",
        translation_integer_bit_allowance,
        limits.max_total_translation_integer_bit_allowance,
    )?;

    // Translation can add at most one distinct input-denominator guard per
    // injectively shifted source term. Bound that retained shape before the
    // complete-row API allocates it.
    let translated_guard_upper_bound = checked_add(
        "translated relation guard upper bound",
        source.guarded_nonzero_conditions().len(),
        source.terms().len(),
    )?;
    check_limit(
        "translated relation guard upper bound",
        translated_guard_upper_bound,
        limits.max_source_guards,
    )?;
    let translation_guard_origin_copy_bound =
        translation_guard_origin_copy_bound(&source, &translation, &target_row_id)?;
    check_limit(
        "guard origin retained bytes",
        translation_guard_origin_copy_bound,
        limits.max_total_guard_origin_retained_bytes,
    )?;

    // This is the only production order: the complete relation is translated
    // before a composition plan is even constructed.
    let translated = source.translated(
        context,
        &translation,
        target_row_id.clone(),
        limits.translation,
    )?;
    check_limit(
        "translated relation terms",
        translated.terms().len(),
        limits.max_terms,
    )?;
    check_limit(
        "translated relation guards",
        translated.guarded_nonzero_conditions().len(),
        limits.max_source_guards,
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
    let plan =
        context.compile_residual_unit_affine_composition_plan(affine_map.clone(), limits.plan)?;

    let mut relation =
        ParametricRelation::new(source.family_fingerprint(), target_row_id.clone(), context);
    let mut base_assumptions = Vec::new();
    base_assumptions
        .try_reserve(
            translated
                .guarded_nonzero_conditions()
                .len()
                .min(limits.max_base_assumptions),
        )
        .map_err(|_| AffineLocusBoundRelationError::AllocationFailure {
            resource: "base assumptions",
            requested: translated
                .guarded_nonzero_conditions()
                .len()
                .min(limits.max_base_assumptions),
        })?;
    let mut stats = AffineLocusBoundRelationStats {
        source_terms: source.terms().len(),
        source_guards: source.guarded_nonzero_conditions().len(),
        translated_terms: translated.terms().len(),
        target_row_label_bytes,
        translation_polynomials,
        translation_source_term_allowance,
        translation_output_term_allowance,
        translation_power_operation_allowance,
        translation_integer_bit_allowance,
        guard_origin_retained_bytes: translation_guard_origin_copy_bound,
        ..Default::default()
    };
    let locator = (
        affine_map.source_case().value(),
        affine_map.source_equality_predicate_ordinal(),
        affine_map.bound_position(),
    );

    for (guard_ordinal, guard) in translated.guarded_nonzero_conditions().iter().enumerate() {
        let call_limits = remaining_composition_limits(limits, stats)?;
        let mapped = context.compose_nonzero_condition_on_residual_unit_affine_map(
            guard,
            &plan,
            call_limits,
        )?;
        let (mapped_class, mapped_stats, mapped_origin_bytes) = mapped.into_parts();
        stats.polynomial_compositions =
            checked_add("polynomial compositions", stats.polynomial_compositions, 1)?;
        check_limit(
            "polynomial compositions",
            stats.polynomial_compositions,
            limits.max_polynomial_compositions,
        )?;
        consume_polynomial_stats(&mut stats, mapped_stats, context, limits)?;
        stats.guard_origin_retained_bytes = checked_add(
            "guard origin retained bytes",
            stats.guard_origin_retained_bytes,
            mapped_origin_bytes,
        )?;
        check_limit(
            "guard origin retained bytes",
            stats.guard_origin_retained_bytes,
            limits.max_total_guard_origin_retained_bytes,
        )?;
        match mapped_class {
            ResidualUnitAffineConditionClass::Unsatisfiable => {
                return unavailable(
                    source,
                    translation,
                    target_row_id,
                    affine_map,
                    AffineLocusUnavailableReason::SourceGuardComposesToZero { guard_ordinal },
                    limits,
                    stats,
                );
            }
            ResidualUnitAffineConditionClass::NonzeroIntegerConstant => {}
            ResidualUnitAffineConditionClass::BaseAssumption(mut condition) => {
                let relation_origin = GuardOrigin::RelationResidualUnitAffineSubstitution {
                    source_row: source.row_id().guard_identity(),
                    target_row: target_row_id.guard_identity(),
                    source_case: locator.0,
                    predicate_ordinal: locator.1,
                    bound_position: locator.2,
                };
                let attached_origin = GuardOrigin::RelationConditionAttached {
                    row: target_row_id.guard_identity(),
                };
                charge_wrapper_guard_origin_copy_bound(
                    &mut stats,
                    &condition,
                    &[relation_origin.clone(), attached_origin.clone()],
                    limits,
                )?;
                condition
                    .add_origin_with_limit(relation_origin, limits.composition.max_guard_origins)
                    .map_err(ResidualUnitAffineCompositionError::from)?;
                condition
                    .add_origin_with_limit(attached_origin, limits.composition.max_guard_origins)
                    .map_err(ResidualUnitAffineCompositionError::from)?;
                insert_base_assumption(&mut base_assumptions, condition, limits)?;
            }
            ResidualUnitAffineConditionClass::IndexDependent(condition) => {
                let relation_origin = GuardOrigin::RelationResidualUnitAffineSubstitution {
                    source_row: source.row_id().guard_identity(),
                    target_row: target_row_id.guard_identity(),
                    source_case: locator.0,
                    predicate_ordinal: locator.1,
                    bound_position: locator.2,
                };
                let attached_origin = GuardOrigin::RelationConditionAttached {
                    row: target_row_id.guard_identity(),
                };
                charge_wrapper_guard_origin_copy_bound(
                    &mut stats,
                    &condition,
                    &[relation_origin.clone(), attached_origin.clone()],
                    limits,
                )?;
                let condition = condition
                    .try_with_origin(relation_origin, limits.composition.max_guard_origins)
                    .and_then(|condition| {
                        condition
                            .try_with_origin(attached_origin, limits.composition.max_guard_origins)
                    })
                    .map_err(ResidualUnitAffineCompositionError::from)?;
                relation.add_guarded_nonzero_condition_with_limits(
                    context,
                    condition,
                    relation_arithmetic_limits(limits),
                )?;
                check_retained_guard_limit(&relation, &base_assumptions, limits)?;
            }
        }
    }

    for (term_ordinal, (shift, coefficient)) in translated.terms().iter().enumerate() {
        let call_limits = remaining_composition_limits(limits, stats)?;
        let mapped = context.compose_coefficient_on_residual_unit_affine_map(
            coefficient,
            &plan,
            call_limits,
        )?;
        stats.polynomial_compositions =
            checked_add("polynomial compositions", stats.polynomial_compositions, 2)?;
        check_limit(
            "polynomial compositions",
            stats.polynomial_compositions,
            limits.max_polynomial_compositions,
        )?;
        consume_coefficient_stats(&mut stats, mapped.stats(), context, limits)?;
        let ResidualUnitAffineCoefficientComposition::Available(mapped) = mapped else {
            return unavailable(
                source,
                translation,
                target_row_id,
                affine_map,
                AffineLocusUnavailableReason::SourceTermDenominatorComposesToZero {
                    term_ordinal,
                    shift: shift.clone(),
                },
                limits,
                stats,
            );
        };
        let expected_stats = mapped.stats();
        let (mapped_value, mapped_denominator, returned_stats) = mapped.into_parts();
        if returned_stats != expected_stats {
            return Err(AffineLocusBoundRelationError::ReplayMismatch);
        }
        if let Some(denominator) = mapped_denominator {
            let target_guard_row = target_row_id.guard_identity();
            let term_origin_bytes =
                GuardOrigin::residual_unit_affine_term_denominator_retained_byte_bound(
                    &target_guard_row,
                    shift.arity(),
                )
                .ok_or(AffineLocusBoundRelationError::ResourceCountOverflow {
                    resource: "guard origin retained bytes",
                })?;
            let attached_origin = GuardOrigin::RelationConditionAttached {
                row: target_guard_row.clone(),
            };
            charge_wrapper_term_denominator_origin_copy_bound(
                &mut stats,
                &denominator,
                term_origin_bytes,
                &attached_origin,
                limits,
            )?;
            let mut shift_payload = Vec::new();
            shift_payload
                .try_reserve_exact(shift.arity())
                .map_err(|_| AffineLocusBoundRelationError::AllocationFailure {
                    resource: "affine-locus term-denominator origin shift",
                    requested: shift.arity(),
                })?;
            shift_payload.extend_from_slice(shift.values());
            let term_origin = GuardOrigin::RelationResidualUnitAffineSubstitutionTermDenominator {
                row: target_guard_row,
                shift: shift_payload.into_boxed_slice(),
                source_case: locator.0,
                predicate_ordinal: locator.1,
                bound_position: locator.2,
            };
            if term_origin.retained_byte_bound() != Some(term_origin_bytes) {
                return Err(AffineLocusBoundRelationError::ReplayMismatch);
            }
            let condition = denominator
                .try_with_origin(term_origin, limits.composition.max_guard_origins)
                .and_then(|condition| {
                    condition.try_with_origin(attached_origin, limits.composition.max_guard_origins)
                })
                .map_err(ResidualUnitAffineCompositionError::from)?;
            classify_denominator(
                context,
                &mut relation,
                &mut base_assumptions,
                condition,
                &target_row_id,
                limits,
            )?;
        }
        relation.insert_prevalidated_distinct_term_without_denominator_discovery(
            context,
            shift.clone(),
            mapped_value,
            relation_arithmetic_limits(limits),
        )?;
    }

    stats.base_assumptions = base_assumptions.len();
    stats.retained_guards = checked_add(
        "retained guards",
        relation.guarded_nonzero_conditions().len(),
        base_assumptions.len(),
    )?;
    check_limit(
        "retained guards",
        stats.retained_guards,
        limits.max_retained_guards,
    )?;
    stats.retained_terms = retained_term_count(&relation, &base_assumptions)?;
    check_limit(
        "retained terms",
        stats.retained_terms,
        limits.max_retained_terms,
    )?;
    stats.retained_bytes = retained_bytes(
        &relation,
        &base_assumptions,
        &translation,
        &target_row_id,
        limits.max_retained_bytes,
    )?;

    Ok(AffineLocusBoundRelationCompilation::Retained(
        AffineLocusBoundParametricRelation {
            schema: AFFINE_LOCUS_BOUND_PARAMETRIC_RELATION_V1_SCHEMA,
            source,
            translation,
            target_row_id,
            affine_map,
            relation,
            base_assumptions: base_assumptions.into_boxed_slice(),
            limits,
            stats,
        },
    ))
}

fn compile_caught(
    context: &ParametricCoefficientContext,
    source: Arc<ParametricRelation>,
    translation: IndexShift,
    target_row_id: ParametricRowId,
    affine_map: Arc<ResidualUnitAffineIndexMapCertificate>,
    limits: AffineLocusBoundRelationLimits,
) -> Result<AffineLocusBoundRelationCompilation, AffineLocusBoundRelationError> {
    catch_unwind(AssertUnwindSafe(|| {
        compile_inner(
            context,
            source,
            translation,
            target_row_id,
            affine_map,
            limits,
        )
    }))
    .map_err(|_| {
        AffineLocusBoundRelationError::Composition(
            ResidualUnitAffineCompositionError::SymbolicaPanic {
                stage: "affine-locus-bound relation compilation",
            },
        )
    })?
}

fn validate_common(
    context: &ParametricCoefficientContext,
    source: &ParametricRelation,
    translation: &IndexShift,
    map: &ResidualUnitAffineIndexMapCertificate,
) -> Result<(), AffineLocusBoundRelationError> {
    if source.context_fingerprint() != context.fingerprint()
        || map.context_fingerprint() != context.fingerprint()
    {
        return Err(AffineLocusBoundRelationError::WrongContext);
    }
    if source.arity() != context.index_count() {
        return Err(AffineLocusBoundRelationError::WrongArity {
            expected: context.index_count(),
            actual: source.arity(),
        });
    }
    if translation.arity() != context.index_count() {
        return Err(AffineLocusBoundRelationError::WrongArity {
            expected: context.index_count(),
            actual: translation.arity(),
        });
    }
    if map.ambient_arity() != context.index_count() {
        return Err(AffineLocusBoundRelationError::WrongArity {
            expected: context.index_count(),
            actual: map.ambient_arity(),
        });
    }
    map.replay(context)
        .map_err(ResidualUnitAffineCompositionError::from)?;
    Ok(())
}

fn classify_denominator(
    context: &ParametricCoefficientContext,
    relation: &mut ParametricRelation,
    assumptions: &mut Vec<AffineLocusBaseAssumption>,
    mut condition: ParametricNonZeroCondition,
    target_row_id: &ParametricRowId,
    limits: AffineLocusBoundRelationLimits,
) -> Result<(), AffineLocusBoundRelationError> {
    if context
        .polynomial_depends_on_indices_with_limits(
            condition.polynomial(),
            limits.composition.exact_algebra,
        )
        .map_err(ResidualUnitAffineCompositionError::from)?
    {
        relation.add_guarded_nonzero_condition_with_limits(
            context,
            condition,
            relation_arithmetic_limits(limits),
        )?;
    } else {
        condition
            .add_origin_with_limit(
                GuardOrigin::RelationConditionAttached {
                    row: target_row_id.guard_identity(),
                },
                limits.composition.max_guard_origins,
            )
            .map_err(ResidualUnitAffineCompositionError::from)?;
        insert_base_assumption(assumptions, condition, limits)?;
    }
    check_retained_guard_limit(relation, assumptions, limits)
}

fn insert_base_assumption(
    assumptions: &mut Vec<AffineLocusBaseAssumption>,
    condition: ParametricNonZeroCondition,
    limits: AffineLocusBoundRelationLimits,
) -> Result<(), AffineLocusBoundRelationError> {
    if let Some(existing) = assumptions
        .iter_mut()
        .find(|entry| entry.condition.polynomial() == condition.polynomial())
    {
        existing
            .condition
            .merge_origins_from(&condition, limits.composition.max_guard_origins)
            .map_err(ResidualUnitAffineCompositionError::from)?;
    } else {
        let requested = checked_add("base assumptions", assumptions.len(), 1)?;
        check_limit("base assumptions", requested, limits.max_base_assumptions)?;
        assumptions.push(AffineLocusBaseAssumption { condition });
    }
    Ok(())
}

fn check_retained_guard_limit(
    relation: &ParametricRelation,
    assumptions: &[AffineLocusBaseAssumption],
    limits: AffineLocusBoundRelationLimits,
) -> Result<(), AffineLocusBoundRelationError> {
    let requested = checked_add(
        "retained guards",
        relation.guarded_nonzero_conditions().len(),
        assumptions.len(),
    )?;
    check_limit("retained guards", requested, limits.max_retained_guards)
}

/// Conservative provenance allocation/copy work performed by complete-row
/// translation before affine composition starts.  It covers the source-set
/// clone, reconstruction with `IndexTranslation`, relation attachment, and
/// the possible translated input-denominator guard for every term.
fn translation_guard_origin_copy_bound(
    source: &ParametricRelation,
    translation: &IndexShift,
    target_row_id: &ParametricRowId,
) -> Result<usize, AffineLocusBoundRelationError> {
    let source_row = source.row_id().guard_identity();
    let target_row = target_row_id.guard_identity();
    let index_translation = GuardOrigin::index_translation_retained_byte_bound(translation.arity())
        .ok_or(AffineLocusBoundRelationError::ResourceCountOverflow {
            resource: "guard origin retained bytes",
        })?;
    let relation_translation = GuardOrigin::relation_translation_retained_byte_bound(
        &source_row,
        &target_row,
        translation.arity(),
    )
    .ok_or(AffineLocusBoundRelationError::ResourceCountOverflow {
        resource: "guard origin retained bytes",
    })?;
    let attached = GuardOrigin::relation_attached_retained_byte_bound(&target_row).ok_or(
        AffineLocusBoundRelationError::ResourceCountOverflow {
            resource: "guard origin retained bytes",
        },
    )?;
    let mut total = 0usize;
    for condition in source.guarded_nonzero_conditions() {
        let mut source_origins = 0usize;
        for origin in condition.origins() {
            source_origins = checked_add(
                "guard origin retained bytes",
                source_origins,
                origin.retained_byte_bound().ok_or(
                    AffineLocusBoundRelationError::ResourceCountOverflow {
                        resource: "guard origin retained bytes",
                    },
                )?,
            )?;
        }
        // First copy the source B-tree, then rebuild a condition containing
        // that provenance plus the index-translation origin.  Charge one
        // further complete set for the worst-case duplicate-polynomial merge
        // into the translated relation.
        total = checked_add("guard origin retained bytes", total, source_origins)?;
        total = checked_add("guard origin retained bytes", total, source_origins)?;
        total = checked_add("guard origin retained bytes", total, source_origins)?;
        total = checked_add("guard origin retained bytes", total, index_translation)?;
        total = checked_add("guard origin retained bytes", total, index_translation)?;
        total = checked_add("guard origin retained bytes", total, relation_translation)?;
        total = checked_add("guard origin retained bytes", total, relation_translation)?;
        total = checked_add("guard origin retained bytes", total, attached)?;
        total = checked_add("guard origin retained bytes", total, attached)?;
    }
    let translated_term_origin =
        GuardOrigin::residual_unit_affine_term_denominator_retained_byte_bound(
            &target_row,
            translation.arity(),
        )
        .ok_or(AffineLocusBoundRelationError::ResourceCountOverflow {
            resource: "guard origin retained bytes",
        })?;
    let per_term_once = checked_add(
        "guard origin retained bytes",
        translated_term_origin,
        attached,
    )?;
    let per_term = checked_mul("guard origin retained bytes", per_term_once, 2)?;
    total = checked_add(
        "guard origin retained bytes",
        total,
        checked_mul(
            "guard origin retained bytes",
            source.terms().len(),
            per_term,
        )?,
    )?;
    Ok(total)
}

fn charge_guard_origin_bytes(
    stats: &mut AffineLocusBoundRelationStats,
    bytes: usize,
    limits: AffineLocusBoundRelationLimits,
) -> Result<(), AffineLocusBoundRelationError> {
    stats.guard_origin_retained_bytes = checked_add(
        "guard origin retained bytes",
        stats.guard_origin_retained_bytes,
        bytes,
    )?;
    check_limit(
        "guard origin retained bytes",
        stats.guard_origin_retained_bytes,
        limits.max_total_guard_origin_retained_bytes,
    )
}

/// Charge the additional allocation/copy work performed when the relation
/// wrapper clones one mapped guard and attaches its relation-level origins.
fn charge_wrapper_guard_origin_copy_bound(
    stats: &mut AffineLocusBoundRelationStats,
    condition: &ParametricNonZeroCondition,
    added_origins: &[GuardOrigin],
    limits: AffineLocusBoundRelationLimits,
) -> Result<(), AffineLocusBoundRelationError> {
    let mut bytes = 0usize;
    for origin in condition.origins().iter().chain(added_origins) {
        bytes = checked_add(
            "guard origin retained bytes",
            bytes,
            origin.retained_byte_bound().ok_or(
                AffineLocusBoundRelationError::ResourceCountOverflow {
                    resource: "guard origin retained bytes",
                },
            )?,
        )?;
    }
    // If an equal polynomial is already retained, insertion clones the
    // missing origins into that condition.  Charging a second complete set
    // covers both the wrapper clone and this worst-case merge path.
    bytes = checked_mul("guard origin retained bytes", bytes, 2)?;
    charge_guard_origin_bytes(stats, bytes, limits)
}

fn charge_wrapper_term_denominator_origin_copy_bound(
    stats: &mut AffineLocusBoundRelationStats,
    condition: &ParametricNonZeroCondition,
    term_origin_bytes: usize,
    attached_origin: &GuardOrigin,
    limits: AffineLocusBoundRelationLimits,
) -> Result<(), AffineLocusBoundRelationError> {
    let mut bytes = term_origin_bytes;
    bytes = checked_add(
        "guard origin retained bytes",
        bytes,
        attached_origin.retained_byte_bound().ok_or(
            AffineLocusBoundRelationError::ResourceCountOverflow {
                resource: "guard origin retained bytes",
            },
        )?,
    )?;
    for origin in condition.origins() {
        bytes = checked_add(
            "guard origin retained bytes",
            bytes,
            origin.retained_byte_bound().ok_or(
                AffineLocusBoundRelationError::ResourceCountOverflow {
                    resource: "guard origin retained bytes",
                },
            )?,
        )?;
    }
    bytes = checked_mul("guard origin retained bytes", bytes, 2)?;
    charge_guard_origin_bytes(stats, bytes, limits)
}

fn relation_arithmetic_limits(
    limits: AffineLocusBoundRelationLimits,
) -> ParametricArithmeticLimits {
    ParametricArithmeticLimits {
        exact_algebra: limits.composition.exact_algebra,
        max_source_terms: limits.composition.max_source_terms,
        max_output_terms: limits.composition.max_output_terms,
        max_specialization_power_operations: limits.composition.max_power_calls,
        max_specialization_integer_bits: limits.composition.max_integer_coefficient_bits,
        max_guard_origins: limits.composition.max_guard_origins,
    }
}

fn remaining_composition_limits(
    limits: AffineLocusBoundRelationLimits,
    stats: AffineLocusBoundRelationStats,
) -> Result<ResidualUnitAffinePolynomialCompositionLimits, AffineLocusBoundRelationError> {
    let mut remaining = limits.composition;
    remaining.max_source_terms = remaining_count(
        "composition source terms",
        limits.max_total_source_terms,
        stats.composition_source_terms,
    )?
    .min(remaining.max_source_terms);
    remaining.max_source_exponent_entries = remaining_count(
        "composition source exponent entries",
        limits.max_total_source_exponent_entries,
        stats.composition_source_exponent_entries,
    )?
    .min(remaining.max_source_exponent_entries);
    remaining.max_expanded_contributions = remaining_count(
        "expanded contributions",
        limits.max_total_expanded_contributions,
        stats.expanded_contribution_bound,
    )?
    .min(remaining.max_expanded_contributions);
    remaining.max_output_terms = remaining_count(
        "composition output terms",
        limits.max_total_output_terms,
        stats.composition_output_terms,
    )?
    .min(remaining.max_output_terms);
    remaining.max_output_exponent_entries = remaining_count(
        "composition output exponent-entry bound",
        limits.max_total_output_exponent_entry_bound,
        stats.composition_output_exponent_entry_bound,
    )?
    .min(remaining.max_output_exponent_entries);
    remaining.max_power_calls = remaining_count(
        "composition power calls",
        limits.max_total_power_calls,
        stats.power_calls,
    )?
    .min(remaining.max_power_calls);
    remaining.max_native_power_heap_pairs = remaining_count(
        "native power heap pairs",
        limits.max_total_native_power_heap_pairs,
        stats.native_power_heap_pair_bound,
    )?
    .min(remaining.max_native_power_heap_pairs);
    remaining.max_multiplication_term_pairs = remaining_count(
        "multiplication term pairs",
        limits.max_total_multiplication_term_pairs,
        stats.multiplication_term_pair_bound,
    )?
    .min(remaining.max_multiplication_term_pairs);
    remaining.max_addition_term_visits = remaining_count(
        "addition term visits",
        limits.max_total_addition_term_visits,
        stats.addition_term_visit_bound,
    )?
    .min(remaining.max_addition_term_visits);
    remaining.max_integer_bit_work = remaining_count(
        "integer bit work",
        limits.max_total_integer_bit_work,
        stats.integer_bit_work_bound,
    )?
    .min(remaining.max_integer_bit_work);
    remaining.max_guard_origin_retained_bytes = remaining_count(
        "guard origin retained bytes",
        limits.max_total_guard_origin_retained_bytes,
        stats.guard_origin_retained_bytes,
    )?
    .min(remaining.max_guard_origin_retained_bytes);
    remaining.max_normalization_input_term_pairs = remaining_count(
        "normalization input term pairs",
        limits.max_total_normalization_input_term_pairs,
        stats.normalization_input_term_pairs,
    )?
    .min(remaining.max_normalization_input_term_pairs);
    Ok(remaining)
}

fn consume_coefficient_stats(
    aggregate: &mut AffineLocusBoundRelationStats,
    stats: crate::ResidualUnitAffineCoefficientCompositionStats,
    context: &ParametricCoefficientContext,
    limits: AffineLocusBoundRelationLimits,
) -> Result<(), AffineLocusBoundRelationError> {
    consume_polynomial_stats(aggregate, stats.aggregate(), context, limits)?;
    aggregate.durable_guard_terms = checked_add(
        "durable denominator guard terms",
        aggregate.durable_guard_terms,
        stats.durable_guard_terms(),
    )?;
    aggregate.durable_guard_exponent_entries = checked_add(
        "durable denominator guard exponent entries",
        aggregate.durable_guard_exponent_entries,
        stats.durable_guard_exponent_entries(),
    )?;
    aggregate.durable_guard_integer_bit_payload = checked_add(
        "durable denominator guard integer-bit payload",
        aggregate.durable_guard_integer_bit_payload,
        stats.durable_guard_integer_bit_payload(),
    )?;
    aggregate.guard_origin_retained_bytes = checked_add(
        "guard origin retained bytes",
        aggregate.guard_origin_retained_bytes,
        stats.durable_guard_origin_retained_bytes(),
    )?;
    check_limit(
        "guard origin retained bytes",
        aggregate.guard_origin_retained_bytes,
        limits.max_total_guard_origin_retained_bytes,
    )?;
    let additional_durable_copy_work = stats
        .total_integer_bit_work_bound()
        .checked_sub(stats.aggregate().integer_bit_work_bound())
        .ok_or(AffineLocusBoundRelationError::ReplayMismatch)?;
    if additional_durable_copy_work != stats.durable_guard_integer_bit_payload() {
        return Err(AffineLocusBoundRelationError::ReplayMismatch);
    }
    aggregate.integer_bit_work_bound = checked_add(
        "integer bit work",
        aggregate.integer_bit_work_bound,
        additional_durable_copy_work,
    )?;
    check_limit(
        "integer bit work",
        aggregate.integer_bit_work_bound,
        limits.max_total_integer_bit_work,
    )?;
    aggregate.normalization_input_term_pairs = checked_add(
        "normalization input term pairs",
        aggregate.normalization_input_term_pairs,
        stats.normalization_input_term_pairs(),
    )?;
    check_limit(
        "normalization input term pairs",
        aggregate.normalization_input_term_pairs,
        limits.max_total_normalization_input_term_pairs,
    )
}

fn consume_polynomial_stats(
    aggregate: &mut AffineLocusBoundRelationStats,
    stats: ResidualUnitAffinePolynomialCompositionStats,
    context: &ParametricCoefficientContext,
    limits: AffineLocusBoundRelationLimits,
) -> Result<(), AffineLocusBoundRelationError> {
    macro_rules! add_limited {
        ($field:ident, $value:expr, $limit:expr, $name:literal) => {{
            aggregate.$field = checked_add($name, aggregate.$field, $value)?;
            check_limit($name, aggregate.$field, $limit)?;
        }};
    }
    add_limited!(
        composition_source_terms,
        stats.source_terms(),
        limits.max_total_source_terms,
        "composition source terms"
    );
    add_limited!(
        composition_source_exponent_entries,
        stats.source_exponent_entries(),
        limits.max_total_source_exponent_entries,
        "composition source exponent entries"
    );
    add_limited!(
        expanded_contribution_bound,
        stats.expanded_contribution_bound(),
        limits.max_total_expanded_contributions,
        "expanded contributions"
    );
    add_limited!(
        composition_output_terms,
        stats.output_terms(),
        limits.max_total_output_terms,
        "composition output terms"
    );
    let _ = context;
    add_limited!(
        composition_output_exponent_entry_bound,
        stats.output_exponent_entry_bound(),
        limits.max_total_output_exponent_entry_bound,
        "composition output exponent-entry bound"
    );
    aggregate.composition_output_exponent_entries = checked_add(
        "composition output exponent entries",
        aggregate.composition_output_exponent_entries,
        stats.output_exponent_entries(),
    )?;
    add_limited!(
        power_calls,
        stats.power_calls(),
        limits.max_total_power_calls,
        "composition power calls"
    );
    add_limited!(
        native_power_heap_pair_bound,
        stats.native_power_heap_pair_bound(),
        limits.max_total_native_power_heap_pairs,
        "native power heap pairs"
    );
    add_limited!(
        multiplication_term_pair_bound,
        stats.multiplication_term_pair_bound(),
        limits.max_total_multiplication_term_pairs,
        "multiplication term pairs"
    );
    add_limited!(
        addition_term_visit_bound,
        stats.addition_term_visit_bound(),
        limits.max_total_addition_term_visits,
        "addition term visits"
    );
    aggregate.largest_kronecker_exponent_bits = aggregate
        .largest_kronecker_exponent_bits
        .max(stats.largest_kronecker_exponent_bits());
    aggregate.largest_integer_coefficient_bit_bound = aggregate
        .largest_integer_coefficient_bit_bound
        .max(stats.largest_integer_coefficient_bit_bound());
    aggregate.native_integer_bit_work_bound = checked_add(
        "native integer bit work",
        aggregate.native_integer_bit_work_bound,
        stats.native_integer_bit_work_bound(),
    )?;
    add_limited!(
        integer_bit_work_bound,
        stats.integer_bit_work_bound(),
        limits.max_total_integer_bit_work,
        "integer bit work"
    );
    Ok(())
}

fn unavailable(
    source: Arc<ParametricRelation>,
    translation: IndexShift,
    target_row_id: ParametricRowId,
    affine_map: Arc<ResidualUnitAffineIndexMapCertificate>,
    reason: AffineLocusUnavailableReason,
    limits: AffineLocusBoundRelationLimits,
    mut stats: AffineLocusBoundRelationStats,
) -> Result<AffineLocusBoundRelationCompilation, AffineLocusBoundRelationError> {
    stats.retained_bytes = unavailable_retained_bytes(
        &translation,
        &target_row_id,
        &reason,
        limits.max_retained_bytes,
    )?;
    Ok(AffineLocusBoundRelationCompilation::Unavailable(
        AffineLocusUnavailableRowCertificate {
            schema: AFFINE_LOCUS_BOUND_PARAMETRIC_RELATION_V1_SCHEMA,
            source,
            translation,
            target_row_id,
            affine_map,
            reason,
            limits,
            stats,
        },
    ))
}

fn retained_payload_common_eq(
    left_source: &Arc<ParametricRelation>,
    right_source: &Arc<ParametricRelation>,
    left_translation: &IndexShift,
    right_translation: &IndexShift,
    left_row: &ParametricRowId,
    right_row: &ParametricRowId,
    left_map: &ResidualUnitAffineIndexMapCertificate,
    right_map: &ResidualUnitAffineIndexMapCertificate,
) -> bool {
    left_source.has_identical_guard_provenance(right_source)
        && left_translation == right_translation
        && left_row == right_row
        && left_map.payload_eq(right_map)
}

fn retained_payload_eq(
    left: &AffineLocusBoundParametricRelation,
    right: &AffineLocusBoundParametricRelation,
) -> bool {
    left.schema == right.schema
        && retained_payload_common_eq(
            &left.source,
            &right.source,
            &left.translation,
            &right.translation,
            &left.target_row_id,
            &right.target_row_id,
            &left.affine_map,
            &right.affine_map,
        )
        && left
            .relation
            .has_identical_guard_provenance(&right.relation)
        && left.base_assumptions == right.base_assumptions
        && left.limits == right.limits
        && left.stats == right.stats
}

fn unavailable_payload_eq(
    left: &AffineLocusUnavailableRowCertificate,
    right: &AffineLocusUnavailableRowCertificate,
) -> bool {
    left.schema == right.schema
        && retained_payload_common_eq(
            &left.source,
            &right.source,
            &left.translation,
            &right.translation,
            &left.target_row_id,
            &right.target_row_id,
            &left.affine_map,
            &right.affine_map,
        )
        && left.reason == right.reason
        && left.limits == right.limits
        && left.stats == right.stats
}

fn evaluate_affine_point(
    map: &ResidualUnitAffineIndexMapCertificate,
    free_values: &[i64],
    limits: AffineLocusConcreteSpecializationLimits,
) -> Result<Vec<i64>, AffineLocusBoundRelationError> {
    let mut output = Vec::new();
    output.try_reserve_exact(map.ambient_arity()).map_err(|_| {
        AffineLocusBoundRelationError::AllocationFailure {
            resource: "concrete ambient point",
            requested: map.ambient_arity(),
        }
    })?;
    for position in 0..map.ambient_arity() {
        let constant = map
            .constant(position)
            .ok_or(AffineLocusBoundRelationError::ReplayMismatch)?;
        check_integer_bits(
            "concrete affine integer bits",
            constant,
            limits.max_affine_integer_bits,
        )?;
        let mut value = constant.clone();
        for (free_ordinal, &free) in free_values.iter().enumerate() {
            let coefficient = map
                .linear_coefficient(position, free_ordinal)
                .ok_or(AffineLocusBoundRelationError::ReplayMismatch)?;
            if coefficient.is_zero() || free == 0 {
                continue;
            }
            let prospective = checked_add(
                "concrete affine integer bits",
                integer_bits(coefficient)?,
                integer_bits(&Integer::from(free))?,
            )?;
            check_limit(
                "concrete affine integer bits",
                prospective,
                limits.max_affine_integer_bits,
            )?;
            value += coefficient * Integer::from(free);
            check_integer_bits(
                "concrete affine integer bits",
                &value,
                limits.max_affine_integer_bits,
            )?;
        }
        output.push(
            value
                .to_i64()
                .ok_or(AffineLocusBoundRelationError::ConcreteAffineValueOutOfRange { position })?,
        );
    }
    Ok(output)
}

fn validate_source_case(
    context: &ParametricCoefficientContext,
    map: &ResidualUnitAffineIndexMapCertificate,
    ambient: &[i64],
    limits: AffineLocusConcreteSpecializationLimits,
) -> Result<(), AffineLocusBoundRelationError> {
    let partition = map.source().source_partition();
    if !partition
        .orthant()
        .contains_integer_point(ambient)
        .map_err(|_| AffineLocusBoundRelationError::ReplayMismatch)?
    {
        return Err(AffineLocusBoundRelationError::ConcretePointOutsideSourceOrthant);
    }
    let case = partition
        .case(map.source_case())
        .ok_or(AffineLocusBoundRelationError::ReplayMismatch)?;
    check_limit(
        "source-case predicates",
        case.predicates().len(),
        limits.max_source_case_predicates,
    )?;
    let mut terms = 0usize;
    for (predicate_ordinal, predicate) in case.predicates().iter().enumerate() {
        terms = checked_add(
            "source-case predicate terms",
            terms,
            predicate.polynomial().term_count(),
        )?;
        check_limit(
            "source-case predicate terms",
            terms,
            limits.max_source_case_predicate_terms,
        )?;
        let value = context
            .specialize_polynomial(predicate.polynomial(), ambient, limits.arithmetic)
            .map_err(ResidualUnitAffineCompositionError::from)?;
        let accepts = match predicate.kind() {
            SymbolicPolynomialPredicateKind::EqualZero => value.is_zero(),
            SymbolicPolynomialPredicateKind::NonZero => !value.is_zero(),
        };
        if !accepts {
            return Err(
                AffineLocusBoundRelationError::ConcretePointOutsideSourceCase { predicate_ordinal },
            );
        }
    }
    Ok(())
}

fn retained_term_count(
    relation: &ParametricRelation,
    assumptions: &[AffineLocusBaseAssumption],
) -> Result<usize, AffineLocusBoundRelationError> {
    let mut count = 0usize;
    for coefficient in relation.terms().values() {
        count = checked_add(
            "retained terms",
            count,
            coefficient.raw().numerator.nterms(),
        )?;
        count = checked_add(
            "retained terms",
            count,
            coefficient.raw().denominator.nterms(),
        )?;
    }
    for condition in relation.guarded_nonzero_conditions() {
        count = checked_add("retained terms", count, condition.polynomial().term_count())?;
    }
    for assumption in assumptions {
        count = checked_add(
            "retained terms",
            count,
            assumption.condition.polynomial().term_count(),
        )?;
    }
    Ok(count)
}

fn retained_bytes(
    relation: &ParametricRelation,
    assumptions: &[AffineLocusBaseAssumption],
    translation: &IndexShift,
    row_id: &ParametricRowId,
    limit: usize,
) -> Result<usize, AffineLocusBoundRelationError> {
    let mut writer = BoundedByteCounter::new(limit);
    let result = (|| -> fmt::Result {
        writer.write_str(AFFINE_LOCUS_BOUND_PARAMETRIC_RELATION_V1_SCHEMA)?;
        writer.write_str(relation.family_fingerprint())?;
        writer.write_str(relation.context_fingerprint())?;
        row_id.guard_identity().write_stable(&mut writer)?;
        for value in translation.values() {
            write!(&mut writer, "{value},")?;
        }
        for (shift, coefficient) in relation.terms() {
            for value in shift.values() {
                write!(&mut writer, "{value},")?;
            }
            coefficient
                .raw()
                .numerator
                .format(&PrintOptions::full(), PrintState::default(), &mut writer)
                .map(|_| ())?;
            coefficient
                .raw()
                .denominator
                .format(&PrintOptions::full(), PrintState::default(), &mut writer)
                .map(|_| ())?;
        }
        for condition in relation.guarded_nonzero_conditions() {
            write_condition(&mut writer, condition)?;
        }
        for assumption in assumptions {
            write_condition(&mut writer, &assumption.condition)?;
        }
        Ok(())
    })();
    if result.is_err() {
        return Err(writer.error("retained bytes"));
    }
    let pointer_bytes = checked_mul("retained bytes", size_of::<Arc<()>>(), 4)?;
    let bytes = checked_add("retained bytes", writer.bytes, pointer_bytes)?;
    check_limit("retained bytes", bytes, limit)?;
    Ok(bytes)
}

fn write_condition(
    writer: &mut BoundedByteCounter,
    condition: &ParametricNonZeroCondition,
) -> fmt::Result {
    condition
        .polynomial()
        .raw()
        .format(&PrintOptions::full(), PrintState::default(), writer)
        .map(|_| ())?;
    for origin in condition.origins() {
        origin.write_stable(writer)?;
    }
    Ok(())
}

fn unavailable_retained_bytes(
    translation: &IndexShift,
    row_id: &ParametricRowId,
    reason: &AffineLocusUnavailableReason,
    limit: usize,
) -> Result<usize, AffineLocusBoundRelationError> {
    let mut writer = BoundedByteCounter::new(limit);
    let result = (|| -> fmt::Result {
        writer.write_str(AFFINE_LOCUS_BOUND_PARAMETRIC_RELATION_V1_SCHEMA)?;
        row_id.guard_identity().write_stable(&mut writer)?;
        for value in translation.values() {
            write!(&mut writer, "{value},")?;
        }
        match reason {
            AffineLocusUnavailableReason::SourceGuardComposesToZero { guard_ordinal } => {
                write!(&mut writer, "guard:{guard_ordinal}")
            }
            AffineLocusUnavailableReason::SourceTermDenominatorComposesToZero {
                term_ordinal,
                shift,
            } => {
                write!(&mut writer, "term:{term_ordinal}:")?;
                for value in shift.values() {
                    write!(&mut writer, "{value},")?;
                }
                Ok(())
            }
        }
    })();
    if result.is_err() {
        return Err(writer.error("retained bytes"));
    }
    let bytes = checked_add(
        "retained bytes",
        writer.bytes,
        checked_mul("retained bytes", size_of::<Arc<()>>(), 2)?,
    )?;
    check_limit("retained bytes", bytes, limit)?;
    Ok(bytes)
}

struct BoundedByteCounter {
    bytes: usize,
    limit: usize,
    overflowed: bool,
}

impl BoundedByteCounter {
    fn new(limit: usize) -> Self {
        Self {
            bytes: 0,
            limit,
            overflowed: false,
        }
    }
    fn error(&self, resource: &'static str) -> AffineLocusBoundRelationError {
        if self.overflowed {
            AffineLocusBoundRelationError::ResourceCountOverflow { resource }
        } else {
            AffineLocusBoundRelationError::ResourceLimit {
                resource,
                requested: self.bytes.max(self.limit.saturating_add(1)),
                limit: self.limit,
            }
        }
    }
}

impl fmt::Write for BoundedByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(requested) = self.bytes.checked_add(value.len()) else {
            self.overflowed = true;
            return Err(fmt::Error);
        };
        self.bytes = requested;
        if requested > self.limit {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

fn row_id_stable_bytes(row_id: &ParametricRowId) -> Result<usize, AffineLocusBoundRelationError> {
    let mut writer = BoundedByteCounter::new(usize::MAX);
    row_id
        .guard_identity()
        .write_stable(&mut writer)
        .map_err(|_| writer.error("target row label bytes"))?;
    Ok(writer.bytes)
}

fn integer_bits(value: &Integer) -> Result<usize, AffineLocusBoundRelationError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| AffineLocusBoundRelationError::ResourceCountOverflow {
        resource: "concrete affine integer bits",
    })
}

fn check_integer_bits(
    resource: &'static str,
    value: &Integer,
    limit: usize,
) -> Result<(), AffineLocusBoundRelationError> {
    check_limit(resource, integer_bits(value)?, limit)
}

fn remaining_count(
    resource: &'static str,
    limit: usize,
    consumed: usize,
) -> Result<usize, AffineLocusBoundRelationError> {
    limit
        .checked_sub(consumed)
        .ok_or(AffineLocusBoundRelationError::ResourceLimit {
            resource,
            requested: consumed,
            limit,
        })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, AffineLocusBoundRelationError> {
    left.checked_add(right)
        .ok_or(AffineLocusBoundRelationError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, AffineLocusBoundRelationError> {
    left.checked_mul(right)
        .ok_or(AffineLocusBoundRelationError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), AffineLocusBoundRelationError> {
    if requested > limit {
        Err(AffineLocusBoundRelationError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
