//! Exact, unpublished recentering candidate for one generated affine group.
//!
//! This V1 kernel closes the arbitrary-precision gap in the older `i64`
//! pivot-target adapter.  Production input is borrowed only from an exact
//! replayed case re-elimination certificate.  Every local integral key is
//! mapped through the retained physical frame, the leading physical key `r`
//! is selected, and the first still-unresolved matching start in the retained
//! solve-plan order is found from `t = r - A r_F`.
//!
//! The output is deliberately private and inert.  It consumes no target,
//! publishes no rule, and makes no master or zero-sector claim.  In
//! particular this raw certificate-row adapter is **not** the authoritative
//! future database ingress: cross-case pivots must first be sealed as exact
//! unrecentered physical rows and reduced against already committed rules.
//! Only the authenticated post-reduction leader may subsequently drive this
//! recentering operation.  The future persistent group database must own that
//! normalization, event epochs, aggregate native-memory policy, and target
//! state.  V1 nevertheless charges the repeated logical preflight performed
//! internally by Symbolica translation and exposes a conservative
//! native-temporary envelope before executing a substitution.

use std::fmt;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::Integer;

#[cfg(test)]
use crate::generated_affine_residual_case_inventory::GeneratedAffineResidualCaseAuthority;
use crate::generated_affine_residual_case_reelimination::{
    GeneratedAffineResidualCaseReeliminationCertificate,
    GeneratedAffineResidualCaseReeliminationError,
};
#[cfg(test)]
use crate::generated_affine_residual_group_exact_recenter_kernel::{
    ExactBorrowedTerm, ExactCenteredShift, centered_shift_arithmetic_operations_for_test,
    execute_centered_shifts, preflight_centered_shifts,
    reset_centered_shift_arithmetic_operations_for_test,
    reset_target_offset_arithmetic_entries_for_test, target_offset_arithmetic_entries_for_test,
};
use crate::generated_affine_residual_group_exact_recenter_kernel::{
    ExactRecenterKernelError, ExactRecenterKernelLimits, ExactRecenterKernelStats,
    ExactRecenteredTerm, admit_inert_owner, arc_vec_retained_bytes_bound, bounded_add, check_limit,
    checked_add, checked_mul, exact_offsets_equal, execute_target_offset, integer_bits,
    native_exact_scratch_bytes, preflight_exact_geometry, prospective_integer_heap_bytes,
    translate_centered_row, try_vec, vec_retained_bytes_bound, verify_target_offset_census,
};
use crate::generated_affine_residual_group_physical_key::{
    GeneratedAffineResidualGroupLatticeShift, GeneratedAffineResidualGroupPhysicalFrame,
    GeneratedAffineResidualGroupPhysicalKey, GeneratedAffineResidualGroupPhysicalKeyError,
};
use crate::generated_affine_residual_group_solve_plan::{
    GeneratedAffineResidualGroupSolvePlan, GeneratedAffineResidualGroupSolvePlanReplayLimits,
    GeneratedAffineResidualGroupSolveTargetLocator,
};
use crate::{
    GuardOrigin, IntegralFamily, ParametricArithmeticLimits, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricNonZeroCondition, ParametricRelation,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_RELATION_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-relation-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactRelationLimits {
    pub(crate) arithmetic: ParametricArithmeticLimits,
    pub(crate) max_reelimination_replays: usize,
    pub(crate) max_parent_allocation_comparisons: usize,
    pub(crate) max_arity: usize,
    pub(crate) max_free_positions: usize,
    pub(crate) max_matrix_entries: usize,
    pub(crate) max_terms: usize,
    pub(crate) max_guards: usize,
    pub(crate) max_witnesses: usize,
    pub(crate) max_unresolved_targets: usize,
    pub(crate) max_target_scans: usize,
    pub(crate) max_physical_key_preflights: usize,
    pub(crate) max_physical_key_constructions: usize,
    pub(crate) max_physical_key_component_scans: usize,
    pub(crate) max_physical_key_integer_bit_work: usize,
    pub(crate) max_physical_key_prospective_integer_bits: usize,
    pub(crate) max_physical_key_prospective_retained_bytes: usize,
    pub(crate) max_geometry_integer_operations: usize,
    pub(crate) max_geometry_integer_bit_work: usize,
    pub(crate) max_target_offset_integer_bits: usize,
    pub(crate) max_target_offset_temporary_bytes: usize,
    pub(crate) max_exact_integer_bits: usize,
    pub(crate) max_exact_shift_components: usize,
    pub(crate) max_exact_shift_integer_bits: usize,
    pub(crate) max_exact_shift_retained_bytes: usize,
    pub(crate) max_centered_shift_outer_buffer_bytes: usize,
    pub(crate) max_borrowed_reference_buffer_bytes: usize,
    pub(crate) max_coefficient_translation_integer_bits: usize,
    pub(crate) max_coefficient_translation_retained_bytes: usize,
    pub(crate) max_translation_preflight_passes: usize,
    pub(crate) max_translation_source_terms: usize,
    pub(crate) max_translation_source_exponent_entries: usize,
    pub(crate) max_translation_output_terms: usize,
    pub(crate) max_translation_output_exponent_entries: usize,
    pub(crate) max_translation_power_operations: usize,
    pub(crate) max_translation_integer_bit_work: usize,
    pub(crate) max_translation_normalized_terms: usize,
    pub(crate) max_translation_retained_output_bytes: usize,
    pub(crate) max_guard_origin_occurrences: usize,
    pub(crate) max_owner_retained_bytes: usize,
    pub(crate) max_native_temporary_byte_envelope: usize,
}

impl Default for GeneratedAffineResidualGroupExactRelationLimits {
    fn default() -> Self {
        const LARGE: usize = 64_000_000_000;
        const VERY_LARGE: usize = 4_000_000_000_000_000_000;
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            max_reelimination_replays: 1,
            max_parent_allocation_comparisons: 6,
            max_arity: 1_000_000,
            max_free_positions: 1_000_000,
            max_matrix_entries: LARGE,
            max_terms: 16_000_000,
            max_guards: 16_000_000,
            max_witnesses: 100_000_000,
            max_unresolved_targets: 256_000_000,
            max_target_scans: 256_000_000,
            max_physical_key_preflights: 16_000_000,
            max_physical_key_constructions: 16_000_000,
            max_physical_key_component_scans: LARGE,
            max_physical_key_integer_bit_work: VERY_LARGE,
            max_physical_key_prospective_integer_bits: VERY_LARGE,
            max_physical_key_prospective_retained_bytes: 128 * 1024 * 1024 * 1024,
            max_geometry_integer_operations: LARGE,
            max_geometry_integer_bit_work: VERY_LARGE,
            max_target_offset_integer_bits: VERY_LARGE,
            max_target_offset_temporary_bytes: 128 * 1024 * 1024 * 1024,
            max_exact_integer_bits: VERY_LARGE,
            max_exact_shift_components: LARGE,
            max_exact_shift_integer_bits: VERY_LARGE,
            max_exact_shift_retained_bytes: 128 * 1024 * 1024 * 1024,
            max_centered_shift_outer_buffer_bytes: 16 * 1024 * 1024 * 1024,
            max_borrowed_reference_buffer_bytes: 16 * 1024 * 1024 * 1024,
            max_coefficient_translation_integer_bits: VERY_LARGE,
            max_coefficient_translation_retained_bytes: 128 * 1024 * 1024 * 1024,
            max_translation_preflight_passes: LARGE,
            max_translation_source_terms: VERY_LARGE,
            max_translation_source_exponent_entries: VERY_LARGE,
            max_translation_output_terms: VERY_LARGE,
            max_translation_output_exponent_entries: VERY_LARGE,
            max_translation_power_operations: VERY_LARGE,
            max_translation_integer_bit_work: VERY_LARGE,
            max_translation_normalized_terms: VERY_LARGE,
            max_translation_retained_output_bytes: 128 * 1024 * 1024 * 1024,
            max_guard_origin_occurrences: LARGE,
            max_owner_retained_bytes: 128 * 1024 * 1024 * 1024,
            max_native_temporary_byte_envelope: 256 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactRelationStats {
    reelimination_replays: usize,
    parent_allocation_comparisons: usize,
    arity: usize,
    free_positions: usize,
    matrix_entries: usize,
    terms: usize,
    guards: usize,
    witnesses: usize,
    unresolved_targets: usize,
    target_scans: usize,
    physical_key_preflights: usize,
    physical_key_constructions: usize,
    physical_key_component_scans: usize,
    physical_key_integer_bit_work: usize,
    physical_key_prospective_integer_bits: usize,
    physical_key_prospective_retained_bytes: usize,
    physical_key_retained_bytes: usize,
    geometry_integer_operations: usize,
    geometry_integer_bit_work: usize,
    target_offset_integer_bits: usize,
    target_offset_temporary_bytes: usize,
    exact_shift_components: usize,
    exact_shift_integer_bits: usize,
    exact_shift_retained_bytes: usize,
    centered_shift_outer_buffer_bytes: usize,
    borrowed_reference_buffer_bytes: usize,
    coefficient_translation_integer_bits: usize,
    coefficient_translation_retained_bytes: usize,
    translation_preflight_passes: usize,
    translation_source_terms: usize,
    translation_source_exponent_entries: usize,
    translation_output_terms: usize,
    translation_output_exponent_entries: usize,
    translation_power_operations: usize,
    translation_integer_bit_work: usize,
    translation_normalized_terms: usize,
    translation_retained_output_bytes: usize,
    guard_origin_occurrences: usize,
    owner_retained_bytes: usize,
    native_temporary_byte_envelope: usize,
}

macro_rules! stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualGroupExactRelationStats {
    stats_getters!(
        reelimination_replays,
        parent_allocation_comparisons,
        arity,
        free_positions,
        matrix_entries,
        terms,
        guards,
        witnesses,
        unresolved_targets,
        target_scans,
        physical_key_preflights,
        physical_key_constructions,
        physical_key_component_scans,
        physical_key_integer_bit_work,
        physical_key_prospective_integer_bits,
        physical_key_prospective_retained_bytes,
        physical_key_retained_bytes,
        geometry_integer_operations,
        geometry_integer_bit_work,
        target_offset_integer_bits,
        target_offset_temporary_bytes,
        exact_shift_components,
        exact_shift_integer_bits,
        exact_shift_retained_bytes,
        centered_shift_outer_buffer_bytes,
        borrowed_reference_buffer_bytes,
        coefficient_translation_integer_bits,
        coefficient_translation_retained_bytes,
        translation_preflight_passes,
        translation_source_terms,
        translation_source_exponent_entries,
        translation_output_terms,
        translation_output_exponent_entries,
        translation_power_operations,
        translation_integer_bit_work,
        translation_normalized_terms,
        translation_retained_output_bytes,
        guard_origin_occurrences,
        owner_retained_bytes,
        native_temporary_byte_envelope,
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactRelationError {
    WrongFamily,
    WrongContext,
    WrongArity,
    WrongParentAllocation,
    WrongCaseBinding,
    WrongGroupBinding,
    WrongWitnessBinding,
    WrongUnresolvedShape,
    EmptyRelation,
    MalformedGeometry,
    Reelimination,
    PhysicalKey,
    Coefficient,
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

impl GeneratedAffineResidualGroupExactRelationError {
    const fn kind(self) -> &'static str {
        match self {
            Self::WrongFamily => "WrongFamily",
            Self::WrongContext => "WrongContext",
            Self::WrongArity => "WrongArity",
            Self::WrongParentAllocation => "WrongParentAllocation",
            Self::WrongCaseBinding => "WrongCaseBinding",
            Self::WrongGroupBinding => "WrongGroupBinding",
            Self::WrongWitnessBinding => "WrongWitnessBinding",
            Self::WrongUnresolvedShape => "WrongUnresolvedShape",
            Self::EmptyRelation => "EmptyRelation",
            Self::MalformedGeometry => "MalformedGeometry",
            Self::Reelimination => "Reelimination",
            Self::PhysicalKey => "PhysicalKey",
            Self::Coefficient => "Coefficient",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::AllocationFailure { .. } => "AllocationFailure",
            Self::SymbolicaPanic => "SymbolicaPanic",
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactRelationError")
            .field("kind", &self.kind())
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "generated affine exact recentering {}",
            self.kind()
        )
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactRelationError {}

impl From<GeneratedAffineResidualGroupPhysicalKeyError>
    for GeneratedAffineResidualGroupExactRelationError
{
    fn from(_: GeneratedAffineResidualGroupPhysicalKeyError) -> Self {
        Self::PhysicalKey
    }
}

impl From<ParametricCoefficientError> for GeneratedAffineResidualGroupExactRelationError {
    fn from(_: ParametricCoefficientError) -> Self {
        Self::Coefficient
    }
}

impl From<ExactRecenterKernelError> for GeneratedAffineResidualGroupExactRelationError {
    fn from(error: ExactRecenterKernelError) -> Self {
        match error {
            ExactRecenterKernelError::MalformedGeometry => Self::MalformedGeometry,
            ExactRecenterKernelError::CensusMismatch => Self::PhysicalKey,
            ExactRecenterKernelError::OutputCensusMismatch => Self::Coefficient,
            ExactRecenterKernelError::Coefficient => Self::Coefficient,
            ExactRecenterKernelError::ResourceLimit {
                resource,
                requested,
                limit,
            } => Self::ResourceLimit {
                resource,
                requested,
                limit,
            },
            ExactRecenterKernelError::ResourceCountOverflow { resource } => {
                Self::ResourceCountOverflow { resource }
            }
            ExactRecenterKernelError::AllocationFailure { resource } => {
                Self::AllocationFailure { resource }
            }
        }
    }
}

fn kernel_limits(
    limits: GeneratedAffineResidualGroupExactRelationLimits,
) -> ExactRecenterKernelLimits {
    ExactRecenterKernelLimits {
        arithmetic: limits.arithmetic,
        max_terms: limits.max_terms,
        max_guards: limits.max_guards,
        max_geometry_integer_operations: limits.max_geometry_integer_operations,
        max_geometry_integer_bit_work: limits.max_geometry_integer_bit_work,
        max_target_offset_integer_bits: limits.max_target_offset_integer_bits,
        max_target_offset_temporary_bytes: limits.max_target_offset_temporary_bytes,
        max_exact_integer_bits: limits.max_exact_integer_bits,
        max_exact_shift_components: limits.max_exact_shift_components,
        max_exact_shift_integer_bits: limits.max_exact_shift_integer_bits,
        max_exact_shift_retained_bytes: limits.max_exact_shift_retained_bytes,
        max_centered_shift_outer_buffer_bytes: limits.max_centered_shift_outer_buffer_bytes,
        max_borrowed_reference_buffer_bytes: limits.max_borrowed_reference_buffer_bytes,
        max_coefficient_translation_integer_bits: limits.max_coefficient_translation_integer_bits,
        max_coefficient_translation_retained_bytes: limits
            .max_coefficient_translation_retained_bytes,
        max_translation_preflight_passes: limits.max_translation_preflight_passes,
        max_translation_source_terms: limits.max_translation_source_terms,
        max_translation_source_exponent_entries: limits.max_translation_source_exponent_entries,
        max_translation_output_terms: limits.max_translation_output_terms,
        max_translation_output_exponent_entries: limits.max_translation_output_exponent_entries,
        max_translation_power_operations: limits.max_translation_power_operations,
        max_translation_integer_bit_work: limits.max_translation_integer_bit_work,
        max_translation_normalized_terms: limits.max_translation_normalized_terms,
        max_translation_retained_output_bytes: limits.max_translation_retained_output_bytes,
        max_guard_origin_occurrences: limits.max_guard_origin_occurrences,
        max_owner_retained_bytes: limits.max_owner_retained_bytes,
        max_combined_live_retained_bytes: limits.max_owner_retained_bytes,
        max_native_temporary_byte_envelope: limits.max_native_temporary_byte_envelope,
    }
}

fn merge_kernel_stats(
    stats: &mut GeneratedAffineResidualGroupExactRelationStats,
    kernel: ExactRecenterKernelStats,
) {
    stats.geometry_integer_operations = kernel.geometry_integer_operations();
    stats.geometry_integer_bit_work = kernel.geometry_integer_bit_work();
    stats.target_offset_integer_bits = kernel.target_offset_integer_bits();
    stats.target_offset_temporary_bytes = kernel.target_offset_temporary_bytes();
    stats.exact_shift_components = kernel.exact_shift_components();
    stats.exact_shift_integer_bits = kernel.exact_shift_integer_bits();
    stats.exact_shift_retained_bytes = kernel.exact_shift_retained_bytes();
    stats.centered_shift_outer_buffer_bytes = kernel.centered_shift_outer_buffer_bytes();
    stats.borrowed_reference_buffer_bytes = kernel.borrowed_reference_buffer_bytes();
    stats.coefficient_translation_integer_bits = kernel.coefficient_translation_integer_bits();
    stats.coefficient_translation_retained_bytes = kernel.coefficient_translation_retained_bytes();
    stats.translation_preflight_passes = kernel.translation_preflight_passes();
    stats.translation_source_terms = kernel.translation_source_terms();
    stats.translation_source_exponent_entries = kernel.translation_source_exponent_entries();
    stats.translation_output_terms = kernel.translation_output_terms();
    stats.translation_output_exponent_entries = kernel.translation_output_exponent_entries();
    stats.translation_power_operations = kernel.translation_power_operations();
    stats.translation_integer_bit_work = kernel.translation_integer_bit_work();
    stats.translation_normalized_terms = kernel.translation_normalized_terms();
    stats.translation_retained_output_bytes = kernel.translation_retained_output_bytes();
    stats.guard_origin_occurrences = kernel.guard_origin_occurrences();
    stats.owner_retained_bytes = kernel.owner_retained_bytes();
    stats.native_temporary_byte_envelope = kernel.native_temporary_byte_envelope();
}

fn physical_native_scratch_bytes(
    stats: &GeneratedAffineResidualGroupExactRelationStats,
) -> Result<usize, GeneratedAffineResidualGroupExactRelationError> {
    let resource = "exact recentering native temporary byte envelope";
    let mut bytes = vec_retained_bytes_bound::<GeneratedAffineResidualGroupPhysicalKey>(
        stats.physical_key_constructions,
    )?;
    for increment in [
        stats.physical_key_prospective_retained_bytes,
        stats.physical_key_retained_bytes,
    ] {
        bytes = checked_add(resource, bytes, increment)?;
    }
    Ok(bytes)
}

#[derive(Clone)]
enum ExactSourceBinding {
    Production(Arc<GeneratedAffineResidualCaseReeliminationCertificate>),
    #[cfg(test)]
    Synthetic(Arc<GeneratedAffineResidualCaseAuthority>),
}

#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualGroupExactRelationCandidate {
    schema: &'static str,
    source: ExactSourceBinding,
    frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
    source_case_ordinal: usize,
    source_row_ordinal: usize,
    witness_ordinal: usize,
    target: GeneratedAffineResidualGroupSolveTargetLocator,
    pivot: GeneratedAffineResidualGroupLatticeShift,
    coefficient_translation: Arc<Vec<Integer>>,
    terms: Arc<Vec<ExactRecenteredTerm>>,
    guards: Arc<Vec<ParametricNonZeroCondition>>,
    limits: GeneratedAffineResidualGroupExactRelationLimits,
    stats: GeneratedAffineResidualGroupExactRelationStats,
}

impl fmt::Debug for GeneratedAffineResidualGroupExactRelationCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactRelationCandidate")
            .field("schema", &self.schema)
            .field("source_case_ordinal", &self.source_case_ordinal)
            .field("source_row_ordinal", &self.source_row_ordinal)
            .field("witness_ordinal", &self.witness_ordinal)
            .field("target_solve_ordinal", &self.target.solve_ordinal())
            .field("term_count", &self.terms.len())
            .field("guard_count", &self.guards.len())
            .field("stats", &self.stats)
            .field("private_source", &"<redacted>")
            .field("private_frame", &"<redacted>")
            .field("private_plan", &"<redacted>")
            .field("private_geometry", &"<redacted>")
            .field("applicable_rule", &false)
            .field("targets_consumed", &0)
            .field("master_inferred", &false)
            .finish()
    }
}

impl GeneratedAffineResidualGroupExactRelationCandidate {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) const fn target_solve_ordinal(&self) -> usize {
        self.target.solve_ordinal()
    }
    pub(crate) const fn target_case_ordinal(&self) -> usize {
        self.target.case_ordinal()
    }
    pub(crate) fn term_count(&self) -> usize {
        self.terms.len()
    }
    pub(crate) fn guard_count(&self) -> usize {
        self.guards.len()
    }
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactRelationStats {
        self.stats
    }
    pub(crate) const fn is_applicable_rule(&self) -> bool {
        false
    }
    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }
    pub(crate) const fn infers_master(&self) -> bool {
        false
    }
    pub(crate) fn same_parent_allocations(
        &self,
        reelimination: &Arc<GeneratedAffineResidualCaseReeliminationCertificate>,
        frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
    ) -> bool {
        matches!(&self.source, ExactSourceBinding::Production(source) if Arc::ptr_eq(source, reelimination))
            && Arc::ptr_eq(&self.frame, frame)
            && Arc::ptr_eq(&self.plan, plan)
    }
}

#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualGroupExactRelationNoTarget {
    source: ExactSourceBinding,
    frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
    stats: GeneratedAffineResidualGroupExactRelationStats,
}

impl fmt::Debug for GeneratedAffineResidualGroupExactRelationNoTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactRelationNoTarget")
            .field("unresolved", &true)
            .field("stats", &self.stats)
            .field("private_source", &"<redacted>")
            .field("private_frame", &"<redacted>")
            .field("private_plan", &"<redacted>")
            .finish()
    }
}

#[derive(Clone)]
pub(crate) enum GeneratedAffineResidualGroupExactRelationOutcome {
    NoTarget(GeneratedAffineResidualGroupExactRelationNoTarget),
    Pending(GeneratedAffineResidualGroupExactRelationCandidate),
}

impl GeneratedAffineResidualGroupExactRelationOutcome {
    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }
    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }
    pub(crate) const fn infers_master(&self) -> bool {
        false
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactRelationOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTarget(value) => value.fmt(formatter),
            Self::Pending(value) => value.fmt(formatter),
        }
    }
}

pub(crate) struct GeneratedAffineResidualGroupExactRelationCompiler;

impl GeneratedAffineResidualGroupExactRelationCompiler {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        reelimination: Arc<GeneratedAffineResidualCaseReeliminationCertificate>,
        retained_row_ordinal: usize,
        witness_ordinal: usize,
        frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
        unresolved_targets: &[bool],
        database_epoch: usize,
        event_ordinal: usize,
        limits: GeneratedAffineResidualGroupExactRelationLimits,
    ) -> Result<
        GeneratedAffineResidualGroupExactRelationOutcome,
        GeneratedAffineResidualGroupExactRelationError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            check_limit(
                "exact recentering re-elimination replays",
                1,
                limits.max_reelimination_replays,
            )?;
            // This is an allocation-free child census.  Reject it before the
            // replay rebuild so a stricter exact-group witness budget cannot
            // still incur the rejected certificate's full replay workspace.
            check_limit(
                "exact recentering witnesses",
                reelimination.witnesses().len(),
                limits.max_witnesses,
            )?;
            reelimination
                .replay(
                    family,
                    context,
                    reelimination.authority(),
                    reelimination.premises(),
                    reelimination.ordering(),
                    reelimination.schedule(),
                )
                .map_err(|_| GeneratedAffineResidualGroupExactRelationError::Reelimination)?;
            let authenticated = reelimination
                .authenticate_retained_source_row(retained_row_ordinal, witness_ordinal)
                .map_err(|error| match error {
                    GeneratedAffineResidualCaseReeliminationError::ResourceLimit {
                        resource,
                        requested,
                        limit,
                    } => GeneratedAffineResidualGroupExactRelationError::ResourceLimit {
                        resource,
                        requested,
                        limit,
                    },
                    GeneratedAffineResidualCaseReeliminationError::ResourceCountOverflow {
                        resource,
                    } => GeneratedAffineResidualGroupExactRelationError::ResourceCountOverflow {
                        resource,
                    },
                    _ => GeneratedAffineResidualGroupExactRelationError::WrongWitnessBinding,
                })?;
            compile_authenticated_relation(
                family,
                context,
                ExactSourceBinding::Production(Arc::clone(&reelimination)),
                authenticated.relation(),
                retained_row_ordinal,
                witness_ordinal,
                frame,
                plan,
                unresolved_targets,
                database_epoch,
                event_ordinal,
                limits,
                1,
            )
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactRelationError::SymbolicaPanic)?
    }
}

#[allow(clippy::too_many_arguments)]
fn compile_authenticated_relation(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source_binding: ExactSourceBinding,
    relation: &ParametricRelation,
    source_row_ordinal: usize,
    witness_ordinal: usize,
    frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
    unresolved_targets: &[bool],
    database_epoch: usize,
    event_ordinal: usize,
    limits: GeneratedAffineResidualGroupExactRelationLimits,
    reelimination_replays: usize,
) -> Result<
    GeneratedAffineResidualGroupExactRelationOutcome,
    GeneratedAffineResidualGroupExactRelationError,
> {
    let authority = match &source_binding {
        ExactSourceBinding::Production(source) => source.authority(),
        #[cfg(test)]
        ExactSourceBinding::Synthetic(authority) => authority,
    };
    let mut stats = GeneratedAffineResidualGroupExactRelationStats {
        reelimination_replays,
        parent_allocation_comparisons: 6,
        terms: relation.terms().len(),
        guards: relation.guarded_nonzero_conditions().len(),
        witnesses: match &source_binding {
            ExactSourceBinding::Production(source) => source.witnesses().len(),
            #[cfg(test)]
            ExactSourceBinding::Synthetic(_) => 0,
        },
        unresolved_targets: unresolved_targets.len(),
        ..Default::default()
    };
    for (resource, requested, limit) in [
        (
            "exact recentering parent allocation comparisons",
            stats.parent_allocation_comparisons,
            limits.max_parent_allocation_comparisons,
        ),
        ("exact recentering terms", stats.terms, limits.max_terms),
        ("exact recentering guards", stats.guards, limits.max_guards),
        (
            "exact recentering unresolved targets",
            stats.unresolved_targets,
            limits.max_unresolved_targets,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    if family.fingerprint_ref() != authority.family_fingerprint()
        || relation.family_fingerprint() != authority.family_fingerprint()
    {
        return Err(GeneratedAffineResidualGroupExactRelationError::WrongFamily);
    }
    if context.fingerprint() != authority.context_fingerprint()
        || relation.context_fingerprint() != authority.context_fingerprint()
    {
        return Err(GeneratedAffineResidualGroupExactRelationError::WrongContext);
    }
    if context.index_count() != authority.arity() || relation.arity() != authority.arity() {
        return Err(GeneratedAffineResidualGroupExactRelationError::WrongArity);
    }
    if !authority.same_inventory_allocation(plan.inventory())
        || !plan.same_parent_allocations(plan.inventory(), plan.authority(), &frame)
        || !Arc::ptr_eq(plan.physical_frame(), &frame)
    {
        return Err(GeneratedAffineResidualGroupExactRelationError::WrongParentAllocation);
    }
    frame
        .replay(family, context, plan.authority())
        .map_err(|_| GeneratedAffineResidualGroupExactRelationError::PhysicalKey)?;
    plan.replay(
        family,
        context,
        plan.inventory(),
        plan.authority(),
        &frame,
        GeneratedAffineResidualGroupSolvePlanReplayLimits::default(),
    )
    .map_err(|_| GeneratedAffineResidualGroupExactRelationError::WrongParentAllocation)?;
    if unresolved_targets.len() != plan.targets().len() {
        return Err(GeneratedAffineResidualGroupExactRelationError::WrongUnresolvedShape);
    }
    let source_case = authority
        .authenticated_case_view(context)
        .map_err(|_| GeneratedAffineResidualGroupExactRelationError::WrongCaseBinding)?;
    let group = authority
        .authenticated_group_view(context)
        .map_err(|_| GeneratedAffineResidualGroupExactRelationError::WrongGroupBinding)?;
    if source_case.ordinal() != authority.case_ordinal()
        || source_case.group_ordinal() != authority.group_ordinal()
        || group.ordinal() != authority.group_ordinal()
        || group.ordinal() != plan.group_ordinal()
        || frame.group_ordinal() != group.ordinal()
    {
        return Err(GeneratedAffineResidualGroupExactRelationError::WrongGroupBinding);
    }
    let arity = group.ambient_arity();
    let free_positions = group.free_positions();
    let matrix_entries = checked_mul(
        "exact recentering matrix entries",
        arity,
        free_positions.len(),
    )?;
    if group.compact_linear_coefficients().len() != matrix_entries
        || free_positions != plan.free_positions()
        || source_case.constants().len() != arity
        || frame.arity() != arity
        || free_positions.iter().any(|&position| position >= arity)
    {
        return Err(GeneratedAffineResidualGroupExactRelationError::MalformedGeometry);
    }
    stats.arity = arity;
    stats.free_positions = free_positions.len();
    stats.matrix_entries = matrix_entries;
    for (resource, requested, limit) in [
        ("exact recentering arity", arity, limits.max_arity),
        (
            "exact recentering free positions",
            free_positions.len(),
            limits.max_free_positions,
        ),
        (
            "exact recentering matrix entries",
            matrix_entries,
            limits.max_matrix_entries,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    // Both slices are authenticated children of the retained inventory.  They
    // remain borrowed: cloning a potentially GMP-backed matrix before a local
    // admission would create precisely the uncharged allocation this kernel
    // is intended to avoid.
    let matrix = group.compact_linear_coefficients();
    let source_position = source_case.ordinal_within_group();
    let source_case_ordinal = source_case.ordinal();
    if frame.case_ordinals().get(source_position).copied() != Some(source_case_ordinal) {
        return Err(GeneratedAffineResidualGroupExactRelationError::WrongCaseBinding);
    }
    if relation.terms().is_empty() {
        return Err(GeneratedAffineResidualGroupExactRelationError::EmptyRelation);
    }

    let mut physical_keys = try_vec("exact recentering physical keys", stats.terms)?;
    for local in relation.terms().keys() {
        let physical = frame.physical_from_local(source_position, source_case_ordinal, local)?;
        stats.physical_key_preflights = bounded_add(
            "exact recentering physical-key preflights",
            stats.physical_key_preflights,
            1,
            limits.max_physical_key_preflights,
        )?;
        let preflight = frame.preflight_key_for_physical(&physical)?;
        stats.physical_key_component_scans = bounded_add(
            "exact recentering physical-key component scans",
            stats.physical_key_component_scans,
            preflight.component_scans(),
            limits.max_physical_key_component_scans,
        )?;
        stats.physical_key_integer_bit_work = bounded_add(
            "exact recentering physical-key integer-bit work",
            stats.physical_key_integer_bit_work,
            preflight.integer_bit_work(),
            limits.max_physical_key_integer_bit_work,
        )?;
        stats.physical_key_prospective_integer_bits = bounded_add(
            "exact recentering physical-key prospective integer bits",
            stats.physical_key_prospective_integer_bits,
            preflight.prospective_retained_integer_bits(),
            limits.max_physical_key_prospective_integer_bits,
        )?;
        stats.physical_key_prospective_retained_bytes = bounded_add(
            "exact recentering physical-key prospective retained bytes",
            stats.physical_key_prospective_retained_bytes,
            preflight.prospective_retained_bytes(),
            limits.max_physical_key_prospective_retained_bytes,
        )?;
        stats.physical_key_constructions = bounded_add(
            "exact recentering physical-key constructions",
            stats.physical_key_constructions,
            1,
            limits.max_physical_key_constructions,
        )?;
        let key = frame.key_for_preflight(preflight)?;
        stats.physical_key_retained_bytes = checked_add(
            "exact recentering physical-key retained bytes",
            stats.physical_key_retained_bytes,
            key.retained_bytes(),
        )?;
        physical_keys.push(key);
    }
    let pivot_position = physical_keys
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.cmp(right))
        .map(|(position, _)| position)
        .ok_or(GeneratedAffineResidualGroupExactRelationError::EmptyRelation)?;
    let pivot = physical_keys[pivot_position].shift().clone();

    let exact_limits = kernel_limits(limits);
    let mut exact_stats =
        ExactRecenterKernelStats::for_row(stats.terms, stats.guards, exact_limits)?;
    preflight_exact_geometry(
        &pivot,
        matrix,
        free_positions,
        exact_limits,
        &mut exact_stats,
    )?;
    let target_offset = execute_target_offset(&pivot, matrix, free_positions, arity)?;
    verify_target_offset_census(&target_offset, &mut exact_stats)?;
    let mut selected = None;
    for locator in plan.targets() {
        stats.target_scans = bounded_add(
            "exact recentering target scans",
            stats.target_scans,
            1,
            limits.max_target_scans,
        )?;
        if unresolved_targets
            .get(locator.solve_ordinal())
            .copied()
            .ok_or(GeneratedAffineResidualGroupExactRelationError::WrongUnresolvedShape)?
            && exact_offsets_equal(
                frame
                    .anchor_offset(locator.inventory_position(), locator.case_ordinal())?
                    .values(),
                target_offset.values(),
                exact_limits,
                &mut exact_stats,
            )?
        {
            selected = Some(*locator);
            break;
        }
    }
    let Some(target) = selected else {
        admit_inert_owner(
            size_of::<GeneratedAffineResidualGroupExactRelationNoTarget>(),
            0,
            physical_native_scratch_bytes(&stats)?,
            false,
            exact_limits,
            &mut exact_stats,
        )?;
        merge_kernel_stats(&mut stats, exact_stats);
        return Ok(GeneratedAffineResidualGroupExactRelationOutcome::NoTarget(
            GeneratedAffineResidualGroupExactRelationNoTarget {
                source: source_binding,
                frame,
                plan,
                stats,
            },
        ));
    };

    let locator_origin = GuardOrigin::GeneratedAffineGroupRecentering {
        solve_group_ordinal: plan.group_ordinal(),
        database_epoch,
        event_ordinal,
    };
    let recentered = translate_centered_row(
        context,
        physical_keys
            .iter()
            .map(GeneratedAffineResidualGroupPhysicalKey::shift)
            .zip(relation.terms().values()),
        relation.guarded_nonzero_conditions().iter(),
        &pivot,
        free_positions,
        &locator_origin,
        size_of::<GeneratedAffineResidualGroupExactRelationCandidate>(),
        pivot.retained_bytes(),
        pivot.retained_bytes(),
        false,
        0,
        physical_native_scratch_bytes(&stats)?,
        exact_limits,
        &mut exact_stats,
    )?;
    let (coefficient_translation, terms, guards, completed_exact_stats) = recentered.into_parts();
    merge_kernel_stats(&mut stats, completed_exact_stats);
    Ok(GeneratedAffineResidualGroupExactRelationOutcome::Pending(
        GeneratedAffineResidualGroupExactRelationCandidate {
            schema: GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_RELATION_V1_SCHEMA,
            source: source_binding,
            frame,
            plan,
            source_case_ordinal,
            source_row_ordinal,
            witness_ordinal,
            target,
            pivot,
            coefficient_translation,
            terms,
            guards,
            limits,
            stats,
        },
    ))
}
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn compile_synthetic_for_test(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    authority: Arc<GeneratedAffineResidualCaseAuthority>,
    relation: &ParametricRelation,
    frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
    unresolved_targets: &[bool],
    database_epoch: usize,
    event_ordinal: usize,
    limits: GeneratedAffineResidualGroupExactRelationLimits,
) -> Result<
    GeneratedAffineResidualGroupExactRelationOutcome,
    GeneratedAffineResidualGroupExactRelationError,
> {
    catch_unwind(AssertUnwindSafe(|| {
        authority
            .replay(family, context)
            .map_err(|_| GeneratedAffineResidualGroupExactRelationError::WrongCaseBinding)?;
        compile_authenticated_relation(
            family,
            context,
            ExactSourceBinding::Synthetic(authority),
            relation,
            0,
            0,
            frame,
            plan,
            unresolved_targets,
            database_epoch,
            event_ordinal,
            limits,
            0,
        )
    }))
    .map_err(|_| GeneratedAffineResidualGroupExactRelationError::SymbolicaPanic)?
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

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
    use crate::generated_affine_residual_case_inventory::{
        GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityLimits,
        GeneratedAffineResidualCaseInventoryCertificate,
        GeneratedAffineResidualCaseInventoryCompiler, GeneratedAffineResidualCaseInventoryLimits,
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
    use crate::generated_affine_residual_group_physical_key::GeneratedAffineResidualGroupPhysicalKeyLimits;
    use crate::generated_affine_residual_group_solve_plan::GeneratedAffineResidualGroupSolvePlanLimits;
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
    use crate::parametric_relation::ParametricAffineFreeRecenteringLimits;
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, IndexShift, IntegralOrderingPolicy,
        ParametricIbpGenerator, ParametricRowId, SectorMask,
    };

    const M: i64 = i64::MAX;

    struct Fixture {
        family: IntegralFamily,
        context: ParametricCoefficientContext,
        inventory: Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
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

    fn fixture(name: &str) -> Fixture {
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
        let group_ordinal = (0..inventory.group_count())
            .max_by_key(|&ordinal| {
                inventory
                    .authenticated_group_view(&context, ordinal)
                    .unwrap()
                    .case_ordinals()
                    .len()
            })
            .unwrap();
        let group = inventory
            .authenticated_group_view(&context, group_ordinal)
            .unwrap();
        assert_eq!(group.case_ordinals(), [1, 3]);
        assert_eq!(group.free_positions(), [0]);
        assert_eq!(group.compact_linear_coefficients(), [1, 0, 0]);
        let anchor_authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                &family,
                &context,
                Arc::clone(&inventory),
                group.anchor_case_ordinal(),
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        let frame = Arc::new(
            GeneratedAffineResidualGroupPhysicalFrame::try_new(
                &family,
                &context,
                Arc::clone(&anchor_authority),
                GeneratedAffineResidualGroupPhysicalKeyLimits::default(),
            )
            .unwrap(),
        );
        assert_eq!(
            frame.anchor_offset(0, 1).unwrap().values(),
            [Integer::from(0), Integer::from(0), Integer::from(0)]
        );
        assert_eq!(
            frame.anchor_offset(1, 3).unwrap().values(),
            [Integer::from(0), Integer::from(M - 1), Integer::from(M - 1)]
        );
        let plan = Arc::new(
            GeneratedAffineResidualGroupSolvePlan::try_new(
                &family,
                &context,
                Arc::clone(&inventory),
                Arc::clone(&anchor_authority),
                Arc::clone(&frame),
                GeneratedAffineResidualGroupSolvePlanLimits::default(),
            )
            .unwrap(),
        );
        Fixture {
            family,
            context,
            inventory,
            frame,
            plan,
        }
    }

    fn authority_for_case(
        fixture: &Fixture,
        case_ordinal: usize,
    ) -> Arc<GeneratedAffineResidualCaseAuthority> {
        Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                &fixture.family,
                &fixture.context,
                Arc::clone(&fixture.inventory),
                case_ordinal,
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        )
    }

    fn source_relation(
        fixture: &Fixture,
        first: [i64; 3],
        second: Option<[i64; 3]>,
        private_label: &str,
    ) -> ParametricRelation {
        let context = &fixture.context;
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let coefficient = context
            .add(&n0, &context.mul(&context.integer(2), &n1).unwrap())
            .unwrap();
        let mut relation = ParametricRelation::new(
            fixture.family.fingerprint_ref(),
            ParametricRowId::Derived {
                label: Arc::from(private_label),
            },
            context,
        );
        relation
            .add_term(
                context,
                IndexShift::try_new(first, context.index_count()).unwrap(),
                coefficient,
            )
            .unwrap();
        if let Some(second) = second {
            relation
                .add_term(
                    context,
                    IndexShift::try_new(second, context.index_count()).unwrap(),
                    context.one(),
                )
                .unwrap();
        }
        let d = context
            .lift(&context.base().parameter("d").unwrap())
            .unwrap();
        let guard = context
            .nonzero_condition(
                context
                    .numerator_condition(&context.add(&d, &n0).unwrap())
                    .unwrap(),
                GuardOrigin::GuardedDivisionDivisorNumerator,
            )
            .unwrap();
        relation
            .add_guarded_nonzero_condition(context, guard)
            .unwrap();
        relation
    }

    fn pending(
        fixture: &Fixture,
        source_case: usize,
        relation: &ParametricRelation,
    ) -> GeneratedAffineResidualGroupExactRelationCandidate {
        let unresolved = vec![true; fixture.plan.targets().len()];
        match compile_synthetic_for_test(
            &fixture.family,
            &fixture.context,
            authority_for_case(fixture, source_case),
            relation,
            Arc::clone(&fixture.frame),
            Arc::clone(&fixture.plan),
            &unresolved,
            17,
            23,
            GeneratedAffineResidualGroupExactRelationLimits::default(),
        )
        .unwrap()
        {
            GeneratedAffineResidualGroupExactRelationOutcome::Pending(candidate) => candidate,
            GeneratedAffineResidualGroupExactRelationOutcome::NoTarget(value) => {
                panic!("expected pending exact relation, got {value:?}")
            }
        }
    }

    fn exact_to_i64(value: &Integer) -> i64 {
        match value {
            Integer::Single(value) => *value,
            Integer::Double(value) => i64::try_from(*value).unwrap(),
            Integer::Large(value) => value.to_i64().unwrap(),
        }
    }

    #[test]
    fn natural_011_case_recenters_exactly_and_matches_legacy_i64_differential() {
        let fixture = fixture("exact-relation-natural-private");
        let q = [7, M - 1, M - 1];
        let q_second = [7, M - 2, M - 1];
        let relation = source_relation(&fixture, q, Some(q_second), "natural-row-private");
        let candidate = pending(&fixture, 1, &relation);

        assert_eq!(candidate.target_case_ordinal(), 3);
        assert_eq!(
            candidate.pivot.values(),
            [Integer::from(7), Integer::from(M - 1), Integer::from(M - 1)]
        );
        assert_eq!(
            candidate.coefficient_translation.as_slice(),
            [Integer::from(-7), Integer::from(0), Integer::from(0)]
        );
        let mut expected_normalized_terms = 0usize;
        let mut expected_retained_output_bytes = 0usize;
        for coefficient in relation.terms().values() {
            let preflight = fixture
                .context
                .preflight_translate_coefficient_exact(
                    coefficient,
                    candidate.coefficient_translation.as_slice(),
                    candidate.limits.arithmetic,
                )
                .unwrap();
            expected_normalized_terms = expected_normalized_terms
                .checked_add(preflight.normalized_coefficient_term_bound())
                .unwrap();
            expected_retained_output_bytes = expected_retained_output_bytes
                .checked_add(preflight.normalized_coefficient_byte_bound())
                .unwrap();
        }
        for guard in relation.guarded_nonzero_conditions() {
            let preflight = fixture
                .context
                .preflight_translate_polynomial_exact(
                    guard.polynomial(),
                    candidate.coefficient_translation.as_slice(),
                    candidate.limits.arithmetic,
                )
                .unwrap();
            expected_normalized_terms = expected_normalized_terms
                .checked_add(preflight.retained_output_term_bound())
                .unwrap();
            expected_retained_output_bytes = expected_retained_output_bytes
                .checked_add(preflight.retained_output_byte_bound())
                .unwrap();
        }
        assert_eq!(
            candidate.stats().translation_normalized_terms(),
            expected_normalized_terms,
            "the repeated allocation-free preflight must not duplicate retained terms"
        );
        assert_eq!(
            candidate.stats().translation_retained_output_bytes(),
            expected_retained_output_bytes,
            "the repeated allocation-free preflight must not duplicate retained bytes"
        );
        let centered = candidate
            .terms
            .iter()
            .map(|term| {
                term.shift()
                    .values()
                    .iter()
                    .map(exact_to_i64)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert!(centered.contains(&vec![0, 0, 0]));
        assert!(centered.contains(&vec![0, -1, 0]));

        let (legacy, _) = relation
            .affine_free_recentered(
                &fixture.context,
                &IndexShift::try_new([-7, 0, 0], 3).unwrap(),
                &IndexShift::try_new(q, 3).unwrap(),
                ParametricRowId::Derived {
                    label: Arc::from("legacy-differential-target"),
                },
                ParametricAffineFreeRecenteringLimits::default(),
            )
            .unwrap();
        let exact_terms = candidate
            .terms
            .iter()
            .map(|term| {
                (
                    term.shift()
                        .values()
                        .iter()
                        .map(exact_to_i64)
                        .collect::<Vec<_>>(),
                    term.coefficient().clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let legacy_terms = legacy
            .terms()
            .iter()
            .map(|(shift, coefficient)| (shift.values().to_vec(), coefficient.clone()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(exact_terms, legacy_terms);
        assert_eq!(candidate.guards.len(), 1);
        assert_eq!(
            candidate.guards[0].polynomial(),
            legacy.guarded_nonzero_conditions()[0].polynomial()
        );
        let source_origins = relation.guarded_nonzero_conditions()[0].origins();
        assert_eq!(
            candidate.guards[0].origins().len(),
            source_origins.len() + 1
        );
        assert!(source_origins.is_subset(candidate.guards[0].origins()));
        assert!(candidate.guards[0].origins().contains(
            &GuardOrigin::GeneratedAffineGroupRecentering {
                solve_group_ordinal: fixture.plan.group_ordinal(),
                database_epoch: 17,
                event_ordinal: 23,
            }
        ));
        assert!(
            !candidate.guards[0]
                .origins()
                .iter()
                .any(|origin| matches!(origin, GuardOrigin::IndexTranslation { .. }))
        );
    }

    #[test]
    fn boundary_case_three_uses_positive_two_to_63_delta_and_selects_case_one() {
        let fixture = fixture("exact-relation-boundary-private");
        let q = [i64::MIN, -(M - 1), -(M - 1)];
        let relation = source_relation(&fixture, q, None, "boundary-row-private");
        let candidate = pending(&fixture, 3, &relation);
        let two_to_63 = Integer::from(1_i128 << 63);

        assert_eq!(candidate.target_case_ordinal(), 1);
        assert_eq!(
            candidate.pivot.values(),
            [Integer::from(i64::MIN), Integer::from(0), Integer::from(0)]
        );
        assert_eq!(
            candidate.coefficient_translation.as_slice(),
            [two_to_63.clone(), Integer::from(0), Integer::from(0)]
        );
        assert!(candidate.terms.iter().all(|term| {
            term.shift()
                .values()
                .iter()
                .all(|value| value == &Integer::from(0))
        }));
        assert!(i64::MIN.checked_neg().is_none(), "legacy i64 delta rejects");

        let expected_delta = fixture
            .context
            .lift(&fixture.context.base().parse("9223372036854775808").unwrap())
            .unwrap();
        let n0 = fixture.context.index(0).unwrap();
        let n1 = fixture.context.index(1).unwrap();
        let expected = fixture
            .context
            .add(
                &fixture.context.add(&n0, &expected_delta).unwrap(),
                &fixture
                    .context
                    .mul(&fixture.context.integer(2), &n1)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(candidate.terms[0].coefficient(), &expected);
    }

    #[test]
    fn cancelling_large_target_offset_charges_live_gmp_product_and_accumulators() {
        let fixture = fixture("exact-relation-gmp-cancellation-private");
        let huge = Integer::from(1) << 4096_u32;
        let pivot_key = fixture
            .frame
            .test_key_for_borrowed_physical_values(&[huge, Integer::from(0), Integer::from(0)])
            .unwrap();
        let pivot = pivot_key.shift().clone();
        assert!(matches!(pivot.values()[0], Integer::Large(_)));
        let group = fixture
            .plan
            .authority()
            .authenticated_group_view(&fixture.context)
            .unwrap();
        let matrix = group.compact_linear_coefficients();
        let free_positions = group.free_positions();
        let limits = GeneratedAffineResidualGroupExactRelationLimits::default();
        let exact_limits = kernel_limits(limits);
        let mut stats = ExactRecenterKernelStats::for_row(0, 0, exact_limits).unwrap();
        preflight_exact_geometry(&pivot, matrix, free_positions, exact_limits, &mut stats).unwrap();
        let huge_bits = integer_bits(&pivot.values()[0]).unwrap();
        let expected_bit_work = huge_bits
            .checked_mul(19)
            .and_then(|work| work.checked_add(25))
            .unwrap();
        let formerly_undercharged_bit_work = huge_bits
            .checked_mul(10)
            .and_then(|work| work.checked_add(13))
            .unwrap();
        assert_eq!(
            stats.geometry_integer_bit_work(),
            expected_bit_work,
            "three rows each charge multiplication operands/result, accumulator-addition operands/result, and final-subtraction operands/result"
        );
        assert!(expected_bit_work > formerly_undercharged_bit_work);
        assert_eq!(stats.geometry_integer_operations(), 9);
        let offset = execute_target_offset(&pivot, matrix, free_positions, pivot.arity()).unwrap();
        assert!(
            offset
                .values()
                .iter()
                .all(|value| value == &Integer::from(0))
        );

        // Reconstruct the previous final-result-only envelope exactly.  It
        // admitted only the output Vec plus prospective result components,
        // omitting the simultaneously live product and accumulators.
        let mut old_final_only_bound = vec_retained_bytes_bound::<Integer>(pivot.arity()).unwrap();
        for row in 0..pivot.arity() {
            let mut sum_bits = 0usize;
            for (free_ordinal, &free_position) in free_positions.iter().enumerate() {
                let matrix_bits =
                    integer_bits(&matrix[row * free_positions.len() + free_ordinal]).unwrap();
                let pivot_bits = integer_bits(&pivot.values()[free_position]).unwrap();
                let product_bits = matrix_bits.checked_add(pivot_bits).unwrap();
                sum_bits = sum_bits.max(product_bits).checked_add(1).unwrap();
            }
            let target_bits = integer_bits(&pivot.values()[row])
                .unwrap()
                .max(sum_bits)
                .checked_add(1)
                .unwrap();
            old_final_only_bound = old_final_only_bound
                .checked_add(prospective_integer_heap_bytes(target_bits).unwrap())
                .unwrap();
        }
        assert!(stats.target_offset_temporary_bytes() > old_final_only_bound);

        let exact_demand = stats.target_offset_temporary_bytes();
        let mut exact_demand_limits = exact_limits;
        exact_demand_limits.max_target_offset_temporary_bytes = exact_demand;
        let mut exact_stats = ExactRecenterKernelStats::for_row(0, 0, exact_demand_limits).unwrap();
        preflight_exact_geometry(
            &pivot,
            matrix,
            free_positions,
            exact_demand_limits,
            &mut exact_stats,
        )
        .unwrap();
        assert_eq!(exact_stats.target_offset_temporary_bytes(), exact_demand);

        assert_eq!(
            native_exact_scratch_bytes(&stats, 0, false).unwrap(),
            exact_demand,
            "the revised target envelope must enter native scratch exactly once"
        );

        let mut formerly_admitted = exact_limits;
        formerly_admitted.max_target_offset_temporary_bytes = old_final_only_bound;
        let mut rejected_stats =
            ExactRecenterKernelStats::for_row(0, 0, formerly_admitted).unwrap();
        assert!(matches!(
            preflight_exact_geometry(
                &pivot,
                matrix,
                free_positions,
                formerly_admitted,
                &mut rejected_stats,
            ),
            Err(ExactRecenterKernelError::ResourceLimit { .. })
        ));

        let mut one_below = exact_limits;
        one_below.max_target_offset_temporary_bytes = exact_demand - 1;
        let mut one_below_stats = ExactRecenterKernelStats::for_row(0, 0, one_below).unwrap();
        assert!(matches!(
            preflight_exact_geometry(
                &pivot,
                matrix,
                free_positions,
                one_below,
                &mut one_below_stats,
            ),
            Err(ExactRecenterKernelError::ResourceLimit { .. })
        ));

        let mut exact_work = exact_limits;
        exact_work.max_geometry_integer_bit_work = expected_bit_work;
        let mut exact_work_stats = ExactRecenterKernelStats::for_row(0, 0, exact_work).unwrap();
        preflight_exact_geometry(
            &pivot,
            matrix,
            free_positions,
            exact_work,
            &mut exact_work_stats,
        )
        .unwrap();
        assert_eq!(
            exact_work_stats.geometry_integer_bit_work(),
            expected_bit_work
        );

        let mut work_one_below = exact_limits;
        work_one_below.max_geometry_integer_bit_work = expected_bit_work - 1;
        let mut work_one_below_stats =
            ExactRecenterKernelStats::for_row(0, 0, work_one_below).unwrap();
        reset_target_offset_arithmetic_entries_for_test();
        assert!(matches!(
            preflight_exact_geometry(
                &pivot,
                matrix,
                free_positions,
                work_one_below,
                &mut work_one_below_stats,
            ),
            Err(ExactRecenterKernelError::ResourceLimit {
                resource: "exact recentering geometry integer-bit work",
                requested,
                limit,
            }) if requested == expected_bit_work && limit + 1 == expected_bit_work
        ));
        assert_eq!(
            target_offset_arithmetic_entries_for_test(),
            0,
            "a rejected geometry admission must not enter target-offset GMP arithmetic"
        );
    }

    #[test]
    fn legacy_adapter_rejects_geometry_work_one_below_before_target_offset_arithmetic() {
        let fixture = fixture("exact-relation-work-admission-private");
        let relation = source_relation(
            &fixture,
            [7, M - 1, M - 1],
            None,
            "work-admission-row-private",
        );
        let candidate = pending(&fixture, 1, &relation);
        let pivot = candidate.pivot.clone();
        let group = fixture
            .plan
            .authority()
            .authenticated_group_view(&fixture.context)
            .unwrap();
        let matrix = group.compact_linear_coefficients();
        let free_positions = group.free_positions();
        let mut limits = GeneratedAffineResidualGroupExactRelationLimits::default();
        let exact_limits = kernel_limits(limits);
        let mut stats = ExactRecenterKernelStats::for_row(0, 0, exact_limits).unwrap();
        preflight_exact_geometry(&pivot, matrix, free_positions, exact_limits, &mut stats).unwrap();
        let exact_work = stats.geometry_integer_bit_work();
        assert!(exact_work > 0);
        limits.max_geometry_integer_bit_work = exact_work - 1;

        let unresolved = vec![true; fixture.plan.targets().len()];
        let unchanged_unresolved = unresolved.clone();
        reset_target_offset_arithmetic_entries_for_test();
        assert!(matches!(
            compile_synthetic_for_test(
                &fixture.family,
                &fixture.context,
                authority_for_case(&fixture, 1),
                &relation,
                Arc::clone(&fixture.frame),
                Arc::clone(&fixture.plan),
                &unresolved,
                17,
                23,
                limits,
            ),
            Err(GeneratedAffineResidualGroupExactRelationError::ResourceLimit {
                resource: "exact recentering geometry integer-bit work",
                requested,
                limit,
            }) if requested == exact_work && limit + 1 == exact_work
        ));
        assert_eq!(unresolved, unchanged_unresolved);
        assert_eq!(
            target_offset_arithmetic_entries_for_test(),
            0,
            "the adapter must return the one-below error before target-offset GMP arithmetic"
        );
    }

    #[test]
    fn centered_subtractions_have_exact_limits_and_replay_does_not_double_charge() {
        let fixture = fixture("exact-relation-centered-work-private");
        let relation = source_relation(
            &fixture,
            [7, M - 1, M - 1],
            Some([7, M - 2, M - 1]),
            "centered-work-row-private",
        );
        let baseline = pending(&fixture, 1, &relation);

        // This fixture has nine target-offset operations (324 bit-work),
        // five target comparisons (191), one free-coordinate negation (3),
        // and six centered subtractions (780).  The last contribution is two
        // rows of [3+3+4, 63+63+64, 63+63+64].
        const EXPECTED_OPERATIONS: usize = 21;
        const EXPECTED_INTEGER_BIT_WORK: usize = 1_298;
        const EXPECTED_CENTERED_SUBTRACTIONS: usize = 6;
        assert_eq!(baseline.stats().target_scans(), 2);
        assert_eq!(
            baseline.stats().geometry_integer_operations(),
            EXPECTED_OPERATIONS
        );
        assert_eq!(
            baseline.stats().geometry_integer_bit_work(),
            EXPECTED_INTEGER_BIT_WORK
        );

        let unresolved = vec![true; fixture.plan.targets().len()];
        let unchanged_unresolved = unresolved.clone();
        let mut exact = GeneratedAffineResidualGroupExactRelationLimits::default();
        exact.max_geometry_integer_operations = EXPECTED_OPERATIONS;
        exact.max_geometry_integer_bit_work = EXPECTED_INTEGER_BIT_WORK;
        reset_centered_shift_arithmetic_operations_for_test();
        let exact_outcome = compile_synthetic_for_test(
            &fixture.family,
            &fixture.context,
            authority_for_case(&fixture, 1),
            &relation,
            Arc::clone(&fixture.frame),
            Arc::clone(&fixture.plan),
            &unresolved,
            17,
            23,
            exact,
        )
        .unwrap();
        let GeneratedAffineResidualGroupExactRelationOutcome::Pending(exact_candidate) =
            exact_outcome
        else {
            panic!("exact centered arithmetic limits must retain the pending outcome")
        };
        assert_eq!(
            exact_candidate.stats().geometry_integer_operations(),
            EXPECTED_OPERATIONS,
            "the isolated execution replay must not charge caller operations twice"
        );
        assert_eq!(
            exact_candidate.stats().geometry_integer_bit_work(),
            EXPECTED_INTEGER_BIT_WORK,
            "the isolated execution replay must not charge caller bit-work twice"
        );
        assert_eq!(
            centered_shift_arithmetic_operations_for_test(),
            EXPECTED_CENTERED_SUBTRACTIONS
        );

        let mut work_one_below = exact;
        work_one_below.max_geometry_integer_bit_work = EXPECTED_INTEGER_BIT_WORK - 1;
        reset_centered_shift_arithmetic_operations_for_test();
        assert!(matches!(
            compile_synthetic_for_test(
                &fixture.family,
                &fixture.context,
                authority_for_case(&fixture, 1),
                &relation,
                Arc::clone(&fixture.frame),
                Arc::clone(&fixture.plan),
                &unresolved,
                17,
                23,
                work_one_below,
            ),
            Err(GeneratedAffineResidualGroupExactRelationError::ResourceLimit {
                resource: "exact recentering geometry integer-bit work",
                requested: EXPECTED_INTEGER_BIT_WORK,
                limit,
            }) if limit + 1 == EXPECTED_INTEGER_BIT_WORK
        ));
        assert_eq!(unresolved, unchanged_unresolved);
        assert_eq!(
            centered_shift_arithmetic_operations_for_test(),
            0,
            "one-below bit-work admission must reject before centered GMP subtraction"
        );

        let mut operations_one_below = exact;
        operations_one_below.max_geometry_integer_operations = EXPECTED_OPERATIONS - 1;
        reset_centered_shift_arithmetic_operations_for_test();
        assert!(matches!(
            compile_synthetic_for_test(
                &fixture.family,
                &fixture.context,
                authority_for_case(&fixture, 1),
                &relation,
                Arc::clone(&fixture.frame),
                Arc::clone(&fixture.plan),
                &unresolved,
                17,
                23,
                operations_one_below,
            ),
            Err(GeneratedAffineResidualGroupExactRelationError::ResourceLimit {
                resource: "exact recentering geometry integer operations",
                requested: EXPECTED_OPERATIONS,
                limit,
            }) if limit + 1 == EXPECTED_OPERATIONS
        ));
        assert_eq!(unresolved, unchanged_unresolved);
        assert_eq!(
            centered_shift_arithmetic_operations_for_test(),
            0,
            "one-below operation admission must reject before centered GMP subtraction"
        );
    }

    #[test]
    fn centered_admission_rejects_same_shape_low_bit_caller_stats_before_arithmetic() {
        let fixture = fixture("exact-relation-centered-binding-private");
        let huge = Integer::from(1) << 4096_u32;
        let high_pivot_key = fixture
            .frame
            .test_key_for_borrowed_physical_values(&[
                huge.clone(),
                Integer::from(0),
                Integer::from(0),
            ])
            .unwrap();
        let high_term_key = fixture
            .frame
            .test_key_for_borrowed_physical_values(&[
                &huge + Integer::from(1),
                Integer::from(1),
                Integer::from(0),
            ])
            .unwrap();
        let low_pivot_key = fixture
            .frame
            .test_key_for_borrowed_physical_values(&[
                Integer::from(1),
                Integer::from(0),
                Integer::from(0),
            ])
            .unwrap();
        let low_term_key = fixture
            .frame
            .test_key_for_borrowed_physical_values(&[
                Integer::from(2),
                Integer::from(1),
                Integer::from(0),
            ])
            .unwrap();
        let coefficient = fixture.context.one();
        let high_terms = [(high_term_key.shift(), &coefficient)];
        let low_terms = [(low_term_key.shift(), &coefficient)];
        let limits = kernel_limits(GeneratedAffineResidualGroupExactRelationLimits::default());
        let mut high_stats = ExactRecenterKernelStats::default();
        let high_admission =
            preflight_centered_shifts(&high_terms, high_pivot_key.shift(), limits, &mut high_stats)
                .unwrap();
        let mut low_stats = ExactRecenterKernelStats::default();
        let low_admission =
            preflight_centered_shifts(&low_terms, low_pivot_key.shift(), limits, &mut low_stats)
                .unwrap();

        assert_eq!(high_admission.shift_count(), low_admission.shift_count());
        assert_eq!(high_admission.components(), low_admission.components());
        assert_eq!(
            high_stats.centered_shift_outer_buffer_bytes(),
            low_stats.centered_shift_outer_buffer_bytes()
        );
        assert!(
            high_admission.prospective_integer_bits() > low_admission.prospective_integer_bits()
        );
        assert!(
            high_admission.prospective_retained_bytes()
                > low_admission.prospective_retained_bytes()
        );
        assert!(high_stats.geometry_integer_bit_work() > low_stats.geometry_integer_bit_work());

        let high_pivot_before = high_pivot_key.shift().values().to_vec();
        let high_term_before = high_term_key.shift().values().to_vec();
        let low_stats_before = low_stats;
        reset_centered_shift_arithmetic_operations_for_test();
        assert!(matches!(
            execute_centered_shifts(
                &high_terms,
                high_pivot_key.shift(),
                high_admission,
                limits,
                &mut low_stats,
            ),
            Err(ExactRecenterKernelError::CensusMismatch)
        ));
        assert_eq!(centered_shift_arithmetic_operations_for_test(), 0);
        assert_eq!(low_stats, low_stats_before);
        assert_eq!(high_pivot_key.shift().values(), high_pivot_before);
        assert_eq!(high_term_key.shift().values(), high_term_before);
    }

    #[test]
    fn centered_outer_and_borrowed_reference_buffers_have_exact_one_below_limits() {
        let fixture = fixture("exact-relation-buffer-envelopes-private");
        let relation = source_relation(
            &fixture,
            [7, M - 1, M - 1],
            Some([7, M - 2, M - 1]),
            "buffer-envelope-row-private",
        );
        let baseline = pending(&fixture, 1, &relation);
        let centered_demand = baseline.stats().centered_shift_outer_buffer_bytes();
        let reference_demand = baseline.stats().borrowed_reference_buffer_bytes();
        assert!(centered_demand > size_of::<Vec<ExactCenteredShift>>());
        assert!(reference_demand > size_of::<Vec<ExactBorrowedTerm<'_>>>());

        let unresolved = vec![true; fixture.plan.targets().len()];
        let mut exact = GeneratedAffineResidualGroupExactRelationLimits::default();
        exact.max_centered_shift_outer_buffer_bytes = centered_demand;
        exact.max_borrowed_reference_buffer_bytes = reference_demand;
        let exact_outcome = compile_synthetic_for_test(
            &fixture.family,
            &fixture.context,
            authority_for_case(&fixture, 1),
            &relation,
            Arc::clone(&fixture.frame),
            Arc::clone(&fixture.plan),
            &unresolved,
            17,
            23,
            exact,
        )
        .unwrap();
        let GeneratedAffineResidualGroupExactRelationOutcome::Pending(exact_candidate) =
            exact_outcome
        else {
            panic!("exact buffer limits must retain the pending outcome")
        };
        assert_eq!(
            exact_candidate.stats().centered_shift_outer_buffer_bytes(),
            centered_demand
        );
        assert_eq!(
            exact_candidate.stats().borrowed_reference_buffer_bytes(),
            reference_demand
        );

        let mut centered_one_below = exact;
        centered_one_below.max_centered_shift_outer_buffer_bytes = centered_demand - 1;
        assert!(matches!(
            compile_synthetic_for_test(
                &fixture.family,
                &fixture.context,
                authority_for_case(&fixture, 1),
                &relation,
                Arc::clone(&fixture.frame),
                Arc::clone(&fixture.plan),
                &unresolved,
                17,
                23,
                centered_one_below,
            ),
            Err(GeneratedAffineResidualGroupExactRelationError::ResourceLimit {
                resource: "exact recentering centered-shift outer buffer bytes",
                requested,
                limit,
            }) if requested == centered_demand && limit + 1 == centered_demand
        ));

        let mut reference_one_below = exact;
        reference_one_below.max_borrowed_reference_buffer_bytes = reference_demand - 1;
        assert!(matches!(
            compile_synthetic_for_test(
                &fixture.family,
                &fixture.context,
                authority_for_case(&fixture, 1),
                &relation,
                Arc::clone(&fixture.frame),
                Arc::clone(&fixture.plan),
                &unresolved,
                17,
                23,
                reference_one_below,
            ),
            Err(GeneratedAffineResidualGroupExactRelationError::ResourceLimit {
                resource: "exact recentering borrowed-reference buffer bytes",
                requested,
                limit,
            }) if requested == reference_demand && limit + 1 == reference_demand
        ));
    }

    #[test]
    fn physical_offset_beyond_i64_is_an_inert_no_target_outcome() {
        let fixture = fixture("exact-relation-wide-no-target-private");
        let relation = source_relation(&fixture, [0, 2, 2], None, "wide-row-private");
        let unresolved = vec![true; fixture.plan.targets().len()];
        let outcome = compile_synthetic_for_test(
            &fixture.family,
            &fixture.context,
            authority_for_case(&fixture, 3),
            &relation,
            Arc::clone(&fixture.frame),
            Arc::clone(&fixture.plan),
            &unresolved,
            0,
            0,
            GeneratedAffineResidualGroupExactRelationLimits::default(),
        )
        .unwrap();
        let GeneratedAffineResidualGroupExactRelationOutcome::NoTarget(no_target) = outcome else {
            panic!("a physical offset above i64 must not fabricate a target")
        };
        assert_eq!(
            no_target.stats.owner_retained_bytes(),
            size_of_val(&no_target)
        );
        assert!(
            no_target.stats.native_temporary_byte_envelope()
                > no_target.stats.owner_retained_bytes()
        );
        assert_eq!(
            GeneratedAffineResidualGroupExactRelationOutcome::NoTarget(no_target)
                .targets_consumed(),
            0
        );
    }

    #[test]
    fn exact_frame_allocation_is_mandatory_even_for_value_equal_clone() {
        let fixture = fixture("exact-relation-cloned-frame-private");
        let relation = source_relation(&fixture, [7, M - 1, M - 1], None, "clone-row-private");
        let cloned_frame = Arc::new(fixture.frame.as_ref().clone());
        assert!(!Arc::ptr_eq(&fixture.frame, &cloned_frame));
        let unresolved = vec![true; fixture.plan.targets().len()];
        assert!(matches!(
            compile_synthetic_for_test(
                &fixture.family,
                &fixture.context,
                authority_for_case(&fixture, 1),
                &relation,
                cloned_frame,
                Arc::clone(&fixture.plan),
                &unresolved,
                0,
                0,
                GeneratedAffineResidualGroupExactRelationLimits::default(),
            ),
            Err(GeneratedAffineResidualGroupExactRelationError::WrongParentAllocation)
        ));
    }

    #[test]
    fn candidate_and_errors_redact_private_geometry_and_symbolic_payloads() {
        let private_name = "exact-relation-redaction-family-private";
        let private_label = "exact-relation-redaction-row-private";
        let fixture = fixture(private_name);
        let relation = source_relation(&fixture, [7, M - 1, M - 1], None, private_label);
        let candidate = pending(&fixture, 1, &relation);
        let rendered = format!("{candidate:?}");
        for secret in [private_name, private_label, "m2", "9223372036854775806"] {
            assert!(!rendered.contains(secret));
        }
        assert!(rendered.contains("<redacted>"));
        assert!(!candidate.is_applicable_rule());
        assert_eq!(candidate.targets_consumed(), 0);
        assert!(!candidate.infers_master());

        let error = GeneratedAffineResidualGroupExactRelationError::ResourceLimit {
            resource: private_label,
            requested: usize::MAX,
            limit: 0,
        };
        let debug = format!("{error:?}");
        let display = error.to_string();
        assert!(!debug.contains(private_label));
        assert!(!debug.contains(&usize::MAX.to_string()));
        assert!(!display.contains(private_label));
        assert!(debug.contains("<redacted>"));
    }

    #[test]
    fn production_ingress_authenticates_exact_reelimination_witness_and_retained_row() {
        let fixture = fixture("exact-relation-production-ingress-private");
        for case_ordinal in [1, 3] {
            let authority = authority_for_case(&fixture, case_ordinal);
            let premises = match compile_generated_affine_residual_case_premises(
                &fixture.family,
                &fixture.context,
                Arc::clone(&authority),
                GeneratedAffineResidualCasePremisesLimits::default(),
            )
            .unwrap()
            {
                GeneratedAffineResidualCasePremisesOutcome::Ready(certificate) => {
                    Arc::new(certificate)
                }
                GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(_) => {
                    continue;
                }
            };
            let ordering = Arc::new(
                GeneratedAffineParametricOrderingCertificate::try_new(
                    &fixture.family,
                    &fixture.context,
                    Arc::clone(&authority),
                    GeneratedAffineParametricOrderingLimits::default(),
                )
                .unwrap(),
            );
            let schedule = Arc::new(
                GeneratedAffinePreparePointScheduleCertificate::compile(
                    &fixture.family,
                    &fixture.context,
                    Arc::clone(&ordering),
                    &authority,
                    0,
                    GeneratedAffinePreparePointScheduleLimits::default(),
                )
                .unwrap(),
            );
            let compilation = GeneratedAffineResidualCaseReeliminationCompiler::compile(
                &fixture.family,
                &fixture.context,
                Arc::clone(&authority),
                premises,
                ordering,
                schedule,
                GeneratedAffineResidualCaseReeliminationLimits::default(),
            )
            .unwrap();
            let GeneratedAffineResidualCaseReeliminationCompilation::Eliminated(certificate) =
                compilation
            else {
                continue;
            };
            let certificate = Arc::new(certificate);
            let Some(witness_ordinal) = certificate
                .witnesses()
                .iter()
                .position(|witness| witness.outcome().is_retained())
            else {
                continue;
            };
            let retained_row_ordinal = certificate.witnesses()[..witness_ordinal]
                .iter()
                .filter(|witness| witness.outcome().is_retained())
                .count();
            let witness = &certificate.witnesses()[witness_ordinal];
            let authenticated = certificate
                .authenticate_retained_source_row(retained_row_ordinal, witness_ordinal)
                .unwrap();
            let retained = authenticated.relation();
            assert_eq!(witness.expanded_ordinal(), witness_ordinal);
            assert!(
                witness
                    .retained_support_shifts()
                    .unwrap()
                    .iter()
                    .eq(retained.terms().keys())
            );

            let unresolved = vec![true; fixture.plan.targets().len()];
            let mut witness_starved = GeneratedAffineResidualGroupExactRelationLimits::default();
            witness_starved.max_witnesses = certificate.witnesses().len() - 1;
            assert!(matches!(
                GeneratedAffineResidualGroupExactRelationCompiler::compile(
                    &fixture.family,
                    &fixture.context,
                    Arc::clone(&certificate),
                    retained_row_ordinal,
                    witness_ordinal,
                    Arc::clone(&fixture.frame),
                    Arc::clone(&fixture.plan),
                    &unresolved,
                    101,
                    103,
                    witness_starved,
                ),
                Err(GeneratedAffineResidualGroupExactRelationError::ResourceLimit {
                    resource: "exact recentering witnesses",
                    requested,
                    limit,
                }) if requested == certificate.witnesses().len() && limit + 1 == requested
            ));
            let outcome = GeneratedAffineResidualGroupExactRelationCompiler::compile(
                &fixture.family,
                &fixture.context,
                Arc::clone(&certificate),
                retained_row_ordinal,
                witness_ordinal,
                Arc::clone(&fixture.frame),
                Arc::clone(&fixture.plan),
                &unresolved,
                101,
                103,
                GeneratedAffineResidualGroupExactRelationLimits::default(),
            )
            .unwrap();
            assert_eq!(outcome.targets_consumed(), 0);
            assert!(!outcome.publishes_rule());
            assert!(!outcome.infers_master());
            assert_eq!(case_ordinal, 1);
            let GeneratedAffineResidualGroupExactRelationOutcome::NoTarget(no_target) = outcome
            else {
                panic!("the authenticated natural raw row must remain an inert NoTarget")
            };
            assert!(matches!(
                &no_target.source,
                ExactSourceBinding::Production(source) if Arc::ptr_eq(source, &certificate)
            ));
            assert!(Arc::ptr_eq(&no_target.frame, &fixture.frame));
            assert!(Arc::ptr_eq(&no_target.plan, &fixture.plan));
            return;
        }
        panic!("natural group produced no retained certificate row for production ingress");
    }
}
