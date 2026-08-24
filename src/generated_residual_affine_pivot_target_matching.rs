//! Authenticated target matching for generated residual-affine pivots.
//!
//! This is deliberately a pre-`WhenBad` seam.  It matches each private pivot
//! produced by branch-bound re-elimination against the complete persisted
//! priority order of its global affine-geometry group, then performs the exact
//! split recentering required by LiteRed's dependent symbolic starts.  A
//! successful match is retained only as [`GeneratedResidualAffinePendingWhenBad`]:
//! it is not an applicable rule and it consumes no target case.

use std::collections::BTreeSet;
use std::fmt;
use std::fmt::Write as _;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::domains::integer::Integer;

use crate::parametric_relation::{
    ParametricAffineFreeRecenteringLimits, ParametricAffineFreeRecenteringStats,
};
use crate::{
    GeneratedResidualAffineBranchReeliminationCertificate,
    GeneratedResidualAffineBranchReeliminationError,
    GeneratedResidualAffineCaseInventoryCertificate, GeneratedResidualAffineCaseInventoryError,
    IndexShift, IntegralFamily, ParametricArithmeticLimits, ParametricCoefficientContext,
    ParametricNonZeroCondition, ParametricRelation, ParametricRelationError, ParametricRowId,
};

/// Stable schema for affine grouped matching before affine `WhenBad`.
pub const GENERATED_RESIDUAL_AFFINE_PIVOT_TARGET_MATCHING_V1_SCHEMA: &str =
    "rustred-generated-residual-affine-pivot-target-matching-v1";

/// Aggregate construction, replay, and retained-payload bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedResidualAffinePivotTargetMatchingLimits {
    pub arithmetic: ParametricArithmeticLimits,
    pub max_family_fingerprint_bytes: usize,
    pub max_context_fingerprint_bytes: usize,
    pub max_scope_fingerprint_comparison_bytes: usize,
    pub max_pivots: usize,
    pub max_ambient_arity: usize,
    pub max_free_positions: usize,
    pub max_group_cases: usize,
    pub max_geometry_comparison_entries: usize,
    pub max_geometry_comparison_integer_bit_work: usize,
    pub max_target_checks: usize,
    pub max_checked_target_ordinals: usize,
    pub max_matching_target_ordinals: usize,
    pub max_matching_flag_bytes: usize,
    pub max_affine_operations: usize,
    pub max_affine_integer_bit_work: usize,
    pub max_affine_integer_bits: usize,
    pub max_target_comparison_entries: usize,
    pub max_target_comparison_integer_bit_work: usize,
    pub max_transformed_constant_entries: usize,
    pub max_retained_integer_bits: usize,
    pub max_retained_shift_components: usize,
    pub max_row_label_bytes: usize,
    pub max_recenter_attempts: usize,
    pub max_recenter_terms: usize,
    pub max_recenter_guards: usize,
    pub max_recenter_translation_components: usize,
    pub max_recenter_key_subtraction_boundary_checks: usize,
    pub max_recenter_source_terms: usize,
    pub max_recenter_source_exponent_entries: usize,
    pub max_recenter_output_terms: usize,
    pub max_recenter_output_exponent_entries: usize,
    pub max_recenter_power_operations: usize,
    pub max_recenter_integer_bit_work: usize,
    pub max_recenter_normalized_coefficient_terms: usize,
    pub max_recenter_retained_bytes: usize,
    pub max_retained_payload_bytes: usize,
    pub max_payload_comparison_units: usize,
    pub max_payload_comparison_bytes: usize,
    pub max_payload_comparison_integer_bits: usize,
    pub max_payload_comparison_relation_manifest_bytes: usize,
}

impl Default for GeneratedResidualAffinePivotTargetMatchingLimits {
    fn default() -> Self {
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            max_family_fingerprint_bytes: 1024 * 1024 * 1024,
            max_context_fingerprint_bytes: 1024 * 1024 * 1024,
            max_scope_fingerprint_comparison_bytes: 2 * 1024 * 1024 * 1024,
            max_pivots: 256_000_000,
            max_ambient_arity: 1_000_000,
            max_free_positions: 1_000_000,
            max_group_cases: 256_000_000,
            max_geometry_comparison_entries: 64_000_000_000,
            max_geometry_comparison_integer_bit_work: 4_000_000_000_000_000_000,
            max_target_checks: 64_000_000_000,
            max_checked_target_ordinals: 64_000_000_000,
            max_matching_target_ordinals: 64_000_000_000,
            max_matching_flag_bytes: 256_000_000,
            max_affine_operations: 64_000_000_000,
            max_affine_integer_bit_work: 4_000_000_000_000_000_000,
            max_affine_integer_bits: 1_000_000_000,
            max_target_comparison_entries: 64_000_000_000,
            max_target_comparison_integer_bit_work: 4_000_000_000_000_000_000,
            max_transformed_constant_entries: 64_000_000_000,
            max_retained_integer_bits: 4_000_000_000_000_000_000,
            max_retained_shift_components: 64_000_000_000,
            max_row_label_bytes: 1024 * 1024,
            max_recenter_attempts: 256_000_000,
            max_recenter_terms: 64_000_000_000,
            max_recenter_guards: 64_000_000_000,
            max_recenter_translation_components: 64_000_000_000,
            max_recenter_key_subtraction_boundary_checks: 4_000_000_000_000_000_000,
            max_recenter_source_terms: 64_000_000_000,
            max_recenter_source_exponent_entries: 4_000_000_000_000_000_000,
            max_recenter_output_terms: 64_000_000_000,
            max_recenter_output_exponent_entries: 4_000_000_000_000_000_000,
            max_recenter_power_operations: 4_000_000_000_000_000_000,
            max_recenter_integer_bit_work: 4_000_000_000_000_000_000,
            max_recenter_normalized_coefficient_terms: 64_000_000_000,
            max_recenter_retained_bytes: 64 * 1024 * 1024 * 1024,
            max_retained_payload_bytes: 128 * 1024 * 1024 * 1024,
            max_payload_comparison_units: 64_000_000_000,
            max_payload_comparison_bytes: 256 * 1024 * 1024 * 1024,
            max_payload_comparison_integer_bits: 4_000_000_000_000_000_000,
            max_payload_comparison_relation_manifest_bytes: 128 * 1024 * 1024 * 1024,
        }
    }
}

/// Exact construction census.  Recentered algebra fields are the prospective
/// bounds returned by Symbolica-aware preflight, summed over successful
/// pending candidates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedResidualAffinePivotTargetMatchingStats {
    family_fingerprint_bytes: usize,
    context_fingerprint_bytes: usize,
    scope_fingerprint_comparison_bytes: usize,
    pivots: usize,
    ambient_arity: usize,
    free_positions: usize,
    group_cases: usize,
    geometry_comparison_entries: usize,
    geometry_comparison_integer_bit_work: usize,
    target_checks: usize,
    checked_target_ordinals: usize,
    matching_target_ordinals: usize,
    maximum_matching_flag_bytes: usize,
    affine_operations: usize,
    affine_integer_bit_work: usize,
    maximum_affine_integer_bits: usize,
    target_comparison_entries: usize,
    target_comparison_integer_bit_work: usize,
    transformed_constant_entries: usize,
    retained_integer_bits: usize,
    retained_shift_components: usize,
    row_label_bytes: usize,
    rejected_no_target_cases: usize,
    rejected_recentering_boundaries: usize,
    pending_when_bad: usize,
    targets_consumed: usize,
    recenter_attempts: usize,
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
    retained_payload_bytes: usize,
    payload_comparison_units: usize,
    payload_comparison_bytes: usize,
    payload_comparison_integer_bits: usize,
    payload_comparison_relation_manifest_bytes: usize,
}

macro_rules! stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedResidualAffinePivotTargetMatchingStats {
    stats_getters!(
        family_fingerprint_bytes,
        context_fingerprint_bytes,
        scope_fingerprint_comparison_bytes,
        pivots,
        ambient_arity,
        free_positions,
        group_cases,
        geometry_comparison_entries,
        geometry_comparison_integer_bit_work,
        target_checks,
        checked_target_ordinals,
        matching_target_ordinals,
        maximum_matching_flag_bytes,
        affine_operations,
        affine_integer_bit_work,
        maximum_affine_integer_bits,
        target_comparison_entries,
        target_comparison_integer_bit_work,
        transformed_constant_entries,
        retained_integer_bits,
        retained_shift_components,
        row_label_bytes,
        rejected_no_target_cases,
        rejected_recentering_boundaries,
        pending_when_bad,
        targets_consumed,
        recenter_attempts,
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
        retained_payload_bytes,
        payload_comparison_units,
        payload_comparison_bytes,
        payload_comparison_integer_bits,
        payload_comparison_relation_manifest_bytes,
    );
}

/// Why a matched pivot could not be split-recentered on the retained `i64`
/// ambient key lattice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedResidualAffineRecenteringBoundaryKind {
    FreeCoefficientTranslationNegation,
    IntegralKeySubtraction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedResidualAffineRejectedNoTargetCase {
    pivot_ordinal: usize,
    pivot: IndexShift,
    transformed_target_constants: Vec<Integer>,
    checked_target_case_ordinals: Vec<usize>,
}

impl GeneratedResidualAffineRejectedNoTargetCase {
    pub const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }
    pub const fn pivot(&self) -> &IndexShift {
        &self.pivot
    }
    pub fn transformed_target_constants(&self) -> &[Integer] {
        &self.transformed_target_constants
    }
    pub fn checked_target_case_ordinals(&self) -> &[usize] {
        &self.checked_target_case_ordinals
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedResidualAffineRejectedRecenteringBoundary {
    pivot_ordinal: usize,
    pivot: IndexShift,
    transformed_target_constants: Vec<Integer>,
    checked_target_case_ordinals: Vec<usize>,
    matching_target_case_ordinals: Vec<usize>,
    kind: GeneratedResidualAffineRecenteringBoundaryKind,
    position: usize,
}

impl GeneratedResidualAffineRejectedRecenteringBoundary {
    pub const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }
    pub const fn pivot(&self) -> &IndexShift {
        &self.pivot
    }
    pub fn transformed_target_constants(&self) -> &[Integer] {
        &self.transformed_target_constants
    }
    pub fn checked_target_case_ordinals(&self) -> &[usize] {
        &self.checked_target_case_ordinals
    }
    pub fn matching_target_case_ordinals(&self) -> &[usize] {
        &self.matching_target_case_ordinals
    }
    pub const fn kind(&self) -> GeneratedResidualAffineRecenteringBoundaryKind {
        self.kind
    }
    pub const fn position(&self) -> usize {
        self.position
    }
}

/// A target-authenticated, split-recentered candidate awaiting affine
/// `WhenBad`.  Its relation remains crate-private and cannot be published or
/// applied through this API.
#[derive(Clone)]
pub struct GeneratedResidualAffinePendingWhenBad {
    pivot_ordinal: usize,
    pivot: IndexShift,
    transformed_target_constants: Vec<Integer>,
    checked_target_case_ordinals: Vec<usize>,
    matching_target_case_ordinals: Vec<usize>,
    coefficient_translation: IndexShift,
    key_center: IndexShift,
    relation: Arc<ParametricRelation>,
    recentering_stats: ParametricAffineFreeRecenteringStats,
}

impl fmt::Debug for GeneratedResidualAffinePendingWhenBad {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffinePendingWhenBad")
            .field("pivot_ordinal", &self.pivot_ordinal)
            .field("pivot", &self.pivot)
            .field(
                "transformed_target_constants",
                &self.transformed_target_constants,
            )
            .field(
                "checked_target_case_ordinals",
                &self.checked_target_case_ordinals,
            )
            .field(
                "matching_target_case_ordinals",
                &self.matching_target_case_ordinals,
            )
            .field("coefficient_translation", &self.coefficient_translation)
            .field("key_center", &self.key_center)
            .field("recentered_term_count", &self.relation.terms().len())
            .field(
                "recentered_guard_count",
                &self.relation.guarded_nonzero_conditions().len(),
            )
            .field("private_relation", &"<redacted>")
            .finish()
    }
}

impl GeneratedResidualAffinePendingWhenBad {
    pub const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }
    pub const fn pivot(&self) -> &IndexShift {
        &self.pivot
    }
    pub fn transformed_target_constants(&self) -> &[Integer] {
        &self.transformed_target_constants
    }
    pub fn checked_target_case_ordinals(&self) -> &[usize] {
        &self.checked_target_case_ordinals
    }
    pub fn matching_target_case_ordinals(&self) -> &[usize] {
        &self.matching_target_case_ordinals
    }
    /// Select the first exact target which a future affine-`WhenBad` driver
    /// has not consumed. This layer itself never mutates `consumed`.
    pub fn first_available_target_case_ordinal(&self, consumed: &BTreeSet<usize>) -> Option<usize> {
        match self
            .first_available_target_for_effective_coverage(
                |case_ordinal| consumed.contains(&case_ordinal),
                usize::MAX,
            )
            .expect("a retained matching-list length always fits usize")
        {
            GeneratedResidualAffineEffectiveTargetSelection::Selected { case_ordinal, .. } => {
                Some(case_ordinal)
            }
            GeneratedResidualAffineEffectiveTargetSelection::Exhausted { .. } => None,
        }
    }
    /// The same persisted-order scan used by
    /// [`Self::first_available_target_case_ordinal`], with the exact list
    /// position and reference census needed by the transactional group owner.
    /// No caller has to rescan, sort, deduplicate, or reinterpret the retained
    /// matching list.
    pub(crate) fn first_available_target_for_effective_coverage<F>(
        &self,
        mut target_is_consumed: F,
        max_references_inspected: usize,
    ) -> Result<
        GeneratedResidualAffineEffectiveTargetSelection,
        GeneratedResidualAffineEffectiveTargetSelectionError,
    >
    where
        F: FnMut(usize) -> bool,
    {
        for position in 0..self.matching_target_case_ordinals.len() {
            let references_inspected = position.checked_add(1).ok_or(
                GeneratedResidualAffineEffectiveTargetSelectionError::ResourceCountOverflow,
            )?;
            if references_inspected > max_references_inspected {
                return Err(
                    GeneratedResidualAffineEffectiveTargetSelectionError::ResourceLimit {
                        requested: references_inspected,
                        limit: max_references_inspected,
                    },
                );
            }
            let case_ordinal = self.matching_target_case_ordinals[position];
            if !target_is_consumed(case_ordinal) {
                return Ok(GeneratedResidualAffineEffectiveTargetSelection::Selected {
                    case_ordinal,
                    position,
                    references_inspected,
                });
            }
        }
        Ok(GeneratedResidualAffineEffectiveTargetSelection::Exhausted {
            references_inspected: self.matching_target_case_ordinals.len(),
        })
    }
    pub const fn coefficient_translation(&self) -> &IndexShift {
        &self.coefficient_translation
    }
    pub const fn key_center(&self) -> &IndexShift {
        &self.key_center
    }
    /// This seam never publishes an applicable rule.
    pub const fn is_applicable_rule(&self) -> bool {
        false
    }
    /// If a future affine `WhenBad` result is identically true, only this
    /// pivot is excluded; none of the exact target candidates was consumed.
    pub const fn target_remains_available_if_when_bad_is_true(&self) -> bool {
        true
    }
    pub fn recentered_term_count(&self) -> usize {
        self.relation.terms().len()
    }
    pub fn recentered_guard_count(&self) -> usize {
        self.relation.guarded_nonzero_conditions().len()
    }
    /// Conservative observed owned bytes of the private recentered row. Only
    /// the census is exposed; the relation, coefficients, guards, and row id
    /// remain private.
    pub fn recentered_owned_retained_byte_bound(&self) -> Option<usize> {
        self.relation.owned_retained_byte_bound()
    }
    /// Prospective envelope admitted before the private row was constructed.
    pub const fn recentered_retained_byte_envelope(&self) -> usize {
        self.recentering_stats.retained_bytes()
    }
    pub(crate) const fn relation_for_affine_when_bad(&self) -> &Arc<ParametricRelation> {
        &self.relation
    }
}

/// Censused result of the pending matcher's one persisted-order availability
/// scan. This is crate-private because it exists only for the effective group
/// transition owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedResidualAffineEffectiveTargetSelection {
    Selected {
        case_ordinal: usize,
        position: usize,
        references_inspected: usize,
    },
    Exhausted {
        references_inspected: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedResidualAffineEffectiveTargetSelectionError {
    ResourceCountOverflow,
    ResourceLimit { requested: usize, limit: usize },
}

#[derive(Clone)]
pub enum GeneratedResidualAffinePivotTargetOutcome {
    RejectedNoTargetCase(GeneratedResidualAffineRejectedNoTargetCase),
    RejectedRecenteringBoundary(GeneratedResidualAffineRejectedRecenteringBoundary),
    PendingAffineWhenBad(GeneratedResidualAffinePendingWhenBad),
}

impl fmt::Debug for GeneratedResidualAffinePivotTargetOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RejectedNoTargetCase(value) => formatter
                .debug_tuple("RejectedNoTargetCase")
                .field(value)
                .finish(),
            Self::RejectedRecenteringBoundary(value) => formatter
                .debug_tuple("RejectedRecenteringBoundary")
                .field(value)
                .finish(),
            Self::PendingAffineWhenBad(value) => formatter
                .debug_tuple("PendingAffineWhenBad")
                .field(value)
                .finish(),
        }
    }
}

impl GeneratedResidualAffinePivotTargetOutcome {
    pub const fn pivot_ordinal(&self) -> usize {
        match self {
            Self::RejectedNoTargetCase(value) => value.pivot_ordinal,
            Self::RejectedRecenteringBoundary(value) => value.pivot_ordinal,
            Self::PendingAffineWhenBad(value) => value.pivot_ordinal,
        }
    }
}

/// Replayable result for every private pivot of one exact branch
/// re-elimination.
#[derive(Clone)]
pub struct GeneratedResidualAffinePivotTargetMatchingCertificate {
    schema: &'static str,
    inventory: Arc<GeneratedResidualAffineCaseInventoryCertificate>,
    source_case_ordinal: usize,
    source_group_ordinal: usize,
    reelimination: Arc<GeneratedResidualAffineBranchReeliminationCertificate>,
    outcomes: Vec<GeneratedResidualAffinePivotTargetOutcome>,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
    stats: GeneratedResidualAffinePivotTargetMatchingStats,
}

impl fmt::Debug for GeneratedResidualAffinePivotTargetMatchingCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffinePivotTargetMatchingCertificate")
            .field("schema", &self.schema)
            .field("source_case_ordinal", &self.source_case_ordinal)
            .field("source_group_ordinal", &self.source_group_ordinal)
            .field("outcomes", &self.outcomes)
            .field("limits", &self.limits)
            .field("stats", &self.stats)
            .field("private_inventory", &"<redacted>")
            .field("private_reelimination", &"<redacted>")
            .finish()
    }
}

impl GeneratedResidualAffinePivotTargetMatchingCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub const fn inventory(&self) -> &Arc<GeneratedResidualAffineCaseInventoryCertificate> {
        &self.inventory
    }
    pub const fn source_case_ordinal(&self) -> usize {
        self.source_case_ordinal
    }
    pub const fn source_group_ordinal(&self) -> usize {
        self.source_group_ordinal
    }
    pub const fn reelimination(
        &self,
    ) -> &Arc<GeneratedResidualAffineBranchReeliminationCertificate> {
        &self.reelimination
    }
    pub fn outcomes(&self) -> &[GeneratedResidualAffinePivotTargetOutcome] {
        &self.outcomes
    }
    pub const fn limits(&self) -> GeneratedResidualAffinePivotTargetMatchingLimits {
        self.limits
    }
    pub const fn stats(&self) -> GeneratedResidualAffinePivotTargetMatchingStats {
        self.stats
    }
    /// Unrecentered source-branch premises retained only for replay lineage.
    /// They are not an applicability domain. A future affine-`WhenBad`
    /// compiler must obtain the selected target case's own authenticated
    /// branch/guard composition and combine it with the recentered relation
    /// guards.
    pub fn source_branch_premises_for_provenance(
        &self,
    ) -> impl Iterator<Item = &ParametricNonZeroCondition> {
        self.reelimination.common_premises()
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
        validate_schema_for_replay(self.schema)?;
        catch_unwind(AssertUnwindSafe(|| {
            let geometry_census = validate_authorities(
                family,
                context,
                &self.inventory,
                self.source_case_ordinal,
                &self.reelimination,
                self.limits,
            )?;
            self.inventory.replay(family, context)?;
            self.reelimination.replay(family, context)?;
            let replayed = compile_replayed(
                family,
                context,
                self.inventory.clone(),
                self.source_case_ordinal,
                self.reelimination.clone(),
                self.limits,
                geometry_census,
            )?;
            if payload_eq(self, &replayed)? {
                Ok(())
            } else {
                Err(GeneratedResidualAffinePivotTargetMatchingError::ReplayMismatch)
            }
        }))
        .map_err(|_| GeneratedResidualAffinePivotTargetMatchingError::SymbolicaPanic)?
    }
}

fn validate_schema_for_replay(
    schema: &str,
) -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
    if schema == GENERATED_RESIDUAL_AFFINE_PIVOT_TARGET_MATCHING_V1_SCHEMA {
        Ok(())
    } else {
        Err(GeneratedResidualAffinePivotTargetMatchingError::SchemaMismatch)
    }
}

pub struct GeneratedResidualAffinePivotTargetMatchingCompiler;

impl GeneratedResidualAffinePivotTargetMatchingCompiler {
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        inventory: Arc<GeneratedResidualAffineCaseInventoryCertificate>,
        source_case_ordinal: usize,
        reelimination: Arc<GeneratedResidualAffineBranchReeliminationCertificate>,
        limits: GeneratedResidualAffinePivotTargetMatchingLimits,
    ) -> Result<
        GeneratedResidualAffinePivotTargetMatchingCertificate,
        GeneratedResidualAffinePivotTargetMatchingError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            let geometry_census = validate_authorities(
                family,
                context,
                &inventory,
                source_case_ordinal,
                &reelimination,
                limits,
            )?;
            inventory.replay(family, context)?;
            reelimination.replay(family, context)?;
            compile_replayed(
                family,
                context,
                inventory,
                source_case_ordinal,
                reelimination,
                limits,
                geometry_census,
            )
        }))
        .map_err(|_| GeneratedResidualAffinePivotTargetMatchingError::SymbolicaPanic)?
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedResidualAffinePivotTargetMatchingError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    SourceCaseOutOfRange {
        ordinal: usize,
    },
    SourceCaseOrdinalMismatch {
        expected: usize,
        actual: usize,
    },
    SourceGroupOutOfRange {
        ordinal: usize,
    },
    SourceGroupOrdinalMismatch {
        expected: usize,
        actual: usize,
    },
    SourceGroupMembershipMismatch,
    SourceCoverAllocationMismatch,
    SourceBranchAllocationMismatch,
    SourceGuardAllocationMismatch,
    ScheduleBranchAllocationMismatch,
    MalformedGeometry {
        detail: &'static str,
    },
    PivotOrdinalMismatch {
        expected: usize,
        actual: usize,
    },
    PivotArityMismatch {
        expected: usize,
        actual: usize,
    },
    RetainedTargetOutsideSourceGroup {
        ordinal: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    ReplayMismatch,
    SymbolicaPanic,
    Inventory(GeneratedResidualAffineCaseInventoryError),
    Reelimination(GeneratedResidualAffineBranchReeliminationError),
    Relation(ParametricRelationError),
}

impl fmt::Display for GeneratedResidualAffinePivotTargetMatchingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for GeneratedResidualAffinePivotTargetMatchingError {}

impl From<GeneratedResidualAffineCaseInventoryError>
    for GeneratedResidualAffinePivotTargetMatchingError
{
    fn from(value: GeneratedResidualAffineCaseInventoryError) -> Self {
        Self::Inventory(value)
    }
}

impl From<GeneratedResidualAffineBranchReeliminationError>
    for GeneratedResidualAffinePivotTargetMatchingError
{
    fn from(value: GeneratedResidualAffineBranchReeliminationError) -> Self {
        Self::Reelimination(value)
    }
}

impl From<ParametricRelationError> for GeneratedResidualAffinePivotTargetMatchingError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

fn validate_authorities(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    inventory: &Arc<GeneratedResidualAffineCaseInventoryCertificate>,
    source_case_ordinal: usize,
    reelimination: &Arc<GeneratedResidualAffineBranchReeliminationCertificate>,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<(usize, usize), GeneratedResidualAffinePivotTargetMatchingError> {
    let family_bytes = family.fingerprint_ref().len();
    let inventory_family_bytes = inventory.family_fingerprint().len();
    let reelimination_family_bytes = reelimination.family_fingerprint().len();
    let context_bytes = context.fingerprint().len();
    let inventory_context_bytes = inventory.context_fingerprint().len();
    let reelimination_context_bytes = reelimination.context_fingerprint().len();
    for requested in [
        family_bytes,
        inventory_family_bytes,
        reelimination_family_bytes,
    ] {
        check_limit(
            "affine target family fingerprint bytes",
            requested,
            limits.max_family_fingerprint_bytes,
        )?;
    }
    for requested in [
        context_bytes,
        inventory_context_bytes,
        reelimination_context_bytes,
    ] {
        check_limit(
            "affine target context fingerprint bytes",
            requested,
            limits.max_context_fingerprint_bytes,
        )?;
    }
    let scope_bytes = [
        family_bytes,
        inventory_family_bytes,
        reelimination_family_bytes,
        context_bytes,
        inventory_context_bytes,
        reelimination_context_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        checked_add(
            "affine target scope fingerprint comparison bytes",
            sum,
            bytes,
        )
    })?;
    check_limit(
        "affine target scope fingerprint comparison bytes",
        scope_bytes,
        limits.max_scope_fingerprint_comparison_bytes,
    )?;
    if family.fingerprint_ref() != inventory.family_fingerprint()
        || family.fingerprint_ref() != reelimination.family_fingerprint()
    {
        return Err(GeneratedResidualAffinePivotTargetMatchingError::WrongFamily);
    }
    if context.fingerprint() != inventory.context_fingerprint()
        || context.fingerprint() != reelimination.context_fingerprint()
    {
        return Err(GeneratedResidualAffinePivotTargetMatchingError::WrongContext);
    }

    let source_case = inventory.cases().get(source_case_ordinal).ok_or(
        GeneratedResidualAffinePivotTargetMatchingError::SourceCaseOutOfRange {
            ordinal: source_case_ordinal,
        },
    )?;
    if source_case.ordinal() != source_case_ordinal {
        return Err(
            GeneratedResidualAffinePivotTargetMatchingError::SourceCaseOrdinalMismatch {
                expected: source_case_ordinal,
                actual: source_case.ordinal(),
            },
        );
    }
    let group = inventory.groups().get(source_case.group_ordinal()).ok_or(
        GeneratedResidualAffinePivotTargetMatchingError::SourceGroupOutOfRange {
            ordinal: source_case.group_ordinal(),
        },
    )?;
    if group.ordinal() != source_case.group_ordinal() {
        return Err(
            GeneratedResidualAffinePivotTargetMatchingError::SourceGroupOrdinalMismatch {
                expected: source_case.group_ordinal(),
                actual: group.ordinal(),
            },
        );
    }
    if group
        .case_ordinals()
        .get(source_case.ordinal_within_group())
        != Some(&source_case_ordinal)
    {
        return Err(GeneratedResidualAffinePivotTargetMatchingError::SourceGroupMembershipMismatch);
    }
    if !Arc::ptr_eq(source_case.source_branch(), reelimination.branch()) {
        return Err(
            GeneratedResidualAffinePivotTargetMatchingError::SourceBranchAllocationMismatch,
        );
    }
    if !Arc::ptr_eq(
        source_case.guard_composition(),
        reelimination.branch_guards(),
    ) {
        return Err(GeneratedResidualAffinePivotTargetMatchingError::SourceGuardAllocationMismatch);
    }
    if !Arc::ptr_eq(
        source_case.source_cover(),
        reelimination.branch().source_cover(),
    ) {
        return Err(GeneratedResidualAffinePivotTargetMatchingError::SourceCoverAllocationMismatch);
    }
    let schedule_branch = reelimination
        .schedule()
        .ordering()
        .residual_branch()
        .ok_or(GeneratedResidualAffinePivotTargetMatchingError::ScheduleBranchAllocationMismatch)?;
    if !Arc::ptr_eq(schedule_branch, reelimination.branch()) {
        return Err(
            GeneratedResidualAffinePivotTargetMatchingError::ScheduleBranchAllocationMismatch,
        );
    }

    check_limit(
        "affine target pivots",
        reelimination.pivot_count(),
        limits.max_pivots,
    )?;
    check_limit(
        "affine target ambient arity",
        group.ambient_arity(),
        limits.max_ambient_arity,
    )?;
    check_limit(
        "affine target free positions",
        group.free_positions().len(),
        limits.max_free_positions,
    )?;
    check_limit(
        "affine target group cases",
        group.case_ordinals().len(),
        limits.max_group_cases,
    )?;
    if group.ambient_arity() != context.index_count()
        || source_case.constants().len() != group.ambient_arity()
        || group.compact_linear_coefficients().len()
            != checked_mul(
                "affine target compact matrix entries",
                group.ambient_arity(),
                group.free_positions().len(),
            )?
        || source_case.affine_map().ambient_arity() != group.ambient_arity()
        || source_case.affine_map().free_positions() != group.free_positions()
    {
        return Err(
            GeneratedResidualAffinePivotTargetMatchingError::MalformedGeometry {
                detail: "source map, compact group, and K(n) arity disagree",
            },
        );
    }
    let geometry_census = validate_geometry_payload(source_case, group, limits)?;
    for &ordinal in group.case_ordinals() {
        let target = inventory.cases().get(ordinal).ok_or(
            GeneratedResidualAffinePivotTargetMatchingError::RetainedTargetOutsideSourceGroup {
                ordinal,
            },
        )?;
        if target.group_ordinal() != group.ordinal()
            || target.constants().len() != group.ambient_arity()
        {
            return Err(
                GeneratedResidualAffinePivotTargetMatchingError::RetainedTargetOutsideSourceGroup {
                    ordinal,
                },
            );
        }
    }
    Ok(geometry_census)
}

fn validate_geometry_payload(
    source_case: &crate::GeneratedResidualAffineInventoryCase,
    group: &crate::GeneratedResidualAffineContiguousCaseGroup,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<(usize, usize), GeneratedResidualAffinePivotTargetMatchingError> {
    let matrix_entries = checked_mul(
        "affine target geometry comparison entries",
        group.ambient_arity(),
        group.free_positions().len(),
    )?;
    let entries = checked_add(
        "affine target geometry comparison entries",
        group.ambient_arity(),
        matrix_entries,
    )?;
    check_limit(
        "affine target geometry comparison entries",
        entries,
        limits.max_geometry_comparison_entries,
    )?;
    let mut integer_bit_work = 0usize;
    let mut compare = |left: &Integer,
                       right: &Integer,
                       mismatch: &'static str|
     -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
        let work = checked_add(
            "affine target geometry comparison integer-bit work",
            integer_magnitude_bits(left)?.max(1),
            integer_magnitude_bits(right)?.max(1),
        )?;
        integer_bit_work = bounded_add(
            "affine target geometry comparison integer-bit work",
            integer_bit_work,
            work,
            limits.max_geometry_comparison_integer_bit_work,
        )?;
        if left != right {
            return Err(
                GeneratedResidualAffinePivotTargetMatchingError::MalformedGeometry {
                    detail: mismatch,
                },
            );
        }
        Ok(())
    };
    for row in 0..group.ambient_arity() {
        let map_constant = source_case.affine_map().constant(row).ok_or(
            GeneratedResidualAffinePivotTargetMatchingError::MalformedGeometry {
                detail: "source affine map omits a constant entry",
            },
        )?;
        let case_constant = source_case.constants().get(row).ok_or(
            GeneratedResidualAffinePivotTargetMatchingError::MalformedGeometry {
                detail: "source case omits a constant entry",
            },
        )?;
        compare(
            map_constant,
            case_constant,
            "source constant vector differs from authenticated affine map",
        )?;
        for (free_ordinal, &free_position) in group.free_positions().iter().enumerate() {
            if free_position >= group.ambient_arity() {
                return Err(
                    GeneratedResidualAffinePivotTargetMatchingError::MalformedGeometry {
                        detail: "free position lies outside ambient arity",
                    },
                );
            }
            let compact = group.compact_linear_coefficient(row, free_ordinal).ok_or(
                GeneratedResidualAffinePivotTargetMatchingError::MalformedGeometry {
                    detail: "compact group matrix omits an entry",
                },
            )?;
            let map = source_case
                .affine_map()
                .linear_coefficient(row, free_position)
                .ok_or(
                    GeneratedResidualAffinePivotTargetMatchingError::MalformedGeometry {
                        detail: "source affine map omits a linear entry",
                    },
                )?;
            compare(
                compact,
                map,
                "compact group matrix differs from source affine map",
            )?;
        }
    }
    Ok((entries, integer_bit_work))
}

fn compile_replayed(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    inventory: Arc<GeneratedResidualAffineCaseInventoryCertificate>,
    source_case_ordinal: usize,
    reelimination: Arc<GeneratedResidualAffineBranchReeliminationCertificate>,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
    geometry_census: (usize, usize),
) -> Result<
    GeneratedResidualAffinePivotTargetMatchingCertificate,
    GeneratedResidualAffinePivotTargetMatchingError,
> {
    let source_case = &inventory.cases()[source_case_ordinal];
    let source_group_ordinal = source_case.group_ordinal();
    let group = &inventory.groups()[source_group_ordinal];
    let family_fingerprint_bytes = family.fingerprint_ref().len();
    let context_fingerprint_bytes = context.fingerprint().len();
    let scope_fingerprint_comparison_bytes = [
        family_fingerprint_bytes,
        inventory.family_fingerprint().len(),
        reelimination.family_fingerprint().len(),
        context_fingerprint_bytes,
        inventory.context_fingerprint().len(),
        reelimination.context_fingerprint().len(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        checked_add(
            "affine target scope fingerprint comparison bytes",
            sum,
            bytes,
        )
    })?;
    let pivots = reelimination
        .elimination_for_affine_target_matching()
        .pivots();
    let (geometry_comparison_entries, geometry_comparison_integer_bit_work) = geometry_census;
    let retained_outcome_buffer_bytes = checked_mul(
        "affine target retained payload bytes",
        pivots.len(),
        size_of::<GeneratedResidualAffinePivotTargetOutcome>(),
    )?;
    let retained_payload_base_bytes = checked_add(
        "affine target retained payload bytes",
        size_of::<GeneratedResidualAffinePivotTargetMatchingCertificate>(),
        retained_outcome_buffer_bytes,
    )?;
    let mut stats = GeneratedResidualAffinePivotTargetMatchingStats {
        family_fingerprint_bytes,
        context_fingerprint_bytes,
        scope_fingerprint_comparison_bytes,
        pivots: pivots.len(),
        ambient_arity: group.ambient_arity(),
        free_positions: group.free_positions().len(),
        group_cases: group.case_ordinals().len(),
        geometry_comparison_entries,
        geometry_comparison_integer_bit_work,
        // Commit the complete fixed outcomes buffer to the exact census before
        // reserving it. Dynamic per-outcome payload is admitted separately.
        retained_payload_bytes: retained_payload_base_bytes,
        ..Default::default()
    };
    check_limit(
        "affine target retained payload bytes",
        stats.retained_payload_bytes,
        limits.max_retained_payload_bytes,
    )?;
    let mut retained_payload_admission = RetainedPayloadAdmission {
        bytes: retained_payload_base_bytes,
        limit: limits.max_retained_payload_bytes,
    };
    let mut outcomes = Vec::new();
    try_reserve_exact("affine target outcomes", &mut outcomes, pivots.len())?;

    for (expected_pivot_ordinal, pivot_equation) in pivots.iter().enumerate() {
        if pivot_equation.ordinal() != expected_pivot_ordinal {
            return Err(
                GeneratedResidualAffinePivotTargetMatchingError::PivotOrdinalMismatch {
                    expected: expected_pivot_ordinal,
                    actual: pivot_equation.ordinal(),
                },
            );
        }
        let pivot = pivot_equation.pivot();
        if pivot.arity() != group.ambient_arity() {
            return Err(
                GeneratedResidualAffinePivotTargetMatchingError::PivotArityMismatch {
                    expected: group.ambient_arity(),
                    actual: pivot.arity(),
                },
            );
        }
        let transformed = transformed_target_constants(
            source_case.constants(),
            group.compact_linear_coefficients(),
            group.free_positions(),
            pivot,
            &mut stats,
            &mut retained_payload_admission,
            limits,
        )?;
        check_limit(
            "affine target matching flags",
            group.case_ordinals().len(),
            limits.max_matching_flag_bytes,
        )?;
        stats.maximum_matching_flag_bytes = stats
            .maximum_matching_flag_bytes
            .max(group.case_ordinals().len());
        let mut matching_flags = Vec::new();
        try_reserve_exact(
            "affine target matching flags",
            &mut matching_flags,
            group.case_ordinals().len(),
        )?;
        matching_flags.resize(group.case_ordinals().len(), 0_u8);
        let mut matching_count = 0usize;
        let mut checked_count = 0usize;
        for (target_position, &target_ordinal) in group.case_ordinals().iter().enumerate() {
            stats.target_checks = bounded_add(
                "affine target checks",
                stats.target_checks,
                1,
                limits.max_target_checks,
            )?;
            checked_count = checked_add("affine target checked target ordinals", checked_count, 1)?;
            let target = &inventory.cases()[target_ordinal];
            stats.target_comparison_entries = bounded_add(
                "affine target comparison entries",
                stats.target_comparison_entries,
                group.ambient_arity(),
                limits.max_target_comparison_entries,
            )?;
            for (left, right) in transformed.iter().zip(target.constants()) {
                stats.target_comparison_integer_bit_work = bounded_add(
                    "affine target comparison integer-bit work",
                    stats.target_comparison_integer_bit_work,
                    checked_add(
                        "affine target comparison integer-bit work",
                        integer_magnitude_bits(left)?.max(1),
                        integer_magnitude_bits(right)?.max(1),
                    )?,
                    limits.max_target_comparison_integer_bit_work,
                )?;
            }
            if transformed == target.constants() {
                matching_flags[target_position] = 1;
                matching_count =
                    checked_add("affine target matching target ordinals", matching_count, 1)?;
            }
        }
        stats.checked_target_ordinals = bounded_add(
            "affine target checked target ordinals",
            stats.checked_target_ordinals,
            checked_count,
            limits.max_checked_target_ordinals,
        )?;
        retained_payload_admission.admit(checked_mul(
            "affine target retained payload bytes",
            checked_count,
            size_of::<usize>(),
        )?)?;
        let mut checked_targets = Vec::new();
        try_reserve_exact(
            "affine target checked target ordinals",
            &mut checked_targets,
            checked_count,
        )?;
        checked_targets.extend_from_slice(&group.case_ordinals()[..checked_count]);
        stats.matching_target_ordinals = bounded_add(
            "affine target matching target ordinals",
            stats.matching_target_ordinals,
            matching_count,
            limits.max_matching_target_ordinals,
        )?;
        retained_payload_admission.admit(checked_mul(
            "affine target retained payload bytes",
            matching_count,
            size_of::<usize>(),
        )?)?;
        let mut matching_targets = Vec::new();
        try_reserve_exact(
            "affine target matching target ordinals",
            &mut matching_targets,
            matching_count,
        )?;
        for (&target_ordinal, &matched) in group.case_ordinals().iter().zip(&matching_flags) {
            if matched != 0 {
                matching_targets.push(target_ordinal);
            }
        }
        if matching_targets.len() != matching_count {
            return Err(GeneratedResidualAffinePivotTargetMatchingError::ReplayMismatch);
        }

        stats.retained_shift_components = bounded_add(
            "affine target retained shift components",
            stats.retained_shift_components,
            pivot.arity(),
            limits.max_retained_shift_components,
        )?;
        retained_payload_admission.admit(checked_mul(
            "affine target retained payload bytes",
            pivot.arity(),
            size_of::<i64>(),
        )?)?;
        let pivot_copy = copy_shift(pivot)?;
        if matching_targets.is_empty() {
            stats.rejected_no_target_cases = checked_add(
                "affine target no-target rejections",
                stats.rejected_no_target_cases,
                1,
            )?;
            add_outcome_payload_bytes(
                &mut stats,
                &transformed,
                group.ambient_arity(),
                checked_targets.len(),
                0,
                0,
                limits,
            )?;
            outcomes.push(
                GeneratedResidualAffinePivotTargetOutcome::RejectedNoTargetCase(
                    GeneratedResidualAffineRejectedNoTargetCase {
                        pivot_ordinal: pivot_equation.ordinal(),
                        pivot: pivot_copy,
                        transformed_target_constants: transformed,
                        checked_target_case_ordinals: checked_targets,
                    },
                ),
            );
            continue;
        }

        let additional_pending_shift_components = match preflight_recentering_disposition(
            &mut stats,
            pivot_equation.unit_relation(),
            group.free_positions(),
            pivot,
            group.ambient_arity(),
            limits,
        )? {
            RecenteringDisposition::Boundary { position, kind } => {
                stats.rejected_recentering_boundaries = checked_add(
                    "affine target recentering-boundary rejections",
                    stats.rejected_recentering_boundaries,
                    1,
                )?;
                add_outcome_payload_bytes(
                    &mut stats,
                    &transformed,
                    group.ambient_arity(),
                    checked_targets.len(),
                    matching_targets.len(),
                    0,
                    limits,
                )?;
                outcomes.push(
                    GeneratedResidualAffinePivotTargetOutcome::RejectedRecenteringBoundary(
                        GeneratedResidualAffineRejectedRecenteringBoundary {
                            pivot_ordinal: pivot_equation.ordinal(),
                            pivot: pivot_copy,
                            transformed_target_constants: transformed,
                            checked_target_case_ordinals: checked_targets,
                            matching_target_case_ordinals: matching_targets,
                            kind,
                            position,
                        },
                    ),
                );
                continue;
            }
            RecenteringDisposition::Pending {
                additional_shift_components,
            } => additional_shift_components,
        };
        // `preflight_recentering_disposition` checks this pending-only shift
        // growth only after both typed boundary exits. Admit the corresponding
        // retained bytes immediately after that successful check.
        retained_payload_admission.admit(checked_mul(
            "affine target retained payload bytes",
            additional_pending_shift_components,
            size_of::<i64>(),
        )?)?;
        let coefficient_translation = coefficient_translation(group.free_positions(), pivot)?;
        let pending_row_label_bytes =
            pending_row_label_byte_len(source_case_ordinal, pivot_equation.ordinal())?;
        check_limit(
            "affine target row label bytes",
            pending_row_label_bytes,
            limits.max_row_label_bytes,
        )?;
        let pending_external_relation_bytes = pending_external_relation_allocation_byte_bound(
            context.fingerprint().len(),
            pending_row_label_bytes,
        )?;
        retained_payload_admission.admit(pending_external_relation_bytes)?;
        let (row_id, _) = pending_row_id(
            source_case_ordinal,
            pivot_equation.ordinal(),
            pending_row_label_bytes,
            &mut stats,
            limits,
        )?;
        let outer_relation_byte_budget = retained_payload_admission.remaining();
        let mut helper_limits = remaining_recentering_limits(stats, limits)?;
        helper_limits.max_retained_bytes = helper_limits
            .max_retained_bytes
            .min(outer_relation_byte_budget);
        let recentered = pivot_equation.unit_relation().affine_free_recentered(
            context,
            &coefficient_translation,
            pivot,
            row_id,
            helper_limits,
        );
        let (relation, recentering_stats) = match recentered {
            Ok(value) => value,
            Err(ParametricRelationError::IndexOverflow { .. }) => {
                return Err(GeneratedResidualAffinePivotTargetMatchingError::ReplayMismatch);
            }
            Err(error) => return Err(error.into()),
        };
        // The relation helper prospectively admitted this complete envelope
        // before Symbolica execution. Commit it to the staged aggregate before
        // moving the relation into its retained Arc.
        retained_payload_admission.admit(recentering_stats.retained_bytes())?;
        stats.retained_shift_components = bounded_add(
            "affine target retained shift components",
            stats.retained_shift_components,
            additional_pending_shift_components,
            limits.max_retained_shift_components,
        )?;
        accumulate_recentering_stats(&mut stats, recentering_stats, limits)?;
        stats.pending_when_bad = checked_add(
            "affine target pending WhenBad candidates",
            stats.pending_when_bad,
            1,
        )?;
        let pending_relation_bytes = pending_relation_retained_payload_byte_bound(
            &relation,
            recentering_stats,
            pending_external_relation_bytes,
        )?;
        add_outcome_payload_bytes(
            &mut stats,
            &transformed,
            checked_mul(
                "affine target pending shift components",
                group.ambient_arity(),
                3,
            )?,
            checked_targets.len(),
            matching_targets.len(),
            pending_relation_bytes,
            limits,
        )?;
        outcomes.push(
            GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(
                GeneratedResidualAffinePendingWhenBad {
                    pivot_ordinal: pivot_equation.ordinal(),
                    pivot: pivot_copy,
                    transformed_target_constants: transformed,
                    checked_target_case_ordinals: checked_targets,
                    matching_target_case_ordinals: matching_targets,
                    coefficient_translation,
                    key_center: copy_shift(pivot)?,
                    relation: Arc::new(relation),
                    recentering_stats,
                },
            ),
        );
    }
    if stats.targets_consumed != 0
        || outcomes.len() != stats.pivots
        || retained_payload_admission.bytes != stats.retained_payload_bytes
    {
        return Err(GeneratedResidualAffinePivotTargetMatchingError::ReplayMismatch);
    }
    let payload_census = payload_census_same(&outcomes, limits)?;
    stats.payload_comparison_units = payload_census.units;
    stats.payload_comparison_bytes = payload_census.bytes;
    stats.payload_comparison_integer_bits = payload_census.integer_bits;
    stats.payload_comparison_relation_manifest_bytes = payload_census.relation_manifest_bytes;
    Ok(GeneratedResidualAffinePivotTargetMatchingCertificate {
        schema: GENERATED_RESIDUAL_AFFINE_PIVOT_TARGET_MATCHING_V1_SCHEMA,
        inventory,
        source_case_ordinal,
        source_group_ordinal,
        reelimination,
        outcomes,
        limits,
        stats,
    })
}

fn transformed_target_constants(
    source_constants: &[Integer],
    compact_linear_coefficients: &[Integer],
    free_positions: &[usize],
    pivot: &IndexShift,
    stats: &mut GeneratedResidualAffinePivotTargetMatchingStats,
    retained_payload_admission: &mut RetainedPayloadAdmission,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<Vec<Integer>, GeneratedResidualAffinePivotTargetMatchingError> {
    let ambient_arity = source_constants.len();
    let operations = checked_mul(
        "affine target affine operations",
        ambient_arity,
        checked_add(
            "affine target affine operations",
            checked_mul("affine target affine operations", free_positions.len(), 2)?,
            1,
        )?,
    )?;
    stats.affine_operations = bounded_add(
        "affine target affine operations",
        stats.affine_operations,
        operations,
        limits.max_affine_operations,
    )?;
    stats.transformed_constant_entries = bounded_add(
        "affine target transformed constant entries",
        stats.transformed_constant_entries,
        ambient_arity,
        limits.max_transformed_constant_entries,
    )?;
    retained_payload_admission.admit(checked_mul(
        "affine target retained payload bytes",
        ambient_arity,
        size_of::<Integer>(),
    )?)?;
    let mut transformed = Vec::new();
    try_reserve_exact(
        "affine target transformed constants",
        &mut transformed,
        ambient_arity,
    )?;
    for row in 0..ambient_arity {
        let source_bits = integer_magnitude_bits(&source_constants[row])?;
        observe_affine_integer_bits(stats, source_bits, limits)?;
        charge_affine_integer_work(stats, source_bits.max(1), limits)?;
        let mut value = source_constants[row].clone();
        for (free_ordinal, &free_position) in free_positions.iter().enumerate() {
            let coefficient =
                &compact_linear_coefficients[row * free_positions.len() + free_ordinal];
            let coordinate = pivot.values()[free_position];
            let product = checked_mul_integer_i64(coefficient, coordinate, stats, limits)?;
            value = checked_sub_integer(value, product, stats, limits)?;
        }
        value = checked_add_integer_i64(value, pivot.values()[row], stats, limits)?;
        stats.retained_integer_bits = bounded_add(
            "affine target retained integer bits",
            stats.retained_integer_bits,
            integer_magnitude_bits(&value)?.max(1),
            limits.max_retained_integer_bits,
        )?;
        // Integer arithmetic necessarily creates a transient result before
        // its exact GMP spare capacity is observable. Admit that exact heap
        // allocation here before the value is pushed into the retained Vec.
        retained_payload_admission.admit(integer_owned_heap_byte_bound(&value)?)?;
        transformed.push(value);
    }
    Ok(transformed)
}

fn checked_mul_integer_i64(
    left: &Integer,
    right: i64,
    stats: &mut GeneratedResidualAffinePivotTargetMatchingStats,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<Integer, GeneratedResidualAffinePivotTargetMatchingError> {
    let left_bits = integer_magnitude_bits(left)?;
    let right_bits = i64_magnitude_bits(right);
    let output_bound = if left_bits == 0 || right_bits == 0 {
        0
    } else {
        checked_add(
            "affine target multiplication integer bits",
            left_bits,
            right_bits,
        )?
    };
    observe_affine_integer_bits(stats, output_bound, limits)?;
    charge_affine_integer_work(
        stats,
        checked_add(
            "affine target affine integer-bit work",
            checked_add(
                "affine target affine integer-bit work",
                left_bits.max(1),
                right_bits.max(1),
            )?,
            output_bound.max(1),
        )?,
        limits,
    )?;
    let result = left * Integer::from(right);
    observe_affine_integer_bits(stats, integer_magnitude_bits(&result)?, limits)?;
    Ok(result)
}

fn checked_sub_integer(
    left: Integer,
    right: Integer,
    stats: &mut GeneratedResidualAffinePivotTargetMatchingStats,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<Integer, GeneratedResidualAffinePivotTargetMatchingError> {
    let left_bits = integer_magnitude_bits(&left)?;
    let right_bits = integer_magnitude_bits(&right)?;
    let output_bound = addition_bit_bound(left_bits, right_bits)?;
    observe_affine_integer_bits(stats, output_bound, limits)?;
    charge_affine_integer_work(
        stats,
        checked_add(
            "affine target affine integer-bit work",
            checked_add(
                "affine target affine integer-bit work",
                left_bits.max(1),
                right_bits.max(1),
            )?,
            output_bound.max(1),
        )?,
        limits,
    )?;
    let result = left - right;
    observe_affine_integer_bits(stats, integer_magnitude_bits(&result)?, limits)?;
    Ok(result)
}

fn checked_add_integer_i64(
    left: Integer,
    right: i64,
    stats: &mut GeneratedResidualAffinePivotTargetMatchingStats,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<Integer, GeneratedResidualAffinePivotTargetMatchingError> {
    let left_bits = integer_magnitude_bits(&left)?;
    let right_bits = i64_magnitude_bits(right);
    let output_bound = addition_bit_bound(left_bits, right_bits)?;
    observe_affine_integer_bits(stats, output_bound, limits)?;
    charge_affine_integer_work(
        stats,
        checked_add(
            "affine target affine integer-bit work",
            checked_add(
                "affine target affine integer-bit work",
                left_bits.max(1),
                right_bits.max(1),
            )?,
            output_bound.max(1),
        )?,
        limits,
    )?;
    let result = left + Integer::from(right);
    observe_affine_integer_bits(stats, integer_magnitude_bits(&result)?, limits)?;
    Ok(result)
}

fn addition_bit_bound(
    left_bits: usize,
    right_bits: usize,
) -> Result<usize, GeneratedResidualAffinePivotTargetMatchingError> {
    if left_bits == 0 {
        Ok(right_bits)
    } else if right_bits == 0 {
        Ok(left_bits)
    } else {
        checked_add(
            "affine target addition integer bits",
            left_bits.max(right_bits),
            1,
        )
    }
}

fn charge_affine_integer_work(
    stats: &mut GeneratedResidualAffinePivotTargetMatchingStats,
    work: usize,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
    stats.affine_integer_bit_work = bounded_add(
        "affine target affine integer-bit work",
        stats.affine_integer_bit_work,
        work,
        limits.max_affine_integer_bit_work,
    )?;
    Ok(())
}

fn coefficient_translation_boundary(
    free_positions: &[usize],
    pivot: &IndexShift,
) -> Option<(usize, GeneratedResidualAffineRecenteringBoundaryKind)> {
    free_positions.iter().copied().find_map(|position| {
        pivot.values()[position].checked_neg().is_none().then_some((
            position,
            GeneratedResidualAffineRecenteringBoundaryKind::FreeCoefficientTranslationNegation,
        ))
    })
}

fn first_integral_key_subtraction_overflow(
    relation: &ParametricRelation,
    key_center: &IndexShift,
) -> Option<usize> {
    relation.terms().keys().find_map(|shift| {
        shift
            .values()
            .iter()
            .zip(key_center.values())
            .enumerate()
            .find_map(|(position, (&value, &center))| {
                value.checked_sub(center).is_none().then_some(position)
            })
    })
}

fn charge_key_subtraction_boundary_checks(
    stats: &mut GeneratedResidualAffinePivotTargetMatchingStats,
    relation: &ParametricRelation,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
    let checks = checked_mul(
        "affine target recentered key-subtraction boundary checks",
        relation.terms().len(),
        relation.arity(),
    )?;
    stats.recenter_key_subtraction_boundary_checks = bounded_add(
        "affine target recentered key-subtraction boundary checks",
        stats.recenter_key_subtraction_boundary_checks,
        checks,
        limits.max_recenter_key_subtraction_boundary_checks,
    )?;
    Ok(())
}

fn classify_recentering_boundary(
    stats: &mut GeneratedResidualAffinePivotTargetMatchingStats,
    relation: &ParametricRelation,
    free_positions: &[usize],
    pivot: &IndexShift,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<
    Option<(usize, GeneratedResidualAffineRecenteringBoundaryKind)>,
    GeneratedResidualAffinePivotTargetMatchingError,
> {
    if let Some(boundary) = coefficient_translation_boundary(free_positions, pivot) {
        return Ok(Some(boundary));
    }
    stats.recenter_attempts = bounded_add(
        "affine target recenter attempts",
        stats.recenter_attempts,
        1,
        limits.max_recenter_attempts,
    )?;
    charge_key_subtraction_boundary_checks(stats, relation, limits)?;
    Ok(
        first_integral_key_subtraction_overflow(relation, pivot).map(|position| {
            (
                position,
                GeneratedResidualAffineRecenteringBoundaryKind::IntegralKeySubtraction,
            )
        }),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecenteringDisposition {
    Boundary {
        position: usize,
        kind: GeneratedResidualAffineRecenteringBoundaryKind,
    },
    Pending {
        additional_shift_components: usize,
    },
}

fn preflight_recentering_disposition(
    stats: &mut GeneratedResidualAffinePivotTargetMatchingStats,
    relation: &ParametricRelation,
    free_positions: &[usize],
    pivot: &IndexShift,
    ambient_arity: usize,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<RecenteringDisposition, GeneratedResidualAffinePivotTargetMatchingError> {
    if let Some((position, kind)) =
        classify_recentering_boundary(stats, relation, free_positions, pivot, limits)?
    {
        return Ok(RecenteringDisposition::Boundary { position, kind });
    }
    let additional_shift_components =
        checked_mul("affine target retained shift components", ambient_arity, 2)?;
    check_limit(
        "affine target retained shift components",
        checked_add(
            "affine target retained shift components",
            stats.retained_shift_components,
            additional_shift_components,
        )?,
        limits.max_retained_shift_components,
    )?;
    Ok(RecenteringDisposition::Pending {
        additional_shift_components,
    })
}

fn coefficient_translation(
    free_positions: &[usize],
    pivot: &IndexShift,
) -> Result<IndexShift, GeneratedResidualAffinePivotTargetMatchingError> {
    let mut values = Vec::new();
    try_reserve_exact(
        "affine target coefficient translation components",
        &mut values,
        pivot.arity(),
    )?;
    values.resize(pivot.arity(), 0);
    for &position in free_positions {
        // `coefficient_translation_boundary` has already authenticated every
        // negation before retained pending payload is admitted.
        let value = pivot.values()[position]
            .checked_neg()
            .ok_or(GeneratedResidualAffinePivotTargetMatchingError::ReplayMismatch)?;
        values[position] = value;
    }
    Ok(IndexShift::try_from_preallocated(values, pivot.arity())?)
}

fn remaining_recentering_limits(
    stats: GeneratedResidualAffinePivotTargetMatchingStats,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<ParametricAffineFreeRecenteringLimits, GeneratedResidualAffinePivotTargetMatchingError>
{
    Ok(ParametricAffineFreeRecenteringLimits {
        arithmetic: limits.arithmetic,
        max_terms: limits
            .max_recenter_terms
            .checked_sub(stats.recenter_terms)
            .ok_or(
                GeneratedResidualAffinePivotTargetMatchingError::ResourceLimit {
                    resource: "affine target recentered terms",
                    requested: stats.recenter_terms,
                    limit: limits.max_recenter_terms,
                },
            )?,
        max_guards: limits
            .max_recenter_guards
            .checked_sub(stats.recenter_guards)
            .ok_or(
                GeneratedResidualAffinePivotTargetMatchingError::ResourceLimit {
                    resource: "affine target recentered guards",
                    requested: stats.recenter_guards,
                    limit: limits.max_recenter_guards,
                },
            )?,
        max_translation_components: limits
            .max_recenter_translation_components
            .checked_sub(stats.recenter_translation_components)
            .ok_or(
                GeneratedResidualAffinePivotTargetMatchingError::ResourceLimit {
                    resource: "affine target recentered translation components",
                    requested: stats.recenter_translation_components,
                    limit: limits.max_recenter_translation_components,
                },
            )?,
        max_key_subtraction_boundary_checks: remaining(
            "affine target recentered key-subtraction boundary checks",
            limits.max_recenter_key_subtraction_boundary_checks,
            stats.recenter_key_subtraction_boundary_checks,
        )?,
        max_source_terms: remaining(
            "affine target recentered source terms",
            limits.max_recenter_source_terms,
            stats.recenter_source_terms,
        )?,
        max_source_exponent_entries: remaining(
            "affine target recentered source exponent entries",
            limits.max_recenter_source_exponent_entries,
            stats.recenter_source_exponent_entries,
        )?,
        max_output_terms: remaining(
            "affine target recentered output terms",
            limits.max_recenter_output_terms,
            stats.recenter_output_terms,
        )?,
        max_output_exponent_entries: remaining(
            "affine target recentered output exponent entries",
            limits.max_recenter_output_exponent_entries,
            stats.recenter_output_exponent_entries,
        )?,
        max_power_operations: remaining(
            "affine target recentered power operations",
            limits.max_recenter_power_operations,
            stats.recenter_power_operations,
        )?,
        max_integer_bit_work: remaining(
            "affine target recentered integer-bit work",
            limits.max_recenter_integer_bit_work,
            stats.recenter_integer_bit_work,
        )?,
        max_normalized_coefficient_terms: remaining(
            "affine target recentered normalized coefficient terms",
            limits.max_recenter_normalized_coefficient_terms,
            stats.recenter_normalized_coefficient_terms,
        )?,
        max_retained_bytes: remaining(
            "affine target recentered retained bytes",
            limits.max_recenter_retained_bytes,
            stats.recenter_retained_bytes,
        )?,
    })
}

fn accumulate_recentering_stats(
    target: &mut GeneratedResidualAffinePivotTargetMatchingStats,
    source: ParametricAffineFreeRecenteringStats,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
    macro_rules! accumulate {
        ($field:ident, $getter:ident, $resource:literal, $limit:expr) => {
            target.$field = bounded_add($resource, target.$field, source.$getter(), $limit)?;
        };
    }
    accumulate!(
        recenter_terms,
        terms,
        "affine target recentered terms",
        limits.max_recenter_terms
    );
    accumulate!(
        recenter_guards,
        guards,
        "affine target recentered guards",
        limits.max_recenter_guards
    );
    accumulate!(
        recenter_translation_components,
        translation_components,
        "affine target recentered translation components",
        limits.max_recenter_translation_components
    );
    accumulate!(
        recenter_key_subtraction_boundary_checks,
        key_subtraction_boundary_checks,
        "affine target recentered key-subtraction boundary checks",
        limits.max_recenter_key_subtraction_boundary_checks
    );
    accumulate!(
        recenter_source_terms,
        source_terms,
        "affine target recentered source terms",
        limits.max_recenter_source_terms
    );
    accumulate!(
        recenter_source_exponent_entries,
        source_exponent_entries,
        "affine target recentered source exponent entries",
        limits.max_recenter_source_exponent_entries
    );
    accumulate!(
        recenter_output_terms,
        output_terms,
        "affine target recentered output terms",
        limits.max_recenter_output_terms
    );
    accumulate!(
        recenter_output_exponent_entries,
        output_exponent_entries,
        "affine target recentered output exponent entries",
        limits.max_recenter_output_exponent_entries
    );
    accumulate!(
        recenter_power_operations,
        power_operations,
        "affine target recentered power operations",
        limits.max_recenter_power_operations
    );
    accumulate!(
        recenter_integer_bit_work,
        integer_bit_work,
        "affine target recentered integer-bit work",
        limits.max_recenter_integer_bit_work
    );
    accumulate!(
        recenter_normalized_coefficient_terms,
        normalized_coefficient_terms,
        "affine target recentered normalized coefficient terms",
        limits.max_recenter_normalized_coefficient_terms
    );
    accumulate!(
        recenter_retained_bytes,
        retained_bytes,
        "affine target recentered retained bytes",
        limits.max_recenter_retained_bytes
    );
    Ok(())
}

fn pending_row_id(
    source_case_ordinal: usize,
    pivot_ordinal: usize,
    bytes: usize,
    stats: &mut GeneratedResidualAffinePivotTargetMatchingStats,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<(ParametricRowId, usize), GeneratedResidualAffinePivotTargetMatchingError> {
    const PREFIX: &str = "generated-residual-affine-pending-when-bad:";
    if bytes != pending_row_label_byte_len(source_case_ordinal, pivot_ordinal)? {
        return Err(GeneratedResidualAffinePivotTargetMatchingError::ReplayMismatch);
    }
    check_limit(
        "affine target row label bytes",
        bytes,
        limits.max_row_label_bytes,
    )?;
    stats.row_label_bytes = bounded_add(
        "affine target row label bytes",
        stats.row_label_bytes,
        bytes,
        limits.max_row_label_bytes,
    )?;
    let mut label = String::new();
    label.try_reserve_exact(bytes).map_err(|_| {
        GeneratedResidualAffinePivotTargetMatchingError::AllocationFailure {
            resource: "affine target row label bytes",
            requested: bytes,
        }
    })?;
    write!(&mut label, "{PREFIX}{source_case_ordinal}:{pivot_ordinal}")
        .map_err(|_| GeneratedResidualAffinePivotTargetMatchingError::ReplayMismatch)?;
    if label.len() != bytes {
        return Err(GeneratedResidualAffinePivotTargetMatchingError::ReplayMismatch);
    }
    Ok((
        ParametricRowId::Derived {
            label: Arc::from(label),
        },
        bytes,
    ))
}

fn pending_row_label_byte_len(
    source_case_ordinal: usize,
    pivot_ordinal: usize,
) -> Result<usize, GeneratedResidualAffinePivotTargetMatchingError> {
    const PREFIX: &str = "generated-residual-affine-pending-when-bad:";
    let digits = decimal_digits(source_case_ordinal)
        .checked_add(decimal_digits(pivot_ordinal))
        .and_then(|digits| digits.checked_add(1))
        .ok_or(
            GeneratedResidualAffinePivotTargetMatchingError::ResourceCountOverflow {
                resource: "affine target row label bytes",
            },
        )?;
    checked_add("affine target row label bytes", PREFIX.len(), digits)
}

/// Independent prospective admission ledger. Exact stats are committed only
/// when an outcome is finalized; this ledger moves ahead of every allocation
/// that is guaranteed to survive in that outcome and must equal the final
/// census before publication.
struct RetainedPayloadAdmission {
    bytes: usize,
    limit: usize,
}

impl RetainedPayloadAdmission {
    fn admit(
        &mut self,
        additional: usize,
    ) -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
        self.bytes = bounded_add(
            "affine target retained payload bytes",
            self.bytes,
            additional,
            self.limit,
        )?;
        Ok(())
    }

    fn remaining(&self) -> usize {
        self.limit - self.bytes
    }
}

fn add_outcome_payload_bytes(
    stats: &mut GeneratedResidualAffinePivotTargetMatchingStats,
    transformed_constants: &[Integer],
    shift_components: usize,
    checked_targets: usize,
    matching_targets: usize,
    relation_bytes: usize,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
    let bytes = outcome_payload_byte_bound(
        transformed_constants,
        shift_components,
        checked_targets,
        matching_targets,
        relation_bytes,
    )?;
    stats.retained_payload_bytes = bounded_add(
        "affine target retained payload bytes",
        stats.retained_payload_bytes,
        bytes,
        limits.max_retained_payload_bytes,
    )?;
    Ok(())
}

fn outcome_payload_byte_bound(
    transformed_constants: &[Integer],
    shift_components: usize,
    checked_targets: usize,
    matching_targets: usize,
    relation_bytes: usize,
) -> Result<usize, GeneratedResidualAffinePivotTargetMatchingError> {
    let mut integer_heap_bytes = 0usize;
    for value in transformed_constants {
        integer_heap_bytes = checked_add(
            "affine target retained payload bytes",
            integer_heap_bytes,
            integer_owned_heap_byte_bound(value)?,
        )?;
    }
    let integer_storage_bytes = checked_add(
        "affine target retained payload bytes",
        checked_mul(
            "affine target retained payload bytes",
            transformed_constants.len(),
            size_of::<Integer>(),
        )?,
        integer_heap_bytes,
    )?;
    checked_add(
        "affine target retained payload bytes",
        checked_mul(
            "affine target retained payload bytes",
            shift_components,
            size_of::<i64>(),
        )?,
        checked_add(
            "affine target retained payload bytes",
            checked_mul(
                "affine target retained payload bytes",
                checked_add(
                    "affine target retained payload bytes",
                    checked_targets,
                    matching_targets,
                )?,
                size_of::<usize>(),
            )?,
            checked_add(
                "affine target retained payload bytes",
                integer_storage_bytes,
                relation_bytes,
            )?,
        )?,
    )
}

/// Complete retained allocation charged for the private relation behind one
/// pending outcome. The outcome enum already contains the `Arc` pointer, so
/// this deliberately adds only the pointee allocation and both control words.
/// The separately created derived-row `Arc<str>` is charged once as its byte
/// payload plus its own control block.
fn pending_relation_retained_payload_byte_bound(
    relation: &ParametricRelation,
    recentering_stats: ParametricAffineFreeRecenteringStats,
    external_allocation_bytes: usize,
) -> Result<usize, GeneratedResidualAffinePivotTargetMatchingError> {
    let owned_relation_bytes = relation.owned_retained_byte_bound().ok_or(
        GeneratedResidualAffinePivotTargetMatchingError::ResourceCountOverflow {
            resource: "affine target pending retained relation bytes",
        },
    )?;
    // The helper calls this a conservative pre-Symbolica envelope. Verify the
    // observed complete owned shape instead of silently relying on that
    // promise; this also keeps the outer prospective census exact on replay.
    check_limit(
        "affine target pending owned relation bytes",
        owned_relation_bytes,
        recentering_stats.retained_bytes(),
    )?;
    checked_add(
        "affine target pending retained relation bytes",
        recentering_stats.retained_bytes(),
        external_allocation_bytes,
    )
}

/// Allocations shared by fields inside a pending relation but not owned by
/// `ParametricRelation::owned_retained_byte_bound`: the relation's Arc header,
/// the derived-row `Arc<str>`, and the fresh context-fingerprint `Arc<str>`
/// created by `ParametricRelation::new`. The family fingerprint is cloned
/// from the source relation and therefore has no new allocation here.
fn pending_external_relation_allocation_byte_bound(
    context_fingerprint_bytes: usize,
    row_label_bytes: usize,
) -> Result<usize, GeneratedResidualAffinePivotTargetMatchingError> {
    checked_add(
        "affine target pending retained relation bytes",
        arc_sized_control_and_padding_byte_bound::<ParametricRelation>()?,
        checked_add(
            "affine target pending retained relation bytes",
            checked_add(
                "affine target pending retained relation bytes",
                row_label_bytes,
                arc_slice_control_and_padding_byte_bound::<u8>()?,
            )?,
            checked_add(
                "affine target pending retained relation bytes",
                context_fingerprint_bytes,
                arc_slice_control_and_padding_byte_bound::<u8>()?,
            )?,
        )?,
    )
}

/// `ArcInner<T>` owns two atomic counters before `T`. The padding bound is
/// conservative and keeps this independent of `std`'s private Arc layout.
fn arc_sized_control_and_padding_byte_bound<T>()
-> Result<usize, GeneratedResidualAffinePivotTargetMatchingError> {
    checked_add(
        "affine target pending retained relation bytes",
        checked_mul(
            "affine target pending retained relation bytes",
            2,
            size_of::<usize>(),
        )?,
        align_of::<T>().saturating_sub(1),
    )
}

/// Control/padding bound for an `Arc<[T]>` allocation. `Arc<str>` has the
/// same allocation shape as `Arc<[u8]>`; its fat pointer is already inline in
/// the retained relation and is therefore not counted here.
fn arc_slice_control_and_padding_byte_bound<T>()
-> Result<usize, GeneratedResidualAffinePivotTargetMatchingError> {
    checked_add(
        "affine target pending retained relation bytes",
        checked_mul(
            "affine target pending retained relation bytes",
            2,
            size_of::<usize>(),
        )?,
        align_of::<T>().saturating_sub(1),
    )
}

/// The `Integer` enum itself lives in the transformed-constants Vec buffer.
/// Only a GMP-backed `Large` value owns a second allocation; charging limbs
/// for `Single`/`Double` would double-count their inline representation.
fn integer_owned_heap_byte_bound(
    value: &Integer,
) -> Result<usize, GeneratedResidualAffinePivotTargetMatchingError> {
    match value {
        Integer::Single(_) | Integer::Double(_) => Ok(0),
        Integer::Large(value) => value.capacity().checked_add(7).map(|bits| bits / 8).ok_or(
            GeneratedResidualAffinePivotTargetMatchingError::ResourceCountOverflow {
                resource: "affine target retained payload bytes",
            },
        ),
    }
}

fn payload_eq(
    left: &GeneratedResidualAffinePivotTargetMatchingCertificate,
    right: &GeneratedResidualAffinePivotTargetMatchingCertificate,
) -> Result<bool, GeneratedResidualAffinePivotTargetMatchingError> {
    let census = payload_census_pair(&left.outcomes, &right.outcomes, left.limits)?;
    if census.units != left.stats.payload_comparison_units
        || census.bytes != left.stats.payload_comparison_bytes
        || census.integer_bits != left.stats.payload_comparison_integer_bits
        || census.relation_manifest_bytes != left.stats.payload_comparison_relation_manifest_bytes
    {
        return Ok(false);
    }
    if left.schema != right.schema
        || left.source_case_ordinal != right.source_case_ordinal
        || left.source_group_ordinal != right.source_group_ordinal
        || left.limits != right.limits
        || left.stats != right.stats
        || left.outcomes.len() != right.outcomes.len()
    {
        return Ok(false);
    }
    if !Arc::ptr_eq(&left.inventory, &right.inventory)
        || !Arc::ptr_eq(&left.reelimination, &right.reelimination)
    {
        return Ok(false);
    }
    for (left, right) in left.outcomes.iter().zip(&right.outcomes) {
        match (left, right) {
            (
                GeneratedResidualAffinePivotTargetOutcome::RejectedNoTargetCase(left),
                GeneratedResidualAffinePivotTargetOutcome::RejectedNoTargetCase(right),
            ) => {
                if left.pivot_ordinal != right.pivot_ordinal
                    || left.pivot != right.pivot
                    || left.checked_target_case_ordinals != right.checked_target_case_ordinals
                    || left.transformed_target_constants != right.transformed_target_constants
                {
                    return Ok(false);
                }
            }
            (
                GeneratedResidualAffinePivotTargetOutcome::RejectedRecenteringBoundary(left),
                GeneratedResidualAffinePivotTargetOutcome::RejectedRecenteringBoundary(right),
            ) => {
                if left.pivot_ordinal != right.pivot_ordinal
                    || left.pivot != right.pivot
                    || left.checked_target_case_ordinals != right.checked_target_case_ordinals
                    || left.matching_target_case_ordinals != right.matching_target_case_ordinals
                    || left.kind != right.kind
                    || left.position != right.position
                    || left.transformed_target_constants != right.transformed_target_constants
                {
                    return Ok(false);
                }
            }
            (
                GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(left),
                GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(right),
            ) => {
                if left.pivot_ordinal != right.pivot_ordinal
                    || left.pivot != right.pivot
                    || left.checked_target_case_ordinals != right.checked_target_case_ordinals
                    || left.matching_target_case_ordinals != right.matching_target_case_ordinals
                    || left.coefficient_translation != right.coefficient_translation
                    || left.key_center != right.key_center
                    || left.recentering_stats != right.recentering_stats
                    || !left
                        .relation
                        .has_identical_guard_provenance(&right.relation)
                    || left.transformed_target_constants != right.transformed_target_constants
                {
                    return Ok(false);
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

fn first_available_target(
    ordered_candidates: &[usize],
    consumed: &BTreeSet<usize>,
) -> Option<usize> {
    ordered_candidates
        .iter()
        .copied()
        .find(|ordinal| !consumed.contains(ordinal))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PayloadCensus {
    units: usize,
    bytes: usize,
    integer_bits: usize,
    relation_manifest_bytes: usize,
}

fn payload_census_same(
    outcomes: &[GeneratedResidualAffinePivotTargetOutcome],
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<PayloadCensus, GeneratedResidualAffinePivotTargetMatchingError> {
    payload_census_pair(outcomes, outcomes, limits)
}

fn payload_census_pair(
    left: &[GeneratedResidualAffinePivotTargetOutcome],
    right: &[GeneratedResidualAffinePivotTargetOutcome],
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<PayloadCensus, GeneratedResidualAffinePivotTargetMatchingError> {
    let mut census = PayloadCensus::default();
    census_add_units(&mut census, 10, limits)?;
    census_add_bytes(
        &mut census,
        checked_mul(
            "affine target payload comparison bytes",
            size_of::<GeneratedResidualAffinePivotTargetMatchingCertificate>(),
            2,
        )?,
        limits,
    )?;
    for outcomes in [left, right] {
        census_add_units(&mut census, outcomes.len(), limits)?;
        census_add_bytes(
            &mut census,
            checked_mul(
                "affine target payload comparison bytes",
                outcomes.len(),
                size_of::<GeneratedResidualAffinePivotTargetOutcome>(),
            )?,
            limits,
        )?;
        for outcome in outcomes {
            match outcome {
                GeneratedResidualAffinePivotTargetOutcome::RejectedNoTargetCase(value) => {
                    census_add_units(&mut census, 2, limits)?;
                    census_shift(&mut census, &value.pivot, limits)?;
                    census_integers(&mut census, &value.transformed_target_constants, limits)?;
                    census_usizes(&mut census, &value.checked_target_case_ordinals, limits)?;
                }
                GeneratedResidualAffinePivotTargetOutcome::RejectedRecenteringBoundary(value) => {
                    census_add_units(&mut census, 6, limits)?;
                    census_shift(&mut census, &value.pivot, limits)?;
                    census_integers(&mut census, &value.transformed_target_constants, limits)?;
                    census_usizes(&mut census, &value.checked_target_case_ordinals, limits)?;
                    census_usizes(&mut census, &value.matching_target_case_ordinals, limits)?;
                }
                GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(value) => {
                    census_add_units(&mut census, 4, limits)?;
                    census_shift(&mut census, &value.pivot, limits)?;
                    census_shift(&mut census, &value.coefficient_translation, limits)?;
                    census_shift(&mut census, &value.key_center, limits)?;
                    census_integers(&mut census, &value.transformed_target_constants, limits)?;
                    census_usizes(&mut census, &value.checked_target_case_ordinals, limits)?;
                    census_usizes(&mut census, &value.matching_target_case_ordinals, limits)?;
                    census_relation(&mut census, &value.relation, limits)?;
                }
            }
        }
    }
    Ok(census)
}

fn census_shift(
    census: &mut PayloadCensus,
    shift: &IndexShift,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
    census_add_units(census, shift.arity(), limits)?;
    census_add_bytes(
        census,
        checked_mul(
            "affine target payload comparison bytes",
            shift.arity(),
            size_of::<i64>(),
        )?,
        limits,
    )
}

fn census_usizes(
    census: &mut PayloadCensus,
    values: &[usize],
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
    census_add_units(census, values.len(), limits)?;
    census_add_bytes(
        census,
        checked_mul(
            "affine target payload comparison bytes",
            values.len(),
            size_of::<usize>(),
        )?,
        limits,
    )
}

fn census_integers(
    census: &mut PayloadCensus,
    values: &[Integer],
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
    census_add_units(census, values.len(), limits)?;
    census_add_bytes(
        census,
        checked_mul(
            "affine target payload comparison bytes",
            values.len(),
            size_of::<Integer>(),
        )?,
        limits,
    )?;
    for value in values {
        let bits = integer_magnitude_bits(value)?.max(1);
        census.integer_bits = bounded_add(
            "affine target payload comparison integer bits",
            census.integer_bits,
            bits,
            limits.max_payload_comparison_integer_bits,
        )?;
        census_add_bytes(
            census,
            checked_mul(
                "affine target payload comparison bytes",
                bits.checked_add(63).ok_or(
                    GeneratedResidualAffinePivotTargetMatchingError::ResourceCountOverflow {
                        resource: "affine target payload comparison bytes",
                    },
                )? / 64,
                size_of::<u64>(),
            )?,
            limits,
        )?;
    }
    Ok(())
}

fn census_relation(
    census: &mut PayloadCensus,
    relation: &ParametricRelation,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
    census_add_units(
        census,
        checked_add(
            "affine target payload comparison units",
            relation.terms().len(),
            relation.guarded_nonzero_conditions().len(),
        )?,
        limits,
    )?;
    let remaining_manifest = limits
        .max_payload_comparison_relation_manifest_bytes
        .checked_sub(census.relation_manifest_bytes)
        .ok_or(
            GeneratedResidualAffinePivotTargetMatchingError::ResourceLimit {
                resource: "affine target payload comparison relation manifest bytes",
                requested: census.relation_manifest_bytes,
                limit: limits.max_payload_comparison_relation_manifest_bytes,
            },
        )?;
    let manifest_bytes = relation.stable_manifest_byte_len_with_limit(remaining_manifest)?;
    census.relation_manifest_bytes = bounded_add(
        "affine target payload comparison relation manifest bytes",
        census.relation_manifest_bytes,
        manifest_bytes,
        limits.max_payload_comparison_relation_manifest_bytes,
    )?;
    census_add_bytes(census, manifest_bytes, limits)?;
    let retained_bytes = relation.owned_retained_byte_bound().ok_or(
        GeneratedResidualAffinePivotTargetMatchingError::ResourceCountOverflow {
            resource: "affine target payload comparison bytes",
        },
    )?;
    census_add_bytes(census, retained_bytes, limits)
}

fn census_add_units(
    census: &mut PayloadCensus,
    additional: usize,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
    census.units = bounded_add(
        "affine target payload comparison units",
        census.units,
        additional,
        limits.max_payload_comparison_units,
    )?;
    Ok(())
}

fn census_add_bytes(
    census: &mut PayloadCensus,
    additional: usize,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
    census.bytes = bounded_add(
        "affine target payload comparison bytes",
        census.bytes,
        additional,
        limits.max_payload_comparison_bytes,
    )?;
    Ok(())
}

fn copy_shift(
    source: &IndexShift,
) -> Result<IndexShift, GeneratedResidualAffinePivotTargetMatchingError> {
    let mut values = Vec::new();
    try_reserve_exact(
        "affine target retained shift components",
        &mut values,
        source.arity(),
    )?;
    values.extend_from_slice(source.values());
    Ok(IndexShift::try_from_preallocated(values, source.arity())?)
}

fn observe_affine_integer_bits(
    stats: &mut GeneratedResidualAffinePivotTargetMatchingStats,
    requested: usize,
    limits: GeneratedResidualAffinePivotTargetMatchingLimits,
) -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
    check_limit(
        "affine target transformed integer bits",
        requested,
        limits.max_affine_integer_bits,
    )?;
    stats.maximum_affine_integer_bits = stats.maximum_affine_integer_bits.max(requested);
    Ok(())
}

fn integer_magnitude_bits(
    value: &Integer,
) -> Result<usize, GeneratedResidualAffinePivotTargetMatchingError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| {
        GeneratedResidualAffinePivotTargetMatchingError::ResourceCountOverflow {
            resource: "affine target integer bits",
        }
    })
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
) -> Result<usize, GeneratedResidualAffinePivotTargetMatchingError> {
    limit.checked_sub(used).ok_or(
        GeneratedResidualAffinePivotTargetMatchingError::ResourceLimit {
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
) -> Result<usize, GeneratedResidualAffinePivotTargetMatchingError> {
    left.checked_add(right)
        .ok_or(GeneratedResidualAffinePivotTargetMatchingError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffinePivotTargetMatchingError> {
    left.checked_mul(right)
        .ok_or(GeneratedResidualAffinePivotTargetMatchingError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedResidualAffinePivotTargetMatchingError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(
            GeneratedResidualAffinePivotTargetMatchingError::ResourceLimit {
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
) -> Result<(), GeneratedResidualAffinePivotTargetMatchingError> {
    let requested = checked_add(resource, target.len(), additional)?;
    target.try_reserve_exact(additional).map_err(|_| {
        GeneratedResidualAffinePivotTargetMatchingError::AllocationFailure {
            resource,
            requested,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CoefficientContext;

    #[test]
    fn deferred_two_pivot_availability_matches_both_litered_transitions() {
        // Two provisional pivots have the same ordered exact-target set.
        // Acceptance of the first consumes only its selected target, so the
        // second falls through. `WhenBad === True` consumes no target, so the
        // second sees the original first candidate again.
        let first_pivot_candidates = [17usize, 23];
        let second_pivot_candidates = [17usize, 23];

        let mut accepted_consumed = BTreeSet::new();
        let accepted_first =
            first_available_target(&first_pivot_candidates, &accepted_consumed).unwrap();
        assert_eq!(accepted_first, 17);
        accepted_consumed.insert(accepted_first);
        assert_eq!(
            first_available_target(&second_pivot_candidates, &accepted_consumed),
            Some(23)
        );

        let when_bad_true_consumed = BTreeSet::new();
        assert_eq!(
            first_available_target(&first_pivot_candidates, &when_bad_true_consumed),
            Some(17)
        );
        assert_eq!(
            first_available_target(&second_pivot_candidates, &when_bad_true_consumed),
            Some(17)
        );
    }

    #[test]
    fn source_integer_clone_is_admitted_by_cumulative_bit_budget_first() {
        let source = [Integer::from(i64::MAX)];
        let pivot = IndexShift::try_new([0], 1).unwrap();
        let source_bits = integer_magnitude_bits(&source[0]).unwrap().max(1);
        let mut limits = GeneratedResidualAffinePivotTargetMatchingLimits::default();
        limits.max_affine_integer_bit_work = source_bits - 1;
        let mut stats = GeneratedResidualAffinePivotTargetMatchingStats::default();
        let mut retained_payload_admission = RetainedPayloadAdmission {
            bytes: 0,
            limit: usize::MAX,
        };
        assert!(matches!(
            transformed_target_constants(
                &source,
                &[],
                &[],
                &pivot,
                &mut stats,
                &mut retained_payload_admission,
                limits,
            ),
            Err(GeneratedResidualAffinePivotTargetMatchingError::ResourceLimit {
                resource: "affine target affine integer-bit work",
                requested,
                limit,
            }) if requested == source_bits && limit + 1 == requested
        ));
    }

    #[test]
    fn coefficient_negation_boundary_uses_only_the_retained_pivot_budget() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(
            &base,
            "affine-target-coefficient-negation-boundary",
            1,
        )
        .unwrap();
        let mut relation = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: Arc::from("coefficient-negation-boundary"),
            },
            &context,
        );
        relation
            .add_term(
                &context,
                IndexShift::try_new([0], 1).unwrap(),
                context.one(),
            )
            .unwrap();
        let pivot = IndexShift::try_new([i64::MIN], 1).unwrap();
        let mut limits = GeneratedResidualAffinePivotTargetMatchingLimits::default();
        limits.max_retained_shift_components = 1;
        limits.max_recenter_attempts = 0;
        limits.max_recenter_key_subtraction_boundary_checks = 0;
        let mut stats = GeneratedResidualAffinePivotTargetMatchingStats {
            retained_shift_components: 1,
            ..Default::default()
        };
        assert_eq!(
            preflight_recentering_disposition(
                &mut stats,
                &relation,
                &[0],
                &pivot,
                1,
                limits,
            )
            .unwrap(),
            RecenteringDisposition::Boundary {
                position: 0,
                kind: GeneratedResidualAffineRecenteringBoundaryKind::FreeCoefficientTranslationNegation,
            }
        );
        assert_eq!(stats.retained_shift_components(), 1);
        assert_eq!(stats.recenter_attempts(), 0);
        assert_eq!(stats.recenter_key_subtraction_boundary_checks(), 0);
    }

    #[test]
    fn key_subtraction_boundary_has_exact_and_one_below_work_limits() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(
            &base,
            "affine-target-key-subtraction-boundary",
            1,
        )
        .unwrap();
        let mut relation = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: Arc::from("key-subtraction-boundary"),
            },
            &context,
        );
        relation
            .add_term(
                &context,
                IndexShift::try_new([i64::MIN], 1).unwrap(),
                context.one(),
            )
            .unwrap();
        let pivot = IndexShift::try_new([1], 1).unwrap();
        let mut exact = GeneratedResidualAffinePivotTargetMatchingLimits::default();
        exact.max_retained_shift_components = 1;
        exact.max_recenter_attempts = 1;
        exact.max_recenter_key_subtraction_boundary_checks = 1;
        let mut stats = GeneratedResidualAffinePivotTargetMatchingStats {
            retained_shift_components: 1,
            ..Default::default()
        };
        assert_eq!(
            preflight_recentering_disposition(&mut stats, &relation, &[], &pivot, 1, exact,)
                .unwrap(),
            RecenteringDisposition::Boundary {
                position: 0,
                kind: GeneratedResidualAffineRecenteringBoundaryKind::IntegralKeySubtraction,
            }
        );
        assert_eq!(stats.retained_shift_components(), 1);
        assert_eq!(stats.recenter_key_subtraction_boundary_checks(), 1);

        let mut one_below = exact;
        one_below.max_recenter_key_subtraction_boundary_checks = 0;
        let mut rejected_stats = GeneratedResidualAffinePivotTargetMatchingStats {
            retained_shift_components: 1,
            ..Default::default()
        };
        assert!(matches!(
            preflight_recentering_disposition(
                &mut rejected_stats,
                &relation,
                &[],
                &pivot,
                1,
                one_below,
            ),
            Err(
                GeneratedResidualAffinePivotTargetMatchingError::ResourceLimit {
                    resource: "affine target recentered key-subtraction boundary checks",
                    requested: 1,
                    limit: 0,
                }
            )
        ));
    }

    #[test]
    fn tampered_schema_is_rejected_by_the_first_replay_gate() {
        assert_eq!(
            validate_schema_for_replay(
                "rustred-generated-residual-affine-pivot-target-matching-tampered"
            ),
            Err(GeneratedResidualAffinePivotTargetMatchingError::SchemaMismatch)
        );
        assert_eq!(
            validate_schema_for_replay(GENERATED_RESIDUAL_AFFINE_PIVOT_TARGET_MATCHING_V1_SCHEMA),
            Ok(())
        );
    }
}
