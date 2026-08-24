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
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use symbolica::prelude::Integer;

use crate::affine_parametric_ordering::integer_magnitude_bits;
#[cfg(test)]
use crate::generated_affine_residual_case_inventory::GeneratedAffineResidualCaseAuthority;
use crate::generated_affine_residual_case_reelimination::{
    GeneratedAffineResidualCaseReeliminationCertificate,
    GeneratedAffineResidualCaseReeliminationError,
};
use crate::generated_affine_residual_group_physical_key::{
    GeneratedAffineResidualGroupLatticeShift, GeneratedAffineResidualGroupPhysicalFrame,
    GeneratedAffineResidualGroupPhysicalKey, GeneratedAffineResidualGroupPhysicalKeyError,
};
use crate::generated_affine_residual_group_solve_plan::{
    GeneratedAffineResidualGroupSolvePlan, GeneratedAffineResidualGroupSolvePlanReplayLimits,
    GeneratedAffineResidualGroupSolveTargetLocator,
};
use crate::parametric_coefficient::{
    ParametricCoefficientTranslationPreflight, ParametricPolynomialTranslationPreflight,
};
use crate::{
    GuardOrigin, IntegralFamily, ParametricArithmeticLimits, ParametricCoefficient,
    ParametricCoefficientContext, ParametricCoefficientError, ParametricNonZeroCondition,
    ParametricRelation,
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

#[derive(Clone)]
struct ExactCenteredShift {
    values: Arc<Vec<Integer>>,
    retained_integer_bits: usize,
    retained_bytes: usize,
}

impl PartialEq for ExactCenteredShift {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
    }
}
impl Eq for ExactCenteredShift {}
impl Ord for ExactCenteredShift {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.values.cmp(&other.values)
    }
}
impl PartialOrd for ExactCenteredShift {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Debug for ExactCenteredShift {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactCenteredShift")
            .field("arity", &self.values.len())
            .field("retained_integer_bits", &self.retained_integer_bits)
            .field("private_values", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
struct ExactRelationTerm {
    shift: ExactCenteredShift,
    coefficient: ParametricCoefficient,
}

impl fmt::Debug for ExactRelationTerm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactRelationTerm")
            .field("shift", &self.shift)
            .field("private_coefficient", &"<redacted>")
            .finish()
    }
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
    terms: Arc<Vec<ExactRelationTerm>>,
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

    preflight_exact_geometry(&pivot, matrix, free_positions, limits, &mut stats)?;
    let target_offset = execute_target_offset(&pivot, matrix, free_positions, arity)?;
    verify_target_offset_census(&target_offset, &stats)?;
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
                target_offset.as_slice(),
                limits,
                &mut stats,
            )?
        {
            selected = Some(*locator);
            break;
        }
    }
    let Some(target) = selected else {
        admit_no_target(&mut stats, limits)?;
        return Ok(GeneratedAffineResidualGroupExactRelationOutcome::NoTarget(
            GeneratedAffineResidualGroupExactRelationNoTarget {
                source: source_binding,
                frame,
                plan,
                stats,
            },
        ));
    };

    preflight_coefficient_translation(&pivot, free_positions, arity, limits, &mut stats)?;
    let coefficient_translation = coefficient_translation(&pivot, free_positions, arity)?;
    verify_coefficient_translation_census(&coefficient_translation, &stats)?;
    let centered_shifts = preflight_and_center_keys(&physical_keys, &pivot, limits, &mut stats)?;
    let locator_origin = GuardOrigin::GeneratedAffineGroupRecentering {
        solve_group_ordinal: plan.group_ordinal(),
        database_epoch,
        event_ordinal,
    };
    let translation_admission = preflight_translations(
        context,
        relation,
        coefficient_translation.as_slice(),
        pivot.retained_bytes(),
        &locator_origin,
        limits,
        &mut stats,
    )?;

    let mut terms = try_vec("exact recentering output terms", stats.terms)?;
    let mut guards = try_vec("exact recentering output guards", stats.guards)?;
    for (coefficient, shift) in relation.terms().values().zip(centered_shifts.into_iter()) {
        terms.push(ExactRelationTerm {
            shift,
            coefficient: context.translate_exact(
                coefficient,
                coefficient_translation.as_slice(),
                limits.arithmetic,
            )?,
        });
    }
    for condition in relation.guarded_nonzero_conditions() {
        let polynomial = context.translate_polynomial_exact(
            condition.polynomial(),
            coefficient_translation.as_slice(),
            limits.arithmetic,
        )?;
        let origins = condition
            .origins()
            .iter()
            .cloned()
            .chain(std::iter::once(locator_origin.clone()));
        guards.push(context.nonzero_condition_with_origins_and_origin_limit(
            polynomial,
            origins,
            limits.arithmetic.exact_algebra,
            limits.arithmetic.max_guard_origins,
        )?);
    }
    let owner_retained_bytes =
        observed_output_bytes(&terms, &guards, &coefficient_translation, &pivot)?;
    check_limit(
        "exact recentering owner retained bytes",
        owner_retained_bytes,
        limits.max_owner_retained_bytes,
    )?;
    if owner_retained_bytes > translation_admission.final_retained_output_bytes {
        return Err(GeneratedAffineResidualGroupExactRelationError::Coefficient);
    }
    stats.owner_retained_bytes = owner_retained_bytes;
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
            coefficient_translation: Arc::new(coefficient_translation),
            terms: Arc::new(terms),
            guards: Arc::new(guards),
            limits,
            stats,
        },
    ))
}

#[derive(Clone, Copy)]
struct TranslationAdmission {
    final_retained_output_bytes: usize,
}

fn preflight_exact_geometry(
    pivot: &GeneratedAffineResidualGroupLatticeShift,
    matrix: &[Integer],
    free_positions: &[usize],
    limits: GeneratedAffineResidualGroupExactRelationLimits,
    stats: &mut GeneratedAffineResidualGroupExactRelationStats,
) -> Result<(), GeneratedAffineResidualGroupExactRelationError> {
    let arity = pivot.arity();
    let operations_per_row = checked_add(
        "exact recentering geometry integer operations",
        checked_mul(
            "exact recentering geometry integer operations",
            free_positions.len(),
            2,
        )?,
        1,
    )?;
    stats.geometry_integer_operations = checked_mul(
        "exact recentering geometry integer operations",
        arity,
        operations_per_row,
    )?;
    check_limit(
        "exact recentering geometry integer operations",
        stats.geometry_integer_operations,
        limits.max_geometry_integer_operations,
    )?;
    let mut bit_work = 0usize;
    let mut target_offset_bits = 0usize;
    let mut target_offset_bytes = vec_retained_bytes_bound::<Integer>(arity)?;
    // `execute_target_offset` evaluates every matrix product before adding it
    // to the live accumulator.  Conservatively admit the product, the old
    // accumulator, and the prospective addition result simultaneously.  The
    // final retained component alone is not a sound temporary bound: a large
    // product and accumulator can cancel when `r - A r_F` is formed.
    let mut target_offset_live_integer_peak = 0usize;
    for row in 0..arity {
        let mut sum_bits = 0usize;
        for (free_ordinal, &free_position) in free_positions.iter().enumerate() {
            let matrix_bits = integer_bits(&matrix[row * free_positions.len() + free_ordinal])?;
            let pivot_bits = integer_bits(&pivot.values()[free_position])?;
            let product_bits =
                checked_add("exact recentering integer bits", matrix_bits, pivot_bits)?;
            check_limit(
                "exact recentering integer bits",
                product_bits,
                limits.max_exact_integer_bits,
            )?;
            let prior_sum_bits = sum_bits;
            let next_sum_bits = checked_add(
                "exact recentering integer bits",
                sum_bits.max(product_bits),
                1,
            )?;
            check_limit(
                "exact recentering integer bits",
                next_sum_bits,
                limits.max_exact_integer_bits,
            )?;
            let live_integer_bytes = checked_add(
                "exact recentering target-offset temporary bytes",
                integer_retained_bytes(product_bits)?,
                checked_add(
                    "exact recentering target-offset temporary bytes",
                    integer_retained_bytes(prior_sum_bits)?,
                    integer_retained_bytes(next_sum_bits)?,
                )?,
            )?;
            target_offset_live_integer_peak =
                target_offset_live_integer_peak.max(live_integer_bytes);
            sum_bits = next_sum_bits;
            bit_work = checked_add(
                "exact recentering geometry integer-bit work",
                bit_work,
                checked_add(
                    "exact recentering geometry integer-bit work",
                    matrix_bits.max(1),
                    checked_add(
                        "exact recentering geometry integer-bit work",
                        pivot_bits.max(1),
                        product_bits.max(1),
                    )?,
                )?,
            )?;
        }
        let row_bits = integer_bits(&pivot.values()[row])?;
        let target_bits = checked_add("exact recentering integer bits", row_bits.max(sum_bits), 1)?;
        check_limit(
            "exact recentering integer bits",
            target_bits,
            limits.max_exact_integer_bits,
        )?;
        bit_work = checked_add(
            "exact recentering geometry integer-bit work",
            bit_work,
            checked_add(
                "exact recentering geometry integer-bit work",
                row_bits.max(1),
                target_bits.max(1),
            )?,
        )?;
        target_offset_bits = checked_add(
            "exact recentering target-offset integer bits",
            target_offset_bits,
            target_bits,
        )?;
        target_offset_bytes = checked_add(
            "exact recentering target-offset temporary bytes",
            target_offset_bytes,
            prospective_integer_heap_bytes(target_bits)?,
        )?;
        let subtraction_live_bytes = checked_add(
            "exact recentering target-offset temporary bytes",
            integer_retained_bytes(sum_bits)?,
            integer_retained_bytes(target_bits)?,
        )?;
        target_offset_live_integer_peak =
            target_offset_live_integer_peak.max(subtraction_live_bytes);
    }
    target_offset_bytes = checked_add(
        "exact recentering target-offset temporary bytes",
        target_offset_bytes,
        target_offset_live_integer_peak,
    )?;
    check_limit(
        "exact recentering geometry integer-bit work",
        bit_work,
        limits.max_geometry_integer_bit_work,
    )?;
    check_limit(
        "exact recentering target-offset integer bits",
        target_offset_bits,
        limits.max_target_offset_integer_bits,
    )?;
    check_limit(
        "exact recentering target-offset temporary bytes",
        target_offset_bytes,
        limits.max_target_offset_temporary_bytes,
    )?;
    stats.geometry_integer_bit_work = bit_work;
    stats.target_offset_integer_bits = target_offset_bits;
    stats.target_offset_temporary_bytes = target_offset_bytes;
    Ok(())
}

fn verify_target_offset_census(
    target_offset: &Vec<Integer>,
    stats: &GeneratedAffineResidualGroupExactRelationStats,
) -> Result<(), GeneratedAffineResidualGroupExactRelationError> {
    let (bits, bytes) = integer_vec_owned_census(target_offset, false)?;
    if bits > stats.target_offset_integer_bits || bytes > stats.target_offset_temporary_bytes {
        return Err(GeneratedAffineResidualGroupExactRelationError::PhysicalKey);
    }
    Ok(())
}

fn exact_offsets_equal(
    left: &[Integer],
    right: &[Integer],
    limits: GeneratedAffineResidualGroupExactRelationLimits,
    stats: &mut GeneratedAffineResidualGroupExactRelationStats,
) -> Result<bool, GeneratedAffineResidualGroupExactRelationError> {
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left, right) in left.iter().zip(right) {
        stats.geometry_integer_operations = bounded_add(
            "exact recentering geometry integer operations",
            stats.geometry_integer_operations,
            1,
            limits.max_geometry_integer_operations,
        )?;
        let comparison_work = integer_bits(left)?.max(integer_bits(right)?).max(1);
        stats.geometry_integer_bit_work = bounded_add(
            "exact recentering geometry integer-bit work",
            stats.geometry_integer_bit_work,
            comparison_work,
            limits.max_geometry_integer_bit_work,
        )?;
        if left != right {
            return Ok(false);
        }
    }
    Ok(true)
}

fn execute_target_offset(
    pivot: &GeneratedAffineResidualGroupLatticeShift,
    matrix: &[Integer],
    free_positions: &[usize],
    arity: usize,
) -> Result<Vec<Integer>, GeneratedAffineResidualGroupExactRelationError> {
    let mut output = try_vec("exact recentering target offset", arity)?;
    for row in 0..arity {
        let mut sum = Integer::from(0);
        for (free_ordinal, &free_position) in free_positions.iter().enumerate() {
            sum +=
                &matrix[row * free_positions.len() + free_ordinal] * &pivot.values()[free_position];
        }
        output.push(canonical_integer(&pivot.values()[row] - sum));
    }
    Ok(output)
}

fn coefficient_translation(
    pivot: &GeneratedAffineResidualGroupLatticeShift,
    free_positions: &[usize],
    arity: usize,
) -> Result<Vec<Integer>, GeneratedAffineResidualGroupExactRelationError> {
    let mut output = try_vec("exact recentering coefficient translation", arity)?;
    let mut free_cursor = 0usize;
    for position in 0..arity {
        let is_free = free_positions.get(free_cursor).copied() == Some(position);
        free_cursor += usize::from(is_free);
        output.push(if is_free {
            canonical_integer(-&pivot.values()[position])
        } else {
            Integer::from(0)
        });
    }
    Ok(output)
}

fn preflight_coefficient_translation(
    pivot: &GeneratedAffineResidualGroupLatticeShift,
    free_positions: &[usize],
    arity: usize,
    limits: GeneratedAffineResidualGroupExactRelationLimits,
    stats: &mut GeneratedAffineResidualGroupExactRelationStats,
) -> Result<(), GeneratedAffineResidualGroupExactRelationError> {
    let mut total_bits = 0usize;
    let mut retained_bytes = arc_vec_retained_bytes_bound::<Integer>(arity)?;
    let mut free_cursor = 0usize;
    for position in 0..arity {
        let is_free = free_positions.get(free_cursor).copied() == Some(position);
        free_cursor += usize::from(is_free);
        if !is_free {
            continue;
        }
        let bits = integer_bits(&pivot.values()[position])?;
        check_limit(
            "exact recentering coefficient-translation integer bits",
            bits,
            limits.max_exact_integer_bits,
        )?;
        total_bits = checked_add(
            "exact recentering coefficient-translation integer bits",
            total_bits,
            bits,
        )?;
        retained_bytes = checked_add(
            "exact recentering coefficient-translation retained bytes",
            retained_bytes,
            prospective_integer_heap_bytes(bits)?,
        )?;
        stats.geometry_integer_operations = bounded_add(
            "exact recentering geometry integer operations",
            stats.geometry_integer_operations,
            1,
            limits.max_geometry_integer_operations,
        )?;
        stats.geometry_integer_bit_work = bounded_add(
            "exact recentering geometry integer-bit work",
            stats.geometry_integer_bit_work,
            bits.max(1),
            limits.max_geometry_integer_bit_work,
        )?;
    }
    check_limit(
        "exact recentering coefficient-translation integer bits",
        total_bits,
        limits.max_coefficient_translation_integer_bits,
    )?;
    check_limit(
        "exact recentering coefficient-translation retained bytes",
        retained_bytes,
        limits.max_coefficient_translation_retained_bytes,
    )?;
    stats.coefficient_translation_integer_bits = total_bits;
    stats.coefficient_translation_retained_bytes = retained_bytes;
    Ok(())
}

fn verify_coefficient_translation_census(
    translation: &Vec<Integer>,
    stats: &GeneratedAffineResidualGroupExactRelationStats,
) -> Result<(), GeneratedAffineResidualGroupExactRelationError> {
    let (bits, bytes) = integer_vec_owned_census(translation, true)?;
    if bits > stats.coefficient_translation_integer_bits
        || bytes > stats.coefficient_translation_retained_bytes
    {
        return Err(GeneratedAffineResidualGroupExactRelationError::PhysicalKey);
    }
    Ok(())
}

fn preflight_and_center_keys(
    physical_keys: &[GeneratedAffineResidualGroupPhysicalKey],
    pivot: &GeneratedAffineResidualGroupLatticeShift,
    limits: GeneratedAffineResidualGroupExactRelationLimits,
    stats: &mut GeneratedAffineResidualGroupExactRelationStats,
) -> Result<Vec<ExactCenteredShift>, GeneratedAffineResidualGroupExactRelationError> {
    let components = checked_mul(
        "exact recentering exact-shift components",
        physical_keys.len(),
        pivot.arity(),
    )?;
    check_limit(
        "exact recentering exact-shift components",
        components,
        limits.max_exact_shift_components,
    )?;
    let mut prospective_bits = 0usize;
    let mut prospective_bytes = 0usize;
    for key in physical_keys {
        prospective_bytes = checked_add(
            "exact recentering exact-shift retained bytes",
            prospective_bytes,
            arc_vec_retained_bytes_bound::<Integer>(pivot.arity())?,
        )?;
        for (value, center) in key.shift().values().iter().zip(pivot.values()) {
            let bits = checked_add(
                "exact recentering exact-shift integer bits",
                integer_bits(value)?.max(integer_bits(center)?),
                1,
            )?;
            check_limit(
                "exact recentering exact-shift integer bits",
                bits,
                limits.max_exact_integer_bits,
            )?;
            prospective_bits = checked_add(
                "exact recentering exact-shift integer bits",
                prospective_bits,
                bits,
            )?;
            prospective_bytes = checked_add(
                "exact recentering exact-shift retained bytes",
                prospective_bytes,
                prospective_integer_heap_bytes(bits)?,
            )?;
        }
    }
    check_limit(
        "exact recentering exact-shift integer bits",
        prospective_bits,
        limits.max_exact_shift_integer_bits,
    )?;
    check_limit(
        "exact recentering exact-shift retained bytes",
        prospective_bytes,
        limits.max_exact_shift_retained_bytes,
    )?;
    let mut output = try_vec("exact recentering centered shifts", physical_keys.len())?;
    let mut observed_bits = 0usize;
    let mut observed_bytes = 0usize;
    for key in physical_keys {
        let mut values = try_vec("exact recentering centered-shift values", pivot.arity())?;
        let mut retained_bits = 0usize;
        for (value, center) in key.shift().values().iter().zip(pivot.values()) {
            let centered = canonical_integer(value - center);
            let bits = integer_bits(&centered)?;
            retained_bits = checked_add(
                "exact recentering exact-shift integer bits",
                retained_bits,
                bits,
            )?;
            values.push(centered);
        }
        let (censused_bits, retained_bytes) = integer_vec_owned_census(&values, true)?;
        if censused_bits != retained_bits {
            return Err(GeneratedAffineResidualGroupExactRelationError::PhysicalKey);
        }
        observed_bits = checked_add(
            "exact recentering exact-shift integer bits",
            observed_bits,
            retained_bits,
        )?;
        observed_bytes = checked_add(
            "exact recentering exact-shift retained bytes",
            observed_bytes,
            retained_bytes,
        )?;
        output.push(ExactCenteredShift {
            values: Arc::new(values),
            retained_integer_bits: retained_bits,
            retained_bytes,
        });
    }
    if observed_bits > prospective_bits || observed_bytes > prospective_bytes {
        return Err(GeneratedAffineResidualGroupExactRelationError::PhysicalKey);
    }
    stats.exact_shift_components = components;
    stats.exact_shift_integer_bits = observed_bits;
    stats.exact_shift_retained_bytes = observed_bytes;
    Ok(output)
}

fn preflight_translations(
    context: &ParametricCoefficientContext,
    relation: &ParametricRelation,
    shift: &[Integer],
    pivot_retained_bytes: usize,
    locator_origin: &GuardOrigin,
    limits: GeneratedAffineResidualGroupExactRelationLimits,
    stats: &mut GeneratedAffineResidualGroupExactRelationStats,
) -> Result<TranslationAdmission, GeneratedAffineResidualGroupExactRelationError> {
    let mut maximum_polynomial_bytes = 0usize;
    let mut final_bytes = size_of::<GeneratedAffineResidualGroupExactRelationCandidate>();
    final_bytes = checked_add(
        "exact recentering final retained output bytes",
        final_bytes,
        arc_vec_retained_bytes_bound::<ExactRelationTerm>(relation.terms().len())?,
    )?;
    final_bytes = checked_add(
        "exact recentering final retained output bytes",
        final_bytes,
        arc_vec_retained_bytes_bound::<ParametricNonZeroCondition>(
            relation.guarded_nonzero_conditions().len(),
        )?,
    )?;
    final_bytes = checked_add(
        "exact recentering final retained output bytes",
        final_bytes,
        stats.exact_shift_retained_bytes,
    )?;
    final_bytes = checked_add(
        "exact recentering final retained output bytes",
        final_bytes,
        stats.coefficient_translation_retained_bytes,
    )?;
    final_bytes = checked_add(
        "exact recentering final retained output bytes",
        final_bytes,
        pivot_retained_bytes,
    )?;
    for coefficient in relation.terms().values() {
        let preflight =
            context.preflight_translate_coefficient_exact(coefficient, shift, limits.arithmetic)?;
        accumulate_coefficient_preflight(stats, preflight, limits)?;
        maximum_polynomial_bytes = maximum_polynomial_bytes
            .max(preflight.numerator().retained_output_byte_bound())
            .max(preflight.denominator().retained_output_byte_bound());
        final_bytes = checked_add(
            "exact recentering final retained output bytes",
            final_bytes,
            preflight.normalized_coefficient_byte_bound(),
        )?;
    }
    for guard in relation.guarded_nonzero_conditions() {
        let prospective_origins = checked_add(
            "exact recentering guard-origin occurrences",
            guard.origins().len(),
            usize::from(!guard.origins().contains(locator_origin)),
        )?;
        check_limit(
            "exact recentering guard origins per condition",
            prospective_origins,
            limits.arithmetic.max_guard_origins,
        )?;
        stats.guard_origin_occurrences = bounded_add(
            "exact recentering guard-origin occurrences",
            stats.guard_origin_occurrences,
            prospective_origins,
            limits.max_guard_origin_occurrences,
        )?;
        let preflight = context.preflight_translate_polynomial_exact(
            guard.polynomial(),
            shift,
            limits.arithmetic,
        )?;
        accumulate_polynomial_preflight(stats, preflight, limits)?;
        maximum_polynomial_bytes =
            maximum_polynomial_bytes.max(preflight.retained_output_byte_bound());
        final_bytes = checked_add(
            "exact recentering final retained output bytes",
            final_bytes,
            preflight.retained_output_byte_bound(),
        )?;
        for origin in guard.origins() {
            final_bytes = checked_add(
                "exact recentering final retained output bytes",
                final_bytes,
                origin.retained_byte_bound().ok_or(
                    GeneratedAffineResidualGroupExactRelationError::ResourceCountOverflow {
                        resource: "exact recentering final retained output bytes",
                    },
                )?,
            )?;
        }
        final_bytes = checked_add(
            "exact recentering final retained output bytes",
            final_bytes,
            locator_origin.retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactRelationError::ResourceCountOverflow {
                    resource: "exact recentering final retained output bytes",
                },
            )?,
        )?;
    }
    check_limit(
        "exact recentering translation retained output bytes",
        stats.translation_retained_output_bytes,
        limits.max_translation_retained_output_bytes,
    )?;
    check_limit(
        "exact recentering owner retained bytes",
        final_bytes,
        limits.max_owner_retained_bytes,
    )?;
    let native_temporary = checked_add(
        "exact recentering native temporary byte envelope",
        final_bytes,
        checked_add(
            "exact recentering native temporary byte envelope",
            checked_mul(
                "exact recentering native temporary byte envelope",
                maximum_polynomial_bytes,
                3,
            )?,
            native_exact_scratch_bytes(stats)?,
        )?,
    )?;
    check_limit(
        "exact recentering native temporary byte envelope",
        native_temporary,
        limits.max_native_temporary_byte_envelope,
    )?;
    stats.native_temporary_byte_envelope = native_temporary;
    Ok(TranslationAdmission {
        final_retained_output_bytes: final_bytes,
    })
}

fn accumulate_coefficient_preflight(
    stats: &mut GeneratedAffineResidualGroupExactRelationStats,
    preflight: ParametricCoefficientTranslationPreflight,
    limits: GeneratedAffineResidualGroupExactRelationLimits,
) -> Result<(), GeneratedAffineResidualGroupExactRelationError> {
    accumulate_translation_counts(
        stats,
        preflight.source_terms(),
        checked_add(
            "exact recentering source exponent entries",
            preflight.numerator().source_exponent_entries(),
            preflight.denominator().source_exponent_entries(),
        )?,
        preflight.output_term_bound(),
        checked_add(
            "exact recentering output exponent entries",
            preflight.numerator().output_exponent_entry_bound(),
            preflight.denominator().output_exponent_entry_bound(),
        )?,
        preflight.power_operation_bound(),
        preflight.integer_bit_work_bound(),
        preflight.normalized_coefficient_term_bound(),
        preflight.normalized_coefficient_byte_bound(),
        limits,
    )
}

fn accumulate_polynomial_preflight(
    stats: &mut GeneratedAffineResidualGroupExactRelationStats,
    preflight: ParametricPolynomialTranslationPreflight,
    limits: GeneratedAffineResidualGroupExactRelationLimits,
) -> Result<(), GeneratedAffineResidualGroupExactRelationError> {
    accumulate_translation_counts(
        stats,
        preflight.source_terms(),
        preflight.source_exponent_entries(),
        preflight.output_term_bound(),
        preflight.output_exponent_entry_bound(),
        preflight.power_operation_bound(),
        preflight.integer_bit_work_bound(),
        preflight.retained_output_term_bound(),
        preflight.retained_output_byte_bound(),
        limits,
    )
}

#[allow(clippy::too_many_arguments)]
fn accumulate_translation_counts(
    stats: &mut GeneratedAffineResidualGroupExactRelationStats,
    source_terms: usize,
    source_exponent_entries: usize,
    output_terms: usize,
    output_exponent_entries: usize,
    power_operations: usize,
    integer_bit_work: usize,
    normalized_terms: usize,
    retained_bytes: usize,
    limits: GeneratedAffineResidualGroupExactRelationLimits,
) -> Result<(), GeneratedAffineResidualGroupExactRelationError> {
    // The exact execution API repeats the allocation-free scan/work preflight
    // internally. Charge both passes for those logical work counters. The
    // normalized terms and retained bytes describe the single successful
    // output, however, and must not be doubled merely because its bound was
    // computed twice.
    stats.translation_preflight_passes = bounded_add(
        "exact recentering translation preflight passes",
        stats.translation_preflight_passes,
        2,
        limits.max_translation_preflight_passes,
    )?;
    for (resource, field, increment, limit) in [
        (
            "exact recentering translation source terms",
            &mut stats.translation_source_terms,
            source_terms,
            limits.max_translation_source_terms,
        ),
        (
            "exact recentering translation source exponent entries",
            &mut stats.translation_source_exponent_entries,
            source_exponent_entries,
            limits.max_translation_source_exponent_entries,
        ),
        (
            "exact recentering translation output terms",
            &mut stats.translation_output_terms,
            output_terms,
            limits.max_translation_output_terms,
        ),
        (
            "exact recentering translation output exponent entries",
            &mut stats.translation_output_exponent_entries,
            output_exponent_entries,
            limits.max_translation_output_exponent_entries,
        ),
        (
            "exact recentering translation power operations",
            &mut stats.translation_power_operations,
            power_operations,
            limits.max_translation_power_operations,
        ),
        (
            "exact recentering translation integer-bit work",
            &mut stats.translation_integer_bit_work,
            integer_bit_work,
            limits.max_translation_integer_bit_work,
        ),
    ] {
        let doubled = checked_mul(resource, increment, 2)?;
        *field = bounded_add(resource, *field, doubled, limit)?;
    }
    stats.translation_normalized_terms = bounded_add(
        "exact recentering translation normalized terms",
        stats.translation_normalized_terms,
        normalized_terms,
        limits.max_translation_normalized_terms,
    )?;
    stats.translation_retained_output_bytes = bounded_add(
        "exact recentering translation retained output bytes",
        stats.translation_retained_output_bytes,
        retained_bytes,
        limits.max_translation_retained_output_bytes,
    )?;
    Ok(())
}

fn admit_no_target(
    stats: &mut GeneratedAffineResidualGroupExactRelationStats,
    limits: GeneratedAffineResidualGroupExactRelationLimits,
) -> Result<(), GeneratedAffineResidualGroupExactRelationError> {
    let owner_retained_bytes = size_of::<GeneratedAffineResidualGroupExactRelationNoTarget>();
    check_limit(
        "exact recentering owner retained bytes",
        owner_retained_bytes,
        limits.max_owner_retained_bytes,
    )?;
    let native_temporary_byte_envelope = checked_add(
        "exact recentering native temporary byte envelope",
        owner_retained_bytes,
        native_exact_scratch_bytes(stats)?,
    )?;
    check_limit(
        "exact recentering native temporary byte envelope",
        native_temporary_byte_envelope,
        limits.max_native_temporary_byte_envelope,
    )?;
    stats.owner_retained_bytes = owner_retained_bytes;
    stats.native_temporary_byte_envelope = native_temporary_byte_envelope;
    Ok(())
}

fn native_exact_scratch_bytes(
    stats: &GeneratedAffineResidualGroupExactRelationStats,
) -> Result<usize, GeneratedAffineResidualGroupExactRelationError> {
    let resource = "exact recentering native temporary byte envelope";
    let mut bytes = vec_retained_bytes_bound::<GeneratedAffineResidualGroupPhysicalKey>(
        stats.physical_key_constructions,
    )?;
    for increment in [
        stats.physical_key_prospective_retained_bytes,
        stats.physical_key_retained_bytes,
        stats.target_offset_temporary_bytes,
        stats.exact_shift_retained_bytes,
        stats.coefficient_translation_retained_bytes,
    ] {
        bytes = checked_add(resource, bytes, increment)?;
    }
    Ok(bytes)
}

fn observed_output_bytes(
    terms: &Vec<ExactRelationTerm>,
    guards: &Vec<ParametricNonZeroCondition>,
    coefficient_translation: &Vec<Integer>,
    pivot: &GeneratedAffineResidualGroupLatticeShift,
) -> Result<usize, GeneratedAffineResidualGroupExactRelationError> {
    let mut bytes = size_of::<GeneratedAffineResidualGroupExactRelationCandidate>();
    bytes = checked_add(
        "exact recentering observed output bytes",
        bytes,
        arc_vec_retained_bytes_bound::<ExactRelationTerm>(terms.capacity())?,
    )?;
    for term in terms {
        bytes = checked_add(
            "exact recentering observed output bytes",
            bytes,
            term.shift.retained_bytes,
        )?;
        bytes = checked_add(
            "exact recentering observed output bytes",
            bytes,
            term.coefficient.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactRelationError::ResourceCountOverflow {
                    resource: "exact recentering observed output bytes",
                },
            )?,
        )?;
    }
    bytes = checked_add(
        "exact recentering observed output bytes",
        bytes,
        arc_vec_retained_bytes_bound::<ParametricNonZeroCondition>(guards.capacity())?,
    )?;
    for guard in guards {
        bytes = checked_add(
            "exact recentering observed output bytes",
            bytes,
            guard.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactRelationError::ResourceCountOverflow {
                    resource: "exact recentering observed output bytes",
                },
            )?,
        )?;
    }
    bytes = checked_add(
        "exact recentering observed output bytes",
        bytes,
        integer_vec_owned_census(coefficient_translation, true)?.1,
    )?;
    checked_add(
        "exact recentering observed output bytes",
        bytes,
        pivot.retained_bytes(),
    )
}

fn canonical_integer(value: Integer) -> Integer {
    match value {
        Integer::Single(value) => Integer::from(value),
        Integer::Double(value) => Integer::from(value),
        Integer::Large(value) => Integer::from(value),
    }
}

fn integer_bits(value: &Integer) -> Result<usize, GeneratedAffineResidualGroupExactRelationError> {
    integer_magnitude_bits(value).map_err(|_| {
        GeneratedAffineResidualGroupExactRelationError::ResourceCountOverflow {
            resource: "exact recentering integer bits",
        }
    })
}

fn integer_owned_heap_bytes(
    value: &Integer,
) -> Result<usize, GeneratedAffineResidualGroupExactRelationError> {
    match value {
        Integer::Single(_) | Integer::Double(_) => Ok(0),
        Integer::Large(value) => value.capacity().checked_add(7).map(|bits| bits / 8).ok_or(
            GeneratedAffineResidualGroupExactRelationError::ResourceCountOverflow {
                resource: "exact recentering integer owned heap bytes",
            },
        ),
    }
}

fn prospective_integer_heap_bytes(
    bits: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactRelationError> {
    if bits <= i128::BITS as usize - 1 {
        Ok(0)
    } else {
        // GMP commonly retains a small number of spare limbs.  Match the
        // conservative allowance used by the exact physical-key layer rather
        // than admitting only the mathematical minimum limb count.
        let limbs = checked_add("exact recentering integer retained bytes", bits, 191)? / 64;
        checked_mul(
            "exact recentering integer retained bytes",
            limbs,
            size_of::<u64>(),
        )
    }
}

fn integer_retained_bytes(
    bits: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactRelationError> {
    checked_add(
        "exact recentering integer retained bytes",
        size_of::<Integer>(),
        prospective_integer_heap_bytes(bits)?,
    )
}

fn arc_payload_control_and_padding_byte_bound<T>()
-> Result<usize, GeneratedAffineResidualGroupExactRelationError> {
    checked_add(
        "exact recentering retained bytes",
        checked_mul(
            "exact recentering retained bytes",
            2,
            size_of::<AtomicUsize>(),
        )?,
        checked_add(
            "exact recentering retained bytes",
            align_of::<T>().saturating_sub(1),
            size_of::<T>(),
        )?,
    )
}

fn vec_retained_bytes_bound<T>(
    capacity: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactRelationError> {
    checked_add(
        "exact recentering retained bytes",
        size_of::<Vec<T>>(),
        checked_mul("exact recentering retained bytes", capacity, size_of::<T>())?,
    )
}

fn arc_vec_retained_bytes_bound<T>(
    capacity: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactRelationError> {
    checked_add(
        "exact recentering retained bytes",
        arc_payload_control_and_padding_byte_bound::<Vec<T>>()?,
        checked_mul("exact recentering retained bytes", capacity, size_of::<T>())?,
    )
}

fn integer_vec_owned_census(
    values: &Vec<Integer>,
    retained_in_arc: bool,
) -> Result<(usize, usize), GeneratedAffineResidualGroupExactRelationError> {
    let mut bits = 0usize;
    let mut bytes = if retained_in_arc {
        arc_vec_retained_bytes_bound::<Integer>(values.capacity())?
    } else {
        vec_retained_bytes_bound::<Integer>(values.capacity())?
    };
    for value in values {
        bits = checked_add(
            "exact recentering integer-vector bits",
            bits,
            integer_bits(value)?,
        )?;
        bytes = checked_add(
            "exact recentering integer-vector retained bytes",
            bytes,
            integer_owned_heap_bytes(value)?,
        )?;
    }
    Ok((bits, bytes))
}

fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, GeneratedAffineResidualGroupExactRelationError> {
    let mut output = Vec::new();
    output.try_reserve_exact(capacity).map_err(|_| {
        GeneratedAffineResidualGroupExactRelationError::AllocationFailure { resource }
    })?;
    Ok(output)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactRelationError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualGroupExactRelationError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactRelationError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineResidualGroupExactRelationError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    current: usize,
    increment: usize,
    limit: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactRelationError> {
    let requested = checked_add(resource, current, increment)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualGroupExactRelationError> {
    if requested > limit {
        Err(
            GeneratedAffineResidualGroupExactRelationError::ResourceLimit {
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
                term.shift
                    .values
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
                    term.shift
                        .values
                        .iter()
                        .map(exact_to_i64)
                        .collect::<Vec<_>>(),
                    term.coefficient.clone(),
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
            term.shift
                .values
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
        assert_eq!(candidate.terms[0].coefficient, expected);
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
        let mut stats = GeneratedAffineResidualGroupExactRelationStats::default();
        preflight_exact_geometry(&pivot, matrix, free_positions, limits, &mut stats).unwrap();
        let offset = execute_target_offset(&pivot, matrix, free_positions, pivot.arity()).unwrap();
        assert!(offset.iter().all(|value| value == &Integer::from(0)));

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
        let mut exact_limits = limits;
        exact_limits.max_target_offset_temporary_bytes = exact_demand;
        let mut exact_stats = GeneratedAffineResidualGroupExactRelationStats::default();
        preflight_exact_geometry(
            &pivot,
            matrix,
            free_positions,
            exact_limits,
            &mut exact_stats,
        )
        .unwrap();
        assert_eq!(exact_stats.target_offset_temporary_bytes(), exact_demand);

        let with_target_scratch = native_exact_scratch_bytes(&stats).unwrap();
        let mut without_target = stats;
        without_target.target_offset_temporary_bytes = 0;
        let without_target_scratch = native_exact_scratch_bytes(&without_target).unwrap();
        assert_eq!(
            with_target_scratch.checked_sub(without_target_scratch),
            Some(exact_demand),
            "the revised target envelope must enter native scratch exactly once"
        );

        let mut formerly_admitted = limits;
        formerly_admitted.max_target_offset_temporary_bytes = old_final_only_bound;
        let mut rejected_stats = GeneratedAffineResidualGroupExactRelationStats::default();
        assert!(matches!(
            preflight_exact_geometry(
                &pivot,
                matrix,
                free_positions,
                formerly_admitted,
                &mut rejected_stats,
            ),
            Err(GeneratedAffineResidualGroupExactRelationError::ResourceLimit { .. })
        ));

        let mut one_below = limits;
        one_below.max_target_offset_temporary_bytes = exact_demand - 1;
        let mut one_below_stats = GeneratedAffineResidualGroupExactRelationStats::default();
        assert!(matches!(
            preflight_exact_geometry(
                &pivot,
                matrix,
                free_positions,
                one_below,
                &mut one_below_stats,
            ),
            Err(GeneratedAffineResidualGroupExactRelationError::ResourceLimit { .. })
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
