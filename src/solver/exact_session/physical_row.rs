//! Sealed, unrecentered exact rows for generated affine group elimination.
//!
//! This topology-neutral ingress converts every compact case-local
//! [`IndexShift`](crate::IndexShift) in one authenticated completed bound row
//! into the common arbitrary-precision physical coordinates retained by a
//! [`GeneratedAffineResidualGroupPhysicalFrame`]. It deliberately performs
//! no pivot selection, row normalization, cross-row elimination, target
//! matching, recentering, rule publication, or master inference. Those state
//! transitions belong to the future persistent exact group database.

use std::cmp::Ordering;
use std::fmt;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use super::physical_key::{
    GeneratedAffineResidualGroupPhysicalFrame, GeneratedAffineResidualGroupPhysicalKey,
    GeneratedAffineResidualGroupPhysicalKeyError,
};
use crate::generated_affine_residual_case_completed_bound_row::GeneratedAffineResidualCaseCompletedBoundRow;
use crate::{
    IntegralFamily, ParametricCoefficient, ParametricCoefficientContext, ParametricNonZeroCondition,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_PHYSICAL_ROW_V2_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-physical-row-v2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactPhysicalRowLimits {
    pub(crate) max_completed_row_replays: usize,
    pub(crate) max_frame_replays: usize,
    pub(crate) max_parent_allocation_comparisons: usize,
    pub(crate) max_arity: usize,
    pub(crate) max_terms: usize,
    pub(crate) max_guards: usize,
    pub(crate) max_guard_origin_occurrences: usize,
    pub(crate) max_local_shift_component_scans: usize,
    pub(crate) max_local_key_census_preflights: usize,
    pub(crate) max_local_key_census_component_scans: usize,
    pub(crate) max_local_key_census_integer_bit_work: usize,
    pub(crate) max_physical_key_preflights: usize,
    pub(crate) max_physical_key_constructions: usize,
    pub(crate) max_physical_key_component_scans: usize,
    pub(crate) max_physical_key_integer_bit_work: usize,
    pub(crate) max_physical_key_prospective_integer_bits: usize,
    pub(crate) max_physical_key_prospective_retained_bytes: usize,
    pub(crate) max_output_vector_retained_bytes: usize,
    pub(crate) max_sort_log_height: usize,
    pub(crate) max_sort_comparison_bound: usize,
    pub(crate) max_sort_swap_bound: usize,
    pub(crate) max_sort_integer_bit_work_bound: usize,
    pub(crate) max_coefficient_clone_retained_bytes: usize,
    pub(crate) max_guard_clone_retained_bytes: usize,
    pub(crate) max_prospective_owner_retained_bytes: usize,
    pub(crate) max_owner_retained_bytes: usize,
    pub(crate) max_native_temporary_byte_envelope: usize,
}

impl Default for GeneratedAffineResidualGroupExactPhysicalRowLimits {
    fn default() -> Self {
        const LARGE: usize = 64_000_000_000;
        const VERY_LARGE: usize = 4_000_000_000_000_000_000;
        Self {
            max_completed_row_replays: 1,
            max_frame_replays: 1,
            max_parent_allocation_comparisons: 6,
            max_arity: 1_000_000,
            max_terms: 16_000_000,
            max_guards: 16_000_000,
            max_guard_origin_occurrences: LARGE,
            max_local_shift_component_scans: LARGE,
            max_local_key_census_preflights: 32_000_000,
            max_local_key_census_component_scans: LARGE,
            max_local_key_census_integer_bit_work: VERY_LARGE,
            max_physical_key_preflights: 16_000_000,
            max_physical_key_constructions: 16_000_000,
            max_physical_key_component_scans: LARGE,
            max_physical_key_integer_bit_work: VERY_LARGE,
            max_physical_key_prospective_integer_bits: VERY_LARGE,
            max_physical_key_prospective_retained_bytes: 128 * 1024 * 1024 * 1024,
            max_output_vector_retained_bytes: 32 * 1024 * 1024 * 1024,
            max_sort_log_height: usize::BITS as usize,
            max_sort_comparison_bound: VERY_LARGE,
            max_sort_swap_bound: VERY_LARGE,
            max_sort_integer_bit_work_bound: VERY_LARGE,
            max_coefficient_clone_retained_bytes: 128 * 1024 * 1024 * 1024,
            max_guard_clone_retained_bytes: 128 * 1024 * 1024 * 1024,
            max_prospective_owner_retained_bytes: 256 * 1024 * 1024 * 1024,
            max_owner_retained_bytes: 256 * 1024 * 1024 * 1024,
            max_native_temporary_byte_envelope: 512 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactPhysicalRowStats {
    completed_row_replays: usize,
    frame_replays: usize,
    parent_allocation_comparisons: usize,
    arity: usize,
    terms: usize,
    guards: usize,
    guard_origin_occurrences: usize,
    local_shift_component_scans: usize,
    local_key_census_preflights: usize,
    local_key_census_component_scans: usize,
    local_key_census_integer_bit_work: usize,
    physical_key_preflights: usize,
    physical_key_constructions: usize,
    physical_key_component_scans: usize,
    physical_key_integer_bit_work: usize,
    physical_key_prospective_integer_bits: usize,
    physical_key_prospective_retained_bytes: usize,
    physical_key_retained_bytes: usize,
    output_vector_retained_bytes: usize,
    sort_log_height: usize,
    sort_comparison_bound: usize,
    sort_swap_bound: usize,
    sort_integer_bit_work_bound: usize,
    sort_integer_bit_work: usize,
    sort_comparisons: usize,
    sort_swaps: usize,
    coefficient_clone_retained_bytes: usize,
    guard_clone_retained_bytes: usize,
    prospective_owner_retained_bytes: usize,
    owner_retained_bytes: usize,
    native_temporary_byte_envelope: usize,
}

macro_rules! stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualGroupExactPhysicalRowStats {
    stats_getters!(
        completed_row_replays,
        frame_replays,
        parent_allocation_comparisons,
        arity,
        terms,
        guards,
        guard_origin_occurrences,
        local_shift_component_scans,
        local_key_census_preflights,
        local_key_census_component_scans,
        local_key_census_integer_bit_work,
        physical_key_preflights,
        physical_key_constructions,
        physical_key_component_scans,
        physical_key_integer_bit_work,
        physical_key_prospective_integer_bits,
        physical_key_prospective_retained_bytes,
        physical_key_retained_bytes,
        output_vector_retained_bytes,
        sort_log_height,
        sort_comparison_bound,
        sort_swap_bound,
        sort_integer_bit_work_bound,
        sort_integer_bit_work,
        sort_comparisons,
        sort_swaps,
        coefficient_clone_retained_bytes,
        guard_clone_retained_bytes,
        prospective_owner_retained_bytes,
        owner_retained_bytes,
        native_temporary_byte_envelope,
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactPhysicalRowError {
    WrongFamily,
    WrongContext,
    WrongArity,
    WrongParentAllocation,
    WrongCaseBinding,
    WrongGroupBinding,
    CompletedRow,
    EmptyRelation,
    DuplicatePhysicalKey,
    ReplayMismatch,
    PhysicalKey,
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

impl GeneratedAffineResidualGroupExactPhysicalRowError {
    const fn kind(self) -> &'static str {
        match self {
            Self::WrongFamily => "WrongFamily",
            Self::WrongContext => "WrongContext",
            Self::WrongArity => "WrongArity",
            Self::WrongParentAllocation => "WrongParentAllocation",
            Self::WrongCaseBinding => "WrongCaseBinding",
            Self::WrongGroupBinding => "WrongGroupBinding",
            Self::CompletedRow => "CompletedRow",
            Self::EmptyRelation => "EmptyRelation",
            Self::DuplicatePhysicalKey => "DuplicatePhysicalKey",
            Self::ReplayMismatch => "ReplayMismatch",
            Self::PhysicalKey => "PhysicalKey",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::AllocationFailure { .. } => "AllocationFailure",
            Self::SymbolicaPanic => "SymbolicaPanic",
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactPhysicalRowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactPhysicalRowError")
            .field("kind", &self.kind())
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactPhysicalRowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "generated affine exact physical-row {}",
            self.kind()
        )
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactPhysicalRowError {}

impl From<GeneratedAffineResidualGroupPhysicalKeyError>
    for GeneratedAffineResidualGroupExactPhysicalRowError
{
    fn from(_: GeneratedAffineResidualGroupPhysicalKeyError) -> Self {
        Self::PhysicalKey
    }
}

#[derive(Clone, PartialEq, Eq)]
struct GeneratedAffineResidualGroupExactPhysicalTerm {
    key: GeneratedAffineResidualGroupPhysicalKey,
    coefficient: ParametricCoefficient,
}

impl fmt::Debug for GeneratedAffineResidualGroupExactPhysicalTerm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactPhysicalTerm")
            .field("private_key", &"<redacted>")
            .field("private_coefficient", &"<redacted>")
            .finish()
    }
}

#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualGroupExactPhysicalRow {
    schema: &'static str,
    source: Arc<GeneratedAffineResidualCaseCompletedBoundRow>,
    frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    source_case_ordinal: usize,
    group_ordinal: usize,
    expanded_ordinal: usize,
    layer_ordinal: usize,
    point_depth: usize,
    point_ordinal: usize,
    source_row_ordinal: usize,
    terms: Arc<Vec<GeneratedAffineResidualGroupExactPhysicalTerm>>,
    guards: Arc<Vec<ParametricNonZeroCondition>>,
    limits: GeneratedAffineResidualGroupExactPhysicalRowLimits,
    stats: GeneratedAffineResidualGroupExactPhysicalRowStats,
}

impl fmt::Debug for GeneratedAffineResidualGroupExactPhysicalRow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactPhysicalRow")
            .field("schema", &self.schema)
            .field("source_case_ordinal", &self.source_case_ordinal)
            .field("group_ordinal", &self.group_ordinal)
            .field("expanded_ordinal", &self.expanded_ordinal)
            .field("layer_ordinal", &self.layer_ordinal)
            .field("point_depth", &self.point_depth)
            .field("point_ordinal", &self.point_ordinal)
            .field("source_row_ordinal", &self.source_row_ordinal)
            .field("term_count", &self.terms.len())
            .field("guard_count", &self.guards.len())
            .field("stats", &self.stats)
            .field("private_source", &"<redacted>")
            .field("private_frame", &"<redacted>")
            .field("private_payload", &"<redacted>")
            .field("recentered", &false)
            .field("normalized", &false)
            .field("targets_consumed", &0)
            .field("rule_published", &false)
            .field("master_inferred", &false)
            .finish()
    }
}

/// Authenticated, borrowed database ingress for one frozen exact row.
///
/// The private row reference is obtainable only through
/// [`GeneratedAffineResidualGroupExactPhysicalRow::replay_for_database`].
/// The view intentionally exposes no source/frame owner, geometry, mutable
/// payload, or elimination behavior.
pub(crate) struct GeneratedAffineResidualGroupReplayedExactPhysicalRow<'a> {
    row: &'a GeneratedAffineResidualGroupExactPhysicalRow,
}

impl fmt::Debug for GeneratedAffineResidualGroupReplayedExactPhysicalRow<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupReplayedExactPhysicalRow")
            .field("source_case_ordinal", &self.source_case_ordinal())
            .field("group_ordinal", &self.group_ordinal())
            .field("expanded_ordinal", &self.expanded_ordinal())
            .field("source_row_ordinal", &self.source_row_ordinal())
            .field("term_count", &self.row.terms.len())
            .field("guard_count", &self.row.guards.len())
            .field("private_payload", &"<redacted>")
            .field("recentered", &self.is_recentered())
            .field("normalized", &self.is_normalized())
            .field("targets_consumed", &self.targets_consumed())
            .field("rule_published", &self.publishes_rule())
            .field("master_inferred", &self.infers_master())
            .finish()
    }
}

impl<'a> GeneratedAffineResidualGroupReplayedExactPhysicalRow<'a> {
    pub(crate) const fn source_case_ordinal(&self) -> usize {
        self.row.source_case_ordinal
    }

    pub(crate) const fn group_ordinal(&self) -> usize {
        self.row.group_ordinal
    }

    pub(crate) const fn expanded_ordinal(&self) -> usize {
        self.row.expanded_ordinal
    }

    pub(crate) const fn source_row_ordinal(&self) -> usize {
        self.row.source_row_ordinal
    }

    pub(crate) fn term_count(&self) -> usize {
        self.row.terms.len()
    }

    pub(crate) fn guard_count(&self) -> usize {
        self.row.guards.len()
    }

    pub(crate) fn terms(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &'a GeneratedAffineResidualGroupPhysicalKey,
            &'a ParametricCoefficient,
        ),
    > + DoubleEndedIterator
    + 'a {
        self.row
            .terms
            .iter()
            .map(|term| (&term.key, &term.coefficient))
    }

    pub(crate) fn guards(&self) -> &'a [ParametricNonZeroCondition] {
        self.row.guards.as_slice()
    }

    pub(crate) const fn is_recentered(&self) -> bool {
        false
    }

    pub(crate) const fn is_normalized(&self) -> bool {
        false
    }

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

impl GeneratedAffineResidualGroupExactPhysicalRow {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn source_case_ordinal(&self) -> usize {
        self.source_case_ordinal
    }

    pub(crate) const fn group_ordinal(&self) -> usize {
        self.group_ordinal
    }

    pub(crate) const fn expanded_ordinal(&self) -> usize {
        self.expanded_ordinal
    }

    pub(crate) const fn layer_ordinal(&self) -> usize {
        self.layer_ordinal
    }

    pub(crate) const fn point_depth(&self) -> usize {
        self.point_depth
    }

    pub(crate) const fn point_ordinal(&self) -> usize {
        self.point_ordinal
    }

    pub(crate) const fn source_row_ordinal(&self) -> usize {
        self.source_row_ordinal
    }

    pub(crate) fn term_count(&self) -> usize {
        self.terms.len()
    }

    pub(crate) fn guard_count(&self) -> usize {
        self.guards.len()
    }

    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactPhysicalRowStats {
        self.stats
    }

    /// Conservative unique-owner graph retained by a staged raw-row recipe.
    ///
    /// This includes the physical-row and completed-row `Arc` allocations and
    /// every uniquely reachable completed-row child. The physical frame and
    /// common retained source are shared with the exact solve plan and excluded.
    /// Only exact authority pointer identity with that frame suppresses the
    /// source-authority allocation; a non-anchor authority which merely shares
    /// the retained source remains charged.
    pub(crate) fn unique_retained_source_graph_byte_bound(&self) -> Option<usize> {
        let charge_source_authority = !self
            .frame
            .same_authority_allocation(self.source.authority());
        let mut bytes = self.stats.owner_retained_bytes();
        for contribution in [
            arc_control_and_padding_byte_bound::<Self>()?,
            arc_control_and_padding_byte_bound::<GeneratedAffineResidualCaseCompletedBoundRow>()?,
            self.source
                .retained_source_graph_byte_bound(charge_source_authority)?,
        ] {
            bytes = bytes.checked_add(contribution)?;
        }
        Some(bytes)
    }

    #[cfg(test)]
    pub(crate) const fn completed_source_for_retained_graph_test(
        &self,
    ) -> &Arc<GeneratedAffineResidualCaseCompletedBoundRow> {
        &self.source
    }

    pub(crate) fn same_parent_allocations(
        &self,
        source: &Arc<GeneratedAffineResidualCaseCompletedBoundRow>,
        frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    ) -> bool {
        Arc::ptr_eq(&self.source, source) && Arc::ptr_eq(&self.frame, frame)
    }

    /// This certificate is an unrecentered database input, never a rule.
    pub(crate) const fn is_recentered(&self) -> bool {
        false
    }

    pub(crate) const fn is_normalized(&self) -> bool {
        false
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    pub(crate) const fn infers_master(&self) -> bool {
        false
    }

    /// Replay the frozen row against its private retained source and the
    /// caller's exact frame allocation, then lend its payload to the group
    /// database without exposing either parent owner.
    pub(crate) fn replay_for_database<'a>(
        &'a self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    ) -> Result<
        GeneratedAffineResidualGroupReplayedExactPhysicalRow<'a>,
        GeneratedAffineResidualGroupExactPhysicalRowError,
    > {
        self.replay(family, context, &self.source, frame)?;
        Ok(GeneratedAffineResidualGroupReplayedExactPhysicalRow { row: self })
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: &Arc<GeneratedAffineResidualCaseCompletedBoundRow>,
        frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    ) -> Result<(), GeneratedAffineResidualGroupExactPhysicalRowError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_PHYSICAL_ROW_V2_SCHEMA {
                return Err(GeneratedAffineResidualGroupExactPhysicalRowError::ReplayMismatch);
            }
            if !self.same_parent_allocations(source, frame) {
                return Err(
                    GeneratedAffineResidualGroupExactPhysicalRowError::WrongParentAllocation,
                );
            }
            let replayed = GeneratedAffineResidualGroupExactPhysicalRowCompiler::compile_inner(
                family,
                context,
                Arc::clone(source),
                Arc::clone(frame),
                self.limits,
            )?;
            if exact_physical_rows_equal(self, &replayed) {
                Ok(())
            } else {
                Err(GeneratedAffineResidualGroupExactPhysicalRowError::ReplayMismatch)
            }
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactPhysicalRowError::SymbolicaPanic)?
    }
}

fn arc_control_and_padding_byte_bound<T>() -> Option<usize> {
    size_of::<AtomicUsize>()
        .checked_mul(2)?
        .checked_add(align_of::<T>().saturating_sub(1))
}

pub(crate) struct GeneratedAffineResidualGroupExactPhysicalRowCompiler;

impl GeneratedAffineResidualGroupExactPhysicalRowCompiler {
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: Arc<GeneratedAffineResidualCaseCompletedBoundRow>,
        frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        limits: GeneratedAffineResidualGroupExactPhysicalRowLimits,
    ) -> Result<
        GeneratedAffineResidualGroupExactPhysicalRow,
        GeneratedAffineResidualGroupExactPhysicalRowError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            Self::compile_inner(family, context, source, frame, limits)
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactPhysicalRowError::SymbolicaPanic)?
    }

    fn compile_inner(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: Arc<GeneratedAffineResidualCaseCompletedBoundRow>,
        frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        limits: GeneratedAffineResidualGroupExactPhysicalRowLimits,
    ) -> Result<
        GeneratedAffineResidualGroupExactPhysicalRow,
        GeneratedAffineResidualGroupExactPhysicalRowError,
    > {
        const COMPLETED_ROW_REPLAYS: usize = 1;
        const FRAME_REPLAYS: usize = 1;
        // Four source-parent `Arc` checks in completed-row replay, one exact
        // shared-source allocation comparison in the frame source seam, and
        // one retained anchor-authority comparison in frame replay.
        const PARENT_ALLOCATION_COMPARISONS: usize = 6;

        for (resource, requested, limit) in [
            (
                "exact physical-row completed-row replays",
                COMPLETED_ROW_REPLAYS,
                limits.max_completed_row_replays,
            ),
            (
                "exact physical-row frame replays",
                FRAME_REPLAYS,
                limits.max_frame_replays,
            ),
            (
                "exact physical-row parent allocation comparisons",
                PARENT_ALLOCATION_COMPARISONS,
                limits.max_parent_allocation_comparisons,
            ),
        ] {
            check_limit(resource, requested, limit)?;
        }

        source
            .replay(
                family,
                context,
                source.authority(),
                source.ordering(),
                source.schedule(),
                source.premises(),
                source.bound(),
            )
            .map_err(|_| GeneratedAffineResidualGroupExactPhysicalRowError::CompletedRow)?;
        let relation = source.relation();
        let authority = source.authority();

        if family.fingerprint_ref() != authority.family_fingerprint()
            || relation.family_fingerprint() != authority.family_fingerprint()
        {
            return Err(GeneratedAffineResidualGroupExactPhysicalRowError::WrongFamily);
        }
        if context.fingerprint() != authority.context_fingerprint()
            || relation.context_fingerprint() != authority.context_fingerprint()
        {
            return Err(GeneratedAffineResidualGroupExactPhysicalRowError::WrongContext);
        }
        if context.index_count() != authority.arity() || relation.arity() != authority.arity() {
            return Err(GeneratedAffineResidualGroupExactPhysicalRowError::WrongArity);
        }

        frame
            .replay_for_source_authority(family, context, authority)
            .map_err(map_frame_authentication_error)?;
        let source_case = authority
            .authenticated_source_neutral_case_view(context)
            .map_err(|_| GeneratedAffineResidualGroupExactPhysicalRowError::WrongCaseBinding)?;
        let source_group = authority
            .authenticated_source_neutral_group_view(context)
            .map_err(|_| GeneratedAffineResidualGroupExactPhysicalRowError::WrongGroupBinding)?;
        let source_case_ordinal = source_case.ordinal();
        let source_position = source_case.ordinal_within_group();
        let group_ordinal = source_group.ordinal();
        if source_case_ordinal != authority.case_ordinal()
            || source_case.group_ordinal() != authority.group_ordinal()
            || group_ordinal != authority.group_ordinal()
            || frame.group_ordinal() != group_ordinal
            || frame.case_ordinals().get(source_position).copied() != Some(source_case_ordinal)
        {
            return Err(GeneratedAffineResidualGroupExactPhysicalRowError::WrongGroupBinding);
        }
        let arity = source_group.ambient_arity();
        let terms = relation.terms().len();
        let guards = relation.guarded_nonzero_conditions().len();
        if relation.arity() != arity || frame.arity() != arity {
            return Err(GeneratedAffineResidualGroupExactPhysicalRowError::WrongArity);
        }
        if terms == 0 {
            return Err(GeneratedAffineResidualGroupExactPhysicalRowError::EmptyRelation);
        }
        for (resource, requested, limit) in [
            ("exact physical-row arity", arity, limits.max_arity),
            ("exact physical-row terms", terms, limits.max_terms),
            ("exact physical-row guards", guards, limits.max_guards),
        ] {
            check_limit(resource, requested, limit)?;
        }

        let local_shift_component_scans = checked_mul(
            "exact physical-row local shift component scans",
            terms,
            arity,
        )?;
        check_limit(
            "exact physical-row local shift component scans",
            local_shift_component_scans,
            limits.max_local_shift_component_scans,
        )?;
        let local_key_census_preflights =
            checked_mul("exact physical-row local-key census preflights", terms, 2)?;
        check_limit(
            "exact physical-row local-key census preflights",
            local_key_census_preflights,
            limits.max_local_key_census_preflights,
        )?;
        let (sort_log_height, sort_comparison_bound, sort_swap_bound) = preflight_heap_sort(terms)?;
        for (resource, requested, limit) in [
            (
                "exact physical-row sort log height",
                sort_log_height,
                limits.max_sort_log_height,
            ),
            (
                "exact physical-row sort comparison bound",
                sort_comparison_bound,
                limits.max_sort_comparison_bound,
            ),
            (
                "exact physical-row sort swap bound",
                sort_swap_bound,
                limits.max_sort_swap_bound,
            ),
        ] {
            check_limit(resource, requested, limit)?;
        }

        // Census every deep payload before the first coefficient or guard is
        // cloned. The source row has already been fully replayed and is kept
        // alive by `source`, so these are authenticated borrowed scans.
        let mut coefficient_clone_retained_bytes = 0usize;
        for coefficient in relation.terms().values() {
            coefficient_clone_retained_bytes = bounded_add(
                "exact physical-row coefficient clone retained bytes",
                coefficient_clone_retained_bytes,
                coefficient.owned_retained_byte_bound().ok_or(
                    GeneratedAffineResidualGroupExactPhysicalRowError::ResourceCountOverflow {
                        resource: "exact physical-row coefficient clone retained bytes",
                    },
                )?,
                limits.max_coefficient_clone_retained_bytes,
            )?;
        }
        let mut guard_clone_retained_bytes = 0usize;
        let mut guard_origin_occurrences = 0usize;
        for guard in relation.guarded_nonzero_conditions() {
            guard_clone_retained_bytes = bounded_add(
                "exact physical-row guard clone retained bytes",
                guard_clone_retained_bytes,
                guard.owned_retained_byte_bound().ok_or(
                    GeneratedAffineResidualGroupExactPhysicalRowError::ResourceCountOverflow {
                        resource: "exact physical-row guard clone retained bytes",
                    },
                )?,
                limits.max_guard_clone_retained_bytes,
            )?;
            guard_origin_occurrences = bounded_add(
                "exact physical-row guard origin occurrences",
                guard_origin_occurrences,
                guard.origins().len(),
                limits.max_guard_origin_occurrences,
            )?;
        }

        let mut stats = GeneratedAffineResidualGroupExactPhysicalRowStats {
            completed_row_replays: COMPLETED_ROW_REPLAYS,
            frame_replays: FRAME_REPLAYS,
            parent_allocation_comparisons: PARENT_ALLOCATION_COMPARISONS,
            arity,
            terms,
            guards,
            guard_origin_occurrences,
            local_shift_component_scans,
            local_key_census_preflights,
            sort_log_height,
            sort_comparison_bound,
            sort_swap_bound,
            coefficient_clone_retained_bytes,
            guard_clone_retained_bytes,
            ..Default::default()
        };

        // First pass: scan borrowed local keys and frame geometry only. This
        // must finish every aggregate owner/sort/native admission before
        // `physical_from_local` can allocate a GMP-backed shift.
        let mut maximum_prospective_key_bytes = 0usize;
        let mut maximum_prospective_comparison_integer_bit_work = 0usize;
        let mut one_pass_census_component_scans = 0usize;
        let mut one_pass_census_integer_bit_work = 0usize;
        for local in relation.terms().keys() {
            let census =
                frame.preflight_key_for_local(source_position, source_case_ordinal, local)?;
            one_pass_census_component_scans = checked_add(
                "exact physical-row local-key census component scans",
                one_pass_census_component_scans,
                census.component_scans(),
            )?;
            one_pass_census_integer_bit_work = checked_add(
                "exact physical-row local-key census integer-bit work",
                one_pass_census_integer_bit_work,
                census.integer_bit_work(),
            )?;
            stats.physical_key_prospective_integer_bits = bounded_add(
                "exact physical-row physical-key prospective integer bits",
                stats.physical_key_prospective_integer_bits,
                census.prospective_retained_integer_bits(),
                limits.max_physical_key_prospective_integer_bits,
            )?;
            stats.physical_key_prospective_retained_bytes = bounded_add(
                "exact physical-row physical-key prospective retained bytes",
                stats.physical_key_prospective_retained_bytes,
                census.prospective_retained_bytes(),
                limits.max_physical_key_prospective_retained_bytes,
            )?;
            maximum_prospective_key_bytes =
                maximum_prospective_key_bytes.max(census.prospective_retained_bytes());
            maximum_prospective_comparison_integer_bit_work =
                maximum_prospective_comparison_integer_bit_work
                    .max(census.prospective_comparison_integer_bit_work());
        }
        stats.local_key_census_component_scans = checked_mul(
            "exact physical-row local-key census component scans",
            one_pass_census_component_scans,
            2,
        )?;
        stats.local_key_census_integer_bit_work = checked_mul(
            "exact physical-row local-key census integer-bit work",
            one_pass_census_integer_bit_work,
            2,
        )?;
        check_limit(
            "exact physical-row local-key census component scans",
            stats.local_key_census_component_scans,
            limits.max_local_key_census_component_scans,
        )?;
        check_limit(
            "exact physical-row local-key census integer-bit work",
            stats.local_key_census_integer_bit_work,
            limits.max_local_key_census_integer_bit_work,
        )?;

        stats.sort_integer_bit_work_bound = checked_mul(
            "exact physical-row sort integer-bit work bound",
            sort_comparison_bound,
            checked_mul(
                "exact physical-row sort integer-bit work bound",
                2,
                maximum_prospective_comparison_integer_bit_work,
            )?,
        )?;
        check_limit(
            "exact physical-row sort integer-bit work bound",
            stats.sort_integer_bit_work_bound,
            limits.max_sort_integer_bit_work_bound,
        )?;

        let planned_structural_owner_bytes = prospective_owner_structural_bytes(terms, guards)?;
        let planned_owner_retained_bytes = checked_sum(
            "exact physical-row prospective owner retained bytes",
            [
                planned_structural_owner_bytes,
                stats.physical_key_prospective_retained_bytes,
                coefficient_clone_retained_bytes,
                guard_clone_retained_bytes,
            ],
        )?;
        check_limit(
            "exact physical-row prospective owner retained bytes",
            planned_owner_retained_bytes,
            limits.max_prospective_owner_retained_bytes,
        )?;
        // The same conservative census also admits the final owner ceiling.
        // Waiting for observed keys would make a too-small final-owner limit
        // fail only after GMP-backed physical shifts had already been built.
        check_limit(
            "exact physical-row owner retained bytes",
            planned_owner_retained_bytes,
            limits.max_owner_retained_bytes,
        )?;
        let maximum_clone_payload = relation
            .terms()
            .values()
            .filter_map(ParametricCoefficient::owned_retained_byte_bound)
            .chain(
                relation
                    .guarded_nonzero_conditions()
                    .iter()
                    .filter_map(ParametricNonZeroCondition::owned_retained_byte_bound),
            )
            .max()
            .unwrap_or(0);
        let planned_native_temporary_byte_envelope = checked_sum(
            "exact physical-row native temporary byte envelope",
            [
                planned_owner_retained_bytes,
                maximum_prospective_key_bytes,
                maximum_clone_payload,
                checked_mul(
                    "exact physical-row native temporary byte envelope",
                    sort_log_height,
                    size_of::<usize>(),
                )?,
            ],
        )?;
        check_limit(
            "exact physical-row native temporary byte envelope",
            planned_native_temporary_byte_envelope,
            limits.max_native_temporary_byte_envelope,
        )?;

        let mut exact_terms = try_vec_with_capacity::<GeneratedAffineResidualGroupExactPhysicalTerm>(
            "exact physical-row terms",
            terms,
        )?;
        let mut exact_guards = try_vec_with_capacity::<ParametricNonZeroCondition>(
            "exact physical-row guards",
            guards,
        )?;

        // `try_reserve_exact` is fallible but may retain more capacity than
        // requested. Re-admit the observed buffers immediately, before any
        // key construction or deep coefficient/guard clone.
        stats.output_vector_retained_bytes = checked_sum(
            "exact physical-row output-vector retained bytes",
            [
                arc_vec_retained_bytes_bound::<GeneratedAffineResidualGroupExactPhysicalTerm>(
                    exact_terms.capacity(),
                )?,
                arc_vec_retained_bytes_bound::<ParametricNonZeroCondition>(
                    exact_guards.capacity(),
                )?,
            ],
        )?;
        check_limit(
            "exact physical-row output-vector retained bytes",
            stats.output_vector_retained_bytes,
            limits.max_output_vector_retained_bytes,
        )?;
        let observed_structural_owner_bytes =
            prospective_owner_structural_bytes(exact_terms.capacity(), exact_guards.capacity())?;
        stats.prospective_owner_retained_bytes = checked_sum(
            "exact physical-row prospective owner retained bytes",
            [
                observed_structural_owner_bytes,
                stats.physical_key_prospective_retained_bytes,
                coefficient_clone_retained_bytes,
                guard_clone_retained_bytes,
            ],
        )?;
        check_limit(
            "exact physical-row prospective owner retained bytes",
            stats.prospective_owner_retained_bytes,
            limits.max_prospective_owner_retained_bytes,
        )?;
        check_limit(
            "exact physical-row owner retained bytes",
            stats.prospective_owner_retained_bytes,
            limits.max_owner_retained_bytes,
        )?;
        stats.native_temporary_byte_envelope = checked_sum(
            "exact physical-row native temporary byte envelope",
            [
                stats.prospective_owner_retained_bytes,
                maximum_prospective_key_bytes,
                maximum_clone_payload,
                checked_mul(
                    "exact physical-row native temporary byte envelope",
                    sort_log_height,
                    size_of::<usize>(),
                )?,
            ],
        )?;
        check_limit(
            "exact physical-row native temporary byte envelope",
            stats.native_temporary_byte_envelope,
            limits.max_native_temporary_byte_envelope,
        )?;

        let mut actual_prospective_integer_bits = 0usize;
        let mut actual_prospective_retained_bytes = 0usize;
        for (local, coefficient) in relation.terms() {
            let local_census =
                frame.preflight_key_for_local(source_position, source_case_ordinal, local)?;
            let physical =
                frame.physical_from_local(source_position, source_case_ordinal, local)?;
            stats.physical_key_preflights = bounded_add(
                "exact physical-row physical-key preflights",
                stats.physical_key_preflights,
                1,
                limits.max_physical_key_preflights,
            )?;
            let preflight = frame.preflight_key_for_physical(&physical)?;
            if !local_census.authenticates_physical_preflight(&preflight) {
                return Err(GeneratedAffineResidualGroupExactPhysicalRowError::ReplayMismatch);
            }
            stats.physical_key_component_scans = bounded_add(
                "exact physical-row physical-key component scans",
                stats.physical_key_component_scans,
                preflight.component_scans(),
                limits.max_physical_key_component_scans,
            )?;
            stats.physical_key_integer_bit_work = bounded_add(
                "exact physical-row physical-key integer-bit work",
                stats.physical_key_integer_bit_work,
                preflight.integer_bit_work(),
                limits.max_physical_key_integer_bit_work,
            )?;
            actual_prospective_integer_bits = checked_add(
                "exact physical-row actual prospective integer bits",
                actual_prospective_integer_bits,
                preflight.prospective_retained_integer_bits(),
            )?;
            actual_prospective_retained_bytes = checked_add(
                "exact physical-row actual prospective retained bytes",
                actual_prospective_retained_bytes,
                preflight.prospective_retained_bytes(),
            )?;
            stats.physical_key_constructions = bounded_add(
                "exact physical-row physical-key constructions",
                stats.physical_key_constructions,
                1,
                limits.max_physical_key_constructions,
            )?;
            let key = frame.key_for_preflight(preflight)?;
            stats.physical_key_retained_bytes = checked_add(
                "exact physical-row physical-key retained bytes",
                stats.physical_key_retained_bytes,
                key.retained_bytes(),
            )?;
            exact_terms.push(GeneratedAffineResidualGroupExactPhysicalTerm {
                key,
                coefficient: coefficient.clone(),
            });
        }
        if actual_prospective_integer_bits > stats.physical_key_prospective_integer_bits
            || actual_prospective_retained_bytes > stats.physical_key_prospective_retained_bytes
        {
            return Err(GeneratedAffineResidualGroupExactPhysicalRowError::ReplayMismatch);
        }

        let (mut sort_comparisons, sort_swaps, mut sort_integer_bit_work) =
            heap_sort_terms(&mut exact_terms)?;
        for pair in exact_terms.windows(2) {
            match counted_key_cmp(
                &pair[0].key,
                &pair[1].key,
                &mut sort_comparisons,
                &mut sort_integer_bit_work,
            )? {
                Ordering::Less => {}
                Ordering::Equal => {
                    return Err(
                        GeneratedAffineResidualGroupExactPhysicalRowError::DuplicatePhysicalKey,
                    );
                }
                Ordering::Greater => {
                    return Err(GeneratedAffineResidualGroupExactPhysicalRowError::ReplayMismatch);
                }
            }
        }
        stats.sort_comparisons = sort_comparisons;
        stats.sort_swaps = sort_swaps;
        stats.sort_integer_bit_work = sort_integer_bit_work;
        if sort_comparisons > sort_comparison_bound || sort_swaps > sort_swap_bound {
            return Err(GeneratedAffineResidualGroupExactPhysicalRowError::ReplayMismatch);
        }
        if sort_integer_bit_work > stats.sort_integer_bit_work_bound {
            return Err(GeneratedAffineResidualGroupExactPhysicalRowError::ReplayMismatch);
        }

        exact_guards.extend(relation.guarded_nonzero_conditions().iter().cloned());
        stats.owner_retained_bytes = observed_owner_retained_bytes(&exact_terms, &exact_guards)?;
        check_limit(
            "exact physical-row owner retained bytes",
            stats.owner_retained_bytes,
            limits.max_owner_retained_bytes,
        )?;
        if stats.owner_retained_bytes > stats.prospective_owner_retained_bytes {
            return Err(GeneratedAffineResidualGroupExactPhysicalRowError::ReplayMismatch);
        }

        let expanded_ordinal = source.expanded_ordinal();
        let layer_ordinal = source.layer_ordinal();
        let point_depth = source.point_depth();
        let point_ordinal = source.point_ordinal();
        let source_row_ordinal = source.source_row_ordinal();
        Ok(GeneratedAffineResidualGroupExactPhysicalRow {
            schema: GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_PHYSICAL_ROW_V2_SCHEMA,
            source,
            frame,
            source_case_ordinal,
            group_ordinal,
            expanded_ordinal,
            layer_ordinal,
            point_depth,
            point_ordinal,
            source_row_ordinal,
            terms: Arc::new(exact_terms),
            guards: Arc::new(exact_guards),
            limits,
            stats,
        })
    }
}

fn map_frame_authentication_error(
    error: GeneratedAffineResidualGroupPhysicalKeyError,
) -> GeneratedAffineResidualGroupExactPhysicalRowError {
    use GeneratedAffineResidualGroupPhysicalKeyError as Physical;
    match error {
        Physical::WrongAuthorityAllocation | Physical::WrongFrameAllocation => {
            GeneratedAffineResidualGroupExactPhysicalRowError::WrongParentAllocation
        }
        Physical::WrongFamily => GeneratedAffineResidualGroupExactPhysicalRowError::WrongFamily,
        Physical::WrongContext => GeneratedAffineResidualGroupExactPhysicalRowError::WrongContext,
        Physical::WrongArity { .. } => {
            GeneratedAffineResidualGroupExactPhysicalRowError::WrongArity
        }
        Physical::WrongGroup | Physical::WrongCase | Physical::WrongCasePosition => {
            GeneratedAffineResidualGroupExactPhysicalRowError::WrongGroupBinding
        }
        _ => GeneratedAffineResidualGroupExactPhysicalRowError::PhysicalKey,
    }
}

fn exact_physical_rows_equal(
    left: &GeneratedAffineResidualGroupExactPhysicalRow,
    right: &GeneratedAffineResidualGroupExactPhysicalRow,
) -> bool {
    left.schema == right.schema
        && Arc::ptr_eq(&left.source, &right.source)
        && Arc::ptr_eq(&left.frame, &right.frame)
        && left.source_case_ordinal == right.source_case_ordinal
        && left.group_ordinal == right.group_ordinal
        && left.expanded_ordinal == right.expanded_ordinal
        && left.layer_ordinal == right.layer_ordinal
        && left.point_depth == right.point_depth
        && left.point_ordinal == right.point_ordinal
        && left.source_row_ordinal == right.source_row_ordinal
        && left.terms == right.terms
        && left.guards == right.guards
        && left.limits == right.limits
        && left.stats == right.stats
}

fn preflight_heap_sort(
    terms: usize,
) -> Result<(usize, usize, usize), GeneratedAffineResidualGroupExactPhysicalRowError> {
    let log_height = ceil_log2(terms.max(1));
    // Bottom-up heap construction and extraction each traverse at most one
    // root-to-leaf path per element; every level performs at most two key
    // comparisons. The deliberately loose factor four covers both phases.
    let heap_comparison_bound = checked_mul(
        "exact physical-row sort comparison bound",
        checked_mul("exact physical-row sort comparison bound", 4, terms)?,
        log_height,
    )?;
    let comparison_bound = checked_add(
        "exact physical-row sort comparison bound",
        heap_comparison_bound,
        terms.saturating_sub(1),
    )?;
    let swap_bound = checked_add(
        "exact physical-row sort swap bound",
        checked_mul(
            "exact physical-row sort swap bound",
            checked_mul("exact physical-row sort swap bound", 2, terms)?,
            log_height,
        )?,
        terms,
    )?;
    Ok((log_height, comparison_bound, swap_bound))
}

fn heap_sort_terms(
    terms: &mut [GeneratedAffineResidualGroupExactPhysicalTerm],
) -> Result<(usize, usize, usize), GeneratedAffineResidualGroupExactPhysicalRowError> {
    let mut comparisons = 0usize;
    let mut swaps = 0usize;
    let mut integer_bit_work = 0usize;
    let len = terms.len();
    for root in (0..len / 2).rev() {
        sift_down(
            terms,
            root,
            len,
            &mut comparisons,
            &mut swaps,
            &mut integer_bit_work,
        )?;
    }
    for end in (1..len).rev() {
        terms.swap(0, end);
        swaps = checked_add("exact physical-row sort swaps", swaps, 1)?;
        sift_down(
            terms,
            0,
            end,
            &mut comparisons,
            &mut swaps,
            &mut integer_bit_work,
        )?;
    }
    Ok((comparisons, swaps, integer_bit_work))
}

fn sift_down(
    terms: &mut [GeneratedAffineResidualGroupExactPhysicalTerm],
    mut root: usize,
    end: usize,
    comparisons: &mut usize,
    swaps: &mut usize,
    integer_bit_work: &mut usize,
) -> Result<(), GeneratedAffineResidualGroupExactPhysicalRowError> {
    loop {
        let Some(mut child) = root.checked_mul(2).and_then(|value| value.checked_add(1)) else {
            return Err(
                GeneratedAffineResidualGroupExactPhysicalRowError::ResourceCountOverflow {
                    resource: "exact physical-row sort child index",
                },
            );
        };
        if child >= end {
            return Ok(());
        }
        if child + 1 < end {
            if counted_key_cmp(
                &terms[child].key,
                &terms[child + 1].key,
                comparisons,
                integer_bit_work,
            )? == Ordering::Less
            {
                child += 1;
            }
        }
        if counted_key_cmp(
            &terms[root].key,
            &terms[child].key,
            comparisons,
            integer_bit_work,
        )? != Ordering::Less
        {
            return Ok(());
        }
        terms.swap(root, child);
        *swaps = checked_add("exact physical-row sort swaps", *swaps, 1)?;
        root = child;
    }
}

fn counted_key_cmp(
    left: &GeneratedAffineResidualGroupPhysicalKey,
    right: &GeneratedAffineResidualGroupPhysicalKey,
    comparisons: &mut usize,
    integer_bit_work: &mut usize,
) -> Result<Ordering, GeneratedAffineResidualGroupExactPhysicalRowError> {
    *comparisons = checked_add("exact physical-row sort comparisons", *comparisons, 1)?;
    *integer_bit_work = checked_add(
        "exact physical-row sort integer-bit work",
        *integer_bit_work,
        left.comparison_integer_bit_work(right)?,
    )?;
    Ok(left.cmp(right))
}

fn prospective_owner_structural_bytes(
    terms: usize,
    guards: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactPhysicalRowError> {
    checked_sum(
        "exact physical-row owner retained bytes",
        [
            size_of::<GeneratedAffineResidualGroupExactPhysicalRow>(),
            arc_vec_retained_bytes_bound::<GeneratedAffineResidualGroupExactPhysicalTerm>(terms)?,
            arc_vec_retained_bytes_bound::<ParametricNonZeroCondition>(guards)?,
        ],
    )
}

fn observed_owner_retained_bytes(
    terms: &Vec<GeneratedAffineResidualGroupExactPhysicalTerm>,
    guards: &Vec<ParametricNonZeroCondition>,
) -> Result<usize, GeneratedAffineResidualGroupExactPhysicalRowError> {
    let mut bytes = prospective_owner_structural_bytes(terms.capacity(), guards.capacity())?;
    for term in terms {
        bytes = checked_add(
            "exact physical-row owner retained bytes",
            bytes,
            term.key.retained_bytes(),
        )?;
        bytes = checked_add(
            "exact physical-row owner retained bytes",
            bytes,
            term.coefficient.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactPhysicalRowError::ResourceCountOverflow {
                    resource: "exact physical-row owner retained bytes",
                },
            )?,
        )?;
    }
    for guard in guards {
        bytes = checked_add(
            "exact physical-row owner retained bytes",
            bytes,
            guard.owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualGroupExactPhysicalRowError::ResourceCountOverflow {
                    resource: "exact physical-row owner retained bytes",
                },
            )?,
        )?;
    }
    Ok(bytes)
}

fn arc_vec_retained_bytes_bound<T>(
    capacity: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactPhysicalRowError> {
    checked_add(
        "exact physical-row Arc<Vec> retained bytes",
        checked_add(
            "exact physical-row Arc<Vec> retained bytes",
            checked_mul(
                "exact physical-row Arc<Vec> retained bytes",
                2,
                size_of::<AtomicUsize>(),
            )?,
            checked_add(
                "exact physical-row Arc<Vec> retained bytes",
                align_of::<Vec<T>>().saturating_sub(1),
                size_of::<Vec<T>>(),
            )?,
        )?,
        checked_mul(
            "exact physical-row Arc<Vec> retained bytes",
            capacity,
            size_of::<T>(),
        )?,
    )
}

fn try_vec_with_capacity<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, GeneratedAffineResidualGroupExactPhysicalRowError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        GeneratedAffineResidualGroupExactPhysicalRowError::AllocationFailure { resource }
    })?;
    Ok(values)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualGroupExactPhysicalRowError> {
    if requested > limit {
        Err(
            GeneratedAffineResidualGroupExactPhysicalRowError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

fn bounded_add(
    resource: &'static str,
    current: usize,
    increment: usize,
    limit: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactPhysicalRowError> {
    let requested = checked_add(resource, current, increment)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactPhysicalRowError> {
    left.checked_add(right).ok_or(
        GeneratedAffineResidualGroupExactPhysicalRowError::ResourceCountOverflow { resource },
    )
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactPhysicalRowError> {
    left.checked_mul(right).ok_or(
        GeneratedAffineResidualGroupExactPhysicalRowError::ResourceCountOverflow { resource },
    )
}

fn checked_sum(
    resource: &'static str,
    values: impl IntoIterator<Item = usize>,
) -> Result<usize, GeneratedAffineResidualGroupExactPhysicalRowError> {
    values
        .into_iter()
        .try_fold(0usize, |total, value| checked_add(resource, total, value))
}

fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

#[cfg(test)]
impl GeneratedAffineResidualGroupExactPhysicalRowCompiler {
    /// Legacy/scouting compatibility used only by differential tests. The
    /// returned physical row owns only the completed per-row certificate.
    pub(crate) fn compile_from_reelimination_for_test(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: Arc<
            crate::generated_affine_residual_case_reelimination::GeneratedAffineResidualCaseReeliminationCertificate,
        >,
        retained_row_ordinal: usize,
        witness_ordinal: usize,
        frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        limits: GeneratedAffineResidualGroupExactPhysicalRowLimits,
    ) -> Result<
        GeneratedAffineResidualGroupExactPhysicalRow,
        GeneratedAffineResidualGroupExactPhysicalRowError,
    > {
        let completed = source
            .compile_completed_retained_source_row(
                family,
                context,
                retained_row_ordinal,
                witness_ordinal,
                crate::generated_affine_residual_case_completed_bound_row::GeneratedAffineResidualCaseCompletedBoundRowLimits::default(),
            )
            .map_err(|_| GeneratedAffineResidualGroupExactPhysicalRowError::CompletedRow)?;
        Self::compile(family, context, Arc::new(completed), frame, limits)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use symbolica::prelude::Integer;

    use super::super::physical_key::{
        GeneratedAffineResidualGroupPhysicalKeyLimits, physical_from_local_executions_for_test,
        reset_physical_from_local_executions_for_test, test_integer_field_comparison_bit_work,
    };
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
    use crate::generated_affine_residual_case_completed_bound_row::{
        GeneratedAffineResidualCaseCompletedBoundRow,
        GeneratedAffineResidualCaseCompletedBoundRowLimits,
    };
    use crate::generated_affine_residual_case_premises::{
        GeneratedAffineResidualCasePremisesLimits, GeneratedAffineResidualCasePremisesOutcome,
        compile_generated_affine_residual_case_premises,
    };
    use crate::generated_affine_residual_case_reelimination::{
        GeneratedAffineResidualCaseReeliminationCertificate,
        GeneratedAffineResidualCaseReeliminationCompilation,
        GeneratedAffineResidualCaseReeliminationCompiler,
        GeneratedAffineResidualCaseReeliminationLimits,
        GeneratedAffineResidualCaseReeliminationRowOutcome,
    };
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
    use crate::solver::closure::case_inventory::{
        GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityLimits,
        GeneratedAffineResidualCaseInventoryCertificate,
        GeneratedAffineResidualCaseInventoryCompiler, GeneratedAffineResidualCaseInventoryLimits,
    };
    use crate::{
        AffineDenominator, GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
        GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits, GuardOrigin,
        IntegralOrderingPolicy, ParametricIbpGenerator, SectorMask, algebra::CoefficientContext,
    };

    struct Fixture {
        family: IntegralFamily,
        context: ParametricCoefficientContext,
        inventory: Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        anchor: Arc<GeneratedAffineResidualCaseAuthority>,
        frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
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

    /// The first propagator is divided by `m2`. Its zero locus is still the
    /// same equal-mass propagator, while the authenticated family domain now
    /// carries the non-vacuous base-field assumption `m2 != 0`.
    fn rescaled_equal_mass_two_loop_family(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_one = coefficients.integer(-1);
        let inverse_m2 = coefficients.parse("1/m2").unwrap();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(minus_one, vec![inverse_m2, zero.clone(), zero.clone()]),
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
        fixture_for_case_group(name, None)
    }

    fn fixture_for_case_group(name: &str, requested_case: Option<usize>) -> Fixture {
        fixture_from_family(equal_mass_two_loop_family(name), requested_case)
    }

    fn fixture_from_family(family: IntegralFamily, requested_case: Option<usize>) -> Fixture {
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
        let group_ordinal = if let Some(case_ordinal) = requested_case {
            GeneratedAffineResidualCaseAuthority::try_new(
                &family,
                &context,
                Arc::clone(&inventory),
                case_ordinal,
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap()
            .group_ordinal()
        } else {
            (0..inventory.group_count())
                .max_by_key(|&ordinal| {
                    inventory
                        .authenticated_group_view(&context, ordinal)
                        .unwrap()
                        .case_ordinals()
                        .len()
                })
                .unwrap()
        };
        let group = inventory
            .authenticated_group_view(&context, group_ordinal)
            .unwrap();
        if requested_case.is_none() {
            assert_eq!(group.case_ordinals(), [1, 3]);
        } else {
            assert!(group.case_ordinals().contains(&requested_case.unwrap()));
        }
        let anchor = Arc::new(
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
                Arc::clone(&anchor),
                GeneratedAffineResidualGroupPhysicalKeyLimits::default(),
            )
            .unwrap(),
        );
        Fixture {
            family,
            context,
            inventory,
            anchor,
            frame,
        }
    }

    fn production_source_for_case(
        fixture: &Fixture,
        case_ordinal: usize,
    ) -> Option<Arc<GeneratedAffineResidualCaseCompletedBoundRow>> {
        production_source_for_case_at_depth(fixture, case_ordinal, 0)
    }

    fn production_source_for_case_at_depth(
        fixture: &Fixture,
        case_ordinal: usize,
        maximum_depth: usize,
    ) -> Option<Arc<GeneratedAffineResidualCaseCompletedBoundRow>> {
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                &fixture.family,
                &fixture.context,
                Arc::clone(&fixture.inventory),
                case_ordinal,
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        production_source_for_authority_at_depth(fixture, authority, maximum_depth)
    }

    fn production_source_for_authority_at_depth(
        fixture: &Fixture,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        maximum_depth: usize,
    ) -> Option<Arc<GeneratedAffineResidualCaseCompletedBoundRow>> {
        let (certificate, retained_row_ordinal, witness_ordinal) =
            legacy_source_for_authority_at_depth(fixture, authority, maximum_depth)?;
        Some(Arc::new(
            certificate
                .compile_completed_retained_source_row(
                    &fixture.family,
                    &fixture.context,
                    retained_row_ordinal,
                    witness_ordinal,
                    GeneratedAffineResidualCaseCompletedBoundRowLimits::default(),
                )
                .unwrap(),
        ))
    }

    fn legacy_source_for_case_at_depth(
        fixture: &Fixture,
        case_ordinal: usize,
        maximum_depth: usize,
    ) -> Option<(
        Arc<GeneratedAffineResidualCaseReeliminationCertificate>,
        usize,
        usize,
    )> {
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                &fixture.family,
                &fixture.context,
                Arc::clone(&fixture.inventory),
                case_ordinal,
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        legacy_source_for_authority_at_depth(fixture, authority, maximum_depth)
    }

    fn legacy_source_for_authority_at_depth(
        fixture: &Fixture,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        maximum_depth: usize,
    ) -> Option<(
        Arc<GeneratedAffineResidualCaseReeliminationCertificate>,
        usize,
        usize,
    )> {
        let premises = match compile_generated_affine_residual_case_premises(
            &fixture.family,
            &fixture.context,
            Arc::clone(&authority),
            GeneratedAffineResidualCasePremisesLimits::default(),
        )
        .unwrap()
        {
            GeneratedAffineResidualCasePremisesOutcome::Ready(value) => Arc::new(value),
            GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(_) => {
                return None;
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
                maximum_depth,
                GeneratedAffinePreparePointScheduleLimits::default(),
            )
            .unwrap(),
        );
        let compilation = GeneratedAffineResidualCaseReeliminationCompiler::compile(
            &fixture.family,
            &fixture.context,
            authority,
            premises,
            ordering,
            schedule,
            GeneratedAffineResidualCaseReeliminationLimits::default(),
        )
        .unwrap();
        let GeneratedAffineResidualCaseReeliminationCompilation::Eliminated(certificate) =
            compilation
        else {
            return None;
        };
        let certificate = Arc::new(certificate);
        let witness_ordinal = certificate
            .witnesses()
            .iter()
            .position(|witness| witness.outcome().is_retained())?;
        let retained_row_ordinal = certificate.witnesses()[..witness_ordinal]
            .iter()
            .filter(|witness| witness.outcome().is_retained())
            .count();
        Some((certificate, retained_row_ordinal, witness_ordinal))
    }

    fn production_source(fixture: &Fixture) -> Arc<GeneratedAffineResidualCaseCompletedBoundRow> {
        fixture
            .frame
            .case_ordinals()
            .iter()
            .find_map(|&case_ordinal| production_source_for_case(fixture, case_ordinal))
            .expect("concrete equal-mass fixture produced no authenticated retained row")
    }

    fn production_guarded_source(
        fixture: &Fixture,
    ) -> Arc<GeneratedAffineResidualCaseCompletedBoundRow> {
        for &case_ordinal in fixture.frame.case_ordinals() {
            let Some((certificate, _, _)) =
                legacy_source_for_case_at_depth(fixture, case_ordinal, 1)
            else {
                continue;
            };
            let mut retained_row_ordinal = 0usize;
            for (witness_ordinal, witness) in certificate.witnesses().iter().enumerate() {
                let GeneratedAffineResidualCaseReeliminationRowOutcome::Retained(bound) =
                    witness.outcome()
                else {
                    continue;
                };
                if !bound.base_assumptions().is_empty() {
                    let authenticated = certificate
                        .authenticate_retained_source_row(retained_row_ordinal, witness_ordinal)
                        .unwrap();
                    if !authenticated
                        .relation()
                        .guarded_nonzero_conditions()
                        .is_empty()
                    {
                        return Arc::new(
                            certificate
                                .compile_completed_retained_source_row(
                                    &fixture.family,
                                    &fixture.context,
                                    retained_row_ordinal,
                                    witness_ordinal,
                                    GeneratedAffineResidualCaseCompletedBoundRowLimits::default(),
                                )
                                .unwrap(),
                        );
                    }
                }
                retained_row_ordinal += 1;
            }
        }
        panic!("concrete equal-mass fixture produced no row-local base-assumption guard")
    }

    #[test]
    fn retained_source_graph_deduplicates_exact_frame_authority_allocation() {
        let fixture = fixture("exact-physical-row-shared-anchor-memory");
        let source =
            production_source_for_authority_at_depth(&fixture, Arc::clone(&fixture.anchor), 0)
                .expect("the concrete anchor case must produce an authenticated retained row");
        assert!(fixture.frame.same_authority_allocation(source.authority()));

        let row = Arc::new(
            GeneratedAffineResidualGroupExactPhysicalRowCompiler::compile(
                &fixture.family,
                &fixture.context,
                Arc::clone(&source),
                Arc::clone(&fixture.frame),
                GeneratedAffineResidualGroupExactPhysicalRowLimits::default(),
            )
            .unwrap(),
        );
        let shared_source_graph = source
            .retained_source_graph_byte_bound(false)
            .expect("the finite shared-authority graph must fit in usize");
        let independently_owned_source_graph = source
            .retained_source_graph_byte_bound(true)
            .expect("the finite independently owned graph must fit in usize");
        assert!(independently_owned_source_graph > shared_source_graph);

        let expected = row
            .stats()
            .owner_retained_bytes()
            .checked_add(
                arc_control_and_padding_byte_bound::<GeneratedAffineResidualGroupExactPhysicalRow>(
                )
                .unwrap(),
            )
            .and_then(|bytes| {
                bytes.checked_add(
                    arc_control_and_padding_byte_bound::<
                        GeneratedAffineResidualCaseCompletedBoundRow,
                    >()
                    .unwrap(),
                )
            })
            .and_then(|bytes| bytes.checked_add(shared_source_graph))
            .unwrap();
        assert_eq!(
            row.unique_retained_source_graph_byte_bound().unwrap(),
            expected,
            "exact frame-authority identity must select the deduplicated source graph"
        );
    }

    #[test]
    fn production_row_is_an_exact_unrecentered_copy_in_common_coordinates() {
        let fixture = fixture_from_family(
            rescaled_equal_mass_two_loop_family("exact-physical-row-copy-private"),
            None,
        );
        let source = production_guarded_source(&fixture);
        let relation = source.relation();
        assert!(
            !relation.guarded_nonzero_conditions().is_empty(),
            "the concrete guard-copy oracle must not be vacuous"
        );
        let bound = source.bound();
        assert!(!bound.base_assumptions().is_empty());
        for assumption in bound.base_assumptions() {
            assert!(relation.guarded_nonzero_conditions().iter().any(|guard| {
                guard.polynomial() == assumption.condition().polynomial()
                    && assumption.condition().origins().is_subset(guard.origins())
            }));
        }
        let source_case = source
            .authority()
            .authenticated_case_view(&fixture.context)
            .unwrap();
        let group = source
            .authority()
            .authenticated_group_view(&fixture.context)
            .unwrap();
        let offset = group.anchor_offsets()[source_case.ordinal_within_group()].as_slice();

        let row = GeneratedAffineResidualGroupExactPhysicalRowCompiler::compile(
            &fixture.family,
            &fixture.context,
            Arc::clone(&source),
            Arc::clone(&fixture.frame),
            GeneratedAffineResidualGroupExactPhysicalRowLimits::default(),
        )
        .unwrap();

        // This oracle performs the coordinate addition directly from the
        // authenticated inventory geometry; it does not call the frame's
        // physicalization or key-construction implementation.
        let expected = relation
            .terms()
            .iter()
            .map(|(local, coefficient)| {
                let physical = offset
                    .iter()
                    .zip(local.values())
                    .map(|(base, delta)| base + Integer::from(*delta))
                    .collect::<Vec<_>>();
                (physical, coefficient.clone())
            })
            .collect::<BTreeMap<_, _>>();
        let actual = row
            .terms
            .iter()
            .map(|term| (term.key.shift().values().to_vec(), term.coefficient.clone()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(actual, expected);
        assert!(row.terms.windows(2).all(|pair| pair[0].key < pair[1].key));
        assert_eq!(row.guards.as_slice(), relation.guarded_nonzero_conditions());
        assert!(
            relation
                .guarded_nonzero_conditions()
                .iter()
                .flat_map(|guard| guard.origins())
                .all(|origin| !matches!(
                    origin,
                    GuardOrigin::GeneratedAffineGroupRecentering { .. }
                ))
        );
        assert!(
            row.guards
                .iter()
                .flat_map(|guard| guard.origins())
                .all(|origin| !matches!(
                    origin,
                    GuardOrigin::GeneratedAffineGroupRecentering { .. }
                ))
        );
        assert_eq!(row.source_case_ordinal(), source_case.ordinal());
        assert_eq!(row.group_ordinal(), group.ordinal());
        assert_eq!(row.expanded_ordinal(), source.expanded_ordinal());
        assert_eq!(row.layer_ordinal(), source.layer_ordinal());
        assert_eq!(row.point_depth(), source.point_depth());
        assert_eq!(row.point_ordinal(), source.point_ordinal());
        assert_eq!(row.source_row_ordinal(), source.source_row_ordinal());
        assert_eq!(row.term_count(), relation.terms().len());
        assert_eq!(
            row.guard_count(),
            relation.guarded_nonzero_conditions().len()
        );
        assert!(row.same_parent_allocations(&source, &fixture.frame));
        assert!(!row.is_recentered());
        assert!(!row.is_normalized());
        assert_eq!(row.targets_consumed(), 0);
        assert!(!row.publishes_rule());
        assert!(!row.infers_master());
    }

    #[test]
    fn non_anchor_case_uses_the_same_exact_frame_allocation_and_offset() {
        let fixture = fixture("exact-physical-row-non-anchor-private");
        let non_anchor_case = fixture
            .frame
            .case_ordinals()
            .iter()
            .copied()
            .find(|case| *case != fixture.frame.anchor_case_ordinal())
            .expect("fixture group has a concrete non-anchor case");
        let source = production_source_for_case(&fixture, non_anchor_case)
            .expect("concrete non-anchor case must retain a generated row");
        assert_eq!(source.authority().case_ordinal(), non_anchor_case);
        let source_case = source
            .authority()
            .authenticated_case_view(&fixture.context)
            .unwrap();
        let group = source
            .authority()
            .authenticated_group_view(&fixture.context)
            .unwrap();
        let offset = group.anchor_offsets()[source_case.ordinal_within_group()].as_slice();
        assert!(offset.iter().any(|value| !value.is_zero()));
        let relation = source.relation();

        let row = GeneratedAffineResidualGroupExactPhysicalRowCompiler::compile(
            &fixture.family,
            &fixture.context,
            Arc::clone(&source),
            Arc::clone(&fixture.frame),
            GeneratedAffineResidualGroupExactPhysicalRowLimits::default(),
        )
        .unwrap();
        let expected = relation
            .terms()
            .iter()
            .map(|(local, coefficient)| {
                let physical = offset
                    .iter()
                    .zip(local.values())
                    .map(|(base, delta)| base + Integer::from(*delta))
                    .collect::<Vec<_>>();
                (physical, coefficient.clone())
            })
            .collect::<BTreeMap<_, _>>();
        let actual = row
            .terms
            .iter()
            .map(|term| (term.key.shift().values().to_vec(), term.coefficient.clone()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(actual, expected);
        assert!(row.same_parent_allocations(&source, &fixture.frame));
        row.replay(&fixture.family, &fixture.context, &source, &fixture.frame)
            .unwrap();
    }

    #[test]
    fn production_auth_replay_and_exact_parent_allocations_are_mandatory() {
        let fixture = fixture("exact-physical-row-auth-private");
        let source = production_source(&fixture);
        let row = GeneratedAffineResidualGroupExactPhysicalRowCompiler::compile(
            &fixture.family,
            &fixture.context,
            Arc::clone(&source),
            Arc::clone(&fixture.frame),
            GeneratedAffineResidualGroupExactPhysicalRowLimits::default(),
        )
        .unwrap();
        row.replay(&fixture.family, &fixture.context, &source, &fixture.frame)
            .unwrap();

        let foreign_source = Arc::new(source.as_ref().clone());
        assert!(matches!(
            row.replay(
                &fixture.family,
                &fixture.context,
                &foreign_source,
                &fixture.frame,
            ),
            Err(GeneratedAffineResidualGroupExactPhysicalRowError::WrongParentAllocation)
        ));

        let value_equal_frame = Arc::new(fixture.frame.as_ref().clone());
        assert!(!Arc::ptr_eq(&fixture.frame, &value_equal_frame));
        assert!(!row.same_parent_allocations(&source, &value_equal_frame));
        assert!(matches!(
            row.replay(
                &fixture.family,
                &fixture.context,
                &source,
                &value_equal_frame,
            ),
            Err(GeneratedAffineResidualGroupExactPhysicalRowError::WrongParentAllocation)
        ));

        let compile_with = |limits| {
            GeneratedAffineResidualGroupExactPhysicalRowCompiler::compile(
                &fixture.family,
                &fixture.context,
                Arc::clone(&source),
                Arc::clone(&fixture.frame),
                limits,
            )
        };

        // Every first-pass and output-allocation boundary is exact at the
        // observed demand. One below must fail before the first GMP-backed
        // local-to-physical construction is executed.
        let census_bit_work = row.stats().local_key_census_integer_bit_work();
        assert!(census_bit_work > 0);
        let mut exact_census = GeneratedAffineResidualGroupExactPhysicalRowLimits::default();
        exact_census.max_local_key_census_integer_bit_work = census_bit_work;
        compile_with(exact_census).unwrap();
        exact_census.max_local_key_census_integer_bit_work = census_bit_work - 1;
        reset_physical_from_local_executions_for_test();
        assert!(matches!(
            compile_with(exact_census),
            Err(GeneratedAffineResidualGroupExactPhysicalRowError::ResourceLimit { .. })
        ));
        assert_eq!(physical_from_local_executions_for_test(), 0);

        let output_bytes = row.stats().output_vector_retained_bytes();
        assert!(output_bytes > 0);
        let mut exact_output = GeneratedAffineResidualGroupExactPhysicalRowLimits::default();
        exact_output.max_output_vector_retained_bytes = output_bytes;
        compile_with(exact_output).unwrap();
        exact_output.max_output_vector_retained_bytes = output_bytes - 1;
        reset_physical_from_local_executions_for_test();
        assert!(matches!(
            compile_with(exact_output),
            Err(GeneratedAffineResidualGroupExactPhysicalRowError::ResourceLimit { .. })
        ));
        assert_eq!(physical_from_local_executions_for_test(), 0);

        let prospective_owner_bytes = row.stats().prospective_owner_retained_bytes();
        assert!(prospective_owner_bytes > 0);
        let mut exact_prospective_owner =
            GeneratedAffineResidualGroupExactPhysicalRowLimits::default();
        exact_prospective_owner.max_prospective_owner_retained_bytes = prospective_owner_bytes;
        compile_with(exact_prospective_owner).unwrap();
        exact_prospective_owner.max_prospective_owner_retained_bytes = prospective_owner_bytes - 1;
        reset_physical_from_local_executions_for_test();
        assert!(matches!(
            compile_with(exact_prospective_owner),
            Err(GeneratedAffineResidualGroupExactPhysicalRowError::ResourceLimit { .. })
        ));
        assert_eq!(physical_from_local_executions_for_test(), 0);

        let mut exact_owner = GeneratedAffineResidualGroupExactPhysicalRowLimits::default();
        exact_owner.max_owner_retained_bytes = prospective_owner_bytes;
        compile_with(exact_owner).unwrap();
        exact_owner.max_owner_retained_bytes = prospective_owner_bytes - 1;
        reset_physical_from_local_executions_for_test();
        assert!(matches!(
            compile_with(exact_owner),
            Err(GeneratedAffineResidualGroupExactPhysicalRowError::ResourceLimit { .. })
        ));
        assert_eq!(physical_from_local_executions_for_test(), 0);

        let native_bytes = row.stats().native_temporary_byte_envelope();
        assert!(native_bytes > 0);
        let mut exact_native = GeneratedAffineResidualGroupExactPhysicalRowLimits::default();
        exact_native.max_native_temporary_byte_envelope = native_bytes;
        compile_with(exact_native).unwrap();
        exact_native.max_native_temporary_byte_envelope = native_bytes - 1;
        reset_physical_from_local_executions_for_test();
        assert!(matches!(
            compile_with(exact_native),
            Err(GeneratedAffineResidualGroupExactPhysicalRowError::ResourceLimit { .. })
        ));
        assert_eq!(physical_from_local_executions_for_test(), 0);

        let sort_bit_work = row.stats().sort_integer_bit_work_bound();
        assert!(sort_bit_work > 0);
        assert!(row.stats().sort_integer_bit_work() > 0);
        assert!(row.stats().sort_integer_bit_work() <= sort_bit_work);
        let mut exact_sort = GeneratedAffineResidualGroupExactPhysicalRowLimits::default();
        exact_sort.max_sort_integer_bit_work_bound = sort_bit_work;
        compile_with(exact_sort).unwrap();
        exact_sort.max_sort_integer_bit_work_bound = sort_bit_work - 1;
        reset_physical_from_local_executions_for_test();
        assert!(matches!(
            compile_with(exact_sort),
            Err(GeneratedAffineResidualGroupExactPhysicalRowError::ResourceLimit { .. })
        ));
        assert_eq!(physical_from_local_executions_for_test(), 0);
    }

    #[test]
    fn database_view_replays_exact_payload_and_remains_borrowed_and_inert() {
        let fixture = fixture_from_family(
            rescaled_equal_mass_two_loop_family("exact-physical-row-database-view-private"),
            None,
        );
        let source = production_guarded_source(&fixture);
        let row = GeneratedAffineResidualGroupExactPhysicalRowCompiler::compile(
            &fixture.family,
            &fixture.context,
            Arc::clone(&source),
            Arc::clone(&fixture.frame),
            GeneratedAffineResidualGroupExactPhysicalRowLimits::default(),
        )
        .unwrap();

        assert!(!row.guards.is_empty());
        let source_expanded_ordinal = source.expanded_ordinal();
        let source_row_ordinal = source.source_row_ordinal();
        let source_weak = Arc::downgrade(&source);
        drop(source);
        let source_owners = Arc::strong_count(&row.source);
        let frame_owners = Arc::strong_count(&fixture.frame);
        let view = row
            .replay_for_database(&fixture.family, &fixture.context, &fixture.frame)
            .unwrap();
        assert!(source_weak.upgrade().is_some());
        assert_eq!(Arc::strong_count(&row.source), source_owners);
        assert_eq!(Arc::strong_count(&fixture.frame), frame_owners);
        assert_eq!(
            std::mem::size_of_val(&view),
            std::mem::size_of::<&GeneratedAffineResidualGroupExactPhysicalRow>()
        );

        assert_eq!(view.source_case_ordinal(), row.source_case_ordinal());
        assert_eq!(view.group_ordinal(), row.group_ordinal());
        assert_eq!(view.expanded_ordinal(), source_expanded_ordinal);
        assert_eq!(view.source_row_ordinal(), source_row_ordinal);
        assert_eq!(view.term_count(), row.term_count());
        assert_eq!(view.guard_count(), row.guard_count());
        let borrowed_terms = view.terms();
        assert_eq!(borrowed_terms.len(), row.terms.len());
        for ((key, coefficient), stored) in borrowed_terms.zip(row.terms.iter()) {
            assert!(std::ptr::eq(key, &stored.key));
            assert!(std::ptr::eq(coefficient, &stored.coefficient));
            assert_eq!(key, &stored.key);
            assert_eq!(coefficient, &stored.coefficient);
        }
        assert_eq!(view.guards(), row.guards.as_slice());
        assert!(std::ptr::eq(
            view.guards().as_ptr(),
            row.guards.as_slice().as_ptr()
        ));

        let debug = format!("{view:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("database-view-private"));
        assert!(!debug.contains("m2"));
        assert!(debug.contains("recentered: false"));
        assert!(debug.contains("rule_published: false"));

        let wrong_family = equal_mass_two_loop_family("exact-physical-row-database-view-wrong");
        let wrong_context = ParametricIbpGenerator::try_new(&wrong_family)
            .unwrap()
            .context()
            .clone();
        assert!(
            row.replay_for_database(&wrong_family, &fixture.context, &fixture.frame)
                .is_err()
        );
        assert!(
            row.replay_for_database(&fixture.family, &wrong_context, &fixture.frame)
                .is_err()
        );

        // A value-equal allocation is not the database frame that owns this
        // row and must be rejected before a view can be obtained.
        let foreign_frame = Arc::new(fixture.frame.as_ref().clone());
        assert_eq!(foreign_frame.as_ref(), fixture.frame.as_ref());
        assert!(!Arc::ptr_eq(&foreign_frame, &fixture.frame));
        assert!(matches!(
            row.replay_for_database(&fixture.family, &fixture.context, &foreign_frame),
            Err(GeneratedAffineResidualGroupExactPhysicalRowError::WrongParentAllocation)
        ));

        // Lending the view cannot recenter, normalize, consume a target, or
        // publish any reduction state on its frozen source row.
        assert!(!view.is_recentered());
        assert!(!view.is_normalized());
        assert_eq!(view.targets_consumed(), 0);
        assert!(!view.publishes_rule());
        assert!(!view.infers_master());
    }

    #[test]
    fn zero_heavy_high_arity_integer_comparison_work_charges_every_field() {
        const ARITY: usize = 16_384;
        let left = vec![Integer::from(0); ARITY];
        let mut right = vec![Integer::from(0); ARITY];
        right[ARITY - 1] = Integer::from(1);

        // Each zero still costs one unit to inspect, as does the final one.
        // This directly exercises the shared helper used for both heap and
        // post-sort adjacent comparisons without needing a giant topology.
        assert_eq!(
            test_integer_field_comparison_bit_work(&left, &right).unwrap(),
            ARITY * 2
        );
    }
}
