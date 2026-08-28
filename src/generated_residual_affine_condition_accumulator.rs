//! Source-neutral domain-condition accumulation for generated affine `WhenBad`.
//!
//! This module deliberately knows nothing about a generated relation or its
//! matcher authority.  Its caller must first authenticate and order the
//! condition inputs, then hand over borrowed polynomial predicates with typed
//! source locators.  The accumulator provides the bounded canonicalization
//! seam shared by the later matcher-bound compiler and by synthetic tests.
//!
//! Raw predicates and source shifts are retained only in crate-private replay
//! payloads.  Every `Debug` implementation and every view returned here is
//! redacted, so logging the certificate cannot disclose a private recurrence.

use std::fmt;
use std::fmt::Write as _;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::domains::integer::Integer;
use symbolica::poly::PolyVariable;

use crate::parametric_coefficient::{
    ParametricBasePolynomialAssociateLimits, ParametricBasePolynomialAssociateStats,
    ParametricPolynomialAssociateLimits, ParametricPolynomialAssociateStats,
};
use crate::{
    IndexShift, ParametricCoefficientContext, ParametricCoefficientError, ParametricPolynomial,
    algebra::ExactAlgebraLimits, algebra::SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
};

/// Whether a condition is already known on the selected target or is newly
/// required by the candidate recurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedResidualAffineConditionScope {
    InheritedTargetPremise,
    CandidateRequired,
}

/// Typed location of a coefficient denominator in centered relation order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum GeneratedResidualAffineConditionRelationTerm {
    Pivot,
    Rhs { rhs_ordinal: usize },
}

/// Redacted, stable source identity.  Denominator translations are kept in a
/// separate private payload and never appear in this type's `Debug` output.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GeneratedResidualAffineConditionSourceLocator {
    TargetBranchGuard {
        entry_ordinal: usize,
        structural_locus_ordinal: usize,
    },
    /// One explicit `NonZero` predicate from an exceptional-domain source.
    /// Keeping the predicate position distinct from the target-guard position
    /// prevents a refinement mapper from flattening two independently ordered
    /// source lanes into one ambiguous ordinal space.
    ExceptionalNonZeroPredicate {
        predicate_ordinal: usize,
        locus_ordinal: usize,
    },
    RecenteredRelationGuard {
        guard_ordinal: usize,
    },
    CoefficientDenominator {
        term: GeneratedResidualAffineConditionRelationTerm,
    },
}

impl fmt::Debug for GeneratedResidualAffineConditionSourceLocator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetBranchGuard {
                entry_ordinal,
                structural_locus_ordinal,
            } => formatter
                .debug_struct("TargetBranchGuard")
                .field("entry_ordinal", entry_ordinal)
                .field("structural_locus_ordinal", structural_locus_ordinal)
                .finish(),
            Self::ExceptionalNonZeroPredicate {
                predicate_ordinal,
                locus_ordinal,
            } => formatter
                .debug_struct("ExceptionalNonZeroPredicate")
                .field("predicate_ordinal", predicate_ordinal)
                .field("locus_ordinal", locus_ordinal)
                .finish(),
            Self::RecenteredRelationGuard { guard_ordinal } => formatter
                .debug_struct("RecenteredRelationGuard")
                .field("guard_ordinal", guard_ordinal)
                .finish(),
            Self::CoefficientDenominator { term } => formatter
                .debug_struct("CoefficientDenominator")
                .field("term", term)
                .finish(),
        }
    }
}

/// One already-authenticated condition input.
///
/// The polynomial and optional shift are borrowed so the source authority can
/// remain private.  Successful accumulation makes every durable copy through
/// an explicit fallible allocation seam.
pub(crate) struct GeneratedResidualAffineConditionInput<'a> {
    polynomial: &'a ParametricPolynomial,
    scope: GeneratedResidualAffineConditionScope,
    source: GeneratedResidualAffineConditionSourceLocator,
    private_shift: Option<&'a IndexShift>,
}

impl<'a> GeneratedResidualAffineConditionInput<'a> {
    pub(crate) const fn new(
        polynomial: &'a ParametricPolynomial,
        scope: GeneratedResidualAffineConditionScope,
        source: GeneratedResidualAffineConditionSourceLocator,
        private_shift: Option<&'a IndexShift>,
    ) -> Self {
        Self {
            polynomial,
            scope,
            source,
            private_shift,
        }
    }

    pub(crate) const fn polynomial(&self) -> &'a ParametricPolynomial {
        self.polynomial
    }

    pub(crate) const fn scope(&self) -> GeneratedResidualAffineConditionScope {
        self.scope
    }

    pub(crate) const fn source(&self) -> GeneratedResidualAffineConditionSourceLocator {
        self.source
    }

    pub(crate) const fn private_shift(&self) -> Option<&'a IndexShift> {
        self.private_shift
    }
}

impl fmt::Debug for GeneratedResidualAffineConditionInput<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineConditionInput")
            .field("polynomial", &"<redacted>")
            .field("scope", &self.scope)
            .field("source", &self.source)
            .field("private_shift", &self.private_shift.map(|_| "<redacted>"))
            .finish()
    }
}

/// Aggregate limits for one condition stream.
///
/// `exact_algebra` is never reset blindly for an associate proof.  Before
/// every call the accumulator intersects it with the still-unspent aggregate
/// associate allowance. Every `max_associate_*` and `max_base_associate_*`
/// field is stream-aggregate: even child fields named `peak_*` or
/// `*_envelope` are summed across calls here so certificate replay has one
/// exact addition law per counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineConditionAccumulatorLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_context_fingerprint_bytes: usize,
    pub max_context_fingerprint_comparison_bytes: usize,
    pub max_variable_map_entry_comparisons: usize,
    pub max_shared_allocation_identity_comparisons: usize,
    pub max_ambient_variables: usize,
    pub max_free_positions: usize,
    pub max_condition_inputs: usize,
    pub max_source_inputs: usize,
    pub max_condition_sources: usize,
    pub max_unique_rows: usize,
    pub max_unique_inherited_rows: usize,
    pub max_unique_candidate_rows: usize,
    pub max_source_shift_components: usize,
    pub max_input_polynomial_terms: usize,
    pub max_input_polynomial_exponent_entries: usize,
    pub max_input_polynomial_integer_bits: usize,
    pub max_dependency_exponent_entries: usize,
    pub max_equality_comparisons: usize,
    pub max_equality_term_units: usize,
    pub max_equality_exponent_entries: usize,
    pub max_equality_integer_bits: usize,
    pub max_associate_checks: usize,
    pub max_associate_term_units: usize,
    pub max_associate_exponent_entries: usize,
    pub max_associate_integer_bits: usize,
    pub max_associate_validation_terms: usize,
    pub max_associate_validation_exponent_entries: usize,
    pub max_associate_validation_integer_bits: usize,
    pub max_associate_projection_exponent_entries: usize,
    pub max_associate_projection_coefficient_capacity_bytes: usize,
    pub max_associate_projection_group_bound: usize,
    pub max_associate_projection_variable_mask_comparison_bound: usize,
    pub max_associate_projection_hash_key_exponent_entry_bound: usize,
    pub max_associate_projection_coefficient_append_comparison_bound: usize,
    pub max_associate_projection_sorted_insert_comparison_bound: usize,
    pub max_associate_projection_sorted_insert_move_exponent_entry_bound: usize,
    pub max_associate_index_groups: usize,
    pub max_associate_index_support_comparison_entries: usize,
    pub max_associate_anchor_cost_operations: usize,
    pub max_associate_native_cross_term_pairs: usize,
    pub max_associate_peak_native_cross_term_pairs: usize,
    pub max_associate_native_base_exponent_additions: usize,
    pub max_associate_native_metadata_exponent_entry_inspection_bound: usize,
    pub max_associate_native_metadata_integer_entry_inspection_bound: usize,
    pub max_associate_native_integer_multiplication_bit_work_bound: usize,
    pub max_associate_native_integer_collection_bit_work_bound: usize,
    pub max_associate_native_output_term_bound: usize,
    pub max_associate_native_output_exponent_entry_bound: usize,
    pub max_associate_native_output_integer_bit_bound: usize,
    pub max_associate_native_dense_workspace_entries: usize,
    pub max_associate_native_heap_workspace_pair_bound: usize,
    pub max_associate_native_workspace_byte_envelope: usize,
    pub max_associate_rustred_visible_temporary_byte_envelope: usize,
    pub max_associate_combined_temporary_byte_envelope: usize,
    pub max_base_associate_validation_terms: usize,
    pub max_base_associate_validation_exponent_entries: usize,
    pub max_base_associate_validation_integer_bits: usize,
    pub max_base_associate_source_owned_bytes: usize,
    pub max_base_associate_index_exponent_entries: usize,
    pub max_base_associate_native_scale_calls: usize,
    pub max_base_associate_native_coefficient_multiplications: usize,
    pub max_base_associate_native_integer_multiplication_bit_work_bound: usize,
    pub max_base_associate_output_terms: usize,
    pub max_base_associate_output_exponent_entries: usize,
    pub max_base_associate_output_integer_bit_bound: usize,
    pub max_base_associate_output_retained_byte_bound: usize,
    pub max_base_associate_payload_comparison_terms: usize,
    pub max_base_associate_payload_comparison_exponent_entries: usize,
    pub max_base_associate_payload_comparison_integer_bit_bound: usize,
    pub max_base_associate_native_workspace_byte_envelope: usize,
    pub max_base_associate_rustred_visible_temporary_byte_envelope: usize,
    pub max_base_associate_combined_temporary_byte_envelope: usize,
    pub max_retained_polynomial_terms: usize,
    pub max_retained_polynomial_exponent_entries: usize,
    pub max_retained_polynomial_integer_bits: usize,
    pub max_retained_polynomial_display_bytes: usize,
    pub max_retained_polynomial_owned_bytes: usize,
    pub max_retained_bytes: usize,
    pub max_final_invariant_entries: usize,
}

impl Default for GeneratedResidualAffineConditionAccumulatorLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_context_fingerprint_bytes: 1024 * 1024,
            max_context_fingerprint_comparison_bytes: 16 * 1024 * 1024 * 1024,
            max_variable_map_entry_comparisons: portable_usize(64_000_000_000),
            max_shared_allocation_identity_comparisons: portable_usize(64_000_000_000),
            max_ambient_variables: 2_000_000,
            max_free_positions: 1_000_000,
            max_condition_inputs: 192_000_000,
            max_source_inputs: 192_000_000,
            max_condition_sources: 512_000_000,
            max_unique_rows: 192_000_000,
            max_unique_inherited_rows: 64_000_000,
            max_unique_candidate_rows: 128_000_000,
            max_source_shift_components: portable_usize(64_000_000_000),
            max_input_polynomial_terms: 1_000_000_000,
            max_input_polynomial_exponent_entries: portable_usize(32_000_000_000),
            max_input_polynomial_integer_bits: portable_usize(16_000_000_000_000_000),
            max_dependency_exponent_entries: portable_usize(64_000_000_000),
            max_equality_comparisons: 1_000_000_000,
            max_equality_term_units: portable_usize(16_000_000_000),
            max_equality_exponent_entries: portable_usize(64_000_000_000),
            max_equality_integer_bits: portable_usize(16_000_000_000_000_000),
            max_associate_checks: 1_000_000_000,
            max_associate_term_units: portable_usize(16_000_000_000),
            max_associate_exponent_entries: portable_usize(64_000_000_000),
            max_associate_integer_bits: portable_usize(16_000_000_000_000_000),
            max_associate_validation_terms: portable_usize(64_000_000_000),
            max_associate_validation_exponent_entries: portable_usize(256_000_000_000),
            max_associate_validation_integer_bits: portable_usize(64_000_000_000_000_000),
            max_associate_projection_exponent_entries: portable_usize(256_000_000_000),
            max_associate_projection_coefficient_capacity_bytes: portable_usize(64_000_000_000_000),
            max_associate_projection_group_bound: portable_usize(64_000_000_000),
            max_associate_projection_variable_mask_comparison_bound: portable_usize(
                64_000_000_000_000_000,
            ),
            max_associate_projection_hash_key_exponent_entry_bound: portable_usize(
                64_000_000_000_000_000,
            ),
            max_associate_projection_coefficient_append_comparison_bound: portable_usize(
                64_000_000_000_000_000,
            ),
            max_associate_projection_sorted_insert_comparison_bound: portable_usize(
                64_000_000_000_000_000,
            ),
            max_associate_projection_sorted_insert_move_exponent_entry_bound: portable_usize(
                64_000_000_000_000_000,
            ),
            max_associate_index_groups: portable_usize(64_000_000_000),
            max_associate_index_support_comparison_entries: portable_usize(64_000_000_000_000_000),
            max_associate_anchor_cost_operations: portable_usize(320_000_000_000),
            max_associate_native_cross_term_pairs: portable_usize(16_000_000_000),
            max_associate_peak_native_cross_term_pairs: portable_usize(16_000_000_000),
            max_associate_native_base_exponent_additions: portable_usize(128_000_000_000_000_000),
            max_associate_native_metadata_exponent_entry_inspection_bound: portable_usize(
                128_000_000_000_000_000,
            ),
            max_associate_native_metadata_integer_entry_inspection_bound: portable_usize(
                64_000_000_000_000_000,
            ),
            max_associate_native_integer_multiplication_bit_work_bound: portable_usize(
                16_000_000_000_000_000,
            ),
            max_associate_native_integer_collection_bit_work_bound: portable_usize(
                16_000_000_000_000_000,
            ),
            max_associate_native_output_term_bound: portable_usize(16_000_000_000),
            max_associate_native_output_exponent_entry_bound: portable_usize(
                16_000_000_000_000_000,
            ),
            max_associate_native_output_integer_bit_bound: portable_usize(16_000_000_000_000_000),
            max_associate_native_dense_workspace_entries: portable_usize(64_000_000_000),
            max_associate_native_heap_workspace_pair_bound: portable_usize(64_000_000_000),
            max_associate_native_workspace_byte_envelope: portable_usize(64_000_000_000_000),
            max_associate_rustred_visible_temporary_byte_envelope: portable_usize(
                64_000_000_000_000,
            ),
            max_associate_combined_temporary_byte_envelope: portable_usize(128_000_000_000_000),
            max_base_associate_validation_terms: portable_usize(64_000_000_000),
            max_base_associate_validation_exponent_entries: portable_usize(256_000_000_000),
            max_base_associate_validation_integer_bits: portable_usize(64_000_000_000_000_000),
            max_base_associate_source_owned_bytes: portable_usize(64_000_000_000_000),
            max_base_associate_index_exponent_entries: portable_usize(256_000_000_000),
            max_base_associate_native_scale_calls: 2_000_000_000,
            max_base_associate_native_coefficient_multiplications: portable_usize(64_000_000_000),
            max_base_associate_native_integer_multiplication_bit_work_bound: portable_usize(
                16_000_000_000_000_000,
            ),
            max_base_associate_output_terms: portable_usize(64_000_000_000),
            max_base_associate_output_exponent_entries: portable_usize(256_000_000_000),
            max_base_associate_output_integer_bit_bound: portable_usize(64_000_000_000_000_000),
            max_base_associate_output_retained_byte_bound: portable_usize(64_000_000_000_000),
            max_base_associate_payload_comparison_terms: portable_usize(64_000_000_000),
            max_base_associate_payload_comparison_exponent_entries: portable_usize(256_000_000_000),
            max_base_associate_payload_comparison_integer_bit_bound: portable_usize(
                64_000_000_000_000_000,
            ),
            max_base_associate_native_workspace_byte_envelope: portable_usize(64_000_000_000_000),
            max_base_associate_rustred_visible_temporary_byte_envelope: portable_usize(
                64_000_000_000_000,
            ),
            max_base_associate_combined_temporary_byte_envelope: portable_usize(
                128_000_000_000_000,
            ),
            max_retained_polynomial_terms: 2_000_000_000,
            max_retained_polynomial_exponent_entries: portable_usize(64_000_000_000),
            max_retained_polynomial_integer_bits: portable_usize(16_000_000_000_000_000),
            max_retained_polynomial_display_bytes: portable_usize(8 * 1024 * 1024 * 1024),
            max_retained_polynomial_owned_bytes: portable_usize(16 * 1024 * 1024 * 1024),
            max_retained_bytes: portable_usize(32 * 1024 * 1024 * 1024),
            max_final_invariant_entries: 1_000_000_000,
        }
    }
}

/// Exact aggregate work and retained-payload census.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineConditionAccumulatorStats {
    context_fingerprint_bytes: usize,
    context_fingerprint_comparison_bytes: usize,
    variable_map_entry_comparisons: usize,
    shared_allocation_identity_comparisons: usize,
    ambient_variables: usize,
    free_positions: usize,
    free_position_membership_entries: usize,
    condition_inputs: usize,
    source_inputs: usize,
    inherited_inputs: usize,
    candidate_inputs: usize,
    condition_sources: usize,
    discharged_nonzero_constants: usize,
    identically_zero_candidate_inputs: usize,
    unique_rows: usize,
    unique_inherited_rows: usize,
    unique_candidate_rows: usize,
    unique_base_rows: usize,
    unique_index_dependent_rows: usize,
    source_shift_components: usize,
    input_polynomial_terms: usize,
    input_polynomial_exponent_entries: usize,
    input_polynomial_integer_bits: usize,
    dependency_exponent_entries: usize,
    equality_comparisons: usize,
    equality_term_units: usize,
    equality_exponent_entries: usize,
    equality_integer_bits: usize,
    associate_checks: usize,
    associate_term_units: usize,
    associate_exponent_entries: usize,
    associate_integer_bits: usize,
    associate_validation_terms: usize,
    associate_validation_exponent_entries: usize,
    associate_validation_integer_bits: usize,
    associate_projection_exponent_entries: usize,
    associate_projection_coefficient_capacity_bytes: usize,
    associate_projection_group_bound: usize,
    associate_projection_variable_mask_comparison_bound: usize,
    associate_projection_hash_key_exponent_entry_bound: usize,
    associate_projection_coefficient_append_comparison_bound: usize,
    associate_projection_sorted_insert_comparison_bound: usize,
    associate_projection_sorted_insert_move_exponent_entry_bound: usize,
    associate_index_groups: usize,
    associate_index_support_comparison_entries: usize,
    associate_anchor_cost_operations: usize,
    associate_native_cross_term_pairs: usize,
    associate_peak_native_cross_term_pairs: usize,
    associate_native_base_exponent_additions: usize,
    associate_native_metadata_exponent_entry_inspection_bound: usize,
    associate_native_metadata_integer_entry_inspection_bound: usize,
    associate_native_integer_multiplication_bit_work_bound: usize,
    associate_native_integer_collection_bit_work_bound: usize,
    associate_native_output_term_bound: usize,
    associate_native_output_exponent_entry_bound: usize,
    associate_native_output_integer_bit_bound: usize,
    associate_native_dense_workspace_entries: usize,
    associate_native_heap_workspace_pair_bound: usize,
    associate_native_workspace_byte_envelope: usize,
    associate_rustred_visible_temporary_byte_envelope: usize,
    associate_combined_temporary_byte_envelope: usize,
    base_associate_validation_terms: usize,
    base_associate_validation_exponent_entries: usize,
    base_associate_validation_integer_bits: usize,
    base_associate_source_owned_bytes: usize,
    base_associate_index_exponent_entries: usize,
    base_associate_native_scale_calls: usize,
    base_associate_native_coefficient_multiplications: usize,
    base_associate_native_integer_multiplication_bit_work_bound: usize,
    base_associate_output_terms: usize,
    base_associate_output_exponent_entries: usize,
    base_associate_output_integer_bit_bound: usize,
    base_associate_output_retained_byte_bound: usize,
    base_associate_payload_comparison_terms: usize,
    base_associate_payload_comparison_exponent_entries: usize,
    base_associate_payload_comparison_integer_bit_bound: usize,
    base_associate_native_workspace_byte_envelope: usize,
    base_associate_rustred_visible_temporary_byte_envelope: usize,
    base_associate_combined_temporary_byte_envelope: usize,
    retained_polynomial_terms: usize,
    retained_polynomial_exponent_entries: usize,
    retained_polynomial_integer_bits: usize,
    retained_polynomial_display_bytes: usize,
    retained_polynomial_owned_byte_envelope: usize,
    retained_polynomial_owned_bytes: usize,
    retained_shared_context_allocations: usize,
    retained_shared_context_allocation_bytes: usize,
    retained_shared_variable_map_allocations: usize,
    retained_shared_variable_map_allocation_bytes: usize,
    retained_byte_envelope: usize,
    retained_bytes: usize,
    final_invariant_entries: usize,
}

macro_rules! generated_affine_condition_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedResidualAffineConditionAccumulatorStats {
    generated_affine_condition_stats_getters!(
        context_fingerprint_bytes,
        context_fingerprint_comparison_bytes,
        variable_map_entry_comparisons,
        shared_allocation_identity_comparisons,
        ambient_variables,
        free_positions,
        free_position_membership_entries,
        condition_inputs,
        source_inputs,
        inherited_inputs,
        candidate_inputs,
        condition_sources,
        discharged_nonzero_constants,
        identically_zero_candidate_inputs,
        unique_rows,
        unique_inherited_rows,
        unique_candidate_rows,
        unique_base_rows,
        unique_index_dependent_rows,
        source_shift_components,
        input_polynomial_terms,
        input_polynomial_exponent_entries,
        input_polynomial_integer_bits,
        dependency_exponent_entries,
        equality_comparisons,
        equality_term_units,
        equality_exponent_entries,
        equality_integer_bits,
        associate_checks,
        associate_term_units,
        associate_exponent_entries,
        associate_integer_bits,
        associate_validation_terms,
        associate_validation_exponent_entries,
        associate_validation_integer_bits,
        associate_projection_exponent_entries,
        associate_projection_coefficient_capacity_bytes,
        associate_projection_group_bound,
        associate_projection_variable_mask_comparison_bound,
        associate_projection_hash_key_exponent_entry_bound,
        associate_projection_coefficient_append_comparison_bound,
        associate_projection_sorted_insert_comparison_bound,
        associate_projection_sorted_insert_move_exponent_entry_bound,
        associate_index_groups,
        associate_index_support_comparison_entries,
        associate_anchor_cost_operations,
        associate_native_cross_term_pairs,
        associate_peak_native_cross_term_pairs,
        associate_native_base_exponent_additions,
        associate_native_metadata_exponent_entry_inspection_bound,
        associate_native_metadata_integer_entry_inspection_bound,
        associate_native_integer_multiplication_bit_work_bound,
        associate_native_integer_collection_bit_work_bound,
        associate_native_output_term_bound,
        associate_native_output_exponent_entry_bound,
        associate_native_output_integer_bit_bound,
        associate_native_dense_workspace_entries,
        associate_native_heap_workspace_pair_bound,
        associate_native_workspace_byte_envelope,
        associate_rustred_visible_temporary_byte_envelope,
        associate_combined_temporary_byte_envelope,
        base_associate_validation_terms,
        base_associate_validation_exponent_entries,
        base_associate_validation_integer_bits,
        base_associate_source_owned_bytes,
        base_associate_index_exponent_entries,
        base_associate_native_scale_calls,
        base_associate_native_coefficient_multiplications,
        base_associate_native_integer_multiplication_bit_work_bound,
        base_associate_output_terms,
        base_associate_output_exponent_entries,
        base_associate_output_integer_bit_bound,
        base_associate_output_retained_byte_bound,
        base_associate_payload_comparison_terms,
        base_associate_payload_comparison_exponent_entries,
        base_associate_payload_comparison_integer_bit_bound,
        base_associate_native_workspace_byte_envelope,
        base_associate_rustred_visible_temporary_byte_envelope,
        base_associate_combined_temporary_byte_envelope,
        retained_polynomial_terms,
        retained_polynomial_exponent_entries,
        retained_polynomial_integer_bits,
        retained_polynomial_display_bytes,
        retained_polynomial_owned_byte_envelope,
        retained_polynomial_owned_bytes,
        retained_shared_context_allocations,
        retained_shared_context_allocation_bytes,
        retained_shared_variable_map_allocations,
        retained_shared_variable_map_allocation_bytes,
        retained_byte_envelope,
        retained_bytes,
        final_invariant_entries,
    );
}

/// Redacted classification retained for every input, including constants and
/// identically-zero candidate predicates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedResidualAffineConditionInputClass {
    DischargedNonzeroIntegerConstant,
    IdenticallyZeroCandidate,
    BaseAssumption { row_ordinal: usize },
    IndexDependent { row_ordinal: usize },
}

/// Private source payload.  Its custom formatter intentionally does not print
/// the exact shift.
#[derive(PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineConditionSourcePayload {
    locator: GeneratedResidualAffineConditionSourceLocator,
    private_shift: Option<IndexShift>,
}

impl GeneratedResidualAffineConditionSourcePayload {
    pub(crate) const fn locator(&self) -> GeneratedResidualAffineConditionSourceLocator {
        self.locator
    }

    pub(crate) const fn private_shift(&self) -> Option<&IndexShift> {
        self.private_shift.as_ref()
    }
}

impl fmt::Debug for GeneratedResidualAffineConditionSourcePayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineConditionSourcePayload")
            .field("locator", &self.locator)
            .field(
                "private_shift",
                &self.private_shift.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

/// One durable input record in original encounter order.
#[derive(PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineConditionInputTranscript {
    ordinal: usize,
    scope: GeneratedResidualAffineConditionScope,
    source: GeneratedResidualAffineConditionSourcePayload,
    class: GeneratedResidualAffineConditionInputClass,
}

impl GeneratedResidualAffineConditionInputTranscript {
    pub(crate) const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub(crate) const fn scope(&self) -> GeneratedResidualAffineConditionScope {
        self.scope
    }

    pub(crate) const fn source(&self) -> &GeneratedResidualAffineConditionSourcePayload {
        &self.source
    }

    pub(crate) const fn class(&self) -> GeneratedResidualAffineConditionInputClass {
        self.class
    }

    pub(crate) const fn view(&self) -> GeneratedResidualAffineConditionInputView {
        GeneratedResidualAffineConditionInputView {
            ordinal: self.ordinal,
            scope: self.scope,
            source: self.source.locator,
            class: self.class,
        }
    }
}

impl fmt::Debug for GeneratedResidualAffineConditionInputTranscript {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineConditionInputTranscript")
            .field("ordinal", &self.ordinal)
            .field("scope", &self.scope)
            .field("source", &self.source)
            .field("class", &self.class)
            .finish()
    }
}

/// Allocation-free public-style input projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineConditionInputView {
    ordinal: usize,
    scope: GeneratedResidualAffineConditionScope,
    source: GeneratedResidualAffineConditionSourceLocator,
    class: GeneratedResidualAffineConditionInputClass,
}

impl GeneratedResidualAffineConditionInputView {
    pub(crate) const fn ordinal(self) -> usize {
        self.ordinal
    }

    pub(crate) const fn scope(self) -> GeneratedResidualAffineConditionScope {
        self.scope
    }

    pub(crate) const fn source(self) -> GeneratedResidualAffineConditionSourceLocator {
        self.source
    }

    pub(crate) const fn class(self) -> GeneratedResidualAffineConditionInputClass {
        self.class
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PolynomialCensus {
    terms: usize,
    exponent_entries: usize,
    integer_bits: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SharedAllocationCensus {
    context_allocations: usize,
    context_allocation_bytes: usize,
    variable_map_allocations: usize,
    variable_map_allocation_bytes: usize,
}

impl SharedAllocationCensus {
    fn total_bytes(self) -> Result<usize, GeneratedResidualAffineConditionAccumulatorError> {
        checked_add(
            "affine condition retained shared-allocation bytes",
            self.context_allocation_bytes,
            self.variable_map_allocation_bytes,
        )
    }

    fn checked_add(
        self,
        other: Self,
    ) -> Result<Self, GeneratedResidualAffineConditionAccumulatorError> {
        Ok(Self {
            context_allocations: checked_add(
                "affine condition retained shared-context allocations",
                self.context_allocations,
                other.context_allocations,
            )?,
            context_allocation_bytes: checked_add(
                "affine condition retained shared-context allocation bytes",
                self.context_allocation_bytes,
                other.context_allocation_bytes,
            )?,
            variable_map_allocations: checked_add(
                "affine condition retained shared variable-map allocations",
                self.variable_map_allocations,
                other.variable_map_allocations,
            )?,
            variable_map_allocation_bytes: checked_add(
                "affine condition retained shared variable-map allocation bytes",
                self.variable_map_allocation_bytes,
                other.variable_map_allocation_bytes,
            )?,
        })
    }
}

/// One first-representative-wins canonical condition row.
#[derive(PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineCanonicalConditionRow {
    polynomial: ParametricPolynomial,
    source_input_ordinals: Vec<usize>,
    scope: GeneratedResidualAffineConditionScope,
    index_dependent: bool,
    census: PolynomialCensus,
}

impl GeneratedResidualAffineCanonicalConditionRow {
    pub(crate) const fn polynomial(&self) -> &ParametricPolynomial {
        &self.polynomial
    }

    pub(crate) fn source_input_ordinals(&self) -> &[usize] {
        &self.source_input_ordinals
    }

    pub(crate) const fn scope(&self) -> GeneratedResidualAffineConditionScope {
        self.scope
    }

    pub(crate) const fn is_index_dependent(&self) -> bool {
        self.index_dependent
    }

    pub(crate) fn view(&self, ordinal: usize) -> GeneratedResidualAffineCanonicalConditionView {
        GeneratedResidualAffineCanonicalConditionView {
            ordinal,
            scope: self.scope,
            index_dependent: self.index_dependent,
            source_count: self.source_input_ordinals.len(),
        }
    }
}

impl fmt::Debug for GeneratedResidualAffineCanonicalConditionRow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineCanonicalConditionRow")
            .field("polynomial", &"<redacted>")
            .field("source_input_ordinals", &self.source_input_ordinals)
            .field("scope", &self.scope)
            .field("index_dependent", &self.index_dependent)
            .finish()
    }
}

/// Allocation-free row projection which contains neither polynomial nor
/// private source shift.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineCanonicalConditionView {
    ordinal: usize,
    scope: GeneratedResidualAffineConditionScope,
    index_dependent: bool,
    source_count: usize,
}

impl GeneratedResidualAffineCanonicalConditionView {
    pub(crate) const fn ordinal(self) -> usize {
        self.ordinal
    }

    pub(crate) const fn scope(self) -> GeneratedResidualAffineConditionScope {
        self.scope
    }

    pub(crate) const fn is_index_dependent(self) -> bool {
        self.index_dependent
    }

    pub(crate) const fn source_count(self) -> usize {
        self.source_count
    }
}

/// Complete source-neutral condition transcript and canonical table.
#[derive(PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineConditionAccumulatorCertificate {
    context_fingerprint: String,
    free_positions: Vec<usize>,
    free_position_membership: Vec<u8>,
    inputs: Vec<GeneratedResidualAffineConditionInputTranscript>,
    rows: Vec<GeneratedResidualAffineCanonicalConditionRow>,
    candidate_is_identically_bad: bool,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: GeneratedResidualAffineConditionAccumulatorStats,
}

impl GeneratedResidualAffineConditionAccumulatorCertificate {
    pub(crate) fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub(crate) fn free_positions(&self) -> &[usize] {
        &self.free_positions
    }

    pub(crate) fn inputs(&self) -> &[GeneratedResidualAffineConditionInputTranscript] {
        &self.inputs
    }

    pub(crate) fn rows(&self) -> &[GeneratedResidualAffineCanonicalConditionRow] {
        &self.rows
    }

    pub(crate) const fn candidate_is_identically_bad(&self) -> bool {
        self.candidate_is_identically_bad
    }

    pub(crate) const fn limits(&self) -> GeneratedResidualAffineConditionAccumulatorLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> GeneratedResidualAffineConditionAccumulatorStats {
        self.stats
    }

    pub(crate) fn input_view(
        &self,
        ordinal: usize,
    ) -> Option<GeneratedResidualAffineConditionInputView> {
        self.inputs.get(ordinal).map(|input| input.view())
    }

    pub(crate) fn row_view(
        &self,
        ordinal: usize,
    ) -> Option<GeneratedResidualAffineCanonicalConditionView> {
        self.rows.get(ordinal).map(|row| row.view(ordinal))
    }
}

impl fmt::Debug for GeneratedResidualAffineConditionAccumulatorCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineConditionAccumulatorCertificate")
            .field("context_fingerprint", &"<redacted>")
            .field("free_position_count", &self.free_positions.len())
            .field("input_count", &self.inputs.len())
            .field("row_count", &self.rows.len())
            .field(
                "candidate_is_identically_bad",
                &self.candidate_is_identically_bad,
            )
            .field("limits", &self.limits)
            .field("stats", &self.stats)
            .finish()
    }
}

/// Typed accumulator failures.  None of these variants owns or formats a raw
/// polynomial or private shift.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedResidualAffineConditionAccumulatorError {
    ConfiguredExponentLimit {
        requested: u128,
        representation_limit: u128,
    },
    FreePositionOutOfRange {
        position: usize,
        index_count: usize,
    },
    NonIncreasingFreePositions {
        previous: usize,
        current: usize,
    },
    MissingPrivateShift {
        input_ordinal: usize,
    },
    UnexpectedPrivateShift {
        input_ordinal: usize,
    },
    SourceScopeMismatch {
        input_ordinal: usize,
    },
    WrongPrivateShiftArity {
        input_ordinal: usize,
        expected: usize,
        actual: usize,
    },
    InheritedConditionIsIdenticallyZero {
        input_ordinal: usize,
    },
    NonfreePrivateIndexSupport {
        input_ordinal: usize,
        position: usize,
    },
    AssociateDependencyMismatch {
        input_ordinal: usize,
        row_ordinal: usize,
    },
    InternalInvariant {
        resource: &'static str,
    },
    RetainedPolynomialByteEnvelopeExceeded {
        observed: usize,
        admitted: usize,
    },
    RetainedByteEnvelopeExceeded {
        observed: usize,
        admitted: usize,
    },
    RetainedByteCensusMismatch,
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
    ParametricCoefficient(ParametricCoefficientError),
}

impl fmt::Display for GeneratedResidualAffineConditionAccumulatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfiguredExponentLimit {
                requested,
                representation_limit,
            } => write!(
                formatter,
                "configured exponent limit {requested} exceeds the Symbolica representation limit {representation_limit}"
            ),
            Self::FreePositionOutOfRange {
                position,
                index_count,
            } => write!(
                formatter,
                "free private-index position {position} is outside arity {index_count}"
            ),
            Self::NonIncreasingFreePositions { previous, current } => write!(
                formatter,
                "free private-index positions are not strictly increasing at {previous}, {current}"
            ),
            Self::MissingPrivateShift { input_ordinal } => write!(
                formatter,
                "condition input {input_ordinal} is a denominator source without a private shift"
            ),
            Self::UnexpectedPrivateShift { input_ordinal } => write!(
                formatter,
                "condition input {input_ordinal} has a private shift for a non-denominator source"
            ),
            Self::SourceScopeMismatch { input_ordinal } => write!(
                formatter,
                "condition input {input_ordinal} has a source/scope mismatch"
            ),
            Self::WrongPrivateShiftArity {
                input_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "condition input {input_ordinal} has private-shift arity {actual}, expected {expected}"
            ),
            Self::InheritedConditionIsIdenticallyZero { input_ordinal } => write!(
                formatter,
                "inherited condition input {input_ordinal} is identically zero"
            ),
            Self::NonfreePrivateIndexSupport {
                input_ordinal,
                position,
            } => write!(
                formatter,
                "condition input {input_ordinal} depends on nonfree private-index position {position}"
            ),
            Self::AssociateDependencyMismatch {
                input_ordinal,
                row_ordinal,
            } => write!(
                formatter,
                "condition input {input_ordinal} and associated row {row_ordinal} disagree on private-index dependency"
            ),
            Self::InternalInvariant { resource } => {
                write!(
                    formatter,
                    "condition accumulator invariant failed for {resource}"
                )
            }
            Self::RetainedPolynomialByteEnvelopeExceeded { observed, admitted } => write!(
                formatter,
                "retained polynomial owns {observed} bytes after admitting {admitted}"
            ),
            Self::RetainedByteEnvelopeExceeded { observed, admitted } => write!(
                formatter,
                "condition certificate owns {observed} bytes after admitting {admitted}"
            ),
            Self::RetainedByteCensusMismatch => {
                formatter.write_str("condition certificate retained-byte census mismatch")
            }
            Self::SymbolicaPanic { stage } => {
                write!(formatter, "Symbolica panicked during condition {stage}")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested} units, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "{resource} allocation of {requested} entries failed after bounded preflight"
            ),
            Self::ParametricCoefficient(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedResidualAffineConditionAccumulatorError {}

impl From<ParametricCoefficientError> for GeneratedResidualAffineConditionAccumulatorError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::ParametricCoefficient(value)
    }
}

/// Canonicalize one already-authenticated, source-ordered condition stream.
pub(crate) fn accumulate_generated_residual_affine_conditions<'a>(
    context: &ParametricCoefficientContext,
    free_positions: &[usize],
    inputs: impl IntoIterator<Item = GeneratedResidualAffineConditionInput<'a>>,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
) -> Result<
    GeneratedResidualAffineConditionAccumulatorCertificate,
    GeneratedResidualAffineConditionAccumulatorError,
> {
    catch_unwind(AssertUnwindSafe(|| {
        accumulate_generated_residual_affine_conditions_inner(
            context,
            free_positions,
            inputs,
            limits,
        )
    }))
    .map_err(
        |_| GeneratedResidualAffineConditionAccumulatorError::SymbolicaPanic {
            stage: "accumulation",
        },
    )?
}

fn accumulate_generated_residual_affine_conditions_inner<'a>(
    context: &ParametricCoefficientContext,
    free_positions: &[usize],
    inputs: impl IntoIterator<Item = GeneratedResidualAffineConditionInput<'a>>,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
) -> Result<
    GeneratedResidualAffineConditionAccumulatorCertificate,
    GeneratedResidualAffineConditionAccumulatorError,
> {
    if limits.exact_algebra.max_exponent > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
        return Err(
            GeneratedResidualAffineConditionAccumulatorError::ConfiguredExponentLimit {
                requested: limits.exact_algebra.max_exponent,
                representation_limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            },
        );
    }

    let mut stats = GeneratedResidualAffineConditionAccumulatorStats::default();
    stats.ambient_variables = checked_add(
        "affine condition ambient variables",
        context.base().variables().len(),
        context.index_count(),
    )?;
    check_limit(
        "affine condition ambient variables",
        stats.ambient_variables,
        limits.max_ambient_variables,
    )?;
    stats.retained_byte_envelope =
        size_of::<GeneratedResidualAffineConditionAccumulatorCertificate>();
    check_limit(
        "affine condition retained bytes",
        stats.retained_byte_envelope,
        limits.max_retained_bytes,
    )?;
    stats.retained_bytes = stats.retained_byte_envelope;
    stats.context_fingerprint_bytes = context.fingerprint().len();
    check_limit(
        "affine condition context fingerprint bytes",
        stats.context_fingerprint_bytes,
        limits.max_context_fingerprint_bytes,
    )?;
    admit_retained_bytes(
        capacity_byte_envelope(context.fingerprint().len(), size_of::<u8>())?,
        limits,
        &mut stats,
    )?;
    let context_fingerprint = try_copy_string(
        context.fingerprint(),
        "affine condition context fingerprint",
    )?;
    observe_retained_bytes(
        checked_mul(
            "affine condition retained bytes",
            context_fingerprint.capacity(),
            size_of::<u8>(),
        )?,
        &mut stats,
    )?;

    validate_free_positions(context.index_count(), free_positions, limits, &mut stats)?;
    admit_retained_bytes(
        capacity_byte_envelope(free_positions.len(), size_of::<usize>())?,
        limits,
        &mut stats,
    )?;
    let free_positions = try_copy_usize_slice(free_positions, "affine condition free positions")?;
    observe_retained_bytes(
        checked_mul(
            "affine condition retained bytes",
            free_positions.capacity(),
            size_of::<usize>(),
        )?,
        &mut stats,
    )?;
    let free_position_membership = try_build_free_position_membership(
        context.index_count(),
        &free_positions,
        limits,
        &mut stats,
    )?;

    let mut transcript = Vec::new();
    let mut rows = Vec::new();
    let mut candidate_is_identically_bad = false;

    for input in inputs {
        let input_ordinal = stats.condition_inputs;
        stats.condition_inputs = bounded_add(
            "affine condition inputs",
            stats.condition_inputs,
            1,
            limits.max_condition_inputs,
        )?;
        stats.source_inputs = bounded_add(
            "affine condition source inputs",
            stats.source_inputs,
            1,
            limits.max_source_inputs,
        )?;
        match input.scope {
            GeneratedResidualAffineConditionScope::InheritedTargetPremise => {
                stats.inherited_inputs = checked_add(
                    "affine inherited condition inputs",
                    stats.inherited_inputs,
                    1,
                )?;
            }
            GeneratedResidualAffineConditionScope::CandidateRequired => {
                stats.candidate_inputs = checked_add(
                    "affine candidate condition inputs",
                    stats.candidate_inputs,
                    1,
                )?;
            }
        }

        validate_source_scope(input_ordinal, &input)?;
        validate_source_shift(context.index_count(), input_ordinal, &input)?;
        let shift_components = input.private_shift.map_or(0, IndexShift::arity);
        stats.source_shift_components = bounded_add(
            "affine condition source shift components",
            stats.source_shift_components,
            shift_components,
            limits.max_source_shift_components,
        )?;

        charge_context_comparison(
            input.polynomial.authenticated_context_fingerprint(),
            context.fingerprint(),
            limits,
            &mut stats,
        )?;
        charge_variable_map_comparisons(stats.ambient_variables, limits, &mut stats)?;
        let source_census = context.preflight_polynomial_validation_payload_with_limits(
            input.polynomial,
            limits.exact_algebra,
            remaining(
                "affine condition input polynomial terms",
                limits.max_input_polynomial_terms,
                stats.input_polynomial_terms,
            )?,
            remaining(
                "affine condition input polynomial exponent entries",
                limits.max_input_polynomial_exponent_entries,
                stats.input_polynomial_exponent_entries,
            )?,
            remaining(
                "affine condition input polynomial integer bits",
                limits.max_input_polynomial_integer_bits,
                stats.input_polynomial_integer_bits,
            )?,
        )?;
        stats.input_polynomial_terms = checked_add(
            "affine condition input polynomial terms",
            stats.input_polynomial_terms,
            source_census.source_terms(),
        )?;
        stats.input_polynomial_exponent_entries = checked_add(
            "affine condition input polynomial exponent entries",
            stats.input_polynomial_exponent_entries,
            source_census.source_exponent_entries(),
        )?;
        stats.input_polynomial_integer_bits = checked_add(
            "affine condition input polynomial integer bits",
            stats.input_polynomial_integer_bits,
            source_census.source_integer_bits(),
        )?;

        let census = PolynomialCensus {
            terms: source_census.source_terms(),
            exponent_entries: source_census.source_exponent_entries(),
            integer_bits: source_census.source_integer_bits(),
        };
        let index_dependent = scan_private_index_support(
            context,
            input.polynomial,
            &free_position_membership,
            input_ordinal,
            limits,
            &mut stats,
        )?;

        let class = if input.polynomial.is_zero() {
            if input.scope == GeneratedResidualAffineConditionScope::InheritedTargetPremise {
                return Err(
                    GeneratedResidualAffineConditionAccumulatorError::InheritedConditionIsIdenticallyZero {
                        input_ordinal,
                    },
                );
            }
            candidate_is_identically_bad = true;
            stats.identically_zero_candidate_inputs = checked_add(
                "identically-zero candidate condition inputs",
                stats.identically_zero_candidate_inputs,
                1,
            )?;
            GeneratedResidualAffineConditionInputClass::IdenticallyZeroCandidate
        } else if input.polynomial.is_nonzero_constant() {
            stats.discharged_nonzero_constants = checked_add(
                "discharged nonzero condition constants",
                stats.discharged_nonzero_constants,
                1,
            )?;
            GeneratedResidualAffineConditionInputClass::DischargedNonzeroIntegerConstant
        } else {
            let row_ordinal = canonical_row(
                context,
                input.polynomial,
                census,
                input.scope,
                index_dependent,
                input_ordinal,
                &mut rows,
                limits,
                &mut stats,
            )?;
            if index_dependent {
                GeneratedResidualAffineConditionInputClass::IndexDependent { row_ordinal }
            } else {
                GeneratedResidualAffineConditionInputClass::BaseAssumption { row_ordinal }
            }
        };

        if let Some(shift) = input.private_shift {
            admit_retained_bytes(
                capacity_byte_envelope(shift.arity(), size_of::<i64>())?,
                limits,
                &mut stats,
            )?;
        }
        let private_shift = input.private_shift.map(try_copy_shift).transpose()?;
        if let Some(shift) = &private_shift {
            observe_retained_bytes(
                shift.owned_retained_byte_bound().ok_or(
                    GeneratedResidualAffineConditionAccumulatorError::ResourceCountOverflow {
                        resource: "affine condition retained bytes",
                    },
                )?,
                &mut stats,
            )?;
        }
        let source = GeneratedResidualAffineConditionSourcePayload {
            locator: input.source,
            private_shift,
        };
        admit_retained_bytes(
            capacity_byte_envelope(
                1,
                size_of::<GeneratedResidualAffineConditionInputTranscript>(),
            )?,
            limits,
            &mut stats,
        )?;
        try_push_observed(
            "affine condition input transcript",
            &mut transcript,
            GeneratedResidualAffineConditionInputTranscript {
                ordinal: input_ordinal,
                scope: input.scope,
                source,
                class,
            },
            &mut stats,
        )?;
    }

    // All candidate-to-inherited promotions are now complete.  Enforce the
    // maintained final category immediately, then independently reconstruct
    // and check the same category inside final replay.
    check_limit(
        "unique candidate affine condition rows",
        stats.unique_candidate_rows,
        limits.max_unique_candidate_rows,
    )?;
    validate_final_invariants(
        context,
        &context_fingerprint,
        &free_positions,
        &free_position_membership,
        &transcript,
        &rows,
        candidate_is_identically_bad,
        limits,
        &mut stats,
    )?;
    let recomputed_shared_allocations =
        recompute_shared_allocation_census(&rows, limits, &mut stats)?;
    if recomputed_shared_allocations.context_allocations
        != stats.retained_shared_context_allocations
        || recomputed_shared_allocations.context_allocation_bytes
            != stats.retained_shared_context_allocation_bytes
        || recomputed_shared_allocations.variable_map_allocations
            != stats.retained_shared_variable_map_allocations
        || recomputed_shared_allocations.variable_map_allocation_bytes
            != stats.retained_shared_variable_map_allocation_bytes
    {
        return Err(
            GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                resource: "retained shared-allocation census",
            },
        );
    }
    let recomputed_envelope = recompute_retained_byte_envelope(
        &context_fingerprint,
        &free_positions,
        &free_position_membership,
        &transcript,
        &rows,
        recomputed_shared_allocations,
    )?;
    let recomputed_observed = recompute_observed_retained_bytes(
        &context_fingerprint,
        &free_positions,
        &free_position_membership,
        &transcript,
        &rows,
        recomputed_shared_allocations,
    )?;
    if recomputed_envelope != stats.retained_byte_envelope
        || recomputed_observed != stats.retained_bytes
    {
        return Err(GeneratedResidualAffineConditionAccumulatorError::RetainedByteCensusMismatch);
    }
    if recomputed_observed > recomputed_envelope {
        return Err(
            GeneratedResidualAffineConditionAccumulatorError::RetainedByteEnvelopeExceeded {
                observed: recomputed_observed,
                admitted: recomputed_envelope,
            },
        );
    }

    Ok(GeneratedResidualAffineConditionAccumulatorCertificate {
        context_fingerprint,
        free_positions,
        free_position_membership,
        inputs: transcript,
        rows,
        candidate_is_identically_bad,
        limits,
        stats,
    })
}

#[allow(clippy::too_many_arguments)]
fn canonical_row(
    context: &ParametricCoefficientContext,
    polynomial: &ParametricPolynomial,
    census: PolynomialCensus,
    scope: GeneratedResidualAffineConditionScope,
    index_dependent: bool,
    input_ordinal: usize,
    rows: &mut Vec<GeneratedResidualAffineCanonicalConditionRow>,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<usize, GeneratedResidualAffineConditionAccumulatorError> {
    // Exact equality is tested against every retained row before *any*
    // coefficient-field associate proof is attempted.
    let mut exact_match = None;
    for (row_ordinal, row) in rows.iter().enumerate() {
        charge_context_comparison(
            polynomial.authenticated_context_fingerprint(),
            row.polynomial.authenticated_context_fingerprint(),
            limits,
            stats,
        )?;
        charge_variable_map_comparisons(polynomial.raw().variables.len(), limits, stats)?;
        charge_equality_comparison(census, row.census, limits, stats)?;
        if &row.polynomial == polynomial {
            if exact_match.replace(row_ordinal).is_some() {
                return Err(
                    GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                        resource: "duplicate exact canonical rows",
                    },
                );
            }
        }
    }

    let matched = if exact_match.is_some() {
        exact_match
    } else {
        let mut associated = None;
        for (row_ordinal, row) in rows.iter().enumerate() {
            // Different dependency classes cannot be associates in either
            // admissible unit group. Skip them before charging or invoking a
            // child proof; in particular, never send an index row through the
            // base-only `Q*` boundary.
            if row.index_dependent != index_dependent {
                continue;
            }
            let is_associated = if index_dependent {
                let call_limits =
                    precharge_associate_comparison(census, row.census, limits, stats)?;
                let result = context.polynomial_loci_are_associates_with_census(
                    &row.polynomial,
                    polynomial,
                    call_limits,
                )?;
                consume_associate_stats(result.stats(), limits, stats)?;
                result.associated()
            } else {
                let call_limits =
                    precharge_base_associate_comparison(census, row.census, limits, stats)?;
                let result = context.base_polynomial_loci_are_rational_associates_with_census(
                    &row.polynomial,
                    polynomial,
                    call_limits,
                )?;
                consume_base_associate_stats(result.stats(), limits, stats)?;
                result.associated()
            };
            if is_associated {
                associated = Some(row_ordinal);
                break;
            }
        }
        associated
    };

    if let Some(row_ordinal) = matched {
        let row = rows.get_mut(row_ordinal).ok_or(
            GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                resource: "matched canonical row ordinal",
            },
        )?;
        if row.index_dependent != index_dependent {
            return Err(
                GeneratedResidualAffineConditionAccumulatorError::AssociateDependencyMismatch {
                    input_ordinal,
                    row_ordinal,
                },
            );
        }
        stats.condition_sources = bounded_add(
            "affine canonical condition sources",
            stats.condition_sources,
            1,
            limits.max_condition_sources,
        )?;
        admit_retained_bytes(
            capacity_byte_envelope(1, size_of::<usize>())?,
            limits,
            stats,
        )?;
        try_push_observed(
            "affine canonical condition source ordinals",
            &mut row.source_input_ordinals,
            input_ordinal,
            stats,
        )?;
        if scope == GeneratedResidualAffineConditionScope::InheritedTargetPremise
            && row.scope == GeneratedResidualAffineConditionScope::CandidateRequired
        {
            let promoted = stats.unique_inherited_rows.checked_add(1).ok_or(
                GeneratedResidualAffineConditionAccumulatorError::ResourceCountOverflow {
                    resource: "unique inherited affine condition rows",
                },
            )?;
            check_limit(
                "unique inherited affine condition rows",
                promoted,
                limits.max_unique_inherited_rows,
            )?;
            stats.unique_candidate_rows = stats.unique_candidate_rows.checked_sub(1).ok_or(
                GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                    resource: "candidate-row promotion",
                },
            )?;
            stats.unique_inherited_rows = promoted;
            row.scope = GeneratedResidualAffineConditionScope::InheritedTargetPremise;
        }
        return Ok(row_ordinal);
    }

    let row_ordinal = rows.len();
    let unique_rows = bounded_add(
        "unique affine condition rows",
        stats.unique_rows,
        1,
        limits.max_unique_rows,
    )?;
    match scope {
        GeneratedResidualAffineConditionScope::InheritedTargetPremise => {
            stats.unique_inherited_rows = bounded_add(
                "unique inherited affine condition rows",
                stats.unique_inherited_rows,
                1,
                limits.max_unique_inherited_rows,
            )?;
        }
        GeneratedResidualAffineConditionScope::CandidateRequired => {
            // Candidate rows may later be promoted when an inherited source
            // reaches the same canonical locus.  The total-row limit bounds
            // streaming retention; the candidate-category limit is checked
            // replay-exactly after every promotion has been resolved.
            stats.unique_candidate_rows = checked_add(
                "unique candidate affine condition rows",
                stats.unique_candidate_rows,
                1,
            )?;
        }
    }
    if index_dependent {
        stats.unique_index_dependent_rows = checked_add(
            "unique index-dependent affine condition rows",
            stats.unique_index_dependent_rows,
            1,
        )?;
    } else {
        stats.unique_base_rows = checked_add(
            "unique base affine condition rows",
            stats.unique_base_rows,
            1,
        )?;
    }

    let condition_sources = bounded_add(
        "affine canonical condition sources",
        stats.condition_sources,
        1,
        limits.max_condition_sources,
    )?;
    let shared_allocation_addition =
        shared_allocation_addition_for_polynomial(polynomial, rows, limits, stats)?;
    admit_retained_shared_allocations(shared_allocation_addition, limits, stats)?;
    admit_retained_bytes(
        capacity_byte_envelope(1, size_of::<GeneratedResidualAffineCanonicalConditionRow>())?,
        limits,
        stats,
    )?;
    admit_retained_bytes(
        capacity_byte_envelope(1, size_of::<usize>())?,
        limits,
        stats,
    )?;
    let admitted_owned_bytes = charge_retained_polynomial(polynomial, census, limits, stats)?;
    // The two pointer checks are admitted before the fallible copy they
    // authenticate.  No allocation may occur first and retroactively consume
    // identity-comparison budget.
    charge_shared_allocation_identity_comparisons(2, limits, stats)?;
    let copied = polynomial
        .try_copy_authenticated_sparse_payload()
        .map_err(|resource| {
            GeneratedResidualAffineConditionAccumulatorError::AllocationFailure {
                resource,
                requested: census.terms.max(census.exponent_entries),
            }
        })?;
    let copied_context_is_shared = std::ptr::eq(
        copied.authenticated_context_fingerprint(),
        polynomial.authenticated_context_fingerprint(),
    );
    let copied_map_is_shared = Arc::ptr_eq(&copied.raw().variables, &polynomial.raw().variables);
    if !copied_context_is_shared || !copied_map_is_shared {
        return Err(
            GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                resource: "retained polynomial shared allocations",
            },
        );
    }
    observe_retained_shared_allocations(shared_allocation_addition, stats)?;
    let observed = copied.owned_retained_byte_bound().ok_or(
        GeneratedResidualAffineConditionAccumulatorError::ResourceCountOverflow {
            resource: "retained affine condition polynomial owned bytes",
        },
    )?;
    if observed > admitted_owned_bytes {
        return Err(
            GeneratedResidualAffineConditionAccumulatorError::RetainedPolynomialByteEnvelopeExceeded {
                observed,
                admitted: admitted_owned_bytes,
            },
        );
    }
    stats.retained_polynomial_owned_bytes = checked_add(
        "retained affine condition polynomial owned bytes",
        stats.retained_polynomial_owned_bytes,
        observed,
    )?;
    if stats.retained_polynomial_owned_bytes > stats.retained_polynomial_owned_byte_envelope {
        return Err(
            GeneratedResidualAffineConditionAccumulatorError::RetainedPolynomialByteEnvelopeExceeded {
                observed: stats.retained_polynomial_owned_bytes,
                admitted: stats.retained_polynomial_owned_byte_envelope,
            },
        );
    }
    observe_retained_bytes(
        observed
            .checked_sub(size_of::<ParametricPolynomial>())
            .ok_or(
                GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                    resource: "retained polynomial inline bytes",
                },
            )?,
        stats,
    )?;

    let mut source_input_ordinals = Vec::new();
    try_push_observed(
        "affine canonical condition source ordinals",
        &mut source_input_ordinals,
        input_ordinal,
        stats,
    )?;
    try_push_observed(
        "affine canonical condition rows",
        rows,
        GeneratedResidualAffineCanonicalConditionRow {
            polynomial: copied,
            source_input_ordinals,
            scope,
            index_dependent,
            census,
        },
        stats,
    )?;
    stats.condition_sources = condition_sources;
    stats.unique_rows = unique_rows;
    Ok(row_ordinal)
}

fn validate_free_positions(
    index_count: usize,
    free_positions: &[usize],
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    check_limit(
        "affine condition free positions",
        free_positions.len(),
        limits.max_free_positions,
    )?;
    for (ordinal, &position) in free_positions.iter().enumerate() {
        if position >= index_count {
            return Err(
                GeneratedResidualAffineConditionAccumulatorError::FreePositionOutOfRange {
                    position,
                    index_count,
                },
            );
        }
        if ordinal > 0 && free_positions[ordinal - 1] >= position {
            return Err(
                GeneratedResidualAffineConditionAccumulatorError::NonIncreasingFreePositions {
                    previous: free_positions[ordinal - 1],
                    current: position,
                },
            );
        }
    }
    stats.free_positions = free_positions.len();
    Ok(())
}

fn try_build_free_position_membership(
    index_count: usize,
    free_positions: &[usize],
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<Vec<u8>, GeneratedResidualAffineConditionAccumulatorError> {
    admit_retained_bytes(
        capacity_byte_envelope(index_count, size_of::<u8>())?,
        limits,
        stats,
    )?;
    let mut membership = Vec::new();
    try_reserve_exact(
        "affine condition free-position membership",
        &mut membership,
        index_count,
    )?;
    membership.resize(index_count, 0);
    for &position in free_positions {
        membership[position] = 1;
    }
    observe_retained_bytes(
        checked_mul(
            "affine condition retained bytes",
            membership.capacity(),
            size_of::<u8>(),
        )?,
        stats,
    )?;
    stats.free_position_membership_entries = membership.len();
    Ok(membership)
}

fn validate_source_shift(
    index_count: usize,
    input_ordinal: usize,
    input: &GeneratedResidualAffineConditionInput<'_>,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    let is_denominator = matches!(
        input.source,
        GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator { .. }
    );
    match (is_denominator, input.private_shift) {
        (true, None) => Err(
            GeneratedResidualAffineConditionAccumulatorError::MissingPrivateShift { input_ordinal },
        ),
        (false, Some(_)) => Err(
            GeneratedResidualAffineConditionAccumulatorError::UnexpectedPrivateShift {
                input_ordinal,
            },
        ),
        (_, Some(shift)) if shift.arity() != index_count => Err(
            GeneratedResidualAffineConditionAccumulatorError::WrongPrivateShiftArity {
                input_ordinal,
                expected: index_count,
                actual: shift.arity(),
            },
        ),
        _ => Ok(()),
    }
}

fn validate_source_scope(
    input_ordinal: usize,
    input: &GeneratedResidualAffineConditionInput<'_>,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    if source_scope_is_valid(input.scope, input.source) {
        Ok(())
    } else {
        Err(GeneratedResidualAffineConditionAccumulatorError::SourceScopeMismatch { input_ordinal })
    }
}

fn source_scope_is_valid(
    scope: GeneratedResidualAffineConditionScope,
    source: GeneratedResidualAffineConditionSourceLocator,
) -> bool {
    matches!(
        (scope, source),
        (
            GeneratedResidualAffineConditionScope::InheritedTargetPremise,
            GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard { .. }
                | GeneratedResidualAffineConditionSourceLocator::ExceptionalNonZeroPredicate { .. }
        ) | (
            GeneratedResidualAffineConditionScope::CandidateRequired,
            GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard { .. }
                | GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator { .. }
        )
    )
}

fn scan_private_index_support(
    context: &ParametricCoefficientContext,
    polynomial: &ParametricPolynomial,
    free_position_membership: &[u8],
    input_ordinal: usize,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<bool, GeneratedResidualAffineConditionAccumulatorError> {
    let raw = polynomial.raw();
    let base_count = context.base().variables().len();
    let index_count = context.index_count();
    stats.dependency_exponent_entries = bounded_product_add(
        "affine condition dependency exponent entries",
        stats.dependency_exponent_entries,
        polynomial.term_count(),
        index_count,
        limits.max_dependency_exponent_entries,
    )?;

    let variable_count = raw.variables.len();
    let mut index_dependent = false;
    for exponents in raw.exponents.chunks_exact(variable_count) {
        for (position, &exponent) in exponents[base_count..].iter().enumerate() {
            if exponent != 0 {
                index_dependent = true;
                if free_position_membership[position] == 0 {
                    return Err(
                        GeneratedResidualAffineConditionAccumulatorError::NonfreePrivateIndexSupport {
                            input_ordinal,
                            position,
                        },
                    );
                }
            }
        }
    }
    Ok(index_dependent)
}

fn charge_equality_comparison(
    left: PolynomialCensus,
    right: PolynomialCensus,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    let pair = pair_census(left, right)?;
    stats.equality_comparisons = bounded_add(
        "affine condition equality comparisons",
        stats.equality_comparisons,
        1,
        limits.max_equality_comparisons,
    )?;
    stats.equality_term_units = bounded_add(
        "affine condition equality term units",
        stats.equality_term_units,
        pair.terms,
        limits.max_equality_term_units,
    )?;
    stats.equality_exponent_entries = bounded_add(
        "affine condition equality exponent entries",
        stats.equality_exponent_entries,
        pair.exponent_entries,
        limits.max_equality_exponent_entries,
    )?;
    stats.equality_integer_bits = bounded_add(
        "affine condition equality integer bits",
        stats.equality_integer_bits,
        pair.integer_bits,
        limits.max_equality_integer_bits,
    )?;
    Ok(())
}

fn precharge_associate_comparison(
    left: PolynomialCensus,
    right: PolynomialCensus,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<ParametricPolynomialAssociateLimits, GeneratedResidualAffineConditionAccumulatorError> {
    let pair = pair_census(left, right)?;
    let remaining_term_units = remaining(
        "affine condition associate term units",
        limits.max_associate_term_units,
        stats.associate_term_units,
    )?;
    check_limit(
        "affine condition associate term units",
        pair.terms,
        remaining_term_units,
    )?;

    let prospective_checks = bounded_add(
        "affine condition associate checks",
        stats.associate_checks,
        1,
        limits.max_associate_checks,
    )?;
    let prospective_terms = bounded_add(
        "affine condition associate term units",
        stats.associate_term_units,
        pair.terms,
        limits.max_associate_term_units,
    )?;
    let prospective_exponents = bounded_add(
        "affine condition associate exponent entries",
        stats.associate_exponent_entries,
        pair.exponent_entries,
        limits.max_associate_exponent_entries,
    )?;
    let prospective_integer_bits = bounded_add(
        "affine condition associate integer bits",
        stats.associate_integer_bits,
        pair.integer_bits,
        limits.max_associate_integer_bits,
    )?;

    let cross_remaining = remaining(
        "affine condition associate native cross term pairs",
        limits.max_associate_native_cross_term_pairs,
        stats.associate_native_cross_term_pairs,
    )?;
    let exact_algebra = ExactAlgebraLimits {
        max_exponent: limits.exact_algebra.max_exponent,
        max_polynomial_terms: limits
            .exact_algebra
            .max_polynomial_terms
            .min(remaining_term_units),
        max_term_operations: limits
            .exact_algebra
            .max_term_operations
            .min(cross_remaining),
    };
    let child_limits = ParametricPolynomialAssociateLimits {
        exact_algebra,
        max_context_fingerprint_comparison_bytes: remaining(
            "affine condition context fingerprint comparison bytes",
            limits.max_context_fingerprint_comparison_bytes,
            stats.context_fingerprint_comparison_bytes,
        )?,
        max_variable_map_entry_comparisons: remaining(
            "affine condition variable-map entry comparisons",
            limits.max_variable_map_entry_comparisons,
            stats.variable_map_entry_comparisons,
        )?,
        max_validation_terms: remaining(
            "affine condition associate validation terms",
            limits.max_associate_validation_terms,
            stats.associate_validation_terms,
        )?
        .min(pair.terms),
        max_validation_exponent_entries: remaining(
            "affine condition associate validation exponent entries",
            limits.max_associate_validation_exponent_entries,
            stats.associate_validation_exponent_entries,
        )?
        .min(pair.exponent_entries),
        max_validation_integer_bits: remaining(
            "affine condition associate validation integer bits",
            limits.max_associate_validation_integer_bits,
            stats.associate_validation_integer_bits,
        )?
        .min(pair.integer_bits),
        max_projection_exponent_entries: remaining(
            "affine condition associate projection exponent entries",
            limits.max_associate_projection_exponent_entries,
            stats.associate_projection_exponent_entries,
        )?,
        max_projection_coefficient_capacity_bytes: remaining(
            "affine condition associate projection coefficient-capacity bytes",
            limits.max_associate_projection_coefficient_capacity_bytes,
            stats.associate_projection_coefficient_capacity_bytes,
        )?,
        max_projection_group_bound: remaining(
            "affine condition associate projection group bound",
            limits.max_associate_projection_group_bound,
            stats.associate_projection_group_bound,
        )?,
        max_projection_variable_mask_comparison_bound: remaining(
            "affine condition associate projection variable-mask comparison bound",
            limits.max_associate_projection_variable_mask_comparison_bound,
            stats.associate_projection_variable_mask_comparison_bound,
        )?,
        max_projection_hash_key_exponent_entry_bound: remaining(
            "affine condition associate projection hash-key exponent-entry bound",
            limits.max_associate_projection_hash_key_exponent_entry_bound,
            stats.associate_projection_hash_key_exponent_entry_bound,
        )?,
        max_projection_coefficient_append_comparison_bound: remaining(
            "affine condition associate projection coefficient append comparison bound",
            limits.max_associate_projection_coefficient_append_comparison_bound,
            stats.associate_projection_coefficient_append_comparison_bound,
        )?,
        max_projection_sorted_insert_comparison_bound: remaining(
            "affine condition associate projection sorted-insert comparison bound",
            limits.max_associate_projection_sorted_insert_comparison_bound,
            stats.associate_projection_sorted_insert_comparison_bound,
        )?,
        max_projection_sorted_insert_move_exponent_entry_bound: remaining(
            "affine condition associate projection sorted-insert move exponent-entry bound",
            limits.max_associate_projection_sorted_insert_move_exponent_entry_bound,
            stats.associate_projection_sorted_insert_move_exponent_entry_bound,
        )?,
        max_index_groups: remaining(
            "affine condition associate index groups",
            limits.max_associate_index_groups,
            stats.associate_index_groups,
        )?,
        max_index_support_comparison_entries: remaining(
            "affine condition associate index support comparison entries",
            limits.max_associate_index_support_comparison_entries,
            stats.associate_index_support_comparison_entries,
        )?,
        max_anchor_cost_operations: remaining(
            "affine condition associate anchor-cost operations",
            limits.max_associate_anchor_cost_operations,
            stats.associate_anchor_cost_operations,
        )?,
        max_native_cross_term_pairs: cross_remaining,
        max_peak_native_cross_term_pairs: remaining(
            "affine condition associate peak native cross term pairs",
            limits.max_associate_peak_native_cross_term_pairs,
            stats.associate_peak_native_cross_term_pairs,
        )?,
        max_native_base_exponent_additions: remaining(
            "affine condition associate native base exponent additions",
            limits.max_associate_native_base_exponent_additions,
            stats.associate_native_base_exponent_additions,
        )?,
        max_native_metadata_exponent_entry_inspection_bound: remaining(
            "affine condition associate native metadata exponent-entry inspection bound",
            limits.max_associate_native_metadata_exponent_entry_inspection_bound,
            stats.associate_native_metadata_exponent_entry_inspection_bound,
        )?,
        max_native_metadata_integer_entry_inspection_bound: remaining(
            "affine condition associate native metadata integer-entry inspection bound",
            limits.max_associate_native_metadata_integer_entry_inspection_bound,
            stats.associate_native_metadata_integer_entry_inspection_bound,
        )?,
        max_native_integer_multiplication_bit_work_bound: remaining(
            "affine condition associate native integer multiplication bit-work bound",
            limits.max_associate_native_integer_multiplication_bit_work_bound,
            stats.associate_native_integer_multiplication_bit_work_bound,
        )?,
        max_native_integer_collection_bit_work_bound: remaining(
            "affine condition associate native integer collection bit-work bound",
            limits.max_associate_native_integer_collection_bit_work_bound,
            stats.associate_native_integer_collection_bit_work_bound,
        )?,
        max_native_output_term_bound: remaining(
            "affine condition associate native output term bound",
            limits.max_associate_native_output_term_bound,
            stats.associate_native_output_term_bound,
        )?,
        max_native_output_exponent_entry_bound: remaining(
            "affine condition associate native output exponent entry bound",
            limits.max_associate_native_output_exponent_entry_bound,
            stats.associate_native_output_exponent_entry_bound,
        )?,
        max_native_output_integer_bit_bound: remaining(
            "affine condition associate native output integer bit bound",
            limits.max_associate_native_output_integer_bit_bound,
            stats.associate_native_output_integer_bit_bound,
        )?,
        max_native_dense_workspace_entries: remaining(
            "affine condition associate native dense workspace entries",
            limits.max_associate_native_dense_workspace_entries,
            stats.associate_native_dense_workspace_entries,
        )?,
        max_native_heap_workspace_pair_bound: remaining(
            "affine condition associate native heap workspace pair bound",
            limits.max_associate_native_heap_workspace_pair_bound,
            stats.associate_native_heap_workspace_pair_bound,
        )?,
        max_native_workspace_byte_envelope: remaining(
            "affine condition associate native workspace byte envelope",
            limits.max_associate_native_workspace_byte_envelope,
            stats.associate_native_workspace_byte_envelope,
        )?,
        max_rustred_visible_temporary_byte_envelope: remaining(
            "affine condition associate RustRed-visible temporary byte envelope",
            limits.max_associate_rustred_visible_temporary_byte_envelope,
            stats.associate_rustred_visible_temporary_byte_envelope,
        )?,
        max_combined_temporary_byte_envelope: remaining(
            "affine condition associate combined temporary byte envelope",
            limits.max_associate_combined_temporary_byte_envelope,
            stats.associate_combined_temporary_byte_envelope,
        )?,
    };
    stats.associate_checks = prospective_checks;
    stats.associate_term_units = prospective_terms;
    stats.associate_exponent_entries = prospective_exponents;
    stats.associate_integer_bits = prospective_integer_bits;
    Ok(child_limits)
}

fn precharge_base_associate_comparison(
    left: PolynomialCensus,
    right: PolynomialCensus,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<ParametricBasePolynomialAssociateLimits, GeneratedResidualAffineConditionAccumulatorError>
{
    let pair = pair_census(left, right)?;
    let remaining_term_units = remaining(
        "affine condition associate term units",
        limits.max_associate_term_units,
        stats.associate_term_units,
    )?;
    check_limit(
        "affine condition associate term units",
        pair.terms,
        remaining_term_units,
    )?;

    let prospective_checks = bounded_add(
        "affine condition associate checks",
        stats.associate_checks,
        1,
        limits.max_associate_checks,
    )?;
    let prospective_terms = bounded_add(
        "affine condition associate term units",
        stats.associate_term_units,
        pair.terms,
        limits.max_associate_term_units,
    )?;
    let prospective_exponents = bounded_add(
        "affine condition associate exponent entries",
        stats.associate_exponent_entries,
        pair.exponent_entries,
        limits.max_associate_exponent_entries,
    )?;
    let prospective_integer_bits = bounded_add(
        "affine condition associate integer bits",
        stats.associate_integer_bits,
        pair.integer_bits,
        limits.max_associate_integer_bits,
    )?;
    let remaining_native_coefficient_multiplications = remaining(
        "affine condition base-associate native coefficient multiplications",
        limits.max_base_associate_native_coefficient_multiplications,
        stats.base_associate_native_coefficient_multiplications,
    )?;
    let exact_algebra = ExactAlgebraLimits {
        max_exponent: limits.exact_algebra.max_exponent,
        max_polynomial_terms: limits
            .exact_algebra
            .max_polynomial_terms
            .min(remaining_term_units),
        max_term_operations: limits
            .exact_algebra
            .max_term_operations
            .min(remaining_native_coefficient_multiplications),
    };
    let child_limits = ParametricBasePolynomialAssociateLimits {
        exact_algebra,
        max_context_fingerprint_comparison_bytes: remaining(
            "affine condition context fingerprint comparison bytes",
            limits.max_context_fingerprint_comparison_bytes,
            stats.context_fingerprint_comparison_bytes,
        )?,
        max_variable_map_entry_comparisons: remaining(
            "affine condition variable-map entry comparisons",
            limits.max_variable_map_entry_comparisons,
            stats.variable_map_entry_comparisons,
        )?,
        max_validation_terms: remaining(
            "affine condition base-associate validation terms",
            limits.max_base_associate_validation_terms,
            stats.base_associate_validation_terms,
        )?
        .min(pair.terms),
        max_validation_exponent_entries: remaining(
            "affine condition base-associate validation exponent entries",
            limits.max_base_associate_validation_exponent_entries,
            stats.base_associate_validation_exponent_entries,
        )?
        .min(pair.exponent_entries),
        max_validation_integer_bits: remaining(
            "affine condition base-associate validation integer bits",
            limits.max_base_associate_validation_integer_bits,
            stats.base_associate_validation_integer_bits,
        )?
        .min(pair.integer_bits),
        max_source_owned_bytes: remaining(
            "affine condition base-associate source owned bytes",
            limits.max_base_associate_source_owned_bytes,
            stats.base_associate_source_owned_bytes,
        )?,
        max_index_exponent_entries: remaining(
            "affine condition base-associate index exponent entries",
            limits.max_base_associate_index_exponent_entries,
            stats.base_associate_index_exponent_entries,
        )?,
        max_native_scale_calls: remaining(
            "affine condition base-associate native scale calls",
            limits.max_base_associate_native_scale_calls,
            stats.base_associate_native_scale_calls,
        )?,
        max_native_coefficient_multiplications: remaining_native_coefficient_multiplications,
        max_native_integer_multiplication_bit_work_bound: remaining(
            "affine condition base-associate native integer multiplication bit-work bound",
            limits.max_base_associate_native_integer_multiplication_bit_work_bound,
            stats.base_associate_native_integer_multiplication_bit_work_bound,
        )?,
        max_output_terms: remaining(
            "affine condition base-associate output terms",
            limits.max_base_associate_output_terms,
            stats.base_associate_output_terms,
        )?,
        max_output_exponent_entries: remaining(
            "affine condition base-associate output exponent entries",
            limits.max_base_associate_output_exponent_entries,
            stats.base_associate_output_exponent_entries,
        )?,
        max_output_integer_bit_bound: remaining(
            "affine condition base-associate output integer bit bound",
            limits.max_base_associate_output_integer_bit_bound,
            stats.base_associate_output_integer_bit_bound,
        )?,
        max_output_retained_byte_bound: remaining(
            "affine condition base-associate output retained byte bound",
            limits.max_base_associate_output_retained_byte_bound,
            stats.base_associate_output_retained_byte_bound,
        )?,
        max_payload_comparison_terms: remaining(
            "affine condition base-associate payload comparison terms",
            limits.max_base_associate_payload_comparison_terms,
            stats.base_associate_payload_comparison_terms,
        )?,
        max_payload_comparison_exponent_entries: remaining(
            "affine condition base-associate payload comparison exponent entries",
            limits.max_base_associate_payload_comparison_exponent_entries,
            stats.base_associate_payload_comparison_exponent_entries,
        )?,
        max_payload_comparison_integer_bit_bound: remaining(
            "affine condition base-associate payload comparison integer bit bound",
            limits.max_base_associate_payload_comparison_integer_bit_bound,
            stats.base_associate_payload_comparison_integer_bit_bound,
        )?,
        max_native_workspace_byte_envelope: remaining(
            "affine condition base-associate native workspace byte envelope",
            limits.max_base_associate_native_workspace_byte_envelope,
            stats.base_associate_native_workspace_byte_envelope,
        )?,
        max_rustred_visible_temporary_byte_envelope: remaining(
            "affine condition base-associate RustRed-visible temporary byte envelope",
            limits.max_base_associate_rustred_visible_temporary_byte_envelope,
            stats.base_associate_rustred_visible_temporary_byte_envelope,
        )?,
        max_combined_temporary_byte_envelope: remaining(
            "affine condition base-associate combined temporary byte envelope",
            limits.max_base_associate_combined_temporary_byte_envelope,
            stats.base_associate_combined_temporary_byte_envelope,
        )?,
    };
    stats.associate_checks = prospective_checks;
    stats.associate_term_units = prospective_terms;
    stats.associate_exponent_entries = prospective_exponents;
    stats.associate_integer_bits = prospective_integer_bits;
    Ok(child_limits)
}

fn consume_associate_stats(
    child: ParametricPolynomialAssociateStats,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    stats.context_fingerprint_comparison_bytes = bounded_add(
        "affine condition context fingerprint comparison bytes",
        stats.context_fingerprint_comparison_bytes,
        child.context_fingerprint_comparison_bytes(),
        limits.max_context_fingerprint_comparison_bytes,
    )?;
    stats.variable_map_entry_comparisons = bounded_add(
        "affine condition variable-map entry comparisons",
        stats.variable_map_entry_comparisons,
        child.variable_map_entry_comparisons(),
        limits.max_variable_map_entry_comparisons,
    )?;
    macro_rules! consume {
        ($field:ident, $getter:ident, $resource:literal, $limit:ident) => {
            stats.$field = bounded_add($resource, stats.$field, child.$getter(), limits.$limit)?;
        };
    }
    consume!(
        associate_validation_terms,
        validation_terms,
        "affine condition associate validation terms",
        max_associate_validation_terms
    );
    consume!(
        associate_validation_exponent_entries,
        validation_exponent_entries,
        "affine condition associate validation exponent entries",
        max_associate_validation_exponent_entries
    );
    consume!(
        associate_validation_integer_bits,
        validation_integer_bits,
        "affine condition associate validation integer bits",
        max_associate_validation_integer_bits
    );
    consume!(
        associate_projection_exponent_entries,
        projection_exponent_entries,
        "affine condition associate projection exponent entries",
        max_associate_projection_exponent_entries
    );
    consume!(
        associate_projection_coefficient_capacity_bytes,
        projection_coefficient_capacity_bytes,
        "affine condition associate projection coefficient-capacity bytes",
        max_associate_projection_coefficient_capacity_bytes
    );
    consume!(
        associate_projection_group_bound,
        projection_group_bound,
        "affine condition associate projection group bound",
        max_associate_projection_group_bound
    );
    consume!(
        associate_projection_variable_mask_comparison_bound,
        projection_variable_mask_comparison_bound,
        "affine condition associate projection variable-mask comparison bound",
        max_associate_projection_variable_mask_comparison_bound
    );
    consume!(
        associate_projection_hash_key_exponent_entry_bound,
        projection_hash_key_exponent_entry_bound,
        "affine condition associate projection hash-key exponent-entry bound",
        max_associate_projection_hash_key_exponent_entry_bound
    );
    consume!(
        associate_projection_coefficient_append_comparison_bound,
        projection_coefficient_append_comparison_bound,
        "affine condition associate projection coefficient append comparison bound",
        max_associate_projection_coefficient_append_comparison_bound
    );
    consume!(
        associate_projection_sorted_insert_comparison_bound,
        projection_sorted_insert_comparison_bound,
        "affine condition associate projection sorted-insert comparison bound",
        max_associate_projection_sorted_insert_comparison_bound
    );
    consume!(
        associate_projection_sorted_insert_move_exponent_entry_bound,
        projection_sorted_insert_move_exponent_entry_bound,
        "affine condition associate projection sorted-insert move exponent-entry bound",
        max_associate_projection_sorted_insert_move_exponent_entry_bound
    );
    consume!(
        associate_index_groups,
        index_groups,
        "affine condition associate index groups",
        max_associate_index_groups
    );
    consume!(
        associate_index_support_comparison_entries,
        index_support_comparison_entries,
        "affine condition associate index support comparison entries",
        max_associate_index_support_comparison_entries
    );
    consume!(
        associate_anchor_cost_operations,
        anchor_cost_operations,
        "affine condition associate anchor-cost operations",
        max_associate_anchor_cost_operations
    );
    consume!(
        associate_native_cross_term_pairs,
        native_cross_term_pairs,
        "affine condition associate native cross term pairs",
        max_associate_native_cross_term_pairs
    );
    consume!(
        associate_peak_native_cross_term_pairs,
        peak_native_cross_term_pairs,
        "affine condition associate peak native cross term pairs",
        max_associate_peak_native_cross_term_pairs
    );
    consume!(
        associate_native_base_exponent_additions,
        native_base_exponent_additions,
        "affine condition associate native base exponent additions",
        max_associate_native_base_exponent_additions
    );
    consume!(
        associate_native_metadata_exponent_entry_inspection_bound,
        native_metadata_exponent_entry_inspection_bound,
        "affine condition associate native metadata exponent-entry inspection bound",
        max_associate_native_metadata_exponent_entry_inspection_bound
    );
    consume!(
        associate_native_metadata_integer_entry_inspection_bound,
        native_metadata_integer_entry_inspection_bound,
        "affine condition associate native metadata integer-entry inspection bound",
        max_associate_native_metadata_integer_entry_inspection_bound
    );
    consume!(
        associate_native_integer_multiplication_bit_work_bound,
        native_integer_multiplication_bit_work_bound,
        "affine condition associate native integer multiplication bit-work bound",
        max_associate_native_integer_multiplication_bit_work_bound
    );
    consume!(
        associate_native_integer_collection_bit_work_bound,
        native_integer_collection_bit_work_bound,
        "affine condition associate native integer collection bit-work bound",
        max_associate_native_integer_collection_bit_work_bound
    );
    consume!(
        associate_native_output_term_bound,
        native_output_term_bound,
        "affine condition associate native output term bound",
        max_associate_native_output_term_bound
    );
    consume!(
        associate_native_output_exponent_entry_bound,
        native_output_exponent_entry_bound,
        "affine condition associate native output exponent entry bound",
        max_associate_native_output_exponent_entry_bound
    );
    consume!(
        associate_native_output_integer_bit_bound,
        native_output_integer_bit_bound,
        "affine condition associate native output integer bit bound",
        max_associate_native_output_integer_bit_bound
    );
    consume!(
        associate_native_dense_workspace_entries,
        native_dense_workspace_entries,
        "affine condition associate native dense workspace entries",
        max_associate_native_dense_workspace_entries
    );
    consume!(
        associate_native_heap_workspace_pair_bound,
        native_heap_workspace_pair_bound,
        "affine condition associate native heap workspace pair bound",
        max_associate_native_heap_workspace_pair_bound
    );
    consume!(
        associate_native_workspace_byte_envelope,
        native_workspace_byte_envelope,
        "affine condition associate native workspace byte envelope",
        max_associate_native_workspace_byte_envelope
    );
    consume!(
        associate_rustred_visible_temporary_byte_envelope,
        rustred_visible_temporary_byte_envelope,
        "affine condition associate RustRed-visible temporary byte envelope",
        max_associate_rustred_visible_temporary_byte_envelope
    );
    let combined = checked_add(
        "affine condition associate combined temporary byte envelope",
        child.native_workspace_byte_envelope(),
        child.rustred_visible_temporary_byte_envelope(),
    )?;
    stats.associate_combined_temporary_byte_envelope = bounded_add(
        "affine condition associate combined temporary byte envelope",
        stats.associate_combined_temporary_byte_envelope,
        combined,
        limits.max_associate_combined_temporary_byte_envelope,
    )?;
    Ok(())
}

fn consume_base_associate_stats(
    child: ParametricBasePolynomialAssociateStats,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    stats.context_fingerprint_comparison_bytes = bounded_add(
        "affine condition context fingerprint comparison bytes",
        stats.context_fingerprint_comparison_bytes,
        child.context_fingerprint_comparison_bytes(),
        limits.max_context_fingerprint_comparison_bytes,
    )?;
    stats.variable_map_entry_comparisons = bounded_add(
        "affine condition variable-map entry comparisons",
        stats.variable_map_entry_comparisons,
        child.variable_map_entry_comparisons(),
        limits.max_variable_map_entry_comparisons,
    )?;
    macro_rules! consume {
        ($field:ident, $getter:ident, $resource:literal, $limit:ident) => {
            stats.$field = bounded_add($resource, stats.$field, child.$getter(), limits.$limit)?;
        };
    }
    consume!(
        base_associate_validation_terms,
        validation_terms,
        "affine condition base-associate validation terms",
        max_base_associate_validation_terms
    );
    consume!(
        base_associate_validation_exponent_entries,
        validation_exponent_entries,
        "affine condition base-associate validation exponent entries",
        max_base_associate_validation_exponent_entries
    );
    consume!(
        base_associate_validation_integer_bits,
        validation_integer_bits,
        "affine condition base-associate validation integer bits",
        max_base_associate_validation_integer_bits
    );
    consume!(
        base_associate_source_owned_bytes,
        source_owned_bytes,
        "affine condition base-associate source owned bytes",
        max_base_associate_source_owned_bytes
    );
    consume!(
        base_associate_index_exponent_entries,
        index_exponent_entries,
        "affine condition base-associate index exponent entries",
        max_base_associate_index_exponent_entries
    );
    consume!(
        base_associate_native_scale_calls,
        native_scale_calls,
        "affine condition base-associate native scale calls",
        max_base_associate_native_scale_calls
    );
    consume!(
        base_associate_native_coefficient_multiplications,
        native_coefficient_multiplications,
        "affine condition base-associate native coefficient multiplications",
        max_base_associate_native_coefficient_multiplications
    );
    consume!(
        base_associate_native_integer_multiplication_bit_work_bound,
        native_integer_multiplication_bit_work_bound,
        "affine condition base-associate native integer multiplication bit-work bound",
        max_base_associate_native_integer_multiplication_bit_work_bound
    );
    consume!(
        base_associate_output_terms,
        output_terms,
        "affine condition base-associate output terms",
        max_base_associate_output_terms
    );
    consume!(
        base_associate_output_exponent_entries,
        output_exponent_entries,
        "affine condition base-associate output exponent entries",
        max_base_associate_output_exponent_entries
    );
    consume!(
        base_associate_output_integer_bit_bound,
        output_integer_bit_bound,
        "affine condition base-associate output integer bit bound",
        max_base_associate_output_integer_bit_bound
    );
    consume!(
        base_associate_output_retained_byte_bound,
        output_retained_byte_bound,
        "affine condition base-associate output retained byte bound",
        max_base_associate_output_retained_byte_bound
    );
    consume!(
        base_associate_payload_comparison_terms,
        payload_comparison_terms,
        "affine condition base-associate payload comparison terms",
        max_base_associate_payload_comparison_terms
    );
    consume!(
        base_associate_payload_comparison_exponent_entries,
        payload_comparison_exponent_entries,
        "affine condition base-associate payload comparison exponent entries",
        max_base_associate_payload_comparison_exponent_entries
    );
    consume!(
        base_associate_payload_comparison_integer_bit_bound,
        payload_comparison_integer_bit_bound,
        "affine condition base-associate payload comparison integer bit bound",
        max_base_associate_payload_comparison_integer_bit_bound
    );
    consume!(
        base_associate_native_workspace_byte_envelope,
        native_workspace_byte_envelope,
        "affine condition base-associate native workspace byte envelope",
        max_base_associate_native_workspace_byte_envelope
    );
    consume!(
        base_associate_rustred_visible_temporary_byte_envelope,
        rustred_visible_temporary_byte_envelope,
        "affine condition base-associate RustRed-visible temporary byte envelope",
        max_base_associate_rustred_visible_temporary_byte_envelope
    );
    let combined = checked_add(
        "affine condition base-associate combined temporary byte envelope",
        child.native_workspace_byte_envelope(),
        child.rustred_visible_temporary_byte_envelope(),
    )?;
    stats.base_associate_combined_temporary_byte_envelope = bounded_add(
        "affine condition base-associate combined temporary byte envelope",
        stats.base_associate_combined_temporary_byte_envelope,
        combined,
        limits.max_base_associate_combined_temporary_byte_envelope,
    )?;
    Ok(())
}

fn pair_census(
    left: PolynomialCensus,
    right: PolynomialCensus,
) -> Result<PolynomialCensus, GeneratedResidualAffineConditionAccumulatorError> {
    Ok(PolynomialCensus {
        terms: checked_add("affine condition pair term units", left.terms, right.terms)?,
        exponent_entries: checked_add(
            "affine condition pair exponent entries",
            left.exponent_entries,
            right.exponent_entries,
        )?,
        integer_bits: checked_add(
            "affine condition pair integer bits",
            left.integer_bits,
            right.integer_bits,
        )?,
    })
}

fn charge_retained_polynomial(
    polynomial: &ParametricPolynomial,
    census: PolynomialCensus,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<usize, GeneratedResidualAffineConditionAccumulatorError> {
    // Structural and ownership admission precedes formatting.  Work is staged
    // in a copy so no partial counter update escapes a rejected polynomial.
    let owned_envelope = deterministic_polynomial_owned_byte_envelope(polynomial)?;
    let polynomial_heap_envelope = owned_envelope
        .checked_sub(size_of::<ParametricPolynomial>())
        .ok_or(
            GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                resource: "retained polynomial heap envelope",
            },
        )?;
    let mut staged = *stats;
    staged.retained_polynomial_terms = bounded_add(
        "retained affine condition polynomial terms",
        staged.retained_polynomial_terms,
        census.terms,
        limits.max_retained_polynomial_terms,
    )?;
    staged.retained_polynomial_exponent_entries = bounded_add(
        "retained affine condition polynomial exponent entries",
        staged.retained_polynomial_exponent_entries,
        census.exponent_entries,
        limits.max_retained_polynomial_exponent_entries,
    )?;
    staged.retained_polynomial_integer_bits = bounded_add(
        "retained affine condition polynomial integer bits",
        staged.retained_polynomial_integer_bits,
        census.integer_bits,
        limits.max_retained_polynomial_integer_bits,
    )?;
    staged.retained_polynomial_owned_byte_envelope = bounded_add(
        "retained affine condition polynomial owned bytes",
        staged.retained_polynomial_owned_byte_envelope,
        owned_envelope,
        limits.max_retained_polynomial_owned_bytes,
    )?;
    admit_retained_bytes(polynomial_heap_envelope, limits, &mut staged)?;

    let remaining_display = remaining(
        "retained affine condition polynomial display bytes",
        limits.max_retained_polynomial_display_bytes,
        staged.retained_polynomial_display_bytes,
    )?;
    let display_bytes =
        bounded_polynomial_display_bytes(polynomial, remaining_display).map_err(|requested| {
            GeneratedResidualAffineConditionAccumulatorError::ResourceLimit {
                resource: "retained affine condition polynomial display bytes",
                requested: staged
                    .retained_polynomial_display_bytes
                    .checked_add(requested)
                    .unwrap_or(usize::MAX),
                limit: limits.max_retained_polynomial_display_bytes,
            }
        })?;
    staged.retained_polynomial_display_bytes = bounded_add(
        "retained affine condition polynomial display bytes",
        staged.retained_polynomial_display_bytes,
        display_bytes,
        limits.max_retained_polynomial_display_bytes,
    )?;
    *stats = staged;
    Ok(owned_envelope)
}

/// Allocator-independent envelope for one fallible sparse-payload copy.
fn deterministic_polynomial_owned_byte_envelope(
    polynomial: &ParametricPolynomial,
) -> Result<usize, GeneratedResidualAffineConditionAccumulatorError> {
    let mut bytes = size_of::<ParametricPolynomial>();
    bytes = checked_add(
        "retained affine condition polynomial owned bytes",
        bytes,
        capacity_byte_envelope(polynomial.raw().coefficients.len(), size_of::<Integer>())?,
    )?;
    bytes = checked_add(
        "retained affine condition polynomial owned bytes",
        bytes,
        capacity_byte_envelope(polynomial.raw().exponents.len(), size_of::<u16>())?,
    )?;
    for coefficient in &polynomial.raw().coefficients {
        if matches!(coefficient, Integer::Large(_)) {
            let magnitude_bytes = integer_magnitude_bits(coefficient)?.checked_add(7).ok_or(
                GeneratedResidualAffineConditionAccumulatorError::ResourceCountOverflow {
                    resource: "retained affine condition polynomial owned bytes",
                },
            )? / 8;
            bytes = checked_add(
                "retained affine condition polynomial owned bytes",
                bytes,
                checked_add(
                    "retained affine condition polynomial owned bytes",
                    magnitude_bytes,
                    size_of::<usize>(),
                )?,
            )?;
        }
    }
    Ok(bytes)
}

fn bounded_polynomial_display_bytes(
    polynomial: &ParametricPolynomial,
    limit: usize,
) -> Result<usize, usize> {
    let mut writer = BoundedByteCounter {
        bytes: 0,
        limit,
        overflowed: false,
    };
    if write!(&mut writer, "{}", polynomial.raw()).is_err() {
        return Err(if writer.overflowed {
            usize::MAX
        } else {
            writer.bytes.max(limit.saturating_add(1))
        });
    }
    Ok(writer.bytes)
}

struct BoundedByteCounter {
    bytes: usize,
    limit: usize,
    overflowed: bool,
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

fn try_copy_shift(
    source: &IndexShift,
) -> Result<IndexShift, GeneratedResidualAffineConditionAccumulatorError> {
    let mut values = Vec::new();
    try_reserve_exact(
        "affine condition private shift components",
        &mut values,
        source.arity(),
    )?;
    values.extend_from_slice(source.values());
    IndexShift::try_from_preallocated(values, source.arity()).map_err(|_| {
        GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
            resource: "private shift copy",
        }
    })
}

fn try_copy_usize_slice(
    source: &[usize],
    resource: &'static str,
) -> Result<Vec<usize>, GeneratedResidualAffineConditionAccumulatorError> {
    let mut copy = Vec::new();
    try_reserve_exact(resource, &mut copy, source.len())?;
    copy.extend_from_slice(source);
    Ok(copy)
}

fn try_copy_string(
    source: &str,
    resource: &'static str,
) -> Result<String, GeneratedResidualAffineConditionAccumulatorError> {
    let mut copy = String::new();
    copy.try_reserve_exact(source.len()).map_err(|_| {
        GeneratedResidualAffineConditionAccumulatorError::AllocationFailure {
            resource,
            requested: source.len(),
        }
    })?;
    copy.push_str(source);
    Ok(copy)
}

fn try_reserve_exact<T>(
    resource: &'static str,
    target: &mut Vec<T>,
    additional: usize,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    target.try_reserve_exact(additional).map_err(|_| {
        GeneratedResidualAffineConditionAccumulatorError::AllocationFailure {
            resource,
            requested: target.len().saturating_add(additional),
        }
    })
}

fn try_push_observed<T>(
    resource: &'static str,
    target: &mut Vec<T>,
    value: T,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    let old_capacity = target.capacity();
    try_reserve_exact(resource, target, 1)?;
    target.push(value);
    let added_capacity = target.capacity().checked_sub(old_capacity).ok_or(
        GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
            resource: "retained vector capacity",
        },
    )?;
    observe_retained_bytes(
        checked_mul(
            "affine condition retained bytes",
            added_capacity,
            size_of::<T>(),
        )?,
        stats,
    )?;
    Ok(())
}

fn charge_context_comparison(
    left: &str,
    right: &str,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    stats.context_fingerprint_comparison_bytes = bounded_add(
        "affine condition context fingerprint comparison bytes",
        stats.context_fingerprint_comparison_bytes,
        checked_add(
            "affine condition context fingerprint comparison bytes",
            left.len(),
            right.len(),
        )?,
        limits.max_context_fingerprint_comparison_bytes,
    )?;
    Ok(())
}

fn charge_variable_map_comparisons(
    entries: usize,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    stats.variable_map_entry_comparisons = bounded_add(
        "affine condition variable-map entry comparisons",
        stats.variable_map_entry_comparisons,
        entries,
        limits.max_variable_map_entry_comparisons,
    )?;
    Ok(())
}

fn charge_shared_allocation_identity_comparisons(
    comparisons: usize,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    stats.shared_allocation_identity_comparisons = bounded_add(
        "affine condition shared-allocation identity comparisons",
        stats.shared_allocation_identity_comparisons,
        comparisons,
        limits.max_shared_allocation_identity_comparisons,
    )?;
    Ok(())
}

fn shared_allocation_addition_for_polynomial(
    polynomial: &ParametricPolynomial,
    previous_rows: &[GeneratedResidualAffineCanonicalConditionRow],
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<SharedAllocationCensus, GeneratedResidualAffineConditionAccumulatorError> {
    let mut context_seen = false;
    let mut variable_map_seen = false;
    for row in previous_rows {
        if !context_seen {
            charge_shared_allocation_identity_comparisons(1, limits, stats)?;
            context_seen = std::ptr::eq(
                polynomial.authenticated_context_fingerprint(),
                row.polynomial.authenticated_context_fingerprint(),
            );
        }
        if !variable_map_seen {
            charge_shared_allocation_identity_comparisons(1, limits, stats)?;
            variable_map_seen =
                Arc::ptr_eq(&polynomial.raw().variables, &row.polynomial.raw().variables);
        }
        if context_seen && variable_map_seen {
            break;
        }
    }

    Ok(SharedAllocationCensus {
        context_allocations: if context_seen { 0 } else { 1 },
        context_allocation_bytes: if context_seen {
            0
        } else {
            shared_context_allocation_byte_envelope(polynomial)?
        },
        variable_map_allocations: if variable_map_seen { 0 } else { 1 },
        variable_map_allocation_bytes: if variable_map_seen {
            0
        } else {
            shared_variable_map_allocation_byte_envelope(polynomial)?
        },
    })
}

fn shared_context_allocation_byte_envelope(
    polynomial: &ParametricPolynomial,
) -> Result<usize, GeneratedResidualAffineConditionAccumulatorError> {
    conservative_arc_allocation_byte_envelope(
        polynomial.authenticated_context_fingerprint().len(),
        align_of::<u8>(),
    )
}

fn shared_variable_map_allocation_byte_envelope(
    polynomial: &ParametricPolynomial,
) -> Result<usize, GeneratedResidualAffineConditionAccumulatorError> {
    checked_add(
        "affine condition retained shared variable-map allocation bytes",
        conservative_arc_allocation_byte_envelope(
            size_of::<Vec<PolyVariable>>(),
            align_of::<Vec<PolyVariable>>(),
        )?,
        checked_mul(
            "affine condition retained shared variable-map allocation bytes",
            polynomial.raw().variables.capacity(),
            size_of::<PolyVariable>(),
        )?,
    )
}

/// Conservative allocation payload for one distinct `Arc<T>` control block.
///
/// Rust's `Arc` control-block layout is private.  Two atomic-word counters,
/// the complete payload, and worst-case padding on both sides of the payload
/// give an allocator-independent upper envelope without claiming to observe
/// private standard-library metadata exactly.
fn conservative_arc_allocation_byte_envelope(
    payload_bytes: usize,
    payload_alignment: usize,
) -> Result<usize, GeneratedResidualAffineConditionAccumulatorError> {
    let allocation_alignment = align_of::<usize>().max(payload_alignment);
    checked_add(
        "affine condition retained shared allocation bytes",
        checked_mul(
            "affine condition retained shared allocation bytes",
            2,
            size_of::<usize>(),
        )?,
        checked_add(
            "affine condition retained shared allocation bytes",
            payload_bytes,
            checked_mul(
                "affine condition retained shared allocation bytes",
                allocation_alignment.saturating_sub(1),
                2,
            )?,
        )?,
    )
}

fn admit_retained_shared_allocations(
    addition: SharedAllocationCensus,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    admit_retained_bytes(addition.total_bytes()?, limits, stats)?;
    stats.retained_shared_context_allocations = checked_add(
        "affine condition retained shared-context allocations",
        stats.retained_shared_context_allocations,
        addition.context_allocations,
    )?;
    stats.retained_shared_context_allocation_bytes = checked_add(
        "affine condition retained shared-context allocation bytes",
        stats.retained_shared_context_allocation_bytes,
        addition.context_allocation_bytes,
    )?;
    stats.retained_shared_variable_map_allocations = checked_add(
        "affine condition retained shared variable-map allocations",
        stats.retained_shared_variable_map_allocations,
        addition.variable_map_allocations,
    )?;
    stats.retained_shared_variable_map_allocation_bytes = checked_add(
        "affine condition retained shared variable-map allocation bytes",
        stats.retained_shared_variable_map_allocation_bytes,
        addition.variable_map_allocation_bytes,
    )?;
    Ok(())
}

fn observe_retained_shared_allocations(
    addition: SharedAllocationCensus,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    observe_retained_bytes(addition.total_bytes()?, stats)
}

fn recompute_shared_allocation_census(
    rows: &[GeneratedResidualAffineCanonicalConditionRow],
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<SharedAllocationCensus, GeneratedResidualAffineConditionAccumulatorError> {
    let mut census = SharedAllocationCensus::default();
    for (row_ordinal, row) in rows.iter().enumerate() {
        census = census.checked_add(shared_allocation_addition_for_polynomial(
            &row.polynomial,
            &rows[..row_ordinal],
            limits,
            stats,
        )?)?;
    }
    Ok(census)
}

fn admit_retained_bytes(
    additional: usize,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    stats.retained_byte_envelope = bounded_add(
        "affine condition retained bytes",
        stats.retained_byte_envelope,
        additional,
        limits.max_retained_bytes,
    )?;
    Ok(())
}

fn observe_retained_bytes(
    additional: usize,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    stats.retained_bytes = checked_add(
        "affine condition retained bytes",
        stats.retained_bytes,
        additional,
    )?;
    if stats.retained_bytes > stats.retained_byte_envelope {
        Err(
            GeneratedResidualAffineConditionAccumulatorError::RetainedByteEnvelopeExceeded {
                observed: stats.retained_bytes,
                admitted: stats.retained_byte_envelope,
            },
        )
    } else {
        Ok(())
    }
}

fn validate_final_invariants(
    context: &ParametricCoefficientContext,
    context_fingerprint: &str,
    free_positions: &[usize],
    free_position_membership: &[u8],
    transcript: &[GeneratedResidualAffineConditionInputTranscript],
    rows: &[GeneratedResidualAffineCanonicalConditionRow],
    candidate_is_identically_bad: bool,
    limits: GeneratedResidualAffineConditionAccumulatorLimits,
    stats: &mut GeneratedResidualAffineConditionAccumulatorStats,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    // Conservative complete pass census, precharged without evaluating an
    // overflowing product:
    //
    //   F + M + 3 I + 4 R + S + 7 T + 4 E + H
    //
    // F/M are the free-position list/bitmap; I is the transcript length
    // (replay plus both retained-byte recomputations); R is the row count
    // (replay, shared-allocation census, and both byte recomputations); S is
    // the source-ordinal payload; T/E are retained polynomial terms/exponents
    // across authentication, dependency/format/ownership replay and both byte
    // recomputations; H is retained shift payload. Shared-pointer searches and
    // variable maps have their own counters; fingerprint and formatted bytes
    // are additionally constrained by their dedicated byte limits below.
    let mut work = 0usize;
    for entries in [free_positions.len(), free_position_membership.len()] {
        work = bounded_add(
            "affine condition final invariant entries",
            work,
            entries,
            limits.max_final_invariant_entries,
        )?;
    }
    for (entries, factor) in [
        (transcript.len(), 3),
        (rows.len(), 4),
        (stats.condition_sources, 1),
        (stats.retained_polynomial_terms, 7),
        (stats.retained_polynomial_exponent_entries, 4),
        (stats.source_shift_components, 1),
    ] {
        work = bounded_product_add(
            "affine condition final invariant entries",
            work,
            entries,
            factor,
            limits.max_final_invariant_entries,
        )?;
    }
    stats.final_invariant_entries = work;

    charge_context_comparison(context_fingerprint, context.fingerprint(), limits, stats)?;
    let replayed_context_fingerprint_bytes = context_fingerprint.len();
    check_limit(
        "affine condition context fingerprint bytes",
        replayed_context_fingerprint_bytes,
        limits.max_context_fingerprint_bytes,
    )?;
    let replayed_ambient_variables = checked_add(
        "affine condition ambient variables",
        context.base().variables().len(),
        context.index_count(),
    )?;
    check_limit(
        "affine condition ambient variables",
        replayed_ambient_variables,
        limits.max_ambient_variables,
    )?;
    if context_fingerprint != context.fingerprint()
        || replayed_context_fingerprint_bytes != stats.context_fingerprint_bytes
        || replayed_ambient_variables != stats.ambient_variables
        || free_position_membership.len() != context.index_count()
        || stats.free_positions != free_positions.len()
        || stats.free_position_membership_entries != free_position_membership.len()
    {
        return Err(
            GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                resource: "final context/free-position schema",
            },
        );
    }
    let mut next_free = 0usize;
    for (position, &membership) in free_position_membership.iter().enumerate() {
        let expected = free_positions.get(next_free).copied() == Some(position);
        if expected {
            next_free = next_free.checked_add(1).ok_or(
                GeneratedResidualAffineConditionAccumulatorError::ResourceCountOverflow {
                    resource: "affine condition final free-position membership",
                },
            )?;
        }
        if membership > 1 || (membership != 0) != expected {
            return Err(
                GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                    resource: "final free-position membership",
                },
            );
        }
    }
    if next_free != free_positions.len() {
        return Err(
            GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                resource: "final free-position coverage",
            },
        );
    }

    let mut inherited_inputs = 0usize;
    let mut candidate_inputs = 0usize;
    let mut condition_sources = 0usize;
    let mut discharged_nonzero_constants = 0usize;
    let mut identically_zero_candidate_inputs = 0usize;
    let mut source_shift_components = 0usize;

    for (ordinal, input) in transcript.iter().enumerate() {
        if input.ordinal != ordinal {
            return Err(
                GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                    resource: "input transcript ordinals",
                },
            );
        }
        match input.scope {
            GeneratedResidualAffineConditionScope::InheritedTargetPremise => {
                inherited_inputs = checked_add(
                    "affine condition final inherited inputs",
                    inherited_inputs,
                    1,
                )?;
            }
            GeneratedResidualAffineConditionScope::CandidateRequired => {
                candidate_inputs = checked_add(
                    "affine condition final candidate inputs",
                    candidate_inputs,
                    1,
                )?;
            }
        }
        if !source_scope_is_valid(input.scope, input.source.locator) {
            return Err(
                GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                    resource: "input source/scope pairing",
                },
            );
        }
        let denominator_source = matches!(
            input.source.locator,
            GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator { .. }
        );
        match (denominator_source, input.source.private_shift.as_ref()) {
            (true, Some(shift)) if shift.arity() == context.index_count() => {
                source_shift_components = checked_add(
                    "affine condition final source-shift components",
                    source_shift_components,
                    shift.arity(),
                )?;
            }
            (false, None) => {}
            _ => {
                return Err(
                    GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                        resource: "final source-shift schema",
                    },
                );
            }
        }
        match input.class {
            GeneratedResidualAffineConditionInputClass::DischargedNonzeroIntegerConstant => {
                discharged_nonzero_constants = checked_add(
                    "affine condition final discharged constants",
                    discharged_nonzero_constants,
                    1,
                )?;
            }
            GeneratedResidualAffineConditionInputClass::IdenticallyZeroCandidate => {
                identically_zero_candidate_inputs = checked_add(
                    "affine condition final zero candidate inputs",
                    identically_zero_candidate_inputs,
                    1,
                )?;
                if input.scope != GeneratedResidualAffineConditionScope::CandidateRequired {
                    return Err(
                        GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                            resource: "zero candidate scope",
                        },
                    );
                }
            }
            GeneratedResidualAffineConditionInputClass::BaseAssumption { row_ordinal }
            | GeneratedResidualAffineConditionInputClass::IndexDependent { row_ordinal } => {
                if row_ordinal >= rows.len() {
                    return Err(
                        GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                            resource: "input canonical row ordinal",
                        },
                    );
                }
                condition_sources = checked_add(
                    "affine condition final condition sources",
                    condition_sources,
                    1,
                )?;
            }
        }
    }

    let mut source_total = 0usize;
    let mut unique_inherited_rows = 0usize;
    let mut unique_candidate_rows = 0usize;
    let mut unique_base_rows = 0usize;
    let mut unique_index_dependent_rows = 0usize;
    let mut retained_polynomial_terms = 0usize;
    let mut retained_polynomial_exponent_entries = 0usize;
    let mut retained_polynomial_integer_bits = 0usize;
    let mut retained_polynomial_display_bytes = 0usize;
    let mut retained_polynomial_owned_byte_envelope = 0usize;
    let mut retained_polynomial_owned_bytes = 0usize;
    for (row_ordinal, row) in rows.iter().enumerate() {
        match row.scope {
            GeneratedResidualAffineConditionScope::InheritedTargetPremise => {
                unique_inherited_rows = checked_add(
                    "affine condition final inherited rows",
                    unique_inherited_rows,
                    1,
                )?;
            }
            GeneratedResidualAffineConditionScope::CandidateRequired => {
                unique_candidate_rows = checked_add(
                    "affine condition final candidate rows",
                    unique_candidate_rows,
                    1,
                )?;
            }
        }
        if row.polynomial.is_zero() || row.polynomial.is_nonzero_constant() {
            return Err(
                GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                    resource: "final canonical polynomial class",
                },
            );
        }
        charge_context_comparison(
            row.polynomial.authenticated_context_fingerprint(),
            context.fingerprint(),
            limits,
            stats,
        )?;
        let ambient_variables = stats.ambient_variables;
        charge_variable_map_comparisons(ambient_variables, limits, stats)?;
        let replayed_census = context.preflight_polynomial_validation_payload_with_limits(
            &row.polynomial,
            limits.exact_algebra,
            row.census.terms,
            row.census.exponent_entries,
            row.census.integer_bits,
        )?;
        let replayed_census = PolynomialCensus {
            terms: replayed_census.source_terms(),
            exponent_entries: replayed_census.source_exponent_entries(),
            integer_bits: replayed_census.source_integer_bits(),
        };
        if replayed_census != row.census {
            return Err(
                GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                    resource: "final canonical polynomial census",
                },
            );
        }
        let replayed_dependency =
            replay_private_index_support(context, &row.polynomial, free_position_membership)?;
        if replayed_dependency != row.index_dependent {
            return Err(
                GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                    resource: "final canonical polynomial dependency",
                },
            );
        }
        if replayed_dependency {
            unique_index_dependent_rows = checked_add(
                "affine condition final index-dependent rows",
                unique_index_dependent_rows,
                1,
            )?;
        } else {
            unique_base_rows =
                checked_add("affine condition final base rows", unique_base_rows, 1)?;
        }
        retained_polynomial_terms = checked_add(
            "affine condition final retained polynomial terms",
            retained_polynomial_terms,
            replayed_census.terms,
        )?;
        retained_polynomial_exponent_entries = checked_add(
            "affine condition final retained polynomial exponent entries",
            retained_polynomial_exponent_entries,
            replayed_census.exponent_entries,
        )?;
        retained_polynomial_integer_bits = checked_add(
            "affine condition final retained polynomial integer bits",
            retained_polynomial_integer_bits,
            replayed_census.integer_bits,
        )?;
        retained_polynomial_display_bytes = bounded_add(
            "retained affine condition polynomial display bytes",
            retained_polynomial_display_bytes,
            bounded_polynomial_display_bytes(
                &row.polynomial,
                limits.max_retained_polynomial_display_bytes,
            )
            .map_err(|requested| {
                GeneratedResidualAffineConditionAccumulatorError::ResourceLimit {
                    resource: "retained affine condition polynomial display bytes",
                    requested,
                    limit: limits.max_retained_polynomial_display_bytes,
                }
            })?,
            limits.max_retained_polynomial_display_bytes,
        )?;
        retained_polynomial_owned_byte_envelope = checked_add(
            "affine condition final retained polynomial owned-byte envelope",
            retained_polynomial_owned_byte_envelope,
            deterministic_polynomial_owned_byte_envelope(&row.polynomial)?,
        )?;
        retained_polynomial_owned_bytes = checked_add(
            "affine condition final retained polynomial owned bytes",
            retained_polynomial_owned_bytes,
            row.polynomial.owned_retained_byte_bound().ok_or(
                GeneratedResidualAffineConditionAccumulatorError::ResourceCountOverflow {
                    resource: "affine condition final retained polynomial owned bytes",
                },
            )?,
        )?;
        source_total = checked_add(
            "affine condition final source ordinals",
            source_total,
            row.source_input_ordinals.len(),
        )?;
        let mut inherited = false;
        for (source_position, &input_ordinal) in row.source_input_ordinals.iter().enumerate() {
            if source_position > 0
                && row.source_input_ordinals[source_position - 1] >= input_ordinal
            {
                return Err(
                    GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                        resource: "canonical source encounter order",
                    },
                );
            }
            let input = transcript.get(input_ordinal).ok_or(
                GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                    resource: "canonical source input ordinal",
                },
            )?;
            inherited |=
                input.scope == GeneratedResidualAffineConditionScope::InheritedTargetPremise;
            let class_matches = match input.class {
                GeneratedResidualAffineConditionInputClass::BaseAssumption {
                    row_ordinal: retained,
                } => retained == row_ordinal && !row.index_dependent,
                GeneratedResidualAffineConditionInputClass::IndexDependent {
                    row_ordinal: retained,
                } => retained == row_ordinal && row.index_dependent,
                GeneratedResidualAffineConditionInputClass::DischargedNonzeroIntegerConstant
                | GeneratedResidualAffineConditionInputClass::IdenticallyZeroCandidate => false,
            };
            if !class_matches {
                return Err(
                    GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                        resource: "canonical source backreference",
                    },
                );
            }
        }
        let expected_scope = if inherited {
            GeneratedResidualAffineConditionScope::InheritedTargetPremise
        } else {
            GeneratedResidualAffineConditionScope::CandidateRequired
        };
        if row.scope != expected_scope || row.source_input_ordinals.is_empty() {
            return Err(
                GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                    resource: "canonical row scope",
                },
            );
        }
    }

    check_limit(
        "unique inherited affine condition rows",
        unique_inherited_rows,
        limits.max_unique_inherited_rows,
    )?;
    check_limit(
        "unique candidate affine condition rows",
        unique_candidate_rows,
        limits.max_unique_candidate_rows,
    )?;
    if transcript.len() != stats.condition_inputs
        || stats.source_inputs != transcript.len()
        || inherited_inputs != stats.inherited_inputs
        || candidate_inputs != stats.candidate_inputs
        || inherited_inputs.checked_add(candidate_inputs) != Some(transcript.len())
        || condition_sources != stats.condition_sources
        || source_total != condition_sources
        || discharged_nonzero_constants != stats.discharged_nonzero_constants
        || identically_zero_candidate_inputs != stats.identically_zero_candidate_inputs
        || source_shift_components != stats.source_shift_components
        || candidate_is_identically_bad != (identically_zero_candidate_inputs != 0)
        || discharged_nonzero_constants
            .checked_add(identically_zero_candidate_inputs)
            .and_then(|value| value.checked_add(condition_sources))
            != Some(transcript.len())
        || rows.len() != stats.unique_rows
        || unique_inherited_rows != stats.unique_inherited_rows
        || unique_candidate_rows != stats.unique_candidate_rows
        || unique_inherited_rows.checked_add(unique_candidate_rows) != Some(rows.len())
        || unique_base_rows != stats.unique_base_rows
        || unique_index_dependent_rows != stats.unique_index_dependent_rows
        || unique_base_rows.checked_add(unique_index_dependent_rows) != Some(rows.len())
        || retained_polynomial_terms != stats.retained_polynomial_terms
        || retained_polynomial_exponent_entries != stats.retained_polynomial_exponent_entries
        || retained_polynomial_integer_bits != stats.retained_polynomial_integer_bits
        || retained_polynomial_display_bytes != stats.retained_polynomial_display_bytes
        || retained_polynomial_owned_byte_envelope != stats.retained_polynomial_owned_byte_envelope
        || retained_polynomial_owned_bytes != stats.retained_polynomial_owned_bytes
    {
        return Err(
            GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                resource: "final replayed counters",
            },
        );
    }
    Ok(())
}

fn replay_private_index_support(
    context: &ParametricCoefficientContext,
    polynomial: &ParametricPolynomial,
    free_position_membership: &[u8],
) -> Result<bool, GeneratedResidualAffineConditionAccumulatorError> {
    let raw = polynomial.raw();
    let base_count = context.base().variables().len();
    let variable_count = raw.variables.len();
    if variable_count
        != base_count
            .checked_add(free_position_membership.len())
            .ok_or(
                GeneratedResidualAffineConditionAccumulatorError::ResourceCountOverflow {
                    resource: "affine condition final variable-map arity",
                },
            )?
    {
        return Err(
            GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                resource: "final variable-map arity",
            },
        );
    }
    let mut index_dependent = false;
    for exponents in raw.exponents.chunks_exact(variable_count) {
        for (position, &exponent) in exponents[base_count..].iter().enumerate() {
            if exponent != 0 {
                index_dependent = true;
                if free_position_membership[position] == 0 {
                    return Err(
                        GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                            resource: "final nonfree private-index support",
                        },
                    );
                }
            }
        }
    }
    Ok(index_dependent)
}

fn recompute_retained_byte_envelope(
    context_fingerprint: &String,
    free_positions: &Vec<usize>,
    free_position_membership: &Vec<u8>,
    transcript: &Vec<GeneratedResidualAffineConditionInputTranscript>,
    rows: &Vec<GeneratedResidualAffineCanonicalConditionRow>,
    shared_allocations: SharedAllocationCensus,
) -> Result<usize, GeneratedResidualAffineConditionAccumulatorError> {
    let mut bytes = size_of::<GeneratedResidualAffineConditionAccumulatorCertificate>();
    for additional in [
        capacity_byte_envelope(context_fingerprint.len(), size_of::<u8>())?,
        capacity_byte_envelope(free_positions.len(), size_of::<usize>())?,
        capacity_byte_envelope(free_position_membership.len(), size_of::<u8>())?,
        capacity_byte_envelope(
            transcript.len(),
            size_of::<GeneratedResidualAffineConditionInputTranscript>(),
        )?,
        capacity_byte_envelope(
            rows.len(),
            size_of::<GeneratedResidualAffineCanonicalConditionRow>(),
        )?,
    ] {
        bytes = checked_add("affine condition retained bytes", bytes, additional)?;
    }
    bytes = checked_add(
        "affine condition retained bytes",
        bytes,
        shared_allocations.total_bytes()?,
    )?;
    for input in transcript {
        if let Some(shift) = &input.source.private_shift {
            bytes = checked_add(
                "affine condition retained bytes",
                bytes,
                capacity_byte_envelope(shift.arity(), size_of::<i64>())?,
            )?;
        }
    }
    for row in rows {
        bytes = checked_add(
            "affine condition retained bytes",
            bytes,
            capacity_byte_envelope(row.source_input_ordinals.len(), size_of::<usize>())?,
        )?;
        bytes = checked_add(
            "affine condition retained bytes",
            bytes,
            deterministic_polynomial_owned_byte_envelope(&row.polynomial)?
                .checked_sub(size_of::<ParametricPolynomial>())
                .ok_or(
                    GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                        resource: "retained polynomial heap envelope",
                    },
                )?,
        )?;
    }
    Ok(bytes)
}

fn recompute_observed_retained_bytes(
    context_fingerprint: &String,
    free_positions: &Vec<usize>,
    free_position_membership: &Vec<u8>,
    transcript: &Vec<GeneratedResidualAffineConditionInputTranscript>,
    rows: &Vec<GeneratedResidualAffineCanonicalConditionRow>,
    shared_allocations: SharedAllocationCensus,
) -> Result<usize, GeneratedResidualAffineConditionAccumulatorError> {
    let mut bytes = size_of::<GeneratedResidualAffineConditionAccumulatorCertificate>();
    for additional in [
        checked_mul(
            "affine condition retained bytes",
            context_fingerprint.capacity(),
            size_of::<u8>(),
        )?,
        checked_mul(
            "affine condition retained bytes",
            free_positions.capacity(),
            size_of::<usize>(),
        )?,
        checked_mul(
            "affine condition retained bytes",
            free_position_membership.capacity(),
            size_of::<u8>(),
        )?,
        checked_mul(
            "affine condition retained bytes",
            transcript.capacity(),
            size_of::<GeneratedResidualAffineConditionInputTranscript>(),
        )?,
        checked_mul(
            "affine condition retained bytes",
            rows.capacity(),
            size_of::<GeneratedResidualAffineCanonicalConditionRow>(),
        )?,
    ] {
        bytes = checked_add("affine condition retained bytes", bytes, additional)?;
    }
    bytes = checked_add(
        "affine condition retained bytes",
        bytes,
        shared_allocations.total_bytes()?,
    )?;
    for input in transcript {
        if let Some(shift) = &input.source.private_shift {
            bytes = checked_add(
                "affine condition retained bytes",
                bytes,
                shift.owned_retained_byte_bound().ok_or(
                    GeneratedResidualAffineConditionAccumulatorError::ResourceCountOverflow {
                        resource: "affine condition retained bytes",
                    },
                )?,
            )?;
        }
    }
    for row in rows {
        bytes = checked_add(
            "affine condition retained bytes",
            bytes,
            checked_mul(
                "affine condition retained bytes",
                row.source_input_ordinals.capacity(),
                size_of::<usize>(),
            )?,
        )?;
        bytes = checked_add(
            "affine condition retained bytes",
            bytes,
            row.polynomial
                .owned_retained_byte_bound()
                .ok_or(
                    GeneratedResidualAffineConditionAccumulatorError::ResourceCountOverflow {
                        resource: "affine condition retained bytes",
                    },
                )?
                .checked_sub(size_of::<ParametricPolynomial>())
                .ok_or(
                    GeneratedResidualAffineConditionAccumulatorError::InternalInvariant {
                        resource: "retained polynomial inline bytes",
                    },
                )?,
        )?;
    }
    Ok(bytes)
}

fn integer_magnitude_bits(
    value: &Integer,
) -> Result<usize, GeneratedResidualAffineConditionAccumulatorError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| {
        GeneratedResidualAffineConditionAccumulatorError::ResourceCountOverflow {
            resource: "affine condition integer magnitude bits",
        }
    })
}

fn capacity_byte_envelope(
    entries: usize,
    width: usize,
) -> Result<usize, GeneratedResidualAffineConditionAccumulatorError> {
    checked_mul(
        "affine condition retained bytes",
        checked_mul("affine condition retained bytes", entries, 2)?,
        width,
    )
}

fn remaining(
    resource: &'static str,
    limit: usize,
    spent: usize,
) -> Result<usize, GeneratedResidualAffineConditionAccumulatorError> {
    limit.checked_sub(spent).ok_or(
        GeneratedResidualAffineConditionAccumulatorError::ResourceLimit {
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
) -> Result<usize, GeneratedResidualAffineConditionAccumulatorError> {
    left.checked_add(right)
        .ok_or(GeneratedResidualAffineConditionAccumulatorError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineConditionAccumulatorError> {
    left.checked_mul(right)
        .ok_or(GeneratedResidualAffineConditionAccumulatorError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, GeneratedResidualAffineConditionAccumulatorError> {
    let requested = checked_add(resource, current, additional)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn bounded_product_add(
    resource: &'static str,
    current: usize,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedResidualAffineConditionAccumulatorError> {
    let available = remaining(resource, limit, current)?;
    if left != 0 && right > available / left {
        if let Some(requested) = limit.checked_add(1) {
            return Err(
                GeneratedResidualAffineConditionAccumulatorError::ResourceLimit {
                    resource,
                    requested,
                    limit,
                },
            );
        }
        return Err(
            GeneratedResidualAffineConditionAccumulatorError::ResourceCountOverflow { resource },
        );
    }
    // The quotient check above proves both operations fit inside `available`,
    // hence inside usize, without first evaluating an overflowing product.
    Ok(current + left * right)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedResidualAffineConditionAccumulatorError> {
    if requested > limit {
        Err(
            GeneratedResidualAffineConditionAccumulatorError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

const fn portable_usize(value: u64) -> usize {
    if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}
