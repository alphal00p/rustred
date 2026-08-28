//! Exact target matching for one generated affine-case re-elimination.
//!
//! This is the topology-neutral V2 seam between per-case forward elimination
//! and the future same-group `WhenBad` transaction.  It derives every parent
//! from one exact [`GeneratedAffineResidualCaseReeliminationCertificate`],
//! scans pivots and same-group targets in their persisted order, evaluates
//! `b' = b - A p_F + p`, and split-recenters each matched pivot.  The retained
//! result is only a private candidate transcript: it consumes no target,
//! publishes no rule, and proves neither a master integral nor an empty case.
//!
//! The current upstream `NoAvailableRows` outcome is unresolved work and
//! cannot be supplied to this compiler.  Adaptive depth growth, `WhenBad`,
//! sequential target consumption, group ownership, and rule publication are
//! deliberately later layers.

use std::fmt;
use std::fmt::Write as _;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::domains::integer::Integer;

use crate::generated_affine_residual_case_reelimination::{
    GENERATED_AFFINE_RESIDUAL_CASE_REELIMINATION_V2_SCHEMA,
    GeneratedAffineResidualCaseReeliminationCertificate,
    GeneratedAffineResidualCaseReeliminationError,
};
use crate::parametric_coefficient::CoefficientPolynomial;
use crate::parametric_relation::{
    ParametricAffineFreeRecenteringLimits, ParametricAffineFreeRecenteringStats,
    ParametricRelationV2Observer, write_relation_manifest_v2_observed,
};
use crate::solver::closure::case_inventory::{
    GeneratedAffineResidualCaseAuthorityError, GeneratedAffineResidualInventoryTerminalLocator,
    GeneratedAffineResidualSameGroupTargetCaseLimits,
    GeneratedAffineResidualSameGroupTargetCasesLimits,
    GeneratedAffineResidualSameGroupTargetHandleLimits,
};
use crate::{
    IndexShift, IntegralFamily, ParametricCoefficientContext, ParametricRelation,
    ParametricRelationError, ParametricRowId,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_CASE_PIVOT_TARGET_MATCHING_V2_SCHEMA: &str =
    "rustred-generated-affine-residual-case-pivot-target-matching-v2";

const REELIMINATION_REPLAYS: usize = 1;
const REELIMINATION_ALLOCATION_COMPARISONS: usize = 1;
const SOURCE_CASE_AUTHENTICATIONS: usize = 1;
const GROUP_AUTHENTICATIONS: usize = 1;

/// Outer owner limits. Target collection/handle/resolver calls keep their own
/// sealed per-call envelopes. Their aggregate wrapper work is additionally
/// bounded here; recursive inventory authentication remains governed by the
/// authority retained transitively by the re-elimination certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCasePivotTargetMatchingLimits {
    pub(crate) same_group_targets: GeneratedAffineResidualSameGroupTargetCasesLimits,
    pub(crate) target_handle: GeneratedAffineResidualSameGroupTargetHandleLimits,
    pub(crate) target_case: GeneratedAffineResidualSameGroupTargetCaseLimits,
    pub(crate) recentering: ParametricAffineFreeRecenteringLimits,
    pub(crate) max_scope_comparison_bytes: usize,
    pub(crate) max_reelimination_replays: usize,
    pub(crate) max_source_case_authentications: usize,
    pub(crate) max_group_authentications: usize,
    pub(crate) max_pivots: usize,
    pub(crate) max_ambient_arity: usize,
    pub(crate) max_free_positions: usize,
    pub(crate) max_compact_matrix_entries: usize,
    pub(crate) max_group_targets: usize,
    pub(crate) max_target_checks: usize,
    pub(crate) max_target_handle_work: usize,
    pub(crate) max_target_resolution_scope_bytes: usize,
    pub(crate) max_target_resolution_work: usize,
    pub(crate) max_target_witnesses: usize,
    pub(crate) max_matching_target_references: usize,
    pub(crate) max_affine_operations: usize,
    pub(crate) max_affine_integer_bit_work: usize,
    pub(crate) max_affine_integer_bits: usize,
    pub(crate) max_transformed_constant_entries: usize,
    pub(crate) max_transformed_integer_bit_envelope: usize,
    pub(crate) max_transformed_integer_heap_byte_envelope: usize,
    pub(crate) max_target_comparison_entries: usize,
    pub(crate) max_target_comparison_integer_bit_work: usize,
    pub(crate) max_recentering_attempts: usize,
    pub(crate) max_recentering_boundary_checks: usize,
    pub(crate) max_retained_shift_components: usize,
    pub(crate) max_row_label_bytes: usize,
    pub(crate) max_no_target_outcomes: usize,
    pub(crate) max_recentering_boundary_outcomes: usize,
    pub(crate) max_pending_outcomes: usize,
    pub(crate) max_owner_retained_bytes: usize,
    pub(crate) max_peak_scratch_bytes: usize,
}

impl Default for GeneratedAffineResidualCasePivotTargetMatchingLimits {
    fn default() -> Self {
        const LARGE: usize = 64_000_000_000;
        const VERY_LARGE: usize = 4_000_000_000_000_000_000;
        Self {
            same_group_targets: GeneratedAffineResidualSameGroupTargetCasesLimits::default(),
            target_handle: GeneratedAffineResidualSameGroupTargetHandleLimits::default(),
            target_case: GeneratedAffineResidualSameGroupTargetCaseLimits::default(),
            recentering: ParametricAffineFreeRecenteringLimits::default(),
            max_scope_comparison_bytes: 64 * 1024 * 1024,
            max_reelimination_replays: REELIMINATION_REPLAYS,
            max_source_case_authentications: SOURCE_CASE_AUTHENTICATIONS,
            max_group_authentications: GROUP_AUTHENTICATIONS,
            max_pivots: 256_000_000,
            max_ambient_arity: 1_000_000,
            max_free_positions: 1_000_000,
            max_compact_matrix_entries: LARGE,
            max_group_targets: 256_000_000,
            max_target_checks: LARGE,
            max_target_handle_work: VERY_LARGE,
            max_target_resolution_scope_bytes: 128 * 1024 * 1024 * 1024,
            max_target_resolution_work: VERY_LARGE,
            max_target_witnesses: LARGE,
            max_matching_target_references: LARGE,
            max_affine_operations: LARGE,
            max_affine_integer_bit_work: VERY_LARGE,
            max_affine_integer_bits: 1_000_000_000,
            max_transformed_constant_entries: LARGE,
            max_transformed_integer_bit_envelope: VERY_LARGE,
            max_transformed_integer_heap_byte_envelope: 64 * 1024 * 1024 * 1024,
            max_target_comparison_entries: LARGE,
            max_target_comparison_integer_bit_work: VERY_LARGE,
            max_recentering_attempts: 256_000_000,
            max_recentering_boundary_checks: VERY_LARGE,
            max_retained_shift_components: LARGE,
            max_row_label_bytes: 1024 * 1024 * 1024,
            max_no_target_outcomes: 256_000_000,
            max_recentering_boundary_outcomes: 256_000_000,
            max_pending_outcomes: 256_000_000,
            max_owner_retained_bytes: 128 * 1024 * 1024 * 1024,
            max_peak_scratch_bytes: 64 * 1024 * 1024 * 1024,
        }
    }
}

/// Limits used only by matcher replay. They are deliberately not retained as
/// construction limits: this keeps replay comparison work from retroactively
/// failing after pivot GMP/recentering construction. The certificate records
/// exact demand, while each replay caller prospectively admits it before the
/// exact allocation check or semantic payload comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCasePivotTargetMatchingReplayLimits {
    pub(crate) max_reelimination_allocation_comparisons: usize,
    pub(crate) max_combined_matcher_owner_bytes: usize,
    pub(crate) max_payload_comparison_units: usize,
    pub(crate) max_payload_comparison_bytes: usize,
    pub(crate) max_payload_comparison_integer_bits: usize,
    pub(crate) max_payload_comparison_relation_manifest_bytes: usize,
}

impl Default for GeneratedAffineResidualCasePivotTargetMatchingReplayLimits {
    fn default() -> Self {
        Self {
            max_reelimination_allocation_comparisons: REELIMINATION_ALLOCATION_COMPARISONS,
            max_combined_matcher_owner_bytes: 256 * 1024 * 1024 * 1024,
            max_payload_comparison_units: 64_000_000_000,
            max_payload_comparison_bytes: 256 * 1024 * 1024 * 1024,
            max_payload_comparison_integer_bits: 4_000_000_000_000_000_000,
            max_payload_comparison_relation_manifest_bytes: 128 * 1024 * 1024 * 1024,
        }
    }
}

/// Exact owner census. Retained and scratch bytes are conservative
/// prospective envelopes admitted before the corresponding allocation or
/// arithmetic. Nested re-centering fields are the prospective statistics
/// reported by the generic relation API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCasePivotTargetMatchingStats {
    scope_comparison_bytes: usize,
    reelimination_replays: usize,
    source_case_authentications: usize,
    group_authentications: usize,
    pivots: usize,
    ambient_arity: usize,
    free_positions: usize,
    compact_matrix_entries: usize,
    group_targets: usize,
    same_group_scope_comparison_bytes: usize,
    same_group_case_lookups: usize,
    same_group_group_lookups: usize,
    same_group_ordinal_comparisons: usize,
    same_group_shape_comparisons: usize,
    same_group_target_case_references: usize,
    target_checks: usize,
    maximum_target_position_lookups: usize,
    maximum_target_handle_case_lookups: usize,
    maximum_target_anchor_offset_lookups: usize,
    maximum_target_handle_ordinal_comparisons: usize,
    maximum_target_case_scope_comparison_bytes: usize,
    maximum_target_authority_allocation_comparisons: usize,
    maximum_target_case_lookups: usize,
    maximum_target_group_lookups: usize,
    maximum_target_case_ordinal_comparisons: usize,
    maximum_target_geometry_reference_comparisons: usize,
    target_handle_work: usize,
    target_resolution_scope_bytes: usize,
    target_resolution_work: usize,
    target_witnesses: usize,
    matching_target_references: usize,
    affine_operations: usize,
    affine_integer_bit_work: usize,
    maximum_affine_integer_bits: usize,
    transformed_constant_entries: usize,
    transformed_integer_bit_envelope: usize,
    transformed_integer_heap_byte_envelope: usize,
    target_comparison_entries: usize,
    target_comparison_integer_bit_work: usize,
    recentering_attempts: usize,
    recentering_boundary_checks: usize,
    retained_shift_components: usize,
    row_label_bytes: usize,
    no_target_outcomes: usize,
    recentering_boundary_outcomes: usize,
    pending_outcomes: usize,
    targets_consumed: usize,
    recenter_terms: usize,
    recenter_guards: usize,
    recenter_translation_components: usize,
    recenter_key_subtraction_boundary_checks: usize,
    recenter_source_terms: usize,
    recenter_source_exponent_entries: usize,
    recenter_output_terms: usize,
    recenter_output_exponent_entries: usize,
    recenter_power_operations: usize,
    recenter_integer_bit_work: usize,
    recenter_normalized_coefficient_terms: usize,
    recenter_retained_bytes: usize,
    owner_retained_bytes: usize,
    peak_scratch_bytes: usize,
    replay_combined_matcher_owner_bytes: usize,
    payload_comparison_units: usize,
    payload_comparison_bytes: usize,
    payload_comparison_integer_bits: usize,
    payload_comparison_relation_manifest_bytes: usize,
}

macro_rules! stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualCasePivotTargetMatchingStats {
    stats_getters!(
        scope_comparison_bytes,
        reelimination_replays,
        source_case_authentications,
        group_authentications,
        pivots,
        ambient_arity,
        free_positions,
        compact_matrix_entries,
        group_targets,
        same_group_scope_comparison_bytes,
        same_group_case_lookups,
        same_group_group_lookups,
        same_group_ordinal_comparisons,
        same_group_shape_comparisons,
        same_group_target_case_references,
        target_checks,
        maximum_target_position_lookups,
        maximum_target_handle_case_lookups,
        maximum_target_anchor_offset_lookups,
        maximum_target_handle_ordinal_comparisons,
        maximum_target_case_scope_comparison_bytes,
        maximum_target_authority_allocation_comparisons,
        maximum_target_case_lookups,
        maximum_target_group_lookups,
        maximum_target_case_ordinal_comparisons,
        maximum_target_geometry_reference_comparisons,
        target_handle_work,
        target_resolution_scope_bytes,
        target_resolution_work,
        target_witnesses,
        matching_target_references,
        affine_operations,
        affine_integer_bit_work,
        maximum_affine_integer_bits,
        transformed_constant_entries,
        transformed_integer_bit_envelope,
        transformed_integer_heap_byte_envelope,
        target_comparison_entries,
        target_comparison_integer_bit_work,
        recentering_attempts,
        recentering_boundary_checks,
        retained_shift_components,
        row_label_bytes,
        no_target_outcomes,
        recentering_boundary_outcomes,
        pending_outcomes,
        targets_consumed,
        recenter_terms,
        recenter_guards,
        recenter_translation_components,
        recenter_key_subtraction_boundary_checks,
        recenter_source_terms,
        recenter_source_exponent_entries,
        recenter_output_terms,
        recenter_output_exponent_entries,
        recenter_power_operations,
        recenter_integer_bit_work,
        recenter_normalized_coefficient_terms,
        recenter_retained_bytes,
        owner_retained_bytes,
        peak_scratch_bytes,
        replay_combined_matcher_owner_bytes,
        payload_comparison_units,
        payload_comparison_bytes,
        payload_comparison_integer_bits,
        payload_comparison_relation_manifest_bytes,
    );
}

/// One persisted target position checked for one pivot. Lifetime-bound handles
/// are never retained; this locator pair is sufficient to mint and
/// authenticate the same target again under the certificate's authority.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCasePivotTargetWitness {
    target_position: usize,
    case_ordinal: usize,
    terminal_locator: GeneratedAffineResidualInventoryTerminalLocator,
    matched: bool,
}

impl fmt::Debug for GeneratedAffineResidualCasePivotTargetWitness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCasePivotTargetWitness")
            .field("target_position", &self.target_position)
            .field("case_ordinal", &self.case_ordinal)
            .field("matched", &self.matched)
            .field("private_terminal_locator", &"<redacted>")
            .finish()
    }
}

impl GeneratedAffineResidualCasePivotTargetWitness {
    pub(crate) const fn target_position(&self) -> usize {
        self.target_position
    }
    pub(crate) const fn case_ordinal(&self) -> usize {
        self.case_ordinal
    }
    pub(crate) const fn terminal_locator(&self) -> GeneratedAffineResidualInventoryTerminalLocator {
        self.terminal_locator
    }
    pub(crate) const fn matched(&self) -> bool {
        self.matched
    }
}

#[derive(Clone)]
struct PivotTargetTranscript {
    pivot_ordinal: usize,
    pivot: IndexShift,
    transformed_target_constants: Vec<Integer>,
    target_witnesses: Vec<GeneratedAffineResidualCasePivotTargetWitness>,
}

impl fmt::Debug for PivotTargetTranscript {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PivotTargetTranscript")
            .field("pivot_ordinal", &self.pivot_ordinal)
            .field("pivot_arity", &self.pivot.arity())
            .field(
                "transformed_constant_count",
                &self.transformed_target_constants.len(),
            )
            .field("target_witness_count", &self.target_witnesses.len())
            .field("matching_target_count", &self.matching_target_count())
            .field("private_pivot", &"<redacted>")
            .field("private_constants", &"<redacted>")
            .finish()
    }
}

impl PivotTargetTranscript {
    fn matching_target_count(&self) -> usize {
        self.target_witnesses
            .iter()
            .filter(|witness| witness.matched)
            .count()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCaseRecenteringBoundaryKind {
    FreeCoefficientTranslationNegation,
    IntegralKeySubtraction,
}

#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualCaseRejectedNoTarget {
    transcript: PivotTargetTranscript,
}

#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualCaseRejectedRecenteringBoundary {
    transcript: PivotTargetTranscript,
    kind: GeneratedAffineResidualCaseRecenteringBoundaryKind,
    position: usize,
}

#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualCasePendingPivotTargetMatch {
    transcript: PivotTargetTranscript,
    coefficient_translation: IndexShift,
    key_center: IndexShift,
    relation: Arc<ParametricRelation>,
    recentering_stats: ParametricAffineFreeRecenteringStats,
}

macro_rules! transcript_accessors {
    () => {
        pub(crate) const fn pivot_ordinal(&self) -> usize {
            self.transcript.pivot_ordinal
        }
        pub(crate) const fn pivot(&self) -> &IndexShift {
            &self.transcript.pivot
        }
        pub(crate) fn transformed_target_constants(&self) -> &[Integer] {
            &self.transcript.transformed_target_constants
        }
        pub(crate) fn target_witnesses(&self) -> &[GeneratedAffineResidualCasePivotTargetWitness] {
            &self.transcript.target_witnesses
        }
        pub(crate) fn matching_target_count(&self) -> usize {
            self.transcript.matching_target_count()
        }
    };
}

impl GeneratedAffineResidualCaseRejectedNoTarget {
    transcript_accessors!();
}

impl GeneratedAffineResidualCaseRejectedRecenteringBoundary {
    transcript_accessors!();
    pub(crate) const fn kind(&self) -> GeneratedAffineResidualCaseRecenteringBoundaryKind {
        self.kind
    }
    pub(crate) const fn position(&self) -> usize {
        self.position
    }
}

impl GeneratedAffineResidualCasePendingPivotTargetMatch {
    transcript_accessors!();
    pub(crate) const fn coefficient_translation(&self) -> &IndexShift {
        &self.coefficient_translation
    }
    pub(crate) const fn key_center(&self) -> &IndexShift {
        &self.key_center
    }
    pub(crate) fn recentered_term_count(&self) -> usize {
        self.relation.terms().len()
    }
    pub(crate) fn recentered_guard_count(&self) -> usize {
        self.relation.guarded_nonzero_conditions().len()
    }
    pub(crate) const fn is_applicable_rule(&self) -> bool {
        false
    }
    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }
    pub(crate) const fn relation_for_future_when_bad(&self) -> &Arc<ParametricRelation> {
        &self.relation
    }
}

impl fmt::Debug for GeneratedAffineResidualCaseRejectedNoTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseRejectedNoTarget")
            .field("transcript", &self.transcript)
            .field("unresolved", &true)
            .finish()
    }
}

impl fmt::Debug for GeneratedAffineResidualCaseRejectedRecenteringBoundary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseRejectedRecenteringBoundary")
            .field("transcript", &self.transcript)
            .field("kind", &self.kind)
            .field("position", &self.position)
            .field("unresolved", &true)
            .finish()
    }
}

impl fmt::Debug for GeneratedAffineResidualCasePendingPivotTargetMatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCasePendingPivotTargetMatch")
            .field("transcript", &self.transcript)
            .field("recentered_term_count", &self.relation.terms().len())
            .field(
                "recentered_guard_count",
                &self.relation.guarded_nonzero_conditions().len(),
            )
            .field("private_translation", &"<redacted>")
            .field("private_relation", &"<redacted>")
            .field("applicable_rule", &false)
            .field("targets_consumed", &0)
            .finish()
    }
}

#[derive(Clone)]
pub(crate) enum GeneratedAffineResidualCasePivotTargetOutcome {
    RejectedNoTarget(GeneratedAffineResidualCaseRejectedNoTarget),
    RejectedRecenteringBoundary(GeneratedAffineResidualCaseRejectedRecenteringBoundary),
    Pending(GeneratedAffineResidualCasePendingPivotTargetMatch),
}

impl GeneratedAffineResidualCasePivotTargetOutcome {
    pub(crate) const fn pivot_ordinal(&self) -> usize {
        match self {
            Self::RejectedNoTarget(value) => value.pivot_ordinal(),
            Self::RejectedRecenteringBoundary(value) => value.pivot_ordinal(),
            Self::Pending(value) => value.pivot_ordinal(),
        }
    }
    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }
}

impl fmt::Debug for GeneratedAffineResidualCasePivotTargetOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RejectedNoTarget(value) => formatter
                .debug_tuple("RejectedNoTarget")
                .field(value)
                .finish(),
            Self::RejectedRecenteringBoundary(value) => formatter
                .debug_tuple("RejectedRecenteringBoundary")
                .field(value)
                .finish(),
            Self::Pending(value) => formatter.debug_tuple("Pending").field(value).finish(),
        }
    }
}

/// Complete pre-`WhenBad` transcript for every pivot of one exact case
/// re-elimination. The sole owning parent is `reelimination`; all four of its
/// parents and the source inventory remain transitive.
#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualCasePivotTargetMatchingCertificate {
    schema: &'static str,
    reelimination: Arc<GeneratedAffineResidualCaseReeliminationCertificate>,
    source_case_ordinal: usize,
    source_group_ordinal: usize,
    outcomes: Arc<Vec<GeneratedAffineResidualCasePivotTargetOutcome>>,
    limits: GeneratedAffineResidualCasePivotTargetMatchingLimits,
    stats: GeneratedAffineResidualCasePivotTargetMatchingStats,
}

impl fmt::Debug for GeneratedAffineResidualCasePivotTargetMatchingCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCasePivotTargetMatchingCertificate")
            .field("schema", &self.schema)
            .field("source_case_ordinal", &self.source_case_ordinal)
            .field("source_group_ordinal", &self.source_group_ordinal)
            .field("outcome_count", &self.outcomes.len())
            .field("stats", &self.stats)
            .field("private_reelimination", &"<redacted>")
            .field("private_outcomes", &"<redacted>")
            .field("targets_consumed", &0)
            .field("applicable_rules", &0)
            .finish()
    }
}

impl GeneratedAffineResidualCasePivotTargetMatchingCertificate {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) const fn reelimination(
        &self,
    ) -> &Arc<GeneratedAffineResidualCaseReeliminationCertificate> {
        &self.reelimination
    }
    pub(crate) const fn source_case_ordinal(&self) -> usize {
        self.source_case_ordinal
    }
    pub(crate) const fn source_group_ordinal(&self) -> usize {
        self.source_group_ordinal
    }
    pub(crate) fn outcomes(&self) -> &[GeneratedAffineResidualCasePivotTargetOutcome] {
        self.outcomes.as_slice()
    }
    pub(crate) const fn limits(&self) -> GeneratedAffineResidualCasePivotTargetMatchingLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualCasePivotTargetMatchingStats {
        self.stats
    }
    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }
    pub(crate) const fn publishes_rules(&self) -> bool {
        false
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        reelimination: &Arc<GeneratedAffineResidualCaseReeliminationCertificate>,
        replay_limits: GeneratedAffineResidualCasePivotTargetMatchingReplayLimits,
    ) -> Result<(), GeneratedAffineResidualCasePivotTargetMatchingError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != GENERATED_AFFINE_RESIDUAL_CASE_PIVOT_TARGET_MATCHING_V2_SCHEMA {
                return Err(GeneratedAffineResidualCasePivotTargetMatchingError::SchemaMismatch);
            }
            check_limit(
                "pivot-target re-elimination allocation comparisons",
                REELIMINATION_ALLOCATION_COMPARISONS,
                replay_limits.max_reelimination_allocation_comparisons,
            )?;
            if !Arc::ptr_eq(&self.reelimination, reelimination) {
                return Err(
                    GeneratedAffineResidualCasePivotTargetMatchingError::WrongReeliminationAllocation,
                );
            }
            check_limit(
                "pivot-target replay combined matcher-owner bytes",
                self.stats.replay_combined_matcher_owner_bytes,
                replay_limits.max_combined_matcher_owner_bytes,
            )?;
            let prepared_payload = stored_payload_census(self.stats);
            admit_payload_census(prepared_payload, replay_limits)?;
            let scope_comparison_bytes =
                validate_parent_and_scope(family, context, &self.reelimination, self.limits)?;
            replay_reelimination(family, context, &self.reelimination)?;
            let replayed = compile_replayed(
                family,
                context,
                Arc::clone(&self.reelimination),
                self.limits,
                scope_comparison_bytes,
                Some(prepared_payload),
            )?;
            if payload_eq(self, &replayed, prepared_payload)? {
                Ok(())
            } else {
                Err(GeneratedAffineResidualCasePivotTargetMatchingError::ReplayMismatch)
            }
        }))
        .map_err(|_| GeneratedAffineResidualCasePivotTargetMatchingError::SymbolicaPanic)?
    }
}

pub(crate) struct GeneratedAffineResidualCasePivotTargetMatchingCompiler;

impl GeneratedAffineResidualCasePivotTargetMatchingCompiler {
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        reelimination: Arc<GeneratedAffineResidualCaseReeliminationCertificate>,
        limits: GeneratedAffineResidualCasePivotTargetMatchingLimits,
    ) -> Result<
        GeneratedAffineResidualCasePivotTargetMatchingCertificate,
        GeneratedAffineResidualCasePivotTargetMatchingError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            let scope_comparison_bytes =
                validate_parent_and_scope(family, context, &reelimination, limits)?;
            replay_reelimination(family, context, &reelimination)?;
            compile_replayed(
                family,
                context,
                reelimination,
                limits,
                scope_comparison_bytes,
                None,
            )
        }))
        .map_err(|_| GeneratedAffineResidualCasePivotTargetMatchingError::SymbolicaPanic)?
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCasePivotTargetMatchingError {
    SchemaMismatch,
    ReplayMismatch,
    WrongFamily,
    WrongContext,
    WrongArity,
    WrongReeliminationAllocation,
    WrongCaseBinding,
    WrongGroupBinding,
    MalformedGeometry,
    PivotOrdinalMismatch,
    PivotArityMismatch,
    Authority,
    Reelimination,
    Relation,
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
    },
    SymbolicaPanic,
}

impl GeneratedAffineResidualCasePivotTargetMatchingError {
    const fn kind(self) -> &'static str {
        match self {
            Self::SchemaMismatch => "SchemaMismatch",
            Self::ReplayMismatch => "ReplayMismatch",
            Self::WrongFamily => "WrongFamily",
            Self::WrongContext => "WrongContext",
            Self::WrongArity => "WrongArity",
            Self::WrongReeliminationAllocation => "WrongReeliminationAllocation",
            Self::WrongCaseBinding => "WrongCaseBinding",
            Self::WrongGroupBinding => "WrongGroupBinding",
            Self::MalformedGeometry => "MalformedGeometry",
            Self::PivotOrdinalMismatch => "PivotOrdinalMismatch",
            Self::PivotArityMismatch => "PivotArityMismatch",
            Self::Authority => "Authority",
            Self::Reelimination => "Reelimination",
            Self::Relation => "Relation",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::AllocationFailure { .. } => "AllocationFailure",
            Self::SymbolicaPanic => "SymbolicaPanic",
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualCasePivotTargetMatchingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCasePivotTargetMatchingError")
            .field("kind", &self.kind())
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualCasePivotTargetMatchingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "generated affine case pivot-target matching {}",
            self.kind()
        )
    }
}

impl std::error::Error for GeneratedAffineResidualCasePivotTargetMatchingError {}

fn map_authority_error(
    error: GeneratedAffineResidualCaseAuthorityError,
) -> GeneratedAffineResidualCasePivotTargetMatchingError {
    match error {
        GeneratedAffineResidualCaseAuthorityError::ResourceLimit {
            resource,
            requested,
            limit,
        } => GeneratedAffineResidualCasePivotTargetMatchingError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow { resource } => {
            GeneratedAffineResidualCasePivotTargetMatchingError::ResourceCountOverflow { resource }
        }
        GeneratedAffineResidualCaseAuthorityError::SymbolicaPanic => {
            GeneratedAffineResidualCasePivotTargetMatchingError::SymbolicaPanic
        }
        _ => GeneratedAffineResidualCasePivotTargetMatchingError::Authority,
    }
}

fn replay_reelimination(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    reelimination: &Arc<GeneratedAffineResidualCaseReeliminationCertificate>,
) -> Result<(), GeneratedAffineResidualCasePivotTargetMatchingError> {
    reelimination
        .replay(
            family,
            context,
            reelimination.authority(),
            reelimination.premises(),
            reelimination.ordering(),
            reelimination.schedule(),
        )
        .map_err(|_: GeneratedAffineResidualCaseReeliminationError| {
            GeneratedAffineResidualCasePivotTargetMatchingError::Reelimination
        })
}

fn validate_parent_and_scope(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    reelimination: &Arc<GeneratedAffineResidualCaseReeliminationCertificate>,
    limits: GeneratedAffineResidualCasePivotTargetMatchingLimits,
) -> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError> {
    if reelimination.schema() != GENERATED_AFFINE_RESIDUAL_CASE_REELIMINATION_V2_SCHEMA {
        return Err(GeneratedAffineResidualCasePivotTargetMatchingError::SchemaMismatch);
    }
    check_limit(
        "pivot-target re-elimination replays",
        REELIMINATION_REPLAYS,
        limits.max_reelimination_replays,
    )?;
    check_limit(
        "pivot-target source-case authentications",
        SOURCE_CASE_AUTHENTICATIONS,
        limits.max_source_case_authentications,
    )?;
    check_limit(
        "pivot-target group authentications",
        GROUP_AUTHENTICATIONS,
        limits.max_group_authentications,
    )?;
    let authority = reelimination.authority();
    let scope_comparison_bytes = checked_sum(
        "pivot-target scope comparison bytes",
        [
            family.fingerprint_ref().len(),
            authority.family_fingerprint().len(),
            context.fingerprint().len(),
            authority.context_fingerprint().len(),
        ],
    )?;
    check_limit(
        "pivot-target scope comparison bytes",
        scope_comparison_bytes,
        limits.max_scope_comparison_bytes,
    )?;
    if family.fingerprint_ref() != authority.family_fingerprint() {
        return Err(GeneratedAffineResidualCasePivotTargetMatchingError::WrongFamily);
    }
    if context.fingerprint() != authority.context_fingerprint() {
        return Err(GeneratedAffineResidualCasePivotTargetMatchingError::WrongContext);
    }
    if context.index_count() != authority.arity() {
        return Err(GeneratedAffineResidualCasePivotTargetMatchingError::WrongArity);
    }
    Ok(scope_comparison_bytes)
}

fn compile_replayed(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    reelimination: Arc<GeneratedAffineResidualCaseReeliminationCertificate>,
    limits: GeneratedAffineResidualCasePivotTargetMatchingLimits,
    scope_comparison_bytes: usize,
    admitted_replay_payload: Option<PayloadCensus>,
) -> Result<
    GeneratedAffineResidualCasePivotTargetMatchingCertificate,
    GeneratedAffineResidualCasePivotTargetMatchingError,
> {
    let authority = reelimination.authority();
    let source_case = authority
        .authenticated_case_view(context)
        .map_err(map_authority_error)?;
    let group = authority
        .authenticated_group_view(context)
        .map_err(map_authority_error)?;
    if source_case.ordinal() != authority.case_ordinal()
        || source_case.group_ordinal() != authority.group_ordinal()
        || group.ordinal() != authority.group_ordinal()
    {
        return Err(GeneratedAffineResidualCasePivotTargetMatchingError::WrongCaseBinding);
    }
    let ambient_arity = group.ambient_arity();
    let free_positions = group.free_positions();
    let compact_matrix = group.compact_linear_coefficients();
    let compact_matrix_entries = checked_mul(
        "pivot-target compact matrix entries",
        ambient_arity,
        free_positions.len(),
    )?;
    if ambient_arity != context.index_count()
        || source_case.constants().len() != ambient_arity
        || compact_matrix.len() != compact_matrix_entries
        || free_positions
            .iter()
            .any(|&position| position >= ambient_arity)
    {
        return Err(GeneratedAffineResidualCasePivotTargetMatchingError::MalformedGeometry);
    }
    for (resource, requested, limit) in [
        (
            "pivot-target ambient arity",
            ambient_arity,
            limits.max_ambient_arity,
        ),
        (
            "pivot-target free positions",
            free_positions.len(),
            limits.max_free_positions,
        ),
        (
            "pivot-target compact matrix entries",
            compact_matrix_entries,
            limits.max_compact_matrix_entries,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }

    let pivots = reelimination
        .elimination_for_case_target_matching()
        .pivots();
    check_limit("pivot-target pivots", pivots.len(), limits.max_pivots)?;
    let targets = authority
        .same_group_target_cases(family, context, limits.same_group_targets)
        .map_err(map_authority_error)?;
    if targets.source_case_ordinal() != source_case.ordinal()
        || targets.group_ordinal() != group.ordinal()
        || targets.is_empty()
        || targets.len() != group.case_ordinals().len()
    {
        return Err(GeneratedAffineResidualCasePivotTargetMatchingError::WrongGroupBinding);
    }
    check_limit(
        "pivot-target group targets",
        targets.len(),
        limits.max_group_targets,
    )?;
    let same_group_stats = targets.stats();

    let target_checks = checked_mul("pivot-target checks", pivots.len(), targets.len())?;
    check_limit(
        "pivot-target checks",
        target_checks,
        limits.max_target_checks,
    )?;
    check_limit(
        "pivot-target witnesses",
        target_checks,
        limits.max_target_witnesses,
    )?;
    // The borrowed handle and resolver APIs have fixed wrapper work. Their
    // nested authentication is sealed by the transitive case authority.
    const HANDLE_WORK_PER_CALL: usize = 1 + 1 + 1 + 3;
    const RESOLUTION_WORK_PER_CALL: usize = 1 + 1 + 1 + 6 + 5;
    let target_handle_work = checked_mul(
        "pivot-target handle work",
        target_checks,
        HANDLE_WORK_PER_CALL,
    )?;
    check_limit(
        "pivot-target handle work",
        target_handle_work,
        limits.max_target_handle_work,
    )?;
    let resolver_scope_per_call = checked_sum(
        "pivot-target resolution scope bytes",
        [
            family.fingerprint_ref().len(),
            authority.family_fingerprint().len(),
            context.fingerprint().len(),
            authority.context_fingerprint().len(),
        ],
    )?;
    let target_resolution_scope_bytes = checked_mul(
        "pivot-target resolution scope bytes",
        target_checks,
        resolver_scope_per_call,
    )?;
    check_limit(
        "pivot-target resolution scope bytes",
        target_resolution_scope_bytes,
        limits.max_target_resolution_scope_bytes,
    )?;
    let target_resolution_work = checked_mul(
        "pivot-target resolution work",
        target_checks,
        RESOLUTION_WORK_PER_CALL,
    )?;
    check_limit(
        "pivot-target resolution work",
        target_resolution_work,
        limits.max_target_resolution_work,
    )?;
    let target_comparison_entries = checked_mul(
        "pivot-target comparison entries",
        target_checks,
        ambient_arity,
    )?;
    check_limit(
        "pivot-target comparison entries",
        target_comparison_entries,
        limits.max_target_comparison_entries,
    )?;

    let mut stats = GeneratedAffineResidualCasePivotTargetMatchingStats {
        scope_comparison_bytes,
        reelimination_replays: REELIMINATION_REPLAYS,
        source_case_authentications: SOURCE_CASE_AUTHENTICATIONS,
        group_authentications: GROUP_AUTHENTICATIONS,
        pivots: pivots.len(),
        ambient_arity,
        free_positions: free_positions.len(),
        compact_matrix_entries,
        group_targets: targets.len(),
        same_group_scope_comparison_bytes: same_group_stats.scope_comparison_bytes(),
        same_group_case_lookups: same_group_stats.case_lookups(),
        same_group_group_lookups: same_group_stats.group_lookups(),
        same_group_ordinal_comparisons: same_group_stats.ordinal_comparisons(),
        same_group_shape_comparisons: same_group_stats.shape_comparisons(),
        same_group_target_case_references: same_group_stats.target_case_references(),
        target_checks,
        target_handle_work,
        target_resolution_scope_bytes,
        target_resolution_work,
        target_witnesses: target_checks,
        target_comparison_entries,
        ..Default::default()
    };

    let outcome_buffer_bytes = checked_mul(
        "pivot-target owner retained bytes",
        pivots.len(),
        size_of::<GeneratedAffineResidualCasePivotTargetOutcome>(),
    )?;
    let owner_base_bytes = checked_add(
        "pivot-target owner retained bytes",
        outcome_buffer_bytes,
        arc_pointee_byte_bound::<Vec<GeneratedAffineResidualCasePivotTargetOutcome>>()?,
    )?;
    let mut admission = RetainedAdmission::new(owner_base_bytes, limits.max_owner_retained_bytes)?;
    let mut outcomes = Vec::new();
    try_reserve_exact("pivot-target outcomes", &mut outcomes, pivots.len())?;

    for (expected_pivot_ordinal, pivot_equation) in pivots.iter().enumerate() {
        if pivot_equation.ordinal() != expected_pivot_ordinal {
            return Err(GeneratedAffineResidualCasePivotTargetMatchingError::PivotOrdinalMismatch);
        }
        let pivot = pivot_equation.pivot();
        if pivot.arity() != ambient_arity {
            return Err(GeneratedAffineResidualCasePivotTargetMatchingError::PivotArityMismatch);
        }
        let prepared = prepare_transformed_constants(
            source_case.constants(),
            compact_matrix,
            free_positions,
            pivot,
        )?;
        admit_prepared_transform(&mut stats, prepared, limits)?;
        observe_peak_scratch(&mut stats, prepared.peak_scratch_bytes, limits)?;

        let pivot_bytes = checked_mul(
            "pivot-target owner retained bytes",
            ambient_arity,
            size_of::<i64>(),
        )?;
        let transformed_storage_bytes = checked_add(
            "pivot-target owner retained bytes",
            checked_mul(
                "pivot-target owner retained bytes",
                ambient_arity,
                size_of::<Integer>(),
            )?,
            prepared.integer_heap_byte_envelope,
        )?;
        let target_witness_bytes = checked_mul(
            "pivot-target owner retained bytes",
            targets.len(),
            size_of::<GeneratedAffineResidualCasePivotTargetWitness>(),
        )?;
        admission.admit(checked_sum(
            "pivot-target owner retained bytes",
            [pivot_bytes, transformed_storage_bytes, target_witness_bytes],
        )?)?;
        stats.retained_shift_components = bounded_add(
            "pivot-target retained shift components",
            stats.retained_shift_components,
            ambient_arity,
            limits.max_retained_shift_components,
        )?;

        let pivot_copy = copy_shift(pivot)?;
        let transformed = execute_transformed_constants(
            source_case.constants(),
            compact_matrix,
            free_positions,
            pivot,
            prepared,
        )?;
        let mut target_witnesses = Vec::new();
        try_reserve_exact(
            "pivot-target witnesses",
            &mut target_witnesses,
            targets.len(),
        )?;
        let mut matching_count = 0usize;
        for target_position in 0..targets.len() {
            let handle = targets
                .target(target_position, limits.target_handle)
                .map_err(map_authority_error)?;
            let handle_stats = handle.stats();
            stats.maximum_target_position_lookups = stats
                .maximum_target_position_lookups
                .max(handle_stats.target_position_lookups());
            stats.maximum_target_handle_case_lookups = stats
                .maximum_target_handle_case_lookups
                .max(handle_stats.case_lookups());
            stats.maximum_target_anchor_offset_lookups = stats
                .maximum_target_anchor_offset_lookups
                .max(handle_stats.anchor_offset_lookups());
            stats.maximum_target_handle_ordinal_comparisons = stats
                .maximum_target_handle_ordinal_comparisons
                .max(handle_stats.ordinal_comparisons());
            if checked_sum(
                "pivot-target handle work",
                [
                    handle_stats.target_position_lookups(),
                    handle_stats.case_lookups(),
                    handle_stats.anchor_offset_lookups(),
                    handle_stats.ordinal_comparisons(),
                ],
            )? != HANDLE_WORK_PER_CALL
            {
                return Err(GeneratedAffineResidualCasePivotTargetMatchingError::ReplayMismatch);
            }
            let resolved = authority
                .authenticated_same_group_target_case_view(
                    family,
                    context,
                    handle,
                    limits.target_case,
                )
                .map_err(map_authority_error)?;
            let resolution_stats = resolved.stats();
            stats.maximum_target_case_scope_comparison_bytes = stats
                .maximum_target_case_scope_comparison_bytes
                .max(resolution_stats.scope_comparison_bytes());
            stats.maximum_target_authority_allocation_comparisons = stats
                .maximum_target_authority_allocation_comparisons
                .max(resolution_stats.authority_allocation_comparisons());
            stats.maximum_target_case_lookups = stats
                .maximum_target_case_lookups
                .max(resolution_stats.case_lookups());
            stats.maximum_target_group_lookups = stats
                .maximum_target_group_lookups
                .max(resolution_stats.group_lookups());
            stats.maximum_target_case_ordinal_comparisons = stats
                .maximum_target_case_ordinal_comparisons
                .max(resolution_stats.ordinal_comparisons());
            stats.maximum_target_geometry_reference_comparisons = stats
                .maximum_target_geometry_reference_comparisons
                .max(resolution_stats.geometry_reference_comparisons());
            if resolution_stats.scope_comparison_bytes() != resolver_scope_per_call
                || checked_sum(
                    "pivot-target resolution work",
                    [
                        resolution_stats.authority_allocation_comparisons(),
                        resolution_stats.case_lookups(),
                        resolution_stats.group_lookups(),
                        resolution_stats.ordinal_comparisons(),
                        resolution_stats.geometry_reference_comparisons(),
                    ],
                )? != RESOLUTION_WORK_PER_CALL
            {
                return Err(GeneratedAffineResidualCasePivotTargetMatchingError::ReplayMismatch);
            }
            let target = resolved.target();
            if handle.ordinal_within_group() != target_position
                || handle.case_ordinal() != target.ordinal()
                || target.group_ordinal() != group.ordinal()
                || target.constants().len() != ambient_arity
            {
                return Err(GeneratedAffineResidualCasePivotTargetMatchingError::WrongGroupBinding);
            }
            let comparison_work = integer_comparison_bit_work(&transformed, target.constants())?;
            stats.target_comparison_integer_bit_work = bounded_add(
                "pivot-target comparison integer-bit work",
                stats.target_comparison_integer_bit_work,
                comparison_work,
                limits.max_target_comparison_integer_bit_work,
            )?;
            let matched = transformed.as_slice() == target.constants();
            if matched {
                matching_count =
                    checked_add("pivot-target matching target references", matching_count, 1)?;
                stats.matching_target_references = bounded_add(
                    "pivot-target matching target references",
                    stats.matching_target_references,
                    1,
                    limits.max_matching_target_references,
                )?;
            }
            target_witnesses.push(GeneratedAffineResidualCasePivotTargetWitness {
                target_position,
                case_ordinal: target.ordinal(),
                terminal_locator: target.locator(),
                matched,
            });
        }
        if target_witnesses.len() != targets.len() {
            return Err(GeneratedAffineResidualCasePivotTargetMatchingError::ReplayMismatch);
        }

        let transcript = PivotTargetTranscript {
            pivot_ordinal: pivot_equation.ordinal(),
            pivot: pivot_copy,
            transformed_target_constants: transformed,
            target_witnesses,
        };
        if matching_count == 0 {
            stats.no_target_outcomes = bounded_add(
                "pivot-target no-target outcomes",
                stats.no_target_outcomes,
                1,
                limits.max_no_target_outcomes,
            )?;
            outcomes.push(
                GeneratedAffineResidualCasePivotTargetOutcome::RejectedNoTarget(
                    GeneratedAffineResidualCaseRejectedNoTarget { transcript },
                ),
            );
            continue;
        }
        if transcript.matching_target_count() != matching_count {
            return Err(GeneratedAffineResidualCasePivotTargetMatchingError::ReplayMismatch);
        }

        if let Some((position, kind)) = classify_recentering_boundary(
            &mut stats,
            pivot_equation.unit_relation(),
            free_positions,
            pivot,
            limits,
        )? {
            stats.recentering_boundary_outcomes = bounded_add(
                "pivot-target recentering-boundary outcomes",
                stats.recentering_boundary_outcomes,
                1,
                limits.max_recentering_boundary_outcomes,
            )?;
            outcomes.push(
                GeneratedAffineResidualCasePivotTargetOutcome::RejectedRecenteringBoundary(
                    GeneratedAffineResidualCaseRejectedRecenteringBoundary {
                        transcript,
                        kind,
                        position,
                    },
                ),
            );
            continue;
        }

        let additional_shift_components =
            checked_mul("pivot-target retained shift components", ambient_arity, 2)?;
        stats.retained_shift_components = bounded_add(
            "pivot-target retained shift components",
            stats.retained_shift_components,
            additional_shift_components,
            limits.max_retained_shift_components,
        )?;
        admission.admit(checked_mul(
            "pivot-target owner retained bytes",
            additional_shift_components,
            size_of::<i64>(),
        )?)?;

        let row_label_bytes =
            pending_row_label_byte_len(source_case.ordinal(), pivot_equation.ordinal())?;
        stats.row_label_bytes = bounded_add(
            "pivot-target row-label bytes",
            stats.row_label_bytes,
            row_label_bytes,
            limits.max_row_label_bytes,
        )?;
        let external_relation_bytes = pending_external_relation_allocation_byte_bound(
            context.fingerprint().len(),
            row_label_bytes,
        )?;
        admission.admit(external_relation_bytes)?;
        observe_peak_scratch(
            &mut stats,
            checked_mul("pivot-target peak scratch bytes", row_label_bytes, 2)?,
            limits,
        )?;

        let coefficient_translation = coefficient_translation(free_positions, pivot)?;
        let key_center = copy_shift(pivot)?;
        let row_id = pending_row_id(
            source_case.ordinal(),
            pivot_equation.ordinal(),
            row_label_bytes,
        )?;
        let mut helper_limits = remaining_recentering_limits(stats, limits)?;
        let recentering_retained_remaining = helper_limits.max_retained_bytes;
        let owner_retained_remaining = admission.remaining();
        let retained_clamp = match owner_retained_remaining.cmp(&recentering_retained_remaining) {
            std::cmp::Ordering::Less => RecenteringRetainedClamp::Owner,
            std::cmp::Ordering::Equal => RecenteringRetainedClamp::Both,
            std::cmp::Ordering::Greater => RecenteringRetainedClamp::Recentering,
        };
        helper_limits.max_retained_bytes =
            recentering_retained_remaining.min(owner_retained_remaining);
        let recentered = pivot_equation.unit_relation().affine_free_recentered(
            context,
            &coefficient_translation,
            &key_center,
            row_id,
            helper_limits,
        );
        let (relation, recentering_stats) = match recentered {
            Ok(value) => value,
            Err(ParametricRelationError::IndexOverflow { .. }) => {
                return Err(GeneratedAffineResidualCasePivotTargetMatchingError::ReplayMismatch);
            }
            Err(error) => {
                return Err(map_recentering_error(
                    error,
                    stats,
                    limits,
                    admission.bytes(),
                    retained_clamp,
                ));
            }
        };
        admission.admit(recentering_stats.retained_bytes())?;
        let observed_relation_bytes = relation.owned_retained_byte_bound().ok_or(
            GeneratedAffineResidualCasePivotTargetMatchingError::ResourceCountOverflow {
                resource: "pivot-target observed recentered relation bytes",
            },
        )?;
        check_limit(
            "pivot-target observed recentered relation bytes",
            observed_relation_bytes,
            recentering_stats.retained_bytes(),
        )?;
        accumulate_recentering_stats(&mut stats, recentering_stats, limits)?;
        stats.pending_outcomes = bounded_add(
            "pivot-target pending outcomes",
            stats.pending_outcomes,
            1,
            limits.max_pending_outcomes,
        )?;
        outcomes.push(GeneratedAffineResidualCasePivotTargetOutcome::Pending(
            GeneratedAffineResidualCasePendingPivotTargetMatch {
                transcript,
                coefficient_translation,
                key_center,
                relation: Arc::new(relation),
                recentering_stats,
            },
        ));
    }

    if outcomes.len() != pivots.len() || stats.targets_consumed != 0 {
        return Err(GeneratedAffineResidualCasePivotTargetMatchingError::ReplayMismatch);
    }
    stats.owner_retained_bytes = admission.bytes();
    stats.replay_combined_matcher_owner_bytes = checked_add(
        "pivot-target replay combined matcher-owner bytes",
        checked_mul(
            "pivot-target replay combined matcher-owner bytes",
            stats.owner_retained_bytes,
            2,
        )?,
        stats.peak_scratch_bytes,
    )?;
    // A normal construction measures its future replay workload once.  A
    // replay has already admitted the retained O(1) census before any deep
    // work, so seed the rebuilt statistics from that census and let the one
    // bounded pair traversal below authenticate it.  Re-censusing here would
    // be an unaccounted third payload scan and would trust understated stored
    // statistics until after that scan had completed.
    let payload = match admitted_replay_payload {
        Some(payload) => payload,
        None => {
            let raw = payload_census_pair(&outcomes, &outcomes, unbounded_payload_replay_limits())?;
            doubled_payload_census(raw)?
        }
    };
    stats.payload_comparison_units = payload.units;
    stats.payload_comparison_bytes = payload.bytes;
    stats.payload_comparison_integer_bits = payload.integer_bits;
    stats.payload_comparison_relation_manifest_bytes = payload.relation_manifest_bytes;

    drop(targets);
    let source_case_ordinal = source_case.ordinal();
    let source_group_ordinal = group.ordinal();
    Ok(GeneratedAffineResidualCasePivotTargetMatchingCertificate {
        schema: GENERATED_AFFINE_RESIDUAL_CASE_PIVOT_TARGET_MATCHING_V2_SCHEMA,
        reelimination,
        source_case_ordinal,
        source_group_ordinal,
        outcomes: Arc::new(outcomes),
        limits,
        stats,
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PreparedTransformedConstants {
    affine_operations: usize,
    integer_bit_work: usize,
    maximum_integer_bits: usize,
    transformed_entries: usize,
    integer_bit_envelope: usize,
    integer_heap_byte_envelope: usize,
    peak_scratch_bytes: usize,
}

fn prepare_transformed_constants(
    source_constants: &[Integer],
    compact_linear_coefficients: &[Integer],
    free_positions: &[usize],
    pivot: &IndexShift,
) -> Result<PreparedTransformedConstants, GeneratedAffineResidualCasePivotTargetMatchingError> {
    let ambient_arity = source_constants.len();
    if pivot.arity() != ambient_arity
        || compact_linear_coefficients.len()
            != checked_mul(
                "pivot-target compact matrix entries",
                ambient_arity,
                free_positions.len(),
            )?
    {
        return Err(GeneratedAffineResidualCasePivotTargetMatchingError::MalformedGeometry);
    }
    let affine_operations = checked_mul(
        "pivot-target affine operations",
        ambient_arity,
        checked_add(
            "pivot-target affine operations",
            checked_mul("pivot-target affine operations", free_positions.len(), 2)?,
            1,
        )?,
    )?;
    let mut prepared = PreparedTransformedConstants {
        affine_operations,
        transformed_entries: ambient_arity,
        ..Default::default()
    };
    for row in 0..ambient_arity {
        let mut value_bits = integer_magnitude_bits(&source_constants[row])?;
        prepared.maximum_integer_bits = prepared.maximum_integer_bits.max(value_bits);
        prepared.integer_bit_work = checked_add(
            "pivot-target affine integer-bit work",
            prepared.integer_bit_work,
            value_bits.max(1),
        )?;
        for (free_ordinal, &free_position) in free_positions.iter().enumerate() {
            let coefficient =
                &compact_linear_coefficients[row * free_positions.len() + free_ordinal];
            let coefficient_bits = integer_magnitude_bits(coefficient)?;
            let coordinate_bits = i64_magnitude_bits(pivot.values()[free_position]);
            let product_bits = multiplication_bit_bound(coefficient_bits, coordinate_bits)?;
            prepared.maximum_integer_bits = prepared
                .maximum_integer_bits
                .max(coefficient_bits)
                .max(coordinate_bits)
                .max(product_bits);
            prepared.integer_bit_work = checked_add(
                "pivot-target affine integer-bit work",
                prepared.integer_bit_work,
                checked_sum(
                    "pivot-target affine integer-bit work",
                    [
                        coefficient_bits.max(1),
                        coordinate_bits.max(1),
                        product_bits.max(1),
                    ],
                )?,
            )?;
            let output_bits = addition_bit_bound(value_bits, product_bits)?;
            prepared.maximum_integer_bits = prepared.maximum_integer_bits.max(output_bits);
            prepared.integer_bit_work = checked_add(
                "pivot-target affine integer-bit work",
                prepared.integer_bit_work,
                checked_sum(
                    "pivot-target affine integer-bit work",
                    [value_bits.max(1), product_bits.max(1), output_bits.max(1)],
                )?,
            )?;
            prepared.peak_scratch_bytes =
                prepared
                    .peak_scratch_bytes
                    .max(prospective_integer_scratch_bytes([
                        value_bits,
                        product_bits,
                        output_bits,
                    ])?);
            value_bits = output_bits;
        }
        let pivot_bits = i64_magnitude_bits(pivot.values()[row]);
        let output_bits = addition_bit_bound(value_bits, pivot_bits)?;
        prepared.maximum_integer_bits = prepared
            .maximum_integer_bits
            .max(pivot_bits)
            .max(output_bits);
        prepared.integer_bit_work = checked_add(
            "pivot-target affine integer-bit work",
            prepared.integer_bit_work,
            checked_sum(
                "pivot-target affine integer-bit work",
                [value_bits.max(1), pivot_bits.max(1), output_bits.max(1)],
            )?,
        )?;
        prepared.peak_scratch_bytes =
            prepared
                .peak_scratch_bytes
                .max(prospective_integer_scratch_bytes([
                    value_bits,
                    pivot_bits,
                    output_bits,
                ])?);
        prepared.integer_bit_envelope = checked_add(
            "pivot-target transformed integer-bit envelope",
            prepared.integer_bit_envelope,
            output_bits.max(1),
        )?;
        prepared.integer_heap_byte_envelope = checked_add(
            "pivot-target transformed integer heap-byte envelope",
            prepared.integer_heap_byte_envelope,
            prospective_integer_heap_byte_bound(output_bits)?,
        )?;
    }
    Ok(prepared)
}

fn admit_prepared_transform(
    stats: &mut GeneratedAffineResidualCasePivotTargetMatchingStats,
    prepared: PreparedTransformedConstants,
    limits: GeneratedAffineResidualCasePivotTargetMatchingLimits,
) -> Result<(), GeneratedAffineResidualCasePivotTargetMatchingError> {
    stats.affine_operations = bounded_add(
        "pivot-target affine operations",
        stats.affine_operations,
        prepared.affine_operations,
        limits.max_affine_operations,
    )?;
    stats.affine_integer_bit_work = bounded_add(
        "pivot-target affine integer-bit work",
        stats.affine_integer_bit_work,
        prepared.integer_bit_work,
        limits.max_affine_integer_bit_work,
    )?;
    stats.maximum_affine_integer_bits = stats
        .maximum_affine_integer_bits
        .max(prepared.maximum_integer_bits);
    check_limit(
        "pivot-target maximum affine integer bits",
        stats.maximum_affine_integer_bits,
        limits.max_affine_integer_bits,
    )?;
    stats.transformed_constant_entries = bounded_add(
        "pivot-target transformed constant entries",
        stats.transformed_constant_entries,
        prepared.transformed_entries,
        limits.max_transformed_constant_entries,
    )?;
    stats.transformed_integer_bit_envelope = bounded_add(
        "pivot-target transformed integer-bit envelope",
        stats.transformed_integer_bit_envelope,
        prepared.integer_bit_envelope,
        limits.max_transformed_integer_bit_envelope,
    )?;
    stats.transformed_integer_heap_byte_envelope = bounded_add(
        "pivot-target transformed integer heap-byte envelope",
        stats.transformed_integer_heap_byte_envelope,
        prepared.integer_heap_byte_envelope,
        limits.max_transformed_integer_heap_byte_envelope,
    )?;
    Ok(())
}

fn execute_transformed_constants(
    source_constants: &[Integer],
    compact_linear_coefficients: &[Integer],
    free_positions: &[usize],
    pivot: &IndexShift,
    prepared: PreparedTransformedConstants,
) -> Result<Vec<Integer>, GeneratedAffineResidualCasePivotTargetMatchingError> {
    let mut transformed = Vec::new();
    try_reserve_exact(
        "pivot-target transformed constants",
        &mut transformed,
        source_constants.len(),
    )?;
    let mut observed_heap_bytes = 0usize;
    for row in 0..source_constants.len() {
        let mut value = source_constants[row].clone();
        for (free_ordinal, &free_position) in free_positions.iter().enumerate() {
            let coefficient =
                &compact_linear_coefficients[row * free_positions.len() + free_ordinal];
            let product = coefficient * Integer::from(pivot.values()[free_position]);
            value = value - product;
        }
        value = value + Integer::from(pivot.values()[row]);
        check_limit(
            "pivot-target observed transformed integer bits",
            integer_magnitude_bits(&value)?,
            prepared.maximum_integer_bits,
        )?;
        observed_heap_bytes = checked_add(
            "pivot-target observed transformed integer heap bytes",
            observed_heap_bytes,
            integer_owned_heap_byte_bound(&value)?,
        )?;
        transformed.push(value);
    }
    check_limit(
        "pivot-target observed transformed integer heap bytes",
        observed_heap_bytes,
        prepared.integer_heap_byte_envelope,
    )?;
    Ok(transformed)
}

fn multiplication_bit_bound(
    left_bits: usize,
    right_bits: usize,
) -> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError> {
    if left_bits == 0 || right_bits == 0 {
        Ok(0)
    } else {
        checked_add(
            "pivot-target multiplication integer bits",
            left_bits,
            right_bits,
        )
    }
}

fn addition_bit_bound(
    left_bits: usize,
    right_bits: usize,
) -> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError> {
    if left_bits == 0 {
        Ok(right_bits)
    } else if right_bits == 0 {
        Ok(left_bits)
    } else {
        checked_add(
            "pivot-target addition integer bits",
            left_bits.max(right_bits),
            1,
        )
    }
}

fn prospective_integer_heap_byte_bound(
    magnitude_bits: usize,
) -> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError> {
    // Two spare 64-bit limbs cover GMP carry growth and capacity rounding.
    let rounded_bits = checked_add(
        "pivot-target transformed integer heap-byte envelope",
        magnitude_bits,
        191,
    )?;
    checked_mul(
        "pivot-target transformed integer heap-byte envelope",
        rounded_bits / 64,
        size_of::<u64>(),
    )
}

fn prospective_integer_scratch_bytes(
    magnitudes: [usize; 3],
) -> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError> {
    let heap = magnitudes.into_iter().try_fold(0usize, |sum, bits| {
        checked_add(
            "pivot-target peak scratch bytes",
            sum,
            prospective_integer_heap_byte_bound(bits)?,
        )
    })?;
    checked_add(
        "pivot-target peak scratch bytes",
        heap,
        checked_mul("pivot-target peak scratch bytes", 3, size_of::<Integer>())?,
    )
}

fn integer_comparison_bit_work(
    left: &[Integer],
    right: &[Integer],
) -> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError> {
    if left.len() != right.len() {
        return Err(GeneratedAffineResidualCasePivotTargetMatchingError::MalformedGeometry);
    }
    left.iter()
        .zip(right)
        .try_fold(0usize, |sum, (left, right)| {
            checked_add(
                "pivot-target comparison integer-bit work",
                sum,
                checked_add(
                    "pivot-target comparison integer-bit work",
                    integer_magnitude_bits(left)?.max(1),
                    integer_magnitude_bits(right)?.max(1),
                )?,
            )
        })
}

fn classify_recentering_boundary(
    stats: &mut GeneratedAffineResidualCasePivotTargetMatchingStats,
    relation: &ParametricRelation,
    free_positions: &[usize],
    pivot: &IndexShift,
    limits: GeneratedAffineResidualCasePivotTargetMatchingLimits,
) -> Result<
    Option<(usize, GeneratedAffineResidualCaseRecenteringBoundaryKind)>,
    GeneratedAffineResidualCasePivotTargetMatchingError,
> {
    stats.recentering_boundary_checks = bounded_add(
        "pivot-target recentering boundary checks",
        stats.recentering_boundary_checks,
        free_positions.len(),
        limits.max_recentering_boundary_checks,
    )?;
    if let Some(position) = free_positions
        .iter()
        .copied()
        .find(|&position| pivot.values()[position].checked_neg().is_none())
    {
        return Ok(Some((
            position,
            GeneratedAffineResidualCaseRecenteringBoundaryKind::FreeCoefficientTranslationNegation,
        )));
    }
    stats.recentering_attempts = bounded_add(
        "pivot-target recentering attempts",
        stats.recentering_attempts,
        1,
        limits.max_recentering_attempts,
    )?;
    let key_checks = checked_mul(
        "pivot-target recentering boundary checks",
        relation.terms().len(),
        relation.arity(),
    )?;
    stats.recentering_boundary_checks = bounded_add(
        "pivot-target recentering boundary checks",
        stats.recentering_boundary_checks,
        key_checks,
        limits.max_recentering_boundary_checks,
    )?;
    for shift in relation.terms().keys() {
        for (position, (&value, &center)) in shift.values().iter().zip(pivot.values()).enumerate() {
            if value.checked_sub(center).is_none() {
                return Ok(Some((
                    position,
                    GeneratedAffineResidualCaseRecenteringBoundaryKind::IntegralKeySubtraction,
                )));
            }
        }
    }
    Ok(None)
}

fn coefficient_translation(
    free_positions: &[usize],
    pivot: &IndexShift,
) -> Result<IndexShift, GeneratedAffineResidualCasePivotTargetMatchingError> {
    let mut values = Vec::new();
    try_reserve_exact(
        "pivot-target coefficient translation",
        &mut values,
        pivot.arity(),
    )?;
    values.resize(pivot.arity(), 0);
    for &position in free_positions {
        values[position] = pivot.values()[position]
            .checked_neg()
            .ok_or(GeneratedAffineResidualCasePivotTargetMatchingError::ReplayMismatch)?;
    }
    IndexShift::try_from_preallocated(values, pivot.arity()).map_err(map_relation_error)
}

fn copy_shift(
    source: &IndexShift,
) -> Result<IndexShift, GeneratedAffineResidualCasePivotTargetMatchingError> {
    let mut values = Vec::new();
    try_reserve_exact(
        "pivot-target retained shift components",
        &mut values,
        source.arity(),
    )?;
    values.extend_from_slice(source.values());
    IndexShift::try_from_preallocated(values, source.arity()).map_err(map_relation_error)
}

fn map_relation_error(
    error: ParametricRelationError,
) -> GeneratedAffineResidualCasePivotTargetMatchingError {
    match error {
        ParametricRelationError::ResourceLimit {
            resource,
            requested,
            limit,
        } => GeneratedAffineResidualCasePivotTargetMatchingError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        ParametricRelationError::ResourceCountOverflow { resource } => {
            GeneratedAffineResidualCasePivotTargetMatchingError::ResourceCountOverflow { resource }
        }
        ParametricRelationError::AllocationFailure { resource, .. } => {
            GeneratedAffineResidualCasePivotTargetMatchingError::AllocationFailure { resource }
        }
        _ => GeneratedAffineResidualCasePivotTargetMatchingError::Relation,
    }
}

fn observe_peak_scratch(
    stats: &mut GeneratedAffineResidualCasePivotTargetMatchingStats,
    requested: usize,
    limits: GeneratedAffineResidualCasePivotTargetMatchingLimits,
) -> Result<(), GeneratedAffineResidualCasePivotTargetMatchingError> {
    stats.peak_scratch_bytes = stats.peak_scratch_bytes.max(requested);
    check_limit(
        "pivot-target peak scratch bytes",
        stats.peak_scratch_bytes,
        limits.max_peak_scratch_bytes,
    )
}

fn remaining_recentering_limits(
    stats: GeneratedAffineResidualCasePivotTargetMatchingStats,
    limits: GeneratedAffineResidualCasePivotTargetMatchingLimits,
) -> Result<
    ParametricAffineFreeRecenteringLimits,
    GeneratedAffineResidualCasePivotTargetMatchingError,
> {
    let configured = limits.recentering;
    Ok(ParametricAffineFreeRecenteringLimits {
        arithmetic: configured.arithmetic,
        max_terms: remaining(
            "pivot-target recentered terms",
            configured.max_terms,
            stats.recenter_terms,
        )?,
        max_guards: remaining(
            "pivot-target recentered guards",
            configured.max_guards,
            stats.recenter_guards,
        )?,
        max_translation_components: remaining(
            "pivot-target recentered translation components",
            configured.max_translation_components,
            stats.recenter_translation_components,
        )?,
        max_key_subtraction_boundary_checks: remaining(
            "pivot-target recentered key-subtraction boundary checks",
            configured.max_key_subtraction_boundary_checks,
            stats.recenter_key_subtraction_boundary_checks,
        )?,
        max_source_terms: remaining(
            "pivot-target recentered source terms",
            configured.max_source_terms,
            stats.recenter_source_terms,
        )?,
        max_source_exponent_entries: remaining(
            "pivot-target recentered source exponent entries",
            configured.max_source_exponent_entries,
            stats.recenter_source_exponent_entries,
        )?,
        max_output_terms: remaining(
            "pivot-target recentered output terms",
            configured.max_output_terms,
            stats.recenter_output_terms,
        )?,
        max_output_exponent_entries: remaining(
            "pivot-target recentered output exponent entries",
            configured.max_output_exponent_entries,
            stats.recenter_output_exponent_entries,
        )?,
        max_power_operations: remaining(
            "pivot-target recentered power operations",
            configured.max_power_operations,
            stats.recenter_power_operations,
        )?,
        max_integer_bit_work: remaining(
            "pivot-target recentered integer-bit work",
            configured.max_integer_bit_work,
            stats.recenter_integer_bit_work,
        )?,
        max_normalized_coefficient_terms: remaining(
            "pivot-target recentered normalized coefficient terms",
            configured.max_normalized_coefficient_terms,
            stats.recenter_normalized_coefficient_terms,
        )?,
        max_retained_bytes: remaining(
            "pivot-target recentered retained bytes",
            configured.max_retained_bytes,
            stats.recenter_retained_bytes,
        )?,
    })
}

fn accumulate_recentering_stats(
    target: &mut GeneratedAffineResidualCasePivotTargetMatchingStats,
    source: ParametricAffineFreeRecenteringStats,
    limits: GeneratedAffineResidualCasePivotTargetMatchingLimits,
) -> Result<(), GeneratedAffineResidualCasePivotTargetMatchingError> {
    let configured = limits.recentering;
    macro_rules! accumulate {
        ($field:ident, $getter:ident, $resource:literal, $limit:expr) => {
            target.$field = bounded_add($resource, target.$field, source.$getter(), $limit)?;
        };
    }
    accumulate!(
        recenter_terms,
        terms,
        "pivot-target recentered terms",
        configured.max_terms
    );
    accumulate!(
        recenter_guards,
        guards,
        "pivot-target recentered guards",
        configured.max_guards
    );
    accumulate!(
        recenter_translation_components,
        translation_components,
        "pivot-target recentered translation components",
        configured.max_translation_components
    );
    accumulate!(
        recenter_key_subtraction_boundary_checks,
        key_subtraction_boundary_checks,
        "pivot-target recentered key-subtraction boundary checks",
        configured.max_key_subtraction_boundary_checks
    );
    accumulate!(
        recenter_source_terms,
        source_terms,
        "pivot-target recentered source terms",
        configured.max_source_terms
    );
    accumulate!(
        recenter_source_exponent_entries,
        source_exponent_entries,
        "pivot-target recentered source exponent entries",
        configured.max_source_exponent_entries
    );
    accumulate!(
        recenter_output_terms,
        output_terms,
        "pivot-target recentered output terms",
        configured.max_output_terms
    );
    accumulate!(
        recenter_output_exponent_entries,
        output_exponent_entries,
        "pivot-target recentered output exponent entries",
        configured.max_output_exponent_entries
    );
    accumulate!(
        recenter_power_operations,
        power_operations,
        "pivot-target recentered power operations",
        configured.max_power_operations
    );
    accumulate!(
        recenter_integer_bit_work,
        integer_bit_work,
        "pivot-target recentered integer-bit work",
        configured.max_integer_bit_work
    );
    accumulate!(
        recenter_normalized_coefficient_terms,
        normalized_coefficient_terms,
        "pivot-target recentered normalized coefficient terms",
        configured.max_normalized_coefficient_terms
    );
    accumulate!(
        recenter_retained_bytes,
        retained_bytes,
        "pivot-target recentered retained bytes",
        configured.max_retained_bytes
    );
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecenteringRetainedClamp {
    Owner,
    Recentering,
    Both,
}

fn map_recentering_error(
    error: ParametricRelationError,
    used: GeneratedAffineResidualCasePivotTargetMatchingStats,
    limits: GeneratedAffineResidualCasePivotTargetMatchingLimits,
    owner_used: usize,
    retained_clamp: RecenteringRetainedClamp,
) -> GeneratedAffineResidualCasePivotTargetMatchingError {
    let ParametricRelationError::ResourceLimit {
        resource,
        requested,
        limit: child_limit,
    } = error
    else {
        return map_relation_error(error);
    };
    let configured = limits.recentering;
    let mapping = match resource {
        "affine free recentering terms" => Some((
            "pivot-target recentered terms",
            used.recenter_terms,
            configured.max_terms,
        )),
        "affine free recentering guards" => Some((
            "pivot-target recentered guards",
            used.recenter_guards,
            configured.max_guards,
        )),
        "affine free recentering translation components" => Some((
            "pivot-target recentered translation components",
            used.recenter_translation_components,
            configured.max_translation_components,
        )),
        "affine free recentering key-subtraction boundary checks" => Some((
            "pivot-target recentered key-subtraction boundary checks",
            used.recenter_key_subtraction_boundary_checks,
            configured.max_key_subtraction_boundary_checks,
        )),
        "affine free recentering source terms" => Some((
            "pivot-target recentered source terms",
            used.recenter_source_terms,
            configured.max_source_terms,
        )),
        "affine free recentering source exponent entries" => Some((
            "pivot-target recentered source exponent entries",
            used.recenter_source_exponent_entries,
            configured.max_source_exponent_entries,
        )),
        "affine free recentering output terms" => Some((
            "pivot-target recentered output terms",
            used.recenter_output_terms,
            configured.max_output_terms,
        )),
        "affine free recentering output exponent entries" => Some((
            "pivot-target recentered output exponent entries",
            used.recenter_output_exponent_entries,
            configured.max_output_exponent_entries,
        )),
        "affine free recentering power operations" => Some((
            "pivot-target recentered power operations",
            used.recenter_power_operations,
            configured.max_power_operations,
        )),
        "affine free recentering integer-bit work" => Some((
            "pivot-target recentered integer-bit work",
            used.recenter_integer_bit_work,
            configured.max_integer_bit_work,
        )),
        "affine free recentering normalized coefficient terms" => Some((
            "pivot-target recentered normalized coefficient terms",
            used.recenter_normalized_coefficient_terms,
            configured.max_normalized_coefficient_terms,
        )),
        "affine free recentering retained-byte envelope" => {
            return GeneratedAffineResidualCasePivotTargetMatchingError::ReplayMismatch;
        }
        "affine free recentering retained bytes"
            if matches!(
                retained_clamp,
                RecenteringRetainedClamp::Owner | RecenteringRetainedClamp::Both
            ) =>
        {
            return match owner_used.checked_add(requested) {
                Some(outer_requested) => {
                    GeneratedAffineResidualCasePivotTargetMatchingError::ResourceLimit {
                        resource: "pivot-target owner retained bytes",
                        requested: outer_requested,
                        limit: limits.max_owner_retained_bytes,
                    }
                }
                None => {
                    GeneratedAffineResidualCasePivotTargetMatchingError::ResourceCountOverflow {
                        resource: "pivot-target owner retained bytes",
                    }
                }
            };
        }
        "affine free recentering retained bytes" => Some((
            "pivot-target recentered retained bytes",
            used.recenter_retained_bytes,
            configured.max_retained_bytes,
        )),
        _ => None,
    };
    if let Some((outer_resource, already_used, outer_limit)) = mapping {
        match already_used.checked_add(requested) {
            Some(outer_requested) => {
                GeneratedAffineResidualCasePivotTargetMatchingError::ResourceLimit {
                    resource: outer_resource,
                    requested: outer_requested,
                    limit: outer_limit,
                }
            }
            None => GeneratedAffineResidualCasePivotTargetMatchingError::ResourceCountOverflow {
                resource: outer_resource,
            },
        }
    } else {
        GeneratedAffineResidualCasePivotTargetMatchingError::ResourceLimit {
            resource,
            requested,
            limit: child_limit,
        }
    }
}

fn pending_row_label_byte_len(
    source_case_ordinal: usize,
    pivot_ordinal: usize,
) -> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError> {
    const PREFIX: &str = "generated-affine-case-pivot-target-pending-v2:";
    checked_add(
        "pivot-target row-label bytes",
        PREFIX.len(),
        checked_add(
            "pivot-target row-label bytes",
            checked_add(
                "pivot-target row-label bytes",
                decimal_digits(source_case_ordinal),
                decimal_digits(pivot_ordinal),
            )?,
            1,
        )?,
    )
}

fn pending_row_id(
    source_case_ordinal: usize,
    pivot_ordinal: usize,
    expected_bytes: usize,
) -> Result<ParametricRowId, GeneratedAffineResidualCasePivotTargetMatchingError> {
    const PREFIX: &str = "generated-affine-case-pivot-target-pending-v2:";
    if pending_row_label_byte_len(source_case_ordinal, pivot_ordinal)? != expected_bytes {
        return Err(GeneratedAffineResidualCasePivotTargetMatchingError::ReplayMismatch);
    }
    let mut label = String::new();
    label.try_reserve_exact(expected_bytes).map_err(|_| {
        GeneratedAffineResidualCasePivotTargetMatchingError::AllocationFailure {
            resource: "pivot-target row-label bytes",
        }
    })?;
    write!(label, "{PREFIX}{source_case_ordinal}:{pivot_ordinal}")
        .map_err(|_| GeneratedAffineResidualCasePivotTargetMatchingError::ReplayMismatch)?;
    if label.len() != expected_bytes {
        return Err(GeneratedAffineResidualCasePivotTargetMatchingError::ReplayMismatch);
    }
    Ok(ParametricRowId::Derived {
        label: Arc::from(label),
    })
}

fn pending_external_relation_allocation_byte_bound(
    context_fingerprint_bytes: usize,
    row_label_bytes: usize,
) -> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError> {
    checked_sum(
        "pivot-target owner retained bytes",
        [
            arc_control_and_padding_byte_bound::<ParametricRelation>()?,
            row_label_bytes,
            arc_control_and_padding_byte_bound::<u8>()?,
            context_fingerprint_bytes,
            arc_control_and_padding_byte_bound::<u8>()?,
        ],
    )
}

fn arc_control_and_padding_byte_bound<T>()
-> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError> {
    checked_add(
        "pivot-target owner retained bytes",
        checked_mul("pivot-target owner retained bytes", 2, size_of::<usize>())?,
        align_of::<T>().saturating_sub(1),
    )
}

fn arc_pointee_byte_bound<T>() -> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError>
{
    checked_add(
        "pivot-target owner retained bytes",
        size_of::<T>(),
        arc_control_and_padding_byte_bound::<T>()?,
    )
}

struct RetainedAdmission {
    bytes: usize,
    limit: usize,
}

impl RetainedAdmission {
    fn new(
        bytes: usize,
        limit: usize,
    ) -> Result<Self, GeneratedAffineResidualCasePivotTargetMatchingError> {
        check_limit("pivot-target owner retained bytes", bytes, limit)?;
        Ok(Self { bytes, limit })
    }
    fn admit(
        &mut self,
        additional: usize,
    ) -> Result<(), GeneratedAffineResidualCasePivotTargetMatchingError> {
        self.bytes = bounded_add(
            "pivot-target owner retained bytes",
            self.bytes,
            additional,
            self.limit,
        )?;
        Ok(())
    }
    const fn remaining(&self) -> usize {
        self.limit - self.bytes
    }
    const fn bytes(&self) -> usize {
        self.bytes
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PayloadCensus {
    units: usize,
    bytes: usize,
    integer_bits: usize,
    relation_manifest_bytes: usize,
}

fn stored_payload_census(
    stats: GeneratedAffineResidualCasePivotTargetMatchingStats,
) -> PayloadCensus {
    PayloadCensus {
        units: stats.payload_comparison_units,
        bytes: stats.payload_comparison_bytes,
        integer_bits: stats.payload_comparison_integer_bits,
        relation_manifest_bytes: stats.payload_comparison_relation_manifest_bytes,
    }
}

fn doubled_payload_census(
    raw: PayloadCensus,
) -> Result<PayloadCensus, GeneratedAffineResidualCasePivotTargetMatchingError> {
    Ok(PayloadCensus {
        units: checked_mul("pivot-target payload comparison units", raw.units, 2)?,
        bytes: checked_mul("pivot-target payload comparison bytes", raw.bytes, 2)?,
        integer_bits: checked_mul(
            "pivot-target payload comparison integer bits",
            raw.integer_bits,
            2,
        )?,
        relation_manifest_bytes: checked_mul(
            "pivot-target payload comparison relation manifest bytes",
            raw.relation_manifest_bytes,
            2,
        )?,
    })
}

fn raw_payload_limits_from_full_census(
    full: PayloadCensus,
) -> Result<
    GeneratedAffineResidualCasePivotTargetMatchingReplayLimits,
    GeneratedAffineResidualCasePivotTargetMatchingError,
> {
    if full.units % 2 != 0
        || full.bytes % 2 != 0
        || full.integer_bits % 2 != 0
        || full.relation_manifest_bytes % 2 != 0
    {
        return Err(GeneratedAffineResidualCasePivotTargetMatchingError::ReplayMismatch);
    }
    Ok(GeneratedAffineResidualCasePivotTargetMatchingReplayLimits {
        max_reelimination_allocation_comparisons: REELIMINATION_ALLOCATION_COMPARISONS,
        max_combined_matcher_owner_bytes: usize::MAX,
        max_payload_comparison_units: full.units / 2,
        max_payload_comparison_bytes: full.bytes / 2,
        max_payload_comparison_integer_bits: full.integer_bits / 2,
        max_payload_comparison_relation_manifest_bytes: full.relation_manifest_bytes / 2,
    })
}

const fn unbounded_payload_replay_limits()
-> GeneratedAffineResidualCasePivotTargetMatchingReplayLimits {
    GeneratedAffineResidualCasePivotTargetMatchingReplayLimits {
        max_reelimination_allocation_comparisons: usize::MAX,
        max_combined_matcher_owner_bytes: usize::MAX,
        max_payload_comparison_units: usize::MAX,
        max_payload_comparison_bytes: usize::MAX,
        max_payload_comparison_integer_bits: usize::MAX,
        max_payload_comparison_relation_manifest_bytes: usize::MAX,
    }
}

fn payload_census_pair(
    left: &[GeneratedAffineResidualCasePivotTargetOutcome],
    right: &[GeneratedAffineResidualCasePivotTargetOutcome],
    limits: GeneratedAffineResidualCasePivotTargetMatchingReplayLimits,
) -> Result<PayloadCensus, GeneratedAffineResidualCasePivotTargetMatchingError> {
    let mut census = PayloadCensus::default();
    census_add_units(&mut census, 12, limits)?;
    census_add_bytes(
        &mut census,
        checked_mul(
            "pivot-target payload comparison bytes",
            2,
            size_of::<GeneratedAffineResidualCasePivotTargetMatchingCertificate>(),
        )?,
        limits,
    )?;
    for outcomes in [left, right] {
        census_add_units(&mut census, outcomes.len(), limits)?;
        census_add_bytes(
            &mut census,
            checked_mul(
                "pivot-target payload comparison bytes",
                outcomes.len(),
                size_of::<GeneratedAffineResidualCasePivotTargetOutcome>(),
            )?,
            limits,
        )?;
        for outcome in outcomes {
            let transcript = outcome_transcript(outcome);
            census_add_units(&mut census, 4, limits)?;
            census_shift(&mut census, &transcript.pivot, limits)?;
            census_integers(
                &mut census,
                &transcript.transformed_target_constants,
                limits,
            )?;
            census_add_units(&mut census, transcript.target_witnesses.len(), limits)?;
            census_add_bytes(
                &mut census,
                checked_mul(
                    "pivot-target payload comparison bytes",
                    transcript.target_witnesses.len(),
                    size_of::<GeneratedAffineResidualCasePivotTargetWitness>(),
                )?,
                limits,
            )?;
            match outcome {
                GeneratedAffineResidualCasePivotTargetOutcome::RejectedNoTarget(_) => {}
                GeneratedAffineResidualCasePivotTargetOutcome::RejectedRecenteringBoundary(_) => {
                    census_add_units(&mut census, 2, limits)?;
                }
                GeneratedAffineResidualCasePivotTargetOutcome::Pending(value) => {
                    census_add_units(&mut census, 3, limits)?;
                    census_shift(&mut census, &value.coefficient_translation, limits)?;
                    census_shift(&mut census, &value.key_center, limits)?;
                    census_relation(&mut census, &value.relation, limits)?;
                }
            }
        }
    }
    Ok(census)
}

fn outcome_transcript(
    outcome: &GeneratedAffineResidualCasePivotTargetOutcome,
) -> &PivotTargetTranscript {
    match outcome {
        GeneratedAffineResidualCasePivotTargetOutcome::RejectedNoTarget(value) => &value.transcript,
        GeneratedAffineResidualCasePivotTargetOutcome::RejectedRecenteringBoundary(value) => {
            &value.transcript
        }
        GeneratedAffineResidualCasePivotTargetOutcome::Pending(value) => &value.transcript,
    }
}

fn census_shift(
    census: &mut PayloadCensus,
    shift: &IndexShift,
    limits: GeneratedAffineResidualCasePivotTargetMatchingReplayLimits,
) -> Result<(), GeneratedAffineResidualCasePivotTargetMatchingError> {
    census_add_units(census, shift.arity(), limits)?;
    census_add_bytes(
        census,
        checked_mul(
            "pivot-target payload comparison bytes",
            shift.arity(),
            size_of::<i64>(),
        )?,
        limits,
    )
}

fn census_integers(
    census: &mut PayloadCensus,
    values: &[Integer],
    limits: GeneratedAffineResidualCasePivotTargetMatchingReplayLimits,
) -> Result<(), GeneratedAffineResidualCasePivotTargetMatchingError> {
    census_add_units(census, values.len(), limits)?;
    census_add_bytes(
        census,
        checked_mul(
            "pivot-target payload comparison bytes",
            values.len(),
            size_of::<Integer>(),
        )?,
        limits,
    )?;
    for value in values {
        let bits = integer_magnitude_bits(value)?.max(1);
        census.integer_bits = bounded_add(
            "pivot-target payload comparison integer bits",
            census.integer_bits,
            bits,
            limits.max_payload_comparison_integer_bits,
        )?;
        census_add_bytes(census, integer_owned_heap_byte_bound(value)?, limits)?;
    }
    Ok(())
}

fn census_relation(
    census: &mut PayloadCensus,
    relation: &ParametricRelation,
    limits: GeneratedAffineResidualCasePivotTargetMatchingReplayLimits,
) -> Result<(), GeneratedAffineResidualCasePivotTargetMatchingError> {
    let local_limits = remaining_payload_limits(census, limits)?;
    let manifest_remaining = local_limits.max_payload_comparison_relation_manifest_bytes;
    let payload_bytes_remaining = local_limits.max_payload_comparison_bytes;
    let (writer_limit, writer_resource, writer_prior, writer_outer_limit) =
        if payload_bytes_remaining < manifest_remaining {
            (
                payload_bytes_remaining,
                "pivot-target payload comparison bytes",
                census.bytes,
                limits.max_payload_comparison_bytes,
            )
        } else {
            (
                manifest_remaining,
                "pivot-target payload comparison relation manifest bytes",
                census.relation_manifest_bytes,
                limits.max_payload_comparison_relation_manifest_bytes,
            )
        };
    let mut observer = RelationPayloadObserver::new(
        local_limits,
        census.units,
        census.integer_bits,
        writer_limit,
        writer_resource,
        writer_prior,
        writer_outer_limit,
        limits,
    );
    let mut writer = BoundedManifestCounter::new(
        writer_limit,
        writer_resource,
        writer_prior,
        writer_outer_limit,
    );
    let result = write_relation_manifest_v2_observed(&mut writer, relation, &mut observer);
    if result.is_err() {
        return Err(observer.error.or(writer.error).unwrap_or(
            GeneratedAffineResidualCasePivotTargetMatchingError::ResourceCountOverflow {
                resource: "pivot-target payload relation identity",
            },
        ));
    }
    census_add_units(census, observer.units, limits)?;
    census.integer_bits = bounded_add(
        "pivot-target payload comparison integer bits",
        census.integer_bits,
        observer.integer_bits,
        limits.max_payload_comparison_integer_bits,
    )?;
    census.relation_manifest_bytes = bounded_add(
        "pivot-target payload comparison relation manifest bytes",
        census.relation_manifest_bytes,
        writer.bytes,
        limits.max_payload_comparison_relation_manifest_bytes,
    )?;
    census_add_bytes(census, writer.bytes, limits)?;
    let retained = relation.owned_retained_byte_bound().ok_or(
        GeneratedAffineResidualCasePivotTargetMatchingError::ResourceCountOverflow {
            resource: "pivot-target payload comparison bytes",
        },
    )?;
    census_add_bytes(census, retained, limits)
}

fn remaining_payload_limits(
    census: &PayloadCensus,
    limits: GeneratedAffineResidualCasePivotTargetMatchingReplayLimits,
) -> Result<
    GeneratedAffineResidualCasePivotTargetMatchingReplayLimits,
    GeneratedAffineResidualCasePivotTargetMatchingError,
> {
    Ok(GeneratedAffineResidualCasePivotTargetMatchingReplayLimits {
        max_reelimination_allocation_comparisons: limits.max_reelimination_allocation_comparisons,
        max_combined_matcher_owner_bytes: limits.max_combined_matcher_owner_bytes,
        max_payload_comparison_units: remaining(
            "pivot-target payload comparison units",
            limits.max_payload_comparison_units,
            census.units,
        )?,
        max_payload_comparison_bytes: remaining(
            "pivot-target payload comparison bytes",
            limits.max_payload_comparison_bytes,
            census.bytes,
        )?,
        max_payload_comparison_integer_bits: remaining(
            "pivot-target payload comparison integer bits",
            limits.max_payload_comparison_integer_bits,
            census.integer_bits,
        )?,
        max_payload_comparison_relation_manifest_bytes: remaining(
            "pivot-target payload comparison relation manifest bytes",
            limits.max_payload_comparison_relation_manifest_bytes,
            census.relation_manifest_bytes,
        )?,
    })
}

struct BoundedManifestCounter {
    bytes: usize,
    limit: usize,
    resource: &'static str,
    prior: usize,
    outer_limit: usize,
    error: Option<GeneratedAffineResidualCasePivotTargetMatchingError>,
}

impl BoundedManifestCounter {
    const fn new(limit: usize, resource: &'static str, prior: usize, outer_limit: usize) -> Self {
        Self {
            bytes: 0,
            limit,
            resource,
            prior,
            outer_limit,
            error: None,
        }
    }
}

impl fmt::Write for BoundedManifestCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(requested) = self.bytes.checked_add(value.len()) else {
            self.error = Some(
                GeneratedAffineResidualCasePivotTargetMatchingError::ResourceCountOverflow {
                    resource: self.resource,
                },
            );
            return Err(fmt::Error);
        };
        if requested > self.limit {
            self.error = Some(outer_limit_error(
                self.resource,
                self.prior,
                requested,
                self.outer_limit,
            ));
            return Err(fmt::Error);
        }
        self.bytes = requested;
        Ok(())
    }
}

struct RelationPayloadObserver {
    limits: GeneratedAffineResidualCasePivotTargetMatchingReplayLimits,
    units: usize,
    integer_bits: usize,
    prior_units: usize,
    prior_integer_bits: usize,
    prefix_limit: usize,
    prefix_resource: &'static str,
    prefix_prior: usize,
    prefix_outer_limit: usize,
    outer_limits: GeneratedAffineResidualCasePivotTargetMatchingReplayLimits,
    error: Option<GeneratedAffineResidualCasePivotTargetMatchingError>,
}

impl RelationPayloadObserver {
    #[allow(clippy::too_many_arguments)]
    const fn new(
        limits: GeneratedAffineResidualCasePivotTargetMatchingReplayLimits,
        prior_units: usize,
        prior_integer_bits: usize,
        prefix_limit: usize,
        prefix_resource: &'static str,
        prefix_prior: usize,
        prefix_outer_limit: usize,
        outer_limits: GeneratedAffineResidualCasePivotTargetMatchingReplayLimits,
    ) -> Self {
        Self {
            limits,
            units: 0,
            integer_bits: 0,
            prior_units,
            prior_integer_bits,
            prefix_limit,
            prefix_resource,
            prefix_prior,
            prefix_outer_limit,
            outer_limits,
            error: None,
        }
    }

    fn charge_unit(&mut self) -> fmt::Result {
        match self.units.checked_add(1) {
            Some(requested) if requested <= self.limits.max_payload_comparison_units => {
                self.units = requested;
                Ok(())
            }
            Some(requested) => {
                self.error = Some(outer_limit_error(
                    "pivot-target payload comparison units",
                    self.prior_units,
                    requested,
                    self.outer_limits.max_payload_comparison_units,
                ));
                Err(fmt::Error)
            }
            None => {
                self.error = Some(
                    GeneratedAffineResidualCasePivotTargetMatchingError::ResourceCountOverflow {
                        resource: "pivot-target payload comparison units",
                    },
                );
                Err(fmt::Error)
            }
        }
    }

    fn charge_bits(&mut self, bits: usize) -> fmt::Result {
        if self.charge_unit().is_err() {
            return Err(fmt::Error);
        }
        match self.integer_bits.checked_add(bits.max(1)) {
            Some(requested) if requested <= self.limits.max_payload_comparison_integer_bits => {
                self.integer_bits = requested;
                Ok(())
            }
            Some(requested) => {
                self.error = Some(outer_limit_error(
                    "pivot-target payload comparison integer bits",
                    self.prior_integer_bits,
                    requested,
                    self.outer_limits.max_payload_comparison_integer_bits,
                ));
                Err(fmt::Error)
            }
            None => {
                self.error = Some(
                    GeneratedAffineResidualCasePivotTargetMatchingError::ResourceCountOverflow {
                        resource: "pivot-target payload comparison integer bits",
                    },
                );
                Err(fmt::Error)
            }
        }
    }
}

impl ParametricRelationV2Observer for RelationPayloadObserver {
    fn length_prefix_byte_limit(&self) -> usize {
        self.prefix_limit
    }

    fn observe_length_prefix_limit_exceeded(
        &mut self,
        requested: usize,
        limit: usize,
    ) -> fmt::Result {
        debug_assert_eq!(limit, self.prefix_limit);
        self.error = Some(outer_limit_error(
            self.prefix_resource,
            self.prefix_prior,
            requested,
            self.prefix_outer_limit,
        ));
        Err(fmt::Error)
    }

    fn observe_text_payload(&mut self, _bytes: usize) -> fmt::Result {
        self.charge_unit()
    }

    fn observe_unsigned(&mut self, value: u128) -> fmt::Result {
        self.charge_bits((u128::BITS - value.leading_zeros()) as usize)
    }

    fn observe_signed_i64(&mut self, value: i64) -> fmt::Result {
        self.charge_bits(i64_magnitude_bits(value))
    }

    fn observe_integer(&mut self, value: &Integer) -> fmt::Result {
        match integer_magnitude_bits(value) {
            Ok(bits) => self.charge_bits(bits),
            Err(error) => {
                self.error = Some(error);
                Err(fmt::Error)
            }
        }
    }

    fn observe_polynomial(&mut self, _polynomial: &CoefficientPolynomial) -> fmt::Result {
        self.charge_unit()
    }
}

fn outer_limit_error(
    resource: &'static str,
    prior: usize,
    local_requested: usize,
    outer_limit: usize,
) -> GeneratedAffineResidualCasePivotTargetMatchingError {
    match prior.checked_add(local_requested) {
        Some(requested) => GeneratedAffineResidualCasePivotTargetMatchingError::ResourceLimit {
            resource,
            requested,
            limit: outer_limit,
        },
        None => {
            GeneratedAffineResidualCasePivotTargetMatchingError::ResourceCountOverflow { resource }
        }
    }
}

fn census_add_units(
    census: &mut PayloadCensus,
    additional: usize,
    limits: GeneratedAffineResidualCasePivotTargetMatchingReplayLimits,
) -> Result<(), GeneratedAffineResidualCasePivotTargetMatchingError> {
    census.units = bounded_add(
        "pivot-target payload comparison units",
        census.units,
        additional,
        limits.max_payload_comparison_units,
    )?;
    Ok(())
}

fn census_add_bytes(
    census: &mut PayloadCensus,
    additional: usize,
    limits: GeneratedAffineResidualCasePivotTargetMatchingReplayLimits,
) -> Result<(), GeneratedAffineResidualCasePivotTargetMatchingError> {
    census.bytes = bounded_add(
        "pivot-target payload comparison bytes",
        census.bytes,
        additional,
        limits.max_payload_comparison_bytes,
    )?;
    Ok(())
}

/// Admit the complete prepared replay demand before any semantic payload
/// comparison. The preparation pass above performs only checked scalar
/// counting and allocation-free manifest-length traversal.
fn admit_payload_census(
    census: PayloadCensus,
    limits: GeneratedAffineResidualCasePivotTargetMatchingReplayLimits,
) -> Result<(), GeneratedAffineResidualCasePivotTargetMatchingError> {
    for (resource, requested, limit) in [
        (
            "pivot-target payload comparison units",
            census.units,
            limits.max_payload_comparison_units,
        ),
        (
            "pivot-target payload comparison bytes",
            census.bytes,
            limits.max_payload_comparison_bytes,
        ),
        (
            "pivot-target payload comparison integer bits",
            census.integer_bits,
            limits.max_payload_comparison_integer_bits,
        ),
        (
            "pivot-target payload comparison relation manifest bytes",
            census.relation_manifest_bytes,
            limits.max_payload_comparison_relation_manifest_bytes,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    Ok(())
}

fn payload_eq(
    left: &GeneratedAffineResidualCasePivotTargetMatchingCertificate,
    right: &GeneratedAffineResidualCasePivotTargetMatchingCertificate,
    admitted_full_census: PayloadCensus,
) -> Result<bool, GeneratedAffineResidualCasePivotTargetMatchingError> {
    let raw_limits = raw_payload_limits_from_full_census(admitted_full_census)?;
    let raw_census = payload_census_pair(&left.outcomes, &right.outcomes, raw_limits)?;
    let census = doubled_payload_census(raw_census)?;
    if census != admitted_full_census || census != stored_payload_census(left.stats) {
        return Ok(false);
    }
    if left.schema != right.schema
        || left.source_case_ordinal != right.source_case_ordinal
        || left.source_group_ordinal != right.source_group_ordinal
        || left.limits != right.limits
        || left.stats != right.stats
        || left.outcomes.len() != right.outcomes.len()
        || !Arc::ptr_eq(&left.reelimination, &right.reelimination)
    {
        return Ok(false);
    }
    for (left, right) in left.outcomes.iter().zip(right.outcomes.iter()) {
        if !outcome_payload_eq(left, right) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn outcome_payload_eq(
    left: &GeneratedAffineResidualCasePivotTargetOutcome,
    right: &GeneratedAffineResidualCasePivotTargetOutcome,
) -> bool {
    match (left, right) {
        (
            GeneratedAffineResidualCasePivotTargetOutcome::RejectedNoTarget(left),
            GeneratedAffineResidualCasePivotTargetOutcome::RejectedNoTarget(right),
        ) => transcript_payload_eq(&left.transcript, &right.transcript),
        (
            GeneratedAffineResidualCasePivotTargetOutcome::RejectedRecenteringBoundary(left),
            GeneratedAffineResidualCasePivotTargetOutcome::RejectedRecenteringBoundary(right),
        ) => {
            left.kind == right.kind
                && left.position == right.position
                && transcript_payload_eq(&left.transcript, &right.transcript)
        }
        (
            GeneratedAffineResidualCasePivotTargetOutcome::Pending(left),
            GeneratedAffineResidualCasePivotTargetOutcome::Pending(right),
        ) => {
            transcript_payload_eq(&left.transcript, &right.transcript)
                && left.coefficient_translation == right.coefficient_translation
                && left.key_center == right.key_center
                && left.recentering_stats == right.recentering_stats
                && relation_payload_eq(&left.relation, &right.relation)
        }
        _ => false,
    }
}

fn relation_payload_eq(left: &ParametricRelation, right: &ParametricRelation) -> bool {
    left.has_identical_guard_provenance(right)
}

fn transcript_payload_eq(left: &PivotTargetTranscript, right: &PivotTargetTranscript) -> bool {
    left.pivot_ordinal == right.pivot_ordinal
        && left.pivot == right.pivot
        && left.transformed_target_constants == right.transformed_target_constants
        && left.target_witnesses == right.target_witnesses
}

fn integer_owned_heap_byte_bound(
    value: &Integer,
) -> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError> {
    match value {
        Integer::Single(_) | Integer::Double(_) => Ok(0),
        Integer::Large(value) => value.capacity().checked_add(7).map(|bits| bits / 8).ok_or(
            GeneratedAffineResidualCasePivotTargetMatchingError::ResourceCountOverflow {
                resource: "pivot-target integer owned heap bytes",
            },
        ),
    }
}

fn integer_magnitude_bits(
    value: &Integer,
) -> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| {
        GeneratedAffineResidualCasePivotTargetMatchingError::ResourceCountOverflow {
            resource: "pivot-target integer bits",
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, OnceLock, Weak};
    use std::thread;

    use super::*;
    use crate::generated_affine_parametric_ordering::{
        GeneratedAffineParametricOrderingCertificate, GeneratedAffineParametricOrderingLimits,
    };
    use crate::generated_affine_prepare_point_schedule::{
        GeneratedAffinePreparePointScheduleCertificate, GeneratedAffinePreparePointScheduleLimits,
    };
    use crate::generated_affine_residual_boolean_cover::{
        GeneratedAffineResidualBooleanCoverCompiler, GeneratedAffineResidualBooleanCoverLimits,
    };
    use crate::generated_affine_residual_case_premises::{
        GeneratedAffineResidualCasePremisesLimits, GeneratedAffineResidualCasePremisesOutcome,
        compile_generated_affine_residual_case_premises,
    };
    use crate::generated_affine_residual_case_reelimination::{
        GeneratedAffineResidualCaseReeliminationCompilation,
        GeneratedAffineResidualCaseReeliminationCompiler,
        GeneratedAffineResidualCaseReeliminationLimits,
    };
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
    use crate::solver::closure::case_inventory::{
        GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityLimits,
        GeneratedAffineResidualCaseInventoryCompiler, GeneratedAffineResidualCaseInventoryLimits,
    };
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, GuardOrigin, IntegralOrderingPolicy,
        ParametricArithmeticLimits, ParametricIbpGenerator, SectorMask,
    };

    struct NaturalFixture {
        family: IntegralFamily,
        context: ParametricCoefficientContext,
        reelimination: Arc<GeneratedAffineResidualCaseReeliminationCertificate>,
    }

    impl NaturalFixture {
        fn compile_matcher(
            &self,
            limits: GeneratedAffineResidualCasePivotTargetMatchingLimits,
        ) -> Result<
            GeneratedAffineResidualCasePivotTargetMatchingCertificate,
            GeneratedAffineResidualCasePivotTargetMatchingError,
        > {
            GeneratedAffineResidualCasePivotTargetMatchingCompiler::compile(
                &self.family,
                &self.context,
                Arc::clone(&self.reelimination),
                limits,
            )
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

    fn build_natural_fixture(name: &str, sector: &str, case_ordinal: usize) -> NaturalFixture {
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
            SectorMask::try_from_bit_string(sector).unwrap(),
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
        assert!(case_ordinal < inventory.case_count());
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                &family,
                &context,
                inventory,
                case_ordinal,
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
                    panic!("selected natural case unexpectedly requires equality refinement")
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
                0,
                GeneratedAffinePreparePointScheduleLimits::default(),
            )
            .unwrap(),
        );
        let compiled = GeneratedAffineResidualCaseReeliminationCompiler::compile(
            &family,
            &context,
            authority,
            premises,
            ordering,
            schedule,
            GeneratedAffineResidualCaseReeliminationLimits::default(),
        )
        .unwrap();
        let GeneratedAffineResidualCaseReeliminationCompilation::Eliminated(reelimination) =
            compiled
        else {
            panic!("selected natural case unexpectedly has no available rows")
        };
        NaturalFixture {
            family,
            context,
            reelimination: Arc::new(reelimination),
        }
    }

    fn success_fixture() -> &'static NaturalFixture {
        static FIXTURE: OnceLock<NaturalFixture> = OnceLock::new();
        FIXTURE.get_or_init(|| build_natural_fixture("pivot-target-success-v2", "001", 0))
    }

    fn multi_case_fixture() -> &'static NaturalFixture {
        static FIXTURE: OnceLock<NaturalFixture> = OnceLock::new();
        FIXTURE.get_or_init(|| build_natural_fixture("pivot-target-multi-case-v2", "011", 1))
    }

    fn outcome_transcript_parts(
        outcome: &GeneratedAffineResidualCasePivotTargetOutcome,
    ) -> (
        &IndexShift,
        &[Integer],
        &[GeneratedAffineResidualCasePivotTargetWitness],
    ) {
        match outcome {
            GeneratedAffineResidualCasePivotTargetOutcome::RejectedNoTarget(value) => (
                value.pivot(),
                value.transformed_target_constants(),
                value.target_witnesses(),
            ),
            GeneratedAffineResidualCasePivotTargetOutcome::RejectedRecenteringBoundary(value) => (
                value.pivot(),
                value.transformed_target_constants(),
                value.target_witnesses(),
            ),
            GeneratedAffineResidualCasePivotTargetOutcome::Pending(value) => (
                value.pivot(),
                value.transformed_target_constants(),
                value.target_witnesses(),
            ),
        }
    }

    fn exact_construction_limits(
        stats: GeneratedAffineResidualCasePivotTargetMatchingStats,
    ) -> GeneratedAffineResidualCasePivotTargetMatchingLimits {
        let mut limits = GeneratedAffineResidualCasePivotTargetMatchingLimits::default();
        limits.same_group_targets.max_scope_comparison_bytes =
            stats.same_group_scope_comparison_bytes();
        limits.same_group_targets.max_case_lookups = stats.same_group_case_lookups();
        limits.same_group_targets.max_group_lookups = stats.same_group_group_lookups();
        limits.same_group_targets.max_ordinal_comparisons = stats.same_group_ordinal_comparisons();
        limits.same_group_targets.max_shape_comparisons = stats.same_group_shape_comparisons();
        limits.same_group_targets.max_target_case_references =
            stats.same_group_target_case_references();
        limits.target_handle.max_target_position_lookups = stats.maximum_target_position_lookups();
        limits.target_handle.max_case_lookups = stats.maximum_target_handle_case_lookups();
        limits.target_handle.max_anchor_offset_lookups =
            stats.maximum_target_anchor_offset_lookups();
        limits.target_handle.max_ordinal_comparisons =
            stats.maximum_target_handle_ordinal_comparisons();
        limits.target_case.max_scope_comparison_bytes =
            stats.maximum_target_case_scope_comparison_bytes();
        limits.target_case.max_authority_allocation_comparisons =
            stats.maximum_target_authority_allocation_comparisons();
        limits.target_case.max_case_lookups = stats.maximum_target_case_lookups();
        limits.target_case.max_group_lookups = stats.maximum_target_group_lookups();
        limits.target_case.max_ordinal_comparisons =
            stats.maximum_target_case_ordinal_comparisons();
        limits.target_case.max_geometry_reference_comparisons =
            stats.maximum_target_geometry_reference_comparisons();
        limits.recentering.max_terms = stats.recenter_terms();
        limits.recentering.max_guards = stats.recenter_guards();
        limits.recentering.max_translation_components = stats.recenter_translation_components();
        limits.recentering.max_key_subtraction_boundary_checks =
            stats.recenter_key_subtraction_boundary_checks();
        limits.recentering.max_source_terms = stats.recenter_source_terms();
        limits.recentering.max_source_exponent_entries = stats.recenter_source_exponent_entries();
        limits.recentering.max_output_terms = stats.recenter_output_terms();
        limits.recentering.max_output_exponent_entries = stats.recenter_output_exponent_entries();
        limits.recentering.max_power_operations = stats.recenter_power_operations();
        limits.recentering.max_integer_bit_work = stats.recenter_integer_bit_work();
        limits.recentering.max_normalized_coefficient_terms =
            stats.recenter_normalized_coefficient_terms();
        limits.recentering.max_retained_bytes = stats.recenter_retained_bytes();
        limits.max_scope_comparison_bytes = stats.scope_comparison_bytes();
        limits.max_reelimination_replays = stats.reelimination_replays();
        limits.max_source_case_authentications = stats.source_case_authentications();
        limits.max_group_authentications = stats.group_authentications();
        limits.max_pivots = stats.pivots();
        limits.max_ambient_arity = stats.ambient_arity();
        limits.max_free_positions = stats.free_positions();
        limits.max_compact_matrix_entries = stats.compact_matrix_entries();
        limits.max_group_targets = stats.group_targets();
        limits.max_target_checks = stats.target_checks();
        limits.max_target_handle_work = stats.target_handle_work();
        limits.max_target_resolution_scope_bytes = stats.target_resolution_scope_bytes();
        limits.max_target_resolution_work = stats.target_resolution_work();
        limits.max_target_witnesses = stats.target_witnesses();
        limits.max_matching_target_references = stats.matching_target_references();
        limits.max_affine_operations = stats.affine_operations();
        limits.max_affine_integer_bit_work = stats.affine_integer_bit_work();
        limits.max_affine_integer_bits = stats.maximum_affine_integer_bits();
        limits.max_transformed_constant_entries = stats.transformed_constant_entries();
        limits.max_transformed_integer_bit_envelope = stats.transformed_integer_bit_envelope();
        limits.max_transformed_integer_heap_byte_envelope =
            stats.transformed_integer_heap_byte_envelope();
        limits.max_target_comparison_entries = stats.target_comparison_entries();
        limits.max_target_comparison_integer_bit_work = stats.target_comparison_integer_bit_work();
        limits.max_recentering_attempts = stats.recentering_attempts();
        limits.max_recentering_boundary_checks = stats.recentering_boundary_checks();
        limits.max_retained_shift_components = stats.retained_shift_components();
        limits.max_row_label_bytes = stats.row_label_bytes();
        limits.max_no_target_outcomes = stats.no_target_outcomes();
        limits.max_recentering_boundary_outcomes = stats.recentering_boundary_outcomes();
        limits.max_pending_outcomes = stats.pending_outcomes();
        limits.max_owner_retained_bytes = stats.owner_retained_bytes();
        limits.max_peak_scratch_bytes = stats.peak_scratch_bytes();
        limits
    }

    fn assert_one_below_error(
        error: GeneratedAffineResidualCasePivotTargetMatchingError,
        expected_requested: usize,
    ) {
        assert!(matches!(
            error,
            GeneratedAffineResidualCasePivotTargetMatchingError::ResourceLimit {
                requested,
                limit,
                ..
            } if requested == expected_requested && limit + 1 == requested
        ));
    }

    #[test]
    fn natural_success_has_independent_affine_and_split_recenter_oracles() {
        let fixture = success_fixture();
        let certificate = fixture
            .compile_matcher(GeneratedAffineResidualCasePivotTargetMatchingLimits::default())
            .unwrap();
        let authority = fixture.reelimination.authority();
        let source = authority.authenticated_case_view(&fixture.context).unwrap();
        let group = authority
            .authenticated_group_view(&fixture.context)
            .unwrap();
        let targets = authority
            .same_group_target_cases(
                &fixture.family,
                &fixture.context,
                GeneratedAffineResidualSameGroupTargetCasesLimits::default(),
            )
            .unwrap();
        let pivots = fixture
            .reelimination
            .elimination_for_case_target_matching()
            .pivots();
        assert_eq!(certificate.outcomes().len(), pivots.len());
        assert!(certificate.stats().pending_outcomes() > 0);

        for (ordinal, (outcome, equation)) in certificate.outcomes().iter().zip(pivots).enumerate()
        {
            assert_eq!(outcome.pivot_ordinal(), ordinal);
            let (pivot, transformed, witnesses) = outcome_transcript_parts(outcome);
            assert_eq!(pivot, equation.pivot());

            // Independent exact-Integer implementation of b' = b - A p_F + p.
            let mut expected = Vec::with_capacity(group.ambient_arity());
            for row in 0..group.ambient_arity() {
                let mut value = source.constants()[row].clone();
                for (free_ordinal, &free_position) in group.free_positions().iter().enumerate() {
                    let coefficient = &group.compact_linear_coefficients()
                        [row * group.free_positions().len() + free_ordinal];
                    value = value - coefficient * Integer::from(pivot.values()[free_position]);
                }
                value = value + Integer::from(pivot.values()[row]);
                expected.push(value);
            }
            assert_eq!(transformed, expected);
            assert_eq!(witnesses.len(), targets.len());
            for (position, witness) in witnesses.iter().enumerate() {
                let handle = targets
                    .target(
                        position,
                        GeneratedAffineResidualSameGroupTargetHandleLimits::default(),
                    )
                    .unwrap();
                let resolved = authority
                    .authenticated_same_group_target_case_view(
                        &fixture.family,
                        &fixture.context,
                        handle,
                        GeneratedAffineResidualSameGroupTargetCaseLimits::default(),
                    )
                    .unwrap();
                assert_eq!(witness.target_position(), position);
                assert_eq!(witness.case_ordinal(), resolved.target().ordinal());
                assert_eq!(witness.terminal_locator(), resolved.target().locator());
                assert_eq!(witness.matched(), expected == resolved.target().constants());
            }

            let GeneratedAffineResidualCasePivotTargetOutcome::Pending(pending) = outcome else {
                continue;
            };
            let expected_translation = (0..pivot.arity())
                .map(|position| {
                    if group.free_positions().contains(&position) {
                        -pivot.values()[position]
                    } else {
                        0
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(
                pending.coefficient_translation().values(),
                expected_translation
            );
            assert_eq!(pending.key_center(), pivot);
            let recentered = pending.relation_for_future_when_bad();
            assert_eq!(
                recentered.terms().len(),
                equation.unit_relation().terms().len()
            );
            for (source_key, source_coefficient) in equation.unit_relation().terms() {
                let expected_key = IndexShift::try_new(
                    source_key
                        .values()
                        .iter()
                        .zip(pivot.values())
                        .map(|(&q, &p)| q.checked_sub(p).unwrap()),
                    pivot.arity(),
                )
                .unwrap();
                let expected_coefficient = fixture
                    .context
                    .translate(
                        source_coefficient,
                        &expected_translation,
                        ParametricArithmeticLimits::default(),
                    )
                    .unwrap();
                assert_eq!(
                    recentered.terms().get(&expected_key),
                    Some(&expected_coefficient)
                );
            }
            assert_eq!(
                recentered.guarded_nonzero_conditions().len(),
                equation.unit_relation().guarded_nonzero_conditions().len()
            );
            for (source_guard, output_guard) in equation
                .unit_relation()
                .guarded_nonzero_conditions()
                .iter()
                .zip(recentered.guarded_nonzero_conditions())
            {
                let translated = fixture
                    .context
                    .translate_polynomial(
                        source_guard.polynomial(),
                        &expected_translation,
                        ParametricArithmeticLimits::default(),
                    )
                    .unwrap();
                assert_eq!(output_guard.polynomial(), &translated);
                assert!(output_guard.origins().contains(
                    &GuardOrigin::RelationAffineFreeRecentering {
                        source_row: equation.unit_relation().row_id().guard_identity(),
                        target_row: recentered.row_id().guard_identity(),
                        coefficient_offset: expected_translation.clone(),
                        key_center: pivot.values().to_vec(),
                    }
                ));
            }
        }
        assert_eq!(certificate.targets_consumed(), 0);
        assert!(!certificate.publishes_rules());
        certificate
            .replay(
                &fixture.family,
                &fixture.context,
                &fixture.reelimination,
                GeneratedAffineResidualCasePivotTargetMatchingReplayLimits::default(),
            )
            .unwrap();
    }

    #[test]
    fn exact_large_integer_transform_agrees_with_an_independent_oracle() {
        let huge = Integer::from(u128::MAX) * Integer::from(17) + Integer::from(3);
        assert!(matches!(huge, Integer::Large(_)));
        let source = vec![huge.clone(), -huge.clone()];
        let matrix = vec![Integer::from(3), Integer::from(-5)];
        let free_positions = vec![1usize];
        let pivot = IndexShift::try_new([7, -11], 2).unwrap();
        let prepared =
            prepare_transformed_constants(&source, &matrix, &free_positions, &pivot).unwrap();
        let actual =
            execute_transformed_constants(&source, &matrix, &free_positions, &pivot, prepared)
                .unwrap();
        let expected = source
            .iter()
            .enumerate()
            .map(|(row, constant)| {
                constant.clone() - &matrix[row] * Integer::from(pivot.values()[1])
                    + Integer::from(pivot.values()[row])
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        assert!(
            actual
                .iter()
                .all(|value| matches!(value, Integer::Large(_)))
        );
    }

    #[test]
    fn natural_multi_case_no_target_preserves_complete_order_and_stops_pre_when_bad() {
        let fixture = multi_case_fixture();
        let certificate = fixture
            .compile_matcher(GeneratedAffineResidualCasePivotTargetMatchingLimits::default())
            .unwrap();
        let authority = fixture.reelimination.authority();
        let group = authority
            .authenticated_group_view(&fixture.context)
            .unwrap();
        let targets = authority
            .same_group_target_cases(
                &fixture.family,
                &fixture.context,
                GeneratedAffineResidualSameGroupTargetCasesLimits::default(),
            )
            .unwrap();
        assert!(targets.len() > 1);
        assert_eq!(targets.len(), group.case_ordinals().len());
        assert!(certificate.stats().no_target_outcomes() > 0);
        assert_eq!(certificate.stats().recentering_boundary_outcomes(), 0);
        assert_eq!(certificate.stats().targets_consumed(), 0);

        let pivots = fixture
            .reelimination
            .elimination_for_case_target_matching()
            .pivots();
        assert_eq!(certificate.outcomes().len(), pivots.len());
        for (ordinal, (outcome, equation)) in certificate.outcomes().iter().zip(pivots).enumerate()
        {
            assert_eq!(outcome.pivot_ordinal(), ordinal);
            let (pivot, _, witnesses) = outcome_transcript_parts(outcome);
            assert_eq!(pivot, equation.pivot());
            assert_eq!(witnesses.len(), targets.len());
            for (position, witness) in witnesses.iter().enumerate() {
                let handle = targets
                    .target(
                        position,
                        GeneratedAffineResidualSameGroupTargetHandleLimits::default(),
                    )
                    .unwrap();
                assert_eq!(witness.target_position(), position);
                assert_eq!(witness.case_ordinal(), handle.case_ordinal());
                assert_eq!(handle.ordinal_within_group(), position);
            }
            if let GeneratedAffineResidualCasePivotTargetOutcome::RejectedNoTarget(rejected) =
                outcome
            {
                assert_eq!(rejected.matching_target_count(), 0);
                assert!(
                    rejected
                        .target_witnesses()
                        .iter()
                        .all(|witness| !witness.matched())
                );
            }

            // The generated depth-0 support theorem keeps p and q in
            // [-66, 65], hence every checked q-p lies in [-131, 131].
            assert!(
                pivot
                    .values()
                    .iter()
                    .all(|&value| (-66..=65).contains(&value))
            );
            for key in equation.unit_relation().terms().keys() {
                assert!(
                    key.values()
                        .iter()
                        .all(|&value| (-66..=65).contains(&value))
                );
                assert!(
                    key.values()
                        .iter()
                        .zip(pivot.values())
                        .all(|(&q, &p)| (-131..=131).contains(&(q - p)))
                );
            }
        }
        assert_eq!(certificate.targets_consumed(), 0);
        assert!(!certificate.publishes_rules());
        assert!(
            certificate
                .outcomes()
                .iter()
                .all(|outcome| outcome.targets_consumed() == 0)
        );
    }

    #[test]
    fn replay_requires_the_exact_reelimination_allocation() {
        let fixture = success_fixture();
        let certificate = fixture
            .compile_matcher(GeneratedAffineResidualCasePivotTargetMatchingLimits::default())
            .unwrap();
        let foreign_equal = Arc::new((*fixture.reelimination).clone());
        assert!(!Arc::ptr_eq(&foreign_equal, &fixture.reelimination));
        foreign_equal
            .replay(
                &fixture.family,
                &fixture.context,
                foreign_equal.authority(),
                foreign_equal.premises(),
                foreign_equal.ordering(),
                foreign_equal.schedule(),
            )
            .unwrap();
        assert_eq!(
            certificate.replay(
                &fixture.family,
                &fixture.context,
                &foreign_equal,
                GeneratedAffineResidualCasePivotTargetMatchingReplayLimits::default(),
            ),
            Err(GeneratedAffineResidualCasePivotTargetMatchingError::WrongReeliminationAllocation)
        );
    }

    #[test]
    fn construction_and_replay_have_exact_and_one_below_resource_boundaries() {
        let fixture = success_fixture();
        let baseline = fixture
            .compile_matcher(GeneratedAffineResidualCasePivotTargetMatchingLimits::default())
            .unwrap();
        let stats = baseline.stats();
        let exact = exact_construction_limits(stats);
        let exact_certificate = fixture.compile_matcher(exact).unwrap();
        assert_eq!(exact_certificate.stats(), stats);

        type Setter = fn(&mut GeneratedAffineResidualCasePivotTargetMatchingLimits, usize);
        macro_rules! axis {
            ($name:literal, $requested:expr, $setter:expr) => {
                ($name, $requested, $setter as Setter)
            };
        }
        let axes = vec![
            axis!(
                "same-group scope",
                stats.same_group_scope_comparison_bytes(),
                |l, v| l.same_group_targets.max_scope_comparison_bytes = v
            ),
            axis!(
                "same-group case lookups",
                stats.same_group_case_lookups(),
                |l, v| l.same_group_targets.max_case_lookups = v
            ),
            axis!(
                "same-group group lookups",
                stats.same_group_group_lookups(),
                |l, v| l.same_group_targets.max_group_lookups = v
            ),
            axis!(
                "same-group ordinals",
                stats.same_group_ordinal_comparisons(),
                |l, v| l.same_group_targets.max_ordinal_comparisons = v
            ),
            axis!(
                "same-group shape",
                stats.same_group_shape_comparisons(),
                |l, v| l.same_group_targets.max_shape_comparisons = v
            ),
            axis!(
                "same-group targets",
                stats.same_group_target_case_references(),
                |l, v| l.same_group_targets.max_target_case_references = v
            ),
            axis!(
                "handle positions",
                stats.maximum_target_position_lookups(),
                |l, v| l.target_handle.max_target_position_lookups = v
            ),
            axis!(
                "handle cases",
                stats.maximum_target_handle_case_lookups(),
                |l, v| l.target_handle.max_case_lookups = v
            ),
            axis!(
                "handle anchors",
                stats.maximum_target_anchor_offset_lookups(),
                |l, v| l.target_handle.max_anchor_offset_lookups = v
            ),
            axis!(
                "handle ordinals",
                stats.maximum_target_handle_ordinal_comparisons(),
                |l, v| l.target_handle.max_ordinal_comparisons = v
            ),
            axis!(
                "resolver scope",
                stats.maximum_target_case_scope_comparison_bytes(),
                |l, v| l.target_case.max_scope_comparison_bytes = v
            ),
            axis!(
                "resolver allocation",
                stats.maximum_target_authority_allocation_comparisons(),
                |l, v| l.target_case.max_authority_allocation_comparisons = v
            ),
            axis!(
                "resolver cases",
                stats.maximum_target_case_lookups(),
                |l, v| l.target_case.max_case_lookups = v
            ),
            axis!(
                "resolver groups",
                stats.maximum_target_group_lookups(),
                |l, v| l.target_case.max_group_lookups = v
            ),
            axis!(
                "resolver ordinals",
                stats.maximum_target_case_ordinal_comparisons(),
                |l, v| l.target_case.max_ordinal_comparisons = v
            ),
            axis!(
                "resolver geometry",
                stats.maximum_target_geometry_reference_comparisons(),
                |l, v| l.target_case.max_geometry_reference_comparisons = v
            ),
            axis!("recenter terms", stats.recenter_terms(), |l, v| l
                .recentering
                .max_terms =
                v),
            axis!("recenter guards", stats.recenter_guards(), |l, v| l
                .recentering
                .max_guards =
                v),
            axis!(
                "recenter translations",
                stats.recenter_translation_components(),
                |l, v| l.recentering.max_translation_components = v
            ),
            axis!(
                "recenter key checks",
                stats.recenter_key_subtraction_boundary_checks(),
                |l, v| l.recentering.max_key_subtraction_boundary_checks = v
            ),
            axis!(
                "recenter source terms",
                stats.recenter_source_terms(),
                |l, v| l.recentering.max_source_terms = v
            ),
            axis!(
                "recenter source exponents",
                stats.recenter_source_exponent_entries(),
                |l, v| l.recentering.max_source_exponent_entries = v
            ),
            axis!(
                "recenter output terms",
                stats.recenter_output_terms(),
                |l, v| l.recentering.max_output_terms = v
            ),
            axis!(
                "recenter output exponents",
                stats.recenter_output_exponent_entries(),
                |l, v| l.recentering.max_output_exponent_entries = v
            ),
            axis!(
                "recenter powers",
                stats.recenter_power_operations(),
                |l, v| l.recentering.max_power_operations = v
            ),
            axis!(
                "recenter integer work",
                stats.recenter_integer_bit_work(),
                |l, v| l.recentering.max_integer_bit_work = v
            ),
            axis!(
                "recenter normalized terms",
                stats.recenter_normalized_coefficient_terms(),
                |l, v| l.recentering.max_normalized_coefficient_terms = v
            ),
            axis!(
                "recenter retained",
                stats.recenter_retained_bytes(),
                |l, v| l.recentering.max_retained_bytes = v
            ),
            axis!("scope", stats.scope_comparison_bytes(), |l, v| l
                .max_scope_comparison_bytes =
                v),
            axis!(
                "re-elimination replay",
                stats.reelimination_replays(),
                |l, v| l.max_reelimination_replays = v
            ),
            axis!(
                "source authentication",
                stats.source_case_authentications(),
                |l, v| l.max_source_case_authentications = v
            ),
            axis!(
                "group authentication",
                stats.group_authentications(),
                |l, v| l.max_group_authentications = v
            ),
            axis!("pivots", stats.pivots(), |l, v| l.max_pivots = v),
            axis!("ambient arity", stats.ambient_arity(), |l, v| l
                .max_ambient_arity =
                v),
            axis!("free positions", stats.free_positions(), |l, v| l
                .max_free_positions =
                v),
            axis!("matrix entries", stats.compact_matrix_entries(), |l, v| l
                .max_compact_matrix_entries =
                v),
            axis!("group targets", stats.group_targets(), |l, v| l
                .max_group_targets =
                v),
            axis!("target checks", stats.target_checks(), |l, v| l
                .max_target_checks =
                v),
            axis!("handle work", stats.target_handle_work(), |l, v| l
                .max_target_handle_work =
                v),
            axis!(
                "resolver scope total",
                stats.target_resolution_scope_bytes(),
                |l, v| l.max_target_resolution_scope_bytes = v
            ),
            axis!("resolver work", stats.target_resolution_work(), |l, v| l
                .max_target_resolution_work =
                v),
            axis!("witnesses", stats.target_witnesses(), |l, v| l
                .max_target_witnesses =
                v),
            axis!("matches", stats.matching_target_references(), |l, v| l
                .max_matching_target_references =
                v),
            axis!("affine operations", stats.affine_operations(), |l, v| l
                .max_affine_operations =
                v),
            axis!(
                "affine integer work",
                stats.affine_integer_bit_work(),
                |l, v| l.max_affine_integer_bit_work = v
            ),
            axis!(
                "affine max bits",
                stats.maximum_affine_integer_bits(),
                |l, v| l.max_affine_integer_bits = v
            ),
            axis!(
                "transformed entries",
                stats.transformed_constant_entries(),
                |l, v| l.max_transformed_constant_entries = v
            ),
            axis!(
                "transformed bits",
                stats.transformed_integer_bit_envelope(),
                |l, v| l.max_transformed_integer_bit_envelope = v
            ),
            axis!(
                "transformed heap",
                stats.transformed_integer_heap_byte_envelope(),
                |l, v| l.max_transformed_integer_heap_byte_envelope = v
            ),
            axis!(
                "comparison entries",
                stats.target_comparison_entries(),
                |l, v| l.max_target_comparison_entries = v
            ),
            axis!(
                "comparison integer work",
                stats.target_comparison_integer_bit_work(),
                |l, v| l.max_target_comparison_integer_bit_work = v
            ),
            axis!("recenter attempts", stats.recentering_attempts(), |l, v| {
                l.max_recentering_attempts = v
            }),
            axis!(
                "recenter boundary checks",
                stats.recentering_boundary_checks(),
                |l, v| l.max_recentering_boundary_checks = v
            ),
            axis!(
                "retained shifts",
                stats.retained_shift_components(),
                |l, v| l.max_retained_shift_components = v
            ),
            axis!("row labels", stats.row_label_bytes(), |l, v| l
                .max_row_label_bytes =
                v),
            axis!("no-target outcomes", stats.no_target_outcomes(), |l, v| l
                .max_no_target_outcomes =
                v),
            axis!(
                "boundary outcomes",
                stats.recentering_boundary_outcomes(),
                |l, v| l.max_recentering_boundary_outcomes = v
            ),
            axis!("pending outcomes", stats.pending_outcomes(), |l, v| l
                .max_pending_outcomes =
                v),
            axis!("owner retained", stats.owner_retained_bytes(), |l, v| l
                .max_owner_retained_bytes =
                v),
            axis!("peak scratch", stats.peak_scratch_bytes(), |l, v| l
                .max_peak_scratch_bytes =
                v),
        ];
        let shard_len = axes.len().div_ceil(4);
        thread::scope(|scope| {
            for shard in axes.chunks(shard_len) {
                scope.spawn(move || {
                    for &(name, requested, setter) in shard {
                        if requested == 0 {
                            continue;
                        }
                        let mut rejected = exact;
                        setter(&mut rejected, requested - 1);
                        let error = fixture.compile_matcher(rejected).unwrap_err();
                        assert!(
                            matches!(
                                error,
                                GeneratedAffineResidualCasePivotTargetMatchingError::ResourceLimit {
                                    requested: actual,
                                    limit,
                                    ..
                                } if actual == requested && limit + 1 == actual
                            ),
                            "axis {name} returned {error:?} for exact demand {requested}"
                        );
                    }
                });
            }
        });

        let replay_exact = GeneratedAffineResidualCasePivotTargetMatchingReplayLimits {
            max_reelimination_allocation_comparisons: REELIMINATION_ALLOCATION_COMPARISONS,
            max_combined_matcher_owner_bytes: stats.replay_combined_matcher_owner_bytes(),
            max_payload_comparison_units: stats.payload_comparison_units(),
            max_payload_comparison_bytes: stats.payload_comparison_bytes(),
            max_payload_comparison_integer_bits: stats.payload_comparison_integer_bits(),
            max_payload_comparison_relation_manifest_bytes: stats
                .payload_comparison_relation_manifest_bytes(),
        };
        baseline
            .replay(
                &fixture.family,
                &fixture.context,
                &fixture.reelimination,
                replay_exact,
            )
            .unwrap();
        let wrong_family = equal_mass_two_loop_family("pivot-target-resource-wrong-family");
        macro_rules! replay_one_below {
            ($field:ident, $requested:expr) => {{
                let requested = $requested;
                assert!(requested > 0);
                let mut rejected = replay_exact;
                rejected.$field = requested - 1;
                let error = baseline
                    .replay(
                        &wrong_family,
                        &fixture.context,
                        &fixture.reelimination,
                        rejected,
                    )
                    .unwrap_err();
                assert_one_below_error(error, requested);
            }};
        }
        replay_one_below!(
            max_reelimination_allocation_comparisons,
            REELIMINATION_ALLOCATION_COMPARISONS
        );
        replay_one_below!(
            max_combined_matcher_owner_bytes,
            stats.replay_combined_matcher_owner_bytes()
        );
        replay_one_below!(
            max_payload_comparison_units,
            stats.payload_comparison_units()
        );
        replay_one_below!(
            max_payload_comparison_bytes,
            stats.payload_comparison_bytes()
        );
        replay_one_below!(
            max_payload_comparison_integer_bits,
            stats.payload_comparison_integer_bits()
        );
        replay_one_below!(
            max_payload_comparison_relation_manifest_bytes,
            stats.payload_comparison_relation_manifest_bytes()
        );
    }

    #[test]
    fn exact_owner_lifetime_and_parallel_replay_are_allocation_safe() {
        let NaturalFixture {
            family,
            context,
            reelimination,
        } = build_natural_fixture("pivot-target-lifetime-v2", "001", 0);
        let weak_reelimination = Arc::downgrade(&reelimination);
        let weak_authority: Weak<GeneratedAffineResidualCaseAuthority> =
            Arc::downgrade(reelimination.authority());
        let weak_premises = Arc::downgrade(reelimination.premises());
        let weak_ordering = Arc::downgrade(reelimination.ordering());
        let weak_schedule = Arc::downgrade(reelimination.schedule());
        let certificate = Arc::new(
            GeneratedAffineResidualCasePivotTargetMatchingCompiler::compile(
                &family,
                &context,
                Arc::clone(&reelimination),
                GeneratedAffineResidualCasePivotTargetMatchingLimits::default(),
            )
            .unwrap(),
        );
        drop(reelimination);
        assert!(weak_reelimination.upgrade().is_some());
        assert!(weak_authority.upgrade().is_some());
        assert!(weak_premises.upgrade().is_some());
        assert!(weak_ordering.upgrade().is_some());
        assert!(weak_schedule.upgrade().is_some());

        thread::scope(|scope| {
            for _ in 0..4 {
                let certificate = Arc::clone(&certificate);
                let family = &family;
                let context = &context;
                scope.spawn(move || {
                    certificate
                        .replay(
                            family,
                            context,
                            certificate.reelimination(),
                            GeneratedAffineResidualCasePivotTargetMatchingReplayLimits::default(),
                        )
                        .unwrap();
                });
            }
        });
        drop(certificate);
        assert!(weak_reelimination.upgrade().is_none());
        assert!(weak_authority.upgrade().is_none());
        assert!(weak_premises.upgrade().is_none());
        assert!(weak_ordering.upgrade().is_none());
        assert!(weak_schedule.upgrade().is_none());
    }

    #[test]
    fn generated_boundary_oracle_redaction_and_production_topology_scan_are_strict() {
        let fixture = success_fixture();
        let certificate = fixture
            .compile_matcher(GeneratedAffineResidualCasePivotTargetMatchingLimits::default())
            .unwrap();
        let group = fixture
            .reelimination
            .authority()
            .authenticated_group_view(&fixture.context)
            .unwrap();
        let &free_position = group
            .free_positions()
            .first()
            .expect("natural affine success fixture must retain a free position");
        let equation = &fixture
            .reelimination
            .elimination_for_case_target_matching()
            .pivots()[0];
        let mut values = vec![0; group.ambient_arity()];
        values[free_position] = i64::MIN;
        let boundary_pivot = IndexShift::try_new(values, group.ambient_arity()).unwrap();
        let mut boundary_stats = GeneratedAffineResidualCasePivotTargetMatchingStats::default();
        assert_eq!(
            classify_recentering_boundary(
                &mut boundary_stats,
                equation.unit_relation(),
                group.free_positions(),
                &boundary_pivot,
                GeneratedAffineResidualCasePivotTargetMatchingLimits::default(),
            )
            .unwrap(),
            Some((
                free_position,
                GeneratedAffineResidualCaseRecenteringBoundaryKind::FreeCoefficientTranslationNegation,
            ))
        );
        assert_eq!(boundary_stats.recentering_attempts(), 0);
        assert_eq!(
            boundary_stats.recentering_boundary_checks(),
            group.free_positions().len()
        );

        // Use an actual generated unit relation and a center one step above
        // i64::MIN. Its negation is representable (so the free-coordinate
        // check passes), while generated q=+1 gives MAX+1 in q-center.
        let (key_boundary_equation, key_boundary_position) = fixture
            .reelimination
            .elimination_for_case_target_matching()
            .pivots()
            .iter()
            .find_map(|candidate| {
                candidate.unit_relation().terms().keys().find_map(|key| {
                    key.values()
                        .iter()
                        .enumerate()
                        .find(|(_, value)| **value > 0)
                        .map(|(position, _)| (candidate, position))
                })
            })
            .expect("generated support must expose a positive key component");
        let mut key_center = vec![0; group.ambient_arity()];
        key_center[key_boundary_position] = i64::MIN + 1;
        let key_center = IndexShift::try_new(key_center, group.ambient_arity()).unwrap();
        let mut key_boundary_stats = GeneratedAffineResidualCasePivotTargetMatchingStats::default();
        assert_eq!(
            classify_recentering_boundary(
                &mut key_boundary_stats,
                key_boundary_equation.unit_relation(),
                group.free_positions(),
                &key_center,
                GeneratedAffineResidualCasePivotTargetMatchingLimits::default(),
            )
            .unwrap(),
            Some((
                key_boundary_position,
                GeneratedAffineResidualCaseRecenteringBoundaryKind::IntegralKeySubtraction,
            ))
        );
        assert_eq!(key_boundary_stats.recentering_attempts(), 1);
        assert_eq!(
            key_boundary_stats.recentering_boundary_checks(),
            group.free_positions().len()
                + key_boundary_equation.unit_relation().terms().len() * group.ambient_arity()
        );

        let debug = format!("{certificate:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("pivot-target-success-v2"));
        let outcome_debug = format!("{:?}", certificate.outcomes());
        assert!(outcome_debug.contains("<redacted>"));
        assert!(!outcome_debug.contains(fixture.family.fingerprint_ref()));
        assert!(!outcome_debug.contains(fixture.context.fingerprint()));
        assert!(!outcome_debug.contains("generated-affine-case-pivot-target-pending-v2"));
        assert!(!outcome_debug.contains("private-proof-payload"));
        let diagnostic = format!(
            "{:?} / {}",
            GeneratedAffineResidualCasePivotTargetMatchingError::ResourceLimit {
                resource: "private-proof-payload",
                requested: 17,
                limit: 16,
            },
            GeneratedAffineResidualCasePivotTargetMatchingError::ResourceLimit {
                resource: "private-proof-payload",
                requested: 17,
                limit: 16,
            }
        );
        assert!(!diagnostic.contains("private-proof-payload"));
        assert!(!diagnostic.contains("17"));
        assert!(!diagnostic.contains("16"));

        let production = include_str!("generated_affine_residual_case_pivot_target_matching.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        for forbidden in [
            "massive",
            "vacuum",
            "sunset",
            "two_loop",
            "three_loop",
            "\"001\"",
            "\"011\"",
        ] {
            assert!(
                !production.contains(forbidden),
                "production matcher contains topology-specific token {forbidden}"
            );
        }
        assert!(production.contains("catch_unwind"));
    }
}

fn i64_magnitude_bits(value: i64) -> usize {
    (i64::BITS - value.unsigned_abs().leading_zeros()) as usize
}

fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn remaining(
    resource: &'static str,
    limit: usize,
    used: usize,
) -> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError> {
    limit.checked_sub(used).ok_or(
        GeneratedAffineResidualCasePivotTargetMatchingError::ResourceLimit {
            resource,
            requested: used,
            limit,
        },
    )
}

fn checked_sum(
    resource: &'static str,
    values: impl IntoIterator<Item = usize>,
) -> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError> {
    values
        .into_iter()
        .try_fold(0usize, |sum, value| checked_add(resource, sum, value))
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError> {
    left.checked_add(right).ok_or(
        GeneratedAffineResidualCasePivotTargetMatchingError::ResourceCountOverflow { resource },
    )
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError> {
    left.checked_mul(right).ok_or(
        GeneratedAffineResidualCasePivotTargetMatchingError::ResourceCountOverflow { resource },
    )
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedAffineResidualCasePivotTargetMatchingError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualCasePivotTargetMatchingError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(
            GeneratedAffineResidualCasePivotTargetMatchingError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    }
}

fn try_reserve_exact<T>(
    resource: &'static str,
    target: &mut Vec<T>,
    additional: usize,
) -> Result<(), GeneratedAffineResidualCasePivotTargetMatchingError> {
    let _requested = checked_add(resource, target.len(), additional)?;
    target.try_reserve_exact(additional).map_err(|_| {
        GeneratedAffineResidualCasePivotTargetMatchingError::AllocationFailure { resource }
    })
}
