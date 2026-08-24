//! Matcher-bound target-local compilation for generated affine `WhenBad`.
//!
//! This is the authenticated outer owner of the relation-free structural
//! partition in [`crate::generated_residual_affine_when_bad`].  The only
//! production authority accepted here is an immutable pivot/target matcher
//! plus persisted pivot and target ordinals.  In particular, this module has
//! no constructor which accepts an arbitrary relation.
//!
//! The first implementation slice below deliberately isolates binding and
//! private-row authentication.  Later slices consume the private authenticated
//! input to compile conditions, affine boundary pullbacks, signed descent,
//! the direct bad formula, and the relative partition transcript.

use std::collections::BTreeSet;
use std::fmt;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use symbolica::domains::SelfRing;
use symbolica::domains::integer::Integer;

use crate::generated_residual_affine_condition_accumulator::{
    GeneratedResidualAffineConditionAccumulatorCertificate,
    GeneratedResidualAffineConditionAccumulatorError,
    GeneratedResidualAffineConditionAccumulatorLimits,
    GeneratedResidualAffineConditionAccumulatorStats, GeneratedResidualAffineConditionInput,
    GeneratedResidualAffineConditionInputClass, GeneratedResidualAffineConditionInputTranscript,
    GeneratedResidualAffineConditionRelationTerm, GeneratedResidualAffineConditionScope,
    GeneratedResidualAffineConditionSourceLocator, accumulate_generated_residual_affine_conditions,
};
use crate::generated_residual_affine_when_bad::{
    AFFINE_WHEN_BAD_RELATIVE_PARTITION_V1_SCHEMA, AffineWhenBadFormulaClause,
    AffineWhenBadInheritedTruth, AffineWhenBadRelativeCase, AffineWhenBadRelativeCaseError,
    AffineWhenBadRelativeCaseId, AffineWhenBadRelativeCaseLimits,
    AffineWhenBadRelativeLeafClassification, AffineWhenBadRelativeLeafDisposition,
    AffineWhenBadRelativePartitionCertificate, AffineWhenBadRelativePartitionCompiler,
    AffineWhenBadRelativePredicate, AffineWhenBadRelativeProblem,
};
use crate::generated_residual_affine_when_bad_descent::{
    GeneratedResidualAffineWhenBadDescentCompilation, GeneratedResidualAffineWhenBadDescentError,
    GeneratedResidualAffineWhenBadDescentReady, GeneratedResidualAffineWhenBadDescentStats,
    GeneratedResidualAffineWhenBadDescentUnsupported,
    GeneratedResidualAffineWhenBadDescentUnsupportedReason,
    compile_generated_residual_affine_when_bad_descent,
};
use crate::generated_residual_affine_when_bad_pullback_gate::{
    AffineBoundaryPullbackClass, AffineWhenBadNumeratorGateClass,
    GeneratedResidualAffineWhenBadPullbackGateCertificate,
    GeneratedResidualAffineWhenBadPullbackGateCompilation,
    GeneratedResidualAffineWhenBadPullbackGateError,
    GeneratedResidualAffineWhenBadPullbackGateLimits,
    GeneratedResidualAffineWhenBadPullbackGateStats,
    compile_generated_residual_affine_when_bad_pullback_gate_table,
};
use crate::generated_sector_affine_effective_coverage::GeneratedSectorAffineSealedLeafAuthorization;
use crate::parametric_coefficient::{
    ParametricPolynomialAssociateLimits, ParametricPolynomialSpecializationPreflight,
};
use crate::parametric_relation::{
    ParametricConcreteSpecializationLimits, ParametricConcreteSpecializationPreflight,
};
use crate::when_bad::{WhenBadBoundaryHazardKind, WhenBadCoreError, WhenBadDescentComponent};
use crate::{
    AffineParametricOrderingError, AffineParametricOrderingLimits,
    AffineStartParametricEliminationOrdering, ConcreteRelation, GeneratedResidualAffineCaseLocator,
    GeneratedResidualAffineInventoryTerminalOutcome,
    GeneratedResidualAffinePivotTargetMatchingCertificate,
    GeneratedResidualAffinePivotTargetMatchingError, GeneratedResidualAffinePivotTargetOutcome,
    GuardOrigin, IndexShift, IntegralFamily, IntegralOrderingPolicy, ParametricArithmeticLimits,
    ParametricCoefficientContext, ParametricCoefficientError, ParametricNonZeroCondition,
    ParametricPolynomial, ParametricRelation, ParametricRelationError,
    ResidualAffineBranchGuardCompositionCertificate, ResidualAffineBranchSystemCertificate,
    ResidualAffineBranchSystemOutcome, ResidualProductLocusBooleanCoverCertificate,
    ResidualUnitAffineCompositionError, ResidualUnitAffineCompositionPlanLimits,
    ResidualUnitAffinePolynomialCompositionLimits, SectorFoundationError, SectorMask,
    SymbolicPolynomialPredicateKind,
};

/// Stable schema for one matcher-bound target-local affine `WhenBad` result.
pub const GENERATED_RESIDUAL_AFFINE_WHEN_BAD_V1_SCHEMA: &str =
    "rustred-generated-residual-affine-when-bad-v1";

/// Nested and aggregate budgets for one exact `(matcher, pivot, target)`
/// compilation.
///
/// Scalar fields are aggregate ceilings.  A later phase must derive the
/// effective per-operation nested limit from the unspent aggregate budget;
/// it must not reset a nested allowance for every RHS or pullback.
/// Recursively replayed matcher work remains governed by `matcher.limits()`
/// and `matcher.stats()` and is not double-charged here; all post-replay local
/// selection, comparison, validation, and copy work is charged below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedResidualAffineWhenBadLimits {
    pub arithmetic: ParametricArithmeticLimits,
    pub target_ordering: AffineParametricOrderingLimits,
    pub composition_plan: ResidualUnitAffineCompositionPlanLimits,
    pub polynomial_composition: ResidualUnitAffinePolynomialCompositionLimits,
    pub relative_partition: AffineWhenBadRelativeCaseLimits,
    pub max_family_fingerprint_bytes: usize,
    pub max_context_fingerprint_bytes: usize,
    pub max_scope_fingerprint_comparison_bytes: usize,
    pub max_ambient_arity: usize,
    pub max_free_positions: usize,
    pub max_map_entries_inspected: usize,
    pub max_matcher_outcomes_inspected: usize,
    pub max_matching_target_references_inspected: usize,
    pub max_inventory_terminals_inspected: usize,
    pub max_target_constant_comparison_entries: usize,
    pub max_target_constant_comparison_integer_bits: usize,
    pub max_private_relation_terms: usize,
    pub max_private_relation_guards: usize,
    pub max_private_relation_origins: usize,
    pub max_private_relation_manifest_bytes: usize,
    pub max_private_relation_shift_components: usize,
    pub max_rhs_terms: usize,
    pub max_descent_witnesses: usize,
    pub max_descent_witness_components: usize,
    pub max_target_guard_entries: usize,
    pub max_relation_guard_condition_inputs: usize,
    pub max_coefficient_denominator_condition_inputs: usize,
    pub max_condition_inputs: usize,
    pub max_condition_source_inputs: usize,
    pub max_inherited_conditions: usize,
    pub max_candidate_conditions: usize,
    pub max_condition_sources: usize,
    pub max_condition_source_shift_components: usize,
    pub max_condition_dependency_exponent_entries: usize,
    pub max_condition_equality_comparisons: usize,
    pub max_condition_equality_term_units: usize,
    pub max_condition_equality_exponent_entries: usize,
    pub max_condition_equality_integer_bits: usize,
    pub max_associate_exponent_entries: usize,
    pub max_associate_integer_bits: usize,
    pub max_boundary_values_per_rhs: usize,
    pub max_boundary_values: usize,
    pub max_pullback_compositions: usize,
    pub max_leak_witnesses: usize,
    pub max_total_source_terms: usize,
    pub max_total_source_exponent_entries: usize,
    pub max_total_source_integer_bits: usize,
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
    pub max_bad_clauses: usize,
    pub max_bad_atoms: usize,
    pub max_structural_loci: usize,
    pub max_associate_checks: usize,
    pub max_associate_term_pairs: usize,
    pub max_retained_polynomial_terms: usize,
    pub max_retained_polynomial_exponent_entries: usize,
    pub max_retained_polynomial_integer_bits: usize,
    pub max_retained_polynomial_display_bytes: usize,
    pub max_retained_bytes: usize,
    pub max_payload_comparison_units: usize,
    pub max_payload_comparison_bytes: usize,
    pub max_payload_comparison_integer_bits: usize,
    pub max_payload_comparison_private_manifest_bytes: usize,
}

impl Default for GeneratedResidualAffineWhenBadLimits {
    fn default() -> Self {
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            target_ordering: AffineParametricOrderingLimits::default(),
            composition_plan: ResidualUnitAffineCompositionPlanLimits::default(),
            polynomial_composition: ResidualUnitAffinePolynomialCompositionLimits::default(),
            relative_partition: AffineWhenBadRelativeCaseLimits::default(),
            max_family_fingerprint_bytes: 1024 * 1024,
            max_context_fingerprint_bytes: 1024 * 1024,
            max_scope_fingerprint_comparison_bytes: 4 * 1024 * 1024,
            max_ambient_arity: 1_000_000,
            max_free_positions: 1_000_000,
            max_map_entries_inspected: 64_000_000,
            max_matcher_outcomes_inspected: 1,
            max_matching_target_references_inspected: 64_000_000,
            max_inventory_terminals_inspected: 64_000_000,
            max_target_constant_comparison_entries: 2_000_000,
            max_target_constant_comparison_integer_bits: portable_usize(4_000_000_000_000_000),
            max_private_relation_terms: 64_000_000,
            max_private_relation_guards: 64_000_000,
            max_private_relation_origins: 256_000_000,
            max_private_relation_manifest_bytes: 2 * 1024 * 1024 * 1024,
            max_private_relation_shift_components: portable_usize(64_000_000_000),
            max_rhs_terms: 64_000_000,
            max_descent_witnesses: 64_000_000,
            max_descent_witness_components: 1_000_000_000,
            max_target_guard_entries: 64_000_000,
            max_relation_guard_condition_inputs: 64_000_000,
            max_coefficient_denominator_condition_inputs: 64_000_000,
            max_condition_inputs: 192_000_000,
            max_condition_source_inputs: 192_000_000,
            max_inherited_conditions: 64_000_000,
            max_candidate_conditions: 128_000_000,
            max_condition_sources: 512_000_000,
            max_condition_source_shift_components: portable_usize(64_000_000_000),
            max_condition_dependency_exponent_entries: portable_usize(64_000_000_000),
            max_condition_equality_comparisons: 1_000_000_000,
            max_condition_equality_term_units: portable_usize(16_000_000_000),
            max_condition_equality_exponent_entries: portable_usize(64_000_000_000),
            max_condition_equality_integer_bits: portable_usize(16_000_000_000_000_000),
            max_associate_exponent_entries: portable_usize(64_000_000_000),
            max_associate_integer_bits: portable_usize(16_000_000_000_000_000),
            max_boundary_values_per_rhs: 16_000_000,
            max_boundary_values: 256_000_000,
            max_pullback_compositions: 256_000_000,
            max_leak_witnesses: 256_000_000,
            max_total_source_terms: 1_000_000_000,
            max_total_source_exponent_entries: portable_usize(32_000_000_000),
            max_total_source_integer_bits: portable_usize(16_000_000_000_000_000),
            max_total_expanded_contributions: 1_000_000_000,
            max_total_output_term_bound: 1_000_000_000,
            max_total_output_terms: 1_000_000_000,
            max_total_output_exponent_entry_bound: portable_usize(64_000_000_000),
            max_total_output_exponent_entries: portable_usize(32_000_000_000),
            max_total_power_calls: portable_usize(32_000_000_000),
            max_total_native_power_heap_pairs: portable_usize(64_000_000_000),
            max_total_multiplication_term_pairs: portable_usize(64_000_000_000),
            max_total_addition_term_visits: portable_usize(64_000_000_000),
            max_total_native_integer_bit_work: portable_usize(32_000_000_000_000_000),
            max_total_integer_bit_work: portable_usize(32_000_000_000_000_000),
            max_bad_clauses: 512_000_000,
            max_bad_atoms: 1_000_000_000,
            max_structural_loci: 512_000_000,
            max_associate_checks: 1_000_000_000,
            max_associate_term_pairs: portable_usize(16_000_000_000),
            max_retained_polynomial_terms: 2_000_000_000,
            max_retained_polynomial_exponent_entries: portable_usize(64_000_000_000),
            max_retained_polynomial_integer_bits: portable_usize(16_000_000_000_000_000),
            max_retained_polynomial_display_bytes: portable_usize(8 * 1024 * 1024 * 1024),
            max_retained_bytes: portable_usize(16 * 1024 * 1024 * 1024),
            max_payload_comparison_units: portable_usize(256_000_000_000),
            max_payload_comparison_bytes: portable_usize(256 * 1024 * 1024 * 1024),
            max_payload_comparison_integer_bits: portable_usize(32_000_000_000_000_000),
            max_payload_comparison_private_manifest_bytes: portable_usize(4 * 1024 * 1024 * 1024),
        }
    }
}

/// Compact public identity of the exact target-local private compilation.
///
/// The private recentered relation is authenticated by a complete manifest
/// retained behind the final certificate, but neither that manifest nor the
/// relation itself is exposed here.
#[derive(Debug, PartialEq, Eq)]
pub struct GeneratedResidualAffineWhenBadBinding {
    source_case_ordinal: usize,
    source_group_ordinal: usize,
    pivot_ordinal: usize,
    target_case_ordinal: usize,
    target_position_in_matching_list: usize,
    target_locator: GeneratedResidualAffineCaseLocator,
    target_ordinal_within_group: usize,
    sector: SectorMask,
    coefficient_translation: IndexShift,
    key_center: IndexShift,
    target_ordering_manifest: Arc<String>,
    private_relation_manifest_bytes: usize,
    rhs_terms: usize,
}

impl GeneratedResidualAffineWhenBadBinding {
    pub const fn source_case_ordinal(&self) -> usize {
        self.source_case_ordinal
    }

    pub const fn source_group_ordinal(&self) -> usize {
        self.source_group_ordinal
    }

    pub const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }

    pub const fn target_case_ordinal(&self) -> usize {
        self.target_case_ordinal
    }

    pub const fn target_position_in_matching_list(&self) -> usize {
        self.target_position_in_matching_list
    }

    pub const fn target_locator(&self) -> GeneratedResidualAffineCaseLocator {
        self.target_locator
    }

    pub const fn target_ordinal_within_group(&self) -> usize {
        self.target_ordinal_within_group
    }

    pub const fn sector(&self) -> &SectorMask {
        &self.sector
    }

    pub const fn coefficient_translation(&self) -> &IndexShift {
        &self.coefficient_translation
    }

    pub const fn key_center(&self) -> &IndexShift {
        &self.key_center
    }

    pub fn target_ordering_manifest(&self) -> &str {
        self.target_ordering_manifest.as_str()
    }

    pub const fn private_relation_manifest_bytes(&self) -> usize {
        self.private_relation_manifest_bytes
    }

    pub const fn rhs_terms(&self) -> usize {
        self.rhs_terms
    }
}

/// Publicly safe reason why a fully authenticated candidate could not be
/// oriented on the selected target.  Exact RHS shifts remain in the private
/// descent transcript.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedResidualAffineWhenBadUnsupportedReason {
    UnsupportedOrderingPolicy {
        actual: IntegralOrderingPolicy,
    },
    NonUniformSameSectorDescent {
        rhs_ordinal: usize,
        first_nonzero_component: WhenBadDescentComponent,
    },
    ZeroSameSectorComplexityDelta {
        rhs_ordinal: usize,
    },
    UnboundedIndexAddition {
        rhs_ordinal: usize,
        coordinate: usize,
    },
    NoUniversalConstantPinch {
        rhs_ordinal: usize,
    },
    NonDescendingTargetSectorPrefix {
        rhs_ordinal: usize,
    },
}

impl From<GeneratedResidualAffineWhenBadDescentUnsupportedReason>
    for GeneratedResidualAffineWhenBadUnsupportedReason
{
    fn from(value: GeneratedResidualAffineWhenBadDescentUnsupportedReason) -> Self {
        match value {
            GeneratedResidualAffineWhenBadDescentUnsupportedReason::UnsupportedOrderingPolicy {
                actual,
            } => Self::UnsupportedOrderingPolicy { actual },
            GeneratedResidualAffineWhenBadDescentUnsupportedReason::NonUniformSameSectorDescent {
                rhs_ordinal,
                first_nonzero_component,
            } => Self::NonUniformSameSectorDescent {
                rhs_ordinal,
                first_nonzero_component,
            },
            GeneratedResidualAffineWhenBadDescentUnsupportedReason::ZeroSameSectorComplexityDelta {
                rhs_ordinal,
            } => Self::ZeroSameSectorComplexityDelta { rhs_ordinal },
            GeneratedResidualAffineWhenBadDescentUnsupportedReason::UnboundedIndexAddition {
                rhs_ordinal,
                coordinate,
            } => Self::UnboundedIndexAddition {
                rhs_ordinal,
                coordinate,
            },
            GeneratedResidualAffineWhenBadDescentUnsupportedReason::NoUniversalConstantPinch {
                rhs_ordinal,
            } => Self::NoUniversalConstantPinch { rhs_ordinal },
            GeneratedResidualAffineWhenBadDescentUnsupportedReason::NonDescendingTargetSectorPrefix {
                rhs_ordinal,
            } => Self::NonDescendingTargetSectorPrefix { rhs_ordinal },
        }
    }
}

/// Why the candidate's bad formula is literal true on its exact target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedResidualAffineWhenBadIdenticallyBadReason {
    RequiredNonzeroConditionIsZero { condition_input_ordinal: usize },
    UniversalCoefficientNonzeroLeak { pullback_ordinal: usize },
    NoStructurallyApplicableRelativeLeaf,
}

/// Redacted condition scope in the completed local certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedResidualAffineWhenBadConditionScope {
    InheritedTargetPremise,
    CandidateRequired,
}

impl From<GeneratedResidualAffineConditionScope> for GeneratedResidualAffineWhenBadConditionScope {
    fn from(value: GeneratedResidualAffineConditionScope) -> Self {
        match value {
            GeneratedResidualAffineConditionScope::InheritedTargetPremise => {
                Self::InheritedTargetPremise
            }
            GeneratedResidualAffineConditionScope::CandidateRequired => Self::CandidateRequired,
        }
    }
}

/// Allocation-free public projection of one canonical condition row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedResidualAffineWhenBadConditionView {
    ordinal: usize,
    scope: GeneratedResidualAffineWhenBadConditionScope,
    index_dependent: bool,
    source_count: usize,
}

impl GeneratedResidualAffineWhenBadConditionView {
    pub const fn ordinal(self) -> usize {
        self.ordinal
    }

    pub const fn scope(self) -> GeneratedResidualAffineWhenBadConditionScope {
        self.scope
    }

    pub const fn is_index_dependent(self) -> bool {
        self.index_dependent
    }

    pub const fn source_count(self) -> usize {
        self.source_count
    }
}

/// Redacted behavior of one exact boundary pullback on the authenticated
/// target.  The boundary coordinate, value, polynomial, and RHS shift remain
/// private replay evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedResidualAffineWhenBadPullbackClass {
    EmptyBoundary,
    WholeTarget,
    FreeIndexDependent,
}

impl From<AffineBoundaryPullbackClass> for GeneratedResidualAffineWhenBadPullbackClass {
    fn from(value: AffineBoundaryPullbackClass) -> Self {
        match value {
            AffineBoundaryPullbackClass::EmptyBoundary => Self::EmptyBoundary,
            AffineBoundaryPullbackClass::WholeTarget => Self::WholeTarget,
            AffineBoundaryPullbackClass::FreeIndexDependent => Self::FreeIndexDependent,
        }
    }
}

/// Redacted behavior of the exact mapped RHS numerator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedResidualAffineWhenBadNumeratorGateClass {
    CoefficientFieldNonzero,
    FreeIndexNonzero,
}

impl From<AffineWhenBadNumeratorGateClass> for GeneratedResidualAffineWhenBadNumeratorGateClass {
    fn from(value: AffineWhenBadNumeratorGateClass) -> Self {
        match value {
            AffineWhenBadNumeratorGateClass::CoefficientFieldNonzero => {
                Self::CoefficientFieldNonzero
            }
            AffineWhenBadNumeratorGateClass::FreeIndexNonzero => Self::FreeIndexNonzero,
        }
    }
}

/// Allocation-free public projection of one ordered boundary event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedResidualAffineWhenBadPullbackView {
    ordinal: usize,
    rhs_ordinal: usize,
    hazard_class: WhenBadBoundaryHazardKind,
    pullback_class: GeneratedResidualAffineWhenBadPullbackClass,
    numerator_gate_class: GeneratedResidualAffineWhenBadNumeratorGateClass,
}

impl GeneratedResidualAffineWhenBadPullbackView {
    pub const fn ordinal(self) -> usize {
        self.ordinal
    }

    pub const fn rhs_ordinal(self) -> usize {
        self.rhs_ordinal
    }

    pub const fn hazard_class(self) -> WhenBadBoundaryHazardKind {
        self.hazard_class
    }

    pub const fn pullback_class(self) -> GeneratedResidualAffineWhenBadPullbackClass {
        self.pullback_class
    }

    pub const fn numerator_gate_class(self) -> GeneratedResidualAffineWhenBadNumeratorGateClass {
        self.numerator_gate_class
    }
}

/// Binding-stage work census.  Later slices extend the final certificate
/// census without weakening these exact source counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedResidualAffineWhenBadStats {
    family_fingerprint_bytes: usize,
    context_fingerprint_bytes: usize,
    scope_fingerprint_comparison_bytes: usize,
    ambient_arity: usize,
    free_positions: usize,
    map_entries_inspected: usize,
    matcher_outcomes_inspected: usize,
    matching_target_references_inspected: usize,
    inventory_terminals_inspected: usize,
    target_constant_comparison_entries: usize,
    target_constant_comparison_integer_bits: usize,
    private_relation_terms: usize,
    private_relation_guards: usize,
    private_relation_origins: usize,
    private_relation_manifest_bytes: usize,
    private_relation_shift_components: usize,
    private_relation_source_terms: usize,
    private_relation_source_exponent_entries: usize,
    private_relation_source_integer_bits: usize,
    rhs_terms: usize,
    retained_byte_envelope: usize,
    retained_bytes: usize,
}

macro_rules! generated_affine_when_bad_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedResidualAffineWhenBadStats {
    generated_affine_when_bad_stats_getters!(
        family_fingerprint_bytes,
        context_fingerprint_bytes,
        scope_fingerprint_comparison_bytes,
        ambient_arity,
        free_positions,
        map_entries_inspected,
        matcher_outcomes_inspected,
        matching_target_references_inspected,
        inventory_terminals_inspected,
        target_constant_comparison_entries,
        target_constant_comparison_integer_bits,
        private_relation_terms,
        private_relation_guards,
        private_relation_origins,
        private_relation_manifest_bytes,
        private_relation_shift_components,
        private_relation_source_terms,
        private_relation_source_exponent_entries,
        private_relation_source_integer_bits,
        rhs_terms,
        retained_byte_envelope,
        retained_bytes,
    );
}

/// Compact aggregate census for the completed transactional compiler.  Each
/// child certificate retains its more detailed private counters; this view
/// reports enough to audit phase ordering and bounded coverage without
/// publishing relation payload.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedResidualAffineWhenBadCompilationStats {
    authority: GeneratedResidualAffineWhenBadStats,
    descent_witnesses_attempted: usize,
    descent_witnesses_proved: usize,
    condition_inputs: usize,
    canonical_conditions: usize,
    inherited_conditions: usize,
    candidate_conditions: usize,
    boundary_values: usize,
    pullback_compositions: usize,
    leak_witnesses: usize,
    structural_loci: usize,
    bad_clauses: usize,
    applicable_leaves: usize,
    exceptional_leaves: usize,
    retained_bytes: usize,
    // Complete private child census retained for the group owner. These
    // fields deliberately have no public getters: the public stats view stays
    // redacted, while the crate-private effective-coverage layer can project
    // the still-unspent aggregate envelope before compiling its next child.
    group_source_terms: usize,
    group_source_exponent_entries: usize,
    group_source_integer_bits: usize,
    group_output_terms: usize,
    group_output_exponent_entries: usize,
    group_native_integer_bit_work: usize,
    group_total_integer_bit_work: usize,
    group_payload_comparison_units: usize,
    group_payload_comparison_bytes: usize,
    group_payload_comparison_integer_bits: usize,
    group_payload_comparison_private_manifest_bytes: usize,
    group_assembly_payload_comparison_units: usize,
    group_assembly_payload_comparison_bytes: usize,
}

impl GeneratedResidualAffineWhenBadCompilationStats {
    pub const fn authority(self) -> GeneratedResidualAffineWhenBadStats {
        self.authority
    }

    pub const fn descent_witnesses_attempted(self) -> usize {
        self.descent_witnesses_attempted
    }

    pub const fn descent_witnesses_proved(self) -> usize {
        self.descent_witnesses_proved
    }

    pub const fn condition_inputs(self) -> usize {
        self.condition_inputs
    }

    pub const fn canonical_conditions(self) -> usize {
        self.canonical_conditions
    }

    pub const fn inherited_conditions(self) -> usize {
        self.inherited_conditions
    }

    pub const fn candidate_conditions(self) -> usize {
        self.candidate_conditions
    }

    pub const fn boundary_values(self) -> usize {
        self.boundary_values
    }

    pub const fn pullback_compositions(self) -> usize {
        self.pullback_compositions
    }

    pub const fn leak_witnesses(self) -> usize {
        self.leak_witnesses
    }

    pub const fn structural_loci(self) -> usize {
        self.structural_loci
    }

    pub const fn bad_clauses(self) -> usize {
        self.bad_clauses
    }

    pub const fn applicable_leaves(self) -> usize {
        self.applicable_leaves
    }

    pub const fn exceptional_leaves(self) -> usize {
        self.exceptional_leaves
    }

    pub const fn retained_bytes(self) -> usize {
        self.retained_bytes
    }

    pub(crate) const fn group_resource_usage(
        self,
    ) -> GeneratedResidualAffineWhenBadGroupResourceUsage {
        GeneratedResidualAffineWhenBadGroupResourceUsage {
            source_terms: self.group_source_terms,
            source_exponent_entries: self.group_source_exponent_entries,
            source_integer_bits: self.group_source_integer_bits,
            output_terms: self.group_output_terms,
            output_exponent_entries: self.group_output_exponent_entries,
            native_integer_bit_work: self.group_native_integer_bit_work,
            total_integer_bit_work: self.group_total_integer_bit_work,
            payload_comparison_units: self.group_payload_comparison_units,
            payload_comparison_bytes: self.group_payload_comparison_bytes,
            payload_comparison_integer_bits: self.group_payload_comparison_integer_bits,
            payload_comparison_private_manifest_bytes: self
                .group_payload_comparison_private_manifest_bytes,
            assembly_payload_comparison_units: self.group_assembly_payload_comparison_units,
            assembly_payload_comparison_bytes: self.group_assembly_payload_comparison_bytes,
            structural_loci: self.structural_loci,
            bad_clauses: self.bad_clauses,
            applicable_leaves: self.applicable_leaves,
            exceptional_leaves: self.exceptional_leaves,
            retained_bytes: self.retained_bytes,
        }
    }
}

/// Exact private local usage consumed by one group-level child.  It contains
/// only counters, never a relation, shift, condition source, or predicate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineWhenBadGroupResourceUsage {
    pub(crate) source_terms: usize,
    pub(crate) source_exponent_entries: usize,
    pub(crate) source_integer_bits: usize,
    pub(crate) output_terms: usize,
    pub(crate) output_exponent_entries: usize,
    pub(crate) native_integer_bit_work: usize,
    pub(crate) total_integer_bit_work: usize,
    pub(crate) payload_comparison_units: usize,
    pub(crate) payload_comparison_bytes: usize,
    pub(crate) payload_comparison_integer_bits: usize,
    pub(crate) payload_comparison_private_manifest_bytes: usize,
    pub(crate) assembly_payload_comparison_units: usize,
    pub(crate) assembly_payload_comparison_bytes: usize,
    pub(crate) structural_loci: usize,
    pub(crate) bad_clauses: usize,
    pub(crate) applicable_leaves: usize,
    pub(crate) exceptional_leaves: usize,
    pub(crate) retained_bytes: usize,
}

/// Aggregate, per-query bounds for locating one exact target-relative leaf.
///
/// These ceilings do not replace the arithmetic policy authenticated by the
/// certificate.  Every polynomial is preflighted and specialized with
/// [`GeneratedResidualAffineWhenBadLimits::arithmetic`]; this envelope only
/// bounds the total work of one complete point query across all leaves.
///
/// The specialization API deliberately authenticates its input again when it
/// executes.  Consequently one point query performs four source-polynomial
/// validation passes and two specialization-preflight passes per predicate:
/// the explicit whole-query preflight contributes two validations and one
/// specialization preflight, and the later checked specialization contributes
/// the same work again.  The dedicated scan bounds below charge that complete
/// call chain before execution starts; arithmetic/output/retained bounds remain
/// charged once per actual specialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedResidualAffineWhenBadPointLimits {
    pub max_context_fingerprint_comparison_bytes: usize,
    pub max_index_entries: usize,
    pub max_cases: usize,
    pub max_classifications: usize,
    pub max_predicates: usize,
    pub max_source_terms: usize,
    pub max_source_exponent_entries: usize,
    pub max_preflight_validation_source_term_scan_bound: usize,
    pub max_preflight_validation_source_exponent_entry_scan_bound: usize,
    pub max_output_term_bound: usize,
    pub max_output_exponent_entry_bound: usize,
    pub max_power_operation_bound: usize,
    pub max_largest_output_integer_bit_bound: usize,
    pub max_integer_bit_work_bound: usize,
    pub max_retained_output_term_bound: usize,
    pub max_retained_output_byte_bound: usize,
}

impl Default for GeneratedResidualAffineWhenBadPointLimits {
    fn default() -> Self {
        Self {
            max_context_fingerprint_comparison_bytes: 2 * 1024 * 1024,
            max_index_entries: 1_000_000,
            max_cases: 64_000_000,
            max_classifications: 64_000_000,
            max_predicates: 256_000_000,
            max_source_terms: 1_000_000_000,
            max_source_exponent_entries: portable_usize(64_000_000_000),
            max_preflight_validation_source_term_scan_bound: portable_usize(8_000_000_000),
            max_preflight_validation_source_exponent_entry_scan_bound: portable_usize(
                640_000_000_000,
            ),
            max_output_term_bound: 4_000_000_000,
            max_output_exponent_entry_bound: portable_usize(256_000_000_000),
            max_power_operation_bound: portable_usize(64_000_000_000),
            max_largest_output_integer_bit_bound: 64_000_000,
            max_integer_bit_work_bound: portable_usize(64_000_000_000_000),
            max_retained_output_term_bound: 4_000_000_000,
            max_retained_output_byte_bound: portable_usize(256 * 1024 * 1024 * 1024),
        }
    }
}

/// Immutable exact census for one successful target-relative point query.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedResidualAffineWhenBadPointStats {
    context_fingerprint_comparison_bytes: usize,
    index_entries: usize,
    cases: usize,
    classifications: usize,
    predicates: usize,
    source_terms: usize,
    source_exponent_entries: usize,
    preflight_validation_source_term_scan_bound: usize,
    preflight_validation_source_exponent_entry_scan_bound: usize,
    output_term_bound: usize,
    output_exponent_entry_bound: usize,
    power_operation_bound: usize,
    largest_output_integer_bit_bound: usize,
    integer_bit_work_bound: usize,
    retained_output_term_bound: usize,
    retained_output_byte_bound: usize,
    matched_cases: usize,
}

macro_rules! generated_affine_point_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedResidualAffineWhenBadPointStats {
    generated_affine_point_stats_getters!(
        context_fingerprint_comparison_bytes,
        index_entries,
        cases,
        classifications,
        predicates,
        source_terms,
        source_exponent_entries,
        preflight_validation_source_term_scan_bound,
        preflight_validation_source_exponent_entry_scan_bound,
        output_term_bound,
        output_exponent_entry_bound,
        power_operation_bound,
        largest_output_integer_bit_bound,
        integer_bit_work_bound,
        retained_output_term_bound,
        retained_output_byte_bound,
        matched_cases,
    );
}

/// Redacted result of an exact query against the private relative partition.
/// No predicate, polynomial, point coordinate, or recurrence payload crosses
/// this API boundary.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct GeneratedResidualAffineWhenBadPointClassification {
    leaf_ordinal: usize,
    case: AffineWhenBadRelativeCaseId,
    disposition: AffineWhenBadRelativeLeafDisposition,
    stats: GeneratedResidualAffineWhenBadPointStats,
}

impl GeneratedResidualAffineWhenBadPointClassification {
    pub const fn leaf_ordinal(self) -> usize {
        self.leaf_ordinal
    }

    pub const fn case(self) -> AffineWhenBadRelativeCaseId {
        self.case
    }

    pub const fn disposition(self) -> AffineWhenBadRelativeLeafDisposition {
        self.disposition
    }

    pub const fn stats(self) -> GeneratedResidualAffineWhenBadPointStats {
        self.stats
    }
}

impl fmt::Debug for GeneratedResidualAffineWhenBadPointClassification {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineWhenBadPointClassification")
            .field("leaf_ordinal", &self.leaf_ordinal)
            .field("case", &self.case)
            .field("disposition", &self.disposition)
            .field("stats", &self.stats)
            .finish()
    }
}

/// Authentication, partition, arithmetic, or resource failure during one
/// exact query.  Query failures never expose the private relative predicates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedResidualAffineWhenBadPointError {
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    PartitionShapeMismatch {
        cases: usize,
        classifications: usize,
    },
    CaseClassificationMismatch {
        leaf_ordinal: usize,
    },
    PartitionEvaluationMismatch {
        matched_cases: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ParametricCoefficient(crate::ParametricCoefficientError),
    SymbolicaPanic,
}

impl fmt::Display for GeneratedResidualAffineWhenBadPointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongContext => formatter
                .write_str("generated affine WhenBad point query belongs to another K(n) context"),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "generated affine WhenBad point query expected arity {expected}, got {actual}",
            ),
            Self::PartitionShapeMismatch {
                cases,
                classifications,
            } => write!(
                formatter,
                "generated affine WhenBad point query found {cases} cases but {classifications} classifications",
            ),
            Self::CaseClassificationMismatch { leaf_ordinal } => write!(
                formatter,
                "generated affine WhenBad point query case/classification mismatch at leaf {leaf_ordinal}",
            ),
            Self::PartitionEvaluationMismatch { matched_cases } => write!(
                formatter,
                "generated affine WhenBad point query matched {matched_cases} relative leaves instead of exactly one",
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
            Self::ParametricCoefficient(error) => error.fmt(formatter),
            Self::SymbolicaPanic => formatter
                .write_str("Symbolica panicked during generated affine WhenBad point query"),
        }
    }
}

impl std::error::Error for GeneratedResidualAffineWhenBadPointError {}

impl From<crate::ParametricCoefficientError> for GeneratedResidualAffineWhenBadPointError {
    fn from(value: crate::ParametricCoefficientError) -> Self {
        Self::ParametricCoefficient(value)
    }
}

/// Hard authentication, resource, child-certificate, or replay failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedResidualAffineWhenBadError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    PivotOrdinalOutOfRange {
        pivot_ordinal: usize,
    },
    PivotOutcomeIsNotPending {
        pivot_ordinal: usize,
    },
    PivotOrdinalMismatch {
        requested: usize,
        retained: usize,
    },
    TargetCaseOrdinalOutOfRange {
        target_case_ordinal: usize,
    },
    TargetNotInMatchingList {
        target_case_ordinal: usize,
    },
    DuplicateTargetInMatchingList {
        target_case_ordinal: usize,
    },
    TargetWrongGroup {
        expected: usize,
        actual: usize,
    },
    TargetMissingFromGroup {
        target_case_ordinal: usize,
    },
    TargetConstantsMismatch,
    TargetTerminalMissing,
    DuplicateTargetTerminal,
    TargetTerminalOutcomeMismatch,
    TargetCoverAllocationMismatch,
    TargetBranchAllocationMismatch,
    TargetGuardCompositionAllocationMismatch,
    TargetBranchCoverAllocationMismatch,
    TargetGuardCoverAllocationMismatch,
    TargetGuardBranchAllocationMismatch,
    TargetOrderingBranchAllocationMismatch,
    TargetBranchOutcomeNotGuardedAffineMap,
    TargetGuardContradiction {
        entry_ordinal: usize,
    },
    MissingTargetIntegerSystem,
    PrivateRelationFamilyMismatch,
    PrivateRelationContextMismatch,
    PrivateRelationMissingCenteredPivot,
    PrivateRelationNonunitCenteredPivot,
    PrivateRelationZeroRhsCoefficient {
        rhs_ordinal: usize,
    },
    BoundaryArithmeticOverflow {
        coordinate: usize,
    },
    DescentArithmeticOverflow,
    ConditionInvariant {
        stage: &'static str,
    },
    ReplayMismatch,
    RetainedByteEnvelopeExceeded {
        observed: usize,
        admitted: usize,
    },
    SymbolicaPanic {
        stage: &'static str,
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
    Matcher(GeneratedResidualAffinePivotTargetMatchingError),
    Ordering(AffineParametricOrderingError),
    Relation(ParametricRelationError),
    Sector(SectorFoundationError),
    ParametricCoefficient(crate::ParametricCoefficientError),
    Composition(ResidualUnitAffineCompositionError),
    RelativePartition(AffineWhenBadRelativeCaseError),
}

impl fmt::Display for GeneratedResidualAffineWhenBadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("generated affine WhenBad schema mismatch"),
            Self::WrongFamily => formatter.write_str("generated affine WhenBad belongs to another family"),
            Self::WrongContext => formatter.write_str("generated affine WhenBad belongs to another K(n) context"),
            Self::WrongArity { expected, actual } => write!(formatter, "generated affine WhenBad expected arity {expected}, got {actual}"),
            Self::PivotOrdinalOutOfRange { pivot_ordinal } => write!(formatter, "generated affine WhenBad pivot ordinal {pivot_ordinal} is out of range"),
            Self::PivotOutcomeIsNotPending { pivot_ordinal } => write!(formatter, "generated affine WhenBad pivot ordinal {pivot_ordinal} is not pending affine WhenBad"),
            Self::PivotOrdinalMismatch { requested, retained } => write!(formatter, "generated affine WhenBad requested pivot ordinal {requested}, but outcome retains {retained}"),
            Self::TargetCaseOrdinalOutOfRange { target_case_ordinal } => write!(formatter, "generated affine WhenBad target case ordinal {target_case_ordinal} is out of range"),
            Self::TargetNotInMatchingList { target_case_ordinal } => write!(formatter, "generated affine WhenBad target case {target_case_ordinal} is absent from the persisted matching list"),
            Self::DuplicateTargetInMatchingList { target_case_ordinal } => write!(formatter, "generated affine WhenBad target case {target_case_ordinal} occurs more than once in the persisted matching list"),
            Self::TargetWrongGroup { expected, actual } => write!(formatter, "generated affine WhenBad target group is {actual}, expected {expected}"),
            Self::TargetMissingFromGroup { target_case_ordinal } => write!(formatter, "generated affine WhenBad target case {target_case_ordinal} is absent from its retained geometry group"),
            Self::TargetConstantsMismatch => formatter.write_str("generated affine WhenBad target constants do not match the pending transformed constants"),
            Self::TargetTerminalMissing => formatter.write_str("generated affine WhenBad target terminal is missing"),
            Self::DuplicateTargetTerminal => formatter.write_str("generated affine WhenBad target locator resolves more than one inventory terminal"),
            Self::TargetTerminalOutcomeMismatch => formatter.write_str("generated affine WhenBad target terminal is not actionable as the selected case"),
            Self::TargetCoverAllocationMismatch => formatter.write_str("generated affine WhenBad target cover allocation mismatch"),
            Self::TargetBranchAllocationMismatch => formatter.write_str("generated affine WhenBad target branch allocation mismatch"),
            Self::TargetGuardCompositionAllocationMismatch => formatter.write_str("generated affine WhenBad target guard-composition allocation mismatch"),
            Self::TargetBranchCoverAllocationMismatch => formatter.write_str("generated affine WhenBad branch does not retain the target cover allocation"),
            Self::TargetGuardCoverAllocationMismatch => formatter.write_str("generated affine WhenBad guard composition does not retain the target cover allocation"),
            Self::TargetGuardBranchAllocationMismatch => formatter.write_str("generated affine WhenBad guard composition does not retain the target branch allocation"),
            Self::TargetOrderingBranchAllocationMismatch => formatter.write_str("generated affine WhenBad target ordering does not retain the selected branch allocation"),
            Self::TargetBranchOutcomeNotGuardedAffineMap => formatter.write_str("generated affine WhenBad target branch is not a guarded affine map"),
            Self::TargetGuardContradiction { entry_ordinal } => write!(formatter, "generated affine WhenBad actionable target guard entry {entry_ordinal} is contradictory"),
            Self::MissingTargetIntegerSystem => formatter.write_str("generated affine WhenBad target branch has no authenticated integer system"),
            Self::PrivateRelationFamilyMismatch => formatter.write_str("generated affine WhenBad private relation belongs to another family"),
            Self::PrivateRelationContextMismatch => formatter.write_str("generated affine WhenBad private relation belongs to another K(n) context"),
            Self::PrivateRelationMissingCenteredPivot => formatter.write_str("generated affine WhenBad private relation has no zero-centered pivot"),
            Self::PrivateRelationNonunitCenteredPivot => formatter.write_str("generated affine WhenBad private relation's zero-centered pivot is not exactly one"),
            Self::PrivateRelationZeroRhsCoefficient { rhs_ordinal } => write!(formatter, "generated affine WhenBad private RHS term {rhs_ordinal} has a zero coefficient"),
            Self::BoundaryArithmeticOverflow { coordinate } => write!(formatter, "generated affine WhenBad boundary arithmetic overflowed at coordinate {coordinate}"),
            Self::DescentArithmeticOverflow => formatter.write_str("generated affine WhenBad descent arithmetic overflowed"),
            Self::ConditionInvariant { stage } => write!(formatter, "generated affine WhenBad condition invariant failed during {stage}"),
            Self::ReplayMismatch => formatter.write_str("generated affine WhenBad did not replay exactly"),
            Self::RetainedByteEnvelopeExceeded { observed, admitted } => write!(formatter, "generated affine WhenBad retained {observed} bytes after admitting {admitted}"),
            Self::SymbolicaPanic { stage } => write!(formatter, "Symbolica panicked during generated affine WhenBad {stage}"),
            Self::ResourceLimit { resource, requested, limit } => write!(formatter, "{resource} requested {requested}, configured limit is {limit}"),
            Self::ResourceCountOverflow { resource } => write!(formatter, "{resource} count overflowed usize"),
            Self::AllocationFailure { resource, requested } => write!(formatter, "{resource} allocation of {requested} entries failed after bounded preflight"),
            Self::Matcher(error) => error.fmt(formatter),
            Self::Ordering(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
            Self::ParametricCoefficient(error) => error.fmt(formatter),
            Self::Composition(error) => error.fmt(formatter),
            Self::RelativePartition(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedResidualAffineWhenBadError {}

impl From<GeneratedResidualAffinePivotTargetMatchingError> for GeneratedResidualAffineWhenBadError {
    fn from(value: GeneratedResidualAffinePivotTargetMatchingError) -> Self {
        Self::Matcher(value)
    }
}

impl From<AffineParametricOrderingError> for GeneratedResidualAffineWhenBadError {
    fn from(value: AffineParametricOrderingError) -> Self {
        Self::Ordering(value)
    }
}

impl From<ParametricRelationError> for GeneratedResidualAffineWhenBadError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

impl From<SectorFoundationError> for GeneratedResidualAffineWhenBadError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

impl From<crate::ParametricCoefficientError> for GeneratedResidualAffineWhenBadError {
    fn from(value: crate::ParametricCoefficientError) -> Self {
        Self::ParametricCoefficient(value)
    }
}

impl From<ResidualUnitAffineCompositionError> for GeneratedResidualAffineWhenBadError {
    fn from(value: ResidualUnitAffineCompositionError) -> Self {
        Self::Composition(value)
    }
}

impl From<AffineWhenBadRelativeCaseError> for GeneratedResidualAffineWhenBadError {
    fn from(value: AffineWhenBadRelativeCaseError) -> Self {
        Self::RelativePartition(value)
    }
}

/// Private authenticated authority handed to the algebraic compilation
/// phases.  Keeping the relation and child allocations here prevents the
/// public binding from becoming an application seam.
pub(crate) struct AuthenticatedGeneratedResidualAffineWhenBadInput {
    matcher: Arc<GeneratedResidualAffinePivotTargetMatchingCertificate>,
    binding: GeneratedResidualAffineWhenBadBinding,
    target_ordering: AffineStartParametricEliminationOrdering,
    target_cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
    target_branch: Arc<ResidualAffineBranchSystemCertificate>,
    target_guard_composition: Arc<ResidualAffineBranchGuardCompositionCertificate>,
    relation: Arc<ParametricRelation>,
    private_relation_manifest: Arc<String>,
    limits: GeneratedResidualAffineWhenBadLimits,
    stats: GeneratedResidualAffineWhenBadStats,
}

impl AuthenticatedGeneratedResidualAffineWhenBadInput {
    pub(crate) const fn matcher(
        &self,
    ) -> &Arc<GeneratedResidualAffinePivotTargetMatchingCertificate> {
        &self.matcher
    }

    pub(crate) const fn binding(&self) -> &GeneratedResidualAffineWhenBadBinding {
        &self.binding
    }

    pub(crate) const fn target_ordering(&self) -> &AffineStartParametricEliminationOrdering {
        &self.target_ordering
    }

    pub(crate) const fn target_cover(&self) -> &Arc<ResidualProductLocusBooleanCoverCertificate> {
        &self.target_cover
    }

    pub(crate) const fn target_branch(&self) -> &Arc<ResidualAffineBranchSystemCertificate> {
        &self.target_branch
    }

    pub(crate) const fn target_guard_composition(
        &self,
    ) -> &Arc<ResidualAffineBranchGuardCompositionCertificate> {
        &self.target_guard_composition
    }

    pub(crate) const fn relation(&self) -> &Arc<ParametricRelation> {
        &self.relation
    }

    pub(crate) fn private_relation_manifest(&self) -> &str {
        self.private_relation_manifest.as_str()
    }

    pub(crate) const fn limits(&self) -> GeneratedResidualAffineWhenBadLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> GeneratedResidualAffineWhenBadStats {
        self.stats
    }

    /// Exact private comparison used only by the owning outer replay.
    /// Shared certificate payloads are required to be the same allocations;
    /// mathematical equality across independently rebuilt matcher graphs is
    /// authenticated by the matcher replay before this local comparison.
    pub(crate) fn payload_eq_same_authority(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.matcher, &other.matcher)
            && self.binding == other.binding
            && self.target_ordering.stable_manifest() == other.target_ordering.stable_manifest()
            && Arc::ptr_eq(&self.target_cover, &other.target_cover)
            && Arc::ptr_eq(&self.target_branch, &other.target_branch)
            && Arc::ptr_eq(
                &self.target_guard_composition,
                &other.target_guard_composition,
            )
            && Arc::ptr_eq(&self.relation, &other.relation)
            && self.private_relation_manifest == other.private_relation_manifest
            && self.limits == other.limits
            && self.stats == other.stats
    }
}

fn generated_affine_private_payload_operand_bytes(
    input: &AuthenticatedGeneratedResidualAffineWhenBadInput,
) -> Result<usize, GeneratedResidualAffineWhenBadError> {
    let binding = input.binding();
    let mut bytes = 0usize;
    for (resource, entries, width) in [
        (
            "generated affine WhenBad binding sector comparison bytes",
            binding.sector().active_bits().len(),
            size_of::<bool>(),
        ),
        (
            "generated affine WhenBad coefficient translation comparison bytes",
            binding.coefficient_translation().values().len(),
            size_of::<i64>(),
        ),
        (
            "generated affine WhenBad key-center comparison bytes",
            binding.key_center().values().len(),
            size_of::<i64>(),
        ),
    ] {
        bytes = checked_add(resource, bytes, checked_mul(resource, entries, width)?)?;
    }
    for (resource, manifest) in [
        (
            "generated affine WhenBad binding ordering-manifest comparison bytes",
            binding.target_ordering_manifest(),
        ),
        (
            "generated affine WhenBad ordering-manifest comparison bytes",
            input.target_ordering().stable_manifest(),
        ),
        (
            "generated affine WhenBad private relation-manifest comparison bytes",
            input.private_relation_manifest(),
        ),
    ] {
        bytes = checked_add(resource, bytes, manifest.len())?;
    }
    Ok(bytes)
}

pub(crate) fn preflight_generated_affine_private_payload_comparison(
    left: &AuthenticatedGeneratedResidualAffineWhenBadInput,
    right: &AuthenticatedGeneratedResidualAffineWhenBadInput,
) -> Result<usize, GeneratedResidualAffineWhenBadError> {
    let bytes = checked_add(
        "generated affine WhenBad private payload comparison bytes",
        generated_affine_private_payload_operand_bytes(left)?,
        generated_affine_private_payload_operand_bytes(right)?,
    )?;
    check_limit(
        "generated affine WhenBad private payload comparison bytes",
        bytes,
        left.limits()
            .max_payload_comparison_private_manifest_bytes
            .min(right.limits().max_payload_comparison_private_manifest_bytes),
    )?;
    check_limit(
        "generated affine WhenBad payload comparison bytes",
        bytes,
        left.limits()
            .max_payload_comparison_bytes
            .min(right.limits().max_payload_comparison_bytes),
    )?;
    Ok(bytes)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct GeneratedAffineOuterPayloadComparisonCensus {
    units: usize,
    bytes: usize,
    integer_bits: usize,
    private_manifest_bytes: usize,
}

#[allow(clippy::too_many_arguments)]
fn generated_affine_outer_payload_comparison_census(
    left_input: &AuthenticatedGeneratedResidualAffineWhenBadInput,
    left_descent: GeneratedResidualAffineWhenBadDescentStats,
    left_conditions: Option<GeneratedResidualAffineConditionAccumulatorStats>,
    left_pullbacks: Option<GeneratedResidualAffineWhenBadPullbackGateStats>,
    left_partition: Option<&AffineWhenBadRelativePartitionCertificate>,
    right_input: &AuthenticatedGeneratedResidualAffineWhenBadInput,
    right_descent: GeneratedResidualAffineWhenBadDescentStats,
    right_conditions: Option<GeneratedResidualAffineConditionAccumulatorStats>,
    right_pullbacks: Option<GeneratedResidualAffineWhenBadPullbackGateStats>,
    right_partition: Option<&AffineWhenBadRelativePartitionCertificate>,
) -> Result<GeneratedAffineOuterPayloadComparisonCensus, GeneratedResidualAffineWhenBadError> {
    let private_bytes =
        preflight_generated_affine_private_payload_comparison(left_input, right_input)?;
    let left_condition = left_conditions.unwrap_or_default();
    let right_condition = right_conditions.unwrap_or_default();
    let left_pullback = left_pullbacks.unwrap_or_default();
    let right_pullback = right_pullbacks.unwrap_or_default();
    let left_partition_stats = left_partition.map(AffineWhenBadRelativePartitionCertificate::stats);
    let right_partition_stats =
        right_partition.map(AffineWhenBadRelativePartitionCertificate::stats);

    let units = checked_add(
        "generated affine WhenBad payload comparison units",
        left_descent
            .payload_comparison_units_observed()
            .max(right_descent.payload_comparison_units_observed()),
        checked_add(
            "generated affine WhenBad payload comparison units",
            generated_affine_condition_payload_comparison_units(left_condition)?.max(
                generated_affine_condition_payload_comparison_units(right_condition)?,
            ),
            checked_add(
                "generated affine WhenBad payload comparison units",
                left_pullback
                    .payload_comparison_units()
                    .max(right_pullback.payload_comparison_units()),
                left_partition_stats
                    .map_or(0, |stats| stats.payload_comparison_units())
                    .max(right_partition_stats.map_or(0, |stats| stats.payload_comparison_units())),
            )?,
        )?,
    )?;
    let bytes = checked_add(
        "generated affine WhenBad payload comparison bytes",
        private_bytes,
        checked_add(
            "generated affine WhenBad payload comparison bytes",
            left_condition
                .context_fingerprint_comparison_bytes()
                .max(right_condition.context_fingerprint_comparison_bytes()),
            checked_add(
                "generated affine WhenBad payload comparison bytes",
                left_pullback
                    .payload_comparison_bytes()
                    .max(right_pullback.payload_comparison_bytes()),
                left_partition_stats
                    .map_or(0, |stats| stats.payload_comparison_bytes())
                    .max(right_partition_stats.map_or(0, |stats| stats.payload_comparison_bytes())),
            )?,
        )?,
    )?;
    let integer_bits = checked_add(
        "generated affine WhenBad payload comparison integer bits",
        left_condition
            .equality_integer_bits()
            .max(right_condition.equality_integer_bits()),
        checked_add(
            "generated affine WhenBad payload comparison integer bits",
            left_pullback
                .payload_comparison_integer_bits()
                .max(right_pullback.payload_comparison_integer_bits()),
            left_partition_stats
                .map_or(0, |stats| stats.payload_comparison_integer_bits())
                .max(
                    right_partition_stats
                        .map_or(0, |stats| stats.payload_comparison_integer_bits()),
                ),
        )?,
    )?;
    Ok(GeneratedAffineOuterPayloadComparisonCensus {
        units,
        bytes,
        integer_bits,
        private_manifest_bytes: private_bytes,
    })
}

#[allow(clippy::too_many_arguments)]
fn preflight_generated_affine_outer_payload_comparison(
    left_input: &AuthenticatedGeneratedResidualAffineWhenBadInput,
    left_descent: GeneratedResidualAffineWhenBadDescentStats,
    left_conditions: Option<GeneratedResidualAffineConditionAccumulatorStats>,
    left_pullbacks: Option<GeneratedResidualAffineWhenBadPullbackGateStats>,
    left_partition: Option<&AffineWhenBadRelativePartitionCertificate>,
    right_input: &AuthenticatedGeneratedResidualAffineWhenBadInput,
    right_descent: GeneratedResidualAffineWhenBadDescentStats,
    right_conditions: Option<GeneratedResidualAffineConditionAccumulatorStats>,
    right_pullbacks: Option<GeneratedResidualAffineWhenBadPullbackGateStats>,
    right_partition: Option<&AffineWhenBadRelativePartitionCertificate>,
) -> Result<GeneratedAffineOuterPayloadComparisonCensus, GeneratedResidualAffineWhenBadError> {
    let census = generated_affine_outer_payload_comparison_census(
        left_input,
        left_descent,
        left_conditions,
        left_pullbacks,
        left_partition,
        right_input,
        right_descent,
        right_conditions,
        right_pullbacks,
        right_partition,
    )?;
    let limits = left_input.limits();
    let right_limits = right_input.limits();
    check_limit(
        "generated affine WhenBad payload comparison units",
        census.units,
        limits
            .max_payload_comparison_units
            .min(right_limits.max_payload_comparison_units),
    )?;
    check_limit(
        "generated affine WhenBad payload comparison bytes",
        census.bytes,
        limits
            .max_payload_comparison_bytes
            .min(right_limits.max_payload_comparison_bytes),
    )?;
    check_limit(
        "generated affine WhenBad payload comparison integer bits",
        census.integer_bits,
        limits
            .max_payload_comparison_integer_bits
            .min(right_limits.max_payload_comparison_integer_bits),
    )?;
    Ok(census)
}

/// Authenticate the exact local tuple without publishing relation content.
///
/// This is crate-private until the remaining compilation phases can return a
/// complete `Certified`/`IdenticallyBad`/`Unsupported` outcome transactionally.
pub(crate) fn authenticate_generated_residual_affine_when_bad_input(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    matcher: Arc<GeneratedResidualAffinePivotTargetMatchingCertificate>,
    pending_pivot_ordinal: usize,
    target_case_ordinal: usize,
    limits: GeneratedResidualAffineWhenBadLimits,
) -> Result<AuthenticatedGeneratedResidualAffineWhenBadInput, GeneratedResidualAffineWhenBadError> {
    catch_unwind(AssertUnwindSafe(|| {
        authenticate_generated_residual_affine_when_bad_input_inner(
            family,
            context,
            matcher,
            pending_pivot_ordinal,
            target_case_ordinal,
            limits,
        )
    }))
    .map_err(|_| GeneratedResidualAffineWhenBadError::SymbolicaPanic {
        stage: "binding authentication",
    })?
}

fn authenticate_generated_residual_affine_when_bad_input_inner(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    matcher: Arc<GeneratedResidualAffinePivotTargetMatchingCertificate>,
    pending_pivot_ordinal: usize,
    target_case_ordinal: usize,
    limits: GeneratedResidualAffineWhenBadLimits,
) -> Result<AuthenticatedGeneratedResidualAffineWhenBadInput, GeneratedResidualAffineWhenBadError> {
    let inventory = matcher.inventory();
    let mut stats = GeneratedResidualAffineWhenBadStats::default();

    let family_fingerprint = family.fingerprint_ref();
    stats.family_fingerprint_bytes = family_fingerprint
        .len()
        .max(inventory.family_fingerprint().len());
    stats.context_fingerprint_bytes = context
        .fingerprint()
        .len()
        .max(inventory.context_fingerprint().len());
    stats.scope_fingerprint_comparison_bytes = checked_add(
        "generated affine WhenBad scope fingerprint comparison bytes",
        checked_add(
            "generated affine WhenBad scope fingerprint comparison bytes",
            family_fingerprint.len(),
            inventory.family_fingerprint().len(),
        )?,
        checked_add(
            "generated affine WhenBad scope fingerprint comparison bytes",
            context.fingerprint().len(),
            inventory.context_fingerprint().len(),
        )?,
    )?;
    for requested in [
        family_fingerprint.len(),
        inventory.family_fingerprint().len(),
    ] {
        check_limit(
            "generated affine WhenBad family fingerprint bytes",
            requested,
            limits.max_family_fingerprint_bytes,
        )?;
    }
    for requested in [
        context.fingerprint().len(),
        inventory.context_fingerprint().len(),
    ] {
        check_limit(
            "generated affine WhenBad context fingerprint bytes",
            requested,
            limits.max_context_fingerprint_bytes,
        )?;
    }
    check_limit(
        "generated affine WhenBad scope fingerprint comparison bytes",
        stats.scope_fingerprint_comparison_bytes,
        limits.max_scope_fingerprint_comparison_bytes,
    )?;
    if inventory.family_fingerprint() != family_fingerprint {
        return Err(GeneratedResidualAffineWhenBadError::WrongFamily);
    }
    if inventory.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedResidualAffineWhenBadError::WrongContext);
    }

    // The matcher replay recursively authenticates its inventory,
    // re-elimination, pending relation, and exact target-matching transcript.
    // Its complete child work is governed and reported by matcher.limits()/
    // matcher.stats(); this outer census starts with the one post-replay
    // outcome selection and charges every new scan/comparison/copy below.
    matcher.replay(family, context)?;

    stats.matcher_outcomes_inspected = 1;
    check_limit(
        "generated affine WhenBad matcher outcomes inspected",
        stats.matcher_outcomes_inspected,
        limits.max_matcher_outcomes_inspected,
    )?;
    let outcome = matcher.outcomes().get(pending_pivot_ordinal).ok_or(
        GeneratedResidualAffineWhenBadError::PivotOrdinalOutOfRange {
            pivot_ordinal: pending_pivot_ordinal,
        },
    )?;
    if outcome.pivot_ordinal() != pending_pivot_ordinal {
        return Err(GeneratedResidualAffineWhenBadError::PivotOrdinalMismatch {
            requested: pending_pivot_ordinal,
            retained: outcome.pivot_ordinal(),
        });
    }
    let GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(pending) = outcome else {
        return Err(
            GeneratedResidualAffineWhenBadError::PivotOutcomeIsNotPending {
                pivot_ordinal: pending_pivot_ordinal,
            },
        );
    };

    stats.matching_target_references_inspected = pending.matching_target_case_ordinals().len();
    check_limit(
        "generated affine WhenBad matching target references inspected",
        stats.matching_target_references_inspected,
        limits.max_matching_target_references_inspected,
    )?;
    let mut target_position = None;
    for (position, &ordinal) in pending.matching_target_case_ordinals().iter().enumerate() {
        if ordinal == target_case_ordinal {
            if target_position.replace(position).is_some() {
                return Err(
                    GeneratedResidualAffineWhenBadError::DuplicateTargetInMatchingList {
                        target_case_ordinal,
                    },
                );
            }
        }
    }
    let target_position = target_position.ok_or(
        GeneratedResidualAffineWhenBadError::TargetNotInMatchingList {
            target_case_ordinal,
        },
    )?;

    let target = inventory.cases().get(target_case_ordinal).ok_or(
        GeneratedResidualAffineWhenBadError::TargetCaseOrdinalOutOfRange {
            target_case_ordinal,
        },
    )?;
    if target.ordinal() != target_case_ordinal {
        return Err(
            GeneratedResidualAffineWhenBadError::TargetCaseOrdinalOutOfRange {
                target_case_ordinal,
            },
        );
    }
    if target.group_ordinal() != matcher.source_group_ordinal() {
        return Err(GeneratedResidualAffineWhenBadError::TargetWrongGroup {
            expected: matcher.source_group_ordinal(),
            actual: target.group_ordinal(),
        });
    }
    let group = inventory
        .groups()
        .get(matcher.source_group_ordinal())
        .ok_or(GeneratedResidualAffineWhenBadError::TargetWrongGroup {
            expected: matcher.source_group_ordinal(),
            actual: target.group_ordinal(),
        })?;
    if group.ordinal() != matcher.source_group_ordinal()
        || group.case_ordinals().get(target.ordinal_within_group()) != Some(&target_case_ordinal)
    {
        return Err(
            GeneratedResidualAffineWhenBadError::TargetMissingFromGroup {
                target_case_ordinal,
            },
        );
    }
    stats.ambient_arity = group.ambient_arity();
    stats.free_positions = group.free_positions().len();
    stats.map_entries_inspected = checked_add(
        "generated affine WhenBad map entries inspected",
        group.ambient_arity(),
        checked_mul(
            "generated affine WhenBad map entries inspected",
            group.ambient_arity(),
            group.free_positions().len(),
        )?,
    )?;
    check_limit(
        "generated affine WhenBad ambient arity",
        stats.ambient_arity,
        limits.max_ambient_arity,
    )?;
    check_limit(
        "generated affine WhenBad free positions",
        stats.free_positions,
        limits.max_free_positions,
    )?;
    check_limit(
        "generated affine WhenBad map entries inspected",
        stats.map_entries_inspected,
        limits.max_map_entries_inspected,
    )?;
    if group.ambient_arity() != context.index_count() {
        return Err(GeneratedResidualAffineWhenBadError::WrongArity {
            expected: context.index_count(),
            actual: group.ambient_arity(),
        });
    }
    for actual in [
        target.constants().len(),
        pending.transformed_target_constants().len(),
    ] {
        if actual != group.ambient_arity() {
            return Err(GeneratedResidualAffineWhenBadError::WrongArity {
                expected: group.ambient_arity(),
                actual,
            });
        }
    }
    stats.target_constant_comparison_entries = checked_mul(
        "generated affine WhenBad target constant comparison entries",
        group.ambient_arity(),
        2,
    )?;
    check_limit(
        "generated affine WhenBad target constant comparison entries",
        stats.target_constant_comparison_entries,
        limits.max_target_constant_comparison_entries,
    )?;
    for value in target
        .constants()
        .iter()
        .chain(pending.transformed_target_constants())
    {
        let prospective = checked_add(
            "generated affine WhenBad target constant comparison integer bits",
            stats.target_constant_comparison_integer_bits,
            integer_magnitude_bits(value)?,
        )?;
        check_limit(
            "generated affine WhenBad target constant comparison integer bits",
            prospective,
            limits.max_target_constant_comparison_integer_bits,
        )?;
        stats.target_constant_comparison_integer_bits = prospective;
    }
    if target.constants() != pending.transformed_target_constants() {
        return Err(GeneratedResidualAffineWhenBadError::TargetConstantsMismatch);
    }

    stats.inventory_terminals_inspected = inventory.terminals().len();
    check_limit(
        "generated affine WhenBad inventory terminals inspected",
        stats.inventory_terminals_inspected,
        limits.max_inventory_terminals_inspected,
    )?;
    let mut terminal = None;
    for candidate in inventory
        .terminals()
        .iter()
        .filter(|terminal| terminal.locator() == target.locator())
    {
        if terminal.replace(candidate).is_some() {
            return Err(GeneratedResidualAffineWhenBadError::DuplicateTargetTerminal);
        }
    }
    let terminal = terminal.ok_or(GeneratedResidualAffineWhenBadError::TargetTerminalMissing)?;
    if terminal.outcome()
        != (GeneratedResidualAffineInventoryTerminalOutcome::Actionable {
            case_ordinal: target_case_ordinal,
        })
    {
        return Err(GeneratedResidualAffineWhenBadError::TargetTerminalOutcomeMismatch);
    }
    let terminal_branch = terminal
        .source_branch()
        .ok_or(GeneratedResidualAffineWhenBadError::TargetBranchAllocationMismatch)?;
    let terminal_guard = terminal
        .guard_composition()
        .ok_or(GeneratedResidualAffineWhenBadError::TargetGuardCompositionAllocationMismatch)?;
    if !Arc::ptr_eq(target.source_cover(), terminal.source_cover()) {
        return Err(GeneratedResidualAffineWhenBadError::TargetCoverAllocationMismatch);
    }
    if !Arc::ptr_eq(target.source_branch(), terminal_branch) {
        return Err(GeneratedResidualAffineWhenBadError::TargetBranchAllocationMismatch);
    }
    if !Arc::ptr_eq(target.guard_composition(), terminal_guard) {
        return Err(GeneratedResidualAffineWhenBadError::TargetGuardCompositionAllocationMismatch);
    }
    if !Arc::ptr_eq(target.source_branch().source_cover(), target.source_cover()) {
        return Err(GeneratedResidualAffineWhenBadError::TargetBranchCoverAllocationMismatch);
    }
    if !Arc::ptr_eq(
        target.guard_composition().source_cover(),
        target.source_cover(),
    ) {
        return Err(GeneratedResidualAffineWhenBadError::TargetGuardCoverAllocationMismatch);
    }
    if !Arc::ptr_eq(
        target.guard_composition().source_branch(),
        target.source_branch(),
    ) {
        return Err(GeneratedResidualAffineWhenBadError::TargetGuardBranchAllocationMismatch);
    }
    if !matches!(
        target.source_branch().outcome(),
        ResidualAffineBranchSystemOutcome::GuardedAffineMap
    ) {
        return Err(GeneratedResidualAffineWhenBadError::TargetBranchOutcomeNotGuardedAffineMap);
    }
    if let Some(entry_ordinal) = target
        .guard_composition()
        .first_contradiction_entry_ordinal()
    {
        return Err(
            GeneratedResidualAffineWhenBadError::TargetGuardContradiction { entry_ordinal },
        );
    }
    let integer_system = target
        .source_branch()
        .integer_system_arc()
        .ok_or(GeneratedResidualAffineWhenBadError::MissingTargetIntegerSystem)?;
    if integer_system.ambient_arity() != context.index_count() {
        return Err(GeneratedResidualAffineWhenBadError::WrongArity {
            expected: context.index_count(),
            actual: integer_system.ambient_arity(),
        });
    }
    let relation = pending.relation_for_affine_when_bad().clone();
    for requested in [
        family_fingerprint.len(),
        relation.family_fingerprint().len(),
    ] {
        check_limit(
            "generated affine WhenBad family fingerprint bytes",
            requested,
            limits.max_family_fingerprint_bytes,
        )?;
    }
    for requested in [
        context.fingerprint().len(),
        relation.context_fingerprint().len(),
    ] {
        check_limit(
            "generated affine WhenBad context fingerprint bytes",
            requested,
            limits.max_context_fingerprint_bytes,
        )?;
    }
    let relation_comparison_bytes = checked_add(
        "generated affine WhenBad scope fingerprint comparison bytes",
        checked_add(
            "generated affine WhenBad scope fingerprint comparison bytes",
            family_fingerprint.len(),
            relation.family_fingerprint().len(),
        )?,
        checked_add(
            "generated affine WhenBad scope fingerprint comparison bytes",
            context.fingerprint().len(),
            relation.context_fingerprint().len(),
        )?,
    )?;
    let prospective_scope_comparison_bytes = checked_add(
        "generated affine WhenBad scope fingerprint comparison bytes",
        stats.scope_fingerprint_comparison_bytes,
        relation_comparison_bytes,
    )?;
    check_limit(
        "generated affine WhenBad scope fingerprint comparison bytes",
        prospective_scope_comparison_bytes,
        limits.max_scope_fingerprint_comparison_bytes,
    )?;
    stats.scope_fingerprint_comparison_bytes = prospective_scope_comparison_bytes;
    stats.family_fingerprint_bytes = stats
        .family_fingerprint_bytes
        .max(relation.family_fingerprint().len());
    stats.context_fingerprint_bytes = stats
        .context_fingerprint_bytes
        .max(relation.context_fingerprint().len());
    if relation.family_fingerprint() != family_fingerprint {
        return Err(GeneratedResidualAffineWhenBadError::PrivateRelationFamilyMismatch);
    }
    if relation.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedResidualAffineWhenBadError::PrivateRelationContextMismatch);
    }
    if relation.arity() != context.index_count() {
        return Err(GeneratedResidualAffineWhenBadError::WrongArity {
            expected: context.index_count(),
            actual: relation.arity(),
        });
    }
    stats.private_relation_terms = relation.terms().len();
    stats.private_relation_guards = relation.guarded_nonzero_conditions().len();
    check_limit(
        "generated affine WhenBad private relation terms",
        stats.private_relation_terms,
        limits.max_private_relation_terms,
    )?;
    check_limit(
        "generated affine WhenBad private relation guards",
        stats.private_relation_guards,
        limits.max_private_relation_guards,
    )?;
    for condition in relation.guarded_nonzero_conditions() {
        let prospective = checked_add(
            "generated affine WhenBad private relation origins",
            stats.private_relation_origins,
            condition.origins().len(),
        )?;
        check_limit(
            "generated affine WhenBad private relation origins",
            prospective,
            limits.max_private_relation_origins,
        )?;
        stats.private_relation_origins = prospective;

        let remaining_source_terms = limits
            .max_total_source_terms
            .checked_sub(stats.private_relation_source_terms)
            .ok_or(GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
                resource: "generated affine WhenBad private relation source terms",
            })?;
        let remaining_source_exponent_entries = limits
            .max_total_source_exponent_entries
            .checked_sub(stats.private_relation_source_exponent_entries)
            .ok_or(GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
                resource: "generated affine WhenBad private relation source exponent entries",
            })?;
        let remaining_source_integer_bits = limits
            .max_total_source_integer_bits
            .checked_sub(stats.private_relation_source_integer_bits)
            .ok_or(GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
                resource: "generated affine WhenBad private relation source integer bits",
            })?;
        let census = context.preflight_polynomial_validation_payload_with_limits(
            condition.polynomial(),
            limits.arithmetic.exact_algebra,
            remaining_source_terms,
            remaining_source_exponent_entries,
            remaining_source_integer_bits,
        )?;
        stats.private_relation_source_terms = checked_add(
            "generated affine WhenBad private relation source terms",
            stats.private_relation_source_terms,
            census.source_terms(),
        )?;
        stats.private_relation_source_exponent_entries = checked_add(
            "generated affine WhenBad private relation source exponent entries",
            stats.private_relation_source_exponent_entries,
            census.source_exponent_entries(),
        )?;
        stats.private_relation_source_integer_bits = checked_add(
            "generated affine WhenBad private relation source integer bits",
            stats.private_relation_source_integer_bits,
            census.source_integer_bits(),
        )?;
    }

    let rhs_term_upper_bound = relation.terms().len();
    let rhs_with_center_limit = limits.max_rhs_terms.checked_add(1).unwrap_or(usize::MAX);
    check_limit(
        "generated affine WhenBad RHS term upper bound",
        rhs_term_upper_bound,
        rhs_with_center_limit,
    )?;
    stats.private_relation_shift_components = checked_mul(
        "generated affine WhenBad private relation shift components",
        relation.terms().len(),
        context.index_count(),
    )?;
    check_limit(
        "generated affine WhenBad private relation shift components",
        stats.private_relation_shift_components,
        limits.max_private_relation_shift_components,
    )?;

    let mut centered_pivot_found = false;
    let mut rhs_ordinal = 0usize;
    for (shift, coefficient) in relation.terms() {
        if shift.arity() != context.index_count() {
            return Err(GeneratedResidualAffineWhenBadError::WrongArity {
                expected: context.index_count(),
                actual: shift.arity(),
            });
        }
        let remaining_source_terms = limits
            .max_total_source_terms
            .checked_sub(stats.private_relation_source_terms)
            .ok_or(GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
                resource: "generated affine WhenBad private relation source terms",
            })?;
        let remaining_source_exponent_entries = limits
            .max_total_source_exponent_entries
            .checked_sub(stats.private_relation_source_exponent_entries)
            .ok_or(GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
                resource: "generated affine WhenBad private relation source exponent entries",
            })?;
        let remaining_source_integer_bits = limits
            .max_total_source_integer_bits
            .checked_sub(stats.private_relation_source_integer_bits)
            .ok_or(GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
                resource: "generated affine WhenBad private relation source integer bits",
            })?;
        let census = context.preflight_validation_payload_with_limits(
            coefficient,
            limits.arithmetic.exact_algebra,
            remaining_source_terms,
            remaining_source_exponent_entries,
            remaining_source_integer_bits,
        )?;
        stats.private_relation_source_terms = checked_add(
            "generated affine WhenBad private relation source terms",
            stats.private_relation_source_terms,
            census.source_terms(),
        )?;
        stats.private_relation_source_exponent_entries = checked_add(
            "generated affine WhenBad private relation source exponent entries",
            stats.private_relation_source_exponent_entries,
            census.source_exponent_entries(),
        )?;
        stats.private_relation_source_integer_bits = checked_add(
            "generated affine WhenBad private relation source integer bits",
            stats.private_relation_source_integer_bits,
            census.source_integer_bits(),
        )?;
        if shift.values().iter().all(|&value| value == 0) {
            centered_pivot_found = true;
            if !coefficient.raw().is_one() {
                return Err(
                    GeneratedResidualAffineWhenBadError::PrivateRelationNonunitCenteredPivot,
                );
            }
        } else {
            if coefficient.is_zero() {
                return Err(
                    GeneratedResidualAffineWhenBadError::PrivateRelationZeroRhsCoefficient {
                        rhs_ordinal,
                    },
                );
            }
            rhs_ordinal = checked_add("generated affine WhenBad RHS terms", rhs_ordinal, 1)?;
        }
    }
    if !centered_pivot_found {
        return Err(GeneratedResidualAffineWhenBadError::PrivateRelationMissingCenteredPivot);
    }
    stats.rhs_terms = rhs_ordinal;
    check_limit(
        "generated affine WhenBad RHS terms",
        stats.rhs_terms,
        limits.max_rhs_terms,
    )?;

    stats.private_relation_manifest_bytes =
        relation.stable_manifest_byte_len_with_limit(limits.max_private_relation_manifest_bytes)?;

    // Admit every authority-local retained allocation before entering the
    // target-ordering builder or copying either complete manifest. Shared
    // matcher/branch/relation payload is referenced by existing Arcs and is
    // deliberately not deep-charged a second time.
    let retained_base_envelope = authority_local_retained_base_envelope(
        context.index_count(),
        stats.private_relation_manifest_bytes,
        limits.target_ordering,
    )?;
    check_limit(
        "generated affine WhenBad retained bytes",
        retained_base_envelope,
        limits.max_retained_bytes,
    )?;
    let remaining_ordering_manifest_capacity_bytes = limits
        .max_retained_bytes
        .checked_sub(retained_base_envelope)
        .ok_or(GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
            resource: "generated affine WhenBad retained bytes",
        })?;
    let mut effective_ordering_limits = limits.target_ordering;
    effective_ordering_limits.max_manifest_bytes = effective_ordering_limits
        .max_manifest_bytes
        .min(remaining_ordering_manifest_capacity_bytes / 4);
    let target_ordering = AffineStartParametricEliminationOrdering::try_new_from_residual_branch(
        family,
        context,
        target.source_cover().clone(),
        inventory.source_queue().ordering(),
        target.source_branch().clone(),
        effective_ordering_limits,
    )?;
    let ordering_branch = target_ordering
        .residual_branch()
        .ok_or(GeneratedResidualAffineWhenBadError::TargetOrderingBranchAllocationMismatch)?;
    if !Arc::ptr_eq(ordering_branch, target.source_branch()) {
        return Err(GeneratedResidualAffineWhenBadError::TargetOrderingBranchAllocationMismatch);
    }
    let retained_manifest_capacity_envelope = checked_mul(
        "generated affine WhenBad retained ordering manifest bytes",
        target_ordering.stable_manifest().len(),
        4,
    )?;
    stats.retained_byte_envelope = checked_add(
        "generated affine WhenBad retained bytes",
        retained_base_envelope,
        retained_manifest_capacity_envelope,
    )?;
    check_limit(
        "generated affine WhenBad retained bytes",
        stats.retained_byte_envelope,
        limits.max_retained_bytes,
    )?;

    let target_ordering_manifest = Arc::new(try_copy_string(
        target_ordering.stable_manifest(),
        "generated affine WhenBad target ordering manifest",
    )?);
    let private_relation_manifest =
        Arc::new(relation.stable_manifest_with_limit(stats.private_relation_manifest_bytes)?);
    if private_relation_manifest.len() != stats.private_relation_manifest_bytes {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch);
    }

    let sector = SectorMask::try_new(target_ordering.sector().active_bits().iter().copied())?;
    let coefficient_translation = IndexShift::try_new(
        pending.coefficient_translation().values().iter().copied(),
        context.index_count(),
    )?;
    let key_center = IndexShift::try_new(
        pending.key_center().values().iter().copied(),
        context.index_count(),
    )?;
    let binding = GeneratedResidualAffineWhenBadBinding {
        source_case_ordinal: matcher.source_case_ordinal(),
        source_group_ordinal: matcher.source_group_ordinal(),
        pivot_ordinal: pending_pivot_ordinal,
        target_case_ordinal,
        target_position_in_matching_list: target_position,
        target_locator: target.locator(),
        target_ordinal_within_group: target.ordinal_within_group(),
        sector,
        coefficient_translation,
        key_center,
        target_ordering_manifest,
        private_relation_manifest_bytes: stats.private_relation_manifest_bytes,
        rhs_terms: stats.rhs_terms,
    };

    stats.retained_bytes = authority_local_observed_retained_bytes(
        &binding,
        &target_ordering,
        &private_relation_manifest,
    )?;
    if stats.retained_bytes > stats.retained_byte_envelope {
        return Err(
            GeneratedResidualAffineWhenBadError::RetainedByteEnvelopeExceeded {
                observed: stats.retained_bytes,
                admitted: stats.retained_byte_envelope,
            },
        );
    }

    Ok(AuthenticatedGeneratedResidualAffineWhenBadInput {
        binding,
        target_ordering,
        target_cover: target.source_cover().clone(),
        target_branch: target.source_branch().clone(),
        target_guard_composition: target.guard_composition().clone(),
        relation,
        private_relation_manifest,
        limits,
        stats,
        matcher,
    })
}

fn try_copy_string(
    source: &str,
    resource: &'static str,
) -> Result<String, GeneratedResidualAffineWhenBadError> {
    let mut output = String::new();
    output.try_reserve_exact(source.len()).map_err(|_| {
        GeneratedResidualAffineWhenBadError::AllocationFailure {
            resource,
            requested: source.len(),
        }
    })?;
    output.push_str(source);
    Ok(output)
}

fn authority_local_retained_base_envelope(
    arity: usize,
    private_relation_manifest_bytes: usize,
    ordering_limits: AffineParametricOrderingLimits,
) -> Result<usize, GeneratedResidualAffineWhenBadError> {
    let mut bytes = size_of::<AuthenticatedGeneratedResidualAffineWhenBadInput>();
    // The target ordering owns two Arc<Vec<usize>> allocations. Charge the
    // exact reservation requests used by that constructor and a factor-two
    // capacity envelope for allocator rounding.
    bytes = checked_add(
        "generated affine WhenBad retained bytes",
        bytes,
        checked_mul(
            "generated affine WhenBad retained bytes",
            2,
            arc_payload_control_and_padding_byte_bound::<Vec<usize>>()?,
        )?,
    )?;
    let position_requests = checked_add(
        "generated affine WhenBad retained bytes",
        arity.min(ordering_limits.max_constant_positions),
        arity.min(ordering_limits.max_symbolic_positions),
    )?;
    bytes = checked_add(
        "generated affine WhenBad retained bytes",
        bytes,
        checked_mul(
            "generated affine WhenBad retained bytes",
            checked_mul(
                "generated affine WhenBad retained bytes",
                position_requests,
                2,
            )?,
            size_of::<usize>(),
        )?,
    )?;
    // Ordering manifest, copied binding manifest, and private-row manifest.
    bytes = checked_add(
        "generated affine WhenBad retained bytes",
        bytes,
        checked_mul(
            "generated affine WhenBad retained bytes",
            3,
            arc_payload_control_and_padding_byte_bound::<String>()?,
        )?,
    )?;
    bytes = checked_add(
        "generated affine WhenBad retained bytes",
        bytes,
        checked_mul(
            "generated affine WhenBad retained bytes",
            private_relation_manifest_bytes,
            2,
        )?,
    )?;
    // One sector bit vector and two full-arity retained shifts in the binding,
    // each under the same factor-two capacity envelope.
    bytes = checked_add(
        "generated affine WhenBad retained bytes",
        bytes,
        checked_mul(
            "generated affine WhenBad retained bytes",
            checked_mul("generated affine WhenBad retained bytes", arity, 2)?,
            size_of::<bool>(),
        )?,
    )?;
    checked_add(
        "generated affine WhenBad retained bytes",
        bytes,
        checked_mul(
            "generated affine WhenBad retained bytes",
            checked_mul("generated affine WhenBad retained bytes", arity, 4)?,
            size_of::<i64>(),
        )?,
    )
}

fn authority_local_observed_retained_bytes(
    binding: &GeneratedResidualAffineWhenBadBinding,
    target_ordering: &AffineStartParametricEliminationOrdering,
    private_relation_manifest: &Arc<String>,
) -> Result<usize, GeneratedResidualAffineWhenBadError> {
    let ordering_owned = target_ordering.owned_retained_byte_bound().ok_or(
        GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
            resource: "generated affine WhenBad retained bytes",
        },
    )?;
    let ordering_heap = ordering_owned
        .checked_sub(size_of::<AffineStartParametricEliminationOrdering>())
        .ok_or(GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
            resource: "generated affine WhenBad retained bytes",
        })?;
    let mut bytes = checked_add(
        "generated affine WhenBad retained bytes",
        size_of::<AuthenticatedGeneratedResidualAffineWhenBadInput>(),
        ordering_heap,
    )?;
    for observed in [
        binding.sector.owned_retained_byte_bound(),
        binding.coefficient_translation.owned_retained_byte_bound(),
        binding.key_center.owned_retained_byte_bound(),
    ] {
        bytes = checked_add(
            "generated affine WhenBad retained bytes",
            bytes,
            observed.ok_or(GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
                resource: "generated affine WhenBad retained bytes",
            })?,
        )?;
    }
    bytes = checked_add(
        "generated affine WhenBad retained bytes",
        bytes,
        arc_string_owned_byte_bound(&binding.target_ordering_manifest)?,
    )?;
    checked_add(
        "generated affine WhenBad retained bytes",
        bytes,
        arc_string_owned_byte_bound(private_relation_manifest)?,
    )
}

fn arc_payload_control_and_padding_byte_bound<T>()
-> Result<usize, GeneratedResidualAffineWhenBadError> {
    checked_add(
        "generated affine WhenBad retained bytes",
        checked_mul(
            "generated affine WhenBad retained bytes",
            size_of::<AtomicUsize>(),
            2,
        )?,
        checked_add(
            "generated affine WhenBad retained bytes",
            align_of::<T>().saturating_sub(1),
            size_of::<T>(),
        )?,
    )
}

fn arc_string_owned_byte_bound(
    value: &Arc<String>,
) -> Result<usize, GeneratedResidualAffineWhenBadError> {
    checked_add(
        "generated affine WhenBad retained bytes",
        arc_payload_control_and_padding_byte_bound::<String>()?,
        value.capacity(),
    )
}

fn integer_magnitude_bits(value: &Integer) -> Result<usize, GeneratedResidualAffineWhenBadError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(
        |_| GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
            resource: "generated affine WhenBad integer magnitude bits",
        },
    )
}

const fn portable_usize(value: u64) -> usize {
    if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadError> {
    left.checked_add(right)
        .ok_or(GeneratedResidualAffineWhenBadError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadError> {
    left.checked_mul(right)
        .ok_or(GeneratedResidualAffineWhenBadError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedResidualAffineWhenBadError> {
    if requested > limit {
        Err(GeneratedResidualAffineWhenBadError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_point_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadPointError> {
    left.checked_add(right)
        .ok_or(GeneratedResidualAffineWhenBadPointError::ResourceCountOverflow { resource })
}

fn checked_point_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadPointError> {
    left.checked_mul(right)
        .ok_or(GeneratedResidualAffineWhenBadPointError::ResourceCountOverflow { resource })
}

fn check_point_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedResidualAffineWhenBadPointError> {
    if requested > limit {
        Err(GeneratedResidualAffineWhenBadPointError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

// `preflight_specialize_polynomial` performs an outer validation followed by
// the validation inside its raw preflight.  `specialize_polynomial` repeats
// that pair before execution.  Across those four validations, coefficients are
// scanned four times and exponent entries have four validation plus four
// canonical-order comparison envelopes.  The two raw preflights each add one
// coefficient/integer-growth scan, one coefficient-capacity scan, and at most
// one full index-exponent scan.  Hence the complete preflight/validation call
// chain is bounded by 8 source-term and 10 source-exponent-entry passes per
// predicate.  The execution pass itself is represented once by `source_terms`
// and `source_exponent_entries`, not duplicated here.
const GENERATED_AFFINE_POINT_PREFLIGHT_VALIDATION_TERM_SCAN_MULTIPLIER: usize = 8;
const GENERATED_AFFINE_POINT_PREFLIGHT_VALIDATION_EXPONENT_SCAN_MULTIPLIER: usize = 10;

fn accumulate_point_specialization_preflight(
    stats: &mut GeneratedResidualAffineWhenBadPointStats,
    preflight: ParametricPolynomialSpecializationPreflight,
    limits: GeneratedResidualAffineWhenBadPointLimits,
) -> Result<(), GeneratedResidualAffineWhenBadPointError> {
    stats.source_terms = checked_point_add(
        "generated affine WhenBad point specialization source terms",
        stats.source_terms,
        preflight.source_terms(),
    )?;
    check_point_limit(
        "generated affine WhenBad point specialization source terms",
        stats.source_terms,
        limits.max_source_terms,
    )?;
    stats.source_exponent_entries = checked_point_add(
        "generated affine WhenBad point specialization source exponent entries",
        stats.source_exponent_entries,
        preflight.source_exponent_entries(),
    )?;
    check_point_limit(
        "generated affine WhenBad point specialization source exponent entries",
        stats.source_exponent_entries,
        limits.max_source_exponent_entries,
    )?;
    stats.preflight_validation_source_term_scan_bound = checked_point_add(
        "generated affine WhenBad point preflight/validation source-term scan bound",
        stats.preflight_validation_source_term_scan_bound,
        checked_point_mul(
            "generated affine WhenBad point preflight/validation source-term scan bound",
            preflight.source_terms(),
            GENERATED_AFFINE_POINT_PREFLIGHT_VALIDATION_TERM_SCAN_MULTIPLIER,
        )?,
    )?;
    check_point_limit(
        "generated affine WhenBad point preflight/validation source-term scan bound",
        stats.preflight_validation_source_term_scan_bound,
        limits.max_preflight_validation_source_term_scan_bound,
    )?;
    stats.preflight_validation_source_exponent_entry_scan_bound = checked_point_add(
        "generated affine WhenBad point preflight/validation source exponent-entry scan bound",
        stats.preflight_validation_source_exponent_entry_scan_bound,
        checked_point_mul(
            "generated affine WhenBad point preflight/validation source exponent-entry scan bound",
            preflight.source_exponent_entries(),
            GENERATED_AFFINE_POINT_PREFLIGHT_VALIDATION_EXPONENT_SCAN_MULTIPLIER,
        )?,
    )?;
    check_point_limit(
        "generated affine WhenBad point preflight/validation source exponent-entry scan bound",
        stats.preflight_validation_source_exponent_entry_scan_bound,
        limits.max_preflight_validation_source_exponent_entry_scan_bound,
    )?;
    stats.output_term_bound = checked_point_add(
        "generated affine WhenBad point specialization output term bound",
        stats.output_term_bound,
        preflight.output_term_bound(),
    )?;
    check_point_limit(
        "generated affine WhenBad point specialization output term bound",
        stats.output_term_bound,
        limits.max_output_term_bound,
    )?;
    stats.output_exponent_entry_bound = checked_point_add(
        "generated affine WhenBad point specialization output exponent-entry bound",
        stats.output_exponent_entry_bound,
        preflight.output_exponent_entry_bound(),
    )?;
    check_point_limit(
        "generated affine WhenBad point specialization output exponent-entry bound",
        stats.output_exponent_entry_bound,
        limits.max_output_exponent_entry_bound,
    )?;
    stats.power_operation_bound = checked_point_add(
        "generated affine WhenBad point specialization power-operation bound",
        stats.power_operation_bound,
        preflight.power_operation_bound(),
    )?;
    check_point_limit(
        "generated affine WhenBad point specialization power-operation bound",
        stats.power_operation_bound,
        limits.max_power_operation_bound,
    )?;
    stats.largest_output_integer_bit_bound = stats
        .largest_output_integer_bit_bound
        .max(preflight.largest_output_integer_bit_bound());
    check_point_limit(
        "generated affine WhenBad point specialization largest output integer-bit bound",
        stats.largest_output_integer_bit_bound,
        limits.max_largest_output_integer_bit_bound,
    )?;
    stats.integer_bit_work_bound = checked_point_add(
        "generated affine WhenBad point specialization integer-bit work bound",
        stats.integer_bit_work_bound,
        preflight.integer_bit_work_bound(),
    )?;
    check_point_limit(
        "generated affine WhenBad point specialization integer-bit work bound",
        stats.integer_bit_work_bound,
        limits.max_integer_bit_work_bound,
    )?;
    stats.retained_output_term_bound = checked_point_add(
        "generated affine WhenBad point specialization retained output term bound",
        stats.retained_output_term_bound,
        preflight.retained_output_term_bound(),
    )?;
    check_point_limit(
        "generated affine WhenBad point specialization retained output term bound",
        stats.retained_output_term_bound,
        limits.max_retained_output_term_bound,
    )?;
    stats.retained_output_byte_bound = checked_point_add(
        "generated affine WhenBad point specialization retained output byte bound",
        stats.retained_output_byte_bound,
        preflight.retained_output_byte_bound(),
    )?;
    check_point_limit(
        "generated affine WhenBad point specialization retained output byte bound",
        stats.retained_output_byte_bound,
        limits.max_retained_output_byte_bound,
    )
}

fn register_generated_affine_point_match(
    matched_cases: &mut usize,
) -> Result<(), GeneratedResidualAffineWhenBadPointError> {
    *matched_cases = checked_point_add(
        "generated affine WhenBad point matched cases",
        *matched_cases,
        1,
    )?;
    if *matched_cases > 1 {
        Err(
            GeneratedResidualAffineWhenBadPointError::PartitionEvaluationMismatch {
                matched_cases: *matched_cases,
            },
        )
    } else {
        Ok(())
    }
}

fn map_generated_affine_descent_error(
    error: GeneratedResidualAffineWhenBadDescentError,
) -> GeneratedResidualAffineWhenBadError {
    match error {
        GeneratedResidualAffineWhenBadDescentError::Authority(error) => error,
        GeneratedResidualAffineWhenBadDescentError::Core(error) => {
            map_generated_affine_core_error(error)
        }
        GeneratedResidualAffineWhenBadDescentError::AuthenticatedRhsCountMismatch { .. }
        | GeneratedResidualAffineWhenBadDescentError::PrivateRhsCountMismatch { .. } => {
            GeneratedResidualAffineWhenBadError::ReplayMismatch
        }
    }
}

fn map_generated_affine_core_error(error: WhenBadCoreError) -> GeneratedResidualAffineWhenBadError {
    match error {
        WhenBadCoreError::WrongArity { expected, actual } => {
            GeneratedResidualAffineWhenBadError::WrongArity { expected, actual }
        }
        WhenBadCoreError::BoundaryArithmeticOverflow { coordinate } => {
            GeneratedResidualAffineWhenBadError::BoundaryArithmeticOverflow { coordinate }
        }
        WhenBadCoreError::DescentArithmeticOverflow => {
            GeneratedResidualAffineWhenBadError::DescentArithmeticOverflow
        }
        WhenBadCoreError::RetainedCapacityEnvelopeExceeded {
            observed_bytes,
            admitted_bytes,
            ..
        } => GeneratedResidualAffineWhenBadError::RetainedByteEnvelopeExceeded {
            observed: observed_bytes,
            admitted: admitted_bytes,
        },
        WhenBadCoreError::ResourceCountOverflow { resource } => {
            GeneratedResidualAffineWhenBadError::ResourceCountOverflow { resource }
        }
        WhenBadCoreError::AllocationFailure {
            resource,
            requested,
        } => GeneratedResidualAffineWhenBadError::AllocationFailure {
            resource,
            requested,
        },
        WhenBadCoreError::ParametricRelation(error) => {
            GeneratedResidualAffineWhenBadError::Relation(error)
        }
    }
}

fn map_generated_affine_condition_error(
    error: GeneratedResidualAffineConditionAccumulatorError,
) -> GeneratedResidualAffineWhenBadError {
    match error {
        GeneratedResidualAffineConditionAccumulatorError::RetainedPolynomialByteEnvelopeExceeded {
            observed,
            admitted,
        }
        | GeneratedResidualAffineConditionAccumulatorError::RetainedByteEnvelopeExceeded {
            observed,
            admitted,
        } => GeneratedResidualAffineWhenBadError::RetainedByteEnvelopeExceeded {
            observed,
            admitted,
        },
        GeneratedResidualAffineConditionAccumulatorError::SymbolicaPanic { stage } => {
            GeneratedResidualAffineWhenBadError::SymbolicaPanic { stage }
        }
        GeneratedResidualAffineConditionAccumulatorError::ResourceLimit {
            resource,
            requested,
            limit,
        } => GeneratedResidualAffineWhenBadError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        GeneratedResidualAffineConditionAccumulatorError::ResourceCountOverflow { resource } => {
            GeneratedResidualAffineWhenBadError::ResourceCountOverflow { resource }
        }
        GeneratedResidualAffineConditionAccumulatorError::AllocationFailure {
            resource,
            requested,
        } => GeneratedResidualAffineWhenBadError::AllocationFailure {
            resource,
            requested,
        },
        GeneratedResidualAffineConditionAccumulatorError::ParametricCoefficient(error) => {
            GeneratedResidualAffineWhenBadError::ParametricCoefficient(error)
        }
        GeneratedResidualAffineConditionAccumulatorError::ConfiguredExponentLimit { .. } => {
            GeneratedResidualAffineWhenBadError::ConditionInvariant {
                stage: "configured exponent validation",
            }
        }
        GeneratedResidualAffineConditionAccumulatorError::FreePositionOutOfRange { .. }
        | GeneratedResidualAffineConditionAccumulatorError::NonIncreasingFreePositions { .. }
        | GeneratedResidualAffineConditionAccumulatorError::NonfreePrivateIndexSupport { .. } => {
            GeneratedResidualAffineWhenBadError::ConditionInvariant {
                stage: "target free-position validation",
            }
        }
        GeneratedResidualAffineConditionAccumulatorError::MissingPrivateShift { .. }
        | GeneratedResidualAffineConditionAccumulatorError::UnexpectedPrivateShift { .. }
        | GeneratedResidualAffineConditionAccumulatorError::SourceScopeMismatch { .. }
        | GeneratedResidualAffineConditionAccumulatorError::WrongPrivateShiftArity { .. } => {
            GeneratedResidualAffineWhenBadError::ConditionInvariant {
                stage: "condition source authentication",
            }
        }
        GeneratedResidualAffineConditionAccumulatorError::InheritedConditionIsIdenticallyZero {
            ..
        } => GeneratedResidualAffineWhenBadError::ConditionInvariant {
            stage: "inherited target premise classification",
        },
        GeneratedResidualAffineConditionAccumulatorError::AssociateDependencyMismatch { .. }
        | GeneratedResidualAffineConditionAccumulatorError::InternalInvariant { .. }
        | GeneratedResidualAffineConditionAccumulatorError::RetainedByteCensusMismatch => {
            GeneratedResidualAffineWhenBadError::ConditionInvariant {
                stage: "condition canonicalization replay",
            }
        }
    }
}

fn remaining_limit(
    resource: &'static str,
    limit: usize,
    consumed: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadError> {
    limit
        .checked_sub(consumed)
        .ok_or(GeneratedResidualAffineWhenBadError::ResourceLimit {
            resource,
            requested: consumed,
            limit,
        })
}

fn generated_affine_descent_inline_allowance() -> usize {
    size_of::<GeneratedResidualAffineWhenBadDescentReady>()
        .max(size_of::<GeneratedResidualAffineWhenBadDescentUnsupported>())
}

fn generated_affine_reserved_outer_heap_budget(
    child_limits: GeneratedResidualAffineWhenBadLimits,
) -> Result<usize, GeneratedResidualAffineWhenBadError> {
    remaining_limit(
        "generated affine WhenBad retained bytes",
        child_limits.max_retained_bytes,
        generated_affine_descent_inline_allowance(),
    )
}

fn generated_affine_incremental_retained_bytes(
    full: usize,
    inline: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadError> {
    full.checked_sub(inline)
        .ok_or(GeneratedResidualAffineWhenBadError::ConditionInvariant {
            stage: "child retained-byte root census",
        })
}

fn generated_affine_full_child_retained_limit(
    outer_heap_budget: usize,
    prior_incremental: usize,
    child_inline: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadError> {
    checked_add(
        "generated affine WhenBad retained bytes",
        remaining_limit(
            "generated affine WhenBad retained bytes",
            outer_heap_budget,
            prior_incremental,
        )?,
        child_inline,
    )
}

fn intersect_exact_algebra_limits(
    left: crate::ExactAlgebraLimits,
    right: crate::ExactAlgebraLimits,
) -> crate::ExactAlgebraLimits {
    crate::ExactAlgebraLimits {
        max_exponent: left.max_exponent.min(right.max_exponent),
        max_polynomial_terms: left.max_polynomial_terms.min(right.max_polynomial_terms),
        max_term_operations: left.max_term_operations.min(right.max_term_operations),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineConditionPayloadPreflight {
    variable_map_entry_comparisons: usize,
    shared_allocation_identity_comparisons: usize,
    final_invariant_entries: usize,
}

fn generated_affine_condition_shared_allocation_preflight(
    total_inputs: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadError> {
    let unordered_pairs = checked_mul(
        "generated affine WhenBad condition comparison-pair bound",
        total_inputs,
        total_inputs.saturating_sub(1),
    )? / 2;
    checked_add(
        "generated affine WhenBad condition shared-allocation comparison bound",
        checked_mul(
            "generated affine WhenBad condition shared-allocation comparison bound",
            total_inputs,
            2,
        )?,
        checked_mul(
            "generated affine WhenBad condition shared-allocation comparison bound",
            unordered_pairs,
            4,
        )?,
    )
}

impl GeneratedAffineConditionPayloadPreflight {
    pub(crate) fn total_units(self) -> Result<usize, GeneratedResidualAffineWhenBadError> {
        checked_add(
            "generated affine WhenBad condition payload comparison units",
            checked_add(
                "generated affine WhenBad condition payload comparison units",
                self.variable_map_entry_comparisons,
                self.shared_allocation_identity_comparisons,
            )?,
            self.final_invariant_entries,
        )
    }
}

pub(crate) fn generated_affine_condition_payload_preflight(
    context: &ParametricCoefficientContext,
    input: &AuthenticatedGeneratedResidualAffineWhenBadInput,
    total_inputs: usize,
) -> Result<GeneratedAffineConditionPayloadPreflight, GeneratedResidualAffineWhenBadError> {
    let ambient_variables = checked_add(
        "generated affine WhenBad condition ambient variables",
        context.base().variables().len(),
        context.index_count(),
    )?;
    let unordered_pairs = checked_mul(
        "generated affine WhenBad condition comparison-pair bound",
        total_inputs,
        total_inputs.saturating_sub(1),
    )? / 2;

    // Every input is authenticated once and every retained row is replayed
    // once.  In the hostile all-unique case, each unordered pair performs one
    // exact comparison and one associate proof; the latter authenticates both
    // variable maps.  This is a complete pre-child upper bound.
    let variable_map_entry_comparisons = checked_mul(
        "generated affine WhenBad condition variable-map comparison bound",
        ambient_variables,
        checked_add(
            "generated affine WhenBad condition variable-map comparison bound",
            checked_mul(
                "generated affine WhenBad condition variable-map comparison bound",
                total_inputs,
                2,
            )?,
            checked_mul(
                "generated affine WhenBad condition variable-map comparison bound",
                unordered_pairs,
                3,
            )?,
        )?,
    )?;
    // Each retained row proves two copy seams.  In the hostile all-distinct
    // case, both insertion and final replay additionally scan two shared Arc
    // seams against every earlier row: 2I + 4*C(I,2).
    let shared_allocation_identity_comparisons =
        generated_affine_condition_shared_allocation_preflight(total_inputs)?;

    let mut retained_terms = 0usize;
    let mut retained_exponent_entries = 0usize;
    for entry in input.target_guard_composition().entries() {
        retained_terms = checked_add(
            "generated affine WhenBad condition retained-term bound",
            retained_terms,
            entry.mapped_polynomial().term_count(),
        )?;
        retained_exponent_entries = checked_add(
            "generated affine WhenBad condition retained-exponent bound",
            retained_exponent_entries,
            entry.mapped_polynomial().raw().exponents.len(),
        )?;
    }
    for guard in input.relation().guarded_nonzero_conditions() {
        retained_terms = checked_add(
            "generated affine WhenBad condition retained-term bound",
            retained_terms,
            guard.polynomial().term_count(),
        )?;
        retained_exponent_entries = checked_add(
            "generated affine WhenBad condition retained-exponent bound",
            retained_exponent_entries,
            guard.polynomial().raw().exponents.len(),
        )?;
    }
    let mut source_shift_components = 0usize;
    for (shift, coefficient) in input.relation().terms() {
        retained_terms = checked_add(
            "generated affine WhenBad condition retained-term bound",
            retained_terms,
            coefficient.raw().denominator.nterms(),
        )?;
        retained_exponent_entries = checked_add(
            "generated affine WhenBad condition retained-exponent bound",
            retained_exponent_entries,
            coefficient.raw().denominator.exponents.len(),
        )?;
        source_shift_components = checked_add(
            "generated affine WhenBad condition source-shift bound",
            source_shift_components,
            shift.arity(),
        )?;
    }

    // The child documents its final complete-pass formula as
    // F + M + 3I + 4R + S + 7T + 4E + H.  Use R<=I and S=I before the child
    // has classified constants or canonicalized duplicate rows.
    let mut final_invariant_entries = checked_add(
        "generated affine WhenBad condition final-invariant bound",
        input.target_ordering().symbolic_positions().len(),
        context.index_count(),
    )?;
    for (entries, factor) in [
        (total_inputs, 8usize),
        (retained_terms, 7usize),
        (retained_exponent_entries, 4usize),
        (source_shift_components, 1usize),
    ] {
        final_invariant_entries = checked_add(
            "generated affine WhenBad condition final-invariant bound",
            final_invariant_entries,
            checked_mul(
                "generated affine WhenBad condition final-invariant bound",
                entries,
                factor,
            )?,
        )?;
    }
    Ok(GeneratedAffineConditionPayloadPreflight {
        variable_map_entry_comparisons,
        shared_allocation_identity_comparisons,
        final_invariant_entries,
    })
}

fn projected_generated_affine_condition_limits(
    context: &ParametricCoefficientContext,
    input: &AuthenticatedGeneratedResidualAffineWhenBadInput,
    retained_bytes_remaining: usize,
    payload_units_remaining: usize,
    payload_bytes_remaining: usize,
    payload: GeneratedAffineConditionPayloadPreflight,
) -> Result<GeneratedResidualAffineConditionAccumulatorLimits, GeneratedResidualAffineWhenBadError>
{
    let outer = input.limits();
    let authority_stats = input.stats();
    let mut limits = GeneratedResidualAffineConditionAccumulatorLimits::default();
    limits.exact_algebra =
        intersect_exact_algebra_limits(limits.exact_algebra, outer.arithmetic.exact_algebra);
    limits.max_context_fingerprint_bytes = limits
        .max_context_fingerprint_bytes
        .min(outer.max_context_fingerprint_bytes);
    limits.max_context_fingerprint_comparison_bytes = limits
        .max_context_fingerprint_comparison_bytes
        .min(payload_bytes_remaining);
    limits.max_variable_map_entry_comparisons = limits
        .max_variable_map_entry_comparisons
        .min(payload.variable_map_entry_comparisons);
    limits.max_shared_allocation_identity_comparisons = limits
        .max_shared_allocation_identity_comparisons
        .min(payload.shared_allocation_identity_comparisons);
    limits.max_ambient_variables = limits.max_ambient_variables.min(checked_add(
        "generated affine WhenBad condition ambient variables",
        input.target_ordering().arity(),
        context.base().variables().len(),
    )?);
    limits.max_free_positions = limits.max_free_positions.min(outer.max_free_positions);
    limits.max_condition_inputs = limits.max_condition_inputs.min(outer.max_condition_inputs);
    limits.max_source_inputs = limits
        .max_source_inputs
        .min(outer.max_condition_source_inputs);
    limits.max_condition_sources = limits
        .max_condition_sources
        .min(outer.max_condition_sources);
    limits.max_unique_rows = limits.max_unique_rows.min(checked_add(
        "generated affine WhenBad unique condition rows",
        outer.max_inherited_conditions,
        outer.max_candidate_conditions,
    )?);
    limits.max_unique_inherited_rows = limits
        .max_unique_inherited_rows
        .min(outer.max_inherited_conditions);
    limits.max_unique_candidate_rows = limits
        .max_unique_candidate_rows
        .min(outer.max_candidate_conditions);
    limits.max_source_shift_components = limits
        .max_source_shift_components
        .min(outer.max_condition_source_shift_components);
    limits.max_input_polynomial_terms = limits.max_input_polynomial_terms.min(remaining_limit(
        "generated affine WhenBad aggregate source terms",
        outer.max_total_source_terms,
        authority_stats.private_relation_source_terms(),
    )?);
    limits.max_input_polynomial_exponent_entries = limits
        .max_input_polynomial_exponent_entries
        .min(remaining_limit(
            "generated affine WhenBad aggregate source exponent entries",
            outer.max_total_source_exponent_entries,
            authority_stats.private_relation_source_exponent_entries(),
        )?);
    limits.max_input_polynomial_integer_bits =
        limits
            .max_input_polynomial_integer_bits
            .min(remaining_limit(
                "generated affine WhenBad aggregate source integer bits",
                outer.max_total_source_integer_bits,
                authority_stats.private_relation_source_integer_bits(),
            )?);
    limits.max_dependency_exponent_entries = limits
        .max_dependency_exponent_entries
        .min(outer.max_condition_dependency_exponent_entries);
    limits.max_equality_comparisons = limits
        .max_equality_comparisons
        .min(outer.max_condition_equality_comparisons);
    limits.max_equality_term_units = limits
        .max_equality_term_units
        .min(outer.max_condition_equality_term_units);
    limits.max_equality_exponent_entries = limits
        .max_equality_exponent_entries
        .min(outer.max_condition_equality_exponent_entries);
    limits.max_equality_integer_bits = limits
        .max_equality_integer_bits
        .min(outer.max_condition_equality_integer_bits);
    limits.max_associate_checks = limits.max_associate_checks.min(outer.max_associate_checks);
    limits.max_associate_term_units = limits
        .max_associate_term_units
        .min(outer.max_associate_term_pairs);
    limits.max_associate_exponent_entries = limits
        .max_associate_exponent_entries
        .min(outer.max_associate_exponent_entries);
    limits.max_associate_integer_bits = limits
        .max_associate_integer_bits
        .min(outer.max_associate_integer_bits);
    limits.max_retained_polynomial_terms = limits
        .max_retained_polynomial_terms
        .min(outer.max_retained_polynomial_terms);
    limits.max_retained_polynomial_exponent_entries = limits
        .max_retained_polynomial_exponent_entries
        .min(outer.max_retained_polynomial_exponent_entries);
    limits.max_retained_polynomial_integer_bits = limits
        .max_retained_polynomial_integer_bits
        .min(outer.max_retained_polynomial_integer_bits);
    limits.max_retained_polynomial_display_bytes = limits
        .max_retained_polynomial_display_bytes
        .min(outer.max_retained_polynomial_display_bytes);
    limits.max_retained_polynomial_owned_bytes = limits
        .max_retained_polynomial_owned_bytes
        .min(retained_bytes_remaining);
    limits.max_retained_bytes = limits.max_retained_bytes.min(retained_bytes_remaining);
    limits.max_final_invariant_entries = limits
        .max_final_invariant_entries
        .min(payload.final_invariant_entries);
    check_limit(
        "generated affine WhenBad condition payload comparison units",
        payload.total_units()?,
        payload_units_remaining,
    )?;
    Ok(limits)
}

/// Build the one mandatory, source-ordered condition stream after descent
/// succeeds.  This helper remains private because its denominator records
/// retain exact relation shifts.
pub(crate) fn compile_generated_residual_affine_when_bad_conditions(
    context: &ParametricCoefficientContext,
    input: &AuthenticatedGeneratedResidualAffineWhenBadInput,
    retained_bytes_remaining: usize,
    payload_units_remaining: usize,
    payload_bytes_remaining: usize,
) -> Result<
    GeneratedResidualAffineConditionAccumulatorCertificate,
    GeneratedResidualAffineWhenBadError,
> {
    let target_guard_count = input.target_guard_composition().entries().len();
    let relation_guard_count = input.relation().guarded_nonzero_conditions().len();
    let coefficient_count = input.relation().terms().len();
    let limits = input.limits();
    check_limit(
        "generated affine WhenBad target guard entries",
        target_guard_count,
        limits.max_target_guard_entries,
    )?;
    check_limit(
        "generated affine WhenBad relation guard condition inputs",
        relation_guard_count,
        limits.max_relation_guard_condition_inputs,
    )?;
    check_limit(
        "generated affine WhenBad coefficient denominator condition inputs",
        coefficient_count,
        limits.max_coefficient_denominator_condition_inputs,
    )?;
    let total_inputs = checked_add(
        "generated affine WhenBad condition inputs",
        checked_add(
            "generated affine WhenBad condition inputs",
            target_guard_count,
            relation_guard_count,
        )?,
        coefficient_count,
    )?;
    check_limit(
        "generated affine WhenBad condition inputs",
        total_inputs,
        limits.max_condition_inputs,
    )?;
    check_limit(
        "generated affine WhenBad condition source inputs",
        total_inputs,
        limits.max_condition_source_inputs,
    )?;
    let payload = generated_affine_condition_payload_preflight(context, input, total_inputs)?;
    let child_limits = projected_generated_affine_condition_limits(
        context,
        input,
        retained_bytes_remaining,
        payload_units_remaining,
        payload_bytes_remaining,
        payload,
    )?;

    // The pivot denominator is first by specification, irrespective of the
    // BTree position of the zero shift.  RHS denominators then follow the
    // relation's canonical nonzero-shift order.
    let mut denominator_conditions = Vec::new();
    denominator_conditions
        .try_reserve_exact(coefficient_count)
        .map_err(|_| GeneratedResidualAffineWhenBadError::AllocationFailure {
            resource: "generated affine WhenBad denominator conditions",
            requested: coefficient_count,
        })?;
    let (pivot_shift, pivot_coefficient) = input
        .relation()
        .terms()
        .iter()
        .find(|(shift, _)| shift.values().iter().all(|&value| value == 0))
        .ok_or(GeneratedResidualAffineWhenBadError::PrivateRelationMissingCenteredPivot)?;
    denominator_conditions.push((
        GeneratedResidualAffineConditionRelationTerm::Pivot,
        pivot_shift,
        pivot_coefficient
            .try_copy_prevalidated_denominator_condition()
            .map_err(
                |resource| GeneratedResidualAffineWhenBadError::AllocationFailure {
                    resource,
                    requested: pivot_coefficient.raw().denominator.nterms(),
                },
            )?,
    ));
    let mut rhs_ordinal = 0usize;
    for (shift, coefficient) in input.relation().terms() {
        if shift.values().iter().all(|&value| value == 0) {
            continue;
        }
        denominator_conditions.push((
            GeneratedResidualAffineConditionRelationTerm::Rhs { rhs_ordinal },
            shift,
            coefficient
                .try_copy_prevalidated_denominator_condition()
                .map_err(
                    |resource| GeneratedResidualAffineWhenBadError::AllocationFailure {
                        resource,
                        requested: coefficient.raw().denominator.nterms(),
                    },
                )?,
        ));
        rhs_ordinal = checked_add(
            "generated affine WhenBad denominator RHS ordinal",
            rhs_ordinal,
            1,
        )?;
    }
    if denominator_conditions.len() != coefficient_count
        || rhs_ordinal != input.binding().rhs_terms()
    {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch);
    }

    let mut condition_inputs = Vec::new();
    condition_inputs
        .try_reserve_exact(total_inputs)
        .map_err(|_| GeneratedResidualAffineWhenBadError::AllocationFailure {
            resource: "generated affine WhenBad condition input transcript",
            requested: total_inputs,
        })?;
    for (entry_ordinal, entry) in input
        .target_guard_composition()
        .entries()
        .iter()
        .enumerate()
    {
        condition_inputs.push(GeneratedResidualAffineConditionInput::new(
            entry.mapped_polynomial(),
            GeneratedResidualAffineConditionScope::InheritedTargetPremise,
            GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
                entry_ordinal,
                structural_locus_ordinal: entry.structural_locus_ordinal(),
            },
            None,
        ));
    }
    for (guard_ordinal, guard) in input
        .relation()
        .guarded_nonzero_conditions()
        .iter()
        .enumerate()
    {
        condition_inputs.push(GeneratedResidualAffineConditionInput::new(
            guard.polynomial(),
            GeneratedResidualAffineConditionScope::CandidateRequired,
            GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard {
                guard_ordinal,
            },
            None,
        ));
    }
    for (term, shift, polynomial) in &denominator_conditions {
        condition_inputs.push(GeneratedResidualAffineConditionInput::new(
            polynomial,
            GeneratedResidualAffineConditionScope::CandidateRequired,
            GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator { term: *term },
            Some(shift),
        ));
    }
    if condition_inputs.len() != total_inputs {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch);
    }
    accumulate_generated_residual_affine_conditions(
        context,
        input.target_ordering().symbolic_positions(),
        condition_inputs,
        child_limits,
    )
    .map_err(map_generated_affine_condition_error)
}

fn generated_affine_condition_retained_polynomial_census(
    stats: GeneratedResidualAffineConditionAccumulatorStats,
) -> (usize, usize, usize, usize) {
    (
        stats.retained_polynomial_terms(),
        stats.retained_polynomial_exponent_entries(),
        stats.retained_polynomial_integer_bits(),
        stats.retained_polynomial_display_bytes(),
    )
}

fn generated_affine_condition_view(
    certificate: &GeneratedResidualAffineConditionAccumulatorCertificate,
    ordinal: usize,
) -> Option<GeneratedResidualAffineWhenBadConditionView> {
    let row = certificate.rows().get(ordinal)?;
    Some(GeneratedResidualAffineWhenBadConditionView {
        ordinal,
        scope: row.scope().into(),
        index_dependent: row.is_index_dependent(),
        source_count: row.source_input_ordinals().len(),
    })
}

fn first_identically_zero_candidate_condition_input(
    certificate: &GeneratedResidualAffineConditionAccumulatorCertificate,
) -> Option<usize> {
    use crate::generated_residual_affine_condition_accumulator::GeneratedResidualAffineConditionInputClass;

    certificate.inputs().iter().find_map(|input| {
        matches!(
            input.class(),
            GeneratedResidualAffineConditionInputClass::IdenticallyZeroCandidate
        )
        .then_some(input.ordinal())
    })
}

fn map_generated_affine_pullback_gate_error(
    error: GeneratedResidualAffineWhenBadPullbackGateError,
) -> GeneratedResidualAffineWhenBadError {
    match error {
        GeneratedResidualAffineWhenBadPullbackGateError::WrongContext => {
            GeneratedResidualAffineWhenBadError::WrongContext
        }
        GeneratedResidualAffineWhenBadPullbackGateError::WrongArity { expected, actual } => {
            GeneratedResidualAffineWhenBadError::WrongArity { expected, actual }
        }
        GeneratedResidualAffineWhenBadPullbackGateError::MissingTargetIntegerSystem => {
            GeneratedResidualAffineWhenBadError::MissingTargetIntegerSystem
        }
        GeneratedResidualAffineWhenBadPullbackGateError::RetainedByteEnvelopeExceeded {
            observed,
            admitted,
        } => GeneratedResidualAffineWhenBadError::RetainedByteEnvelopeExceeded {
            observed,
            admitted,
        },
        GeneratedResidualAffineWhenBadPullbackGateError::ResourceLimit {
            resource,
            requested,
            limit,
        } => GeneratedResidualAffineWhenBadError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        GeneratedResidualAffineWhenBadPullbackGateError::ResourceCountOverflow { resource } => {
            GeneratedResidualAffineWhenBadError::ResourceCountOverflow { resource }
        }
        GeneratedResidualAffineWhenBadPullbackGateError::AllocationFailure {
            resource,
            requested,
        } => GeneratedResidualAffineWhenBadError::AllocationFailure {
            resource,
            requested,
        },
        GeneratedResidualAffineWhenBadPullbackGateError::SymbolicaPanic { stage } => {
            GeneratedResidualAffineWhenBadError::SymbolicaPanic { stage }
        }
        GeneratedResidualAffineWhenBadPullbackGateError::ParametricCoefficient(error) => {
            GeneratedResidualAffineWhenBadError::ParametricCoefficient(error)
        }
        GeneratedResidualAffineWhenBadPullbackGateError::Composition(error) => {
            GeneratedResidualAffineWhenBadError::Composition(error)
        }
        GeneratedResidualAffineWhenBadPullbackGateError::Core(error) => {
            map_generated_affine_core_error(error)
        }
        GeneratedResidualAffineWhenBadPullbackGateError::SchemaMismatch
        | GeneratedResidualAffineWhenBadPullbackGateError::CompositionPlanIntegerSystemAllocationMismatch
        | GeneratedResidualAffineWhenBadPullbackGateError::CompositionPlanStatsMismatch { .. }
        | GeneratedResidualAffineWhenBadPullbackGateError::ReadyAuthorityMismatch
        | GeneratedResidualAffineWhenBadPullbackGateError::PrivateRhsCountMismatch { .. }
        | GeneratedResidualAffineWhenBadPullbackGateError::DescentProofMismatch { .. }
        | GeneratedResidualAffineWhenBadPullbackGateError::ActivationObligationMismatch { .. }
        | GeneratedResidualAffineWhenBadPullbackGateError::ReplayMismatch => {
            GeneratedResidualAffineWhenBadError::ReplayMismatch
        }
        GeneratedResidualAffineWhenBadPullbackGateError::NonfreeNumeratorSupport { .. } => {
            GeneratedResidualAffineWhenBadError::ConditionInvariant {
                stage: "pullback numerator free-position validation",
            }
        }
        GeneratedResidualAffineWhenBadPullbackGateError::ZeroNumeratorGate { .. } => {
            GeneratedResidualAffineWhenBadError::ConditionInvariant {
                stage: "pullback numerator nonzero validation",
            }
        }
    }
}

fn projected_generated_affine_pullback_gate_limits(
    ready: &GeneratedResidualAffineWhenBadDescentReady,
    conditions: &GeneratedResidualAffineConditionAccumulatorCertificate,
    private_payload_comparison_bytes: usize,
) -> Result<GeneratedResidualAffineWhenBadPullbackGateLimits, GeneratedResidualAffineWhenBadError> {
    let outer = ready.input().limits();
    let authority = ready.input().stats();
    let descent = ready.stats();
    let condition = conditions.stats();
    let mut limits = GeneratedResidualAffineWhenBadPullbackGateLimits::from_outer(outer);
    let prior_source_terms = checked_add(
        "generated affine WhenBad aggregate source terms",
        authority.private_relation_source_terms(),
        condition.input_polynomial_terms(),
    )?;
    let prior_source_exponents = checked_add(
        "generated affine WhenBad aggregate source exponent entries",
        authority.private_relation_source_exponent_entries(),
        condition.input_polynomial_exponent_entries(),
    )?;
    let prior_source_bits = checked_add(
        "generated affine WhenBad aggregate source integer bits",
        authority.private_relation_source_integer_bits(),
        condition.input_polynomial_integer_bits(),
    )?;
    limits.max_total_source_terms = remaining_limit(
        "generated affine WhenBad aggregate source terms",
        outer.max_total_source_terms,
        prior_source_terms,
    )?;
    limits.max_total_source_exponent_entries = remaining_limit(
        "generated affine WhenBad aggregate source exponent entries",
        outer.max_total_source_exponent_entries,
        prior_source_exponents,
    )?;
    limits.max_total_source_integer_bits = remaining_limit(
        "generated affine WhenBad aggregate source integer bits",
        outer.max_total_source_integer_bits,
        prior_source_bits,
    )?;
    limits.max_retained_polynomial_terms = remaining_limit(
        "generated affine WhenBad retained polynomial terms",
        outer.max_retained_polynomial_terms,
        condition.retained_polynomial_terms(),
    )?;
    limits.max_retained_polynomial_exponent_entries = remaining_limit(
        "generated affine WhenBad retained polynomial exponent entries",
        outer.max_retained_polynomial_exponent_entries,
        condition.retained_polynomial_exponent_entries(),
    )?;
    limits.max_retained_polynomial_integer_bits = remaining_limit(
        "generated affine WhenBad retained polynomial integer bits",
        outer.max_retained_polynomial_integer_bits,
        condition.retained_polynomial_integer_bits(),
    )?;
    limits.max_retained_polynomial_display_bytes = remaining_limit(
        "generated affine WhenBad retained polynomial display bytes",
        outer.max_retained_polynomial_display_bytes,
        condition.retained_polynomial_display_bytes(),
    )?;
    let retained_before = checked_add(
        "generated affine WhenBad retained bytes",
        generated_affine_incremental_retained_bytes(
            descent.retained_bytes(),
            size_of::<GeneratedResidualAffineWhenBadDescentReady>(),
        )?,
        generated_affine_incremental_retained_bytes(
            condition.retained_bytes(),
            size_of::<GeneratedResidualAffineConditionAccumulatorCertificate>(),
        )?,
    )?;
    limits.max_retained_bytes = generated_affine_full_child_retained_limit(
        generated_affine_reserved_outer_heap_budget(outer)?,
        retained_before,
        size_of::<GeneratedResidualAffineWhenBadPullbackGateCertificate>(),
    )?;
    let payload_units_before = checked_add(
        "generated affine WhenBad payload comparison units",
        descent.payload_comparison_units_observed(),
        generated_affine_condition_payload_comparison_units(condition)?,
    )?;
    limits.max_payload_comparison_units = remaining_limit(
        "generated affine WhenBad payload comparison units",
        outer.max_payload_comparison_units,
        payload_units_before,
    )?;
    limits.max_payload_comparison_bytes = remaining_limit(
        "generated affine WhenBad payload comparison bytes",
        outer.max_payload_comparison_bytes,
        checked_add(
            "generated affine WhenBad payload comparison bytes",
            private_payload_comparison_bytes,
            condition.context_fingerprint_comparison_bytes(),
        )?,
    )?;
    limits.max_payload_comparison_integer_bits = remaining_limit(
        "generated affine WhenBad payload comparison integer bits",
        outer.max_payload_comparison_integer_bits,
        condition.equality_integer_bits(),
    )?;
    Ok(limits)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct GeneratedAffineLocusAssemblyStats {
    exact_comparisons: usize,
    associate_checks: usize,
    comparison_term_pairs: usize,
    comparison_exponent_entries: usize,
    comparison_integer_bits: usize,
    payload_comparison_units: usize,
    payload_comparison_bytes: usize,
    bad_atoms: usize,
    retained_terms: usize,
    retained_exponent_entries: usize,
    retained_integer_bits: usize,
    retained_byte_envelope: usize,
    retained_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct GeneratedAffineComparisonWork {
    checks: usize,
    term_pairs: usize,
    exponent_entries: usize,
    integer_bits: usize,
}

impl GeneratedAffineComparisonWork {
    fn checked_add(
        self,
        resource_prefix: &'static str,
        other: Self,
    ) -> Result<Self, GeneratedResidualAffineWhenBadError> {
        Ok(Self {
            checks: checked_add(resource_prefix, self.checks, other.checks)?,
            term_pairs: checked_add(resource_prefix, self.term_pairs, other.term_pairs)?,
            exponent_entries: checked_add(
                resource_prefix,
                self.exponent_entries,
                other.exponent_entries,
            )?,
            integer_bits: checked_add(resource_prefix, self.integer_bits, other.integer_bits)?,
        })
    }

    fn checked_pair(
        left: &ParametricPolynomial,
        right: &ParametricPolynomial,
    ) -> Result<Self, GeneratedResidualAffineWhenBadError> {
        Ok(Self {
            checks: 1,
            term_pairs: checked_mul(
                "generated affine WhenBad comparison term pairs",
                left.term_count(),
                right.term_count(),
            )?,
            exponent_entries: checked_add(
                "generated affine WhenBad comparison exponent entries",
                left.raw().exponents.len(),
                right.raw().exponents.len(),
            )?,
            integer_bits: checked_add(
                "generated affine WhenBad comparison integer bits",
                generated_affine_polynomial_integer_bits(left)?,
                generated_affine_polynomial_integer_bits(right)?,
            )?,
        })
    }

    fn check_limits(
        self,
        limits: GeneratedResidualAffineWhenBadLimits,
    ) -> Result<(), GeneratedResidualAffineWhenBadError> {
        check_limit(
            "generated affine WhenBad associate checks",
            self.checks,
            limits.max_associate_checks,
        )?;
        check_limit(
            "generated affine WhenBad associate term pairs",
            self.term_pairs,
            limits.max_associate_term_pairs,
        )?;
        check_limit(
            "generated affine WhenBad associate exponent entries",
            self.exponent_entries,
            limits.max_associate_exponent_entries,
        )?;
        check_limit(
            "generated affine WhenBad associate integer bits",
            self.integer_bits,
            limits.max_associate_integer_bits,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct GeneratedAffineRelativeValidationCensus {
    aggregate: GeneratedAffineComparisonWork,
    equality_checks: usize,
    associate_checks: usize,
    associate_term_pairs: usize,
}

fn generated_affine_polynomial_integer_bits(
    polynomial: &ParametricPolynomial,
) -> Result<usize, GeneratedResidualAffineWhenBadError> {
    polynomial
        .raw()
        .coefficients
        .iter()
        .try_fold(0usize, |total, coefficient| {
            checked_add(
                "generated affine WhenBad comparison integer bits",
                total,
                integer_magnitude_bits(coefficient)?,
            )
        })
}

fn generated_affine_condition_comparison_work(
    stats: GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<GeneratedAffineComparisonWork, GeneratedResidualAffineWhenBadError> {
    Ok(GeneratedAffineComparisonWork {
        checks: checked_add(
            "generated affine WhenBad associate checks",
            stats.equality_comparisons(),
            stats.associate_checks(),
        )?,
        term_pairs: checked_add(
            "generated affine WhenBad associate term pairs",
            stats.equality_term_units(),
            stats.associate_term_units(),
        )?,
        exponent_entries: checked_add(
            "generated affine WhenBad associate exponent entries",
            stats.equality_exponent_entries(),
            stats.associate_exponent_entries(),
        )?,
        integer_bits: checked_add(
            "generated affine WhenBad associate integer bits",
            stats.equality_integer_bits(),
            stats.associate_integer_bits(),
        )?,
    })
}

pub(crate) fn check_generated_affine_condition_comparison_limits(
    stats: GeneratedResidualAffineConditionAccumulatorStats,
    limits: GeneratedResidualAffineWhenBadLimits,
) -> Result<(), GeneratedResidualAffineWhenBadError> {
    generated_affine_condition_comparison_work(stats)?.check_limits(limits)
}

fn generated_affine_condition_payload_comparison_units(
    stats: GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<usize, GeneratedResidualAffineWhenBadError> {
    checked_add(
        "generated affine WhenBad payload comparison units",
        checked_add(
            "generated affine WhenBad payload comparison units",
            stats.variable_map_entry_comparisons(),
            stats.shared_allocation_identity_comparisons(),
        )?,
        stats.final_invariant_entries(),
    )
}

fn generated_affine_assembly_comparison_work(
    stats: GeneratedAffineLocusAssemblyStats,
) -> Result<GeneratedAffineComparisonWork, GeneratedResidualAffineWhenBadError> {
    Ok(GeneratedAffineComparisonWork {
        checks: checked_add(
            "generated affine WhenBad associate checks",
            stats.exact_comparisons,
            stats.associate_checks,
        )?,
        term_pairs: stats.comparison_term_pairs,
        exponent_entries: stats.comparison_exponent_entries,
        integer_bits: stats.comparison_integer_bits,
    })
}

struct GeneratedAffineRelativeProblemAssembly {
    structural_loci: Vec<ParametricPolynomial>,
    inherited_truths: Vec<AffineWhenBadInheritedTruth>,
    clauses: Vec<AffineWhenBadFormulaClause>,
    stats: GeneratedAffineLocusAssemblyStats,
}

#[derive(Clone, Copy)]
enum AssemblyReserve {
    Structural,
    Inherited,
    Clauses,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct GeneratedAffineLocusAssemblyAdmission {
    structural_sources: usize,
    inherited_truths: usize,
    clauses: usize,
    bad_atoms: usize,
    retained_byte_envelope: usize,
}

impl GeneratedAffineLocusAssemblyAdmission {
    fn observe_structural_source(
        &mut self,
        polynomial: &ParametricPolynomial,
    ) -> Result<(), GeneratedResidualAffineWhenBadError> {
        self.structural_sources = checked_add(
            "generated affine WhenBad structural source census",
            self.structural_sources,
            1,
        )?;
        self.retained_byte_envelope = checked_add(
            "generated affine WhenBad structural assembly bytes",
            self.retained_byte_envelope,
            polynomial.owned_retained_byte_bound().ok_or(
                GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
                    resource: "generated affine WhenBad structural polynomial bytes",
                },
            )?,
        )?;
        Ok(())
    }

    fn observe_inherited_truth(&mut self) -> Result<(), GeneratedResidualAffineWhenBadError> {
        self.inherited_truths = checked_add(
            "generated affine WhenBad inherited-truth census",
            self.inherited_truths,
            1,
        )?;
        Ok(())
    }

    fn observe_clause(&mut self, atoms: usize) -> Result<(), GeneratedResidualAffineWhenBadError> {
        self.clauses = checked_add(
            "generated affine WhenBad bad-clause census",
            self.clauses,
            1,
        )?;
        self.bad_atoms = checked_add(
            "generated affine WhenBad bad-atom census",
            self.bad_atoms,
            atoms,
        )?;
        Ok(())
    }

    fn finish(
        mut self,
        ready: &GeneratedResidualAffineWhenBadDescentReady,
        conditions: &GeneratedResidualAffineConditionAccumulatorCertificate,
        pullbacks: &GeneratedResidualAffineWhenBadPullbackGateCertificate,
    ) -> Result<Self, GeneratedResidualAffineWhenBadError> {
        let limits = ready.input().limits();
        check_limit(
            "generated affine WhenBad inherited conditions",
            self.inherited_truths,
            limits.max_inherited_conditions,
        )?;
        check_limit(
            "generated affine WhenBad bad clauses",
            self.clauses,
            limits.max_bad_clauses,
        )?;
        check_limit(
            "generated affine WhenBad bad atoms",
            self.bad_atoms,
            limits.max_bad_atoms,
        )?;

        let structural_capacity = checked_mul(
            "generated affine WhenBad structural capacity envelope",
            self.structural_sources,
            2,
        )?;
        let inherited_capacity = checked_mul(
            "generated affine WhenBad inherited capacity envelope",
            self.inherited_truths,
            2,
        )?;
        let clause_capacity = checked_mul(
            "generated affine WhenBad clause capacity envelope",
            self.clauses,
            2,
        )?;
        self.retained_byte_envelope = checked_add(
            "generated affine WhenBad structural assembly bytes",
            self.retained_byte_envelope,
            checked_add(
                "generated affine WhenBad structural assembly bytes",
                size_of::<GeneratedAffineRelativeProblemAssembly>(),
                checked_add(
                    "generated affine WhenBad structural assembly bytes",
                    checked_mul(
                        "generated affine WhenBad structural assembly bytes",
                        structural_capacity,
                        size_of::<ParametricPolynomial>(),
                    )?,
                    checked_add(
                        "generated affine WhenBad structural assembly bytes",
                        checked_mul(
                            "generated affine WhenBad structural assembly bytes",
                            inherited_capacity,
                            size_of::<AffineWhenBadInheritedTruth>(),
                        )?,
                        checked_mul(
                            "generated affine WhenBad structural assembly bytes",
                            clause_capacity,
                            size_of::<AffineWhenBadFormulaClause>(),
                        )?,
                    )?,
                )?,
            )?,
        )?;
        let child_retained = checked_add(
            "generated affine WhenBad retained bytes",
            checked_add(
                "generated affine WhenBad retained bytes",
                generated_affine_incremental_retained_bytes(
                    ready.stats().retained_bytes(),
                    size_of::<GeneratedResidualAffineWhenBadDescentReady>(),
                )?,
                generated_affine_incremental_retained_bytes(
                    conditions.stats().retained_bytes(),
                    size_of::<GeneratedResidualAffineConditionAccumulatorCertificate>(),
                )?,
            )?,
            generated_affine_incremental_retained_bytes(
                pullbacks.stats().retained_bytes(),
                size_of::<GeneratedResidualAffineWhenBadPullbackGateCertificate>(),
            )?,
        )?;
        check_limit(
            "generated affine WhenBad retained bytes",
            checked_add(
                "generated affine WhenBad retained bytes",
                child_retained,
                generated_affine_incremental_retained_bytes(
                    self.retained_byte_envelope,
                    size_of::<GeneratedAffineRelativeProblemAssembly>(),
                )?,
            )?,
            generated_affine_reserved_outer_heap_budget(limits)?,
        )?;
        Ok(self)
    }
}

impl GeneratedAffineRelativeProblemAssembly {
    fn try_with_precharged_capacities(
        structural_loci: usize,
        inherited_truths: usize,
        clauses: usize,
        retained_byte_envelope: usize,
    ) -> Result<Self, GeneratedResidualAffineWhenBadError> {
        let mut result = Self {
            structural_loci: Vec::new(),
            inherited_truths: Vec::new(),
            clauses: Vec::new(),
            stats: GeneratedAffineLocusAssemblyStats {
                retained_byte_envelope,
                retained_bytes: size_of::<Self>(),
                ..GeneratedAffineLocusAssemblyStats::default()
            },
        };
        for (resource, requested, reserve) in [
            (
                "generated affine WhenBad structural-locus assembly",
                structural_loci,
                AssemblyReserve::Structural,
            ),
            (
                "generated affine WhenBad inherited-truth assembly",
                inherited_truths,
                AssemblyReserve::Inherited,
            ),
            (
                "generated affine WhenBad bad-clause assembly",
                clauses,
                AssemblyReserve::Clauses,
            ),
        ] {
            match reserve {
                AssemblyReserve::Structural => result
                    .structural_loci
                    .try_reserve_exact(requested)
                    .map_err(|_| GeneratedResidualAffineWhenBadError::AllocationFailure {
                        resource,
                        requested,
                    })?,
                AssemblyReserve::Inherited => result
                    .inherited_truths
                    .try_reserve_exact(requested)
                    .map_err(|_| GeneratedResidualAffineWhenBadError::AllocationFailure {
                        resource,
                        requested,
                    })?,
                AssemblyReserve::Clauses => {
                    result.clauses.try_reserve_exact(requested).map_err(|_| {
                        GeneratedResidualAffineWhenBadError::AllocationFailure {
                            resource,
                            requested,
                        }
                    })?
                }
            }
        }
        result.observe_vector_backing_bytes()?;
        Ok(result)
    }

    fn observe_vector_backing_bytes(&mut self) -> Result<(), GeneratedResidualAffineWhenBadError> {
        self.stats.retained_bytes = checked_add(
            "generated affine WhenBad structural assembly bytes",
            self.stats.retained_bytes,
            checked_add(
                "generated affine WhenBad structural assembly bytes",
                checked_mul(
                    "generated affine WhenBad structural assembly bytes",
                    self.structural_loci.capacity(),
                    size_of::<ParametricPolynomial>(),
                )?,
                checked_add(
                    "generated affine WhenBad structural assembly bytes",
                    checked_mul(
                        "generated affine WhenBad structural assembly bytes",
                        self.inherited_truths.capacity(),
                        size_of::<AffineWhenBadInheritedTruth>(),
                    )?,
                    checked_mul(
                        "generated affine WhenBad structural assembly bytes",
                        self.clauses.capacity(),
                        size_of::<AffineWhenBadFormulaClause>(),
                    )?,
                )?,
            )?,
        )?;
        if self.stats.retained_bytes > self.stats.retained_byte_envelope {
            return Err(
                GeneratedResidualAffineWhenBadError::RetainedByteEnvelopeExceeded {
                    observed: self.stats.retained_bytes,
                    admitted: self.stats.retained_byte_envelope,
                },
            );
        }
        Ok(())
    }

    fn push_inherited(
        &mut self,
        value: AffineWhenBadInheritedTruth,
        limits: GeneratedResidualAffineWhenBadLimits,
    ) -> Result<(), GeneratedResidualAffineWhenBadError> {
        let requested = checked_add(
            "generated affine WhenBad inherited conditions",
            self.inherited_truths.len(),
            1,
        )?;
        check_limit(
            "generated affine WhenBad inherited conditions",
            requested,
            limits.max_inherited_conditions,
        )?;
        if self.inherited_truths.len() == self.inherited_truths.capacity() {
            return Err(GeneratedResidualAffineWhenBadError::ConditionInvariant {
                stage: "precharged inherited-truth capacity",
            });
        }
        self.inherited_truths.push(value);
        Ok(())
    }

    fn push_clause(
        &mut self,
        value: AffineWhenBadFormulaClause,
        atom_count: usize,
        limits: GeneratedResidualAffineWhenBadLimits,
    ) -> Result<(), GeneratedResidualAffineWhenBadError> {
        let requested = checked_add(
            "generated affine WhenBad bad clauses",
            self.clauses.len(),
            1,
        )?;
        check_limit(
            "generated affine WhenBad bad clauses",
            requested,
            limits.max_bad_clauses,
        )?;
        let requested_atoms = checked_add(
            "generated affine WhenBad bad atoms",
            self.stats.bad_atoms,
            atom_count,
        )?;
        check_limit(
            "generated affine WhenBad bad atoms",
            requested_atoms,
            limits.max_bad_atoms,
        )?;
        if self.clauses.len() == self.clauses.capacity() {
            return Err(GeneratedResidualAffineWhenBadError::ConditionInvariant {
                stage: "precharged bad-clause capacity",
            });
        }
        self.clauses.push(value);
        self.stats.bad_atoms = requested_atoms;
        Ok(())
    }

    fn intern(
        &mut self,
        context: &ParametricCoefficientContext,
        polynomial: &ParametricPolynomial,
        limits: GeneratedResidualAffineWhenBadLimits,
        prior_condition_work: GeneratedAffineComparisonWork,
        prior_payload_units: usize,
        prior_payload_bytes: usize,
    ) -> Result<usize, GeneratedResidualAffineWhenBadError> {
        if polynomial.is_zero()
            || !context.polynomial_depends_on_indices_with_limits(
                polynomial,
                limits.arithmetic.exact_algebra,
            )?
        {
            return Err(GeneratedResidualAffineWhenBadError::ConditionInvariant {
                stage: "relative structural-locus classification",
            });
        }
        let source_terms = polynomial.term_count();
        let source_exponents = polynomial.raw().exponents.len();
        let source_bits = generated_affine_polynomial_integer_bits(polynomial)?;

        for (ordinal, retained) in self.structural_loci.iter().enumerate() {
            let pair_work = GeneratedAffineComparisonWork::checked_pair(polynomial, retained)?;
            prior_condition_work
                .checked_add(
                    "generated affine WhenBad comparison work",
                    generated_affine_assembly_comparison_work(self.stats)?,
                )?
                .checked_add("generated affine WhenBad comparison work", pair_work)?
                .check_limits(limits)?;
            self.stats.exact_comparisons = checked_add(
                "generated affine WhenBad associate checks",
                self.stats.exact_comparisons,
                1,
            )?;
            self.stats.comparison_term_pairs = checked_add(
                "generated affine WhenBad associate term pairs",
                self.stats.comparison_term_pairs,
                pair_work.term_pairs,
            )?;
            self.stats.comparison_exponent_entries = checked_add(
                "generated affine WhenBad associate exponent entries",
                self.stats.comparison_exponent_entries,
                pair_work.exponent_entries,
            )?;
            self.stats.comparison_integer_bits = checked_add(
                "generated affine WhenBad associate integer bits",
                self.stats.comparison_integer_bits,
                pair_work.integer_bits,
            )?;
            if polynomial == retained {
                return Ok(ordinal);
            }
        }

        for (ordinal, retained) in self.structural_loci.iter().enumerate() {
            let pair_work = GeneratedAffineComparisonWork::checked_pair(polynomial, retained)?;
            let assembly_before = generated_affine_assembly_comparison_work(self.stats)?;
            let nonterm_pair_work = GeneratedAffineComparisonWork {
                term_pairs: 0,
                ..pair_work
            };
            prior_condition_work
                .checked_add("generated affine WhenBad comparison work", assembly_before)?
                .checked_add(
                    "generated affine WhenBad comparison work",
                    nonterm_pair_work,
                )?
                .check_limits(limits)?;
            let remaining_pairs = remaining_limit(
                "generated affine WhenBad associate term pairs",
                limits.max_associate_term_pairs,
                checked_add(
                    "generated affine WhenBad associate term pairs",
                    prior_condition_work.term_pairs,
                    self.stats.comparison_term_pairs,
                )?,
            )?;
            let payload_units_before = checked_add(
                "generated affine WhenBad payload comparison units",
                prior_payload_units,
                self.stats.payload_comparison_units,
            )?;
            let remaining_payload_units = remaining_limit(
                "generated affine WhenBad payload comparison units",
                limits.max_payload_comparison_units,
                payload_units_before,
            )?;
            let payload_bytes_before = checked_add(
                "generated affine WhenBad payload comparison bytes",
                prior_payload_bytes,
                self.stats.payload_comparison_bytes,
            )?;
            let remaining_payload_bytes = remaining_limit(
                "generated affine WhenBad payload comparison bytes",
                limits.max_payload_comparison_bytes,
                payload_bytes_before,
            )?;
            let mut child = ParametricPolynomialAssociateLimits::default();
            child.exact_algebra = intersect_exact_algebra_limits(
                child.exact_algebra,
                limits.arithmetic.exact_algebra,
            );
            child.max_cross_terms = child.max_cross_terms.min(remaining_pairs);
            child.max_peak_cross_terms = child.max_peak_cross_terms.min(remaining_pairs);
            child.max_context_fingerprint_comparison_bytes = child
                .max_context_fingerprint_comparison_bytes
                .min(remaining_payload_bytes);
            child.max_variable_map_entry_comparisons = child
                .max_variable_map_entry_comparisons
                .min(remaining_payload_units);
            child.max_validation_terms = child
                .max_validation_terms
                .min(limits.max_retained_polynomial_terms);
            child.max_validation_exponent_entries = child
                .max_validation_exponent_entries
                .min(limits.max_retained_polynomial_exponent_entries);
            child.max_validation_integer_bits = child
                .max_validation_integer_bits
                .min(limits.max_retained_polynomial_integer_bits);
            let result =
                context.polynomial_loci_are_associates_with_census(retained, polynomial, child)?;
            let stats = result.stats();
            self.stats.associate_checks = checked_add(
                "generated affine WhenBad associate checks",
                self.stats.associate_checks,
                1,
            )?;
            self.stats.comparison_term_pairs = checked_add(
                "generated affine WhenBad associate term pairs",
                self.stats.comparison_term_pairs,
                stats.cross_terms(),
            )?;
            self.stats.comparison_exponent_entries = checked_add(
                "generated affine WhenBad associate exponent entries",
                self.stats.comparison_exponent_entries,
                pair_work.exponent_entries,
            )?;
            self.stats.comparison_integer_bits = checked_add(
                "generated affine WhenBad associate integer bits",
                self.stats.comparison_integer_bits,
                pair_work.integer_bits,
            )?;
            self.stats.payload_comparison_units = checked_add(
                "generated affine WhenBad payload comparison units",
                self.stats.payload_comparison_units,
                stats.variable_map_entry_comparisons(),
            )?;
            self.stats.payload_comparison_bytes = checked_add(
                "generated affine WhenBad payload comparison bytes",
                self.stats.payload_comparison_bytes,
                stats.context_fingerprint_comparison_bytes(),
            )?;
            check_limit(
                "generated affine WhenBad payload comparison units",
                checked_add(
                    "generated affine WhenBad payload comparison units",
                    prior_payload_units,
                    self.stats.payload_comparison_units,
                )?,
                limits.max_payload_comparison_units,
            )?;
            check_limit(
                "generated affine WhenBad payload comparison bytes",
                checked_add(
                    "generated affine WhenBad payload comparison bytes",
                    prior_payload_bytes,
                    self.stats.payload_comparison_bytes,
                )?,
                limits.max_payload_comparison_bytes,
            )?;
            check_limit(
                "generated affine WhenBad associate term pairs",
                checked_add(
                    "generated affine WhenBad associate term pairs",
                    prior_condition_work.term_pairs,
                    self.stats.comparison_term_pairs,
                )?,
                limits.max_associate_term_pairs,
            )?;
            prior_condition_work
                .checked_add(
                    "generated affine WhenBad comparison work",
                    generated_affine_assembly_comparison_work(self.stats)?,
                )?
                .check_limits(limits)?;
            if result.associated() {
                return Ok(ordinal);
            }
        }

        let requested = checked_add(
            "generated affine WhenBad structural loci",
            self.structural_loci.len(),
            1,
        )?;
        check_limit(
            "generated affine WhenBad structural loci",
            requested,
            limits.max_structural_loci,
        )?;
        let retained_terms = checked_add(
            "generated affine WhenBad retained polynomial terms",
            self.stats.retained_terms,
            source_terms,
        )?;
        let retained_exponents = checked_add(
            "generated affine WhenBad retained polynomial exponent entries",
            self.stats.retained_exponent_entries,
            source_exponents,
        )?;
        let retained_bits = checked_add(
            "generated affine WhenBad retained polynomial integer bits",
            self.stats.retained_integer_bits,
            source_bits,
        )?;
        check_limit(
            "generated affine WhenBad retained polynomial terms",
            retained_terms,
            limits.max_retained_polynomial_terms,
        )?;
        check_limit(
            "generated affine WhenBad retained polynomial exponent entries",
            retained_exponents,
            limits.max_retained_polynomial_exponent_entries,
        )?;
        check_limit(
            "generated affine WhenBad retained polynomial integer bits",
            retained_bits,
            limits.max_retained_polynomial_integer_bits,
        )?;
        if self.structural_loci.len() == self.structural_loci.capacity() {
            return Err(GeneratedResidualAffineWhenBadError::ConditionInvariant {
                stage: "precharged structural-locus capacity",
            });
        }
        let copied = polynomial
            .try_copy_authenticated_sparse_payload()
            .map_err(
                |resource| GeneratedResidualAffineWhenBadError::AllocationFailure {
                    resource,
                    requested: source_terms.max(source_exponents),
                },
            )?;
        self.stats.retained_bytes = checked_add(
            "generated affine WhenBad structural assembly bytes",
            self.stats.retained_bytes,
            copied.owned_retained_byte_bound().ok_or(
                GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
                    resource: "generated affine WhenBad structural polynomial bytes",
                },
            )?,
        )?;
        if self.stats.retained_bytes > self.stats.retained_byte_envelope {
            return Err(
                GeneratedResidualAffineWhenBadError::RetainedByteEnvelopeExceeded {
                    observed: self.stats.retained_bytes,
                    admitted: self.stats.retained_byte_envelope,
                },
            );
        }
        self.stats.retained_terms = retained_terms;
        self.stats.retained_exponent_entries = retained_exponents;
        self.stats.retained_integer_bits = retained_bits;
        self.structural_loci.push(copied);
        Ok(requested - 1)
    }

    fn into_problem(self) -> AffineWhenBadRelativeProblem {
        AffineWhenBadRelativeProblem::from_preallocated(
            self.structural_loci,
            self.inherited_truths,
            self.clauses,
        )
    }
}

fn generated_affine_relative_validation_census(
    structural_loci: &[ParametricPolynomial],
) -> Result<GeneratedAffineRelativeValidationCensus, GeneratedResidualAffineWhenBadError> {
    let mut census = GeneratedAffineRelativeValidationCensus::default();
    for (ordinal, polynomial) in structural_loci.iter().enumerate() {
        for retained in &structural_loci[..ordinal] {
            let pair = GeneratedAffineComparisonWork::checked_pair(polynomial, retained)?;
            census.aggregate = census
                .aggregate
                .checked_add("generated affine WhenBad comparison work", pair)?
                .checked_add("generated affine WhenBad comparison work", pair)?;
            census.equality_checks = checked_add(
                "generated affine WhenBad relative equality checks",
                census.equality_checks,
                1,
            )?;
            census.associate_checks = checked_add(
                "generated affine WhenBad relative associate checks",
                census.associate_checks,
                1,
            )?;
            census.associate_term_pairs = checked_add(
                "generated affine WhenBad relative associate term pairs",
                census.associate_term_pairs,
                pair.term_pairs,
            )?;
        }
    }
    Ok(census)
}

fn projected_generated_affine_relative_limits(
    ready: &GeneratedResidualAffineWhenBadDescentReady,
    conditions: &GeneratedResidualAffineConditionAccumulatorCertificate,
    pullbacks: &GeneratedResidualAffineWhenBadPullbackGateCertificate,
    private_payload_comparison_bytes: usize,
    assembly: GeneratedAffineLocusAssemblyStats,
    validation: GeneratedAffineRelativeValidationCensus,
) -> Result<AffineWhenBadRelativeCaseLimits, GeneratedResidualAffineWhenBadError> {
    let outer = ready.input().limits();
    let condition = conditions.stats();
    let pullback = pullbacks.stats();
    let mut limits = outer.relative_partition;
    limits.exact_algebra =
        intersect_exact_algebra_limits(limits.exact_algebra, outer.arithmetic.exact_algebra);
    limits.max_context_fingerprint_bytes = limits
        .max_context_fingerprint_bytes
        .min(outer.max_context_fingerprint_bytes);
    limits.max_structural_loci = limits.max_structural_loci.min(outer.max_structural_loci);
    generated_affine_condition_comparison_work(condition)?
        .checked_add(
            "generated affine WhenBad comparison work",
            generated_affine_assembly_comparison_work(assembly)?,
        )?
        .checked_add(
            "generated affine WhenBad comparison work",
            validation.aggregate,
        )?
        .check_limits(outer)?;
    limits.max_structural_locus_equality_comparisons = limits
        .max_structural_locus_equality_comparisons
        .min(validation.equality_checks);
    limits.max_structural_locus_associate_comparisons = limits
        .max_structural_locus_associate_comparisons
        .min(validation.associate_checks);
    limits.max_structural_locus_associate_term_pairs = limits
        .max_structural_locus_associate_term_pairs
        .min(validation.associate_term_pairs);
    limits.max_bad_clauses = limits.max_bad_clauses.min(outer.max_bad_clauses);
    limits.max_bad_atoms = limits.max_bad_atoms.min(outer.max_bad_atoms);
    let prior_terms = checked_add(
        "generated affine WhenBad retained polynomial terms",
        condition.retained_polynomial_terms(),
        pullback.retained_polynomial_terms(),
    )?;
    let prior_exponents = checked_add(
        "generated affine WhenBad retained polynomial exponent entries",
        condition.retained_polynomial_exponent_entries(),
        pullback.retained_polynomial_exponent_entries(),
    )?;
    let prior_bits = checked_add(
        "generated affine WhenBad retained polynomial integer bits",
        condition.retained_polynomial_integer_bits(),
        pullback.retained_polynomial_integer_bits(),
    )?;
    let prior_display = checked_add(
        "generated affine WhenBad retained polynomial display bytes",
        condition.retained_polynomial_display_bytes(),
        pullback.retained_polynomial_display_bytes(),
    )?;
    limits.max_retained_polynomial_terms =
        limits.max_retained_polynomial_terms.min(remaining_limit(
            "generated affine WhenBad retained polynomial terms",
            outer.max_retained_polynomial_terms,
            prior_terms,
        )?);
    limits.max_retained_polynomial_exponent_entries = limits
        .max_retained_polynomial_exponent_entries
        .min(remaining_limit(
            "generated affine WhenBad retained polynomial exponent entries",
            outer.max_retained_polynomial_exponent_entries,
            prior_exponents,
        )?);
    limits.max_retained_polynomial_integer_bits =
        limits
            .max_retained_polynomial_integer_bits
            .min(remaining_limit(
                "generated affine WhenBad retained polynomial integer bits",
                outer.max_retained_polynomial_integer_bits,
                prior_bits,
            )?);
    limits.max_retained_polynomial_display_bytes = limits
        .max_retained_polynomial_display_bytes
        .min(remaining_limit(
            "generated affine WhenBad retained polynomial display bytes",
            outer.max_retained_polynomial_display_bytes,
            prior_display,
        )?);
    let retained_before = checked_add(
        "generated affine WhenBad retained bytes",
        checked_add(
            "generated affine WhenBad retained bytes",
            generated_affine_incremental_retained_bytes(
                ready.stats().retained_bytes(),
                size_of::<GeneratedResidualAffineWhenBadDescentReady>(),
            )?,
            generated_affine_incremental_retained_bytes(
                condition.retained_bytes(),
                size_of::<GeneratedResidualAffineConditionAccumulatorCertificate>(),
            )?,
        )?,
        generated_affine_incremental_retained_bytes(
            pullback.retained_bytes(),
            size_of::<GeneratedResidualAffineWhenBadPullbackGateCertificate>(),
        )?,
    )?;
    limits.max_retained_bytes =
        limits
            .max_retained_bytes
            .min(generated_affine_full_child_retained_limit(
                generated_affine_reserved_outer_heap_budget(outer)?,
                retained_before,
                size_of::<AffineWhenBadRelativePartitionCertificate>(),
            )?);
    let prior_payload_units = checked_add(
        "generated affine WhenBad payload comparison units",
        checked_add(
            "generated affine WhenBad payload comparison units",
            ready.stats().payload_comparison_units_observed(),
            generated_affine_condition_payload_comparison_units(condition)?,
        )?,
        checked_add(
            "generated affine WhenBad payload comparison units",
            pullback.payload_comparison_units(),
            assembly.payload_comparison_units,
        )?,
    )?;
    limits.max_payload_comparison_units = limits.max_payload_comparison_units.min(remaining_limit(
        "generated affine WhenBad payload comparison units",
        outer.max_payload_comparison_units,
        prior_payload_units,
    )?);
    let prior_payload_bytes = checked_add(
        "generated affine WhenBad payload comparison bytes",
        private_payload_comparison_bytes,
        checked_add(
            "generated affine WhenBad payload comparison bytes",
            condition.context_fingerprint_comparison_bytes(),
            checked_add(
                "generated affine WhenBad payload comparison bytes",
                pullback.payload_comparison_bytes(),
                assembly.payload_comparison_bytes,
            )?,
        )?,
    )?;
    limits.max_payload_comparison_bytes = limits.max_payload_comparison_bytes.min(remaining_limit(
        "generated affine WhenBad payload comparison bytes",
        outer.max_payload_comparison_bytes,
        prior_payload_bytes,
    )?);
    let prior_payload_integer_bits = checked_add(
        "generated affine WhenBad payload comparison integer bits",
        condition.equality_integer_bits(),
        pullback.payload_comparison_integer_bits(),
    )?;
    limits.max_payload_comparison_integer_bits =
        limits
            .max_payload_comparison_integer_bits
            .min(remaining_limit(
                "generated affine WhenBad payload comparison integer bits",
                outer.max_payload_comparison_integer_bits,
                prior_payload_integer_bits,
            )?);
    Ok(limits)
}

struct GeneratedAffineRelativePartitionCompilation {
    certificate: AffineWhenBadRelativePartitionCertificate,
    assembly_stats: GeneratedAffineLocusAssemblyStats,
}

fn compile_generated_affine_relative_partition(
    context: &ParametricCoefficientContext,
    ready: &GeneratedResidualAffineWhenBadDescentReady,
    conditions: &GeneratedResidualAffineConditionAccumulatorCertificate,
    pullbacks: &GeneratedResidualAffineWhenBadPullbackGateCertificate,
    private_payload_comparison_bytes: usize,
) -> Result<GeneratedAffineRelativePartitionCompilation, GeneratedResidualAffineWhenBadError> {
    let outer = ready.input().limits();
    let mut admission = GeneratedAffineLocusAssemblyAdmission::default();
    for row in conditions.rows() {
        if !row.is_index_dependent() {
            continue;
        }
        admission.observe_structural_source(row.polynomial())?;
        match row.scope() {
            GeneratedResidualAffineConditionScope::InheritedTargetPremise => {
                admission.observe_inherited_truth()?;
            }
            GeneratedResidualAffineConditionScope::CandidateRequired => {
                admission.observe_clause(1)?;
            }
        }
    }
    for event in pullbacks.events() {
        match (event.pullback_class(), event.numerator_gate().class()) {
            (AffineBoundaryPullbackClass::EmptyBoundary, _) => {}
            (
                AffineBoundaryPullbackClass::WholeTarget,
                AffineWhenBadNumeratorGateClass::CoefficientFieldNonzero,
            ) => {
                return Err(GeneratedResidualAffineWhenBadError::ConditionInvariant {
                    stage: "universal pullback leak routing",
                });
            }
            (
                AffineBoundaryPullbackClass::WholeTarget,
                AffineWhenBadNumeratorGateClass::FreeIndexNonzero,
            ) => {
                admission.observe_structural_source(event.numerator_gate().polynomial())?;
                admission.observe_clause(1)?;
            }
            (
                AffineBoundaryPullbackClass::FreeIndexDependent,
                AffineWhenBadNumeratorGateClass::CoefficientFieldNonzero,
            ) => {
                admission.observe_structural_source(event.pullback())?;
                admission.observe_clause(1)?;
            }
            (
                AffineBoundaryPullbackClass::FreeIndexDependent,
                AffineWhenBadNumeratorGateClass::FreeIndexNonzero,
            ) => {
                admission.observe_structural_source(event.pullback())?;
                admission.observe_structural_source(event.numerator_gate().polynomial())?;
                admission.observe_clause(2)?;
            }
        }
    }
    let admission = admission.finish(ready, conditions, pullbacks)?;
    let condition_stats = conditions.stats();
    let prior_condition_work = generated_affine_condition_comparison_work(condition_stats)?;
    prior_condition_work.check_limits(outer)?;
    let prior_payload_units = checked_add(
        "generated affine WhenBad payload comparison units",
        checked_add(
            "generated affine WhenBad payload comparison units",
            ready.stats().payload_comparison_units_observed(),
            generated_affine_condition_payload_comparison_units(condition_stats)?,
        )?,
        pullbacks.stats().payload_comparison_units(),
    )?;
    check_limit(
        "generated affine WhenBad payload comparison units",
        prior_payload_units,
        outer.max_payload_comparison_units,
    )?;
    let prior_payload_bytes = checked_add(
        "generated affine WhenBad payload comparison bytes",
        private_payload_comparison_bytes,
        checked_add(
            "generated affine WhenBad payload comparison bytes",
            condition_stats.context_fingerprint_comparison_bytes(),
            pullbacks.stats().payload_comparison_bytes(),
        )?,
    )?;
    check_limit(
        "generated affine WhenBad payload comparison bytes",
        prior_payload_bytes,
        outer.max_payload_comparison_bytes,
    )?;
    let mut assembly = GeneratedAffineRelativeProblemAssembly::try_with_precharged_capacities(
        admission.structural_sources,
        admission.inherited_truths,
        admission.clauses,
        admission.retained_byte_envelope,
    )?;
    for (condition_ordinal, row) in conditions.rows().iter().enumerate() {
        if !row.is_index_dependent() {
            continue;
        }
        let locus = assembly.intern(
            context,
            row.polynomial(),
            outer,
            prior_condition_work,
            prior_payload_units,
            prior_payload_bytes,
        )?;
        match row.scope() {
            GeneratedResidualAffineConditionScope::InheritedTargetPremise => {
                assembly.push_inherited(
                    AffineWhenBadInheritedTruth::new(
                        locus,
                        SymbolicPolynomialPredicateKind::NonZero,
                    ),
                    outer,
                )?;
            }
            GeneratedResidualAffineConditionScope::CandidateRequired => {
                assembly.push_clause(
                    AffineWhenBadFormulaClause::candidate_required_guard_zero(
                        condition_ordinal,
                        locus,
                    ),
                    1,
                    outer,
                )?;
            }
        }
    }

    for event in pullbacks.events() {
        let pullback_ordinal = event.ordinal();
        match (event.pullback_class(), event.numerator_gate().class()) {
            (AffineBoundaryPullbackClass::EmptyBoundary, _) => {}
            (
                AffineBoundaryPullbackClass::WholeTarget,
                AffineWhenBadNumeratorGateClass::CoefficientFieldNonzero,
            ) => {
                return Err(GeneratedResidualAffineWhenBadError::ConditionInvariant {
                    stage: "universal pullback leak routing",
                });
            }
            (
                AffineBoundaryPullbackClass::WholeTarget,
                AffineWhenBadNumeratorGateClass::FreeIndexNonzero,
            ) => {
                let numerator = assembly.intern(
                    context,
                    event.numerator_gate().polynomial(),
                    outer,
                    prior_condition_work,
                    prior_payload_units,
                    prior_payload_bytes,
                )?;
                assembly.push_clause(
                    AffineWhenBadFormulaClause::whole_target_free_index_leak(
                        pullback_ordinal,
                        numerator,
                    ),
                    1,
                    outer,
                )?;
            }
            (
                AffineBoundaryPullbackClass::FreeIndexDependent,
                AffineWhenBadNumeratorGateClass::CoefficientFieldNonzero,
            ) => {
                let boundary = assembly.intern(
                    context,
                    event.pullback(),
                    outer,
                    prior_condition_work,
                    prior_payload_units,
                    prior_payload_bytes,
                )?;
                assembly.push_clause(
                    AffineWhenBadFormulaClause::coefficient_field_leak_boundary_zero(
                        pullback_ordinal,
                        boundary,
                    ),
                    1,
                    outer,
                )?;
            }
            (
                AffineBoundaryPullbackClass::FreeIndexDependent,
                AffineWhenBadNumeratorGateClass::FreeIndexNonzero,
            ) => {
                let boundary = assembly.intern(
                    context,
                    event.pullback(),
                    outer,
                    prior_condition_work,
                    prior_payload_units,
                    prior_payload_bytes,
                )?;
                let numerator = assembly.intern(
                    context,
                    event.numerator_gate().polynomial(),
                    outer,
                    prior_condition_work,
                    prior_payload_units,
                    prior_payload_bytes,
                )?;
                assembly.push_clause(
                    AffineWhenBadFormulaClause::free_index_leak(
                        pullback_ordinal,
                        boundary,
                        numerator,
                    ),
                    2,
                    outer,
                )?;
            }
        }
    }
    let assembly_stats = assembly.stats;
    let validation = generated_affine_relative_validation_census(&assembly.structural_loci)?;
    let relative_limits = projected_generated_affine_relative_limits(
        ready,
        conditions,
        pullbacks,
        private_payload_comparison_bytes,
        assembly_stats,
        validation,
    )?;
    let certificate = AffineWhenBadRelativePartitionCompiler::compile(
        context,
        assembly.into_problem(),
        relative_limits,
    )
    .map_err(GeneratedResidualAffineWhenBadError::from)?;
    let stats = certificate.stats();
    if stats.structural_locus_equality_comparisons() != validation.equality_checks
        || stats.structural_locus_associate_comparisons() != validation.associate_checks
        || stats.structural_locus_associate_term_pairs() != validation.associate_term_pairs
    {
        return Err(GeneratedResidualAffineWhenBadError::ConditionInvariant {
            stage: "relative structural comparison census",
        });
    }
    Ok(GeneratedAffineRelativePartitionCompilation {
        certificate,
        assembly_stats,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeneratedAffineCompilationShape {
    Unsupported,
    Condition,
    Pullback,
    Partition,
}

fn generated_affine_outer_retained_bytes(
    shape: GeneratedAffineCompilationShape,
    descent_retained_bytes: usize,
    condition_retained_bytes: Option<usize>,
    pullback_retained_bytes: Option<usize>,
    partition_retained_bytes: Option<usize>,
) -> Result<usize, GeneratedResidualAffineWhenBadError> {
    let valid_shape = match shape {
        GeneratedAffineCompilationShape::Unsupported => {
            condition_retained_bytes.is_none()
                && pullback_retained_bytes.is_none()
                && partition_retained_bytes.is_none()
        }
        GeneratedAffineCompilationShape::Condition => {
            condition_retained_bytes.is_some()
                && pullback_retained_bytes.is_none()
                && partition_retained_bytes.is_none()
        }
        GeneratedAffineCompilationShape::Pullback => {
            condition_retained_bytes.is_some()
                && pullback_retained_bytes.is_some()
                && partition_retained_bytes.is_none()
        }
        GeneratedAffineCompilationShape::Partition => {
            condition_retained_bytes.is_some()
                && pullback_retained_bytes.is_some()
                && partition_retained_bytes.is_some()
        }
    };
    if !valid_shape {
        return Err(GeneratedResidualAffineWhenBadError::ConditionInvariant {
            stage: "outer compilation shape census",
        });
    }
    let descent_inline = match shape {
        GeneratedAffineCompilationShape::Unsupported => {
            size_of::<GeneratedResidualAffineWhenBadDescentUnsupported>()
        }
        GeneratedAffineCompilationShape::Condition
        | GeneratedAffineCompilationShape::Pullback
        | GeneratedAffineCompilationShape::Partition => {
            size_of::<GeneratedResidualAffineWhenBadDescentReady>()
        }
    };
    let mut retained = checked_add(
        "generated affine WhenBad retained bytes",
        size_of::<GeneratedResidualAffineWhenBadCompilation>(),
        generated_affine_incremental_retained_bytes(descent_retained_bytes, descent_inline)?,
    )?;
    for (full, inline) in [
        (
            condition_retained_bytes,
            size_of::<GeneratedResidualAffineConditionAccumulatorCertificate>(),
        ),
        (
            pullback_retained_bytes,
            size_of::<GeneratedResidualAffineWhenBadPullbackGateCertificate>(),
        ),
        (
            partition_retained_bytes,
            size_of::<AffineWhenBadRelativePartitionCertificate>(),
        ),
    ] {
        if let Some(full) = full {
            retained = checked_add(
                "generated affine WhenBad retained bytes",
                retained,
                generated_affine_incremental_retained_bytes(full, inline)?,
            )?;
        }
    }
    Ok(retained)
}

fn generated_affine_compilation_stats(
    input: &AuthenticatedGeneratedResidualAffineWhenBadInput,
    descent: GeneratedResidualAffineWhenBadDescentStats,
    conditions: Option<GeneratedResidualAffineConditionAccumulatorStats>,
    pullbacks: Option<GeneratedResidualAffineWhenBadPullbackGateStats>,
    partition: Option<&AffineWhenBadRelativePartitionCertificate>,
    assembly: Option<GeneratedAffineLocusAssemblyStats>,
    shape: GeneratedAffineCompilationShape,
    limits: GeneratedResidualAffineWhenBadLimits,
) -> Result<GeneratedResidualAffineWhenBadCompilationStats, GeneratedResidualAffineWhenBadError> {
    let authority = input.stats();
    if matches!(shape, GeneratedAffineCompilationShape::Partition) != assembly.is_some() {
        return Err(GeneratedResidualAffineWhenBadError::ConditionInvariant {
            stage: "outer compilation assembly census",
        });
    }
    let condition_retained_bytes = conditions.map(|stats| stats.retained_bytes());
    let pullback_retained_bytes = pullbacks.map(|stats| stats.retained_bytes());
    let partition_retained_bytes = partition
        .map(AffineWhenBadRelativePartitionCertificate::stats)
        .map(|stats| stats.retained_bytes());
    let condition = conditions.unwrap_or_default();
    let pullback = pullbacks.unwrap_or_default();
    let partition_stats = partition.map(AffineWhenBadRelativePartitionCertificate::stats);
    let applicable_leaves = partition
        .into_iter()
        .flat_map(|certificate| certificate.classifications())
        .filter(|leaf| {
            matches!(
                leaf.disposition(),
                AffineWhenBadRelativeLeafDisposition::Applicable
            )
        })
        .count();
    let leaf_count = partition_stats.map_or(0, |stats| stats.leaf_classifications());
    let exceptional_leaves = leaf_count.checked_sub(applicable_leaves).ok_or(
        GeneratedResidualAffineWhenBadError::ConditionInvariant {
            stage: "relative leaf census",
        },
    )?;
    let retained_bytes = generated_affine_outer_retained_bytes(
        shape,
        descent.retained_bytes(),
        condition_retained_bytes,
        pullback_retained_bytes,
        partition_retained_bytes,
    )?;
    check_limit(
        "generated affine WhenBad retained bytes",
        retained_bytes,
        limits.max_retained_bytes,
    )?;
    let group_source_terms = checked_add(
        "generated affine WhenBad aggregate source terms",
        checked_add(
            "generated affine WhenBad aggregate source terms",
            authority.private_relation_source_terms(),
            condition.input_polynomial_terms(),
        )?,
        pullback.total_source_terms(),
    )?;
    let group_source_exponent_entries = checked_add(
        "generated affine WhenBad aggregate source exponent entries",
        checked_add(
            "generated affine WhenBad aggregate source exponent entries",
            authority.private_relation_source_exponent_entries(),
            condition.input_polynomial_exponent_entries(),
        )?,
        pullback.total_source_exponent_entries(),
    )?;
    let group_source_integer_bits = checked_add(
        "generated affine WhenBad aggregate source integer bits",
        checked_add(
            "generated affine WhenBad aggregate source integer bits",
            authority.private_relation_source_integer_bits(),
            condition.input_polynomial_integer_bits(),
        )?,
        pullback.total_source_integer_bits(),
    )?;
    let payload = generated_affine_outer_payload_comparison_census(
        input, descent, conditions, pullbacks, partition, input, descent, conditions, pullbacks,
        partition,
    )?;
    let assembly = assembly.unwrap_or_default();
    let group_payload_comparison_units = checked_add(
        "generated affine WhenBad payload comparison units",
        payload.units,
        assembly.payload_comparison_units,
    )?;
    let group_payload_comparison_bytes = checked_add(
        "generated affine WhenBad payload comparison bytes",
        payload.bytes,
        assembly.payload_comparison_bytes,
    )?;
    for (resource, requested, limit) in [
        (
            "generated affine WhenBad aggregate source terms",
            group_source_terms,
            limits.max_total_source_terms,
        ),
        (
            "generated affine WhenBad aggregate source exponent entries",
            group_source_exponent_entries,
            limits.max_total_source_exponent_entries,
        ),
        (
            "generated affine WhenBad aggregate source integer bits",
            group_source_integer_bits,
            limits.max_total_source_integer_bits,
        ),
        (
            "generated affine WhenBad aggregate output terms",
            pullback.total_output_terms(),
            limits.max_total_output_terms,
        ),
        (
            "generated affine WhenBad aggregate output exponent entries",
            pullback.total_output_exponent_entries(),
            limits.max_total_output_exponent_entries,
        ),
        (
            "generated affine WhenBad aggregate native integer-bit work",
            pullback.total_native_integer_bit_work(),
            limits.max_total_native_integer_bit_work,
        ),
        (
            "generated affine WhenBad aggregate integer-bit work",
            pullback.total_integer_bit_work(),
            limits.max_total_integer_bit_work,
        ),
        (
            "generated affine WhenBad payload comparison units",
            group_payload_comparison_units,
            limits.max_payload_comparison_units,
        ),
        (
            "generated affine WhenBad payload comparison bytes",
            group_payload_comparison_bytes,
            limits.max_payload_comparison_bytes,
        ),
        (
            "generated affine WhenBad payload comparison integer bits",
            payload.integer_bits,
            limits.max_payload_comparison_integer_bits,
        ),
        (
            "generated affine WhenBad private payload comparison bytes",
            payload.private_manifest_bytes,
            limits.max_payload_comparison_private_manifest_bytes,
        ),
        (
            "generated affine WhenBad structural loci",
            partition_stats.map_or(0, |stats| stats.structural_loci()),
            limits.max_structural_loci,
        ),
        (
            "generated affine WhenBad bad clauses",
            partition_stats.map_or(0, |stats| stats.bad_clauses()),
            limits.max_bad_clauses,
        ),
        (
            "generated affine WhenBad relative leaf classifications",
            leaf_count,
            limits.relative_partition.max_leaf_classifications,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    Ok(GeneratedResidualAffineWhenBadCompilationStats {
        authority,
        descent_witnesses_attempted: descent.descent_witnesses_attempted(),
        descent_witnesses_proved: descent.descent_witnesses_proved(),
        condition_inputs: condition.condition_inputs(),
        canonical_conditions: condition.unique_rows(),
        inherited_conditions: condition.unique_inherited_rows(),
        candidate_conditions: condition.unique_candidate_rows(),
        boundary_values: pullback.boundary_values(),
        pullback_compositions: pullback.pullback_compositions(),
        leak_witnesses: pullback.leak_witnesses(),
        structural_loci: partition_stats.map_or(0, |stats| stats.structural_loci()),
        bad_clauses: partition_stats.map_or(0, |stats| stats.bad_clauses()),
        applicable_leaves,
        exceptional_leaves,
        retained_bytes,
        group_source_terms,
        group_source_exponent_entries,
        group_source_integer_bits,
        group_output_terms: pullback.total_output_terms(),
        group_output_exponent_entries: pullback.total_output_exponent_entries(),
        group_native_integer_bit_work: pullback.total_native_integer_bit_work(),
        group_total_integer_bit_work: pullback.total_integer_bit_work(),
        group_payload_comparison_units,
        group_payload_comparison_bytes,
        group_payload_comparison_integer_bits: payload.integer_bits,
        group_payload_comparison_private_manifest_bytes: payload.private_manifest_bytes,
        group_assembly_payload_comparison_units: assembly.payload_comparison_units,
        group_assembly_payload_comparison_bytes: assembly.payload_comparison_bytes,
    })
}

enum GeneratedResidualAffineWhenBadIdenticallyBadPayload {
    Condition {
        ready: GeneratedResidualAffineWhenBadDescentReady,
        conditions: GeneratedResidualAffineConditionAccumulatorCertificate,
    },
    Pullback {
        ready: GeneratedResidualAffineWhenBadDescentReady,
        conditions: GeneratedResidualAffineConditionAccumulatorCertificate,
        pullbacks: GeneratedResidualAffineWhenBadPullbackGateCertificate,
    },
    Partition {
        ready: GeneratedResidualAffineWhenBadDescentReady,
        conditions: GeneratedResidualAffineConditionAccumulatorCertificate,
        pullbacks: GeneratedResidualAffineWhenBadPullbackGateCertificate,
        partition: AffineWhenBadRelativePartitionCertificate,
    },
}

impl GeneratedResidualAffineWhenBadIdenticallyBadPayload {
    fn ready(&self) -> &GeneratedResidualAffineWhenBadDescentReady {
        match self {
            Self::Condition { ready, .. }
            | Self::Pullback { ready, .. }
            | Self::Partition { ready, .. } => ready,
        }
    }

    fn conditions(&self) -> &GeneratedResidualAffineConditionAccumulatorCertificate {
        match self {
            Self::Condition { conditions, .. }
            | Self::Pullback { conditions, .. }
            | Self::Partition { conditions, .. } => conditions,
        }
    }

    fn pullbacks(&self) -> Option<&GeneratedResidualAffineWhenBadPullbackGateCertificate> {
        match self {
            Self::Condition { .. } => None,
            Self::Pullback { pullbacks, .. } | Self::Partition { pullbacks, .. } => Some(pullbacks),
        }
    }

    fn partition(&self) -> Option<&AffineWhenBadRelativePartitionCertificate> {
        match self {
            Self::Partition { partition, .. } => Some(partition),
            Self::Condition { .. } | Self::Pullback { .. } => None,
        }
    }

    pub(crate) fn payload_eq_checked(
        &self,
        other: &Self,
    ) -> Result<bool, GeneratedResidualAffineWhenBadError> {
        let equal = match (self, other) {
            (
                Self::Condition {
                    ready: left_ready,
                    conditions: left_conditions,
                },
                Self::Condition {
                    ready: right_ready,
                    conditions: right_conditions,
                },
            ) => {
                preflight_generated_affine_outer_payload_comparison(
                    left_ready.input(),
                    left_ready.stats(),
                    Some(left_conditions.stats()),
                    None,
                    None,
                    right_ready.input(),
                    right_ready.stats(),
                    Some(right_conditions.stats()),
                    None,
                    None,
                )?;
                left_ready.payload_eq_same_authority(right_ready)
                    && left_conditions == right_conditions
            }
            (
                Self::Pullback {
                    ready: left_ready,
                    conditions: left_conditions,
                    pullbacks: left_pullbacks,
                },
                Self::Pullback {
                    ready: right_ready,
                    conditions: right_conditions,
                    pullbacks: right_pullbacks,
                },
            ) => {
                preflight_generated_affine_outer_payload_comparison(
                    left_ready.input(),
                    left_ready.stats(),
                    Some(left_conditions.stats()),
                    Some(left_pullbacks.stats()),
                    None,
                    right_ready.input(),
                    right_ready.stats(),
                    Some(right_conditions.stats()),
                    Some(right_pullbacks.stats()),
                    None,
                )?;
                left_ready.payload_eq_same_authority(right_ready)
                    && left_conditions == right_conditions
                    && left_pullbacks
                        .payload_eq_checked(right_pullbacks)
                        .map_err(map_generated_affine_pullback_gate_error)?
            }
            (
                Self::Partition {
                    ready: left_ready,
                    conditions: left_conditions,
                    pullbacks: left_pullbacks,
                    partition: left_partition,
                },
                Self::Partition {
                    ready: right_ready,
                    conditions: right_conditions,
                    pullbacks: right_pullbacks,
                    partition: right_partition,
                },
            ) => {
                preflight_generated_affine_outer_payload_comparison(
                    left_ready.input(),
                    left_ready.stats(),
                    Some(left_conditions.stats()),
                    Some(left_pullbacks.stats()),
                    Some(left_partition),
                    right_ready.input(),
                    right_ready.stats(),
                    Some(right_conditions.stats()),
                    Some(right_pullbacks.stats()),
                    Some(right_partition),
                )?;
                left_ready.payload_eq_same_authority(right_ready)
                    && left_conditions == right_conditions
                    && left_pullbacks
                        .payload_eq_checked(right_pullbacks)
                        .map_err(map_generated_affine_pullback_gate_error)?
                    && left_partition == right_partition
            }
            _ => false,
        };
        Ok(equal)
    }
}

/// Prospective resource envelope for materializing the private conditions of
/// one owner-authorized concrete affine rule.
///
/// These limits are intentionally separate from the compilation limits: a
/// retained certificate may be queried long after compilation, and no
/// compilation allowance may silently reset for every concrete point.  The
/// condition census is completed before the first condition vector, B-tree
/// node, polynomial payload, or provenance payload is allocated or cloned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineSealedApplicationLimits {
    pub(crate) max_condition_rows: usize,
    pub(crate) max_condition_source_lookups: usize,
    pub(crate) max_condition_copy_terms: usize,
    pub(crate) max_condition_copy_exponent_entries: usize,
    pub(crate) max_condition_copy_integer_bits: usize,
    pub(crate) max_condition_origin_inputs: usize,
    pub(crate) max_condition_origin_retained_bytes: usize,
    pub(crate) max_temporary_condition_retained_byte_bound: usize,
    pub(crate) max_temporary_plus_relation_peak_byte_bound: usize,
    pub(crate) relation: ParametricConcreteSpecializationLimits,
}

impl Default for GeneratedResidualAffineSealedApplicationLimits {
    fn default() -> Self {
        Self {
            max_condition_rows: 64_000_000,
            max_condition_source_lookups: 512_000_000,
            max_condition_copy_terms: 1_000_000_000,
            max_condition_copy_exponent_entries: portable_usize(64_000_000_000),
            max_condition_copy_integer_bits: portable_usize(16_000_000_000_000_000),
            max_condition_origin_inputs: 1_000_000_000,
            max_condition_origin_retained_bytes: portable_usize(16_u64 * 1024 * 1024 * 1024),
            max_temporary_condition_retained_byte_bound: portable_usize(
                16_u64 * 1024 * 1024 * 1024,
            ),
            max_temporary_plus_relation_peak_byte_bound: portable_usize(
                512_u64 * 1024 * 1024 * 1024,
            ),
            relation: ParametricConcreteSpecializationLimits::default(),
        }
    }
}

/// Immutable, redacted census for one sealed affine application.
///
/// Every field is a scalar resource count.  No private condition, source
/// locator, polynomial, relation, point coordinate, or affine map is retained
/// here or exposed through `Debug`.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineSealedApplicationStats {
    condition_rows: usize,
    condition_source_lookups: usize,
    condition_copy_terms: usize,
    condition_copy_exponent_entries: usize,
    condition_copy_integer_bits: usize,
    condition_origin_inputs: usize,
    condition_origin_retained_bytes: usize,
    temporary_condition_retained_byte_bound: usize,
    temporary_condition_retained_bytes: usize,
    temporary_plus_relation_peak_byte_bound: usize,
    relation: ParametricConcreteSpecializationPreflight,
}

impl fmt::Debug for GeneratedResidualAffineSealedApplicationStats {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineSealedApplicationStats")
            .field("condition_rows", &self.condition_rows)
            .field("condition_source_lookups", &self.condition_source_lookups)
            .field("condition_copy_terms", &self.condition_copy_terms)
            .field(
                "condition_copy_exponent_entries",
                &self.condition_copy_exponent_entries,
            )
            .field(
                "condition_copy_integer_bits",
                &self.condition_copy_integer_bits,
            )
            .field("condition_origin_inputs", &self.condition_origin_inputs)
            .field(
                "condition_origin_retained_bytes",
                &self.condition_origin_retained_bytes,
            )
            .field(
                "temporary_condition_retained_byte_bound",
                &self.temporary_condition_retained_byte_bound,
            )
            .field(
                "temporary_condition_retained_bytes",
                &self.temporary_condition_retained_bytes,
            )
            .field(
                "temporary_plus_relation_peak_byte_bound",
                &self.temporary_plus_relation_peak_byte_bound,
            )
            // The nested type is itself scalar-only, but its internal field
            // vocabulary describes proof-private payload classes. Keep that
            // vocabulary out of the public-adjacent application formatter.
            .field("relation_specialization", &"<redacted resource census>")
            .finish()
    }
}

impl GeneratedResidualAffineSealedApplicationStats {
    pub(crate) const fn condition_rows(self) -> usize {
        self.condition_rows
    }

    pub(crate) const fn condition_source_lookups(self) -> usize {
        self.condition_source_lookups
    }

    pub(crate) const fn condition_copy_terms(self) -> usize {
        self.condition_copy_terms
    }

    pub(crate) const fn condition_copy_exponent_entries(self) -> usize {
        self.condition_copy_exponent_entries
    }

    pub(crate) const fn condition_copy_integer_bits(self) -> usize {
        self.condition_copy_integer_bits
    }

    pub(crate) const fn condition_origin_inputs(self) -> usize {
        self.condition_origin_inputs
    }

    pub(crate) const fn condition_origin_retained_bytes(self) -> usize {
        self.condition_origin_retained_bytes
    }

    pub(crate) const fn temporary_condition_retained_byte_bound(self) -> usize {
        self.temporary_condition_retained_byte_bound
    }

    pub(crate) const fn temporary_condition_retained_bytes(self) -> usize {
        self.temporary_condition_retained_bytes
    }

    pub(crate) const fn temporary_plus_relation_peak_byte_bound(self) -> usize {
        self.temporary_plus_relation_peak_byte_bound
    }

    pub(crate) const fn relation(self) -> ParametricConcreteSpecializationPreflight {
        self.relation
    }
}

/// Consume-once authorization for the allocation-bearing half of sealed
/// condition materialization.
///
/// The type and all of its fields are private to this module.  It borrows the
/// exact certificate, K(n) context, point assignment, and owner-created leaf
/// authorization that were checked by the allocation-free prepare pass, so a
/// caller cannot detach a resource census from the authority it censused.
struct GeneratedResidualAffineSealedApplicationPlan<
    'certificate,
    'context,
    'indices,
    'authorization,
> {
    certificate: &'certificate GeneratedResidualAffineWhenBadCertificate,
    context: &'context ParametricCoefficientContext,
    indices: &'indices [i64],
    authorization: &'authorization GeneratedSectorAffineSealedLeafAuthorization<'certificate>,
    limits: GeneratedResidualAffineSealedApplicationLimits,
    stats: GeneratedResidualAffineSealedApplicationStats,
}

/// Exceptional source class retained by one authenticated relative leaf.
///
/// Unlike [`AffineWhenBadRelativeLeafDisposition`], this type cannot describe
/// an applicable leaf.  The source ordinal remains part of the kind so a
/// caller must authenticate the exact domain condition or leak pullback it
/// expects before receiving any private relative predicates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedResidualAffineWhenBadExceptionalKind {
    Domain { condition_ordinal: usize },
    Leak { pullback_ordinal: usize },
}

impl GeneratedResidualAffineWhenBadExceptionalKind {
    const fn from_disposition(disposition: AffineWhenBadRelativeLeafDisposition) -> Option<Self> {
        match disposition {
            AffineWhenBadRelativeLeafDisposition::Applicable => None,
            AffineWhenBadRelativeLeafDisposition::ExceptionalDomain { condition_ordinal } => {
                Some(Self::Domain { condition_ordinal })
            }
            AffineWhenBadRelativeLeafDisposition::ExceptionalLeak { pullback_ordinal } => {
                Some(Self::Leak { pullback_ordinal })
            }
        }
    }
}

/// Lifetime-bound view of the private predicates for one authenticated
/// exceptional relative leaf.
///
/// This view deliberately contains no recurrence, condition table, pullback
/// table, or general partition handle.  Its custom `Debug` implementation
/// also withholds the borrowed predicates and their polynomials.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedResidualAffineWhenBadExceptionalLeafSourceView<'certificate> {
    leaf_ordinal: usize,
    relative_case: &'certificate AffineWhenBadRelativeCase,
    predicates: &'certificate [AffineWhenBadRelativePredicate],
    kind: GeneratedResidualAffineWhenBadExceptionalKind,
}

impl<'certificate> GeneratedResidualAffineWhenBadExceptionalLeafSourceView<'certificate> {
    pub(crate) const fn leaf_ordinal(self) -> usize {
        self.leaf_ordinal
    }

    pub(crate) const fn relative_case(self) -> &'certificate AffineWhenBadRelativeCase {
        self.relative_case
    }

    pub(crate) const fn predicates(self) -> &'certificate [AffineWhenBadRelativePredicate] {
        self.predicates
    }

    pub(crate) const fn kind(self) -> GeneratedResidualAffineWhenBadExceptionalKind {
        self.kind
    }
}

impl fmt::Debug for GeneratedResidualAffineWhenBadExceptionalLeafSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineWhenBadExceptionalLeafSourceView")
            .field("leaf_ordinal", &self.leaf_ordinal)
            .field("relative_case", &self.relative_case.id())
            .field("predicate_count", &self.predicates.len())
            .field("kind", &self.kind)
            .field("private_predicates", &"<redacted>")
            .finish()
    }
}

/// Authentication failure while borrowing one exceptional leaf's private
/// relative predicate source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedResidualAffineWhenBadExceptionalLeafSourceError {
    SchemaMismatch,
    PartitionShapeMismatch {
        cases: usize,
        classifications: usize,
    },
    LeafOutOfRange {
        leaf_ordinal: usize,
        available: usize,
    },
    ExpectedCaseMismatch {
        expected: AffineWhenBadRelativeCaseId,
        retained: AffineWhenBadRelativeCaseId,
    },
    CaseClassificationMismatch {
        leaf_ordinal: usize,
        case: AffineWhenBadRelativeCaseId,
        classification: AffineWhenBadRelativeCaseId,
    },
    LeafNotExceptional {
        leaf_ordinal: usize,
    },
    ExceptionalKindMismatch {
        expected: GeneratedResidualAffineWhenBadExceptionalKind,
        retained: GeneratedResidualAffineWhenBadExceptionalKind,
    },
}

impl fmt::Display for GeneratedResidualAffineWhenBadExceptionalLeafSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter
                .write_str("generated affine WhenBad exceptional-leaf source schema mismatch"),
            Self::PartitionShapeMismatch {
                cases,
                classifications,
            } => write!(
                formatter,
                "generated affine WhenBad exceptional-leaf source found {cases} cases but {classifications} classifications",
            ),
            Self::LeafOutOfRange {
                leaf_ordinal,
                available,
            } => write!(
                formatter,
                "generated affine WhenBad exceptional-leaf source leaf {leaf_ordinal} is outside {available} retained leaves",
            ),
            Self::ExpectedCaseMismatch { expected, retained } => write!(
                formatter,
                "generated affine WhenBad exceptional-leaf source expected case {expected}, retained case is {retained}",
            ),
            Self::CaseClassificationMismatch {
                leaf_ordinal,
                case,
                classification,
            } => write!(
                formatter,
                "generated affine WhenBad exceptional-leaf source case/classification mismatch at leaf {leaf_ordinal}: case {case}, classification {classification}",
            ),
            Self::LeafNotExceptional { leaf_ordinal } => write!(
                formatter,
                "generated affine WhenBad leaf {leaf_ordinal} is applicable rather than exceptional",
            ),
            Self::ExceptionalKindMismatch { expected, retained } => write!(
                formatter,
                "generated affine WhenBad exceptional-leaf source expected {expected:?}, retained {retained:?}",
            ),
        }
    }
}

impl std::error::Error for GeneratedResidualAffineWhenBadExceptionalLeafSourceError {}

/// A complete matcher-bound local recurrence whose good target-relative
/// domain is nonempty.  This object does not consume the target; the later
/// ordered group pass owns that transition.
pub struct GeneratedResidualAffineWhenBadCertificate {
    schema: &'static str,
    limits: GeneratedResidualAffineWhenBadLimits,
    ready: GeneratedResidualAffineWhenBadDescentReady,
    conditions: GeneratedResidualAffineConditionAccumulatorCertificate,
    pullbacks: GeneratedResidualAffineWhenBadPullbackGateCertificate,
    partition: AffineWhenBadRelativePartitionCertificate,
    stats: GeneratedResidualAffineWhenBadCompilationStats,
}

impl GeneratedResidualAffineWhenBadCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn binding(&self) -> &GeneratedResidualAffineWhenBadBinding {
        self.ready.binding()
    }

    pub const fn limits(&self) -> GeneratedResidualAffineWhenBadLimits {
        self.limits
    }

    pub const fn stats(&self) -> GeneratedResidualAffineWhenBadCompilationStats {
        self.stats
    }

    pub fn condition_count(&self) -> usize {
        self.conditions.rows().len()
    }

    pub fn condition_view(
        &self,
        ordinal: usize,
    ) -> Option<GeneratedResidualAffineWhenBadConditionView> {
        generated_affine_condition_view(&self.conditions, ordinal)
    }

    pub fn pullback_count(&self) -> usize {
        self.pullbacks.events().len()
    }

    pub fn pullback_view(
        &self,
        ordinal: usize,
    ) -> Option<GeneratedResidualAffineWhenBadPullbackView> {
        let view = self.pullbacks.event_view(ordinal)?;
        Some(GeneratedResidualAffineWhenBadPullbackView {
            ordinal: view.ordinal(),
            rhs_ordinal: view.rhs_ordinal(),
            hazard_class: view.hazard_class(),
            pullback_class: view.pullback_class().into(),
            numerator_gate_class: view.numerator_gate_class().into(),
        })
    }

    pub fn leaf_classifications(&self) -> &[AffineWhenBadRelativeLeafClassification] {
        self.partition.classifications()
    }

    /// Borrow the exact private predicate source for one already-identified
    /// exceptional leaf.
    ///
    /// The caller must present the leaf ordinal, case identifier, and typed
    /// exceptional source (including its source ordinal).  Authentication is
    /// positional: the retained case and classification at that exact leaf
    /// must agree before any predicate reference is returned.
    pub(crate) fn exceptional_leaf_source_view(
        &self,
        leaf_ordinal: usize,
        expected_case: AffineWhenBadRelativeCaseId,
        expected_kind: GeneratedResidualAffineWhenBadExceptionalKind,
    ) -> Result<
        GeneratedResidualAffineWhenBadExceptionalLeafSourceView<'_>,
        GeneratedResidualAffineWhenBadExceptionalLeafSourceError,
    > {
        if self.schema != GENERATED_RESIDUAL_AFFINE_WHEN_BAD_V1_SCHEMA
            || self.partition.schema() != AFFINE_WHEN_BAD_RELATIVE_PARTITION_V1_SCHEMA
        {
            return Err(GeneratedResidualAffineWhenBadExceptionalLeafSourceError::SchemaMismatch);
        }

        let cases = self.partition.cases();
        let classifications = self.partition.classifications();
        if cases.len() != classifications.len() {
            return Err(
                GeneratedResidualAffineWhenBadExceptionalLeafSourceError::PartitionShapeMismatch {
                    cases: cases.len(),
                    classifications: classifications.len(),
                },
            );
        }

        let classification = classifications.get(leaf_ordinal).ok_or(
            GeneratedResidualAffineWhenBadExceptionalLeafSourceError::LeafOutOfRange {
                leaf_ordinal,
                available: classifications.len(),
            },
        )?;
        if classification.case() != expected_case {
            return Err(
                GeneratedResidualAffineWhenBadExceptionalLeafSourceError::ExpectedCaseMismatch {
                    expected: expected_case,
                    retained: classification.case(),
                },
            );
        }

        let relative_case = &cases[leaf_ordinal];
        if relative_case.id() != classification.case() {
            return Err(
                GeneratedResidualAffineWhenBadExceptionalLeafSourceError::CaseClassificationMismatch {
                    leaf_ordinal,
                    case: relative_case.id(),
                    classification: classification.case(),
                },
            );
        }

        let Some(retained_kind) = GeneratedResidualAffineWhenBadExceptionalKind::from_disposition(
            classification.disposition(),
        ) else {
            return Err(
                GeneratedResidualAffineWhenBadExceptionalLeafSourceError::LeafNotExceptional {
                    leaf_ordinal,
                },
            );
        };
        if retained_kind != expected_kind {
            return Err(
                GeneratedResidualAffineWhenBadExceptionalLeafSourceError::ExceptionalKindMismatch {
                    expected: expected_kind,
                    retained: retained_kind,
                },
            );
        }

        Ok(GeneratedResidualAffineWhenBadExceptionalLeafSourceView {
            leaf_ordinal,
            relative_case,
            predicates: relative_case.predicates(),
            kind: retained_kind,
        })
    }

    /// Specialize one leaf which has already been authenticated as the exact
    /// `Applicable` child by its owning sector certificate.
    ///
    /// The complete condition/source census is admitted before the first
    /// condition allocation or clone.  The returned statistics are scalar
    /// and redacted; the recentered parametric row, target predicates, affine
    /// map, exact condition table, and authorization remain sealed.
    pub(crate) fn specialize_sealed_applicable_leaf<'certificate>(
        &'certificate self,
        context: &ParametricCoefficientContext,
        indices: &[i64],
        authorization: &GeneratedSectorAffineSealedLeafAuthorization<'certificate>,
        limits: GeneratedResidualAffineSealedApplicationLimits,
    ) -> Result<
        (
            ConcreteRelation,
            GeneratedResidualAffineSealedApplicationStats,
        ),
        GeneratedResidualAffineWhenBadApplicationError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            self.prepare_sealed_applicable_leaf(context, indices, authorization, limits)?
                .execute()
        }))
        .map_err(|_| GeneratedResidualAffineWhenBadApplicationError::SymbolicaPanic)?
    }

    /// Allocation-free first stage of a sealed application.  The private plan
    /// is consume-once and borrows every authority input it authenticates.
    fn prepare_sealed_applicable_leaf<'certificate, 'context, 'indices, 'authorization>(
        &'certificate self,
        context: &'context ParametricCoefficientContext,
        indices: &'indices [i64],
        authorization: &'authorization GeneratedSectorAffineSealedLeafAuthorization<'certificate>,
        limits: GeneratedResidualAffineSealedApplicationLimits,
    ) -> Result<
        GeneratedResidualAffineSealedApplicationPlan<
            'certificate,
            'context,
            'indices,
            'authorization,
        >,
        GeneratedResidualAffineWhenBadApplicationError,
    > {
        if self.schema != GENERATED_RESIDUAL_AFFINE_WHEN_BAD_V1_SCHEMA {
            return Err(GeneratedResidualAffineWhenBadApplicationError::SchemaMismatch);
        }
        if !authorization.authorizes(self) {
            return Err(GeneratedResidualAffineWhenBadApplicationError::AuthorizationMismatch);
        }
        let expected_leaf_ordinal = authorization.leaf_ordinal();
        let expected_case = authorization.relative_case();
        let input = self.ready.input();
        if context.fingerprint() != self.partition.context_fingerprint()
            || context.fingerprint() != input.relation().context_fingerprint()
        {
            return Err(GeneratedResidualAffineWhenBadApplicationError::WrongContext);
        }
        let expected_arity = self.binding().sector().arity();
        if context.index_count() != expected_arity {
            return Err(GeneratedResidualAffineWhenBadApplicationError::WrongArity {
                expected: expected_arity,
                actual: context.index_count(),
            });
        }
        if indices.len() != expected_arity {
            return Err(GeneratedResidualAffineWhenBadApplicationError::WrongArity {
                expected: expected_arity,
                actual: indices.len(),
            });
        }
        if !self.binding().sector().contains_indices(indices)? {
            return Err(GeneratedResidualAffineWhenBadApplicationError::OutsideSector);
        }
        let classification = self
            .partition
            .classifications()
            .get(expected_leaf_ordinal)
            .ok_or(
                GeneratedResidualAffineWhenBadApplicationError::LeafOutOfRange {
                    leaf_ordinal: expected_leaf_ordinal,
                    available: self.partition.classifications().len(),
                },
            )?;
        if classification.case() != expected_case {
            return Err(GeneratedResidualAffineWhenBadApplicationError::LeafCaseMismatch);
        }
        if classification.disposition() != AffineWhenBadRelativeLeafDisposition::Applicable {
            return Err(GeneratedResidualAffineWhenBadApplicationError::LeafNotApplicable);
        }

        let rows = self.conditions.rows();
        sealed_application_check_limit(
            "sealed affine condition rows",
            rows.len(),
            limits.max_condition_rows,
        )?;
        let mut stats = GeneratedResidualAffineSealedApplicationStats {
            condition_rows: rows.len(),
            ..Default::default()
        };
        let condition_vector_capacity = sealed_condition_vec_capacity_bound(rows.len())?;
        let condition_structure_entries = sealed_application_checked_add(
            "sealed affine temporary condition retained-byte bound",
            condition_vector_capacity,
            rows.len(),
        )?;
        let condition_structure_bytes = sealed_application_checked_mul(
            "sealed affine temporary condition retained-byte bound",
            condition_structure_entries,
            size_of::<ParametricNonZeroCondition>(),
        )?;
        stats.temporary_condition_retained_byte_bound = sealed_application_bounded_add(
            "sealed affine temporary condition retained-byte bound",
            stats.temporary_condition_retained_byte_bound,
            condition_structure_bytes,
            limits.max_temporary_condition_retained_byte_bound,
        )?;

        // This scan authenticates every source locator and charges every
        // provenance occurrence. Associated or duplicate origins are counted
        // repeatedly here: execution may deduplicate them, but preflight must
        // never rely on that allocation-bearing operation to establish its
        // bound.
        for (row_ordinal, row) in rows.iter().enumerate() {
            let remaining_terms = limits
                .max_condition_copy_terms
                .checked_sub(stats.condition_copy_terms)
                .ok_or(
                    GeneratedResidualAffineWhenBadApplicationError::ResourceCountOverflow {
                        resource: "sealed affine condition copy terms",
                    },
                )?;
            let remaining_exponents = limits
                .max_condition_copy_exponent_entries
                .checked_sub(stats.condition_copy_exponent_entries)
                .ok_or(
                    GeneratedResidualAffineWhenBadApplicationError::ResourceCountOverflow {
                        resource: "sealed affine condition copy exponent entries",
                    },
                )?;
            let remaining_integer_bits = limits
                .max_condition_copy_integer_bits
                .checked_sub(stats.condition_copy_integer_bits)
                .ok_or(
                    GeneratedResidualAffineWhenBadApplicationError::ResourceCountOverflow {
                        resource: "sealed affine condition copy integer bits",
                    },
                )?;
            let census = context.preflight_polynomial_validation_payload_with_limits(
                row.polynomial(),
                self.limits.arithmetic.exact_algebra,
                remaining_terms,
                remaining_exponents,
                remaining_integer_bits,
            )?;
            stats.condition_copy_terms = sealed_application_bounded_add(
                "sealed affine condition copy terms",
                stats.condition_copy_terms,
                census.source_terms(),
                limits.max_condition_copy_terms,
            )?;
            stats.condition_copy_exponent_entries = sealed_application_bounded_add(
                "sealed affine condition copy exponent entries",
                stats.condition_copy_exponent_entries,
                census.source_exponent_entries(),
                limits.max_condition_copy_exponent_entries,
            )?;
            stats.condition_copy_integer_bits = sealed_application_bounded_add(
                "sealed affine condition copy integer bits",
                stats.condition_copy_integer_bits,
                census.source_integer_bits(),
                limits.max_condition_copy_integer_bits,
            )?;

            let polynomial_bytes = row.polynomial().owned_retained_byte_bound().ok_or(
                GeneratedResidualAffineWhenBadApplicationError::ResourceCountOverflow {
                    resource: "sealed affine temporary condition retained-byte bound",
                },
            )?;
            stats.temporary_condition_retained_byte_bound = sealed_application_bounded_add(
                "sealed affine temporary condition retained-byte bound",
                stats.temporary_condition_retained_byte_bound,
                polynomial_bytes,
                limits.max_temporary_condition_retained_byte_bound,
            )?;

            let mut row_origin_inputs = 0usize;
            for &input_ordinal in row.source_input_ordinals() {
                stats.condition_source_lookups = sealed_application_bounded_add(
                    "sealed affine condition source lookups",
                    stats.condition_source_lookups,
                    1,
                    limits.max_condition_source_lookups,
                )?;
                let source = self.conditions.inputs().get(input_ordinal).ok_or(
                    GeneratedResidualAffineWhenBadApplicationError::ConditionSourceMismatch,
                )?;
                authenticate_sealed_condition_source_row(source, row_ordinal)?;
                match source.source().locator() {
                    GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
                        entry_ordinal,
                        structural_locus_ordinal,
                    } => {
                        let entry = input
                            .target_guard_composition()
                            .entries()
                            .get(entry_ordinal)
                            .ok_or(
                                GeneratedResidualAffineWhenBadApplicationError::ConditionSourceMismatch,
                            )?;
                        if entry.structural_locus_ordinal() != structural_locus_ordinal {
                            return Err(
                                GeneratedResidualAffineWhenBadApplicationError::ConditionSourceMismatch,
                            );
                        }
                        let condition = entry.class().condition().ok_or(
                            GeneratedResidualAffineWhenBadApplicationError::ConditionSourceMismatch,
                        )?;
                        for origin in condition.origins() {
                            preflight_sealed_condition_origin(
                                origin,
                                &mut row_origin_inputs,
                                &mut stats,
                                limits,
                                self.limits.arithmetic.max_guard_origins,
                            )?;
                        }
                    }
                    GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard {
                        guard_ordinal,
                    } => {
                        let condition = input
                            .relation()
                            .guarded_nonzero_conditions()
                            .get(guard_ordinal)
                            .ok_or(
                                GeneratedResidualAffineWhenBadApplicationError::ConditionSourceMismatch,
                            )?;
                        for origin in condition.origins() {
                            preflight_sealed_condition_origin(
                                origin,
                                &mut row_origin_inputs,
                                &mut stats,
                                limits,
                                self.limits.arithmetic.max_guard_origins,
                            )?;
                        }
                    }
                    GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator {
                        ..
                    } => {
                        preflight_sealed_condition_origin(
                            &GuardOrigin::CoefficientSpecializationDenominator,
                            &mut row_origin_inputs,
                            &mut stats,
                            limits,
                            self.limits.arithmetic.max_guard_origins,
                        )?;
                    }
                }
            }
            if row_origin_inputs == 0 {
                return Err(
                    GeneratedResidualAffineWhenBadApplicationError::ConditionSourceMismatch,
                );
            }
        }

        // The complete condition-side prospective peak is known before any
        // condition container, polynomial, or origin is materialized. Reject
        // here if it cannot fit even before the relation-side peak is added.
        stats.temporary_plus_relation_peak_byte_bound =
            stats.temporary_condition_retained_byte_bound;
        sealed_application_check_limit(
            "sealed affine temporary-plus-relation peak byte bound",
            stats.temporary_plus_relation_peak_byte_bound,
            limits.max_temporary_plus_relation_peak_byte_bound,
        )?;

        Ok(GeneratedResidualAffineSealedApplicationPlan {
            certificate: self,
            context,
            indices,
            authorization,
            limits,
            stats,
        })
    }
}

impl GeneratedResidualAffineSealedApplicationPlan<'_, '_, '_, '_> {
    /// Consume the allocation-free authorization and materialize exactly the
    /// condition stream it admitted.  No plan can be replayed for a second
    /// point or detached from its owner-created leaf authorization.
    fn execute(
        self,
    ) -> Result<
        (
            ConcreteRelation,
            GeneratedResidualAffineSealedApplicationStats,
        ),
        GeneratedResidualAffineWhenBadApplicationError,
    > {
        let Self {
            certificate,
            context,
            indices,
            authorization,
            limits,
            mut stats,
        } = self;
        if !authorization.authorizes(certificate) {
            return Err(GeneratedResidualAffineWhenBadApplicationError::AuthorizationMismatch);
        }
        let input = certificate.ready.input();

        // Rebuild each canonical condition from its complete private source
        // transcript. This unions provenance when target premises, relation
        // guards, or coefficient denominators define the same (or associated)
        // locus. The temporary conditions never leave this sealed call.
        let mut canonical_conditions = Vec::new();
        canonical_conditions
            .try_reserve_exact(certificate.conditions.rows().len())
            .map_err(
                |_| GeneratedResidualAffineWhenBadApplicationError::AllocationFailure {
                    resource: "sealed affine canonical conditions",
                    requested: certificate.conditions.rows().len(),
                },
            )?;
        for (row_ordinal, row) in certificate.conditions.rows().iter().enumerate() {
            let mut origins = BTreeSet::new();
            for &input_ordinal in row.source_input_ordinals() {
                let source = certificate.conditions.inputs().get(input_ordinal).ok_or(
                    GeneratedResidualAffineWhenBadApplicationError::ConditionSourceMismatch,
                )?;
                authenticate_sealed_condition_source_row(source, row_ordinal)?;
                match source.source().locator() {
                    GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
                        entry_ordinal,
                        structural_locus_ordinal,
                    } => {
                        let entry = input
                            .target_guard_composition()
                            .entries()
                            .get(entry_ordinal)
                            .ok_or(
                                GeneratedResidualAffineWhenBadApplicationError::ConditionSourceMismatch,
                            )?;
                        if entry.structural_locus_ordinal() != structural_locus_ordinal {
                            return Err(
                                GeneratedResidualAffineWhenBadApplicationError::ConditionSourceMismatch,
                            );
                        }
                        let condition = entry.class().condition().ok_or(
                            GeneratedResidualAffineWhenBadApplicationError::ConditionSourceMismatch,
                        )?;
                        origins.extend(condition.origins().iter().cloned());
                    }
                    GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard {
                        guard_ordinal,
                    } => {
                        let condition = input
                            .relation()
                            .guarded_nonzero_conditions()
                            .get(guard_ordinal)
                            .ok_or(
                                GeneratedResidualAffineWhenBadApplicationError::ConditionSourceMismatch,
                            )?;
                        origins.extend(condition.origins().iter().cloned());
                    }
                    GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator {
                        ..
                    } => {
                        origins.insert(GuardOrigin::CoefficientSpecializationDenominator);
                    }
                }
            }
            let polynomial = row
                .polynomial()
                .try_copy_authenticated_sparse_payload()
                .map_err(|resource| {
                    GeneratedResidualAffineWhenBadApplicationError::AllocationFailure {
                        resource,
                        requested: row.polynomial().raw().nterms(),
                    }
                })?;
            canonical_conditions.push(context.nonzero_condition_from_prevalidated_parts(
                polynomial,
                origins,
                certificate.limits.arithmetic.exact_algebra,
                certificate.limits.arithmetic.max_guard_origins,
            )?);
        }

        let mut observed_temporary_bytes = canonical_conditions
            .capacity()
            .checked_mul(size_of::<ParametricNonZeroCondition>())
            .ok_or(
                GeneratedResidualAffineWhenBadApplicationError::ResourceCountOverflow {
                    resource: "sealed affine observed temporary condition retained bytes",
                },
            )?;
        for condition in &canonical_conditions {
            observed_temporary_bytes = sealed_application_checked_add(
                "sealed affine observed temporary condition retained bytes",
                observed_temporary_bytes,
                condition.owned_retained_byte_bound().ok_or(
                    GeneratedResidualAffineWhenBadApplicationError::ResourceCountOverflow {
                        resource: "sealed affine observed temporary condition retained bytes",
                    },
                )?,
            )?;
        }
        if observed_temporary_bytes > stats.temporary_condition_retained_byte_bound {
            return Err(
                GeneratedResidualAffineWhenBadApplicationError::MaterializationInvariant {
                    resource: "sealed affine observed temporary condition retained bytes",
                    observed: observed_temporary_bytes,
                    bound: stats.temporary_condition_retained_byte_bound,
                },
            );
        }
        stats.temporary_condition_retained_bytes = observed_temporary_bytes;

        // Stage two owns the complete allocation-free specialization census.
        // It borrows this exact, already bounded condition vector, and its
        // consume-once plan is executed before the vector can be changed.
        let relation_plan = input
            .relation()
            .prepare_concrete_specialization_with_additional_nonzero_conditions(
                context,
                indices,
                &canonical_conditions,
                limits.relation,
            )
            .map_err(GeneratedResidualAffineWhenBadApplicationError::Relation)?;
        stats.relation = relation_plan.preflight();
        stats.temporary_plus_relation_peak_byte_bound = sealed_application_checked_add(
            "sealed affine temporary-plus-relation peak byte bound",
            stats.temporary_condition_retained_byte_bound,
            stats.relation.peak_execution_retained_byte_bound(),
        )?;
        sealed_application_check_limit(
            "sealed affine temporary-plus-relation peak byte bound",
            stats.temporary_plus_relation_peak_byte_bound,
            limits.max_temporary_plus_relation_peak_byte_bound,
        )?;
        let concrete = relation_plan
            .execute()
            .map_err(GeneratedResidualAffineWhenBadApplicationError::Relation)?;
        Ok((concrete, stats))
    }
}

fn authenticate_sealed_condition_source_row(
    source: &GeneratedResidualAffineConditionInputTranscript,
    expected_row_ordinal: usize,
) -> Result<(), GeneratedResidualAffineWhenBadApplicationError> {
    let source_row_ordinal = match source.class() {
        GeneratedResidualAffineConditionInputClass::BaseAssumption { row_ordinal }
        | GeneratedResidualAffineConditionInputClass::IndexDependent { row_ordinal } => row_ordinal,
        GeneratedResidualAffineConditionInputClass::DischargedNonzeroIntegerConstant
        | GeneratedResidualAffineConditionInputClass::IdenticallyZeroCandidate => {
            return Err(GeneratedResidualAffineWhenBadApplicationError::ConditionSourceMismatch);
        }
    };
    if source_row_ordinal != expected_row_ordinal {
        return Err(GeneratedResidualAffineWhenBadApplicationError::ConditionSourceMismatch);
    }
    Ok(())
}

/// Charge one borrowed provenance occurrence before execution can clone it.
/// Duplicate atoms deliberately consume the aggregate and per-condition
/// occurrence budgets again; B-tree deduplication is an execution detail and
/// cannot be part of the prospective proof.
fn preflight_sealed_condition_origin(
    origin: &GuardOrigin,
    row_origin_inputs: &mut usize,
    stats: &mut GeneratedResidualAffineSealedApplicationStats,
    limits: GeneratedResidualAffineSealedApplicationLimits,
    max_origins_per_condition: usize,
) -> Result<(), GeneratedResidualAffineWhenBadApplicationError> {
    *row_origin_inputs = sealed_application_bounded_add(
        "sealed affine condition row origin inputs",
        *row_origin_inputs,
        1,
        max_origins_per_condition,
    )?;
    stats.condition_origin_inputs = sealed_application_bounded_add(
        "sealed affine condition origin inputs",
        stats.condition_origin_inputs,
        1,
        limits.max_condition_origin_inputs,
    )?;
    let retained_bytes = origin.retained_byte_bound().ok_or(
        GeneratedResidualAffineWhenBadApplicationError::ResourceCountOverflow {
            resource: "sealed affine condition origin retained bytes",
        },
    )?;
    stats.condition_origin_retained_bytes = sealed_application_bounded_add(
        "sealed affine condition origin retained bytes",
        stats.condition_origin_retained_bytes,
        retained_bytes,
        limits.max_condition_origin_retained_bytes,
    )?;
    stats.temporary_condition_retained_byte_bound = sealed_application_bounded_add(
        "sealed affine temporary condition retained-byte bound",
        stats.temporary_condition_retained_byte_bound,
        retained_bytes,
        limits.max_temporary_condition_retained_byte_bound,
    )?;
    Ok(())
}

fn sealed_application_checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadApplicationError> {
    left.checked_add(right)
        .ok_or(GeneratedResidualAffineWhenBadApplicationError::ResourceCountOverflow { resource })
}

fn sealed_application_checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadApplicationError> {
    left.checked_mul(right)
        .ok_or(GeneratedResidualAffineWhenBadApplicationError::ResourceCountOverflow { resource })
}

/// Conservative capacity envelope for a fresh `Vec` after
/// `try_reserve_exact(entries)`. Rust's allocator may round the requested
/// capacity up, so prospective retention cannot assume that observed capacity
/// equals the logical row count.
fn sealed_condition_vec_capacity_bound(
    entries: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadApplicationError> {
    if entries == 0 {
        Ok(0)
    } else {
        Ok(sealed_application_checked_mul(
            "sealed affine temporary condition Vec capacity",
            entries,
            2,
        )?
        .max(8))
    }
}

fn sealed_application_check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedResidualAffineWhenBadApplicationError> {
    if requested > limit {
        Err(
            GeneratedResidualAffineWhenBadApplicationError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

fn sealed_application_bounded_add(
    resource: &'static str,
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadApplicationError> {
    let requested = sealed_application_checked_add(resource, current, additional)?;
    sealed_application_check_limit(resource, requested, limit)?;
    Ok(requested)
}

impl GeneratedResidualAffineWhenBadCertificate {
    /// Locate the unique target-relative leaf containing one complete integer
    /// point.  The private partition stays sealed: only its stable leaf
    /// ordinal, case identifier, semantic disposition, and an immutable work
    /// census are returned.
    ///
    /// The complete case/predicate/specialization census is performed before
    /// the first specialization allocation.  Polynomial arithmetic remains
    /// governed by the policy authenticated in this certificate; `limits`
    /// supplies only aggregate ceilings for this query.
    pub fn classify_relative_point(
        &self,
        context: &ParametricCoefficientContext,
        indices: &[i64],
        limits: GeneratedResidualAffineWhenBadPointLimits,
    ) -> Result<
        GeneratedResidualAffineWhenBadPointClassification,
        GeneratedResidualAffineWhenBadPointError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            self.classify_relative_point_inner(context, indices, limits)
        }))
        .map_err(|_| GeneratedResidualAffineWhenBadPointError::SymbolicaPanic)?
    }

    fn classify_relative_point_inner(
        &self,
        context: &ParametricCoefficientContext,
        indices: &[i64],
        limits: GeneratedResidualAffineWhenBadPointLimits,
    ) -> Result<
        GeneratedResidualAffineWhenBadPointClassification,
        GeneratedResidualAffineWhenBadPointError,
    > {
        let mut stats = GeneratedResidualAffineWhenBadPointStats::default();
        stats.context_fingerprint_comparison_bytes = checked_point_add(
            "generated affine WhenBad point context fingerprint comparison bytes",
            self.partition.context_fingerprint().len(),
            context.fingerprint().len(),
        )?;
        check_point_limit(
            "generated affine WhenBad point context fingerprint comparison bytes",
            stats.context_fingerprint_comparison_bytes,
            limits.max_context_fingerprint_comparison_bytes,
        )?;
        if self.partition.context_fingerprint() != context.fingerprint() {
            return Err(GeneratedResidualAffineWhenBadPointError::WrongContext);
        }

        let expected_arity = self.binding().sector().arity();
        if context.index_count() != expected_arity {
            return Err(GeneratedResidualAffineWhenBadPointError::WrongArity {
                expected: expected_arity,
                actual: context.index_count(),
            });
        }
        if indices.len() != expected_arity {
            return Err(GeneratedResidualAffineWhenBadPointError::WrongArity {
                expected: expected_arity,
                actual: indices.len(),
            });
        }
        stats.index_entries = indices.len();
        check_point_limit(
            "generated affine WhenBad point index entries",
            stats.index_entries,
            limits.max_index_entries,
        )?;

        let cases = self.partition.cases();
        let classifications = self.partition.classifications();
        stats.cases = cases.len();
        stats.classifications = classifications.len();
        check_point_limit(
            "generated affine WhenBad point cases",
            stats.cases,
            limits.max_cases,
        )?;
        check_point_limit(
            "generated affine WhenBad point classifications",
            stats.classifications,
            limits.max_classifications,
        )?;
        if cases.len() != classifications.len() {
            return Err(
                GeneratedResidualAffineWhenBadPointError::PartitionShapeMismatch {
                    cases: cases.len(),
                    classifications: classifications.len(),
                },
            );
        }

        // Allocation-free whole-query preflight.  No specialized polynomial
        // is created until every leaf and every aggregate bound has passed.
        for (leaf_ordinal, (case, classification)) in
            cases.iter().zip(classifications.iter()).enumerate()
        {
            if case.id() != classification.case() {
                return Err(
                    GeneratedResidualAffineWhenBadPointError::CaseClassificationMismatch {
                        leaf_ordinal,
                    },
                );
            }
            stats.predicates = checked_point_add(
                "generated affine WhenBad point predicates",
                stats.predicates,
                case.predicates().len(),
            )?;
            check_point_limit(
                "generated affine WhenBad point predicates",
                stats.predicates,
                limits.max_predicates,
            )?;
            for predicate in case.predicates() {
                let preflight = context.preflight_specialize_polynomial(
                    predicate.polynomial(),
                    indices,
                    self.limits.arithmetic,
                )?;
                accumulate_point_specialization_preflight(&mut stats, preflight, limits)?;
            }
        }

        let mut matched = None;
        let mut matched_cases = 0usize;
        for (leaf_ordinal, (case, classification)) in
            cases.iter().zip(classifications.iter()).enumerate()
        {
            let mut accepts = true;
            for predicate in case.predicates() {
                let specialized = context.specialize_polynomial(
                    predicate.polynomial(),
                    indices,
                    self.limits.arithmetic,
                )?;
                accepts &= match predicate.kind() {
                    SymbolicPolynomialPredicateKind::EqualZero => specialized.is_zero(),
                    SymbolicPolynomialPredicateKind::NonZero => !specialized.is_zero(),
                };
            }
            if accepts {
                register_generated_affine_point_match(&mut matched_cases)?;
                matched = Some((leaf_ordinal, classification));
            }
        }

        let Some((leaf_ordinal, classification)) = matched else {
            return Err(
                GeneratedResidualAffineWhenBadPointError::PartitionEvaluationMismatch {
                    matched_cases: 0,
                },
            );
        };
        stats.matched_cases = matched_cases;
        Ok(GeneratedResidualAffineWhenBadPointClassification {
            leaf_ordinal,
            case: classification.case(),
            disposition: classification.disposition(),
            stats,
        })
    }

    /// Test-only reference evaluator over the sealed partition.  It deliberately
    /// does not call [`Self::classify_relative_point`], does not zip cases to
    /// classifications by position, and does not share the resource-census
    /// machinery.  That makes concrete fixture assertions capable of detecting
    /// a wrong leaf ordinal or disposition in the production query without
    /// publishing any private predicate outside test builds.
    #[cfg(test)]
    pub(crate) fn independently_classify_relative_point_for_test(
        &self,
        context: &ParametricCoefficientContext,
        indices: &[i64],
    ) -> Result<
        (
            usize,
            AffineWhenBadRelativeCaseId,
            AffineWhenBadRelativeLeafDisposition,
        ),
        GeneratedResidualAffineWhenBadPointError,
    > {
        if self.partition.context_fingerprint() != context.fingerprint() {
            return Err(GeneratedResidualAffineWhenBadPointError::WrongContext);
        }
        let expected_arity = self.binding().sector().arity();
        if context.index_count() != expected_arity {
            return Err(GeneratedResidualAffineWhenBadPointError::WrongArity {
                expected: expected_arity,
                actual: context.index_count(),
            });
        }
        if indices.len() != expected_arity {
            return Err(GeneratedResidualAffineWhenBadPointError::WrongArity {
                expected: expected_arity,
                actual: indices.len(),
            });
        }

        let mut matched = None;
        let mut matched_cases = 0usize;
        for case in self.partition.cases() {
            let mut accepts = true;
            for predicate in case.predicates() {
                let specialized = context.specialize_polynomial(
                    predicate.polynomial(),
                    indices,
                    self.limits.arithmetic,
                )?;
                accepts &= match predicate.kind() {
                    SymbolicPolynomialPredicateKind::EqualZero => specialized.is_zero(),
                    SymbolicPolynomialPredicateKind::NonZero => !specialized.is_zero(),
                };
            }
            if !accepts {
                continue;
            }
            matched_cases = matched_cases.checked_add(1).ok_or(
                GeneratedResidualAffineWhenBadPointError::ResourceCountOverflow {
                    resource: "generated affine WhenBad independent point-oracle matched cases",
                },
            )?;
            if matched_cases > 1 {
                return Err(
                    GeneratedResidualAffineWhenBadPointError::PartitionEvaluationMismatch {
                        matched_cases,
                    },
                );
            }

            let mut classification_match = None;
            for (leaf_ordinal, classification) in
                self.partition.classifications().iter().enumerate()
            {
                if classification.case() != case.id() {
                    continue;
                }
                if classification_match
                    .replace((leaf_ordinal, classification))
                    .is_some()
                {
                    return Err(
                        GeneratedResidualAffineWhenBadPointError::CaseClassificationMismatch {
                            leaf_ordinal,
                        },
                    );
                }
            }
            let Some((leaf_ordinal, classification)) = classification_match else {
                return Err(
                    GeneratedResidualAffineWhenBadPointError::CaseClassificationMismatch {
                        leaf_ordinal: self.partition.classifications().len(),
                    },
                );
            };
            matched = Some((
                leaf_ordinal,
                classification.case(),
                classification.disposition(),
            ));
        }
        matched.ok_or(
            GeneratedResidualAffineWhenBadPointError::PartitionEvaluationMismatch {
                matched_cases: 0,
            },
        )
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedResidualAffineWhenBadError> {
        if self.schema != GENERATED_RESIDUAL_AFFINE_WHEN_BAD_V1_SCHEMA {
            return Err(GeneratedResidualAffineWhenBadError::SchemaMismatch);
        }
        let rebuilt = GeneratedResidualAffineWhenBadCompiler::compile(
            family,
            context,
            self.ready.input().matcher().clone(),
            self.binding().pivot_ordinal(),
            self.binding().target_case_ordinal(),
            self.limits(),
        )?;
        let GeneratedResidualAffineWhenBadCompilation::Certified(rebuilt) = rebuilt else {
            return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch);
        };
        if self.payload_eq_checked(&rebuilt)? {
            Ok(())
        } else {
            Err(GeneratedResidualAffineWhenBadError::ReplayMismatch)
        }
    }

    pub(crate) fn payload_eq_checked(
        &self,
        other: &Self,
    ) -> Result<bool, GeneratedResidualAffineWhenBadError> {
        preflight_generated_affine_outer_payload_comparison(
            self.ready.input(),
            self.ready.stats(),
            Some(self.conditions.stats()),
            Some(self.pullbacks.stats()),
            Some(&self.partition),
            other.ready.input(),
            other.ready.stats(),
            Some(other.conditions.stats()),
            Some(other.pullbacks.stats()),
            Some(&other.partition),
        )?;
        Ok(self.schema == other.schema
            && self.limits == other.limits
            && self.ready.payload_eq_same_authority(&other.ready)
            && self.conditions == other.conditions
            && self
                .pullbacks
                .payload_eq_checked(&other.pullbacks)
                .map_err(map_generated_affine_pullback_gate_error)?
            && self.partition == other.partition
            && self.stats == other.stats)
    }
}

/// Failure while an owning sector certificate specializes its exact sealed
/// affine leaf. No private polynomial, relation, or point coordinate appears
/// in this error vocabulary.
#[derive(Debug)]
pub(crate) enum GeneratedResidualAffineWhenBadApplicationError {
    AuthorizationMismatch,
    SchemaMismatch,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    OutsideSector,
    LeafOutOfRange {
        leaf_ordinal: usize,
        available: usize,
    },
    LeafCaseMismatch,
    LeafNotApplicable,
    ConditionSourceMismatch,
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    MaterializationInvariant {
        resource: &'static str,
        observed: usize,
        bound: usize,
    },
    SymbolicaPanic,
    Relation(ParametricRelationError),
    Coefficient(ParametricCoefficientError),
    Sector(SectorFoundationError),
}

impl fmt::Display for GeneratedResidualAffineWhenBadApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthorizationMismatch => {
                formatter.write_str("sealed affine leaf lacks its owning sector authorization")
            }
            Self::SchemaMismatch => formatter.write_str("sealed affine rule schema mismatch"),
            Self::WrongContext => formatter.write_str("sealed affine rule context mismatch"),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "sealed affine rule arity is {actual}, expected {expected}"
            ),
            Self::OutsideSector => {
                formatter.write_str("sealed affine rule point lies outside its sector")
            }
            Self::LeafOutOfRange {
                leaf_ordinal,
                available,
            } => write!(
                formatter,
                "sealed affine leaf {leaf_ordinal} is outside {available} retained leaves"
            ),
            Self::LeafCaseMismatch => {
                formatter.write_str("sealed affine leaf case differs from its owner locator")
            }
            Self::LeafNotApplicable => formatter
                .write_str("sealed affine owner locator does not address an applicable leaf"),
            Self::ConditionSourceMismatch => formatter
                .write_str("sealed affine canonical condition has inconsistent private provenance"),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} units for {resource}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::MaterializationInvariant {
                resource,
                observed,
                bound,
            } => write!(
                formatter,
                "{resource} observed {observed} bytes after admitting bound {bound}"
            ),
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during sealed affine specialization")
            }
            Self::Relation(error) => error.fmt(formatter),
            Self::Coefficient(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedResidualAffineWhenBadApplicationError {}

impl From<SectorFoundationError> for GeneratedResidualAffineWhenBadApplicationError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

impl From<ParametricCoefficientError> for GeneratedResidualAffineWhenBadApplicationError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::Coefficient(value)
    }
}

impl fmt::Debug for GeneratedResidualAffineWhenBadCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineWhenBadCertificate")
            .field("schema", &self.schema)
            .field("binding", self.binding())
            .field("condition_count", &self.condition_count())
            .field("pullback_count", &self.pullback_count())
            .field("leaf_count", &self.partition.classifications().len())
            .field("private_payload", &"<redacted>")
            .field("stats", &self.stats)
            .finish()
    }
}

/// Replayable evidence that the candidate's bad formula is literal true on
/// the selected exact target.  It consumes nothing.
pub struct GeneratedResidualAffineWhenBadIdenticallyBad {
    schema: &'static str,
    limits: GeneratedResidualAffineWhenBadLimits,
    reason: GeneratedResidualAffineWhenBadIdenticallyBadReason,
    payload: GeneratedResidualAffineWhenBadIdenticallyBadPayload,
    stats: GeneratedResidualAffineWhenBadCompilationStats,
}

impl GeneratedResidualAffineWhenBadIdenticallyBad {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn binding(&self) -> &GeneratedResidualAffineWhenBadBinding {
        self.payload.ready().binding()
    }

    pub const fn reason(&self) -> GeneratedResidualAffineWhenBadIdenticallyBadReason {
        self.reason
    }

    pub const fn limits(&self) -> GeneratedResidualAffineWhenBadLimits {
        self.limits
    }

    pub const fn stats(&self) -> GeneratedResidualAffineWhenBadCompilationStats {
        self.stats
    }

    pub fn condition_count(&self) -> usize {
        self.payload.conditions().rows().len()
    }

    pub fn condition_view(
        &self,
        ordinal: usize,
    ) -> Option<GeneratedResidualAffineWhenBadConditionView> {
        generated_affine_condition_view(self.payload.conditions(), ordinal)
    }

    pub fn pullback_count(&self) -> usize {
        self.payload
            .pullbacks()
            .map_or(0, |value| value.events().len())
    }

    pub fn leaf_classifications(&self) -> &[AffineWhenBadRelativeLeafClassification] {
        self.payload
            .partition()
            .map_or(&[], |partition| partition.classifications())
    }

    pub(crate) fn payload_eq_checked(
        &self,
        other: &Self,
    ) -> Result<bool, GeneratedResidualAffineWhenBadError> {
        Ok(self.schema == other.schema
            && self.limits == other.limits
            && self.reason == other.reason
            && self.stats == other.stats
            && self.payload.payload_eq_checked(&other.payload)?)
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedResidualAffineWhenBadError> {
        if self.schema != GENERATED_RESIDUAL_AFFINE_WHEN_BAD_V1_SCHEMA {
            return Err(GeneratedResidualAffineWhenBadError::SchemaMismatch);
        }
        let ready = self.payload.ready();
        let rebuilt = GeneratedResidualAffineWhenBadCompiler::compile(
            family,
            context,
            ready.input().matcher().clone(),
            self.binding().pivot_ordinal(),
            self.binding().target_case_ordinal(),
            self.limits,
        )?;
        let GeneratedResidualAffineWhenBadCompilation::IdenticallyBad(rebuilt) = rebuilt else {
            return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch);
        };
        if self.limits == rebuilt.limits
            && self.reason == rebuilt.reason
            && self.stats == rebuilt.stats
            && self.payload.payload_eq_checked(&rebuilt.payload)?
        {
            Ok(())
        } else {
            Err(GeneratedResidualAffineWhenBadError::ReplayMismatch)
        }
    }
}

impl fmt::Debug for GeneratedResidualAffineWhenBadIdenticallyBad {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineWhenBadIdenticallyBad")
            .field("schema", &self.schema)
            .field("binding", self.binding())
            .field("reason", &self.reason)
            .field("condition_count", &self.condition_count())
            .field("pullback_count", &self.pullback_count())
            .field("private_payload", &"<redacted>")
            .field("stats", &self.stats)
            .finish()
    }
}

/// Replayable evidence that the selected ordering cannot orient the private
/// generated row.  The exact RHS shift remains sealed and the target remains
/// available to later pivots.
pub struct GeneratedResidualAffineWhenBadUnsupported {
    schema: &'static str,
    limits: GeneratedResidualAffineWhenBadLimits,
    descent: GeneratedResidualAffineWhenBadDescentUnsupported,
    reason: GeneratedResidualAffineWhenBadUnsupportedReason,
    stats: GeneratedResidualAffineWhenBadCompilationStats,
}

impl GeneratedResidualAffineWhenBadUnsupported {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn binding(&self) -> &GeneratedResidualAffineWhenBadBinding {
        self.descent.binding()
    }

    pub const fn reason(&self) -> GeneratedResidualAffineWhenBadUnsupportedReason {
        self.reason
    }

    pub const fn limits(&self) -> GeneratedResidualAffineWhenBadLimits {
        self.limits
    }

    pub const fn stats(&self) -> GeneratedResidualAffineWhenBadCompilationStats {
        self.stats
    }

    pub(crate) fn payload_eq_checked(
        &self,
        other: &Self,
    ) -> Result<bool, GeneratedResidualAffineWhenBadError> {
        preflight_generated_affine_outer_payload_comparison(
            self.descent.input(),
            self.descent.stats(),
            None,
            None,
            None,
            other.descent.input(),
            other.descent.stats(),
            None,
            None,
            None,
        )?;
        Ok(self.schema == other.schema
            && self.limits == other.limits
            && self.reason == other.reason
            && self.stats == other.stats
            && self.descent.payload_eq_same_authority(&other.descent))
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedResidualAffineWhenBadError> {
        if self.schema != GENERATED_RESIDUAL_AFFINE_WHEN_BAD_V1_SCHEMA {
            return Err(GeneratedResidualAffineWhenBadError::SchemaMismatch);
        }
        let rebuilt = GeneratedResidualAffineWhenBadCompiler::compile(
            family,
            context,
            self.descent.input().matcher().clone(),
            self.binding().pivot_ordinal(),
            self.binding().target_case_ordinal(),
            self.limits,
        )?;
        let GeneratedResidualAffineWhenBadCompilation::Unsupported(rebuilt) = rebuilt else {
            return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch);
        };
        preflight_generated_affine_outer_payload_comparison(
            self.descent.input(),
            self.descent.stats(),
            None,
            None,
            None,
            rebuilt.descent.input(),
            rebuilt.descent.stats(),
            None,
            None,
            None,
        )?;
        if self.limits == rebuilt.limits
            && self.reason == rebuilt.reason
            && self.stats == rebuilt.stats
            && self.descent.payload_eq_same_authority(&rebuilt.descent)
        {
            Ok(())
        } else {
            Err(GeneratedResidualAffineWhenBadError::ReplayMismatch)
        }
    }
}

impl fmt::Debug for GeneratedResidualAffineWhenBadUnsupported {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineWhenBadUnsupported")
            .field("schema", &self.schema)
            .field("binding", self.binding())
            .field("reason", &self.reason)
            .field("private_payload", &"<redacted>")
            .field("stats", &self.stats)
            .finish()
    }
}

/// Transactional target-local outcome.  None of the variants mutates the
/// static inventory or consumes the matched target.
pub enum GeneratedResidualAffineWhenBadCompilation {
    Certified(GeneratedResidualAffineWhenBadCertificate),
    IdenticallyBad(GeneratedResidualAffineWhenBadIdenticallyBad),
    Unsupported(GeneratedResidualAffineWhenBadUnsupported),
}

impl GeneratedResidualAffineWhenBadCompilation {
    pub fn binding(&self) -> &GeneratedResidualAffineWhenBadBinding {
        match self {
            Self::Certified(value) => value.binding(),
            Self::IdenticallyBad(value) => value.binding(),
            Self::Unsupported(value) => value.binding(),
        }
    }

    pub const fn stats(&self) -> GeneratedResidualAffineWhenBadCompilationStats {
        match self {
            Self::Certified(value) => value.stats(),
            Self::IdenticallyBad(value) => value.stats(),
            Self::Unsupported(value) => value.stats(),
        }
    }

    pub(crate) const fn group_resource_usage(
        &self,
    ) -> GeneratedResidualAffineWhenBadGroupResourceUsage {
        self.stats().group_resource_usage()
    }

    /// Complete same-variant equality for the group replay owner. Every
    /// private polynomial/relation-bearing child comparison performs its own
    /// checked preflight; public redacted views are never used as evidence.
    pub(crate) fn payload_eq_checked(
        &self,
        other: &Self,
    ) -> Result<bool, GeneratedResidualAffineWhenBadError> {
        match (self, other) {
            (Self::Certified(left), Self::Certified(right)) => left.payload_eq_checked(right),
            (Self::IdenticallyBad(left), Self::IdenticallyBad(right)) => {
                left.payload_eq_checked(right)
            }
            (Self::Unsupported(left), Self::Unsupported(right)) => left.payload_eq_checked(right),
            _ => Ok(false),
        }
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedResidualAffineWhenBadError> {
        match self {
            Self::Certified(value) => value.replay(family, context),
            Self::IdenticallyBad(value) => value.replay(family, context),
            Self::Unsupported(value) => value.replay(family, context),
        }
    }
}

impl fmt::Debug for GeneratedResidualAffineWhenBadCompilation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Certified(value) => formatter.debug_tuple("Certified").field(value).finish(),
            Self::IdenticallyBad(value) => formatter
                .debug_tuple("IdenticallyBad")
                .field(value)
                .finish(),
            Self::Unsupported(value) => formatter.debug_tuple("Unsupported").field(value).finish(),
        }
    }
}

/// Stateless entry point for one exact `(matcher, pivot, target)` tuple.
pub struct GeneratedResidualAffineWhenBadCompiler;

impl GeneratedResidualAffineWhenBadCompiler {
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        matcher: Arc<GeneratedResidualAffinePivotTargetMatchingCertificate>,
        pending_pivot_ordinal: usize,
        target_case_ordinal: usize,
        limits: GeneratedResidualAffineWhenBadLimits,
    ) -> Result<GeneratedResidualAffineWhenBadCompilation, GeneratedResidualAffineWhenBadError>
    {
        catch_unwind(AssertUnwindSafe(|| {
            Self::compile_inner(
                family,
                context,
                matcher,
                pending_pivot_ordinal,
                target_case_ordinal,
                limits,
            )
        }))
        .map_err(|_| GeneratedResidualAffineWhenBadError::SymbolicaPanic {
            stage: "transactional compilation",
        })?
    }

    fn compile_inner(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        matcher: Arc<GeneratedResidualAffinePivotTargetMatchingCertificate>,
        pending_pivot_ordinal: usize,
        target_case_ordinal: usize,
        limits: GeneratedResidualAffineWhenBadLimits,
    ) -> Result<GeneratedResidualAffineWhenBadCompilation, GeneratedResidualAffineWhenBadError>
    {
        let outcome_inline_bytes = size_of::<GeneratedResidualAffineWhenBadCompilation>();
        check_limit(
            "generated affine WhenBad retained bytes",
            outcome_inline_bytes,
            limits.max_retained_bytes,
        )?;
        let mut child_limits = limits;
        let outer_heap_budget = remaining_limit(
            "generated affine WhenBad retained bytes",
            limits.max_retained_bytes,
            outcome_inline_bytes,
        )?;
        child_limits.max_retained_bytes = checked_add(
            "generated affine WhenBad retained bytes",
            outer_heap_budget,
            generated_affine_descent_inline_allowance(),
        )?;
        let input = authenticate_generated_residual_affine_when_bad_input(
            family,
            context,
            matcher,
            pending_pivot_ordinal,
            target_case_ordinal,
            child_limits,
        )?;
        let private_payload_comparison_bytes =
            preflight_generated_affine_private_payload_comparison(&input, &input)?;
        let descent = compile_generated_residual_affine_when_bad_descent(input)
            .map_err(map_generated_affine_descent_error)?;
        let ready = match descent {
            GeneratedResidualAffineWhenBadDescentCompilation::Unsupported(descent) => {
                let reason = descent.reason().into();
                preflight_generated_affine_outer_payload_comparison(
                    descent.input(),
                    descent.stats(),
                    None,
                    None,
                    None,
                    descent.input(),
                    descent.stats(),
                    None,
                    None,
                    None,
                )?;
                let stats = generated_affine_compilation_stats(
                    descent.input(),
                    descent.stats(),
                    None,
                    None,
                    None,
                    None,
                    GeneratedAffineCompilationShape::Unsupported,
                    limits,
                )?;
                return Ok(GeneratedResidualAffineWhenBadCompilation::Unsupported(
                    GeneratedResidualAffineWhenBadUnsupported {
                        schema: GENERATED_RESIDUAL_AFFINE_WHEN_BAD_V1_SCHEMA,
                        limits,
                        descent,
                        reason,
                        stats,
                    },
                ));
            }
            GeneratedResidualAffineWhenBadDescentCompilation::Ready(ready) => ready,
        };

        let ready_incremental_retained = generated_affine_incremental_retained_bytes(
            ready.stats().retained_bytes(),
            size_of::<GeneratedResidualAffineWhenBadDescentReady>(),
        )?;
        let condition_retained_remaining = generated_affine_full_child_retained_limit(
            outer_heap_budget,
            ready_incremental_retained,
            size_of::<GeneratedResidualAffineConditionAccumulatorCertificate>(),
        )?;
        let condition_payload_units_remaining = remaining_limit(
            "generated affine WhenBad payload comparison units",
            limits.max_payload_comparison_units,
            ready.stats().payload_comparison_units_observed(),
        )?;
        let condition_payload_bytes_remaining = remaining_limit(
            "generated affine WhenBad payload comparison bytes",
            limits.max_payload_comparison_bytes,
            private_payload_comparison_bytes,
        )?;
        let conditions = compile_generated_residual_affine_when_bad_conditions(
            context,
            ready.input(),
            condition_retained_remaining,
            condition_payload_units_remaining,
            condition_payload_bytes_remaining,
        )?;
        check_limit(
            "generated affine WhenBad payload comparison units",
            checked_add(
                "generated affine WhenBad payload comparison units",
                ready.stats().payload_comparison_units_observed(),
                generated_affine_condition_payload_comparison_units(conditions.stats())?,
            )?,
            limits.max_payload_comparison_units,
        )?;
        check_limit(
            "generated affine WhenBad payload comparison bytes",
            checked_add(
                "generated affine WhenBad payload comparison bytes",
                private_payload_comparison_bytes,
                conditions.stats().context_fingerprint_comparison_bytes(),
            )?,
            limits.max_payload_comparison_bytes,
        )?;
        check_limit(
            "generated affine WhenBad payload comparison integer bits",
            conditions.stats().equality_integer_bits(),
            limits.max_payload_comparison_integer_bits,
        )?;
        check_generated_affine_condition_comparison_limits(
            conditions.stats(),
            ready.input().limits(),
        )?;
        if conditions.candidate_is_identically_bad() {
            preflight_generated_affine_outer_payload_comparison(
                ready.input(),
                ready.stats(),
                Some(conditions.stats()),
                None,
                None,
                ready.input(),
                ready.stats(),
                Some(conditions.stats()),
                None,
                None,
            )?;
            let condition_input_ordinal = first_identically_zero_candidate_condition_input(
                &conditions,
            )
            .ok_or(GeneratedResidualAffineWhenBadError::ConditionInvariant {
                stage: "identically-zero candidate condition selection",
            })?;
            let stats = generated_affine_compilation_stats(
                ready.input(),
                ready.stats(),
                Some(conditions.stats()),
                None,
                None,
                None,
                GeneratedAffineCompilationShape::Condition,
                limits,
            )?;
            return Ok(GeneratedResidualAffineWhenBadCompilation::IdenticallyBad(
                GeneratedResidualAffineWhenBadIdenticallyBad {
                    schema: GENERATED_RESIDUAL_AFFINE_WHEN_BAD_V1_SCHEMA,
                    limits,
                    reason: GeneratedResidualAffineWhenBadIdenticallyBadReason::RequiredNonzeroConditionIsZero {
                        condition_input_ordinal,
                    },
                    payload: GeneratedResidualAffineWhenBadIdenticallyBadPayload::Condition {
                        ready,
                        conditions,
                    },
                    stats,
                },
            ));
        }

        let pullback_limits = projected_generated_affine_pullback_gate_limits(
            &ready,
            &conditions,
            private_payload_comparison_bytes,
        )?;
        let pullback_compilation = compile_generated_residual_affine_when_bad_pullback_gate_table(
            context,
            &ready,
            pullback_limits,
        )
        .map_err(map_generated_affine_pullback_gate_error)?;
        if let Some(pullback_ordinal) =
            pullback_compilation.universal_coefficient_nonzero_leak_ordinal()
        {
            let pullbacks = pullback_compilation.into_certificate();
            preflight_generated_affine_outer_payload_comparison(
                ready.input(),
                ready.stats(),
                Some(conditions.stats()),
                Some(pullbacks.stats()),
                None,
                ready.input(),
                ready.stats(),
                Some(conditions.stats()),
                Some(pullbacks.stats()),
                None,
            )?;
            let stats = generated_affine_compilation_stats(
                ready.input(),
                ready.stats(),
                Some(conditions.stats()),
                Some(pullbacks.stats()),
                None,
                None,
                GeneratedAffineCompilationShape::Pullback,
                limits,
            )?;
            return Ok(GeneratedResidualAffineWhenBadCompilation::IdenticallyBad(
                GeneratedResidualAffineWhenBadIdenticallyBad {
                    schema: GENERATED_RESIDUAL_AFFINE_WHEN_BAD_V1_SCHEMA,
                    limits,
                    reason: GeneratedResidualAffineWhenBadIdenticallyBadReason::UniversalCoefficientNonzeroLeak {
                        pullback_ordinal,
                    },
                    payload: GeneratedResidualAffineWhenBadIdenticallyBadPayload::Pullback {
                        ready,
                        conditions,
                        pullbacks,
                    },
                    stats,
                },
            ));
        }
        let GeneratedResidualAffineWhenBadPullbackGateCompilation::Ready(pullbacks) =
            pullback_compilation
        else {
            return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch);
        };
        let partition_compilation = compile_generated_affine_relative_partition(
            context,
            &ready,
            &conditions,
            &pullbacks,
            private_payload_comparison_bytes,
        )?;
        let partition = partition_compilation.certificate;
        let assembly_stats = partition_compilation.assembly_stats;
        preflight_generated_affine_outer_payload_comparison(
            ready.input(),
            ready.stats(),
            Some(conditions.stats()),
            Some(pullbacks.stats()),
            Some(&partition),
            ready.input(),
            ready.stats(),
            Some(conditions.stats()),
            Some(pullbacks.stats()),
            Some(&partition),
        )?;
        let stats = generated_affine_compilation_stats(
            ready.input(),
            ready.stats(),
            Some(conditions.stats()),
            Some(pullbacks.stats()),
            Some(&partition),
            Some(assembly_stats),
            GeneratedAffineCompilationShape::Partition,
            limits,
        )?;
        if stats.applicable_leaves() == 0 {
            return Ok(GeneratedResidualAffineWhenBadCompilation::IdenticallyBad(
                GeneratedResidualAffineWhenBadIdenticallyBad {
                    schema: GENERATED_RESIDUAL_AFFINE_WHEN_BAD_V1_SCHEMA,
                    limits,
                    reason: GeneratedResidualAffineWhenBadIdenticallyBadReason::NoStructurallyApplicableRelativeLeaf,
                    payload: GeneratedResidualAffineWhenBadIdenticallyBadPayload::Partition {
                        ready,
                        conditions,
                        pullbacks,
                        partition,
                    },
                    stats,
                },
            ));
        }
        Ok(GeneratedResidualAffineWhenBadCompilation::Certified(
            GeneratedResidualAffineWhenBadCertificate {
                schema: GENERATED_RESIDUAL_AFFINE_WHEN_BAD_V1_SCHEMA,
                limits,
                ready,
                conditions,
                pullbacks,
                partition,
                stats,
            },
        ))
    }
}

#[cfg(test)]
mod focused_budget_tests {
    use super::*;
    use crate::{CoefficientContext, ExactAlgebraError, ParametricCoefficientError};

    fn payload_limit(error: GeneratedResidualAffineWhenBadError) -> (&'static str, usize, usize) {
        match error {
            GeneratedResidualAffineWhenBadError::ResourceLimit {
                resource,
                requested,
                limit,
            }
            | GeneratedResidualAffineWhenBadError::ParametricCoefficient(
                ParametricCoefficientError::ResourceLimit {
                    resource,
                    requested,
                    limit,
                },
            )
            | GeneratedResidualAffineWhenBadError::ParametricCoefficient(
                ParametricCoefficientError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
                    resource,
                    requested,
                    limit,
                }),
            ) => (resource, requested, limit),
            other => panic!("expected payload resource limit, got {other:?}"),
        }
    }

    #[test]
    fn condition_shared_arc_hostile_preflight_has_exact_one_below_edge() {
        // I=2 and P=C(2,2)=1: two copy seams per input plus two
        // insertion/replay seams per pair gives 2I+4P=8.
        let exact = generated_affine_condition_shared_allocation_preflight(2).unwrap();
        assert_eq!(exact, 8);
        check_limit(
            "generated affine WhenBad condition shared-allocation comparison bound",
            exact,
            exact,
        )
        .unwrap();
        let (resource, requested, limit) = payload_limit(
            check_limit(
                "generated affine WhenBad condition shared-allocation comparison bound",
                exact,
                exact - 1,
            )
            .unwrap_err(),
        );
        assert_eq!(
            resource,
            "generated affine WhenBad condition shared-allocation comparison bound"
        );
        assert_eq!((requested, limit), (8, 7));
    }

    fn assembly_payload_run(
        mut limits: GeneratedResidualAffineWhenBadLimits,
        prior_payload_units: usize,
        prior_payload_bytes: usize,
    ) -> Result<GeneratedAffineLocusAssemblyStats, (usize, GeneratedResidualAffineWhenBadError)>
    {
        let base = CoefficientContext::new(["theta"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "outer-assembly-payload-budget", 1)
                .unwrap();
        let n = context.index(0).unwrap();
        let one = context.one();
        let two = context.integer(2);
        let n_squared = context.mul(&n, &n).unwrap();
        let loci = [
            context
                .numerator_condition(&context.add(&n, &one).unwrap())
                .unwrap(),
            context
                .numerator_condition(&context.add(&n, &two).unwrap())
                .unwrap(),
            context
                .numerator_condition(&context.add(&n_squared, &one).unwrap())
                .unwrap(),
        ];
        // Leave every unrelated aggregate at its production default.
        limits.max_structural_loci = limits.max_structural_loci.max(loci.len());
        let mut assembly = GeneratedAffineRelativeProblemAssembly::try_with_precharged_capacities(
            loci.len(),
            0,
            0,
            usize::MAX,
        )
        .unwrap();
        for (ordinal, locus) in loci.iter().enumerate() {
            if let Err(error) = assembly.intern(
                &context,
                locus,
                limits,
                GeneratedAffineComparisonWork::default(),
                prior_payload_units,
                prior_payload_bytes,
            ) {
                return Err((ordinal, error));
            }
        }
        Ok(assembly.stats)
    }

    #[test]
    fn assembly_associate_payload_is_cumulative_with_exact_one_below_edges() {
        let prior_units = 17usize;
        let prior_bytes = 29usize;
        let baseline = assembly_payload_run(
            GeneratedResidualAffineWhenBadLimits::default(),
            prior_units,
            prior_bytes,
        )
        .unwrap();
        assert!(baseline.associate_checks >= 3);
        assert!(baseline.payload_comparison_units > 0);
        assert!(baseline.payload_comparison_bytes > 0);

        let mut exact = GeneratedResidualAffineWhenBadLimits::default();
        exact.max_payload_comparison_units = prior_units + baseline.payload_comparison_units;
        exact.max_payload_comparison_bytes = prior_bytes + baseline.payload_comparison_bytes;
        assert_eq!(
            assembly_payload_run(exact, prior_units, prior_bytes).unwrap(),
            baseline
        );

        let mut units_one_below = exact;
        units_one_below.max_payload_comparison_units -= 1;
        let (ordinal, error) =
            assembly_payload_run(units_one_below, prior_units, prior_bytes).unwrap_err();
        assert_eq!(
            ordinal, 2,
            "the third distinct locus must be the offending call"
        );
        let (_, requested, limit) = payload_limit(error);
        assert!(requested > limit);

        let mut bytes_one_below = exact;
        bytes_one_below.max_payload_comparison_bytes -= 1;
        let (ordinal, error) =
            assembly_payload_run(bytes_one_below, prior_units, prior_bytes).unwrap_err();
        assert_eq!(
            ordinal, 2,
            "the third distinct locus must be the offending call"
        );
        let (_, requested, limit) = payload_limit(error);
        assert!(requested > limit);
    }

    #[test]
    fn every_outer_shape_replaces_child_roots_exactly() {
        let enum_inline = size_of::<GeneratedResidualAffineWhenBadCompilation>();
        let shapes = [
            (
                GeneratedAffineCompilationShape::Unsupported,
                size_of::<GeneratedResidualAffineWhenBadDescentUnsupported>() + 3,
                None,
                None,
                None,
                3usize,
            ),
            (
                GeneratedAffineCompilationShape::Condition,
                size_of::<GeneratedResidualAffineWhenBadDescentReady>() + 3,
                Some(size_of::<GeneratedResidualAffineConditionAccumulatorCertificate>() + 5),
                None,
                None,
                8usize,
            ),
            (
                GeneratedAffineCompilationShape::Pullback,
                size_of::<GeneratedResidualAffineWhenBadDescentReady>() + 3,
                Some(size_of::<GeneratedResidualAffineConditionAccumulatorCertificate>() + 5),
                Some(size_of::<GeneratedResidualAffineWhenBadPullbackGateCertificate>() + 7),
                None,
                15usize,
            ),
            (
                GeneratedAffineCompilationShape::Partition,
                size_of::<GeneratedResidualAffineWhenBadDescentReady>() + 3,
                Some(size_of::<GeneratedResidualAffineConditionAccumulatorCertificate>() + 5),
                Some(size_of::<GeneratedResidualAffineWhenBadPullbackGateCertificate>() + 7),
                Some(size_of::<AffineWhenBadRelativePartitionCertificate>() + 11),
                26usize,
            ),
        ];
        for (shape, descent, condition, pullback, partition, heap) in shapes {
            let retained = generated_affine_outer_retained_bytes(
                shape, descent, condition, pullback, partition,
            )
            .unwrap();
            assert_eq!(retained, enum_inline + heap);
            check_limit(
                "generated affine WhenBad retained bytes",
                retained,
                retained,
            )
            .unwrap();
            let (_, requested, limit) = payload_limit(
                check_limit(
                    "generated affine WhenBad retained bytes",
                    retained,
                    retained - 1,
                )
                .unwrap_err(),
            );
            assert_eq!(requested, retained);
            assert_eq!(limit + 1, retained);
        }

        assert!(matches!(
            generated_affine_outer_retained_bytes(
                GeneratedAffineCompilationShape::Condition,
                size_of::<GeneratedResidualAffineWhenBadDescentReady>(),
                None,
                None,
                None,
            ),
            Err(GeneratedResidualAffineWhenBadError::ConditionInvariant {
                stage: "outer compilation shape census"
            })
        ));
    }

    #[test]
    fn relative_point_match_registration_rejects_a_second_leaf() {
        let mut matched_cases = 0usize;
        register_generated_affine_point_match(&mut matched_cases).unwrap();
        assert_eq!(matched_cases, 1);
        assert_eq!(
            register_generated_affine_point_match(&mut matched_cases).unwrap_err(),
            GeneratedResidualAffineWhenBadPointError::PartitionEvaluationMismatch {
                matched_cases: 2,
            },
        );
    }
}
